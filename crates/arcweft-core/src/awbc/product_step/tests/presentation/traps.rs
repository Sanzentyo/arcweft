use super::*;
use crate::awbc::fiber::{FiberStateError, FiberTerminalValue};
use crate::awbc::product_step::checkpoint::ProductCheckpointTarget;
use crate::awbc::schema::*;
use crate::awbc::vm::{VmError, VmStepOptions};
use crate::presentation::RuntimePresentationError;
use crate::value::AwbcRuntimeValueSnapshotError;

fn bind_body(
    executor: &mut AwbcProductStepExecutor,
    value: bool,
) -> RuntimePresentationHandleToken {
    let owner = crate::task::RuntimeProgramOwner::Awbc(executor.program_arc());
    let ledger = executor
        .presentation
        .as_mut()
        .unwrap()
        .ledger_mut(RuntimePresentationBankId::Root)
        .unwrap();
    let scope = executor
        .fiber
        .bind_presentation_scope(&owner, ledger)
        .unwrap();
    ledger
        .register(
            &owner,
            scope,
            PresentationRegistrationSite::Independent(PresentationResourceKind::Image),
            PresentationBody {
                expected: crate::pattern::RuntimeCheckedType::Bool.semantic_identity_digest(),
                binding: None,
                value: RuntimePayload(RuntimeValue::Bool(value)),
            },
        )
        .unwrap()
        .0
}

fn one_op() -> RuntimeStepOptions {
    RuntimeStepOptions {
        budget: crate::step::RuntimeStepBudget { max_ops: 1 },
        ..Default::default()
    }
}

fn advance(executor: &mut AwbcProductStepExecutor) {
    let step = executor.step(Default::default(), one_op());
    assert!(
        step.output.diagnostics.is_empty(),
        "{:?}",
        step.output.diagnostics
    );
}

pub(super) fn nested_program() -> AwbcProgram {
    let mut program = trap_program(AwbcTrapCode::ExplicitPanic, "nested presentation trap");
    program.runtime_types.push(AwbcRuntimeType::new(
        crate::pattern::RuntimeCheckedType::Bool.semantic_identity_digest(),
        AwbcRuntimeTypeShape::Bool,
    ));
    program.frame_layouts.push(AwbcFrameLayout {
        scopes: vec![AwbcScopeDefinition {
            kind: crate::scope::RuntimeScopeFrameKind::EmittedLexical,
            parent: None,
            identity: crate::scope::RuntimeScopeIdentity::Anonymous,
        }],
        slots: vec![],
        max_scope_depth: 1,
    });
    let mut callee = program.functions[0].clone();
    callee.definition = crate::plan::RuntimeFunctionDefinitionIdentity::from_accepted_identity(
        *blake3::hash(b"awbc.presentation.trap.callee.v1\0").as_bytes(),
    );
    callee.public_id = None;
    callee.kind = AwbcFunctionKind::Ordinary;
    callee.frame_layout = AwbcFrameLayoutId(1);
    callee.blocks = AwbcTableRange::new(2, 1);
    callee.entry_block = AwbcBlockId(2);
    program.functions.push(callee);
    program.functions[0].blocks = AwbcTableRange::new(0, 2);
    let mut trap_block = program.blocks[0].clone();
    trap_block.owner = AwbcFunctionId(1);
    trap_block.instructions = AwbcTableRange::new(0, 1);
    trap_block.safe_point = AwbcSafePointKind::CallableBoundary;
    program.blocks[0].terminator = AwbcTerminator::CallFunction {
        function: AwbcFunctionId(1),
        args: vec![],
        dst: None,
        resume: AwbcResumePointId(0),
    };
    program.blocks.push(AwbcBlock {
        owner: AwbcFunctionId(0),
        instructions: AwbcTableRange::new(0, 0),
        terminator: AwbcTerminator::Return { value: None },
        safe_point: AwbcSafePointKind::Return,
        source_map: None,
    });
    program.blocks.push(trap_block);
    program.instructions.push(AwbcInstruction::EnterScope {
        scope: AwbcScopeId(0),
    });
    program.resume_points.push(AwbcResumePoint {
        function: AwbcFunctionId(0),
        block: AwbcBlockId(1),
        frame_layout: AwbcFrameLayoutId(0),
        kind: AwbcSafePointKind::CallableBoundary,
    });
    program
}

