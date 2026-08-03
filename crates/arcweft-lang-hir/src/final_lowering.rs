//! Private all-or-nothing construction of one final-HIR module revision.
//!
//! Attached syntax lowerers stage directly into this owner. No public reader
//! is exposed until all eight arenas, declaration members, source components,
//! diagnostics, and slot lifetimes have frozen into one validated module.

mod capture_lowering;
mod expression_lowering;
pub(crate) mod id_ref_projection;
mod item_lowering;
pub(crate) mod literal_projection;
pub(crate) mod name_projection;
mod path_projection;
pub(crate) mod pattern_lowering;
mod statement_lowering;
mod thread_body_lowering;
mod type_ref_lowering;

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use arcweft_lang_syntax::text::{
    MAX_RICH_TEXT_CONTENT_ARGUMENTS, MAX_RICH_TEXT_CONTENT_TAGS, MAX_RICH_TEXT_TAG_ARGUMENTS,
    MAX_RICH_TEXT_TAG_BODY_BYTES, MAX_RICH_TEXT_TAG_KEY_BYTES, MAX_RICH_TEXT_TAG_VALUE_BYTES,
};

use crate::arena::{HirArenaError, StagedArena};
use crate::database::{HirDatabase, HirLowerOutput, StagedModuleCommit};
use crate::diagnostic::{HirDiagnostic, HirRecoveryDiagnostic};
use crate::dialogue_application::{
    HirDialogueExpressionExpectation, HirDialogueTransactionContext,
    HirDialogueTransactionRequirement, HirRichTextCharge,
};
use crate::expr::{HirExpr, HirExprKind};
use crate::identity::{
    CaptureId, ExprId, HirLimit, HirSnapshotId, ItemId, LocalGeneration, LocalId, PatternId,
    ScopeId, StmtId, SyntheticOwner, TypeId,
};
use crate::item::{HirDeclarationMemberArena, HirDeclarationMemberIndexBuilder, HirItem};
use crate::lower::{HirInvariantFailure, HirLimitError, HirLowerFailure, LoweringRequest};
use crate::module::{HirModule, HirModuleArenaParts, HirModuleArenas};
use crate::pattern::{HirPattern, HirPatternResolver};
use crate::scope::{HirCapture, HirLocal, HirScope};
use crate::slot::{HirOrigin, HirSlotError, StagedSlotTransaction};
use crate::source_index::{HirSourceCommitInvariantError, StagedHirSourceIndex};
use crate::stmt::HirStmt;
use crate::type_ref::{HirType, HirTypeResolver};

use capture_lowering::ClosureCaptureFrame;

fn require_limit(limit: HirLimit, observed: usize) -> Result<(), HirLowerFailure> {
    let maximum = limit.maximum();
    if observed <= maximum {
        Ok(())
    } else {
        Err(HirLimitError::with_maximum(limit, observed, maximum).into())
    }
}

/// The eight mutable typed arenas owned by one final-HIR transaction.
pub(crate) struct StagedHirModuleArenas {
    items: StagedArena<HirItem, ItemId>,
    scopes: StagedArena<HirScope, ScopeId>,
    locals: StagedArena<HirLocal, LocalId>,
    expressions: StagedArena<HirExpr, ExprId>,
    statements: StagedArena<HirStmt, StmtId>,
    types: StagedArena<HirType, TypeId>,
    patterns: StagedArena<HirPattern, PatternId>,
    captures: StagedArena<HirCapture, CaptureId>,
}

