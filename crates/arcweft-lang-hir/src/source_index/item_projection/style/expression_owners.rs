//! Ephemeral Style expression-owner inventory used by publication freeze.

use std::collections::BTreeSet;

use crate::arena::ArenaSnapshot;
use crate::identity::{ExprId, ItemId};
use crate::item::{HirItem, HirItemKind};
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
        if !style
            .value_expression_roots()
            .into_iter()
            .all(|owner| retained.insert(owner))
        {
            return None;
        }
    }
    Some(retained)
}
