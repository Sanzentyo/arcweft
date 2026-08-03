//! Test and Bench payload re-derivation for final item publication.

use std::collections::BTreeSet;

use arcweft_lang_syntax::attachment::{
    AttachedBenchDeclaration, AttachedPlanBody, AttachedPlanId, AttachedTestDeclaration,
    AttachedTestKind,
};
use arcweft_lang_syntax::incremental::ParsedSource;

use crate::identity::ItemId;
use crate::item::{
    HirBenchItem, HirItem, HirItemIssue, HirItemKind, HirItemPoisonState, HirTestItem, HirTestKind,
    HirTestKindIssue,
};
use crate::leaf::{HirIdRefIssue, HirIdRefRecovery, HirIdRefShape, HirIdRefValue};
use crate::slot::SlotSnapshot;
use crate::source_index::block_projection::{
    AttachedStatementBlock, BlockValidationArenas, ItemStatementBlockRetained,
    item_statement_block_matches,
};

use super::{
    ItemValidationArenas, expression_tree_is_unallocated, item_prefix_matches, item_state,
    prefix_issue,
};

pub(super) fn test_payload_matches(
    owner: ItemId,
    attached: &AttachedTestDeclaration,
    item: &HirItem,
    parsed: &ParsedSource,
    slots: &SlotSnapshot,
    arenas: &ItemValidationArenas<'_>,
) -> bool {
    let HirItemKind::Test(retained) = item.kind() else {
        return false;
    };
    let Some(statement_recovery) = plan_body_matches(
        ItemStatementBlockRetained {
            owner,
            parent_scope: item.scope(),
            scope: retained.scope(),
            statements: retained.body(),
        },
        attached.body(),
        parsed,
        slots,
        arenas,
    ) else {
        return false;
    };
    item_prefix_matches(item, attached.prefix(), slots)
        && plan_id_matches(retained.id(), attached.id())
        && plan_id_is_unallocated(attached.id(), slots)
        && test_kind_matches(retained.kind(), attached.kind())
        && item.members().is_empty()
        && item.state() == &test_item_state(attached, retained, item, slots, statement_recovery)
}

pub(super) fn bench_payload_matches(
    owner: ItemId,
    attached: &AttachedBenchDeclaration,
    item: &HirItem,
    parsed: &ParsedSource,
    slots: &SlotSnapshot,
    arenas: &ItemValidationArenas<'_>,
) -> bool {
    let HirItemKind::Bench(retained) = item.kind() else {
        return false;
    };
    let Some(statement_recovery) = plan_body_matches(
        ItemStatementBlockRetained {
            owner,
            parent_scope: item.scope(),
            scope: retained.scope(),
            statements: retained.body(),
        },
        attached.body(),
        parsed,
        slots,
        arenas,
    ) else {
        return false;
    };
    item_prefix_matches(item, attached.prefix(), slots)
        && plan_id_matches(retained.id(), attached.id())
        && plan_id_is_unallocated(attached.id(), slots)
        && item.members().is_empty()
        && item.state() == &bench_item_state(attached, retained, item, slots, statement_recovery)
}

fn plan_body_matches(
    retained: ItemStatementBlockRetained<'_>,
    attached: &AttachedPlanBody,
    parsed: &ParsedSource,
    slots: &SlotSnapshot,
    arenas: &ItemValidationArenas<'_>,
) -> Option<bool> {
    item_statement_block_matches(
        parsed,
        slots,
        &BlockValidationArenas {
            expressions: arenas.expressions,
            statements: arenas.statements,
            scopes: arenas.scopes,
            locals: arenas.locals,
            patterns: arenas.patterns,
        },
        retained,
        AttachedStatementBlock {
            id: attached.syntax().id(),
            source: attached.syntax().source_span(),
            statements: attached.statements(),
        },
    )
}