impl StagedHirModuleArenas {
    fn from_previous(previous: Option<&HirModule>) -> Self {
        let Some(previous) = previous else {
            return Self {
                items: StagedArena::new(),
                scopes: StagedArena::new(),
                locals: StagedArena::new(),
                expressions: StagedArena::new(),
                statements: StagedArena::new(),
                types: StagedArena::new(),
                patterns: StagedArena::new(),
                captures: StagedArena::new(),
            };
        };
        let arenas = previous.arenas();
        Self {
            items: StagedArena::from_snapshot(arenas.items()),
            scopes: StagedArena::from_snapshot(arenas.scopes()),
            locals: StagedArena::from_snapshot(arenas.locals()),
            expressions: StagedArena::from_snapshot(arenas.expressions()),
            statements: StagedArena::from_snapshot(arenas.statements()),
            types: StagedArena::from_snapshot(arenas.types()),
            patterns: StagedArena::from_snapshot(arenas.patterns()),
            captures: StagedArena::from_snapshot(arenas.captures()),
        }
    }

    pub(crate) const fn items(&mut self) -> &mut StagedArena<HirItem, ItemId> {
        &mut self.items
    }

    pub(crate) const fn scopes(&mut self) -> &mut StagedArena<HirScope, ScopeId> {
        &mut self.scopes
    }

    pub(crate) const fn locals(&mut self) -> &mut StagedArena<HirLocal, LocalId> {
        &mut self.locals
    }

    pub(crate) const fn expressions(&mut self) -> &mut StagedArena<HirExpr, ExprId> {
        &mut self.expressions
    }

    pub(crate) const fn statements(&mut self) -> &mut StagedArena<HirStmt, StmtId> {
        &mut self.statements
    }

    pub(crate) const fn types(&mut self) -> &mut StagedArena<HirType, TypeId> {
        &mut self.types
    }

    pub(crate) const fn patterns(&mut self) -> &mut StagedArena<HirPattern, PatternId> {
        &mut self.patterns
    }

    pub(crate) const fn captures(&mut self) -> &mut StagedArena<HirCapture, CaptureId> {
        &mut self.captures
    }

    fn freeze(
        self,
        slots: &mut StagedSlotTransaction,
    ) -> Result<HirModuleArenaParts, HirLowerFailure> {
        Ok(HirModuleArenaParts {
            items: self.items.into_snapshot(slots)?,
            scopes: self.scopes.into_snapshot(slots)?,
            locals: self.locals.into_snapshot(slots)?,
            expressions: self.expressions.into_snapshot(slots)?,
            statements: self.statements.into_snapshot(slots)?,
            types: self.types.into_snapshot(slots)?,
            patterns: self.patterns.into_snapshot(slots)?,
            captures: self.captures.into_snapshot(slots)?,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct LocalGenerationLedgerEntry {
    local: LocalId,
    generation: LocalGeneration,
    binding_name_start: usize,
}

impl LocalGenerationLedgerEntry {
    const fn new(local: LocalId, generation: LocalGeneration, binding_name_start: usize) -> Self {
        Self {
            local,
            generation,
            binding_name_start,
        }
    }
}

/// Source-ordered publication history for one `(scope, name)` binding key.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct LocalPublicationTimeline {
    entries: Vec<LocalGenerationLedgerEntry>,
}

impl LocalPublicationTimeline {
    fn last(&self) -> Option<LocalGenerationLedgerEntry> {
        self.entries.last().copied()
    }

    fn visible_at(
        &self,
        use_start: usize,
    ) -> impl Iterator<Item = LocalGenerationLedgerEntry> + '_ {
        self.entries
            .iter()
            .rev()
            .copied()
            .filter(move |entry| entry.binding_name_start < use_start)
    }

    fn publish(&mut self, entry: LocalGenerationLedgerEntry) -> Result<(), HirLowerFailure> {
        if self.entries.last().is_some_and(|previous| {
            previous.binding_name_start >= entry.binding_name_start
                || previous.generation >= entry.generation
        }) {
            return Err(HirInvariantFailure::InvalidLocalTimeline.into());
        }
        self.entries.push(entry);
        Ok(())
    }

    #[cfg(test)]
    fn entries(&self) -> &[LocalGenerationLedgerEntry] {
        &self.entries
    }
}

/// Private owner of every fallible input to one final-HIR publication.
pub(crate) struct StagedHirModuleTransaction<'source> {
    request: LoweringRequest<'source>,
    plan: StagedModuleCommit,
    slots: StagedSlotTransaction,
    arenas: StagedHirModuleArenas,
    declaration_members: HirDeclarationMemberIndexBuilder,
    source_components: StagedHirSourceIndex,
    source_ordered_items: Vec<ItemId>,
    diagnostics: Vec<HirDiagnostic>,
    local_timelines: BTreeMap<(ScopeId, crate::leaf::HirName), LocalPublicationTimeline>,
    pattern_locals: BTreeMap<PatternId, Box<[LocalId]>>,
    closure_capture_frames: Vec<ClosureCaptureFrame>,
    #[cfg(test)]
    reverse_pattern_child_insertion: bool,
}

impl HirDatabase {
    /// Starts a private final-HIR transaction without publishing a second
    /// production reader or reserving an observable module revision.
    pub(crate) fn stage_final_hir<'source>(
        &self,
        request: LoweringRequest<'source>,
    ) -> Result<StagedHirModuleTransaction<'source>, HirLowerFailure> {
        StagedHirModuleTransaction::stage(self, request)
    }
}

