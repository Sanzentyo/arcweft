//! Private staging for grammar identity and typed attachment.

use core::num::NonZeroU64;
use std::rc::Rc;
use std::sync::Arc;

use arcweft_source::identity::SourceSnapshotId;
use arcweft_source::{SourceDocument, SourceRange, SourceSpan};

use crate::attachment::{
    GrammarIdentityMap, SyntaxDatabaseId, SyntaxLineageId, SyntaxNodeId, SyntaxSnapshotData,
    SyntaxSnapshotId, attach_typed_tree,
};
use crate::grammar::build::{GrammarBuild, GrammarBuildError, build_grammar};
use crate::grammar::event::SyntaxEvent;
use crate::grammar::kinds::{SyntaxKind, SyntaxRole};
use crate::incremental::shape::{GrammarShapeError, GrammarShapeNode};
use crate::parser::parse_shadow_document;
use crate::parser::unbound_fragment::{AttachedFragment, FragmentKind, FragmentTree};

use super::bound::BoundParsedSource;
use super::{ParseFailure, SyntaxIdentityKind, reconcile};

#[derive(Debug)]
pub(super) struct SyntaxTransactionState {
    database: SyntaxDatabaseId,
    next_lineage: Option<NonZeroU64>,
}

impl SyntaxTransactionState {
    pub(super) fn try_new() -> Option<Self> {
        Some(Self {
            database: SyntaxDatabaseId::allocate()?,
            next_lineage: Some(NonZeroU64::MIN),
        })
    }
}

#[derive(Clone, Debug)]
pub(super) struct SyntaxLineageState {
    current: Rc<BoundParsedSource>,
    shape: Arc<GrammarShapeNode>,
    identities: GrammarIdentityMap,
    allocator: GrammarNodeAllocator,
}

#[derive(Debug)]
pub(super) struct StagedInitial {
    lineage: SyntaxLineageState,
    next_lineage: Option<NonZeroU64>,
}

#[derive(Debug)]
pub(super) struct StagedReparse {
    lineage: SyntaxLineageState,
}

pub(super) struct StagedFragment<K: FragmentKind> {
    fragment: AttachedFragment<K>,
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
pub(super) enum TransactionFault {
    None,
    #[cfg(test)]
    MissingAttachment,
}

impl SyntaxTransactionState {
    pub(super) fn stage_initial(
        &self,
        source: &SourceSnapshotId,
        document: &Arc<SourceDocument>,
        fault: TransactionFault,
    ) -> Result<StagedInitial, ParseFailure> {
        let build =
            parse_shadow_document(document).map_err(|error| map_grammar_build_failure(&error))?;
        let staged = self.stage_fresh_attachment(source, document, build, fault)?;
        let current = Rc::new(
            BoundParsedSource::try_new(staged.syntax, &staged.build)
                .map_err(|_| ParseFailure::InternalInvariant)?,
        );
        Ok(StagedInitial {
            lineage: SyntaxLineageState {
                current,
                shape: Arc::new(staged.shape),
                identities: staged.identities,
                allocator: staged.allocator,
            },
            next_lineage: staged.next_lineage,
        })
    }

    pub(super) fn stage_fragment<K: FragmentKind>(
        &self,
        source: &SourceSnapshotId,
        document: &Arc<SourceDocument>,
        span: &SourceSpan,
        tree: &FragmentTree,
        fault: TransactionFault,
    ) -> Result<StagedFragment<K>, ParseFailure> {
        let build = project_fragment_tree(document, span.range(), tree)?;
        let staged = self.stage_fresh_attachment(source, document, build, fault)?;
        let root = staged
            .syntax
            .root_handle()
            .child(SyntaxRole::Element(0))
            .ok_or(ParseFailure::InternalInvariant)?;
        let root = staged
            .syntax
            .typed_node::<K::AstKind>(root.id())
            .map_err(|_| ParseFailure::InternalInvariant)?;
        let fragment = AttachedFragment::new(staged.syntax, root);
        Ok(StagedFragment {
            fragment,
            next_lineage: staged.next_lineage,
        })
    }

