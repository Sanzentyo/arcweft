//! Immutable ownership of one accepted HIR module snapshot.

mod capture_validation;
mod local_resolution;
mod resolution;

pub(crate) use local_resolution::HirLocalResolver;

use core::num::NonZeroU64;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use arcweft_lang_syntax::attachment::SyntaxSnapshotId;
use arcweft_lang_syntax::attachment::source_file::SourceFileEntryNode;
use arcweft_lang_syntax::incremental::{ParseStatus, ParsedSource};
use arcweft_source::identity::SourceSnapshotId;
use arcweft_source::{SourceDocument, SourceDocumentIdentity};

use crate::arena::ArenaSnapshot;
use crate::diagnostic::{HirDiagnostic, HirRecoveryDiagnostic, HirRecoveryPrimary};
use crate::expr::{
    HirExpr, HirExprKind, HirExpressionRecoveryIssue, HirPoisonState, HirRecoveryIssue,
    HirSelectedMember, HirThreadBody, HirThreadBodyOwner,
};
use crate::identity::{
    CaptureId, ExprId, HirLimit, HirModuleId, HirSnapshotId, ItemId, LocalId, PatternId, ScopeId,
    StmtId, SyntheticOwner, SyntheticRole, TypeId,
};
#[cfg(test)]
use crate::item::HirDeclarationMemberIndexBuilder;
use crate::item::{HirDeclarationMember, HirDeclarationMemberIndex, HirItem, HirItemKind};
use crate::line_identity::HirDialogueLineCandidates;
use crate::lowering::{HirInvariantFailure, HirLimitError, HirLowerFailure, HirModuleKey};
use crate::pattern::HirPattern;
use crate::scope::{HirCapture, HirLocal, HirLocalKind, HirScope, HirScopeKind, HirScopeOwner};
use crate::slot::{HirOrigin, HirSlotError, HirSlotMetadata, SlotSnapshot};
use crate::source_index::{
    HirExprSourceRole, HirItemSourceRole, HirLocalSourceRole, HirPatternSourceRole,
    HirResolvedSourceRole, HirScopeSourceRole, HirSourceIndex, HirSourceIndexLookupError,
    HirSourceLookup, HirSourceOwnerStatus, HirSourcePresence, HirSourceQuery, HirSourceQueryError,
    HirStmtSourceRole, HirTestBenchSourceRole, HirThreadBodySourceRole,
    HirThreadFlowItemSourcePart, HirTypeSourceRole, ItemValidationArenas,
};
use crate::stmt::HirStmt;
use crate::type_ref::HirType;

/// Executability of one complete immutable HIR module snapshot.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirModuleStatus {
    Clean,
    Recovered,
}

/// Exact attached syntax and immutable source accepted by one HIR snapshot.
pub struct HirModuleProvenance {
    syntax_snapshot: SyntaxSnapshotId,
    source_snapshot: SourceSnapshotId,
    source_identity: SourceDocumentIdentity,
    document: Arc<SourceDocument>,
}

impl HirModuleProvenance {
    fn try_from_parsed(key: &HirModuleKey, parsed: &ParsedSource) -> Result<Self, HirLowerFailure> {
        if parsed.snapshot_id().source() != parsed.source_snapshot_id()
            || parsed.source_snapshot_id().name() != parsed.document().display_name()
            || key.source() != parsed.document().identity()
        {
            return Err(HirInvariantFailure::InvalidModuleProvenance.into());
        }
        Ok(Self {
            syntax_snapshot: parsed.snapshot_id().clone(),
            source_snapshot: parsed.source_snapshot_id().clone(),
            source_identity: parsed.document().identity().clone(),
            document: Arc::clone(parsed.document_lease()),
        })
    }

    pub const fn syntax_snapshot(&self) -> &SyntaxSnapshotId {
        &self.syntax_snapshot
    }

    pub const fn source_snapshot(&self) -> &SourceSnapshotId {
        &self.source_snapshot
    }

    pub const fn source_identity(&self) -> &SourceDocumentIdentity {
        &self.source_identity
    }

    pub const fn document(&self) -> &Arc<SourceDocument> {
        &self.document
    }
}

/// The eight exact typed arena snapshots owned by one HIR module revision.
pub(crate) struct HirModuleArenas {
    items: ArenaSnapshot<HirItem, ItemId>,
    scopes: ArenaSnapshot<HirScope, ScopeId>,
    locals: ArenaSnapshot<HirLocal, LocalId>,
    expressions: ArenaSnapshot<HirExpr, ExprId>,
    statements: ArenaSnapshot<HirStmt, StmtId>,
    types: ArenaSnapshot<HirType, TypeId>,
    patterns: ArenaSnapshot<HirPattern, PatternId>,
    captures: ArenaSnapshot<HirCapture, CaptureId>,
}

pub(crate) struct HirModuleArenaParts {
    pub(crate) items: ArenaSnapshot<HirItem, ItemId>,
    pub(crate) scopes: ArenaSnapshot<HirScope, ScopeId>,
    pub(crate) locals: ArenaSnapshot<HirLocal, LocalId>,
    pub(crate) expressions: ArenaSnapshot<HirExpr, ExprId>,
    pub(crate) statements: ArenaSnapshot<HirStmt, StmtId>,
    pub(crate) types: ArenaSnapshot<HirType, TypeId>,
    pub(crate) patterns: ArenaSnapshot<HirPattern, PatternId>,
    pub(crate) captures: ArenaSnapshot<HirCapture, CaptureId>,
}

impl HirModuleArenas {
    pub(crate) fn try_new(
        slots: &SlotSnapshot,
        parts: HirModuleArenaParts,
    ) -> Result<Self, HirLowerFailure> {
        let HirModuleArenaParts {
            items,
            scopes,
            locals,
            expressions,
            statements,
            types,
            patterns,
            captures,
        } = parts;
        if !items.validates_prepared(slots)
            || !scopes.validates_prepared(slots)
            || !locals.validates_prepared(slots)
            || !expressions.validates_prepared(slots)
            || !statements.validates_prepared(slots)
            || !types.validates_prepared(slots)
            || !patterns.validates_prepared(slots)
            || !captures.validates_prepared(slots)
        {
            return Err(HirInvariantFailure::InvalidModuleArenaSnapshot.into());
        }
        let arenas = Self {
            items,
            scopes,
            locals,
            expressions,
            statements,
            types,
            patterns,
            captures,
        };
        if !arenas.validates_scope_graph(slots) || !arenas.validates_capture_graph(slots) {
            return Err(HirInvariantFailure::InvalidModuleArenaSnapshot.into());
        }
        Ok(arenas)
    }

    #[cfg(test)]
    fn empty(slots: &SlotSnapshot) -> Self {
        Self {
            items: ArenaSnapshot::empty(slots),
            scopes: ArenaSnapshot::empty(slots),
            locals: ArenaSnapshot::empty(slots),
            expressions: ArenaSnapshot::empty(slots),
            statements: ArenaSnapshot::empty(slots),
            types: ArenaSnapshot::empty(slots),
            patterns: ArenaSnapshot::empty(slots),
            captures: ArenaSnapshot::empty(slots),
        }
    }

    fn validates_prepared(&self, slots: &SlotSnapshot) -> bool {
        self.items.validates_prepared(slots)
            && self.scopes.validates_prepared(slots)
            && self.locals.validates_prepared(slots)
            && self.expressions.validates_prepared(slots)
            && self.statements.validates_prepared(slots)
            && self.types.validates_prepared(slots)
            && self.patterns.validates_prepared(slots)
            && self.captures.validates_prepared(slots)
    }

    fn validates_payload_poison(&self, slots: &SlotSnapshot) -> bool {
        validate_arena_poison(&self.items, slots, HirItem::is_poisoned)
            && validate_arena_poison(&self.scopes, slots, |_| false)
            && validate_arena_poison(&self.locals, slots, HirLocal::is_poisoned)
            && validate_arena_poison(&self.expressions, slots, HirExpr::is_poisoned)
            && validate_arena_poison(&self.statements, slots, HirStmt::is_poisoned)
            && validate_arena_poison(&self.types, slots, HirType::is_poisoned)
            && validate_arena_poison(&self.patterns, slots, HirPattern::is_poisoned)
            && validate_arena_poison(&self.captures, slots, |_| false)
    }

