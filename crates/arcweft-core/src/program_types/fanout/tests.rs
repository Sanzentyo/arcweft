use super::*;
use crate::{
    awbc::schema::{
        AwbcProgram, AwbcRuntimeType, AwbcStringId, AwbcTypeId, AwbcVariantCase,
        AwbcVariantIdentity,
    },
    pattern::RuntimeBuiltinVariantIdentity,
    plan::{RuntimePlanBuilder, RuntimePlanSequenceKind, RuntimePlanTypeSeed},
};

fn identity(index: u8) -> RuntimeSemanticTypeId {
    RuntimeSemanticTypeId::from_bytes([index; 32])
}

fn fixtures() -> (crate::plan::RuntimePlan, AwbcProgram) {
    fixtures_scoped(false)
}

#[expect(
    clippy::too_many_lines,
    reason = "one equivalent original type graph keeps Native and AWBC scope and payload coordinates together"
)]
fn fixtures_scoped(scoped: bool) -> (crate::plan::RuntimePlan, AwbcProgram) {
    use AwbcRuntimeTypeShape as A;
    use RuntimePlanTypeProjection as P;
    let sequence = |item| P::Sequence {
        kind: RuntimePlanSequenceKind::Vec,
        item: identity(item),
    };
    let result = |value, error, value_payload, error_payload| P::Result {
        value: identity(value),
        error: identity(error),
        value_payload: identity(value_payload),
        error_payload: identity(error_payload),
    };
    let binder = crate::plan::RuntimeTypeBinder::new(0, 0, 1);
    let scope = crate::plan::RuntimeTypeScope::root().enter(binder).unwrap();
    let contract = || {
        crate::plan::RuntimeFunctionTypeContract::new(
            binder,
            crate::effect_row::EffectPredicate::unconstrained(),
            crate::effect_row::EffectFormula::empty(),
        )
    };
    let mut rows = vec![
        P::Bool,
        P::String,
        sequence(1),
        result(1, 2, 7, 8),
        result(3, 2, 9, 8),
        sequence(4),
        P::Tuple(vec![identity(1)].into()),
        P::Tuple(vec![identity(2)].into()),
        P::Tuple(vec![identity(3)].into()),
        result(3, 1, 9, 7),
    ];
    if scoped {
        rows.push(P::Function {
            contract: contract(),
            parameters: Box::new([]),
            result: identity(5),
        });
    }
    let mut builder = RuntimePlanBuilder::new();
    builder
        .admit_type_batch(
            rows.into_iter().zip(1_u8..).map(|(row, id)| {
                let seed = RuntimePlanTypeSeed::new(identity(id), row);
                if scoped && matches!(id, 4..=6) {
                    seed.with_scope(scope.clone())
                } else {
                    seed
                }
            }),
            [],
        )
        .unwrap();
    let plan = builder.finish().unwrap();

    let sequence = |item| A::Sequence {
        kind: RuntimePlanSequenceKind::Vec,
        item: AwbcTypeId(item),
    };
    let result = |ok, error| A::Variant {
        owner: AwbcVariantIdentity::Builtin(RuntimeBuiltinVariantIdentity::Result),
        arguments: vec![],
        cases: vec![
            AwbcVariantCase {
                name: AwbcStringId(0),
                payload: Some(AwbcTypeId(ok)),
            },
            AwbcVariantCase {
                name: AwbcStringId(1),
                payload: Some(AwbcTypeId(error)),
            },
        ],
    };
    let mut rows = vec![
        A::Bool,
        A::String,
        sequence(0),
        result(6, 7),
        result(8, 7),
        sequence(3),
        A::Tuple(vec![AwbcTypeId(0)]),
        A::Tuple(vec![AwbcTypeId(1)]),
        A::Tuple(vec![AwbcTypeId(2)]),
        result(8, 6),
    ];
    if scoped {
        rows.push(A::Function {
            contract: contract(),
            parameters: vec![],
            result: AwbcTypeId(4),
        });
    }
    let program = AwbcProgram {
        strings: vec!["Ok".into(), "Err".into()],
        runtime_types: rows
            .into_iter()
            .zip(1_u8..)
            .map(|(row, id)| {
                let row = AwbcRuntimeType::new(identity(id), row);
                if scoped && matches!(id, 4..=6) {
                    row.with_scope(scope.clone())
                } else {
                    row
                }
            })
            .collect(),
        ..AwbcProgram::default()
    };
    program.validate_type_graph().unwrap();
    (plan, program)
}

