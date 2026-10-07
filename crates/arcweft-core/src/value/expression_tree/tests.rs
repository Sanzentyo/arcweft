use super::*;
use crate::runtime_id::{
    RuntimeCallableStateId, RuntimeDialogueContentTemplateId, RuntimeDialogueEffectSiteId,
    RuntimePlanTypeId,
};
use crate::value::{
    RuntimeAgentExpr, RuntimeAgentPredicateExpr, RuntimeDialogueContentEffectBindingExpr,
    RuntimeExprMatchArm, RuntimeUnaryOp, RuntimeValue,
};
use RuntimeExpressionChildRole as R;
use std::num::NonZeroU32;

fn expression(kind: RuntimeExprKind) -> RuntimeExpr {
    RuntimeExpr::from_admitted_parts(
        RuntimePlanTypeId::from_accepted_ordinal(NonZeroU32::MIN),
        kind,
    )
}

fn literal(label: &str) -> RuntimeExpr {
    expression(RuntimeExprKind::Value(RuntimeValue::String(
        label.to_owned(),
    )))
}

fn discard() -> RuntimePattern {
    RuntimePattern::from_admitted_parts(
        RuntimePlanTypeId::from_accepted_ordinal(NonZeroU32::MIN),
        RuntimePatternKind::Discard,
    )
}

fn labels(root: &RuntimeExpr) -> Vec<(RuntimeExpressionChildRole, String)> {
    let mut labels = Vec::new();
    root.try_visit_owned_tree(&mut |role, node| {
        if let RuntimeExpressionNode::Expression(expr) = node
            && let RuntimeExprKind::Value(RuntimeValue::String(label)) = expr.kind()
        {
            labels.push((role, label.clone()));
        }
        Ok::<(), ()>(())
    })
    .unwrap();
    labels
}

#[test]
fn content_values_and_sparse_effect_captures_keep_their_source_roles() {
    let binding = |effect, captures| RuntimeDialogueContentEffectBindingExpr {
        site: RuntimeDialogueEffectSiteId::from_zero_based(effect).unwrap(),
        state: RuntimeCallableStateId::from_zero_based(effect).unwrap(),
        captures,
    };
    let root = expression(RuntimeExprKind::DialogueContent {
        template: RuntimeDialogueContentTemplateId::from_zero_based(0).unwrap(),
        values: vec![literal("value")],
        effects: vec![
            binding(0, vec![]),
            binding(1, vec![literal("first"), literal("second")]),
        ],
    });
    assert_eq!(
        labels(&root),
        [
            (R::ContentValue { ordinal: 0 }, "value".to_owned()),
            (
                R::ContentCapture {
                    effect: 1,
                    capture: 0
                },
                "first".to_owned()
            ),
            (
                R::ContentCapture {
                    effect: 1,
                    capture: 1
                },
                "second".to_owned()
            ),
        ]
    );
}

#[test]
fn sparse_match_guards_and_optional_range_end_keep_distinct_roles() {
    let root = expression(RuntimeExprKind::Match {
        scrutinee: Box::new(expression(RuntimeExprKind::Range {
            start: None,
            end: Some(Box::new(literal("end"))),
            inclusive: true,
        })),
        arms: vec![
            RuntimeExprMatchArm::from_admitted_parts(discard(), None, literal("arm0")),
            RuntimeExprMatchArm::from_admitted_parts(
                discard(),
                Some(literal("guard1")),
                literal("arm1"),
            ),
        ],
    });
    assert_eq!(
        labels(&root),
        [
            (R::RangeEnd, "end".to_owned()),
            (R::MatchValue { arm: 0 }, "arm0".to_owned()),
            (R::MatchGuard { arm: 1 }, "guard1".to_owned()),
            (R::MatchValue { arm: 1 }, "arm1".to_owned()),
        ]
    );
    assert_eq!(
        root.owned_children()
            .map(|(role, _)| role)
            .collect::<Vec<_>>(),
        [
            R::Scrutinee,
            R::MatchPattern { arm: 0 },
            R::MatchValue { arm: 0 },
            R::MatchPattern { arm: 1 },
            R::MatchGuard { arm: 1 },
            R::MatchValue { arm: 1 },
        ]
    );
}