    #[allow(
        clippy::too_many_lines,
        reason = "one closed validation pass proves root, parent, local, timeline, and capture scope-graph invariants together"
    )]
    fn validates_scope_graph(&self, slots: &SlotSnapshot) -> bool {
        let Ok(scope_entries) = self.scopes.try_iter_prepared(slots) else {
            return false;
        };
        let scopes = scope_entries.collect::<BTreeMap<_, _>>();
        let Ok(local_entries) = self.locals.try_iter_prepared(slots) else {
            return false;
        };
        let locals = local_entries.collect::<BTreeMap<_, _>>();
        let mut roots = scopes
            .iter()
            .filter_map(|(&scope_id, scope)| scope.parent().is_none().then_some(scope_id));
        let Some(root) = roots.next() else {
            return false;
        };
        if roots.next().is_some() {
            return false;
        }
        let Some(root_scope) = scopes.get(&root) else {
            return false;
        };
        if root_scope.kind() != crate::scope::HirScopeKind::Module
            || root_scope.owner() != &HirScopeOwner::Module(slots.snapshot_id().module())
        {
            return false;
        }

        let mut visited_scopes = BTreeSet::new();
        let mut referenced_locals = BTreeSet::new();
        let mut pending = vec![(root, None::<ItemId>)];
        while let Some((scope_id, inherited_item)) = pending.pop() {
            if !visited_scopes.insert(scope_id) {
                return false;
            }
            let Some(scope) = scopes.get(&scope_id).copied() else {
                return false;
            };
            if !scope.has_admitted_owner() {
                return false;
            }

            let descendant_item = match *scope.owner() {
                HirScopeOwner::Module(module) => {
                    if scope_id != root
                        || module != slots.snapshot_id().module()
                        || scope.parent().is_some()
                    {
                        return false;
                    }
                    inherited_item
                }
                HirScopeOwner::Item(owner) => {
                    if scope.parent().is_none()
                        || self.items.resolve_prepared(slots, owner).is_err()
                        || inherited_item.is_some_and(|ancestor| ancestor != owner)
                    {
                        return false;
                    }
                    Some(owner)
                }
                HirScopeOwner::Expr(owner) => {
                    let Ok(expression) = self.expressions.resolve_prepared(slots, owner) else {
                        return false;
                    };
                    if scope.parent() != Some(expression.scope()) {
                        return false;
                    }
                    inherited_item
                }
                HirScopeOwner::Stmt(owner) => {
                    let Ok(statement) = self.statements.resolve_prepared(slots, owner) else {
                        return false;
                    };
                    let Some(parent_id) = scope.parent() else {
                        return false;
                    };
                    let directly_nested = parent_id == statement.scope();
                    let nested_under_same_owner = scopes
                        .get(&parent_id)
                        .is_some_and(|parent| parent.owner() == &HirScopeOwner::Stmt(owner));
                    if !directly_nested && !nested_under_same_owner {
                        return false;
                    }
                    inherited_item
                }
            };

            for &local_id in scope.locals() {
                let Some(local) = locals.get(&local_id).copied() else {
                    return false;
                };
                if local.scope() != scope_id || !referenced_locals.insert(local_id) {
                    return false;
                }
            }
            for &child in scope.children().iter().rev() {
                let Some(child_scope) = scopes.get(&child).copied() else {
                    return false;
                };
                if child_scope.parent() != Some(scope_id) {
                    return false;
                }
                pending.push((child, descendant_item));
            }
        }

        if visited_scopes.len() != scopes.len() {
            return false;
        }
        referenced_locals.len() == locals.len()
    }

    fn validates_member_index(
        &self,
        slots: &SlotSnapshot,
        members: &HirDeclarationMemberIndex,
    ) -> bool {
        let Ok(items) = self.items.try_iter_prepared(slots) else {
            return false;
        };
        for (id, item) in items {
            match (item.members().is_empty(), members.arena(id)) {
                (true, None) => {}
                (false, Some(arena))
                    if arena.family() == item.family()
                        && item
                            .members()
                            .iter()
                            .zip(arena.members())
                            .all(|(expected, actual)| *expected == actual.id())
                        && item.members().len() == arena.members().len()
                        && (item.family() != crate::item::HirItemFamily::Layer
                            || item.is_poisoned()
                            || !arena
                                .members()
                                .iter()
                                .any(HirDeclarationMember::is_poisoned)) => {}
                _ => return false,
            }
        }
        members
            .arenas()
            .keys()
            .all(|owner| self.items.resolve_prepared(slots, *owner).is_ok())
    }

    fn validates_source_ordered_items(
        &self,
        slots: &SlotSnapshot,
        source_ordered_items: &[ItemId],
        parsed: &ParsedSource,
    ) -> bool {
        let Ok(entries) = parsed.entries() else {
            return false;
        };
        let mut authored_positions = BTreeMap::new();
        for (position, entry) in entries
            .into_iter()
            .filter(|entry| !matches!(entry, SourceFileEntryNode::Attribute(_)))
            .enumerate()
        {
            if authored_positions.insert(entry.id(), position).is_some() {
                return false;
            }
        }

        let mut seen = BTreeSet::new();
        let mut previous_position = None;
        for &item in source_ordered_items {
            if item.module() != slots.snapshot_id().module() || !seen.insert(item) {
                return false;
            }
            if self.items.resolve_prepared(slots, item).is_err() {
                return false;
            }
            let Ok(metadata) = slots.resolve_prepared(item) else {
                return false;
            };
            let HirOrigin::Source(origin) = metadata.origin() else {
                return false;
            };
            let Some(&position) = authored_positions.get(&origin.syntax()) else {
                return false;
            };
            if previous_position.is_some_and(|previous| previous >= position) {
                return false;
            }
            previous_position = Some(position);
        }

        self.items.try_iter_prepared(slots).is_ok_and(|items| {
            items.into_iter().all(|(item, _)| {
                slots.resolve_prepared(item).is_ok_and(|metadata| {
                    matches!(metadata.origin(), HirOrigin::Source(_)) == seen.contains(&item)
                })
            })
        })
    }

    pub(crate) const fn items(&self) -> &ArenaSnapshot<HirItem, ItemId> {
        &self.items
    }

    pub(crate) const fn scopes(&self) -> &ArenaSnapshot<HirScope, ScopeId> {
        &self.scopes
    }

    pub(crate) const fn locals(&self) -> &ArenaSnapshot<HirLocal, LocalId> {
        &self.locals
    }

    pub(crate) const fn expressions(&self) -> &ArenaSnapshot<HirExpr, ExprId> {
        &self.expressions
    }

    pub(crate) const fn statements(&self) -> &ArenaSnapshot<HirStmt, StmtId> {
        &self.statements
    }

    pub(crate) const fn types(&self) -> &ArenaSnapshot<HirType, TypeId> {
        &self.types
    }

    pub(crate) const fn patterns(&self) -> &ArenaSnapshot<HirPattern, PatternId> {
        &self.patterns
    }

    pub(crate) const fn captures(&self) -> &ArenaSnapshot<HirCapture, CaptureId> {
        &self.captures
    }
}

/// Immutable, module-qualified HIR snapshot.
///
/// This is the sole immutable owner of attached provenance, all eight typed
/// arenas, declaration members, typed source components, recoverable
/// diagnostics, slot lifetimes, and the invalidation epoch for one revision.
/// Public consumers share one exact `Arc<HirModule>` rather than cloning or
/// relinking semantic payloads.
pub struct HirModule {
    snapshot: HirSnapshotId,
    key: HirModuleKey,
    provenance: HirModuleProvenance,
    status: HirModuleStatus,
    diagnostics: Arc<[HirDiagnostic]>,
    slots: Arc<SlotSnapshot>,
    arenas: HirModuleArenas,
    source_ordered_items: Box<[ItemId]>,
    declaration_members: HirDeclarationMemberIndex,
    source_components: HirSourceIndex,
    dialogue_line_candidates: HirDialogueLineCandidates,
    invalidation_epoch: NonZeroU64,
}

