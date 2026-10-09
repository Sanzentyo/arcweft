use super::*;
use crate::stream::StreamOp;

fn condition() -> crate::value::RuntimeExpr {
    let (plan, _, function) = crate::plan::body_semantic::tests::producer_host_plan(true);
    let RuntimeFunctionSiteBody::Executable(body) =
        plan.function_sites().get(function).unwrap().body()
    else {
        panic!("actual Host fixture body");
    };
    let crate::plan::FlowOp::If { condition, .. } = &body.ops()[0] else {
        panic!("actual Host fixture condition");
    };
    condition.clone()
}

#[test]
fn stream_child_preflight_preserves_then_before_else_and_poison() {
    for (then_count, else_count) in [(3, 4), (4, 3)] {
        let ops = vec![StreamOp::If {
            condition: condition(),
            then_ops: vec![StreamOp::Return; then_count],
            else_ops: vec![StreamOp::Return; else_count],
        }];
        let builder = crate::plan::RuntimePlanBuilder::new();
        let plan = builder.finish().unwrap();
        let mut auxiliary = RuntimeCallableChildPreflight::new(&plan.inventory);
        let mut meter = TaskSemanticMeter::new(0, 0);
        let mut check = Children {
            table: 13,
            ordinal: 0,
            maximum: 2,
            meter: &mut meter,
        };
        assert!(
            matches!(check.stream(&mut auxiliary, &ops),Err(RuntimeTaskPlanImageError::Children{table:13,ordinal:0,actual,maximum:2}) if actual==then_count)
        );
        assert!(check.stream(&mut auxiliary, &[]).is_err());
        assert_eq!(meter.totals(), (0, 0));
    }
}

#[test]
fn deep_stream_child_count_uses_cursors_without_recursive_rust_calls() {
    let source = condition();
    let pattern = crate::pattern::RuntimePattern::from_admitted_parts(
        source.ty(),
        crate::pattern::RuntimePatternKind::Discard,
    );
    let mut ops = vec![StreamOp::Return];
    for _ in 0..8192 {
        ops = vec![StreamOp::ForNext {
            pattern: pattern.clone(),
            source: source.clone(),
            body: ops,
        }];
    }
    let builder = crate::plan::RuntimePlanBuilder::new();
    let plan = builder.finish().unwrap();
    let mut auxiliary = RuntimeCallableChildPreflight::new(&plan.inventory);
    let mut meter = TaskSemanticMeter::new(0, 0);
    Children {
        table: 13,
        ordinal: 0,
        maximum: 1,
        meter: &mut meter,
    }
    .stream(&mut auxiliary, &ops)
    .unwrap();
    assert_eq!(meter.totals(), (0, 0));
    // Drop the deliberately deep owned fixture iteratively too.
    while let Some(op) = ops.pop() {
        if let StreamOp::ForNext { body, .. } = op {
            ops = body;
        }
    }
}

#[test]
fn expression_and_pattern_literal_children_precede_invalid_types_and_hash_limits() {
    let ty = crate::runtime_id::RuntimePlanTypeId::from_accepted_ordinal(
        std::num::NonZeroU32::new(99).unwrap(),
    );
    for in_pattern in [false, true] {
        let literal = crate::value::RuntimeValue::Tuple(vec![
            crate::value::RuntimeValue::Tuple(vec![crate::value::RuntimeValue::Unit; 3]),
            crate::value::RuntimeValue::NeedHandle(crate::tests::reusable_need(
                "need.invalid_literal",
            )),
        ]);
        let builder = crate::plan::RuntimePlanBuilder::new();
        let plan = builder.finish().unwrap();
        let mut auxiliary = RuntimeCallableChildPreflight::new(&plan.inventory);
        let mut meter = TaskSemanticMeter::new(0, 0);
        let mut check = Children {
            table: 4,
            ordinal: 7,
            maximum: 2,
            meter: &mut meter,
        };
        let result = if in_pattern {
            let pattern = crate::pattern::RuntimePattern::from_admitted_parts(
                ty,
                crate::pattern::RuntimePatternKind::Literal(literal),
            );
            check.node(&mut auxiliary, RuntimeExpressionNode::Pattern(&pattern))
        } else {
            let expression = crate::value::RuntimeExpr::from_admitted_parts(
                ty,
                crate::value::RuntimeExprKind::Value(literal),
            );
            check.node(
                &mut auxiliary,
                RuntimeExpressionNode::Expression(&expression),
            )
        };
        assert!(matches!(
            result,
            Err(RuntimeTaskPlanImageError::Children {
                table: 4,
                ordinal: 7,
                actual: 3,
                maximum: 2
            })
        ));
        assert!(meter.status().is_err());
        assert_eq!(meter.totals(), (0, 0));
    }
}