#[test]
fn fanout_construction_accepts_original_scoped_result_bindings() {
    use crate::entry::RuntimeSchemaLimits;
    use crate::entry::schema::value_budget::ValidationWork;
    use crate::task::RuntimeProgramOwner;
    use crate::value::{RuntimeRetainedValueAdmission, RuntimeValue as V, RuntimeVariantValue};
    use std::sync::Arc;
    let (plan, awbc) = fixtures_scoped(true);
    for owner in [
        RuntimeProgramOwner::Plan(Arc::new(plan)),
        RuntimeProgramOwner::Awbc(Arc::new(awbc)),
    ] {
        let types = owner.types();
        let binding = Arc::new(
            types
                .instantiate_function_effects(
                    identity(11),
                    &[],
                    &ValidationWork::new(RuntimeSchemaLimits::engine_default()),
                )
                .unwrap(),
        );
        let case = types.variant_case(identity(4), 0).unwrap();
        let value = RuntimeVariantValue::try_from_program_case(
            case,
            Some(V::Tuple(vec![V::Bool(true)])),
            Some(binding.clone()),
            |_| Ok::<_, ()>(()),
        )
        .unwrap();
        let mut slots = [Some(value)];
        let mut admission = RuntimeRetainedValueAdmission::for_program(
            owner.clone(),
            RuntimeSchemaLimits::engine_default(),
        );
        let value = admission
            .prepare_fanout(identity(5), identity(4), Some(&binding), &mut slots)
            .unwrap()
            .commit();
        assert_eq!(
            value,
            V::result_ok(crate::value::runtime_sequence_values(vec![V::Bool(true)]))
        );
        assert!(slots[0].is_none());
    }
}

#[test]
fn fanout_output_follows_original_sequence_and_result_types() {
    let (plan, awbc) = fixtures();
    for types in [
        RuntimeProgramTypes::Plan(&plan),
        RuntimeProgramTypes::Awbc(&awbc),
    ] {
        assert_eq!(
            types.fanout_output_kind(identity(3), identity(1)).unwrap(),
            RuntimeFanoutOutputKind::Sequence
        );
        assert_eq!(
            types.fanout_output_kind(identity(6), identity(4)).unwrap(),
            RuntimeFanoutOutputKind::Sequence
        );
        assert_eq!(
            types.fanout_output_kind(identity(5), identity(4)).unwrap(),
            RuntimeFanoutOutputKind::ResultSequence
        );
        for (aggregate, child) in [(5, 1), (10, 4), (3, 4), (1, 1)] {
            assert!(matches!(
                types.fanout_output_kind(identity(aggregate), identity(child)),
                Err(RuntimeProgramTypeError::InvalidFanoutOutput { .. })
            ));
        }
        assert!(matches!(
            types.fanout_output_kind(identity(99), identity(1)),
            Err(RuntimeProgramTypeError::Missing { .. })
        ));
    }
}

fn owners() -> [crate::task::RuntimeProgramOwner; 2] {
    let (plan, awbc) = fixtures();
    [
        crate::task::RuntimeProgramOwner::Plan(std::sync::Arc::new(plan)),
        crate::task::RuntimeProgramOwner::Awbc(std::sync::Arc::new(awbc)),
    ]
}

