use super::*;
use crate::awbc::schema::{
    AwbcFunctionId, AwbcRuntimeType, AwbcRuntimeTypeShape, AwbcStreamPlan, AwbcTypeId,
};
use crate::entry::{RuntimeSchemaLimits, schema::value_encoding::ValueAdmissionOrigin};
use crate::pattern::RuntimeCheckedType;

fn program(count: usize) -> AwbcProgram {
    AwbcProgram {
        runtime_types: vec![AwbcRuntimeType::new(
            RuntimeCheckedType::String.semantic_identity_digest(),
            AwbcRuntimeTypeShape::String,
        )],
        stream_plans: (0..count)
            .map(|_| AwbcStreamPlan {
                public_id: crate::awbc::schema::AwbcStringId(0),
                item_type: AwbcTypeId(0),
                error_type: AwbcTypeId(0),
                transform: AwbcFunctionId(0),
            })
            .collect(),
        ..AwbcProgram::default()
    }
}

fn queue<V>(value: V) -> FiberStreamState<V> {
    FiberStreamState {
        plan: AwbcStreamPlanId(0),
        queue: vec![value],
        closed: false,
        emitted_count: 1,
    }
}

#[test]
fn live_and_inert_queues_share_exact_resources_and_original_item_row() {
    let mut program = program(1);
    let live = [queue(RuntimeValue::String("cause".into()))];
    let inert = [queue(AwbcRuntimeValueSnapshot::String("cause".into()))];
    let exact = RuntimeSchemaLimits {
        max_nodes: 1,
        max_encoded_bytes: 7,
        max_string_bytes: 5,
        max_validation_work: 4,
        ..RuntimeSchemaLimits::engine_default()
    };
    for origin in [
        ValueAdmissionOrigin::Live,
        ValueAdmissionOrigin::InertSnapshot,
    ] {
        let validate = |limits| {
            let mut resources = ValueResourceAdmission::snapshot(limits, origin);
            match origin {
                ValueAdmissionOrigin::Live => FiberStreamState::validate_inventory(
                    &program,
                    AwbcFiberRoot::Function(AwbcFunctionId(0)),
                    &live,
                    &mut resources,
                    RuntimeValue::view,
                ),
                ValueAdmissionOrigin::InertSnapshot => FiberStreamState::validate_inventory(
                    &program,
                    AwbcFiberRoot::Function(AwbcFunctionId(0)),
                    &inert,
                    &mut resources,
                    AwbcRuntimeValueSnapshot::view,
                ),
            }
        };
        validate(exact).unwrap();
        for short in [
            RuntimeSchemaLimits {
                max_nodes: 0,
                ..exact
            },
            RuntimeSchemaLimits {
                max_encoded_bytes: 6,
                ..exact
            },
            RuntimeSchemaLimits {
                max_string_bytes: 4,
                ..exact
            },
            RuntimeSchemaLimits {
                max_validation_work: 3,
                ..exact
            },
            RuntimeSchemaLimits {
                max_sequence_items: 0,
                ..exact
            },
        ] {
            assert!(validate(short).is_err(), "one-over {short:?}");
        }
    }
    program.runtime_types.push(AwbcRuntimeType::new(
        program.runtime_types[0].semantic_identity(),
        AwbcRuntimeTypeShape::Bool,
    ));
    program.stream_plans[0].item_type = AwbcTypeId(1);
    assert!(matches!(
        FiberStreamState::validate_inventory(
            &program,
            AwbcFiberRoot::Function(AwbcFunctionId(0)),
            &inert,
            &mut ValueResourceAdmission::snapshot(exact, ValueAdmissionOrigin::InertSnapshot),
            AwbcRuntimeValueSnapshot::view
        ),
        Err(FiberStateError::Snapshot(
            crate::value::AwbcRuntimeValueSnapshotError::ProgramType(_)
        ))
    ));
}

