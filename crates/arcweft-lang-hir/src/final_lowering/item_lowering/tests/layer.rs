use std::fmt::Write as _;

use super::*;

use crate::expr::HirExpr;
use crate::item::{
    HirAccessibilityPolicy, HirCapturePolicy, HirDeclarationMember, HirDeclarationMemberIssue,
    HirHitTestPolicy, HirInputPolicy, HirLayerDeclaration, HirLayerExpressionMember, HirLayerKind,
    HirLayerMemberValue, HirLayerPolicyMember, HirLayerReferenceMember, HirRenderPhase,
};
use crate::leaf::HirIdRefValue;

fn layer(
    module: &HirModule,
    ordinal: usize,
) -> (crate::identity::ItemId, &HirItem, &HirLayerDeclaration) {
    let owner = module.source_ordered_items()[ordinal];
    let item = resolve_item(module, ordinal);
    let HirItemKind::Layer(layer) = item.kind() else {
        panic!("source-ordered item {ordinal} must be a Layer")
    };
    (owner, item, layer)
}

fn member(module: &HirModule, position: usize) -> &HirDeclarationMember {
    let item = resolve_item(module, 0);
    module
        .declaration_members()
        .resolve(item.members()[position])
        .unwrap()
}

fn layer_named<'module>(
    module: &'module HirModule,
    expected: &str,
) -> (
    crate::identity::ItemId,
    &'module HirItem,
    &'module HirLayerDeclaration,
) {
    (0..module.source_ordered_items().len())
        .find_map(|ordinal| {
            let (owner, item, layer) = layer(module, ordinal);
            matches!(
                layer.header().name(),
                HirRetainedName::Resolved(name) if name.as_str() == expected
            )
            .then_some((owner, item, layer))
        })
        .unwrap_or_else(|| panic!("missing Layer declaration `{expected}`"))
}

fn layer_expression(
    module: &HirModule,
    layer: &HirLayerDeclaration,
    select: impl Fn(&HirLayerExpressionMember) -> bool,
) -> ExprId {
    layer
        .members()
        .iter()
        .copied()
        .find_map(|id| {
            let retained = module.declaration_members().resolve(id).unwrap();
            let HirDeclarationMemberKind::LayerExpression(member) = retained.kind() else {
                return None;
            };
            if !select(member) {
                return None;
            }
            match member.payload().value() {
                HirLayerMemberValue::Present(owner)
                | HirLayerMemberValue::Recovered(Some(owner)) => Some(*owner),
                HirLayerMemberValue::Recovered(None) | HirLayerMemberValue::Missing => None,
            }
        })
        .expect("Layer expression member")
}

fn lower_output(
    database: &mut HirDatabase,
    parsed: &ParsedSource,
    key: &HirModuleKey,
) -> crate::database::HirLowerOutput {
    let mut transaction = stage(database, parsed, key);
    transaction
        .lower_attached_source_file_items(&parsed.tree())
        .unwrap();
    transaction.finish(database).unwrap()
}

