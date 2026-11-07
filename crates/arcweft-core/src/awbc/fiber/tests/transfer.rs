use super::*;
use crate::value::{AwbcRuntimeValueSnapshotError, RuntimeRetainedValueAdmission};

fn pending() -> (FiberState, RuntimeProgramOwner) {
    let mut program = one_unit_parameter_entry_program();
    program.blocks[0].terminator = AwbcTerminator::GotoStatic {
        function: AwbcFunctionId(0),
        args: vec![AwbcRegisterId(0)],
    };
    let mut fiber = FiberState::for_entry(&program, AwbcEntryId(0), 0, 64).unwrap();
    fiber
        .bind_function_argument_values_owned(&program, vec![RuntimeValue::Unit])
        .unwrap();
    let prepared = fiber
        .prepare_root_replacement(&program, AwbcFunctionId(0), &[&RuntimeValue::Unit])
        .unwrap();
    let origin = FiberFlowTransferOrigin::Static {
        source: fiber.cursor,
    };
    fiber.stage_flow_transfer(prepared, vec![RuntimeValue::Unit], origin);
    (
        fiber,
        RuntimeProgramOwner::Awbc(std::sync::Arc::new(program)),
    )
}

#[test]
fn pending_flow_transfer_admits_original_arguments_before_constructing_destination_values() {
    let (fiber, owner) = pending();
    let image = AwbcFiberStateSnapshot::from_live(
        &fiber,
        &mut RuntimeRetainedValueAdmission::for_program(
            owner.clone(),
            crate::entry::RuntimeSchemaLimits::engine_default(),
        ),
    )
    .unwrap();
    let bytes = serde_json::to_vec(&image).unwrap();
    let roundtrip: AwbcFiberStateSnapshot = serde_json::from_slice(&bytes).unwrap();
    let restored = roundtrip.into_live_for_program(&owner).unwrap();
    let RuntimeProgramOwner::Awbc(program) = &owner else {
        unreachable!()
    };
    restored.validate_for_program(program).unwrap();
    assert_eq!(
        restored
            .pending_flow_transfer
            .as_ref()
            .unwrap()
            .destination
            .register(AwbcRegisterId(0))
            .unwrap(),
        &RuntimeValue::Unit
    );
    for mutation in 0..9 {
        let mut candidate = image.clone();
        let pending = candidate.pending_flow_transfer.as_mut().unwrap();
        match mutation {
            0 => pending.destination.function = AwbcFunctionId(99),
            1 => pending.destination.layout = AwbcFrameLayoutId(99),
            2 => pending.destination.instance = candidate.frames[0].instance,
            3 => pending.destination.registers[0] = AwbcRuntimeValueSnapshot::Bool(true).into(),
            4 => pending.destination.registers[0] = Default::default(),
            5 => {
                pending.destination.return_to = Some(FiberReturnPoint {
                    cursor: candidate.cursor,
                    destination: None,
                    continuation: FiberReturnContinuation::Ordinary,
                })
            }
            6 => {
                pending.origin = FiberFlowTransferOrigin::Static {
                    source: FiberCursor {
                        block: AwbcBlockId(99),
                        ..candidate.cursor
                    },
                }
            }
            7 => {
                pending.origin = FiberFlowTransferOrigin::Dynamic {
                    source: candidate.cursor,
                    selector: AwbcRuntimeValueSnapshot::Unit,
                    binding: None,
                }
            }
            8 => candidate.status = FiberStatus::Returned,
            _ => unreachable!(),
        }
        let mut admission = RuntimeRetainedValueAdmission::for_program(
            owner.clone(),
            crate::entry::RuntimeSchemaLimits::engine_default(),
        );
        let before = serde_json::to_vec(&candidate).unwrap();
        let failure = candidate
            .admit_retained(&mut admission)
            .expect_err("forged transfer is rejected");
        assert!(
            candidate.clone().into_live_for_program(&owner).is_err(),
            "mutation {mutation}"
        );
        assert_eq!(
            admission.validate(&RuntimeValue::Unit).unwrap_err(),
            failure
        );
        assert_eq!(serde_json::to_vec(&candidate).unwrap(), before);
    }
}