pub(super) fn nested_fixture(
    queued: bool,
) -> (AwbcProductStepExecutor, Vec<RuntimePresentationHandleToken>) {
    let program = nested_program();
    let mut executor = AwbcProductStepExecutor::for_entry(program, AwbcEntryId(0), 64).unwrap();
    let owner = crate::task::RuntimeProgramOwner::Awbc(executor.program_arc());
    executor.presentation = Some(PresentationBanks::new(
        owner.clone(),
        executor.artifact_fingerprint,
        if queued {
            PresentationLedgerLimits {
                handles: 3,
                scopes: 8,
            }
        } else {
            PresentationLedgerLimits::for_schema(crate::entry::RuntimeSchemaLimits::engine_default())
        },
    ));
    if queued {
        bind_body(&mut executor, false);
        let retired = executor
            .fiber
            .release_presentation_scope(
                &owner,
                executor
                    .presentation
                    .as_mut()
                    .unwrap()
                    .ledger_mut(RuntimePresentationBankId::Root)
                    .unwrap(),
            )
            .unwrap();
        executor
            .presentation
            .as_mut()
            .unwrap()
            .retirements_mut()
            .extend(retired);
    }
    let caller = bind_body(&mut executor, true);
    advance(&mut executor); // The real call allocates the callee's frame identity.
    assert_eq!(executor.fiber.frames.len(), 2);
    let callee = bind_body(&mut executor, false);
    advance(&mut executor); // The real EnterScope allocates lexical custody.
    assert_eq!(executor.fiber.active_frame().unwrap().scopes.len(), 1);
    let lexical = bind_body(&mut executor, true);
    executor.snapshot().unwrap();
    (executor, vec![lexical, callee, caller])
}

#[test]
fn trap_unwinds_actual_callee_lexical_and_caller_scopes_and_checkpoint_restores_all() {
    let (mut executor, tokens) = nested_fixture(false);
    let before = executor.snapshot().unwrap();
    let checkpoint = executor
        .execution_checkpoint(ProductCheckpointTarget::Main)
        .unwrap();
    executor.step(Default::default(), one_op());
    assert_eq!(executor.fiber.status, FiberStatus::Trapped);
    let Some(FiberTerminalValue::Trapped(trap)) = &executor.fiber.terminal else {
        panic!("the actual trap must survive resource retirement")
    };
    assert_eq!(trap.code, AwbcTrapCode::ExplicitPanic);
    assert_eq!(trap.message.as_deref(), Some("nested presentation trap"));
    let retired = executor.presentation.as_ref().unwrap().retirements();
    assert_eq!(
        retired
            .iter()
            .map(|item| item.token.clone())
            .collect::<Vec<_>>(),
        tokens
    );
    assert_eq!(
        retired
            .iter()
            .map(|item| &item.body.value.0)
            .collect::<Vec<_>>(),
        vec![
            &RuntimeValue::Bool(true),
            &RuntimeValue::Bool(false),
            &RuntimeValue::Bool(true)
        ]
    );
    assert!(
        executor
            .fiber
            .presentation_unwind_scopes()
            .unwrap()
            .is_empty()
    );
    let after = executor.snapshot().unwrap();
    let save = AwbcProductExecutorSaveSnapshot::from_live(&after, &executor.program).unwrap();
    let wire = serde_json::to_vec(&save).unwrap();
    executor = executor
        .restore_inert_snapshot_owned(serde_json::from_slice(&wire).unwrap())
        .unwrap();
    assert_eq!(executor.snapshot().unwrap(), after);
    checkpoint
        .restore(
            &mut executor.fiber,
            &mut executor.presentation,
            &executor.program,
        )
        .unwrap();
    assert_eq!(executor.snapshot().unwrap(), before);
}

#[test]
fn product_trap_capacity_refusal_preserves_the_complete_continuation_and_retries() {
    let (mut executor, tokens) = nested_fixture(true);
    let before = executor.snapshot().unwrap();
    for _ in 0..2 {
        let output = executor.step(Default::default(), one_op());
        assert_eq!(output.output.diagnostics.len(), 1);
        assert!(
            output.output.diagnostics[0]
                .message
                .contains(&RuntimePresentationError::RetirementCapacity.to_string())
        );
        assert_eq!(
            executor.snapshot().unwrap(),
            before,
            "a refused trap retains budget, cursor, status, all grants, bodies and the old retirement"
        );
    }
    // Model acknowledgement of the old retirement; neither policy nor live
    // custody changes before retrying the identical trapped instruction.
    executor
        .presentation
        .as_mut()
        .unwrap()
        .retirements_mut()
        .clear();
    executor.step(Default::default(), one_op());
    assert_eq!(executor.fiber.status, FiberStatus::Trapped);
    assert_eq!(
        executor
            .presentation
            .as_ref()
            .unwrap()
            .retirements()
            .iter()
            .map(|item| item.token.clone())
            .collect::<Vec<_>>(),
        tokens
    );
}