#[test]
fn standard_map_retains_selected_operand_order_and_roles() {
    for (order, roles) in [
        (
            RuntimeStandardMapOperandOrder::MappingThenReceiver,
            [R::Mapping, R::Source],
        ),
        (
            RuntimeStandardMapOperandOrder::ReceiverThenMapping,
            [R::Source, R::Mapping],
        ),
    ] {
        let root = expression(RuntimeExprKind::StandardMap {
            family: crate::value::RuntimeStandardMapFamily::Vec,
            order,
            mapping: Box::new(literal("mapping")),
            source: Box::new(literal("source")),
        });
        assert_eq!(
            root.owned_children()
                .map(|(role, _)| role)
                .collect::<Vec<_>>(),
            roles
        );
    }
}

#[test]
fn sparse_dialogue_clear_fields_do_not_renumber_set_children() {
    use arcweft_interaction_model::dialogue::{
        CharacterDialogueFieldCoordinate as Field, CharacterDialogueOperation,
        CharacterDialoguePatchField, CharacterDialoguePatchOperation as Operation,
    };
    let root = expression(RuntimeExprKind::CharacterDialogue {
        operation: CharacterDialogueOperation::Reconfigure,
        target: Box::new(literal("target")),
        fields: vec![
            CharacterDialoguePatchField {
                coordinate: Field::Voice,
                operation: Operation::Clear,
            },
            CharacterDialoguePatchField {
                coordinate: Field::SourceLocale,
                operation: Operation::Set(literal("locale")),
            },
        ],
    });
    assert_eq!(
        labels(&root),
        [
            (RuntimeExpressionChildRole::Target, "target".to_owned()),
            (
                RuntimeExpressionChildRole::DialogueField { ordinal: 1 },
                "locale".to_owned()
            ),
        ]
    );
}

#[test]
fn affine_literal_in_a_later_pattern_alternative_still_rejects_copy() {
    let pattern = RuntimePattern::from_admitted_parts(
        discard().ty(),
        RuntimePatternKind::Or(Box::new([
            discard(),
            RuntimePattern::from_admitted_parts(
                discard().ty(),
                RuntimePatternKind::Literal(RuntimeValue::NeedHandle(crate::tests::reusable_need(
                    "need.tree.pattern",
                ))),
            ),
        ])),
    );
    let root = expression(RuntimeExprKind::IfLet {
        pattern,
        expr: Box::new(literal("input")),
        guard: None,
        then_expr: Box::new(literal("then")),
        else_expr: Box::new(literal("else")),
    });
    assert!(!root.literals_permit_copy());
    let mut visited = Vec::new();
    let error = root.try_visit_owned_tree(&mut |role, node| {
        visited.push(role);
        if matches!(node, RuntimeExpressionNode::Pattern(pattern) if matches!(pattern.kind(), RuntimePatternKind::Literal(_))) {
            return Err("pattern literal");
        }
        Ok(())
    });
    assert_eq!(error, Err("pattern literal"));
    assert_eq!(
        visited,
        [
            RuntimeExpressionChildRole::Root,
            RuntimeExpressionChildRole::Pattern,
            RuntimeExpressionChildRole::PatternItem { ordinal: 0 },
            RuntimeExpressionChildRole::PatternItem { ordinal: 1 }
        ]
    );
}

#[test]
fn agent_operand_projection_borrows_the_original_ordered_nodes() {
    let root = expression(RuntimeExprKind::Agent(RuntimeAgentExpr::Predicate(
        RuntimeAgentPredicateExpr::Compare {
            probe: Box::new(literal("probe")),
            op: crate::value::RuntimeAgentCompareOp::Eq,
            value: Box::new(literal("expected")),
        },
    )));
    assert_eq!(
        labels(&root),
        [
            (
                RuntimeExpressionChildRole::AgentOperand { ordinal: 0 },
                "probe".to_owned()
            ),
            (
                RuntimeExpressionChildRole::AgentOperand { ordinal: 1 },
                "expected".to_owned()
            ),
        ]
    );
    let RuntimeExprKind::Agent(agent) = root.kind() else {
        panic!("agent")
    };
    let mut iterator = agent.operands();
    assert!(std::ptr::eq(
        iterator.next().unwrap(),
        agent.operand(0).unwrap()
    ));
    assert!(std::ptr::eq(
        iterator.next().unwrap(),
        agent.operand(1).unwrap()
    ));
    assert!(iterator.next().is_none());
    assert!(iterator.next().is_none());
}