impl HirModule {
    #[allow(
        clippy::too_many_arguments,
        reason = "the constructor atomically validates every owner of the immutable published module schema"
    )]
    pub(crate) fn try_new(
        snapshot: HirSnapshotId,
        key: HirModuleKey,
        parsed: &ParsedSource,
        diagnostics: Arc<[HirDiagnostic]>,
        slots: Arc<SlotSnapshot>,
        arenas: HirModuleArenas,
        source_ordered_items: Box<[ItemId]>,
        declaration_members: HirDeclarationMemberIndex,
        source_components: HirSourceIndex,
        invalidation_epoch: NonZeroU64,
    ) -> Result<Self, HirLowerFailure> {
        if slots.snapshot_id() != snapshot
            || !arenas.validates_prepared(&slots)
            || !arenas.validates_payload_poison(&slots)
        {
            return Err(HirInvariantFailure::InvalidModuleCommit.into());
        }
        let provenance = HirModuleProvenance::try_from_parsed(&key, parsed)?;
        if !slots.validates_provenance(parsed) {
            return Err(HirInvariantFailure::InvalidModuleProvenance.into());
        }
        if declaration_members.module() != snapshot.module()
            || !arenas.validates_member_index(&slots, &declaration_members)
        {
            return Err(HirInvariantFailure::InvalidDeclarationMemberIndex.into());
        }
        if !source_components.validates_prepared(&slots, &provenance.source_identity)
            || !source_components.validates_attached_items(
                parsed,
                &slots,
                arenas.items(),
                &declaration_members,
                &ItemValidationArenas {
                    scopes: arenas.scopes(),
                    locals: arenas.locals(),
                    expressions: arenas.expressions(),
                    statements: arenas.statements(),
                    patterns: arenas.patterns(),
                    types: arenas.types(),
                },
            )
            || !source_components.validates_attached_expressions(
                parsed,
                &slots,
                arenas.items(),
                arenas.expressions(),
                arenas.types(),
                arenas.statements(),
                arenas.scopes(),
                arenas.locals(),
                arenas.patterns(),
            )
            || !source_components.validates_attached_patterns(parsed, &slots, arenas.patterns())
            || !source_components.validates_attached_types(
                parsed,
                &slots,
                arenas.items(),
                arenas.types(),
            )
            || !source_components.validates_attached_statements(parsed, &slots, arenas.statements())
            || !source_components.validates_attached_thread_bodies(
                parsed,
                &slots,
                arenas.items(),
                arenas.expressions(),
                arenas.statements(),
                arenas.scopes(),
                arenas.locals(),
                arenas.patterns(),
            )
        {
            return Err(HirInvariantFailure::InvalidSourceIndex.into());
        }
        validate_diagnostics(
            parsed,
            &provenance.source_identity,
            &slots,
            &arenas,
            &source_components,
            &diagnostics,
        )?;
        if !arenas.validates_source_ordered_items(&slots, &source_ordered_items, parsed) {
            return Err(HirInvariantFailure::InvalidSourceOrderedItems.into());
        }
        let status = if parsed.status() == ParseStatus::Recovered || slots.has_poisoned_live_slots()
        {
            HirModuleStatus::Recovered
        } else {
            HirModuleStatus::Clean
        };
        let empty_dialogue_lines = HirDialogueLineCandidates::empty(key.clone());
        let mut module = Self {
            snapshot,
            key,
            provenance,
            status,
            diagnostics,
            slots,
            arenas,
            source_ordered_items,
            declaration_members,
            source_components,
            dialogue_line_candidates: empty_dialogue_lines,
            invalidation_epoch,
        };
        attach_dialogue_line_candidates(&mut module, parsed.diagnostics().len())?;
        Ok(module)
    }

    /// Exact immutable module revision represented by this snapshot.
    pub fn snapshot_id(&self) -> HirSnapshotId {
        debug_assert_eq!(self.snapshot, self.slots.snapshot_id());
        self.snapshot
    }

    /// Stable module identity qualifying every typed ID in this snapshot.
    pub const fn module_id(&self) -> HirModuleId {
        self.snapshot.module()
    }

    /// Package, canonical path, and logical document admitted for this module.
    pub const fn key(&self) -> &HirModuleKey {
        &self.key
    }

    pub const fn provenance(&self) -> &HirModuleProvenance {
        &self.provenance
    }

    /// Recovery status of this complete snapshot.
    pub const fn status(&self) -> HirModuleStatus {
        self.status
    }

    pub fn diagnostics(&self) -> &[HirDiagnostic] {
        &self.diagnostics
    }

    pub(crate) const fn dialogue_line_candidates(&self) -> &HirDialogueLineCandidates {
        &self.dialogue_line_candidates
    }

    /// Whether semantic, verifier, compiler, and runtime consumers may execute it.
    pub const fn is_executable(&self) -> bool {
        matches!(self.status, HirModuleStatus::Clean)
    }

    /// Whether persistent compilation caches may admit this snapshot.
    pub const fn is_cache_eligible(&self) -> bool {
        matches!(self.status, HirModuleStatus::Clean)
    }

    /// Monotonic cache-invalidation generation published with this snapshot.
    pub const fn invalidation_epoch(&self) -> NonZeroU64 {
        self.invalidation_epoch
    }

    /// Source-backed top-level items in exact authored order.
    ///
    /// Arena iteration remains raw-slot ordered and must not be used as a
    /// source-order projection. Synthetic items and declaration members are
    /// intentionally absent from this sequence.
    pub fn source_ordered_items(&self) -> &[ItemId] {
        &self.source_ordered_items
    }

    pub(crate) const fn slots(&self) -> &Arc<SlotSnapshot> {
        &self.slots
    }

    pub(crate) const fn arenas(&self) -> &HirModuleArenas {
        &self.arenas
    }

    pub(crate) const fn source_components(&self) -> &HirSourceIndex {
        &self.source_components
    }

    pub const fn declaration_members(&self) -> &HirDeclarationMemberIndex {
        &self.declaration_members
    }

    /// Resolves one typed source role through the module's sole immutable
    /// source-role manifest.
    #[allow(
        clippy::needless_pass_by_value,
        reason = "the public lookup boundary consumes one typed query value and never retains a borrowed caller carrier"
    )]
    pub fn source_site(
        &self,
        expected_source: &SourceDocumentIdentity,
        query: HirSourceQuery,
    ) -> Result<HirSourceLookup<'_>, HirSourceQueryError> {
        match self.source_components.lookup(
            &self.provenance.source_identity,
            expected_source,
            &query,
            |query| self.resolve_source_role(query),
        ) {
            Ok(lookup) => Ok(lookup),
            Err(HirSourceIndexLookupError::Query(error)) => Err(error),
            Err(HirSourceIndexLookupError::Invariant(error)) => {
                unreachable!("validated HIR source manifest failed lookup: {error}")
            }
        }
    }

    #[allow(
        clippy::too_many_lines,
        reason = "one exhaustive dispatcher validates the complete typed source-role family before projection"
    )]
    fn resolve_source_role(
        &self,
        query: &HirSourceQuery,
    ) -> Result<HirResolvedSourceRole<'_>, HirSourceQueryError> {
        let metadata = match query {
            HirSourceQuery::Item { owner, role } => {
                let metadata = self.resolve_source_owner(query, *owner)?;
                let payload = self
                    .arenas
                    .items
                    .resolve(&self.slots, *owner)
                    .expect("published item slot has its validated payload");
                match (payload.kind(), role) {
                    (kind, HirItemSourceRole::Declaration(declaration_role)) => {
                        kind.validate_declaration_source_role(*owner, *declaration_role)?;
                    }
                    (kind, HirItemSourceRole::Entry(entry_part)) => {
                        kind.validate_entry_source_part(*owner, *entry_part)?;
                    }
                    (kind, HirItemSourceRole::Callable(callable_role)) => {
                        kind.validate_callable_source_role(*owner, *callable_role)?;
                    }
                    (HirItemKind::Use(declaration), HirItemSourceRole::Use(use_role)) => {
                        declaration.validate_use_source_role(*owner, *use_role)?;
                    }
                    (
                        HirItemKind::Test(_) | HirItemKind::Bench(_),
                        HirItemSourceRole::TestBench(HirTestBenchSourceRole::Whole),
                    ) => {}
                    (HirItemKind::Flow(flow), HirItemSourceRole::Flow(flow_role)) => {
                        self.source_components
                            .validate_flow_source_role(flow, *owner, *flow_role)?;
                    }
                    (HirItemKind::Style(style), HirItemSourceRole::Style(style_role)) => {
                        style.validate_source_role(*owner, style_role)?;
                    }
                    (HirItemKind::View(view), HirItemSourceRole::View(view_role)) => {
                        view.validate_source_role(*owner, *view_role)?;
                    }
                    _ => return Err(HirSourceQueryError::role_not_applicable(query)),
                }
                metadata
            }
            HirSourceQuery::Expr { owner, role } => {
                let metadata = self.resolve_source_owner(query, *owner)?;
                let payload = self
                    .arenas
                    .expressions
                    .resolve(&self.slots, *owner)
                    .expect("published expression slot has its validated payload");
                let target_call_argument_count =
                    if let HirExprKind::DialogueContentApplication(application) = payload.kind() {
                        let target = self
                            .arenas
                            .expressions
                            .resolve(&self.slots, application.target())
                            .expect("published dialogue target has its validated payload");
                        Some(match target.kind() {
                            HirExprKind::Call(call) => call.arguments().len(),
                            _ => 0,
                        })
                    } else {
                        None
                    };
                payload.kind().validate_source_role_with_context(
                    *owner,
                    *role,
                    target_call_argument_count,
                )?;
                metadata
            }
            HirSourceQuery::Pattern { owner, role } => {
                let metadata = self.resolve_source_owner(query, *owner)?;
                let payload = self
                    .arenas
                    .patterns
                    .resolve(&self.slots, *owner)
                    .expect("published pattern slot has its validated payload");
                payload.kind().validate_source_role(*owner, *role)?;
                metadata
            }
            HirSourceQuery::Type { owner, role } => {
                let metadata = self.resolve_source_owner(query, *owner)?;
                let payload = self
                    .arenas
                    .types
                    .resolve(&self.slots, *owner)
                    .expect("published type slot has its validated payload");
                payload.kind().validate_source_role(*owner, *role)?;
                metadata
            }
            HirSourceQuery::Stmt { owner, role } => {
                let metadata = self.resolve_source_owner(query, *owner)?;
                let payload = self
                    .arenas
                    .statements
                    .resolve(&self.slots, *owner)
                    .expect("published statement slot has its validated payload");
                payload.kind().validate_source_role(*owner, *role)?;
                metadata
            }
            HirSourceQuery::Scope { owner, role } => {
                return self.resolve_scope_source_role(query, *owner, *role);
            }
            HirSourceQuery::Local { owner, role } => {
                return self.resolve_local_source_role(query, *owner, *role);
            }
            HirSourceQuery::ThreadBody { owner, role } => {
                return self.resolve_thread_body_source_role(query, *owner, *role);
            }
        };
        let status = if metadata.is_poisoned() {
            HirSourceOwnerStatus::Poisoned
        } else {
            HirSourceOwnerStatus::Clean
        };
        if query.is_slot_whole() {
            return Ok(HirResolvedSourceRole::whole(metadata.source_site(), status));
        }
        let requirement = self
            .source_components
            .requirement(query)
            .ok_or_else(|| HirSourceQueryError::role_not_applicable(query))?;
        Ok(HirResolvedSourceRole::component(requirement, status))
    }

    fn resolve_source_owner<I: crate::identity::HirTypedId>(
        &self,
        query: &HirSourceQuery,
        owner: I,
    ) -> Result<&HirSlotMetadata, HirSourceQueryError> {
        match self.slots.resolve(owner) {
            Ok(metadata) => Ok(metadata),
            Err(HirSlotError::Resolve(error)) => Err(HirSourceQueryError::resolve(query, error)),
            Err(error) => unreachable!("validated HIR source owner failed resolution: {error}"),
        }
    }

    fn resolve_scope_source_role(
        &self,
        query: &HirSourceQuery,
        owner: ScopeId,
        role: HirScopeSourceRole,
    ) -> Result<HirResolvedSourceRole<'_>, HirSourceQueryError> {
        let metadata = self.resolve_source_owner(query, owner)?;
        let scope = self
            .arenas
            .scopes
            .resolve(&self.slots, owner)
            .expect("published scope slot has its validated payload");
        let status = source_owner_status(metadata);
        match role {
            HirScopeSourceRole::Whole => {
                Ok(HirResolvedSourceRole::whole(metadata.source_site(), status))
            }
            HirScopeSourceRole::SyntheticOrigin if scope_has_synthetic_origin(scope, metadata) => {
                Ok(HirResolvedSourceRole::related(
                    HirSourcePresence::Present(metadata.source_site()),
                    status,
                ))
            }
            HirScopeSourceRole::OpenDelimiter | HirScopeSourceRole::CloseDelimiter => {
                let Some((body_owner, _)) =
                    published_thread_body_for_scope(&self.slots, &self.arenas, owner, scope)
                else {
                    return Err(HirSourceQueryError::role_not_applicable(query));
                };
                let body_role = match role {
                    HirScopeSourceRole::OpenDelimiter => HirThreadBodySourceRole::OpenDelimiter,
                    HirScopeSourceRole::CloseDelimiter => HirThreadBodySourceRole::CloseDelimiter,
                    HirScopeSourceRole::Whole | HirScopeSourceRole::SyntheticOrigin => {
                        unreachable!("scope body relation is delimiter-only")
                    }
                };
                let body_query = HirSourceQuery::ThreadBody {
                    owner: body_owner,
                    role: body_role,
                };
                self.source_components
                    .component_presence(&body_query)
                    .map(|presence| HirResolvedSourceRole::related(presence, status))
                    .ok_or_else(|| HirSourceQueryError::role_not_applicable(query))
            }
            HirScopeSourceRole::SyntheticOrigin => {
                Err(HirSourceQueryError::role_not_applicable(query))
            }
        }
    }

    fn resolve_local_source_role(
        &self,
        query: &HirSourceQuery,
        owner: LocalId,
        role: HirLocalSourceRole,
    ) -> Result<HirResolvedSourceRole<'_>, HirSourceQueryError> {
        let metadata = self.resolve_source_owner(query, owner)?;
        let local = self
            .arenas
            .locals
            .resolve(&self.slots, owner)
            .expect("published local slot has its validated payload");
        let status = source_owner_status(metadata);
        match role {
            HirLocalSourceRole::Whole => {
                Ok(HirResolvedSourceRole::whole(metadata.source_site(), status))
            }
            HirLocalSourceRole::SyntheticOrigin
                if local.kind() == HirLocalKind::PostconditionResult
                    && local_has_synthetic_origin(local, metadata) =>
            {
                Ok(HirResolvedSourceRole::related(
                    HirSourcePresence::Present(metadata.source_site()),
                    status,
                ))
            }
            HirLocalSourceRole::Name if local.kind() != HirLocalKind::PostconditionResult => {
                Ok(HirResolvedSourceRole::related(
                    HirSourcePresence::Present(metadata.source_site()),
                    status,
                ))
            }
            HirLocalSourceRole::Pattern if local.kind() != HirLocalKind::PostconditionResult => {
                let Some(pattern) = local.pattern() else {
                    return Err(HirSourceQueryError::role_not_applicable(query));
                };
                let pattern = self.resolve_source_owner(query, pattern)?;
                Ok(HirResolvedSourceRole::related(
                    HirSourcePresence::Present(pattern.source_site()),
                    status,
                ))
            }
            HirLocalSourceRole::Type if local.kind() != HirLocalKind::PostconditionResult => {
                let presence = match local.annotation() {
                    Some(annotation) => HirSourcePresence::Present(
                        self.resolve_source_owner(query, annotation)?.source_site(),
                    ),
                    None => HirSourcePresence::AbsentOptional,
                };
                Ok(HirResolvedSourceRole::related(presence, status))
            }
            HirLocalSourceRole::Name
            | HirLocalSourceRole::Type
            | HirLocalSourceRole::Pattern
            | HirLocalSourceRole::SyntheticOrigin => {
                Err(HirSourceQueryError::role_not_applicable(query))
            }
        }
    }

    fn resolve_thread_body_source_role(
        &self,
        query: &HirSourceQuery,
        owner: HirThreadBodyOwner,
        role: HirThreadBodySourceRole,
    ) -> Result<HirResolvedSourceRole<'_>, HirSourceQueryError> {
        let resolved = self.resolve_thread_body(query, owner)?;
        let status = source_owner_status(resolved.owner_metadata);
        match role {
            HirThreadBodySourceRole::Whole => {
                let scope = self.resolve_source_owner(query, resolved.body.scope())?;
                Ok(HirResolvedSourceRole::related(
                    HirSourcePresence::Present(scope.source_site()),
                    status,
                ))
            }
            HirThreadBodySourceRole::OpenDelimiter | HirThreadBodySourceRole::CloseDelimiter => {
                self.source_components
                    .requirement(query)
                    .map(|requirement| HirResolvedSourceRole::component(requirement, status))
                    .ok_or_else(|| HirSourceQueryError::role_not_applicable(query))
            }
            HirThreadBodySourceRole::Item { ordinal, part } => {
                let Some(item) = usize::try_from(ordinal)
                    .ok()
                    .and_then(|ordinal| resolved.body.items().get(ordinal))
                else {
                    let length = u32::try_from(resolved.body.items().len())
                        .expect("Thread body length is bounded below u32::MAX");
                    return Err(HirSourceQueryError::ThreadBodyOrdinalOutOfBounds {
                        owner,
                        role,
                        length,
                    });
                };
                match part {
                    HirThreadFlowItemSourcePart::Whole => self
                        .source_components
                        .requirement(query)
                        .map(|requirement| HirResolvedSourceRole::component(requirement, status))
                        .ok_or_else(|| HirSourceQueryError::role_not_applicable(query)),
                    HirThreadFlowItemSourcePart::ChildWhole => {
                        let child = self
                            .resolve_thread_flow_item_metadata(query, item)
                            .expect("published Thread body has a validated child owner");
                        Ok(HirResolvedSourceRole::related(
                            HirSourcePresence::Present(child.source_site()),
                            status,
                        ))
                    }
                }
            }
        }
    }

    fn resolve_thread_body<'a>(
        &'a self,
        query: &HirSourceQuery,
        owner: HirThreadBodyOwner,
    ) -> Result<ResolvedThreadBody<'a>, HirSourceQueryError> {
        let (body, owner_metadata) = match owner {
            HirThreadBodyOwner::Flow(owner) => {
                let metadata = self.resolve_source_owner(query, owner)?;
                let item = self
                    .arenas
                    .items
                    .resolve(&self.slots, owner)
                    .expect("published Flow slot has its validated payload");
                let HirItemKind::Flow(flow) = item.kind() else {
                    return Err(HirSourceQueryError::role_not_applicable(query));
                };
                (flow.body(), metadata)
            }
            HirThreadBodyOwner::ThreadExpression(owner) => {
                let metadata = self.resolve_source_owner(query, owner)?;
                let expression = self
                    .arenas
                    .expressions
                    .resolve(&self.slots, owner)
                    .expect("published Thread slot has its validated payload");
                let HirExprKind::Thread(thread) = expression.kind() else {
                    return Err(HirSourceQueryError::role_not_applicable(query));
                };
                (thread.body(), metadata)
            }
            HirThreadBodyOwner::NestedScope(scope_id) => {
                let _scope_metadata = self.resolve_source_owner(query, scope_id)?;
                let scope = self
                    .arenas
                    .scopes
                    .resolve(&self.slots, scope_id)
                    .expect("published nested scope has its validated payload");
                let Some((HirThreadBodyOwner::NestedScope(_), body)) =
                    published_thread_body_for_scope(&self.slots, &self.arenas, scope_id, scope)
                else {
                    return Err(HirSourceQueryError::role_not_applicable(query));
                };
                let metadata = match scope.owner() {
                    HirScopeOwner::Expr(owner) => self.resolve_source_owner(query, *owner)?,
                    HirScopeOwner::Stmt(owner) => self.resolve_source_owner(query, *owner)?,
                    HirScopeOwner::Module(_) | HirScopeOwner::Item(_) => {
                        return Err(HirSourceQueryError::role_not_applicable(query));
                    }
                };
                (body, metadata)
            }
        };
        if body.scope().module() != self.module_id() {
            return Err(HirSourceQueryError::role_not_applicable(query));
        }
        Ok(ResolvedThreadBody {
            body,
            owner_metadata,
        })
    }

    fn resolve_thread_flow_item_metadata<'a>(
        &'a self,
        query: &HirSourceQuery,
        item: &crate::expr::HirThreadFlowItem,
    ) -> Result<&'a HirSlotMetadata, HirSourceQueryError> {
        match item {
            crate::expr::HirThreadFlowItem::DialogueApplication(owner) => {
                self.resolve_source_owner(query, *owner)
            }
            crate::expr::HirThreadFlowItem::Statement(owner)
            | crate::expr::HirThreadFlowItem::Choice(owner)
            | crate::expr::HirThreadFlowItem::If(owner)
            | crate::expr::HirThreadFlowItem::IfLet(owner)
            | crate::expr::HirThreadFlowItem::Match(owner)
            | crate::expr::HirThreadFlowItem::Loop(owner)
            | crate::expr::HirThreadFlowItem::While(owner)
            | crate::expr::HirThreadFlowItem::WhileLet(owner)
            | crate::expr::HirThreadFlowItem::For(owner)
            | crate::expr::HirThreadFlowItem::Select(owner)
            | crate::expr::HirThreadFlowItem::SourceLocale(owner)
            | crate::expr::HirThreadFlowItem::Scope(owner)
            | crate::expr::HirThreadFlowItem::Include(owner)
            | crate::expr::HirThreadFlowItem::AwaitWith(owner)
            | crate::expr::HirThreadFlowItem::Error(owner) => {
                self.resolve_source_owner(query, *owner)
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn from_validated_parts(
        snapshot: HirSnapshotId,
        key: HirModuleKey,
        status: HirModuleStatus,
        slots: Arc<SlotSnapshot>,
        invalidation_epoch: NonZeroU64,
    ) -> Result<Self, HirLowerFailure> {
        use arcweft_lang_syntax::incremental::SyntaxDatabase;
        use arcweft_source::SourceName;
        use arcweft_source::identity::SourceSnapshotId;

        if status != HirModuleStatus::Clean {
            return Err(HirInvariantFailure::InvalidModuleStatus.into());
        }
        let name = SourceName::path("proof/hir-module-test.arcw");
        let document = Arc::new(
            SourceDocument::try_new(key.source().id().clone(), name.clone(), "")
                .expect("test source identity is valid"),
        );
        let mut syntax = SyntaxDatabase::try_new().expect("test syntax database");
        let parsed = syntax
            .parse_initial(
                SourceSnapshotId::initial(name),
                document,
                arcweft_lang_syntax::parser::ParseOptions::default(),
            )
            .expect("empty test source parses");
        let arenas = HirModuleArenas::empty(&slots);
        let source_components = HirSourceIndex::empty(parsed.document().identity().clone(), &slots);
        Self::try_new(
            snapshot,
            key,
            &parsed,
            Arc::from([]),
            slots,
            arenas,
            Box::new([]),
            HirDeclarationMemberIndexBuilder::new(snapshot.module()).freeze(),
            source_components,
            invalidation_epoch,
        )
    }

    #[cfg(test)]
    pub(crate) fn from_validated_test(
        snapshot: HirSnapshotId,
        key: HirModuleKey,
        invalidation_epoch: NonZeroU64,
    ) -> Self {
        Self::from_validated_parts(
            snapshot,
            key,
            HirModuleStatus::Clean,
            Arc::new(SlotSnapshot::empty(snapshot.module(), snapshot.revision())),
            invalidation_epoch,
        )
        .expect("test module parts are exact")
    }
}

fn attach_dialogue_line_candidates(
    module: &mut HirModule,
    syntax_diagnostic_count: usize,
) -> Result<(), HirLowerFailure> {
    let (candidates, line_diagnostics) =
        crate::line_identity::module_candidates::build_module_candidates(module)?;
    module.dialogue_line_candidates = candidates;
    if line_diagnostics.is_empty() {
        return Ok(());
    }
    let mut diagnostics = Vec::from(module.diagnostics.as_ref());
    diagnostics.extend(
        line_diagnostics
            .iter()
            .cloned()
            .map(HirDiagnostic::LineIdentity),
    );
    diagnostics[syntax_diagnostic_count..].sort_by(HirDiagnostic::compare_for_publication);
    module.diagnostics = Arc::from(diagnostics);
    module.status = HirModuleStatus::Recovered;
    Ok(())
}

struct ResolvedThreadBody<'a> {
    body: &'a HirThreadBody,
    owner_metadata: &'a HirSlotMetadata,
}

