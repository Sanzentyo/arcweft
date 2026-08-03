//! Prepared lexical-local resolution shared by freeze validation consumers.

use std::collections::{BTreeMap, BTreeSet};

use crate::arena::ArenaSnapshot;
use crate::identity::{LocalId, ScopeId, StmtId};
use crate::leaf::HirName;
use crate::scope::{HirLocal, HirLocalKind, HirScope, LocalLookup};
use crate::slot::{HirSlotMetadata, SlotSnapshot};
use crate::source_index::HirSourceSite;
use crate::stmt::HirStmt;

#[derive(Clone, Copy)]
enum ResolutionState {
    Prepared,
    Published,
}

/// One immutable view of local visibility in a frozen module.
///
/// Sequential `let` binding points are derived once from final statement
/// ownership. No source-index consumer may reconstruct visibility from a
/// local's authored name span.
pub(crate) struct HirLocalResolver<'arena> {
    slots: &'arena SlotSnapshot,
    scopes: &'arena ArenaSnapshot<HirScope, ScopeId>,
    locals: &'arena ArenaSnapshot<HirLocal, LocalId>,
    state: ResolutionState,
    visibility_starts: BTreeMap<LocalId, usize>,
}

impl<'arena> HirLocalResolver<'arena> {
    pub(crate) fn prepared(
        slots: &'arena SlotSnapshot,
        scopes: &'arena ArenaSnapshot<HirScope, ScopeId>,
        locals: &'arena ArenaSnapshot<HirLocal, LocalId>,
        statements: &'arena ArenaSnapshot<HirStmt, StmtId>,
    ) -> Option<Self> {
        Self::try_new(slots, scopes, locals, statements, ResolutionState::Prepared)
    }

    pub(crate) fn published(
        slots: &'arena SlotSnapshot,
        scopes: &'arena ArenaSnapshot<HirScope, ScopeId>,
        locals: &'arena ArenaSnapshot<HirLocal, LocalId>,
        statements: &'arena ArenaSnapshot<HirStmt, StmtId>,
    ) -> Option<Self> {
        Self::try_new(
            slots,
            scopes,
            locals,
            statements,
            ResolutionState::Published,
        )
    }

    fn try_new(
        slots: &'arena SlotSnapshot,
        scopes: &'arena ArenaSnapshot<HirScope, ScopeId>,
        locals: &'arena ArenaSnapshot<HirLocal, LocalId>,
        statements: &'arena ArenaSnapshot<HirStmt, StmtId>,
        state: ResolutionState,
    ) -> Option<Self> {
        let mut visibility_starts = BTreeMap::new();
        let statement_entries = match state {
            ResolutionState::Prepared => statements.try_iter_prepared(slots),
            ResolutionState::Published => statements.try_iter(slots),
        }
        .ok()?;
        for (statement, payload) in statement_entries {
            let statement_end = source_site_end(resolve_metadata(slots, state, statement)?);
            for local in payload.kind().post_statement_locals() {
                let local_payload = resolve_local(locals, slots, state, *local)?;
                if local_payload.kind() != HirLocalKind::LetBinding
                    || local_payload.scope() != payload.scope()
                    || visibility_starts.insert(*local, statement_end).is_some()
                {
                    return None;
                }
            }
        }

        let local_entries = match state {
            ResolutionState::Prepared => locals.try_iter_prepared(slots),
            ResolutionState::Published => locals.try_iter(slots),
        }
        .ok()?;
        for (local, _) in local_entries {
            if visibility_starts.contains_key(&local) {
                continue;
            }
            // Statement payloads, rather than the broad local kind, are the
            // authority for sequential publication. Branch-head body bindings
            // share `LetBinding` but are visible from their own source-backed
            // position inside the branch scope.
            visibility_starts.insert(
                local,
                source_site_start(resolve_metadata(slots, state, local)?),
            );
        }

        Some(Self {
            slots,
            scopes,
            locals,
            state,
            visibility_starts,
        })
    }

    /// Resolves the highest visible generation in the nearest lexical scope.
    ///
    /// Poisoned locals shadow earlier generations but are never returned as a
    /// successful semantic binding. Their exact same-generation inventory is
    /// retained for tooling.
    pub(crate) fn lookup(
        &self,
        use_scope: ScopeId,
        name: &HirName,
        before_start: usize,
    ) -> Option<LocalLookup> {
        let mut current = Some(use_scope);
        let mut visited = BTreeSet::new();
        while let Some(scope) = current {
            if !visited.insert(scope) {
                return None;
            }
            let scope_payload = resolve_scope(self.scopes, self.slots, self.state, scope)?;
            let mut generation = None;
            let mut found = None;
            let mut poisoned = Vec::new();
            for local in scope_payload.locals().iter().copied() {
                let payload = resolve_local(self.locals, self.slots, self.state, local)?;
                let start = self.visibility_starts.get(&local).copied()?;
                if payload.scope() != scope || payload.name() != name || start >= before_start {
                    continue;
                }
                match generation {
                    Some(current) if payload.generation() < current => continue,
                    Some(current) if payload.generation() == current => {}
                    Some(_) | None => {
                        generation = Some(payload.generation());
                        found = None;
                        poisoned.clear();
                    }
                }
                if payload.is_poisoned() {
                    poisoned.push(local);
                } else if found.replace(local).is_some() {
                    return None;
                }
            }
            if let Some(local) = found {
                return Some(LocalLookup::Found(local));
            }
            if !poisoned.is_empty() {
                return Some(LocalLookup::AmbiguousPoisoned(poisoned.into_boxed_slice()));
            }
            current = scope_payload.parent();
        }
        Some(LocalLookup::NotFound)
    }
}

fn resolve_metadata<I: crate::identity::HirTypedId>(
    slots: &SlotSnapshot,
    state: ResolutionState,
    id: I,
) -> Option<&HirSlotMetadata> {
    match state {
        ResolutionState::Prepared => slots.resolve_prepared(id),
        ResolutionState::Published => slots.resolve(id),
    }
    .ok()
}

fn resolve_scope<'arena>(
    scopes: &'arena ArenaSnapshot<HirScope, ScopeId>,
    slots: &SlotSnapshot,
    state: ResolutionState,
    id: ScopeId,
) -> Option<&'arena HirScope> {
    match state {
        ResolutionState::Prepared => scopes.resolve_prepared(slots, id),
        ResolutionState::Published => scopes.resolve(slots, id),
    }
    .ok()
}

fn resolve_local<'arena>(
    locals: &'arena ArenaSnapshot<HirLocal, LocalId>,
    slots: &SlotSnapshot,
    state: ResolutionState,
    id: LocalId,
) -> Option<&'arena HirLocal> {
    match state {
        ResolutionState::Prepared => locals.resolve_prepared(slots, id),
        ResolutionState::Published => locals.resolve(slots, id),
    }
    .ok()
}

fn source_site_start(metadata: &HirSlotMetadata) -> usize {
    match metadata.source_site() {
        HirSourceSite::Span(span) => span.range().start(),
        HirSourceSite::Insertion(insertion) => insertion.offset(),
    }
}

fn source_site_end(metadata: &HirSlotMetadata) -> usize {
    match metadata.source_site() {
        HirSourceSite::Span(span) => span.range().end(),
        HirSourceSite::Insertion(insertion) => insertion.offset(),
    }
}
