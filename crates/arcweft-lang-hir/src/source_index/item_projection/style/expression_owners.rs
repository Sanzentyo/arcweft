//! Ephemeral Style expression-owner inventory used by publication freeze.

use std::collections::BTreeSet;

use crate::arena::ArenaSnapshot;
use crate::identity::{ExprId, ItemId};
use crate::item::{HirItem, HirItemKind, HirStyleBodyItem};
use crate::slot::SlotSnapshot;

/// Collects the exact expression owners retained by final Style records.
///
/// This inventory is not a source map. It narrows source-backed missing
/// expression recovery to values reachable from accepted Style HIR after item
/// payload validation succeeds.
pub(in crate::source_index) fn retained_expression_owners(
    items: &ArenaSnapshot<HirItem, ItemId>,
    slots: &SlotSnapshot,
) -> Option<BTreeSet<ExprId>> {
    let entries = items.try_iter_prepared(slots).ok()?;
    let mut retained = BTreeSet::new();
    for (_, item) in entries {
        let HirItemKind::Style(style) = item.kind() else {
            continue;
        };
        for token in style.tokens() {
            if !retained.insert(token.value()) {
                return None;
            }
        }
        if !collect_body_expression_owners(style.body(), &mut retained) {
            return None;
        }
    }
    Some(retained)
}

fn collect_body_expression_owners(
    body: &[HirStyleBodyItem],
    retained: &mut BTreeSet<ExprId>,
) -> bool {
    body.iter().all(|item| match item {
        HirStyleBodyItem::Recovered(_) => true,
        HirStyleBodyItem::Rule(rule) => rule
            .declarations()
            .iter()
            .all(|declaration| retained.insert(declaration.value())),
        HirStyleBodyItem::Environment(environment) => {
            environment
                .clauses()
                .iter()
                .all(|clause| retained.insert(clause.value()))
                && collect_body_expression_owners(environment.body(), retained)
        }
    })
}