const fn source_owner_status(metadata: &HirSlotMetadata) -> HirSourceOwnerStatus {
    if metadata.is_poisoned() {
        HirSourceOwnerStatus::Poisoned
    } else {
        HirSourceOwnerStatus::Clean
    }
}

fn scope_has_synthetic_origin(scope: &HirScope, metadata: &HirSlotMetadata) -> bool {
    let HirOrigin::Synthetic(key) = metadata.origin() else {
        return false;
    };
    match (scope.kind(), scope.owner(), key.role(), key.owner()) {
        (
            HirScopeKind::ContractRequires,
            HirScopeOwner::Item(expected),
            SyntheticRole::ContractRequiresScope,
            SyntheticOwner::Item(actual),
        )
        | (
            HirScopeKind::ContractEnsures,
            HirScopeOwner::Item(expected),
            SyntheticRole::ContractEnsuresScope,
            SyntheticOwner::Item(actual),
        ) => *expected == actual,
        _ => false,
    }
}

fn local_has_synthetic_origin(local: &HirLocal, metadata: &HirSlotMetadata) -> bool {
    matches!(
        metadata.origin(),
        HirOrigin::Synthetic(key)
            if key.role() == SyntheticRole::PostconditionResult
                && key.owner() == SyntheticOwner::Scope(local.scope())
    )
}