impl<'source> StagedHirModuleTransaction<'source> {
    fn stage(
        database: &HirDatabase,
        request: LoweringRequest<'source>,
    ) -> Result<Self, HirLowerFailure> {
        let plan = database.stage_module(request.key())?;
        if let Some(previous) = plan.previous() {
            let accepted = previous.provenance().syntax_snapshot().lineage();
            let supplied = request.source().snapshot_id().lineage();
            if accepted.database() != supplied.database() {
                return Err(HirLowerFailure::WrongSyntaxDatabase {
                    expected: accepted.database(),
                    actual: supplied.database(),
                });
            }
            if accepted != supplied {
                return Err(HirLowerFailure::WrongSyntaxLineage {
                    expected: accepted,
                    actual: supplied,
                });
            }
        }

        let slots = match plan.previous() {
            Some(previous) => {
                StagedSlotTransaction::from_snapshot(previous.slots(), plan.revision())
            }
            None => StagedSlotTransaction::new(plan.module_id(), plan.revision()),
        };
        let arenas = StagedHirModuleArenas::from_previous(plan.previous().map(Arc::as_ref));
        let declaration_members = HirDeclarationMemberIndexBuilder::new(plan.module_id());
        let source_components =
            StagedHirSourceIndex::new(request.source().document().identity().clone(), &slots);
        let diagnostics = request
            .source()
            .diagnostics()
            .iter()
            .cloned()
            .map(HirDiagnostic::Syntax)
            .collect();

        Ok(Self {
            request,
            plan,
            slots,
            arenas,
            declaration_members,
            source_components,
            source_ordered_items: Vec::new(),
            diagnostics,
            local_timelines: BTreeMap::new(),
            pattern_locals: BTreeMap::new(),
            closure_capture_frames: Vec::new(),
            #[cfg(test)]
            reverse_pattern_child_insertion: false,
        })
    }

    fn next_sequential_local_generation(
        &self,
        scope: ScopeId,
        name: &crate::leaf::HirName,
        binding_name_start: usize,
    ) -> Result<LocalGeneration, HirLowerFailure> {
        let Some(previous) = self
            .local_timelines
            .get(&(scope, name.clone()))
            .and_then(LocalPublicationTimeline::last)
        else {
            return Ok(LocalGeneration::FIRST);
        };
        if binding_name_start <= previous.binding_name_start {
            return Err(HirLowerFailure::LocalBindingSourceOrderViolation {
                scope,
                name: name.clone(),
                previous_start: previous.binding_name_start,
                attempted_start: binding_name_start,
            });
        }
        previous.generation.checked_next().ok_or_else(|| {
            HirLowerFailure::LocalGenerationExhausted {
                scope,
                name: name.clone(),
            }
        })
    }

    pub(crate) const fn snapshot_id(&self) -> HirSnapshotId {
        self.plan.snapshot_id()
    }