#[test]
fn fanout_construction_preserves_order_and_transposes_only_declared_result_outputs() {
    use crate::value::{
        RuntimeRetainedValueAdmission, RuntimeValue as V, runtime_sequence_values as seq,
    };
    for owner in owners() {
        for values in [
            vec![],
            vec![V::result_ok(V::Bool(true)), V::result_ok(V::Bool(false))],
        ] {
            let expected = values
                .iter()
                .map(|v| v.builtin_variant_case().unwrap().1.unwrap().clone())
                .collect();
            let mut slots = values.into_iter().map(Some).collect::<Vec<_>>();
            let mut admission = RuntimeRetainedValueAdmission::for_program(
                owner.clone(),
                crate::entry::RuntimeSchemaLimits::engine_default(),
            );
            let prepared = admission
                .prepare_fanout(identity(5), identity(4), None, &mut slots)
                .unwrap();
            prepared.with_view(|view| {
                assert_eq!(
                    view.inspect().builtin_variant_case().unwrap().0,
                    crate::pattern::RuntimeBuiltinVariantCaseIdentity::ResultOk
                );
            });
            assert_eq!(prepared.commit(), V::result_ok(seq(expected)));
            assert!(slots.iter().all(Option::is_none));
        }
        let error = "first source error".to_owned();
        let pointer = error.as_ptr();
        let mut slots = [None, None, None];
        slots[2] = Some(V::result_err(V::String("later source error".to_owned())));
        slots[0] = Some(V::result_ok(V::Bool(true)));
        slots[1] = Some(V::result_err(V::String(error)));
        let mut admission = RuntimeRetainedValueAdmission::for_program(
            owner.clone(),
            crate::entry::RuntimeSchemaLimits::engine_default(),
        );
        let value = admission
            .prepare_fanout(identity(5), identity(4), None, &mut slots)
            .unwrap()
            .commit();
        assert_eq!(
            value.builtin_variant_case().unwrap().0,
            crate::pattern::RuntimeBuiltinVariantCaseIdentity::ResultErr
        );
        let (_, Some(V::String(error))) = value.try_into_builtin_variant_case().unwrap() else {
            panic!("expected original error payload")
        };
        assert_eq!(error, "first source error");
        assert_eq!(
            error.as_ptr(),
            pointer,
            "the error must move without a payload clone"
        );
        assert!(slots.iter().all(Option::is_none));

        let values = vec![
            V::result_ok(V::Bool(false)),
            V::result_err(V::String("retained".into())),
        ];
        let expected = seq(values.clone());
        let mut slots = values.into_iter().map(Some).collect::<Vec<_>>();
        let mut admission = RuntimeRetainedValueAdmission::for_program(
            owner,
            crate::entry::RuntimeSchemaLimits::engine_default(),
        );
        assert_eq!(
            admission
                .prepare_fanout(identity(6), identity(4), None, &mut slots)
                .unwrap()
                .commit(),
            expected
        );
    }
}

#[test]
fn fanout_construction_shares_nodes_and_preserves_every_slot_on_preflight_failure() {
    use crate::entry::RuntimeSchemaLimits;
    use crate::value::{RuntimeRetainedValueAdmission, RuntimeValue as V};
    for owner in owners() {
        let values = vec![Some(V::Bool(true)), Some(V::Bool(false))];
        let limits = RuntimeSchemaLimits {
            max_nodes: 3,
            ..RuntimeSchemaLimits::engine_default()
        };
        let mut slots = values.clone();
        let mut admission = RuntimeRetainedValueAdmission::for_program(owner.clone(), limits);
        let prepared = admission
            .prepare_fanout(identity(3), identity(1), None, &mut slots)
            .unwrap();
        prepared.abort();
        assert_eq!(
            slots, values,
            "a consumer may reject without taking any original slot"
        );
        let mut admission = RuntimeRetainedValueAdmission::for_program(owner.clone(), limits);
        admission.validate(&V::Bool(true)).unwrap();
        let Err(error) = admission.prepare_fanout(identity(3), identity(1), None, &mut slots)
        else {
            panic!("existing bank consumption must count");
        };
        assert_eq!(slots, values);
        assert!(matches!(
            error,
            crate::value::AwbcRuntimeValueSnapshotError::Resource(
                crate::entry::RuntimeSchemaError::BudgetExceeded { budget: "nodes" }
            )
        ));
        let Err(repeated) = admission.prepare_fanout(identity(3), identity(1), None, &mut slots)
        else {
            panic!("failed bank must remain failed");
        };
        assert_eq!(error, repeated);
        assert_eq!(slots, values);
        let mut missing = [Some(V::Bool(true)), None];
        let mut admission = RuntimeRetainedValueAdmission::for_program(
            owner.clone(),
            RuntimeSchemaLimits::engine_default(),
        );
        assert!(
            admission
                .prepare_fanout(identity(3), identity(1), None, &mut missing)
                .is_err()
        );
        assert_eq!(missing, [Some(V::Bool(true)), None]);
        let mut wrong = [Some(V::String("wrong child".to_owned()))];
        let mut admission = RuntimeRetainedValueAdmission::for_program(
            owner,
            RuntimeSchemaLimits::engine_default(),
        );
        assert!(
            admission
                .prepare_fanout(identity(3), identity(1), None, &mut wrong)
                .is_err()
        );
        assert_eq!(wrong, [Some(V::String("wrong child".to_owned()))]);
    }
}

