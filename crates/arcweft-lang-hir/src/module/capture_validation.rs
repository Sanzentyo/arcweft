//! Cross-arena closure-capture publication invariants.

use std::collections::{BTreeMap, BTreeSet};

use crate::expr::HirExprKind;
use crate::identity::{CaptureId, LocalId, ScopeId, SyntheticKey, SyntheticOwner, SyntheticRole};
use crate::scope::{HirScopeKind, HirScopeOwner};
use crate::slot::{HirOrigin, SlotSnapshot};
use crate::source_index::HirSourceSite;

use super::HirModuleArenas;

impl HirModuleArenas {
    pub(super) fn validates_capture_graph(&self, slots: &SlotSnapshot) -> bool {
        let Ok(capture_entries) = self.captures.try_iter_prepared(slots) else {
            return false;
        };
        let captures = capture_entries.collect::<BTreeMap<_, _>>();
        let Ok(expression_entries) = self.expressions.try_iter_prepared(slots) else {
            return false;
        };
        let mut referenced = BTreeSet::<CaptureId>::new();

        for (closure_id, payload) in expression_entries {
            let HirExprKind::Closure(closure) = payload.kind() else {
                continue;
            };
            let Ok(scope) = self.scopes.resolve_prepared(slots, closure.scope()) else {
                return false;
            };
            if scope.kind() != HirScopeKind::Closure
                || scope.owner() != &HirScopeOwner::Expr(closure_id)
            {
                return false;
            }
            let Ok(closure_metadata) = slots.resolve_prepared(closure_id) else {
                return false;
            };
            let HirSourceSite::Span(closure_source) = closure_metadata.source_site() else {
                return false;
            };

            let mut locals = BTreeSet::<LocalId>::new();
            let mut previous_order = None;
            for (ordinal, capture_id) in closure.captures().iter().copied().enumerate() {
                let Some(capture) = captures.get(&capture_id).copied() else {
                    return false;
                };
                let Ok(local) = self.locals.resolve_prepared(slots, capture.local()) else {
                    return false;
                };
                let Ok(metadata) = slots.resolve_prepared(capture_id) else {
                    return false;
                };
                let Ok(ordinal) = u32::try_from(ordinal) else {
                    return false;
                };
                let Ok(expected_key) = SyntheticKey::try_new(
                    SyntheticOwner::Expr(closure_id),
                    SyntheticRole::ClosureCapture,
                    ordinal,
                ) else {
                    return false;
                };
                let first_use = capture.first_use();
                let range = first_use.range();
                let order = (range.start(), capture.local());
                let HirSourceSite::Insertion(slot_source) = metadata.source_site() else {
                    return false;
                };
                if capture.closure() != closure_id
                    || local.is_poisoned()
                    || self.scope_descends_from(local.scope(), closure.scope(), slots)
                    || !self.scope_descends_from(closure.scope(), local.scope(), slots)
                    || !locals.insert(capture.local())
                    || !referenced.insert(capture_id)
                    || metadata.origin() != &HirOrigin::Synthetic(expected_key)
                    || !matches!(
                        slots.resolve_prepared_synthetic::<CaptureId>(expected_key),
                        Ok(Some(resolved)) if resolved == capture_id
                    )
                    || slot_source.source_identity() != first_use.source()
                    || slot_source.offset() != range.start()
                    || first_use.source() != closure_source.source()
                    || range.start() >= range.end()
                    || range.start() < closure_source.range().start()
                    || range.end() > closure_source.range().end()
                    || previous_order.is_some_and(|previous| previous >= order)
                {
                    return false;
                }
                previous_order = Some(order);
            }
        }

        referenced.len() == captures.len()
    }

    fn scope_descends_from(&self, scope: ScopeId, ancestor: ScopeId, slots: &SlotSnapshot) -> bool {
        let mut current = Some(scope);
        let mut visited = BTreeSet::new();
        while let Some(scope) = current {
            if !visited.insert(scope) {
                return true;
            }
            if scope == ancestor {
                return true;
            }
            let Ok(payload) = self.scopes.resolve_prepared(slots, scope) else {
                return true;
            };
            current = payload.parent();
        }
        false
    }
}