fn assert_layer_freeze_rejects(
    case: &str,
    source: &str,
    tamper: impl FnOnce(&mut StagedHirModuleTransaction<'_>, crate::identity::ItemId),
) {
    let parsed = parse(
        &format!("arcweft-test://proof/final-hir-layer-{case}"),
        source,
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let mut transaction = stage(&database, &parsed, &key);
    transaction
        .lower_attached_source_file_items(&parsed.tree())
        .unwrap();
    let owner = transaction.source_ordered_items[0];
    tamper(&mut transaction, owner);
    assert!(
        matches!(
            transaction.finish(&mut database),
            Err(HirLowerFailure::Invariant(
                HirInvariantFailure::InvalidSourceIndex
            ))
        ),
        "Layer freeze accepted {case}"
    );
    assert!(database.current(&key).is_none());
}

fn revise_expression_scope(
    transaction: &mut StagedHirModuleTransaction<'_>,
    owner: ExprId,
    scope: ScopeId,
) {
    let (kind, state) = {
        let (slots, arenas) = transaction.storage_mut();
        let original = arenas.expressions().resolve_staged(slots, owner).unwrap();
        (original.kind().clone(), original.state().clone())
    };
    let replacement = HirExpr::try_new(scope, kind, state).unwrap();
    let (slots, arenas) = transaction.storage_mut();
    arenas
        .expressions()
        .revise_finalized(slots, owner, replacement)
        .unwrap();
}

#[test]
fn canonical_layer_freezes_closed_kind_members_and_exact_expression_owners() {
    let parsed = parse(
        "arcweft-test://proof/final-hir-layer-clean",
        concat!(
            "/// Dialogue surface\n",
            "pub layer @layer.dialogue dialogue_ui: dialogue {\n",
            "    parent = @layer.root\n",
            "    phase = dialogue\n",
            "    z = 100\n",
            "    visible = true\n",
            "    transform = Transform.identity()\n",
            "    input = hit_test\n",
            "    hit_test = view_tree\n",
            "    capture = none\n",
            "    accessibility = container\n",
            "    view = @<view.MainDialogue>\n",
            "}\n",
            "layer Background: background {}\n",
        ),
    );
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    let (owner, item, declaration) = layer(&module, 0);

    assert_eq!(item.state(), &HirItemPoisonState::Clean);
    assert_eq!(declaration.kind(), HirLayerKind::Dialogue);
    assert_eq!(declaration.members(), item.members());
    assert_eq!(item.members().len(), 10);
    assert_eq!(
        module
            .declaration_members()
            .arena(owner)
            .unwrap()
            .members()
            .len(),
        10
    );
    for (position, member_id) in item.members().iter().copied().enumerate() {
        assert_eq!(member_id.ordinal(), u32::try_from(position).unwrap());
        assert_eq!(
            member(&module, position).state(),
            HirDeclarationMemberPoisonState::Clean
        );
    }

    let HirDeclarationMemberKind::LayerReference(HirLayerReferenceMember::Parent(parent)) =
        member(&module, 0).kind()
    else {
        panic!("first Layer member must be parent")
    };
    assert!(matches!(
        parent.value(),
        HirLayerMemberValue::Present(HirIdRefValue::Resolved(_))
    ));
    let HirDeclarationMemberKind::LayerPolicy(HirLayerPolicyMember::Phase(phase)) =
        member(&module, 1).kind()
    else {
        panic!("second Layer member must be phase")
    };
    assert_eq!(
        phase.value(),
        &HirLayerMemberValue::Present(HirRenderPhase::Dialogue)
    );

    for position in 2..=4 {
        let payload = match member(&module, position).kind() {
            HirDeclarationMemberKind::LayerExpression(HirLayerExpressionMember::Z(payload))
            | HirDeclarationMemberKind::LayerExpression(HirLayerExpressionMember::Visible(
                payload,
            ))
            | HirDeclarationMemberKind::LayerExpression(HirLayerExpressionMember::Transform(
                payload,
            )) => payload,
            other => panic!("expected Layer expression member, got {other:?}"),
        };
        let HirLayerMemberValue::Present(expression) = payload.value() else {
            panic!("clean Layer expression must retain one expression owner")
        };
        let expression_owner = *expression;
        let expression = module
            .arenas()
            .expressions()
            .resolve(module.slots(), expression_owner)
            .unwrap();
        assert_eq!(expression.scope(), item.scope());
        assert_source_backed_child(&module, expression_owner);
    }

    assert!(matches!(
        member(&module, 5).kind(),
        HirDeclarationMemberKind::LayerPolicy(HirLayerPolicyMember::Input(payload))
            if payload.value() == &HirLayerMemberValue::Present(HirInputPolicy::HitTest)
    ));
    assert!(matches!(
        member(&module, 6).kind(),
        HirDeclarationMemberKind::LayerPolicy(HirLayerPolicyMember::HitTest(payload))
            if payload.value() == &HirLayerMemberValue::Present(HirHitTestPolicy::ViewTree)
    ));
    assert!(matches!(
        member(&module, 7).kind(),
        HirDeclarationMemberKind::LayerPolicy(HirLayerPolicyMember::Capture(payload))
            if payload.value() == &HirLayerMemberValue::Present(HirCapturePolicy::None)
    ));
    assert!(matches!(
        member(&module, 8).kind(),
        HirDeclarationMemberKind::LayerPolicy(HirLayerPolicyMember::Accessibility(payload))
            if payload.value()
                == &HirLayerMemberValue::Present(HirAccessibilityPolicy::Container)
    ));
    assert!(matches!(
        member(&module, 9).kind(),
        HirDeclarationMemberKind::LayerReference(HirLayerReferenceMember::View(payload))
            if matches!(
                payload.value(),
                HirLayerMemberValue::Present(HirIdRefValue::Resolved(_))
            )
    ));

    let (background_owner, background_item, background) = layer(&module, 1);
    assert_eq!(background.kind(), HirLayerKind::Background);
    assert_eq!(
        background.kind().default_phase(),
        Some(HirRenderPhase::Background)
    );
    assert!(background.members().is_empty());
    assert!(background_item.members().is_empty());
    assert!(
        module
            .declaration_members()
            .arena(background_owner)
            .is_none()
    );
    assert_item_slot_whole(&module, &parsed, owner);
    assert_item_slot_whole(&module, &parsed, background_owner);
}

#[test]
fn layer_hir_covers_every_authored_kind_and_closed_policy_value() {
    let kinds = [
        ("background", HirLayerKind::Background),
        ("world_2d", HirLayerKind::World2d),
        ("character", HirLayerKind::Character),
        ("effects", HirLayerKind::Effects),
        ("dialogue", HirLayerKind::Dialogue),
        ("game_view", HirLayerKind::GameView),
        ("html_view", HirLayerKind::HtmlView),
        ("activity", HirLayerKind::Activity),
        ("modal", HirLayerKind::Modal),
        ("overlay", HirLayerKind::Overlay),
        ("debug", HirLayerKind::Debug),
        ("agent", HirLayerKind::Agent),
        ("offscreen", HirLayerKind::Offscreen),
        ("custom", HirLayerKind::Custom),
    ];
    let policies = [
        ("phase", "background", 0_u8),
        ("phase", "world", 1),
        ("phase", "characters", 2),
        ("phase", "effects", 3),
        ("phase", "dialogue", 4),
        ("phase", "game_view", 5),
        ("phase", "html_view", 6),
        ("phase", "modal", 7),
        ("phase", "debug", 8),
        ("phase", "agent_overlay", 9),
        ("input", "ignore", 10),
        ("input", "pass_through", 11),
        ("input", "hit_test", 12),
        ("input", "modal", 13),
        ("input", "capture", 14),
        ("hit_test", "none", 15),
        ("hit_test", "bounds", 16),
        ("hit_test", "view_tree", 17),
        ("hit_test", "object_id_mask", 18),
        ("capture", "none", 19),
        ("capture", "color", 20),
        ("capture", "object_id", 21),
        ("capture", "mask", 22),
        ("capture", "all", 23),
        ("accessibility", "hidden", 24),
        ("accessibility", "exposed", 25),
        ("accessibility", "container", 26),
    ];
    let mut source = String::new();
    for (index, (spelling, _)) in kinds.iter().enumerate() {
        writeln!(source, "layer Kind{index}: {spelling} {{}}").unwrap();
    }
    for (index, (member, value, _)) in policies.iter().enumerate() {
        writeln!(
            source,
            "layer Policy{index}: custom {{ {member} = {value} }}"
        )
        .unwrap();
    }

    let parsed = parse(
        "arcweft-test://proof/final-hir-layer-closed-vocabulary",
        &source,
    );
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);

    for (ordinal, (_, expected)) in kinds.iter().enumerate() {
        let (owner, item, declaration) = layer(&module, ordinal);
        assert_eq!(declaration.kind(), *expected);
        assert_eq!(item.state(), &HirItemPoisonState::Clean);
        assert!(item.members().is_empty());
        assert_item_slot_whole(&module, &parsed, owner);
    }
    for (index, (member_name, value_name, expected)) in policies.iter().enumerate() {
        let ordinal = kinds.len() + index;
        let (owner, item, _) = layer(&module, ordinal);
        assert_eq!(item.state(), &HirItemPoisonState::Clean);
        assert_eq!(item.members().len(), 1);
        let retained = module
            .declaration_members()
            .resolve(item.members()[0])
            .unwrap();
        assert_eq!(
            layer_policy_ordinal(retained.kind()),
            Some(*expected),
            "{member_name} = {value_name}"
        );
        assert_item_slot_whole(&module, &parsed, owner);
    }
}

fn layer_policy_ordinal(kind: &HirDeclarationMemberKind) -> Option<u8> {
    let value = match kind {
        HirDeclarationMemberKind::LayerPolicy(HirLayerPolicyMember::Phase(payload)) => {
            match payload.value() {
                HirLayerMemberValue::Present(HirRenderPhase::Background) => 0,
                HirLayerMemberValue::Present(HirRenderPhase::World) => 1,
                HirLayerMemberValue::Present(HirRenderPhase::Characters) => 2,
                HirLayerMemberValue::Present(HirRenderPhase::Effects) => 3,
                HirLayerMemberValue::Present(HirRenderPhase::Dialogue) => 4,
                HirLayerMemberValue::Present(HirRenderPhase::GameView) => 5,
                HirLayerMemberValue::Present(HirRenderPhase::HtmlView) => 6,
                HirLayerMemberValue::Present(HirRenderPhase::Modal) => 7,
                HirLayerMemberValue::Present(HirRenderPhase::Debug) => 8,
                HirLayerMemberValue::Present(HirRenderPhase::AgentOverlay) => 9,
                HirLayerMemberValue::Recovered(_) | HirLayerMemberValue::Missing => return None,
            }
        }
        HirDeclarationMemberKind::LayerPolicy(HirLayerPolicyMember::Input(payload)) => {
            match payload.value() {
                HirLayerMemberValue::Present(HirInputPolicy::Ignore) => 10,
                HirLayerMemberValue::Present(HirInputPolicy::PassThrough) => 11,
                HirLayerMemberValue::Present(HirInputPolicy::HitTest) => 12,
                HirLayerMemberValue::Present(HirInputPolicy::Modal) => 13,
                HirLayerMemberValue::Present(HirInputPolicy::Capture) => 14,
                HirLayerMemberValue::Recovered(_) | HirLayerMemberValue::Missing => return None,
            }
        }
        HirDeclarationMemberKind::LayerPolicy(HirLayerPolicyMember::HitTest(payload)) => {
            match payload.value() {
                HirLayerMemberValue::Present(HirHitTestPolicy::None) => 15,
                HirLayerMemberValue::Present(HirHitTestPolicy::Bounds) => 16,
                HirLayerMemberValue::Present(HirHitTestPolicy::ViewTree) => 17,
                HirLayerMemberValue::Present(HirHitTestPolicy::ObjectIdMask) => 18,
                HirLayerMemberValue::Recovered(_) | HirLayerMemberValue::Missing => return None,
            }
        }
        HirDeclarationMemberKind::LayerPolicy(HirLayerPolicyMember::Capture(payload)) => {
            match payload.value() {
                HirLayerMemberValue::Present(HirCapturePolicy::None) => 19,
                HirLayerMemberValue::Present(HirCapturePolicy::Color) => 20,
                HirLayerMemberValue::Present(HirCapturePolicy::ObjectId) => 21,
                HirLayerMemberValue::Present(HirCapturePolicy::Mask) => 22,
                HirLayerMemberValue::Present(HirCapturePolicy::All) => 23,
                HirLayerMemberValue::Recovered(_) | HirLayerMemberValue::Missing => return None,
            }
        }
        HirDeclarationMemberKind::LayerPolicy(HirLayerPolicyMember::Accessibility(payload)) => {
            match payload.value() {
                HirLayerMemberValue::Present(HirAccessibilityPolicy::Hidden) => 24,
                HirLayerMemberValue::Present(HirAccessibilityPolicy::Exposed) => 25,
                HirLayerMemberValue::Present(HirAccessibilityPolicy::Container) => 26,
                HirLayerMemberValue::Recovered(_) | HirLayerMemberValue::Missing => return None,
            }
        }
        _ => return None,
    };
    Some(value)
}

#[test]
fn recovered_layer_retains_typed_member_failures_without_fabricating_unknown_members() {
    let parsed = parse(
        "arcweft-test://proof/final-hir-layer-recovery",
        concat!(
            "layer Broken root {\n",
            "    phase = impossible extra\n",
            "    z\n",
            "    z = 2\n",
            "    parent = @view.parent\n",
            "    view = @<activity.game>\n",
            "    unknown = true\n",
            "}\n",
        ),
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    let (owner, item, declaration) = layer(&module, 0);

    assert_eq!(
        declaration.kind(),
        HirLayerKind::Recovered(crate::item::HirLayerKindIssue::Invalid)
    );
    assert_eq!(declaration.kind().default_phase(), None);
    assert_eq!(
        item.state(),
        &HirItemPoisonState::Poisoned(HirItemIssue::MalformedHeader)
    );
    assert_eq!(declaration.members(), item.members());
    assert_eq!(item.members().len(), 5);
    assert_eq!(
        module
            .declaration_members()
            .arena(owner)
            .unwrap()
            .members()
            .len(),
        5,
        "the unknown recovery entry poisons the item but fabricates no valid Layer member"
    );
    assert_eq!(
        member(&module, 0).state(),
        HirDeclarationMemberPoisonState::Poisoned(HirDeclarationMemberIssue::RecoveredChild)
    );
    assert_eq!(
        member(&module, 1).state(),
        HirDeclarationMemberPoisonState::Poisoned(HirDeclarationMemberIssue::MissingAssignment)
    );
    assert_eq!(
        member(&module, 2).state(),
        HirDeclarationMemberPoisonState::Poisoned(HirDeclarationMemberIssue::Duplicate)
    );
    assert_eq!(
        member(&module, 3).state(),
        HirDeclarationMemberPoisonState::Poisoned(HirDeclarationMemberIssue::RecoveredChild)
    );
    assert_eq!(
        member(&module, 4).state(),
        HirDeclarationMemberPoisonState::Poisoned(HirDeclarationMemberIssue::RecoveredChild)
    );
    assert_item_owner_whole_recovery(&module, owner);
}

#[test]
fn recovered_layer_expression_is_lowered_once_and_poisoned_through_the_member() {
    let parsed = parse(
        "arcweft-test://proof/final-hir-layer-expression-recovery",
        "layer Recovered: custom { z = left + }\n",
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    let (owner, item, _) = layer(&module, 0);
    assert_eq!(
        item.state(),
        &HirItemPoisonState::Poisoned(HirItemIssue::InvalidMember)
    );
    let retained = module
        .declaration_members()
        .resolve(item.members()[0])
        .unwrap();
    assert_eq!(
        retained.state(),
        HirDeclarationMemberPoisonState::Poisoned(HirDeclarationMemberIssue::RecoveredChild)
    );
    let HirDeclarationMemberKind::LayerExpression(HirLayerExpressionMember::Z(payload)) =
        retained.kind()
    else {
        panic!("recovered member must stay in the typed z family")
    };
    let HirLayerMemberValue::Recovered(Some(expression)) = payload.value() else {
        panic!("recovered z must retain its exact expression owner")
    };
    assert!(module.slots().resolve(*expression).unwrap().is_poisoned());
    assert_source_backed_child(&module, *expression);
    assert_item_owner_whole_recovery(&module, owner);
}

#[test]
fn layer_freeze_rejects_kind_and_expression_scope_tampering() {
    let source = concat!(
        "layer Surface: custom { z = 1 }\n",
        "action scope_donor() { return }\n",
    );

    assert_layer_freeze_rejects("kind-tamper", source, |transaction, owner| {
        let (slots, arenas) = transaction.storage_mut();
        let original = arenas.items().resolve_staged(slots, owner).unwrap().clone();
        let HirItemKind::Layer(layer) = original.kind() else {
            panic!("final Layer item")
        };
        let replacement_layer = HirLayerDeclaration::try_new(
            owner,
            layer.header().clone(),
            HirLayerKind::Background,
            layer.members().into(),
        )
        .unwrap();
        let replacement = HirItem::try_new_with_state(
            owner,
            original.scope(),
            original.prefix().clone(),
            HirItemKind::Layer(replacement_layer),
            original.members().into(),
            *original.state(),
        )
        .unwrap();
        arenas
            .items()
            .revise_finalized(slots, owner, replacement)
            .unwrap();
    });

    assert_layer_freeze_rejects("expression-scope-tamper", source, |transaction, owner| {
        let sibling = transaction.source_ordered_items[1];
        let (expression, foreign_scope) = {
            let (slots, arenas) = transaction.storage_mut();
            let layer_scope = arenas.items().resolve_staged(slots, owner).unwrap().scope();
            let expression = slots
                .live_ids::<ExprId>()
                .find(|candidate| {
                    arenas
                        .expressions()
                        .resolve_staged(slots, *candidate)
                        .is_ok_and(|expression| expression.scope() == layer_scope)
                })
                .expect("Layer expression owner");
            let sibling = arenas.items().resolve_staged(slots, sibling).unwrap();
            let HirItemKind::Action(sibling) = sibling.kind() else {
                panic!("scope donor must be an Action")
            };
            (expression, sibling.callable_scope())
        };
        revise_expression_scope(transaction, expression, foreign_scope);
    });
}

#[test]
fn incremental_layer_preserves_reconciled_owners_and_retires_only_edited_children() {
    let name = SourceName::path("proof/layer-incremental.arcw");
    let document_id = "arcweft-test://proof/layer-incremental";
    let initial_source = concat!(
        "layer First: custom {\n",
        "    z = 1\n",
        "    visible = true\n",
        "}\n",
        "layer Second: dialogue {\n",
        "    z = 2\n",
        "}\n",
    );
    let reordered_source = concat!(
        "layer Second: dialogue {\n",
        "    z = 2\n",
        "}\n",
        "layer Inserted: background {}\n",
        "layer First: custom {\n",
        "    z = 1\n",
        "    visible = true\n",
        "}\n",
    );
    let modified_source = concat!(
        "layer Second: dialogue {\n",
        "    z = 2\n",
        "}\n",
        "layer Inserted: background {}\n",
        "layer First: custom {\n",
        "    z = 3\n",
        "    visible = true\n",
        "}\n",
    );
    let mut syntax = SyntaxDatabase::try_new().unwrap();
    let initial = syntax
        .parse_initial(
            SourceSnapshotId::initial(name.clone()),
            source_document(document_id, &name, initial_source),
            arcweft_lang_syntax::parser::ParseOptions::default(),
        )
        .unwrap();
    let key = module_key(&initial);
    let mut database = HirDatabase::try_new().unwrap();
    let first = lower(&mut database, &initial, &key);
    let (first_owner, first_item, first_layer) = layer_named(&first, "First");
    let (second_owner, _, second_layer) = layer_named(&first, "Second");
    let first_members = first_item.members().to_vec();
    let first_z = layer_expression(&first, first_layer, |member| {
        matches!(member, HirLayerExpressionMember::Z(_))
    });
    let first_visible = layer_expression(&first, first_layer, |member| {
        matches!(member, HirLayerExpressionMember::Visible(_))
    });
    let second_z = layer_expression(&first, second_layer, |member| {
        matches!(member, HirLayerExpressionMember::Z(_))
    });

    let reordered = syntax
        .reparse(
            &initial,
            &[SourceEdit::new(
                initial
                    .document()
                    .span(SourceRange::new(0, initial_source.len()))
                    .unwrap(),
                reordered_source,
            )],
            arcweft_lang_syntax::parser::ParseOptions::default(),
        )
        .unwrap();
    let second_output = lower_output(&mut database, &reordered, &key);
    let second = Arc::clone(second_output.module());
    let (reordered_first_owner, reordered_first_item, reordered_first) =
        layer_named(&second, "First");
    let (reordered_second_owner, _, reordered_second) = layer_named(&second, "Second");
    let (inserted_owner, _, _) = layer_named(&second, "Inserted");
    assert_eq!(reordered_first_owner, first_owner);
    assert_eq!(reordered_second_owner, second_owner);
    assert_ne!(inserted_owner, first_owner);
    assert_ne!(inserted_owner, second_owner);
    assert_eq!(
        second_output.invalidations().changed_items(),
        [inserted_owner]
    );
    assert_eq!(reordered_first_item.members(), first_members.as_slice());
    assert_eq!(
        layer_expression(&second, reordered_first, |member| matches!(
            member,
            HirLayerExpressionMember::Z(_)
        )),
        first_z
    );
    assert_eq!(
        layer_expression(&second, reordered_first, |member| matches!(
            member,
            HirLayerExpressionMember::Visible(_)
        )),
        first_visible
    );
    assert_eq!(
        layer_expression(&second, reordered_second, |member| matches!(
            member,
            HirLayerExpressionMember::Z(_)
        )),
        second_z
    );

    let modified = syntax
        .reparse(
            &reordered,
            &[SourceEdit::new(
                reordered
                    .document()
                    .span(SourceRange::new(0, reordered_source.len()))
                    .unwrap(),
                modified_source,
            )],
            arcweft_lang_syntax::parser::ParseOptions::default(),
        )
        .unwrap();
    let third_output = lower_output(&mut database, &modified, &key);
    let third = third_output.module();
    let (modified_first_owner, modified_first_item, modified_first) = layer_named(third, "First");
    let (modified_second_owner, _, modified_second) = layer_named(third, "Second");
    assert_eq!(modified_first_owner, first_owner);
    assert_eq!(modified_second_owner, second_owner);
    assert_eq!(modified_first_item.members(), first_members.as_slice());
    assert_eq!(third_output.invalidations().changed_items(), [first_owner]);

    let new_first_z = layer_expression(third, modified_first, |member| {
        matches!(member, HirLayerExpressionMember::Z(_))
    });
    assert_ne!(new_first_z, first_z);
    assert_eq!(
        layer_expression(third, modified_first, |member| matches!(
            member,
            HirLayerExpressionMember::Visible(_)
        )),
        first_visible
    );
    assert_eq!(
        layer_expression(third, modified_second, |member| matches!(
            member,
            HirLayerExpressionMember::Z(_)
        )),
        second_z
    );
    assert!(
        third
            .arenas()
            .expressions()
            .resolve(third.slots(), first_z)
            .is_err()
    );
    assert_source_backed_child(third, new_first_z);
}