    /// Grants attached lowerers simultaneous access to the one slot ledger and
    /// its eight typed payload arenas. Neither value can outlive this owner.
    pub(crate) fn storage_mut(
        &mut self,
    ) -> (&mut StagedSlotTransaction, &mut StagedHirModuleArenas) {
        (&mut self.slots, &mut self.arenas)
    }

    pub(crate) fn stage_declaration_members(
        &mut self,
        owner: ItemId,
        item: &HirItem,
        arena: HirDeclarationMemberArena,
    ) -> Result<(), HirLowerFailure> {
        require_limit(HirLimit::DeclarationMembers, arena.members().len())?;
        self.declaration_members
            .stage(owner, item, arena)
            .map_err(|_| HirInvariantFailure::InvalidDeclarationMemberIndex.into())
    }

    pub(crate) const fn source_components(&mut self) -> &mut StagedHirSourceIndex {
        &mut self.source_components
    }

    pub(crate) fn stage_recovery_diagnostic(&mut self, diagnostic: HirRecoveryDiagnostic) {
        self.diagnostics.push(HirDiagnostic::Recovery(diagnostic));
    }

    /// Resolves the latest binding published before one exact source use.
    ///
    /// Lookup walks lexical parents. A later same-scope shadow does not hide an
    /// earlier ancestor at a use that precedes the shadow's publication.
    pub(super) fn visible_local(
        &self,
        scope: ScopeId,
        name: &crate::leaf::HirName,
        use_start: usize,
    ) -> Result<Option<LocalId>, HirLowerFailure> {
        let mut current = Some(scope);
        let mut visited = BTreeSet::new();
        while let Some(scope) = current {
            if !visited.insert(scope) {
                return Err(HirInvariantFailure::InvalidScopeParent.into());
            }
            if let Some(timeline) = self.local_timelines.get(&(scope, name.clone())) {
                if let Some(entry) = timeline.visible_at(use_start).next() {
                    let local = self
                        .arenas
                        .locals
                        .resolve_staged(&self.slots, entry.local)?;
                    if local.scope() != scope
                        || local.name() != name
                        || local.generation() != entry.generation
                    {
                        return Err(HirInvariantFailure::InvalidLocalTimeline.into());
                    }
                    if !local.is_poisoned() {
                        return Ok(Some(entry.local));
                    }
                    return Ok(None);
                }
            }
            current = self
                .arenas
                .scopes
                .resolve_staged(&self.slots, scope)?
                .parent();
        }
        Ok(None)
    }

    /// Records one source-backed top-level item in attached source order.
    /// Final publication validates complete, non-duplicated source-item
    /// coverage once against the immutable item arena and slot ledger.
    pub(crate) fn stage_source_ordered_item(&mut self, item: ItemId) {
        self.source_ordered_items.push(item);
    }

    #[cfg(test)]
    pub(crate) fn staged_source_ordered_items(&self) -> &[ItemId] {
        &self.source_ordered_items
    }

    /// Freezes and publishes this revision as one all-or-nothing operation.
    pub(crate) fn finish(
        mut self,
        database: &mut HirDatabase,
    ) -> Result<HirLowerOutput, HirLowerFailure> {
        if !self.closure_capture_frames.is_empty() {
            return Err(HirInvariantFailure::InvalidArenaCommit.into());
        }
        self.slots.retire_untouched()?;

        let arena_parts = self.arenas.freeze(&mut self.slots)?;
        let source_components = self.source_components.commit()?;
        let declaration_members = self.declaration_members.freeze();

        let syntax_diagnostic_count = self.request.source().diagnostics().len();
        self.diagnostics[syntax_diagnostic_count..].sort_by(HirDiagnostic::compare_for_publication);
        let diagnostics = Arc::from(self.diagnostics);

        let prepared_slots = self.slots.prepare()?;
        let arenas = HirModuleArenas::try_new(prepared_slots.snapshot(), arena_parts)?;
        let module = Arc::new(HirModule::try_new(
            self.plan.snapshot_id(),
            self.plan.key().clone(),
            self.request.source(),
            diagnostics,
            Arc::clone(prepared_slots.snapshot()),
            arenas,
            self.source_ordered_items.into_boxed_slice(),
            declaration_members,
            source_components,
            self.plan.invalidation_epoch(),
        )?);
        database.publish_module(self.plan, prepared_slots, module)
    }
}