#[test]
fn standalone_trap_refuses_real_grants_without_a_borrowed_ledger() {
    let (mut executor, _) = nested_fixture(false);
    let before = executor.snapshot().unwrap();
    let checkpoint = executor
        .execution_checkpoint(ProductCheckpointTarget::Main)
        .unwrap();
    let error = crate::awbc::vm::step(
        &executor.program,
        &mut executor.fiber,
        VmStepOptions {
            max_instructions: 1,
        },
    )
    .unwrap_err();
    assert!(matches!(
        error,
        VmError::Fiber(FiberStateError::Snapshot(
            AwbcRuntimeValueSnapshotError::Presentation(RuntimePresentationError::MissingLedger)
        ))
    ));
    assert_eq!(executor.fiber.status, FiberStatus::Running);
    assert_eq!(
        executor.fiber.presentation_unwind_scopes().unwrap().len(),
        3
    );
    assert!(
        executor
            .presentation
            .as_ref()
            .unwrap()
            .retirements()
            .is_empty()
    );
    // Public standalone stepping does not own a Product transaction; the
    // original complete checkpoint also restores its consumed instruction budget.
    checkpoint
        .restore(
            &mut executor.fiber,
            &mut executor.presentation,
            &executor.program,
        )
        .unwrap();
    assert_eq!(executor.snapshot().unwrap(), before);
}

#[test]
fn every_vm_trap_route_retires_its_original_bank_body() {
    for route in 0..4 {
        let mut program = trap_program(AwbcTrapCode::ExplicitPanic, "explicit trap");
        let expected = match route {
            0 => AwbcTrapCode::ExplicitPanic,
            1 => {
                program.blocks[0].terminator = AwbcTerminator::Unreachable;
                AwbcTrapCode::InternalInvariant
            }
            _ => {
                let unsigned = crate::pattern::RuntimeCheckedType::Unsigned(
                    crate::value::RuntimeUnsignedIntWidth::U64,
                );
                let scalar = AwbcTypeId(program.runtime_types.len().try_into().unwrap());
                program.runtime_types.push(AwbcRuntimeType::new(
                    unsigned.semantic_identity_digest(),
                    AwbcRuntimeTypeShape::UInt(AwbcUnsignedIntKind::U64),
                ));
                let sequence = AwbcTypeId(program.runtime_types.len().try_into().unwrap());
                program.runtime_types.push(AwbcRuntimeType::new(
                    crate::pattern::RuntimeCheckedType::Sequence(Box::new(unsigned))
                        .semantic_identity_digest(),
                    AwbcRuntimeTypeShape::Sequence {
                        kind: crate::plan::RuntimePlanSequenceKind::Seq,
                        item: scalar,
                    },
                ));
                program.frame_layouts[0].slots = (0..3)
                    .map(|index| AwbcFrameSlot {
                        name: None,
                        ty: if route == 2 && index == 0 {
                            sequence
                        } else {
                            scalar
                        },
                        role: AwbcFrameSlotRole::Temporary,
                        scope_depth: 0,
                    })
                    .collect();
                program.constants = vec![
                    if route == 2 {
                        AwbcConstant::Sequence(vec![])
                    } else {
                        AwbcConstant::UInt {
                            kind: AwbcUnsignedIntKind::U64,
                            bits: 1_u128.to_le_bytes(),
                        }
                    },
                    AwbcConstant::UInt {
                        kind: AwbcUnsignedIntKind::U64,
                        bits: 0_u128.to_le_bytes(),
                    },
                ];
                program.instructions = vec![
                    AwbcInstruction::LoadConst {
                        dst: AwbcRegisterId(0),
                        constant: AwbcConstantId(0),
                    },
                    AwbcInstruction::LoadConst {
                        dst: AwbcRegisterId(1),
                        constant: AwbcConstantId(1),
                    },
                    if route == 2 {
                        AwbcInstruction::SequenceGet {
                            dst: AwbcRegisterId(2),
                            sequence: AwbcRegisterId(0),
                            index: AwbcRegisterId(1),
                        }
                    } else {
                        AwbcInstruction::Binary {
                            dst: AwbcRegisterId(2),
                            op: AwbcBinaryOp::Div,
                            lhs: AwbcRegisterId(0),
                            rhs: AwbcRegisterId(1),
                        }
                    },
                ];
                program.blocks[0].instructions = AwbcTableRange::new(0, 3);
                program.blocks[0].terminator = AwbcTerminator::Unreachable;
                if route == 2 {
                    AwbcTrapCode::InvalidIndex
                } else {
                    AwbcTrapCode::DivisionByZero
                }
            }
        };
        let (mut executor, token) = fixture_with_program(program);
        for _ in 0..4 {
            if executor.fiber.status == FiberStatus::Trapped {
                break;
            }
            executor.step(Default::default(), one_op());
        }
        let Some(FiberTerminalValue::Trapped(trap)) = &executor.fiber.terminal else {
            panic!("trap route {route} did not terminate")
        };
        assert_eq!(trap.code, expected, "route {route}");
        let retired = executor.presentation.as_ref().unwrap().retirements();
        assert_eq!(retired.len(), 1, "route {route}");
        assert_eq!(retired[0].token, token, "route {route}");
        assert_eq!(retired[0].body.value.0, RuntimeValue::Bool(true));
        assert!(
            executor
                .fiber
                .presentation_unwind_scopes()
                .unwrap()
                .is_empty()
        );
        executor.snapshot().unwrap();
    }
}