fn plan_id_matches(retained: &HirIdRefValue, attached: &AttachedPlanId) -> bool {
    let projected = match attached {
        AttachedPlanId::Authored(_) => attached
            .value()
            .and_then(|value| crate::final_lowering::id_ref_projection::id_ref(value).ok()),
        AttachedPlanId::Missing(_) => Some(HirIdRefValue::Recovered(HirIdRefRecovery::new(
            HirIdRefShape::Missing,
            HirIdRefIssue::Missing,
        ))),
    };
    projected.as_ref() == Some(retained)
}

fn plan_id_is_unallocated(attached: &AttachedPlanId, slots: &SlotSnapshot) -> bool {
    match attached {
        AttachedPlanId::Authored(expression) => {
            expression_tree_is_unallocated(expression, slots, &mut BTreeSet::new())
        }
        AttachedPlanId::Missing(missing) => slots
            .prepared_source_owner::<crate::identity::ExprId>(missing.id())
            .is_none(),
    }
}

fn test_kind_matches(retained: &HirTestKind, attached: &AttachedTestKind) -> bool {
    match (retained, attached) {
        (HirTestKind::Scenario, AttachedTestKind::Scenario(_))
        | (HirTestKind::Visual, AttachedTestKind::Visual(_))
        | (HirTestKind::Audio, AttachedTestKind::Audio(_))
        | (HirTestKind::Fixture, AttachedTestKind::Fixture(_))
        | (HirTestKind::Recovered(HirTestKindIssue::Missing), AttachedTestKind::Missing(_)) => true,
        (HirTestKind::Custom(retained), AttachedTestKind::Custom { value, .. }) => {
            retained.as_str() == value.as_str()
        }
        _ => false,
    }
}

fn test_item_state(
    attached: &AttachedTestDeclaration,
    retained: &HirTestItem,
    item: &HirItem,
    slots: &SlotSnapshot,
    statement_recovery: bool,
) -> HirItemPoisonState {
    item_state(
        prefix_issue(attached.prefix(), item.prefix(), slots)
            .or_else(|| plan_id_issue(attached.id(), retained.id()))
            .or_else(|| {
                matches!(attached.kind(), AttachedTestKind::Missing(_))
                    .then_some(HirItemIssue::MissingKind)
            })
            .or_else(|| plan_body_issue(attached.body(), statement_recovery))
            .or_else(|| {
                (!attached.trailing_recoveries().is_empty())
                    .then_some(HirItemIssue::MalformedHeader)
            }),
    )
}

fn bench_item_state(
    attached: &AttachedBenchDeclaration,
    retained: &HirBenchItem,
    item: &HirItem,
    slots: &SlotSnapshot,
    statement_recovery: bool,
) -> HirItemPoisonState {
    item_state(
        prefix_issue(attached.prefix(), item.prefix(), slots)
            .or_else(|| plan_id_issue(attached.id(), retained.id()))
            .or_else(|| plan_body_issue(attached.body(), statement_recovery))
            .or_else(|| {
                (!attached.trailing_recoveries().is_empty())
                    .then_some(HirItemIssue::MalformedHeader)
            }),
    )
}

fn plan_id_issue(attached: &AttachedPlanId, retained: &HirIdRefValue) -> Option<HirItemIssue> {
    match attached {
        AttachedPlanId::Missing(_) => Some(HirItemIssue::MissingId),
        AttachedPlanId::Authored(_) if retained.recovery_issue().is_some() => {
            Some(HirItemIssue::Recovery)
        }
        AttachedPlanId::Authored(_) => None,
    }
}

fn plan_body_issue(attached: &AttachedPlanBody, statement_recovery: bool) -> Option<HirItemIssue> {
    match attached {
        AttachedPlanBody::Missing(_) => Some(HirItemIssue::MissingBody),
        AttachedPlanBody::Braced { .. } if attached.has_recovery() || statement_recovery => {
            Some(HirItemIssue::Recovery)
        }
        AttachedPlanBody::Braced { .. } => None,
    }
}