#[test]
fn pending_flow_transfer_spends_one_value_allowance_across_both_real_frames() {
    let (fiber, owner) = pending();
    for max_nodes in [2, 1] {
        let limits = crate::entry::RuntimeSchemaLimits {
            max_nodes,
            ..crate::entry::RuntimeSchemaLimits::engine_default()
        };
        let mut admission = RuntimeRetainedValueAdmission::for_program(owner.clone(), limits);
        let result = AwbcFiberStateSnapshot::from_live(&fiber, &mut admission);
        if max_nodes == 2 {
            let image = result.unwrap();
            let mut inert = RuntimeRetainedValueAdmission::for_program(owner.clone(), limits);
            image.admit_retained(&mut inert).unwrap();
            assert!(matches!(
                inert.validate(&RuntimeValue::Unit).unwrap_err(),
                AwbcRuntimeValueSnapshotError::Resource(_)
            ));
        } else {
            let error = result.unwrap_err();
            assert!(
                matches!(&error, AwbcRuntimeValueSnapshotError::ProgramType(cause)
                if matches!(cause.as_ref(), crate::program_types::RuntimeProgramTypeError::Resource(resource)
                    if matches!(resource.as_ref(), crate::entry::RuntimeSchemaError::BudgetExceeded { budget: "nodes" }))),
                "{error:?}"
            );
            assert_eq!(admission.validate(&RuntimeValue::Unit).unwrap_err(), error);
        }
    }
}

#[test]
fn pending_flow_transfer_survives_its_original_budget_yield() {
    let (mut fiber, owner) = pending();
    let RuntimeProgramOwner::Awbc(program) = &owner else {
        unreachable!()
    };
    fiber.budget.remaining = 0;
    let output = crate::awbc::vm::step(
        program,
        &mut fiber,
        crate::awbc::vm::VmStepOptions {
            max_instructions: 1,
        },
    )
    .unwrap();
    assert!(matches!(
        output.exit,
        crate::awbc::vm::VmExit::BudgetYield(_)
    ));
    let image = AwbcFiberStateSnapshot::from_live(
        &fiber,
        &mut RuntimeRetainedValueAdmission::for_program(
            owner.clone(),
            crate::entry::RuntimeSchemaLimits::engine_default(),
        ),
    )
    .unwrap();
    let restored = image.into_live_for_program(&owner).unwrap();
    assert_eq!(restored.status, FiberStatus::Suspended);
    assert!(restored.pending_flow_transfer.is_some());
    restored.validate_for_program(program).unwrap();
}

#[test]
fn goto_without_callbacks_commits_within_its_original_vm_operation() {
    let mut program = zero_parameter_entry_program();
    program.blocks[0].terminator = AwbcTerminator::GotoStatic {
        function: AwbcFunctionId(0),
        args: Vec::new(),
    };
    let mut fiber = FiberState::for_entry(&program, AwbcEntryId(0), 0, 64).unwrap();
    let original = fiber.active_frame().unwrap().instance;
    let output = crate::awbc::vm::step(
        &program,
        &mut fiber,
        crate::awbc::vm::VmStepOptions {
            max_instructions: 1,
        },
    )
    .unwrap();
    assert_eq!(output.executed, 1);
    assert_eq!(output.exit, crate::awbc::vm::VmExit::Running);
    assert_ne!(fiber.active_frame().unwrap().instance, original);
    assert_eq!(fiber.frames.len(), 1);
    assert!(fiber.pending_flow_transfer.is_none());
    assert_eq!(
        output
            .observations
            .iter()
            .filter(|observation| matches!(
                observation,
                crate::awbc::vm::VmObservation::Goto(AwbcFunctionId(0))
            ))
            .count(),
        1
    );
}