#[test]
fn callable_metadata_counts_reach_expression_row_and_keep_sticky_error() {
    let (mut plan, root) = crate::plan::body_semantic::tests::callable_graph_fixture(2, 1);
    let mut rows = plan.callable_states().iter().cloned().collect::<Vec<_>>();
    rows[root.index()].position = crate::plan::RuntimeCallablePosition::WithinGroup {
        group: 0,
        bound: (0..3)
            .map(
                |parameter| crate::plan::RuntimeCallableParameterCoordinate {
                    group: 0,
                    parameter,
                },
            )
            .collect(),
    };
    let ty = rows[root.index()].function_type;
    plan.inventory.callable_states = crate::plan::RuntimeCallableStateTable::from_admitted(rows);
    let expression = crate::value::RuntimeExpr::from_admitted_parts(
        ty,
        crate::value::RuntimeExprKind::MakeCallable {
            state: root,
            captures: vec![],
        },
    );
    let mut auxiliary = RuntimeCallableChildPreflight::new(&plan.inventory);
    let mut meter = TaskSemanticMeter::new(0, 0);
    let mut check = Children {
        table: 10,
        ordinal: 2,
        maximum: 2,
        meter: &mut meter,
    };
    assert!(matches!(
        check.node(
            &mut auxiliary,
            RuntimeExpressionNode::Expression(&expression)
        ),
        Err(RuntimeTaskPlanImageError::Children {
            table: 10,
            ordinal: 2,
            actual: 3,
            maximum: 2
        })
    ));
    assert!(
        check
            .node(
                &mut auxiliary,
                RuntimeExpressionNode::Expression(&expression)
            )
            .is_err()
    );
    assert_eq!(meter.totals(), (0, 0));
}

fn image_with_types(
    seeds: impl IntoIterator<Item = crate::plan::RuntimePlanTypeSeed>,
) -> UnsealedRuntimePlanImage {
    let mut builder = crate::plan::RuntimePlanBuilder::new();
    builder.admit_type_batch(seeds, []).unwrap();
    let owner = builder.task_coordinate_owner(0);
    let inventory = builder
        .prepare_inventory(RuntimeTaskPlanSealLimits::default())
        .unwrap();
    UnsealedRuntimePlanImage::new(inventory, owner, Box::new([]), None).unwrap()
}
#[test]
fn common_codec_width_limit_precedes_unbound_default_and_hash_budget() {
    use crate::entry::schema::{RuntimeCodecUse as Codec, RuntimeFieldCodecUse};
    use crate::plan::{
        RuntimePlanRecordField, RuntimePlanTypeProjection as Type, RuntimePlanTypeSeed as Seed,
    };
    let identity = |tag| crate::pattern::RuntimeSemanticTypeId::from_bytes([tag; 32]);
    let image = image_with_types([
        Seed::new(
            identity(1),
            Type::Record(Box::new([RuntimePlanRecordField::new(
                "value",
                identity(2),
            )])),
        )
        .with_data_codec(Codec::Record {
            name: "Wire".into(),
            deny_unknown_fields: false,
            fields: Box::new([RuntimeFieldCodecUse {
                wire_name: "value".into(),
                has_default: true,
                default_program: Some(
                    arcweft_id::runtime_program::RuntimePureProgramId::from_checked_digest(
                        [99; 32],
                    ),
                ),
                skip: false,
                bytes_format: None,
                value: Codec::Tuple {
                    items: Box::new([Codec::Plain, Codec::Plain, Codec::Plain]),
                },
            }]),
        }),
        Seed::new(identity(2), Type::Tuple(Box::new([identity(3); 3]))),
        Seed::new(identity(3), Type::Bool),
    ]);
    let mut meter = TaskSemanticMeter::new(0, 0);
    assert!(matches!(
        image.preflight(
            RuntimeTaskPlanSealLimits {
                max_children_per_row: 2,
                ..RuntimeTaskPlanSealLimits::default()
            },
            &mut meter
        ),
        Err(RuntimeTaskPlanImageError::Children {
            table: 0,
            ordinal: 0,
            actual: 3,
            maximum: 2
        })
    ));
    assert_eq!(meter.totals(), (0, 0));
    assert!(meter.status().is_err());
}
#[test]
fn lexical_scope_and_function_effect_lists_enter_ordered_type_preflight() {
    use crate::plan::{
        RuntimeFunctionTypeContract, RuntimePlanTypeProjection as Type,
        RuntimePlanTypeSeed as Seed, RuntimeTypeBinder, RuntimeTypeScope,
    };
    let identity = |tag| crate::pattern::RuntimeSemanticTypeId::from_bytes([tag; 32]);
    let mut scope = RuntimeTypeScope::root();
    for _ in 0..3 {
        scope = scope.enter(RuntimeTypeBinder::new(1, 0, 0)).unwrap();
    }
    let scope_image = image_with_types([Seed::new(identity(1), Type::Bool).with_scope(scope)]);
    let effect_image = image_with_types([
        Seed::new(
            identity(1),
            Type::Function {
                contract: RuntimeFunctionTypeContract::monomorphic(
                    crate::effect_row::EffectSet::from_labels(["fs.read", "fs.write", "io.read"])
                        .unwrap(),
                ),
                parameters: Box::new([]),
                result: identity(2),
            },
        ),
        Seed::new(identity(2), Type::Bool),
    ]);
    for image in [&scope_image, &effect_image] {
        let mut meter = TaskSemanticMeter::new(0, 0);
        assert!(matches!(
            image.preflight(
                RuntimeTaskPlanSealLimits {
                    max_children_per_row: 2,
                    ..RuntimeTaskPlanSealLimits::default()
                },
                &mut meter
            ),
            Err(RuntimeTaskPlanImageError::Children {
                table: 0,
                ordinal: 0,
                actual: 3,
                maximum: 2
            })
        ));
        assert_eq!(meter.totals(), (0, 0));
        assert!(meter.status().is_err());
    }
}
