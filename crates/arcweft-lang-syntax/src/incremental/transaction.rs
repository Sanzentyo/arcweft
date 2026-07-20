//! Private staging for grammar identity and typed attachment.

use core::num::NonZeroU64;
use std::sync::Arc;

use arcweft_source::SourceDocument;
use arcweft_source::identity::SourceSnapshotId;

use crate::attachment::{
    GrammarIdentityMap, SyntaxDatabaseId, SyntaxLineageId, SyntaxNodeId, SyntaxSnapshotData,
    SyntaxSnapshotId, attach_typed_tree,
};
use crate::grammar::build::GrammarBuildError;
use crate::incremental::shape::{GrammarShapeError, GrammarShapeNode};
use crate::parser::parse_shadow_document;

use super::{ParseFailure, SyntaxIdentityKind, reconcile};

#[derive(Debug)]
pub(super) struct ShadowDatabaseState {
    database: Option<SyntaxDatabaseId>,
    next_lineage: Option<NonZeroU64>,
}

impl Default for ShadowDatabaseState {
    fn default() -> Self {
        Self {
            database: SyntaxDatabaseId::allocate(),
            next_lineage: Some(NonZeroU64::MIN),
        }
    }
}

#[derive(Clone, Debug)]
pub(super) struct ShadowLineageState {
    current: Arc<SyntaxSnapshotData>,
    shape: Arc<GrammarShapeNode>,
    identities: GrammarIdentityMap,
    allocator: GrammarNodeAllocator,
}

#[derive(Debug)]
pub(super) struct StagedInitial {
    lineage: ShadowLineageState,
    next_lineage: Option<NonZeroU64>,
}

#[derive(Debug)]
pub(super) struct StagedReparse {
    lineage: ShadowLineageState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ShadowFault {
    None,
    #[cfg(test)]
    MissingAttachment,
}

impl ShadowDatabaseState {
    pub(super) fn stage_initial(
        &self,
        source: &SourceSnapshotId,
        document: &SourceDocument,
        fault: ShadowFault,
    ) -> Result<StagedInitial, ParseFailure> {
        let database = self.database.ok_or(ParseFailure::InternalInvariant)?;
        let ordinal = self.next_lineage.ok_or(ParseFailure::InternalInvariant)?;
        let lineage_id = SyntaxLineageId::new(database, ordinal);
        let next_lineage = ordinal.get().checked_add(1).and_then(NonZeroU64::new);
        let build =
            parse_shadow_document(document).map_err(|error| map_grammar_build_failure(&error))?;
        let shape = GrammarShapeNode::from_build(&build).map_err(map_grammar_shape_failure)?;
        let mut allocator = GrammarNodeAllocator::new(lineage_id);
        let mut identities =
            reconcile::allocate_initial_grammar(&shape, &mut || allocator.allocate())?;
        apply_fault(fault, &build, &mut identities);
        let snapshot_id = SyntaxSnapshotId::new(lineage_id, source.clone());
        let current =
            attach_typed_tree(&build, &identities, snapshot_id, Arc::new(document.clone()))
                .map_err(|_| ParseFailure::InternalInvariant)?;
        Ok(StagedInitial {
            lineage: ShadowLineageState {
                current,
                shape: Arc::new(shape),
                identities,
                allocator,
            },
            next_lineage,
        })
    }

    pub(super) fn commit_initial(&mut self, staged: StagedInitial) -> ShadowLineageState {
        self.next_lineage = staged.next_lineage;
        staged.lineage
    }

    #[cfg(test)]
    pub(super) const fn next_lineage_for_test(&self) -> Option<NonZeroU64> {
        self.next_lineage
    }
}

impl ShadowLineageState {
    pub(super) fn stage_reparse(
        &self,
        source: &SourceSnapshotId,
        document: &SourceDocument,
        fault: ShadowFault,
    ) -> Result<StagedReparse, ParseFailure> {
        if source.name() != self.current.snapshot_id().source().name()
            || source.generation() <= self.current.snapshot_id().source().generation()
        {
            return Err(ParseFailure::InternalInvariant);
        }
        let build =
            parse_shadow_document(document).map_err(|error| map_grammar_build_failure(&error))?;
        let shape = GrammarShapeNode::from_build(&build).map_err(map_grammar_shape_failure)?;
        let mut allocator = self.allocator.clone();
        let mut identities =
            reconcile::reconcile_grammar(&self.shape, &shape, &self.identities, &mut || {
                allocator.allocate()
            })?;
        apply_fault(fault, &build, &mut identities);
        let snapshot_id = SyntaxSnapshotId::new(self.allocator.lineage, source.clone());
        let current =
            attach_typed_tree(&build, &identities, snapshot_id, Arc::new(document.clone()))
                .map_err(|_| ParseFailure::InternalInvariant)?;
        Ok(StagedReparse {
            lineage: Self {
                current,
                shape: Arc::new(shape),
                identities,
                allocator,
            },
        })
    }

    pub(super) const fn current(&self) -> &Arc<SyntaxSnapshotData> {
        &self.current
    }

    #[cfg(test)]
    pub(super) const fn next_node_for_test(&self) -> Option<NonZeroU64> {
        self.allocator.next
    }
}

impl StagedInitial {
    pub(super) const fn current(&self) -> &Arc<SyntaxSnapshotData> {
        self.lineage.current()
    }
}

impl StagedReparse {
    pub(super) fn into_lineage(self) -> ShadowLineageState {
        self.lineage
    }

    pub(super) const fn current(&self) -> &Arc<SyntaxSnapshotData> {
        self.lineage.current()
    }
}

#[derive(Clone, Debug)]
struct GrammarNodeAllocator {
    lineage: SyntaxLineageId,
    next: Option<NonZeroU64>,
}

impl GrammarNodeAllocator {
    const fn new(lineage: SyntaxLineageId) -> Self {
        Self {
            lineage,
            next: Some(NonZeroU64::MIN),
        }
    }

    fn allocate(&mut self) -> Result<SyntaxNodeId, ParseFailure> {
        let slot = self
            .next
            .ok_or(ParseFailure::IdentityExhausted(SyntaxIdentityKind::Node))?;
        self.next = slot.get().checked_add(1).and_then(NonZeroU64::new);
        Ok(SyntaxNodeId::new(self.lineage, slot))
    }
}

fn map_grammar_build_failure(error: &GrammarBuildError) -> ParseFailure {
    match error {
        GrammarBuildError::LimitExceeded(limit) => ParseFailure::LimitExceeded(*limit),
        _ => ParseFailure::InternalInvariant,
    }
}

const fn map_grammar_shape_failure(_: GrammarShapeError) -> ParseFailure {
    ParseFailure::InternalInvariant
}

fn apply_fault(
    fault: ShadowFault,
    build: &crate::grammar::build::GrammarBuild,
    identities: &mut GrammarIdentityMap,
) {
    match fault {
        ShadowFault::None => {}
        #[cfg(test)]
        ShadowFault::MissingAttachment => {
            if let Some(entry) = build.index().entries().last() {
                identities.remove_path(entry.path());
            }
        }
    }
    #[cfg(not(test))]
    let _ = (build, identities);
}
