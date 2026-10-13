use std::sync::Arc;

use super::*;
use crate::awbc::schema::{AwbcProgram, AwbcRuntimeType, AwbcRuntimeTypeShape, AwbcTypeId};
use crate::entry::RuntimeSchemaError;
use crate::entry::schema::value_encoding::ValueAdmissionOrigin;
use crate::plan::{
    RuntimeCallableParameterCoordinate, RuntimeCallableParameterInput, RuntimeCallablePosition,
    RuntimeCallableRetainedInput, RuntimeCallableRetainedRole,
};
use crate::value::{AwbcRuntimeCallableSnapshot, AwbcRuntimeValueSnapshot};

fn pending_owner() -> RuntimeProgramOwner {
    let boolean = AwbcTypeId(0);
    let function = AwbcTypeId(1);
    let state = RuntimeCallableStateId::from_zero_based(0).unwrap();
    let program = AwbcProgram {
        runtime_types: vec![
            AwbcRuntimeType::new(
                crate::pattern::RuntimeCheckedType::Bool.semantic_identity_digest(),
                AwbcRuntimeTypeShape::Bool,
            ),
            AwbcRuntimeType::new(
                RuntimeSemanticTypeId::from_bytes([0x95; 32]),
                AwbcRuntimeTypeShape::Function {
                    contract: crate::plan::RuntimeFunctionTypeContract::default(),
                    parameters: vec![boolean],
                    result: boolean,
                },
            ),
        ],
        callable_states: vec![RuntimeCallableStateDefinition {
            function_type: function,
            origin: state,
            position: RuntimeCallablePosition::Unapplied,
            retained: Box::new([
                RuntimeCallableRetainedInput {
                    role: RuntimeCallableRetainedRole::Capture { position: 0 },
                    ty: boolean,
                },
                RuntimeCallableRetainedInput {
                    role: RuntimeCallableRetainedRole::Capture { position: 1 },
                    ty: boolean,
                },
            ]),
            parameters: Box::new([RuntimeCallableParameterInput {
                coordinate: RuntimeCallableParameterCoordinate {
                    group: 0,
                    parameter: 0,
                },
                kind: RuntimeCallableParameterKind::Fixed,
                abi_ty: boolean,
                binding_ty: boolean,
            }]),
            result: boolean,
            attached: RuntimeCallableAttachedContract::Defaulted {
                ty: boolean,
                default: crate::plan::RuntimeCallableDefault::Body {
                    function: AwbcFunctionId(0),
                    captures: Box::new([
                        RuntimeCallableInputSource::Retained { position: 0 },
                        RuntimeCallableInputSource::Argument { position: 0 },
                    ]),
                },
            },
            transition: RuntimeCallableTransition::Invoke {
                function: AwbcFunctionId(1),
                captures: Box::new([
                    RuntimeCallableInputSource::Retained { position: 0 },
                    RuntimeCallableInputSource::Retained { position: 1 },
                ]),
                arguments: Box::new([RuntimeCallableInputSource::Attached]),
            },
            partials: Box::new([]),
        }],
        ..AwbcProgram::default()
    };
    RuntimeProgramOwner::Awbc(Arc::new(program))
}

fn packet() -> RuntimeCallablePendingSnapshot {
    RuntimeCallablePendingSnapshot {
        state: RuntimeCallableStateId::from_zero_based(0).unwrap(),
        retained: vec![
            Some(AwbcRuntimeValueSnapshot::Bool(true)),
            Some(AwbcRuntimeValueSnapshot::Bool(false)),
        ],
        arguments: vec![None],
        attached: None,
    }
}

#[test]
fn pending_round_trip_preserves_moved_slots_and_the_once_only_default_transition() {
    let owner = pending_owner();
    let snapshot = packet();
    let bytes = serde_json::to_vec(&snapshot).unwrap();
    let decoded: RuntimeCallablePendingSnapshot = serde_json::from_slice(&bytes).unwrap();
    let live = decoded.into_live(&owner).unwrap();
    assert_eq!(live.arguments(), [None]);
    assert_eq!(
        RuntimeCallablePendingSnapshot::from_live(&live, &owner).unwrap(),
        snapshot
    );
    let RuntimeCallableApplication::Invoke(invocation) =
        live.complete_default(RuntimeValue::Bool(true)).unwrap()
    else {
        panic!("restored default resumes the target exactly once");
    };
    assert!(matches!(
        invocation.body,
        RuntimeCallableBodyReference::Awbc(AwbcFunctionId(1))
    ));
    assert_eq!(
        invocation.captures,
        [RuntimeValue::Bool(true), RuntimeValue::Bool(false)]
    );
    assert_eq!(invocation.arguments, [RuntimeValue::Bool(true)]);
}