fn published_thread_body_for_scope<'a>(
    slots: &SlotSnapshot,
    arenas: &'a HirModuleArenas,
    scope_id: ScopeId,
    scope: &HirScope,
) -> Option<(HirThreadBodyOwner, &'a HirThreadBody)> {
    match (scope.kind(), scope.owner()) {
        (HirScopeKind::Flow, HirScopeOwner::Item(owner)) => {
            let item = arenas.items.resolve(slots, *owner).ok()?;
            let HirItemKind::Flow(flow) = item.kind() else {
                return None;
            };
            (flow.body().scope() == scope_id)
                .then_some((HirThreadBodyOwner::Flow(*owner), flow.body()))
        }
        (HirScopeKind::Block, HirScopeOwner::Expr(owner)) => {
            let expression = arenas.expressions.resolve(slots, *owner).ok()?;
            if let HirExprKind::Thread(thread) = expression.kind()
                && thread.body().scope() == scope_id
            {
                return Some((HirThreadBodyOwner::ThreadExpression(*owner), thread.body()));
            }
            expression
                .kind()
                .thread_body_for_scope(scope_id)
                .map(|body| (HirThreadBodyOwner::NestedScope(scope_id), body))
        }
        (HirScopeKind::Block, HirScopeOwner::Stmt(owner)) => arenas
            .statements
            .resolve(slots, *owner)
            .ok()?
            .kind()
            .thread_body_for_scope(scope_id)
            .map(|body| (HirThreadBodyOwner::NestedScope(scope_id), body)),
        _ => None,
    }
}

fn validate_arena_poison<T, I: crate::identity::HirTypedId>(
    arena: &ArenaSnapshot<T, I>,
    slots: &SlotSnapshot,
    is_poisoned: impl Fn(&T) -> bool,
) -> bool {
    let Ok(entries) = arena.try_iter_prepared(slots) else {
        return false;
    };
    entries.into_iter().all(|(id, value)| {
        slots
            .resolve_prepared(id)
            .is_ok_and(|metadata| metadata.is_poisoned() == is_poisoned(value))
    })
}