impl HirTypeResolver for StagedHirModuleTransaction<'_> {
    fn scope_is_live(&self, scope: ScopeId) -> bool {
        self.arenas
            .scopes
            .resolve_staged(&self.slots, scope)
            .is_ok()
    }

    fn resolve_type(&self, scope: ScopeId, ty: TypeId) -> Option<&HirType> {
        HirTypeResolver::scope_is_live(self, scope)
            .then(|| self.arenas.types.resolve_staged(&self.slots, ty).ok())
            .flatten()
            .filter(|ty| ty.scope() == scope)
    }
}

impl HirPatternResolver for StagedHirModuleTransaction<'_> {
    fn scope_is_live(&self, scope: ScopeId) -> bool {
        HirTypeResolver::scope_is_live(self, scope)
    }

    fn local_is_visible(&self, scope: ScopeId, local: LocalId) -> bool {
        HirPatternResolver::scope_is_live(self, scope)
            && self
                .arenas
                .locals
                .resolve_staged(&self.slots, local)
                .is_ok_and(|local| local.scope() == scope)
    }

    fn resolve_type_state(
        &self,
        scope: ScopeId,
        ty: TypeId,
    ) -> Option<&crate::expr::HirPoisonState> {
        HirTypeResolver::resolve_type(self, scope, ty).map(HirType::state)
    }

    fn resolve_pattern(&self, scope: ScopeId, pattern: PatternId) -> Option<&HirPattern> {
        HirPatternResolver::scope_is_live(self, scope)
            .then(|| {
                self.arenas
                    .patterns
                    .resolve_staged(&self.slots, pattern)
                    .ok()
            })
            .flatten()
            .filter(|pattern| pattern.scope() == scope)
    }
}

