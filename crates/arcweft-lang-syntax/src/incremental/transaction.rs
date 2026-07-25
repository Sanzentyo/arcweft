//! Private staging for grammar identity and typed attachment.

use core::num::NonZeroU64;
use std::rc::Rc;
use std::sync::Arc;

use arcweft_source::identity::SourceSnapshotId;
use arcweft_source::{SourceDocument, SourceSpan};

use crate::attachment::{
    GrammarIdentityMap, SyntaxDatabaseId, SyntaxLineageId, SyntaxNodeId, SyntaxSnapshotData,
    SyntaxSnapshotId, attach_typed_tree,
};
use crate::grammar::build::{GrammarBuild, GrammarBuildError};
use crate::incremental::shape::{GrammarShapeError, GrammarShapeNode};
use crate::parser::{parse_shadow_document, parse_shadow_fragment};

use super::bound::{BoundFragment, BoundFragmentKind, BoundParsedSource};
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
    current: Rc<BoundParsedSource>,
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

#[derive(Debug)]
pub(super) struct StagedFragment<K> {
    fragment: BoundFragment<K>,
    next_lineage: Option<NonZeroU64>,
}

#[derive(Debug)]
struct StagedFreshAttachment {
    build: GrammarBuild,
    syntax: Arc<SyntaxSnapshotData>,
    shape: GrammarShapeNode,
    identities: GrammarIdentityMap,
    allocator: GrammarNodeAllocator,
    next_lineage: Option<NonZeroU64>,
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
        let build =
            parse_shadow_document(document).map_err(|error| map_grammar_build_failure(&error))?;
        let staged = self.stage_fresh_attachment(source, document, build, fault)?;
        let current = Rc::new(
            BoundParsedSource::try_new(staged.syntax, &staged.build)
                .map_err(|_| ParseFailure::InternalInvariant)?,
        );
        Ok(StagedInitial {
            lineage: ShadowLineageState {
                current,
                shape: Arc::new(staged.shape),
                identities: staged.identities,
                allocator: staged.allocator,
            },
            next_lineage: staged.next_lineage,
        })
    }

    pub(super) fn stage_fragment<K: BoundFragmentKind>(
        &self,
        source: &SourceSnapshotId,
        document: &SourceDocument,
        span: &SourceSpan,
        fault: ShadowFault,
    ) -> Result<StagedFragment<K>, ParseFailure> {
        let build = parse_shadow_fragment(document, span.range(), K::GRAMMAR)
            .map_err(|error| map_grammar_build_failure(&error))?;
        let staged = self.stage_fresh_attachment(source, document, build, fault)?;
        let fragment = BoundFragment::<K>::try_new(staged.syntax, &staged.build, span.clone())?;
        Ok(StagedFragment {
            fragment,
            next_lineage: staged.next_lineage,
        })
    }

    fn stage_fresh_attachment(
        &self,
        source: &SourceSnapshotId,
        document: &SourceDocument,
        build: GrammarBuild,
        fault: ShadowFault,
    ) -> Result<StagedFreshAttachment, ParseFailure> {
        let database = self.database.ok_or(ParseFailure::InternalInvariant)?;
        let ordinal = self.next_lineage.ok_or(ParseFailure::InternalInvariant)?;
        let lineage_id = SyntaxLineageId::new(database, ordinal);
        let next_lineage = ordinal.get().checked_add(1).and_then(NonZeroU64::new);
        let shape = GrammarShapeNode::from_build(&build).map_err(map_grammar_shape_failure)?;
        let mut allocator = GrammarNodeAllocator::new(lineage_id);
        let mut identities =
            reconcile::allocate_initial_grammar(&shape, &mut || allocator.allocate())?;
        apply_fault(fault, &build, &mut identities);
        let snapshot_id = SyntaxSnapshotId::new(lineage_id, source.clone());
        let syntax =
            attach_typed_tree(&build, &identities, snapshot_id, Arc::new(document.clone()))
                .map_err(|_| ParseFailure::InternalInvariant)?;
        Ok(StagedFreshAttachment {
            build,
            syntax,
            shape,
            identities,
            allocator,
            next_lineage,
        })
    }

    pub(super) fn commit_initial(&mut self, staged: StagedInitial) -> ShadowLineageState {
        self.next_lineage = staged.next_lineage;
        staged.lineage
    }

    pub(super) fn commit_fragment<K>(&mut self, staged: StagedFragment<K>) -> BoundFragment<K> {
        self.next_lineage = staged.next_lineage;
        staged.fragment
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
        let syntax =
            attach_typed_tree(&build, &identities, snapshot_id, Arc::new(document.clone()))
                .map_err(|_| ParseFailure::InternalInvariant)?;
        let current = Rc::new(
            BoundParsedSource::try_new(syntax, &build)
                .map_err(|_| ParseFailure::InternalInvariant)?,
        );
        Ok(StagedReparse {
            lineage: Self {
                current,
                shape: Arc::new(shape),
                identities,
                allocator,
            },
        })
    }

    pub(super) const fn current(&self) -> &Rc<BoundParsedSource> {
        &self.current
    }

    #[cfg(test)]
    pub(super) const fn next_node_for_test(&self) -> Option<NonZeroU64> {
        self.allocator.next
    }

    #[cfg(test)]
    pub(super) const fn set_next_node_for_test(&mut self, next: Option<NonZeroU64>) {
        self.allocator.next = next;
    }
}

impl StagedInitial {
    pub(super) const fn current(&self) -> &Rc<BoundParsedSource> {
        self.lineage.current()
    }
}

impl StagedReparse {
    pub(super) fn into_lineage(self) -> ShadowLineageState {
        self.lineage
    }

    pub(super) const fn current(&self) -> &Rc<BoundParsedSource> {
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
        GrammarBuildError::InvalidFragmentRange { .. } => ParseFailure::SourceMismatch,
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