#[test]
fn deep_tree_and_copy_admission_use_the_iterative_owner() {
    let depth = 20_000;
    let mut root = literal("leaf");
    for _ in 0..depth {
        root = expression(RuntimeExprKind::Unary {
            op: RuntimeUnaryOp::Not,
            expr: Box::new(root),
        });
    }
    let mut count = 0;
    root.try_visit_owned_tree(&mut |_, _| {
        count += 1;
        Ok::<(), ()>(())
    })
    .unwrap();
    assert_eq!(count, depth + 1);
    assert!(root.literals_permit_copy());
    // The synthetic fixture is dropped iteratively too; this test covers the
    // tree/copy visitor, not recursive Box drop glue.
    loop {
        let kind = std::mem::replace(&mut root.kind, RuntimeExprKind::Value(RuntimeValue::Unit));
        if let RuntimeExprKind::Unary { expr, .. } = kind {
            root = *expr;
        } else {
            break;
        }
    }
}

#[test]
fn balanced_events_retain_each_actual_node_and_close_nested_empty_trees() {
    let root = expression(RuntimeExprKind::Tuple(vec![
        expression(RuntimeExprKind::Tuple(vec![])),
        literal("leaf"),
    ]));
    let mut actual = Vec::new();
    RuntimeExpressionNode::Expression(&root)
        .try_visit_owned_events(&mut |event| {
            match event {
                RuntimeExpressionTreeEvent::Enter { role, node } => actual.push((
                    true,
                    Some(role),
                    match node {
                        RuntimeExpressionNode::Expression(node) => {
                            std::ptr::from_ref(node) as usize
                        }
                        RuntimeExpressionNode::Pattern(_) => unreachable!(),
                    },
                )),
                RuntimeExpressionTreeEvent::Exit { node } => actual.push((
                    false,
                    None,
                    match node {
                        RuntimeExpressionNode::Expression(node) => {
                            std::ptr::from_ref(node) as usize
                        }
                        RuntimeExpressionNode::Pattern(_) => unreachable!(),
                    },
                )),
            }
            Ok::<(), ()>(())
        })
        .unwrap();
    assert_eq!(
        actual
            .iter()
            .map(|(enter, role, _)| (*enter, *role))
            .collect::<Vec<_>>(),
        [
            (true, Some(R::Root)),
            (true, Some(R::Item { ordinal: 0 })),
            (false, None),
            (true, Some(R::Item { ordinal: 1 })),
            (false, None),
            (false, None),
        ]
    );
    assert_eq!(actual[0].2, actual[5].2);
    assert_eq!(actual[1].2, actual[2].2);
    assert_eq!(actual[3].2, actual[4].2);
}

#[test]
fn balanced_tree_rejection_does_not_emit_pending_exits() {
    let root = expression(RuntimeExprKind::Tuple(vec![
        literal("first"),
        literal("later"),
    ]));
    let mut enters = 0;
    let mut exits = 0;
    let result = RuntimeExpressionNode::Expression(&root).try_visit_owned_events(&mut |event| {
        match event {
            RuntimeExpressionTreeEvent::Enter { .. } => {
                enters += 1;
                if enters == 2 {
                    return Err("child rejection");
                }
            }
            RuntimeExpressionTreeEvent::Exit { .. } => exits += 1,
        }
        Ok(())
    });
    assert_eq!(result, Err("child rejection"));
    assert_eq!((enters, exits), (2, 0));
}

#[test]
fn semantic_child_roles_use_explicit_tags_and_checked_source_ordinals() {
    let mut meter = crate::task::semantic::TaskSemanticMeter::new(20, 100);
    let mut encoder = crate::task::semantic::TaskSemanticEncoder::new(b"roles.v1\0", &mut meter);
    for role in [
        R::Root,
        R::Item { ordinal: 2 },
        R::ContentCapture {
            effect: 3,
            capture: 4,
        },
        R::MatchGuard { arm: 5 },
    ] {
        role.encode_semantic_path(&mut encoder);
    }
    let mut expected = b"roles.v1\0".to_vec();
    expected.extend([0, 4]);
    expected.extend(2_u32.to_le_bytes());
    expected.push(6);
    expected.extend(3_u32.to_le_bytes());
    expected.extend(4_u32.to_le_bytes());
    expected.push(33);
    expected.extend(5_u32.to_le_bytes());
    assert_eq!(encoder.finish().unwrap(), blake3::hash(&expected));
    if usize::BITS > 32 {
        let mut meter = crate::task::semantic::TaskSemanticMeter::new(20, 100);
        let mut encoder =
            crate::task::semantic::TaskSemanticEncoder::new(b"roles.v1\0", &mut meter);
        R::MatchGuard { arm: usize::MAX }.encode_semantic_path(&mut encoder);
        assert_eq!(
            encoder.finish(),
            Err(crate::task::semantic::TaskSemanticEncodingError::CountOverflow)
        );
    }
}