impl HirDialogueTransactionContext for StagedHirModuleTransaction<'_> {
    type Error = HirLowerFailure;

    fn require(
        &mut self,
        requirement: HirDialogueTransactionRequirement,
    ) -> Result<(), Self::Error> {
        let valid = match requirement {
            HirDialogueTransactionRequirement::Expression { id, expected } => {
                let metadata = self.slots.resolve_staged(id)?;
                let expression = self.arenas.expressions.resolve_staged(&self.slots, id)?;
                let scope_is_live = HirTypeResolver::scope_is_live(self, expression.scope());
                let kind_matches = match expected {
                    HirDialogueExpressionExpectation::Any => true,
                    HirDialogueExpressionExpectation::Call => {
                        matches!(expression.kind(), HirExprKind::Call(_))
                    }
                    HirDialogueExpressionExpectation::PostfixIndexCandidate {
                        owner,
                        role,
                        target,
                    } => {
                        matches!(
                            (metadata.origin(), expression.kind()),
                            (
                                HirOrigin::Synthetic(key),
                                HirExprKind::Index(index)
                            ) if key.owner() == SyntheticOwner::Expr(owner)
                                && key.role() == role
                                && index.target() == target
                        )
                    }
                    HirDialogueExpressionExpectation::DialogueContentCandidate {
                        owner,
                        role,
                        target,
                    } => {
                        matches!(
                            (metadata.origin(), expression.kind()),
                            (
                                HirOrigin::Synthetic(key),
                                HirExprKind::DialogueContentApplication(application)
                            ) if key.owner() == SyntheticOwner::Expr(owner)
                                && key.role() == role
                                && application.target() == target
                        )
                    }
                };
                scope_is_live && kind_matches
            }
            HirDialogueTransactionRequirement::Statement(id) => self
                .arenas
                .statements
                .resolve_staged(&self.slots, id)
                .is_ok_and(|statement| HirTypeResolver::scope_is_live(self, statement.scope())),
            HirDialogueTransactionRequirement::Pattern(id) => self
                .arenas
                .patterns
                .resolve_staged(&self.slots, id)
                .is_ok_and(|pattern| HirPatternResolver::scope_is_live(self, pattern.scope())),
            HirDialogueTransactionRequirement::Scope(id) => {
                HirTypeResolver::scope_is_live(self, id)
            }
            HirDialogueTransactionRequirement::Item(id) => self
                .arenas
                .items
                .resolve_staged(&self.slots, id)
                .is_ok_and(|item| HirTypeResolver::scope_is_live(self, item.scope())),
            HirDialogueTransactionRequirement::RichTextCharge(charge) => match charge {
                HirRichTextCharge::ContentTags { observed } => {
                    observed <= MAX_RICH_TEXT_CONTENT_TAGS
                }
                HirRichTextCharge::ContentArguments { observed } => {
                    observed <= MAX_RICH_TEXT_CONTENT_ARGUMENTS
                }
                HirRichTextCharge::TagBodyEncodedBytes { observed } => {
                    observed <= MAX_RICH_TEXT_TAG_BODY_BYTES
                }
                HirRichTextCharge::TagArguments { observed } => {
                    observed <= MAX_RICH_TEXT_TAG_ARGUMENTS
                }
                HirRichTextCharge::ArgumentKeyBytes { observed } => {
                    observed <= MAX_RICH_TEXT_TAG_KEY_BYTES
                }
                HirRichTextCharge::ArgumentValueEncodedBytes { observed }
                | HirRichTextCharge::ArgumentValueDecodedBytes { observed } => {
                    observed <= MAX_RICH_TEXT_TAG_VALUE_BYTES
                }
            },
        };
        if valid {
            Ok(())
        } else {
            Err(HirInvariantFailure::InvalidArenaCommit.into())
        }
    }
}

impl From<HirSlotError> for HirLowerFailure {
    fn from(error: HirSlotError) -> Self {
        match error {
            HirSlotError::Resolve(error) => Self::IdResolve(error),
            HirSlotError::Limit(error) => Self::Limit(error),
            HirSlotError::SlotIdentityExhausted { module, kind } => {
                Self::SlotIdentityExhausted { module, kind }
            }
            HirSlotError::OwnerNotReserved { .. }
            | HirSlotError::ConflictingSlotView { .. }
            | HirSlotError::MetadataMismatch { .. }
            | HirSlotError::InvalidKeyOnlyRole { .. }
            | HirSlotError::InvalidRetirement
            | HirSlotError::CommitConflict
            | HirSlotError::TransactionPoisoned => {
                Self::Invariant(HirInvariantFailure::InvalidSlotCommit)
            }
        }
    }
}

impl From<HirArenaError> for HirLowerFailure {
    fn from(error: HirArenaError) -> Self {
        match error {
            HirArenaError::Slot(error) => error.into(),
            HirArenaError::Limit(error) => Self::Limit(error),
            HirArenaError::InvalidReservation { .. }
            | HirArenaError::UnfinalizedReservations { .. }
            | HirArenaError::CoverageMismatch { .. }
            | HirArenaError::SnapshotMismatch { .. }
            | HirArenaError::TransactionMismatch { .. }
            | HirArenaError::MissingPayload { .. }
            | HirArenaError::BaseSnapshotMismatch { .. }
            | HirArenaError::TransactionPoisoned => {
                Self::Invariant(HirInvariantFailure::InvalidArenaCommit)
            }
        }
    }
}

impl From<HirSourceCommitInvariantError> for HirLowerFailure {
    fn from(_: HirSourceCommitInvariantError) -> Self {
        Self::Invariant(HirInvariantFailure::InvalidSourceIndex)
    }
}

#[cfg(test)]
#[path = "final_lowering/tests.rs"]
mod tests;