fn validate_diagnostics(
    parsed: &ParsedSource,
    source_identity: &SourceDocumentIdentity,
    slots: &SlotSnapshot,
    arenas: &HirModuleArenas,
    source_components: &HirSourceIndex,
    diagnostics: &[HirDiagnostic],
) -> Result<(), HirLowerFailure> {
    validate_diagnostic_limit(diagnostics.len())?;
    let syntax_count = parsed.diagnostics().len();
    if diagnostics.len() < syntax_count {
        return Err(HirInvariantFailure::InvalidModuleDiagnostics.into());
    }
    let (syntax, recovery) = diagnostics.split_at(syntax_count);
    if !syntax
        .iter()
        .zip(parsed.diagnostics())
        .all(|(retained, parsed)| retained.syntax() == Some(parsed))
        || recovery
            .iter()
            .any(|diagnostic| diagnostic.syntax().is_some())
        || recovery
            .windows(2)
            .any(|pair| pair[0].compare_for_publication(&pair[1]).is_gt() || pair[0] == pair[1])
    {
        return Err(HirInvariantFailure::InvalidModuleDiagnostics.into());
    }

    let mut recovered = BTreeSet::new();
    for diagnostic in diagnostics {
        if diagnostic.source_site().source_identity() != source_identity {
            return Err(HirInvariantFailure::InvalidModuleDiagnostics.into());
        }
        if let HirDiagnostic::Syntax(syntax) = diagnostic
            && syntax
                .related()
                .is_some_and(|related| related.source() != source_identity)
        {
            return Err(HirInvariantFailure::InvalidModuleDiagnostics.into());
        }
        if let HirDiagnostic::Recovery(recovery) = diagnostic {
            let owner = recovery.owner();
            let Some(metadata) = resolve_prepared_recovery_owner(slots, owner) else {
                return Err(HirInvariantFailure::InvalidModuleDiagnostics.into());
            };
            if owner.module() != slots.snapshot_id().module()
                || !metadata.is_poisoned()
                || !recovered.insert(owner)
                || !recovery_primary_matches_payload(slots, arenas, recovery, metadata)
                || !recovery_primary_matches(
                    source_identity,
                    slots,
                    arenas,
                    source_components,
                    recovery,
                    metadata,
                )
            {
                return Err(HirInvariantFailure::InvalidModuleDiagnostics.into());
            }
        }
    }

    if recovered != recovery_diagnostic_obligations(slots, arenas) {
        return Err(HirInvariantFailure::InvalidModuleDiagnostics.into());
    }
    Ok(())
}

fn recovery_primary_matches_payload(
    slots: &SlotSnapshot,
    arenas: &HirModuleArenas,
    recovery: &HirRecoveryDiagnostic,
    metadata: &HirSlotMetadata,
) -> bool {
    if matches!(metadata.origin(), HirOrigin::Synthetic(_)) {
        return true;
    }
    let SyntheticOwner::Expr(owner) = recovery.owner() else {
        return true;
    };
    let Ok(payload) = arenas.expressions.resolve_prepared(slots, owner) else {
        return false;
    };
    let expected = match (payload.kind(), payload.state()) {
        (HirExprKind::Select(select), _)
            if matches!(select.member(), HirSelectedMember::Missing) =>
        {
            Some(HirExprSourceRole::SelectedMember)
        }
        (_, HirPoisonState::Poisoned(HirRecoveryIssue::MissingOperand { role })) => Some(*role),
        _ => None,
    };
    expected.is_none_or(|role| {
        recovery.primary_role() == HirRecoveryPrimary::query(HirSourceQuery::Expr { owner, role })
    })
}

fn recovery_diagnostic_obligations(
    slots: &SlotSnapshot,
    arenas: &HirModuleArenas,
) -> BTreeSet<SyntheticOwner> {
    let poisoned = slots.poisoned_live_owners().collect::<BTreeSet<_>>();
    let mut obligations = poisoned.clone();
    for child in poisoned {
        if let SyntheticOwner::Expr(owner) = child
            && arenas
                .expressions
                .resolve_prepared(slots, owner)
                .is_ok_and(|payload| {
                    matches!(
                        (payload.kind(), payload.state()),
                        (
                            HirExprKind::Select(select),
                            HirPoisonState::Poisoned(HirRecoveryIssue::InvalidExpression(
                                HirExpressionRecoveryIssue::RecoveredChild {
                                    role: HirExprSourceRole::Target,
                                },
                            )),
                        ) if matches!(select.member(), HirSelectedMember::Name(_))
                    )
                })
        {
            obligations.remove(&child);
        }
        let Some(metadata) = resolve_prepared_recovery_owner(slots, child) else {
            continue;
        };
        if let HirOrigin::Synthetic(key) = metadata.origin()
            && let Some(parent) = recovery_child_covered_parent(slots, arenas, *key)
        {
            obligations.remove(&parent);
        }
    }
    obligations
}

fn recovery_child_covered_parent(
    slots: &SlotSnapshot,
    arenas: &HirModuleArenas,
    key: crate::identity::SyntheticKey,
) -> Option<SyntheticOwner> {
    let SyntheticOwner::Expr(parent) = key.owner() else {
        if let SyntheticOwner::Scope(scope) = key.owner()
            && key.role() == SyntheticRole::MissingRequiredTail
            && let Ok(scope_payload) = arenas.scopes.resolve_prepared(slots, scope)
            && let HirScopeOwner::Expr(parent) = scope_payload.owner()
            && let Ok(Some(tail)) = slots.resolve_prepared_synthetic::<ExprId>(key)
            && arenas
                .expressions
                .resolve_prepared(slots, *parent)
                .is_ok_and(|payload| {
                    matches!(
                        (payload.kind(), payload.state()),
                        (
                            HirExprKind::Match(expression),
                            HirPoisonState::Poisoned(HirRecoveryIssue::MissingRequiredTail),
                        ) if expression
                            .arms()
                            .iter()
                            .any(|arm| arm.scope() == scope && arm.value() == tail)
                    )
                })
        {
            return Some(SyntheticOwner::Expr(*parent));
        }
        return (!matches!(key.owner(), SyntheticOwner::Stmt(_))
            && matches!(
                key.role(),
                SyntheticRole::RecoveryOperand | SyntheticRole::MissingRequiredTail
            ))
        .then_some(key.owner());
    };
    arenas
        .expressions
        .resolve_prepared(slots, parent)
        .is_ok_and(|payload| {
            matches!(
                (key.role(), payload.state()),
                (
                    SyntheticRole::RecoveryOperand,
                    HirPoisonState::Poisoned(HirRecoveryIssue::MissingOperand { .. }),
                ) | (
                    SyntheticRole::MissingRequiredTail,
                    HirPoisonState::Poisoned(HirRecoveryIssue::MissingRequiredTail),
                )
            )
        })
        .then_some(SyntheticOwner::Expr(parent))
}

fn recovery_primary_matches(
    source_identity: &SourceDocumentIdentity,
    slots: &SlotSnapshot,
    arenas: &HirModuleArenas,
    source_components: &HirSourceIndex,
    recovery: &HirRecoveryDiagnostic,
    metadata: &HirSlotMetadata,
) -> bool {
    let owner = recovery.owner();
    let primary = recovery.primary_role();
    if primary.owner() != owner {
        return false;
    }

    if matches!(metadata.origin(), HirOrigin::Synthetic(_)) {
        return recovery_primary_is_owner_whole(primary, owner)
            && metadata.source_site() == recovery.primary();
    }

    match primary {
        HirRecoveryPrimary::OwnerWhole(_) => metadata.source_site() == recovery.primary(),
        HirRecoveryPrimary::Query(query) => source_components
            .lookup(source_identity, source_identity, &query, |query| {
                if !recovery_query_applies(slots, arenas, query) {
                    return Err(HirSourceQueryError::role_not_applicable(query));
                }
                let status = HirSourceOwnerStatus::Poisoned;
                if matches!(
                    query,
                    HirSourceQuery::Scope { .. }
                        | HirSourceQuery::Local { .. }
                        | HirSourceQuery::ThreadBody { .. }
                ) {
                    return resolve_prepared_relational_source_role(
                        slots,
                        arenas,
                        source_components,
                        query,
                        status,
                    )
                    .ok_or_else(|| HirSourceQueryError::role_not_applicable(query));
                }
                if query.is_slot_whole() {
                    return Ok(HirResolvedSourceRole::whole(metadata.source_site(), status));
                }
                let requirement = source_components
                    .requirement(query)
                    .ok_or_else(|| HirSourceQueryError::role_not_applicable(query))?;
                Ok(HirResolvedSourceRole::component(requirement, status))
            })
            .is_ok_and(|lookup| {
                lookup.owner_status() == HirSourceOwnerStatus::Poisoned
                    && matches!(
                        lookup.presence(),
                        HirSourcePresence::Present(site) if site == recovery.primary()
                    )
            }),
    }
}

