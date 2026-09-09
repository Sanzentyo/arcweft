//! Exact validation of expression graphs generated from typed dialogue syntax.

use std::collections::BTreeSet;

use arcweft_lang_syntax::expressions::{
    SyntaxDialogueContentProjection, SyntaxDialogueNodeProjection,
};

use crate::arena::ArenaSnapshot;
use crate::dialogue_application::{
    HirDialogueContent, HirDialogueNodeKind, ruby::HirRubyDesugaring,
};
use crate::expr::HirExpr;
use crate::identity::ExprId;
use crate::slot::{HirOrigin, SlotSnapshot};

pub(super) fn dialogue_desugared_expressions(
    slots: &SlotSnapshot,
    expressions: &ArenaSnapshot<HirExpr, ExprId>,
    content: &HirDialogueContent,
    source: &SyntaxDialogueContentProjection,
) -> Option<BTreeSet<ExprId>> {
    let mut admitted = BTreeSet::new();
    let SyntaxDialogueContentProjection::Present(source) = source else {
        return content.nodes().is_empty().then_some(admitted);
    };
    if content.nodes().len() != source.nodes().len() {
        return None;
    }
    let owner = content.id().owner();
    let parent = expressions.resolve_prepared(slots, owner).ok()?;
    let parent_metadata = slots.resolve_prepared(owner).ok()?;
    for (ordinal, (node, expected)) in content.nodes().iter().zip(source.nodes()).enumerate() {
        let SyntaxDialogueNodeProjection::Ruby { base, ruby } = expected else {
            continue;
        };
        let HirDialogueNodeKind::ContentApplication(application) = node.kind() else {
            return None;
        };
        let recipe = HirRubyDesugaring::new(
            owner,
            u32::try_from(ordinal).ok()?,
            parent.scope(),
            base,
            ruby,
        );
        let ids = recipe
            .keys()
            .ok()?
            .into_iter()
            .map(|key| {
                slots
                    .resolve_prepared_synthetic::<ExprId>(key)
                    .ok()
                    .flatten()
            })
            .collect::<Option<Vec<_>>>()?;
        let ids: [ExprId; 3] = ids.try_into().ok()?;
        if ids[2] != *application {
            return None;
        }
        for (id, expected) in ids.into_iter().zip(recipe.payloads(ids).ok()?) {
            let metadata = slots.resolve_prepared(id).ok()?;
            if metadata.source_site() != parent_metadata.source_site()
                || expressions.resolve_prepared(slots, id).ok()? != &expected
                || !admitted.insert(id)
            {
                return None;
            }
        }
    }
    Some(admitted)
}

pub(super) fn desugared_expression_slots_match(
    slots: &SlotSnapshot,
    expected: &BTreeSet<ExprId>,
) -> bool {
    let actual = slots.prepared_live_ids::<ExprId>().try_fold(BTreeSet::new(), |mut actual, id| {
        let metadata = slots.resolve_prepared(id).ok()?;
        if matches!(metadata.origin(), HirOrigin::Synthetic(key) if HirRubyDesugaring::ROLES.contains(&key.role())) { actual.insert(id); }
        Some(actual)
    });
    actual.as_ref() == Some(expected)
}