    fn stage_fresh_attachment(
        &self,
        source: &SourceSnapshotId,
        document: &Arc<SourceDocument>,
        build: GrammarBuild,
        fault: TransactionFault,
    ) -> Result<StagedFreshAttachment, ParseFailure> {
        let ordinal = self.next_lineage.ok_or(ParseFailure::InternalInvariant)?;
        let lineage_id = SyntaxLineageId::new(self.database, ordinal);
        let next_lineage = ordinal.get().checked_add(1).and_then(NonZeroU64::new);
        let shape = GrammarShapeNode::from_build(&build).map_err(map_grammar_shape_failure)?;
        let mut allocator = GrammarNodeAllocator::new(lineage_id);
        let mut identities =
            reconcile::allocate_initial_grammar(&shape, &mut || allocator.allocate())?;
        apply_fault(fault, &build, &mut identities);
        let snapshot_id = SyntaxSnapshotId::new(lineage_id, source.clone());
        let syntax = attach_typed_tree(&build, &identities, snapshot_id, Arc::clone(document))
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

    pub(super) fn commit_initial(&mut self, staged: StagedInitial) -> SyntaxLineageState {
        self.next_lineage = staged.next_lineage;
        staged.lineage
    }

    pub(super) fn commit_fragment<K: FragmentKind>(
        &mut self,
        staged: StagedFragment<K>,
    ) -> AttachedFragment<K> {
        self.next_lineage = staged.next_lineage;
        staged.fragment
    }

    #[cfg(test)]
    pub(super) const fn next_lineage_for_test(&self) -> Option<NonZeroU64> {
        self.next_lineage
    }
}

impl SyntaxLineageState {
    pub(super) fn stage_reparse(
        &self,
        source: &SourceSnapshotId,
        document: &Arc<SourceDocument>,
        fault: TransactionFault,
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
        let syntax = attach_typed_tree(&build, &identities, snapshot_id, Arc::clone(document))
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
    pub(super) fn into_lineage(self) -> SyntaxLineageState {
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
        _ => ParseFailure::InternalInvariant,
    }
}

fn project_fragment_tree(
    document: &SourceDocument,
    target: SourceRange,
    tree: &FragmentTree,
) -> Result<GrammarBuild, ParseFailure> {
    let events = tree.events();
    if events.len() < 3
        || events.first()
            != Some(&SyntaxEvent::start(
                SyntaxKind::SourceFile,
                SyntaxRole::Root,
            ))
        || events.last() != Some(&SyntaxEvent::FinishNode)
    {
        return Err(ParseFailure::InternalInvariant);
    }

    let fragment_len = target
        .end()
        .checked_sub(target.start())
        .ok_or(ParseFailure::InternalInvariant)?;
    let eof_index = events.len() - 2;
    if events[eof_index]
        != SyntaxEvent::token(
            SyntaxKind::EofToken,
            SourceRange::new(fragment_len, fragment_len),
        )
        || events[1..eof_index].iter().any(|event| {
            matches!(
                event,
                SyntaxEvent::Token {
                    kind: SyntaxKind::EofToken,
                    ..
                }
            )
        })
    {
        return Err(ParseFailure::InternalInvariant);
    }

    let mut projected = Vec::with_capacity(events.len() + 2);
    projected.push(SyntaxEvent::start(SyntaxKind::SourceFile, SyntaxRole::Root));
    if target.start() > 0 {
        projected.push(SyntaxEvent::token(
            SyntaxKind::TextToken,
            SourceRange::new(0, target.start()),
        ));
    }
    for event in &events[1..eof_index] {
        projected.push(
            event
                .rebased(target.start())
                .ok_or(ParseFailure::InternalInvariant)?,
        );
    }
    if target.end() < document.text().len() {
        projected.push(SyntaxEvent::token(
            SyntaxKind::TextToken,
            SourceRange::new(target.end(), document.text().len()),
        ));
    }
    projected.push(SyntaxEvent::token(
        SyntaxKind::EofToken,
        SourceRange::new(document.text().len(), document.text().len()),
    ));
    projected.push(SyntaxEvent::FinishNode);

    build_grammar(document, &projected).map_err(|error| map_grammar_build_failure(&error))
}

const fn map_grammar_shape_failure(_: GrammarShapeError) -> ParseFailure {
    ParseFailure::InternalInvariant
}

fn apply_fault(
    fault: TransactionFault,
    build: &crate::grammar::build::GrammarBuild,
    identities: &mut GrammarIdentityMap,
) {
    match fault {
        TransactionFault::None => {}
        #[cfg(test)]
        TransactionFault::MissingAttachment => {
            if let Some(entry) = build.index().entries().last() {
                identities.remove_path(entry.path());
            }
        }
    }
    #[cfg(not(test))]
    let _ = (build, identities);
}