#[test]
fn pending_presence_mask_is_checked_before_any_input_constructor() {
    let owner = pending_owner();
    let mut snapshot = packet();
    snapshot.retained[0] = Some(AwbcRuntimeValueSnapshot::Callable(
        AwbcRuntimeCallableSnapshot {
            state: RuntimeCallableStateId::from_zero_based(999).unwrap(),
            retained: vec![],
        },
    ));
    snapshot.arguments[0] = Some(AwbcRuntimeValueSnapshot::Unit);
    let error = snapshot.into_live(&owner).unwrap_err();
    assert!(
        matches!(error, crate::value::AwbcRuntimeValueSnapshotError::Message { message }
        if message.contains("input projection"))
    );
}

#[test]
fn pending_expected_row_rejects_wrong_input_before_constructing_callable() {
    let owner = pending_owner();
    let mut snapshot = packet();
    snapshot.retained[0] = Some(AwbcRuntimeValueSnapshot::Callable(
        AwbcRuntimeCallableSnapshot {
            state: RuntimeCallableStateId::from_zero_based(999).unwrap(),
            retained: vec![],
        },
    ));
    let error = snapshot.into_live(&owner).unwrap_err();
    assert!(
        matches!(error, crate::value::AwbcRuntimeValueSnapshotError::ProgramType(error)
        if matches!(*error, crate::program_types::RuntimeProgramTypeError::AwbcValue(_)))
    );
    let live = packet().into_live(&owner).unwrap();
    let foreign = pending_owner();
    assert!(
        matches!(RuntimeCallablePendingSnapshot::from_live(&live, &foreign),
        Err(crate::value::AwbcRuntimeValueSnapshotError::Message { message })
        if message.contains("another executable"))
    );
}

#[test]
fn pending_values_share_exact_node_byte_and_work_allowances_for_live_and_inert() {
    let owner = pending_owner();
    let snapshot = packet();
    let live = snapshot.clone().into_live(&owner).unwrap();
    let limits = RuntimeSchemaLimits::engine_default();
    let exact = RuntimeSchemaLimits {
        max_nodes: 2,
        max_encoded_bytes: 4,
        // Header + five projection edges + three slots + four value visits.
        max_validation_work: 13,
        ..limits
    };
    for origin in [
        ValueAdmissionOrigin::Live,
        ValueAdmissionOrigin::InertSnapshot,
    ] {
        let validate = |limits| {
            let mut resources = ValueResourceAdmission::snapshot(limits, origin);
            match origin {
                ValueAdmissionOrigin::Live => live.validate_snapshot_inputs(&owner, &mut resources),
                ValueAdmissionOrigin::InertSnapshot => {
                    snapshot.validate_inputs(&owner, &mut resources)
                }
            }
        };
        validate(exact).unwrap();
        for short in [
            RuntimeSchemaLimits {
                max_nodes: 1,
                ..exact
            },
            RuntimeSchemaLimits {
                max_encoded_bytes: 3,
                ..exact
            },
            RuntimeSchemaLimits {
                max_validation_work: 12,
                ..exact
            },
        ] {
            assert!(
                validate(short).is_err(),
                "both retained inputs share one allowance"
            );
        }
        let mut resources = ValueResourceAdmission::snapshot(exact, origin);
        match origin {
            ValueAdmissionOrigin::Live => live
                .validate_snapshot_inputs(&owner, &mut resources)
                .unwrap(),
            ValueAdmissionOrigin::InertSnapshot => {
                snapshot.validate_inputs(&owner, &mut resources).unwrap();
            }
        }
        assert!(matches!(snapshot.validate_inputs(&owner, &mut resources),
            Err(RuntimeCallableValueError::PendingResource { source, .. })
            if matches!(*source, RuntimeSchemaError::ValidationWork { limit: 13, consumed: 14, .. })));
    }
}