#[test]
fn indexed_inventory_rejects_missing_duplicate_reordered_and_unknown_headers_first() {
    let program = program(2);
    let first = queue(AwbcRuntimeValueSnapshot::Callable(
        crate::value::AwbcRuntimeCallableSnapshot {
            state: crate::runtime_id::RuntimeCallableStateId::from_zero_based(0).unwrap(),
            retained: vec![],
        },
    ));
    let second = FiberStreamState {
        plan: AwbcStreamPlanId(1),
        ..queue(AwbcRuntimeValueSnapshot::String("cause".into()))
    };
    for streams in [
        vec![first.clone()],
        vec![first.clone(), first.clone()],
        vec![second.clone(), first.clone()],
        vec![
            first.clone(),
            FiberStreamState {
                plan: AwbcStreamPlanId(2),
                ..second.clone()
            },
        ],
        vec![
            first.clone(),
            FiberStreamState {
                emitted_count: 0,
                ..second.clone()
            },
        ],
    ] {
        assert!(matches!(
            FiberStreamState::validate_inventory(
                &program,
                AwbcFiberRoot::Function(AwbcFunctionId(0)),
                &streams,
                &mut ValueResourceAdmission::snapshot(
                    RuntimeSchemaLimits::engine_default(),
                    ValueAdmissionOrigin::InertSnapshot
                ),
                AwbcRuntimeValueSnapshot::view
            ),
            Err(FiberStateError::InvalidFrame)
        ));
    }
    let encoded = serde_json::to_value(&second).unwrap();
    assert_eq!(
        serde_json::from_value::<FiberStreamState<AwbcRuntimeValueSnapshot>>(encoded.clone())
            .unwrap(),
        second
    );
    let mut unknown = encoded;
    unknown
        .as_object_mut()
        .unwrap()
        .insert("hidden_owner".into(), serde_json::Value::Bool(true));
    assert!(serde_json::from_value::<FiberStreamState<AwbcRuntimeValueSnapshot>>(unknown).is_err());
}

#[test]
fn queues_share_one_bank_and_nested_affine_items_fail_while_inert() {
    let program = program(2);
    let streams = [
        queue(AwbcRuntimeValueSnapshot::String("cause".into())),
        FiberStreamState {
            plan: AwbcStreamPlanId(1),
            ..queue(AwbcRuntimeValueSnapshot::String("cause".into()))
        },
    ];
    let exact = RuntimeSchemaLimits {
        max_nodes: 2,
        max_encoded_bytes: 14,
        max_validation_work: 8,
        ..RuntimeSchemaLimits::engine_default()
    };
    for limits in [
        exact,
        RuntimeSchemaLimits {
            max_nodes: 1,
            ..exact
        },
        RuntimeSchemaLimits {
            max_encoded_bytes: 13,
            ..exact
        },
        RuntimeSchemaLimits {
            max_validation_work: 7,
            ..exact
        },
    ] {
        let result = FiberStreamState::validate_inventory(
            &program,
            AwbcFiberRoot::Function(AwbcFunctionId(0)),
            &streams,
            &mut ValueResourceAdmission::snapshot(limits, ValueAdmissionOrigin::InertSnapshot),
            AwbcRuntimeValueSnapshot::view,
        );
        assert_eq!(result.is_ok(), limits == exact);
    }
    let live = RuntimeValue::Tuple(vec![RuntimeValue::NeedHandle(crate::tests::reusable_need(
        "need.stream-custody",
    ))]);
    let inert = AwbcRuntimeValueSnapshot::from_standalone_value(&live).unwrap();
    for result in [
        RuntimeStreamYieldCopyProof::validate_inputs(
            live.view(),
            &mut ValueResourceAdmission::live(RuntimeSchemaLimits::engine_default()),
        ),
        RuntimeStreamYieldCopyProof::validate_inputs(
            inert.view(),
            &mut ValueResourceAdmission::snapshot(
                RuntimeSchemaLimits::engine_default(),
                ValueAdmissionOrigin::InertSnapshot,
            ),
        ),
    ] {
        assert_eq!(
            result,
            Err(crate::value::AwbcRuntimeValueSnapshotError::AffineValue)
        );
    }
}

#[test]
fn rejected_yields_preserve_queue_and_counter_before_copy() {
    let program = program(1);
    let mut stream = queue(RuntimeValue::String("cause".into()));
    assert_eq!(
        stream
            .prepare_yield(&program, &RuntimeValue::String("next".into()))
            .unwrap(),
        2
    );
    let before = stream.clone();
    assert!(
        stream
            .prepare_yield(&program, &RuntimeValue::Bool(true))
            .is_err()
    );
    assert_eq!(stream, before);
    for (closed, emitted_count) in [(true, 1), (false, u64::MAX), (false, 0)] {
        stream.closed = closed;
        stream.emitted_count = emitted_count;
        let before = stream.clone();
        assert!(
            stream
                .prepare_yield(&program, &RuntimeValue::String("next".into()))
                .is_err()
        );
        assert_eq!(stream, before);
    }
}