fn recovery_primary_is_owner_whole(primary: HirRecoveryPrimary, owner: SyntheticOwner) -> bool {
    match (primary, owner) {
        (
            HirRecoveryPrimary::Query(HirSourceQuery::Expr {
                owner: actual,
                role: HirExprSourceRole::Whole,
            }),
            SyntheticOwner::Expr(expected),
        ) => actual == expected,
        (
            HirRecoveryPrimary::Query(HirSourceQuery::Pattern {
                owner: actual,
                role: HirPatternSourceRole::Whole,
            }),
            SyntheticOwner::Pattern(expected),
        ) => actual == expected,
        (
            HirRecoveryPrimary::Query(HirSourceQuery::Type {
                owner: actual,
                role: HirTypeSourceRole::Whole,
            }),
            SyntheticOwner::Type(expected),
        ) => actual == expected,
        (
            HirRecoveryPrimary::Query(HirSourceQuery::Stmt {
                owner: actual,
                role: HirStmtSourceRole::Whole,
            }),
            SyntheticOwner::Stmt(expected),
        ) => actual == expected,
        (
            HirRecoveryPrimary::Query(HirSourceQuery::Scope {
                owner: actual,
                role: HirScopeSourceRole::Whole,
            }),
            SyntheticOwner::Scope(expected),
        ) => actual == expected,
        (
            HirRecoveryPrimary::Query(HirSourceQuery::Local {
                owner: actual,
                role: HirLocalSourceRole::Whole,
            }),
            SyntheticOwner::Local(expected),
        ) => actual == expected,
        (HirRecoveryPrimary::OwnerWhole(actual), expected) => actual == expected,
        _ => false,
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "one exhaustive predicate keeps every source-query variant aligned with its typed owner validation"
)]
fn recovery_query_applies(
    slots: &SlotSnapshot,
    arenas: &HirModuleArenas,
    query: &HirSourceQuery,
) -> bool {
    match query {
        HirSourceQuery::Item { owner, role } => arenas
            .items
            .resolve_prepared(slots, *owner)
            .is_ok_and(|payload| match (payload.kind(), role) {
                (kind, HirItemSourceRole::Declaration(declaration_role)) => kind
                    .validate_declaration_source_role(*owner, *declaration_role)
                    .is_ok(),
                (kind, HirItemSourceRole::Callable(callable_role)) => kind
                    .validate_callable_source_role(*owner, *callable_role)
                    .is_ok(),
                (HirItemKind::Use(declaration), HirItemSourceRole::Use(use_role)) => declaration
                    .validate_use_source_role(*owner, *use_role)
                    .is_ok(),
                (
                    HirItemKind::Test(_) | HirItemKind::Bench(_),
                    HirItemSourceRole::TestBench(HirTestBenchSourceRole::Whole),
                ) => true,
                (HirItemKind::Flow(flow), HirItemSourceRole::Flow(flow_role)) => {
                    flow.validate_source_role(*owner, *flow_role).is_ok()
                }
                (HirItemKind::Style(style), HirItemSourceRole::Style(style_role)) => {
                    style.validate_source_role(*owner, style_role).is_ok()
                }
                _ => false,
            }),
        HirSourceQuery::Expr { owner, role } => arenas
            .expressions
            .resolve_prepared(slots, *owner)
            .is_ok_and(|payload| {
                let target_call_argument_count =
                    if let HirExprKind::DialogueContentApplication(application) = payload.kind() {
                        Some(
                            arenas
                                .expressions
                                .resolve_prepared(slots, application.target())
                                .ok()
                                .map_or(0, |target| match target.kind() {
                                    HirExprKind::Call(call) => call.arguments().len(),
                                    _ => 0,
                                }),
                        )
                    } else {
                        None
                    };
                payload
                    .kind()
                    .validate_source_role_with_context(*owner, *role, target_call_argument_count)
                    .is_ok()
            }),
        HirSourceQuery::Pattern { owner, role } => arenas
            .patterns
            .resolve_prepared(slots, *owner)
            .is_ok_and(|payload| payload.kind().validate_source_role(*owner, *role).is_ok()),
        HirSourceQuery::Type { owner, role } => arenas
            .types
            .resolve_prepared(slots, *owner)
            .is_ok_and(|payload| payload.kind().validate_source_role(*owner, *role).is_ok()),
        HirSourceQuery::Stmt { owner, role } => arenas
            .statements
            .resolve_prepared(slots, *owner)
            .is_ok_and(|payload| payload.kind().validate_source_role(*owner, *role).is_ok()),
        HirSourceQuery::Scope { owner, role } => {
            let Ok(metadata) = slots.resolve_prepared(*owner) else {
                return false;
            };
            let Ok(scope) = arenas.scopes.resolve_prepared(slots, *owner) else {
                return false;
            };
            match role {
                HirScopeSourceRole::Whole => true,
                HirScopeSourceRole::SyntheticOrigin => scope_has_synthetic_origin(scope, metadata),
                HirScopeSourceRole::OpenDelimiter | HirScopeSourceRole::CloseDelimiter => {
                    prepared_thread_body_for_scope(
                        slots,
                        &arenas.items,
                        &arenas.expressions,
                        &arenas.statements,
                        *owner,
                        scope,
                    )
                    .is_some()
                }
            }
        }
        HirSourceQuery::Local { owner, role } => {
            let Ok(metadata) = slots.resolve_prepared(*owner) else {
                return false;
            };
            let Ok(local) = arenas.locals.resolve_prepared(slots, *owner) else {
                return false;
            };
            match role {
                HirLocalSourceRole::Whole => true,
                HirLocalSourceRole::Name | HirLocalSourceRole::Type => {
                    local.kind() != HirLocalKind::PostconditionResult
                }
                HirLocalSourceRole::Pattern => {
                    local.kind() != HirLocalKind::PostconditionResult && local.pattern().is_some()
                }
                HirLocalSourceRole::SyntheticOrigin => {
                    local.kind() == HirLocalKind::PostconditionResult
                        && local_has_synthetic_origin(local, metadata)
                }
            }
        }
        HirSourceQuery::ThreadBody { owner, role } => {
            let Some(body) = prepared_thread_body(slots, arenas, *owner) else {
                return false;
            };
            match role {
                HirThreadBodySourceRole::Whole
                | HirThreadBodySourceRole::OpenDelimiter
                | HirThreadBodySourceRole::CloseDelimiter => true,
                HirThreadBodySourceRole::Item { ordinal, .. } => usize::try_from(*ordinal)
                    .ok()
                    .is_some_and(|ordinal| ordinal < body.items().len()),
            }
        }
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "one exhaustive relational projection keeps scope, local, and thread-body source roles under one validated owner lookup"
)]
fn resolve_prepared_relational_source_role<'a>(
    slots: &'a SlotSnapshot,
    arenas: &'a HirModuleArenas,
    source_components: &'a HirSourceIndex,
    query: &HirSourceQuery,
    status: HirSourceOwnerStatus,
) -> Option<HirResolvedSourceRole<'a>> {
    match query {
        HirSourceQuery::Scope { owner, role } => {
            let metadata = slots.resolve_prepared(*owner).ok()?;
            let scope = arenas.scopes.resolve_prepared(slots, *owner).ok()?;
            match role {
                HirScopeSourceRole::Whole => {
                    Some(HirResolvedSourceRole::whole(metadata.source_site(), status))
                }
                HirScopeSourceRole::SyntheticOrigin
                    if scope_has_synthetic_origin(scope, metadata) =>
                {
                    Some(HirResolvedSourceRole::related(
                        HirSourcePresence::Present(metadata.source_site()),
                        status,
                    ))
                }
                HirScopeSourceRole::OpenDelimiter | HirScopeSourceRole::CloseDelimiter => {
                    let (body_owner, _) = prepared_thread_body_for_scope(
                        slots,
                        &arenas.items,
                        &arenas.expressions,
                        &arenas.statements,
                        *owner,
                        scope,
                    )?;
                    let body_role = match role {
                        HirScopeSourceRole::OpenDelimiter => HirThreadBodySourceRole::OpenDelimiter,
                        HirScopeSourceRole::CloseDelimiter => {
                            HirThreadBodySourceRole::CloseDelimiter
                        }
                        HirScopeSourceRole::Whole | HirScopeSourceRole::SyntheticOrigin => {
                            unreachable!("scope body relation is delimiter-only")
                        }
                    };
                    source_components
                        .component_presence(&HirSourceQuery::ThreadBody {
                            owner: body_owner,
                            role: body_role,
                        })
                        .map(|presence| HirResolvedSourceRole::related(presence, status))
                }
                HirScopeSourceRole::SyntheticOrigin => None,
            }
        }
        HirSourceQuery::Local { owner, role } => {
            let metadata = slots.resolve_prepared(*owner).ok()?;
            let local = arenas.locals.resolve_prepared(slots, *owner).ok()?;
            match role {
                HirLocalSourceRole::Whole => {
                    Some(HirResolvedSourceRole::whole(metadata.source_site(), status))
                }
                HirLocalSourceRole::SyntheticOrigin
                    if local.kind() == HirLocalKind::PostconditionResult
                        && local_has_synthetic_origin(local, metadata) =>
                {
                    Some(HirResolvedSourceRole::related(
                        HirSourcePresence::Present(metadata.source_site()),
                        status,
                    ))
                }
                HirLocalSourceRole::Name if local.kind() != HirLocalKind::PostconditionResult => {
                    Some(HirResolvedSourceRole::related(
                        HirSourcePresence::Present(metadata.source_site()),
                        status,
                    ))
                }
                HirLocalSourceRole::Pattern
                    if local.kind() != HirLocalKind::PostconditionResult =>
                {
                    let pattern = slots.resolve_prepared(local.pattern()?).ok()?;
                    Some(HirResolvedSourceRole::related(
                        HirSourcePresence::Present(pattern.source_site()),
                        status,
                    ))
                }
                HirLocalSourceRole::Type if local.kind() != HirLocalKind::PostconditionResult => {
                    let presence = match local.annotation() {
                        Some(annotation) => HirSourcePresence::Present(
                            slots.resolve_prepared(annotation).ok()?.source_site(),
                        ),
                        None => HirSourcePresence::AbsentOptional,
                    };
                    Some(HirResolvedSourceRole::related(presence, status))
                }
                HirLocalSourceRole::Name
                | HirLocalSourceRole::Type
                | HirLocalSourceRole::Pattern
                | HirLocalSourceRole::SyntheticOrigin => None,
            }
        }
        HirSourceQuery::ThreadBody { owner, role } => {
            let body = prepared_thread_body(slots, arenas, *owner)?;
            match role {
                HirThreadBodySourceRole::Whole => {
                    let scope = slots.resolve_prepared(body.scope()).ok()?;
                    Some(HirResolvedSourceRole::related(
                        HirSourcePresence::Present(scope.source_site()),
                        status,
                    ))
                }
                HirThreadBodySourceRole::OpenDelimiter
                | HirThreadBodySourceRole::CloseDelimiter => source_components
                    .requirement(query)
                    .map(|requirement| HirResolvedSourceRole::component(requirement, status)),
                HirThreadBodySourceRole::Item { ordinal, part } => {
                    let item = usize::try_from(*ordinal)
                        .ok()
                        .and_then(|ordinal| body.items().get(ordinal))?;
                    match part {
                        HirThreadFlowItemSourcePart::Whole => {
                            source_components.requirement(query).map(|requirement| {
                                HirResolvedSourceRole::component(requirement, status)
                            })
                        }
                        HirThreadFlowItemSourcePart::ChildWhole => {
                            let child = prepared_thread_flow_item_metadata(slots, item)?;
                            Some(HirResolvedSourceRole::related(
                                HirSourcePresence::Present(child.source_site()),
                                status,
                            ))
                        }
                    }
                }
            }
        }
        HirSourceQuery::Item { .. }
        | HirSourceQuery::Expr { .. }
        | HirSourceQuery::Pattern { .. }
        | HirSourceQuery::Type { .. }
        | HirSourceQuery::Stmt { .. } => None,
    }
}