#[test]
fn pending_empty_slots_and_projection_edges_consume_work_without_inventing_values() {
    let RuntimeProgramOwner::Awbc(program) = pending_owner() else {
        unreachable!()
    };
    let mut program = (*program).clone();
    let definition = &mut program.callable_states[0];
    let RuntimeCallableAttachedContract::Defaulted {
        default: crate::plan::RuntimeCallableDefault::Body { captures, .. },
        ..
    } = &mut definition.attached
    else {
        unreachable!()
    };
    *captures = Box::new([
        RuntimeCallableInputSource::Retained { position: 0 },
        RuntimeCallableInputSource::Retained { position: 1 },
        RuntimeCallableInputSource::Argument { position: 0 },
    ]);
    definition.transition = RuntimeCallableTransition::Invoke {
        function: AwbcFunctionId(1),
        captures: Box::new([]),
        arguments: Box::new([RuntimeCallableInputSource::Attached]),
    };
    let owner = RuntimeProgramOwner::Awbc(Arc::new(program));
    let mut snapshot = packet();
    snapshot.retained = vec![None, None];
    let before = snapshot.clone();
    // Header + four projection edges + three empty slots, with no value nodes.
    let exact = RuntimeSchemaLimits {
        max_validation_work: 8,
        max_nodes: 0,
        max_encoded_bytes: 0,
        ..RuntimeSchemaLimits::engine_default()
    };
    snapshot
        .validate_inputs(
            &owner,
            &mut ValueResourceAdmission::snapshot(exact, ValueAdmissionOrigin::InertSnapshot),
        )
        .unwrap();
    let error = snapshot
        .validate_inputs(
            &owner,
            &mut ValueResourceAdmission::snapshot(
                RuntimeSchemaLimits {
                    max_validation_work: 7,
                    ..exact
                },
                ValueAdmissionOrigin::InertSnapshot,
            ),
        )
        .unwrap_err();
    let RuntimeCallableValueError::PendingResource { source, .. } = error else {
        panic!("empty slots must retain the structured resource rejection");
    };
    assert!(matches!(
        *source,
        RuntimeSchemaError::ValidationWork {
            limit: 7,
            consumed: 8,
            ..
        }
    ));
    assert_eq!(snapshot, before);
    let live = snapshot.into_live(&owner).unwrap();
    assert_eq!(live.retained(), [None, None]);
    assert_eq!(live.arguments(), [None]);
    assert_eq!(
        RuntimeCallablePendingSnapshot::from_live(&live, &owner).unwrap(),
        before
    );
}

#[test]
fn pending_known_type_row_cannot_redirect_through_a_duplicate_semantic_identity() {
    let RuntimeProgramOwner::Awbc(owner) = pending_owner() else {
        unreachable!()
    };
    let mut program = (*owner).clone();
    // An unsealed inventory deliberately repeats the semantic identity. The
    // original state selects row 2, whose actual shape is String, not row 0.
    program.runtime_types.push(AwbcRuntimeType::new(
        program.runtime_types[0].semantic_identity(),
        AwbcRuntimeTypeShape::String,
    ));
    program.callable_states[0].retained[1].ty = AwbcTypeId(2);
    let owner = RuntimeProgramOwner::Awbc(Arc::new(program));
    let error = packet().into_live(&owner).unwrap_err();
    let crate::value::AwbcRuntimeValueSnapshotError::ProgramType(error) = error else {
        panic!("the original row must report a structured type error");
    };
    let crate::program_types::RuntimeProgramTypeError::AwbcValue(error) = *error else {
        panic!("the original AWBC row must own the value rejection");
    };
    assert!(matches!(
        *error,
        crate::awbc::value_admission::AwbcValueAdmissionError::Value {
            ty: AwbcTypeId(2),
            ..
        }
    ));
    let pending = RuntimeCallablePendingGroup {
        owner: owner.clone(),
        state: packet().state,
        retained: vec![
            Some(RuntimeValue::Bool(true)),
            Some(RuntimeValue::Bool(false)),
        ],
        arguments: vec![None],
        attached: None,
    };
    assert!(matches!(
        RuntimeCallablePendingSnapshot::from_live(&pending, &owner),
        Err(crate::value::AwbcRuntimeValueSnapshotError::ProgramType(_))
    ));
}