#[test]
fn fanout_construction_admits_result_headers_depth_and_legacy_payload_without_double_charging() {
    use crate::entry::RuntimeSchemaLimits;
    use crate::value::{RuntimePayload, RuntimeRetainedValueAdmission, RuntimeValue as V};
    for owner in owners() {
        for (nodes, depth, accepted) in [(9, 3, true), (8, 3, false), (9, 2, false)] {
            let values = [
                Some(V::result_ok(V::Bool(true))),
                Some(V::result_ok(V::Bool(false))),
            ];
            let mut slots = values.clone();
            let mut admission = RuntimeRetainedValueAdmission::for_program(
                owner.clone(),
                RuntimeSchemaLimits {
                    max_nodes: nodes,
                    max_depth: depth,
                    ..RuntimeSchemaLimits::engine_default()
                },
            );
            let result = admission.prepare_fanout(identity(5), identity(4), None, &mut slots);
            assert_eq!(result.is_ok(), accepted, "nodes={nodes}, depth={depth}");
            drop(result);
            assert_eq!(slots, values);
        }
        let mut slots = [Some(RuntimePayload::new(V::Bool(true)))];
        let mut admission = RuntimeRetainedValueAdmission::for_program(
            owner,
            RuntimeSchemaLimits {
                max_nodes: 2,
                ..RuntimeSchemaLimits::engine_default()
            },
        );
        let value = admission
            .prepare_fanout(identity(3), identity(1), None, &mut slots)
            .unwrap()
            .commit();
        assert_eq!(
            value,
            crate::value::runtime_sequence_values(vec![V::Bool(true)])
        );
        assert!(slots[0].is_none());
    }
}

#[test]
fn fanout_construction_charges_canonical_headers_and_existing_payload_bytes_once() {
    use crate::entry::RuntimeSchemaLimits;
    use crate::value::{
        RuntimeRetainedValueAdmission, RuntimeValue as V, runtime_sequence_values as seq,
    };
    let bytes = |value: &V| value.try_canonical_bytes(1_000_000).unwrap().len() as u64;
    for owner in owners() {
        for result in [false, true] {
            let children = if result {
                vec![V::result_ok(V::Bool(true)), V::result_ok(V::Bool(false))]
            } else {
                vec![V::Bool(true), V::Bool(false)]
            };
            let output = seq(vec![V::Bool(true), V::Bool(false)]);
            let expected = if result {
                children.iter().map(bytes).sum::<u64>() + bytes(&V::result_ok(output))
                    - 2 * bytes(&V::Bool(true))
            } else {
                bytes(&output)
            };
            for (limit, previously_admitted, accepted) in [
                (expected, false, true),
                (expected - 1, false, false),
                (expected + bytes(&V::Bool(true)), true, true),
                (expected, true, false),
            ] {
                let mut slots = children.iter().cloned().map(Some).collect::<Vec<_>>();
                let before = slots.clone();
                let mut admission = RuntimeRetainedValueAdmission::for_program(
                    owner.clone(),
                    RuntimeSchemaLimits {
                        max_encoded_bytes: limit,
                        ..RuntimeSchemaLimits::engine_default()
                    },
                );
                if previously_admitted {
                    admission.validate(&V::Bool(true)).unwrap();
                }
                let prepared = admission.prepare_fanout(
                    identity(if result { 5 } else { 3 }),
                    identity(if result { 4 } else { 1 }),
                    None,
                    &mut slots,
                );
                assert_eq!(
                    prepared.is_ok(),
                    accepted,
                    "Result={result}, bytes={limit}, prior={previously_admitted}"
                );
                drop(prepared);
                assert_eq!(slots, before);
            }
        }
    }
}