fn prepared_thread_flow_item_metadata<'a>(
    slots: &'a SlotSnapshot,
    item: &crate::expr::HirThreadFlowItem,
) -> Option<&'a HirSlotMetadata> {
    match item {
        crate::expr::HirThreadFlowItem::DialogueApplication(owner) => {
            slots.resolve_prepared(*owner).ok()
        }
        crate::expr::HirThreadFlowItem::Statement(owner)
        | crate::expr::HirThreadFlowItem::Choice(owner)
        | crate::expr::HirThreadFlowItem::If(owner)
        | crate::expr::HirThreadFlowItem::IfLet(owner)
        | crate::expr::HirThreadFlowItem::Match(owner)
        | crate::expr::HirThreadFlowItem::Loop(owner)
        | crate::expr::HirThreadFlowItem::While(owner)
        | crate::expr::HirThreadFlowItem::WhileLet(owner)
        | crate::expr::HirThreadFlowItem::For(owner)
        | crate::expr::HirThreadFlowItem::Select(owner)
        | crate::expr::HirThreadFlowItem::SourceLocale(owner)
        | crate::expr::HirThreadFlowItem::Scope(owner)
        | crate::expr::HirThreadFlowItem::Include(owner)
        | crate::expr::HirThreadFlowItem::AwaitWith(owner)
        | crate::expr::HirThreadFlowItem::Error(owner) => slots.resolve_prepared(*owner).ok(),
    }
}

fn prepared_thread_body<'a>(
    slots: &SlotSnapshot,
    arenas: &'a HirModuleArenas,
    owner: HirThreadBodyOwner,
) -> Option<&'a HirThreadBody> {
    match owner {
        HirThreadBodyOwner::Flow(owner) => {
            let item = arenas.items.resolve_prepared(slots, owner).ok()?;
            let HirItemKind::Flow(flow) = item.kind() else {
                return None;
            };
            Some(flow.body())
        }
        HirThreadBodyOwner::ThreadExpression(owner) => {
            let expression = arenas.expressions.resolve_prepared(slots, owner).ok()?;
            let HirExprKind::Thread(thread) = expression.kind() else {
                return None;
            };
            Some(thread.body())
        }
        HirThreadBodyOwner::NestedScope(scope_id) => {
            let scope = arenas.scopes.resolve_prepared(slots, scope_id).ok()?;
            let (HirThreadBodyOwner::NestedScope(_), body) = prepared_thread_body_for_scope(
                slots,
                &arenas.items,
                &arenas.expressions,
                &arenas.statements,
                scope_id,
                scope,
            )?
            else {
                return None;
            };
            Some(body)
        }
    }
}

pub(crate) fn prepared_thread_body_for_scope<'a>(
    slots: &SlotSnapshot,
    items: &'a ArenaSnapshot<HirItem, ItemId>,
    expressions: &'a ArenaSnapshot<HirExpr, ExprId>,
    statements: &'a ArenaSnapshot<HirStmt, StmtId>,
    scope_id: ScopeId,
    scope: &HirScope,
) -> Option<(HirThreadBodyOwner, &'a HirThreadBody)> {
    match (scope.kind(), scope.owner()) {
        (HirScopeKind::Flow, HirScopeOwner::Item(owner)) => {
            let item = items.resolve_prepared(slots, *owner).ok()?;
            let HirItemKind::Flow(flow) = item.kind() else {
                return None;
            };
            (flow.body().scope() == scope_id)
                .then_some((HirThreadBodyOwner::Flow(*owner), flow.body()))
        }
        (HirScopeKind::Block, HirScopeOwner::Expr(owner)) => {
            let expression = expressions.resolve_prepared(slots, *owner).ok()?;
            if let HirExprKind::Thread(thread) = expression.kind()
                && thread.body().scope() == scope_id
            {
                return Some((HirThreadBodyOwner::ThreadExpression(*owner), thread.body()));
            }
            expression
                .kind()
                .thread_body_for_scope(scope_id)
                .map(|body| (HirThreadBodyOwner::NestedScope(scope_id), body))
        }
        (HirScopeKind::Block, HirScopeOwner::Stmt(owner)) => statements
            .resolve_prepared(slots, *owner)
            .ok()?
            .kind()
            .thread_body_for_scope(scope_id)
            .map(|body| (HirThreadBodyOwner::NestedScope(scope_id), body)),
        _ => None,
    }
}

fn resolve_prepared_recovery_owner(
    slots: &SlotSnapshot,
    owner: SyntheticOwner,
) -> Option<&HirSlotMetadata> {
    match owner {
        SyntheticOwner::Item(owner) => slots.resolve_prepared(owner).ok(),
        SyntheticOwner::Scope(owner) => slots.resolve_prepared(owner).ok(),
        SyntheticOwner::Local(owner) => slots.resolve_prepared(owner).ok(),
        SyntheticOwner::Expr(owner) => slots.resolve_prepared(owner).ok(),
        SyntheticOwner::Stmt(owner) => slots.resolve_prepared(owner).ok(),
        SyntheticOwner::Type(owner) => slots.resolve_prepared(owner).ok(),
        SyntheticOwner::Pattern(owner) => slots.resolve_prepared(owner).ok(),
        SyntheticOwner::Capture(owner) => slots.resolve_prepared(owner).ok(),
    }
}

fn validate_diagnostic_limit(observed: usize) -> Result<(), HirLowerFailure> {
    let limit = HirLimit::Diagnostics;
    let maximum = limit.maximum();
    if observed > maximum {
        return Err(HirLimitError::with_maximum(limit, observed, maximum).into());
    }
    Ok(())
}

#[cfg(test)]
#[path = "module/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "module/scope_graph_tests.rs"]
mod scope_graph_tests;

#[cfg(test)]
#[path = "module/resolution_tests.rs"]
mod resolution_tests;
