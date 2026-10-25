use super::*;
use crate::awbc::schema::*;
use crate::awbc::verify::{AwbcVerifyBudget, AwbcVerifyContext};
use crate::plan::task_source::RuntimeTaskSource;
use crate::plan::{RuntimeTaskPlanIndex, RuntimeTaskSourceArtifactLimits};
use std::sync::Arc;

fn meter() -> TaskSemanticMeter {
    TaskSemanticMeter::new(1_000_000, 1_000_000)
}

fn context() -> AwbcVerifyContext<'static> {
    AwbcVerifyContext {
        require_entrypoint: false,
        ..AwbcVerifyContext::default()
    }
}

pub(crate) fn executable() -> AwbcProgram {
    let original = crate::plan::task_source::template_tests::thread();
    executable_from(original)
}

fn executable_from(original: RuntimeTaskSourceProgram) -> AwbcProgram {
    let original = original.original();
    let row = original
        .task_plans()
        .by_index(RuntimeTaskPlanIndex(0))
        .unwrap();
    let source = original
        .task_source_by_expected_key(
            &row.key().into(),
            RuntimeTaskPlanSealLimits::default(),
            &mut meter(),
        )
        .unwrap();
    let Some(FlowOp::Thread {
        producer, captures, ..
    }) = source.request_endpoint().operation()
    else {
        panic!("fixture retains its original Thread")
    };
    let declaration = original.local_declarations().get(captures[0]).unwrap();
    let unit = crate::pattern::RuntimeCheckedType::Unit.semantic_identity_digest();
    let boolean = crate::pattern::RuntimeCheckedType::Bool.semantic_identity_digest();
    AwbcProgram {
        original_source: Some(
            original
                .task_source_artifact(RuntimeTaskSourceArtifactLimits::default())
                .unwrap(),
        ),
        strings: vec![producer.cancel_scope.0.clone(), "wrong".into()],
        runtime_types: vec![
            AwbcRuntimeType::new(unit, AwbcRuntimeTypeShape::Unit),
            AwbcRuntimeType::new(boolean, AwbcRuntimeTypeShape::Bool),
        ],
        effect_sets: vec![AwbcEffectSet::default()],
        signatures: vec![AwbcSignature {
            params: vec![AwbcTypeId(1)],
            result: Some(AwbcTypeId(0)),
            effects: AwbcEffectSetId(0),
        }],
        frame_layouts: vec![AwbcFrameLayout {
            slots: vec![
                AwbcFrameSlot {
                    name: None,
                    ty: AwbcTypeId(1),
                    role: AwbcFrameSlotRole::Parameter,
                    scope_depth: 0,
                },
                AwbcFrameSlot {
                    name: None,
                    ty: AwbcTypeId(0),
                    role: AwbcFrameSlotRole::Temporary,
                    scope_depth: 0,
                },
            ],
            scopes: vec![],
            max_scope_depth: 0,
        }],
        functions: vec![AwbcFunction {
            definition: producer
                .function
                .generated_child(RuntimeGeneratedFunctionRole::TaskBody),
            semantic_role: RuntimeFunctionSemanticRole::Ordinary,
            public_id: None,
            kind: AwbcFunctionKind::ProgramTask,
            signature: AwbcSignatureId(0),
            type_context: None,
            input_ownership: vec![AwbcFunctionInputOwnership::owned(
                declaration.origin(),
                RuntimeFunctionInputSource::Capture { position: 0 },
                RuntimeFunctionInputTransfer::ExternalBinding,
            )],
            frame_layout: AwbcFrameLayoutId(0),
            blocks: AwbcTableRange::new(0, 1),
            entry_block: AwbcBlockId(0),
            flags: AwbcFunctionFlags::empty()
                .with(AwbcFunctionFlag::Deterministic)
                .with(AwbcFunctionFlag::MayAllocate)
                .with(AwbcFunctionFlag::MaySuspend),
        }],
        blocks: vec![AwbcBlock {
            owner: AwbcFunctionId(0),
            instructions: AwbcTableRange::new(0, 1),
            terminator: AwbcTerminator::Return {
                value: Some(AwbcRegisterId(1)),
            },
            safe_point: AwbcSafePointKind::CallableBoundary,
            source_map: None,
        }],
        constants: vec![AwbcConstant::Unit],
        instructions: vec![AwbcInstruction::LoadConst {
            dst: AwbcRegisterId(1),
            constant: AwbcConstantId(0),
        }],
        task_plans: vec![AwbcTaskPlan {
            class: AwbcTaskClass::Cpu,
            priority: producer.priority.0,
            cancel_scope: AwbcStringId(0),
            policy: AwbcTaskPolicy::AlwaysStart,
            handle_type: producer.handle_type.map_payload(|_| AwbcTypeId(0)),
            execution: AwbcTaskPlanExecution::Program(AwbcProgramTaskPlan {
                source: row.key().into(),
                kernel: AwbcFunctionId(0),
            }),
        }],
        ..AwbcProgram::default()
    }
}

fn admit(code: AwbcProgram) -> RuntimeTaskSourceProgram {
    RuntimeTaskSourceProgram::admit_awbc(
        Arc::new(code),
        AwbcVerifyBudget::default(),
        context(),
        RuntimeTaskSourceArtifactLimits::default(),
        None,
    )
    .unwrap()
}

#[test]
fn program_destination_verifier_checks_the_row_carrier_and_payload() {
    use crate::task::TaskHandleType;
    let mut code = executable();
    let mut caller = code.functions[0].clone();
    caller.definition =
        crate::plan::RuntimeFunctionDefinitionIdentity::from_accepted_identity([0x63; 32]);
    caller.kind = AwbcFunctionKind::Ordinary;
    caller.signature = AwbcSignatureId(1);
    caller.frame_layout = AwbcFrameLayoutId(1);
    caller.blocks = AwbcTableRange::new(1, 1);
    caller.entry_block = AwbcBlockId(1);
    code.functions.push(caller);
    let mut signature = code.signatures[0].clone();
    signature.result = None;
    code.signatures.push(signature);
    let mut frame = code.frame_layouts[0].clone();
    frame.slots[1].ty = AwbcTypeId(2);
    code.frame_layouts.push(frame);
    code.instructions.push(AwbcInstruction::StartProgram {
        dst: AwbcRegisterId(1),
        plan: AwbcTaskPlanId(0),
        args: vec![AwbcRegisterId(0)],
    });
    code.blocks.push(AwbcBlock {
        owner: AwbcFunctionId(1),
        instructions: AwbcTableRange::new(1, 1),
        terminator: AwbcTerminator::Return { value: None },
        safe_point: AwbcSafePointKind::CallableBoundary,
        source_map: None,
    });
    code.runtime_types.push(AwbcRuntimeType::new(
        crate::pattern::RuntimeSemanticTypeId::from_bytes([0x64; 32]),
        AwbcRuntimeTypeShape::Task(AwbcTypeId(0)),
    ));
    for handle_type in [
        TaskHandleType::Need {
            payload: AwbcTypeId(0),
        },
        TaskHandleType::ThreadHandle {
            payload: AwbcTypeId(0),
        },
    ] {
        code.task_plans[0].handle_type = handle_type;
        for (destination, expected) in [
            (
                AwbcRuntimeTypeShape::Need(AwbcTypeId(0)),
                matches!(handle_type, TaskHandleType::Need { .. }),
            ),
            (
                AwbcRuntimeTypeShape::Task(AwbcTypeId(0)),
                matches!(handle_type, TaskHandleType::ThreadHandle { .. }),
            ),
            (AwbcRuntimeTypeShape::Need(AwbcTypeId(1)), false),
            (AwbcRuntimeTypeShape::Task(AwbcTypeId(1)), false),
        ] {
            code.runtime_types[2] = AwbcRuntimeType::new(
                crate::pattern::RuntimeSemanticTypeId::from_bytes([0x64; 32]),
                destination,
            );
            let result = code.verify(AwbcVerifyBudget::default(), context());
            if expected {
                result.unwrap();
            } else {
                assert!(
                    matches!(
                        result.as_ref(),
                        Err(crate::awbc::verify::AwbcVerifyError::InvalidInvariant { message, .. })
                            if message == "Program destination matching task carrier and payload"
                    ),
                    "{handle_type:?}: {result:?}"
                );
            }
        }
    }
}

#[test]
fn program_kernel_source_round_trips_without_a_host_request_and_reborrows_its_actual_row() {
    let code = executable();
    let bytes = code.encode_canonical().unwrap();
    let decoded =
        AwbcProgram::decode_canonical(&bytes, crate::awbc::codec::AwbcDecodeBudget::default())
            .unwrap();
    assert_eq!(decoded, code);
    assert!(decoded.task_plans[0].host().is_none());
    let program = admit(decoded);
    let source = program
        .awbc_program_source(
            AwbcTaskPlanId(0),
            RuntimeTaskPlanSealLimits::default(),
            &mut meter(),
        )
        .unwrap();
    let owner = program.program_owner();
    let retained = RuntimeTaskSource::from_borrowed(&source, &program, &owner).unwrap();
    assert!(matches!(
        retained.assertion().execution,
        crate::plan::task_source::owned::RuntimeTaskSourceExecutionAssertion::Awbc(
            RuntimeTaskExecutionRow::Program(AwbcTaskPlanId(0))
        )
    ));
    let borrowed = retained
        .borrow(RuntimeTaskPlanSealLimits::default(), &mut meter())
        .unwrap();
    assert_eq!(borrowed.task_plan().key(), source.task_plan().key());
    let assertion = retained.assertion();
    let rebound = program
        .borrow_assertion(
            &assertion,
            RuntimeTaskPlanSealLimits::default(),
            &mut meter(),
        )
        .unwrap();
    assert_eq!(rebound.task_plan().key(), source.task_plan().key());
    let other = admit(executable());
    assert!(RuntimeTaskSource::from_borrowed(&source, &other, &other.program_owner()).is_none());
}

#[test]
fn program_kernel_source_rejects_scheduling_definition_and_capture_transfer_drift() {
    use RuntimeAwbcProgramSourceField as Field;
    for field in [
        Field::Class,
        Field::Priority,
        Field::CancelScope,
        Field::Policy,
        Field::HandleType,
        Field::Definition,
        Field::Capture { index: 0 },
    ] {
        let mut code = executable();
        match field {
            Field::Class => code.task_plans[0].class = AwbcTaskClass::Io,
            Field::Priority => code.task_plans[0].priority += 1,
            Field::CancelScope => code.task_plans[0].cancel_scope = AwbcStringId(1),
            Field::Policy => code.task_plans[0].policy = AwbcTaskPolicy::JoinSameKey,
            Field::HandleType => {
                code.task_plans[0].handle_type = crate::task::TaskHandleType::Need {
                    payload: code.task_plans[0].payload_type(),
                };
            }
            Field::Definition => {
                code.functions[0].definition =
                    crate::plan::RuntimeFunctionDefinitionIdentity::from_accepted_identity(
                        [0x71; 32],
                    )
            }
            Field::Capture { .. } => {
                code.functions[0].input_ownership[0].transfer =
                    RuntimeFunctionInputTransfer::Transferred(
                        crate::plan::RuntimeFunctionCaptureMode::Copy,
                    )
            }
            _ => unreachable!(),
        }
        let program = admit(code);
        let mut work = meter();
        assert!(
            matches!(program.awbc_program_source(AwbcTaskPlanId(0), RuntimeTaskPlanSealLimits::default(), &mut work),
            Err(RuntimeAwbcProgramSourceError::Mismatch { field: actual, .. }) if actual == field),
            "{field:?}"
        );
        assert!(
            work.charge_work(0).is_err(),
            "a rejected Source must poison its caller's work allowance"
        );
    }
}

#[test]
fn program_kernel_source_checks_payload_and_capture_types_against_the_original() {
    use RuntimeAwbcProgramSourceField as Field;
    for field in [Field::Payload, Field::CaptureType { index: 0 }] {
        let mut code = executable();
        match field {
            Field::Payload => {
                code.task_plans[0].handle_type = code.task_plans[0]
                    .handle_type
                    .map_payload(|_| AwbcTypeId(1));
                code.signatures[0].result = Some(AwbcTypeId(1));
                code.frame_layouts[0].slots[1].ty = AwbcTypeId(1);
                code.constants[0] = AwbcConstant::Bool(true);
            }
            Field::CaptureType { .. } => {
                code.signatures[0].params[0] = AwbcTypeId(0);
                code.frame_layouts[0].slots[0].ty = AwbcTypeId(0);
            }
            _ => unreachable!(),
        }
        let program = admit(code);
        assert!(
            matches!(program.awbc_program_source(AwbcTaskPlanId(0), RuntimeTaskPlanSealLimits::default(), &mut meter()),
            Err(RuntimeAwbcProgramSourceError::Type { field: actual, .. }) if actual == field),
            "{field:?}"
        );
    }
}

#[test]
fn program_kernel_requires_its_task_kind_and_suspending_role_flags() {
    let code = executable();
    assert!(
        crate::awbc::fiber::FiberState::for_function(
            &code,
            crate::awbc::fiber::AwbcFiberRoot::Function(AwbcFunctionId(0)),
            AwbcFunctionId(0),
            1,
            1,
        )
        .is_err(),
        "an ordinary function root cannot issue a Program launch"
    );
    let mut sourceless = executable();
    sourceless.original_source = None;
    assert!(
        sourceless
            .verify(AwbcVerifyBudget::default(), context())
            .is_err()
    );
    let mut ordinary = executable();
    ordinary.functions[0].kind = AwbcFunctionKind::Ordinary;
    assert!(
        ordinary
            .verify(AwbcVerifyBudget::default(), context())
            .is_err()
    );
    ordinary.task_plans.clear();
    ordinary
        .verify(AwbcVerifyBudget::default(), context())
        .unwrap();
    assert!(
        crate::awbc::fiber::FiberState::for_function(
            &ordinary,
            crate::awbc::fiber::AwbcFiberRoot::Function(AwbcFunctionId(0)),
            AwbcFunctionId(0),
            1,
            1,
        )
        .is_ok(),
        "the same ordinary function frame remains a valid control"
    );
    let mut nonsuspending = executable();
    nonsuspending.functions[0].flags = AwbcFunctionFlags::empty()
        .with(AwbcFunctionFlag::Deterministic)
        .with(AwbcFunctionFlag::MayAllocate);
    assert!(
        nonsuspending
            .verify(AwbcVerifyBudget::default(), context())
            .is_err()
    );
}

#[test]
fn program_source_rejects_host_rows_and_exhausted_work_without_issuing_a_lease() {
    let host = admit(super::super::template::tests::executable());
    assert!(matches!(
        host.awbc_program_source(
            AwbcTaskPlanId(0),
            RuntimeTaskPlanSealLimits::default(),
            &mut meter()
        ),
        Err(RuntimeAwbcProgramSourceError::Mismatch {
            field: RuntimeAwbcProgramSourceField::Kind,
            ..
        })
    ));
    let program = admit(executable());
    let mut work = TaskSemanticMeter::new(0, 0);
    assert!(matches!(
        program.awbc_program_source(
            AwbcTaskPlanId(0),
            RuntimeTaskPlanSealLimits::default(),
            &mut work
        ),
        Err(RuntimeAwbcProgramSourceError::Encoding(_))
    ));
    assert!(work.charge_work(0).is_err());
}

#[test]
fn program_kernel_retains_the_original_lexical_context_with_its_distinct_capture_abi() {
    let original = crate::plan::task_source::template_tests::thread_with_lexical_context();
    let mut code = executable_from(original);
    code.runtime_types.push(AwbcRuntimeType::new(
        crate::pattern::RuntimeSemanticTypeId::from_bytes([40; 32]),
        AwbcRuntimeTypeShape::Function {
            contract: crate::plan::RuntimeFunctionTypeContract::default(),
            parameters: Vec::new(),
            result: AwbcTypeId(0),
        },
    ));
    code.functions[0].type_context = Some(AwbcFunctionTypeContext::ProgramBody(AwbcTypeId(2)));
    let encoded = code.encode_canonical().unwrap();
    let decoded =
        AwbcProgram::decode_canonical(&encoded, crate::awbc::codec::AwbcDecodeBudget::default())
            .unwrap();
    assert_eq!(decoded, code);
    let program = admit(decoded);
    program
        .awbc_program_source(
            AwbcTaskPlanId(0),
            RuntimeTaskPlanSealLimits::default(),
            &mut meter(),
        )
        .unwrap();
    let mut wrong = code.clone();
    wrong.functions[0].type_context = Some(AwbcFunctionTypeContext::Signature(AwbcTypeId(2)));
    assert!(
        wrong
            .verify(AwbcVerifyBudget::default(), context())
            .is_err()
    );
    let mut wrong = code.clone();
    wrong.functions[0].kind = AwbcFunctionKind::Ordinary;
    wrong.task_plans.clear();
    assert!(
        wrong
            .verify(AwbcVerifyBudget::default(), context())
            .is_err()
    );
    let mut wrong = code;
    let plain = executable();
    wrong.original_source = plain.original_source;
    let AwbcTaskPlanExecution::Program(plain) = &plain.task_plans[0].execution else {
        unreachable!()
    };
    let AwbcTaskPlanExecution::Program(actual) = &mut wrong.task_plans[0].execution else {
        unreachable!()
    };
    actual.source = plain.source;
    let program = admit(wrong);
    assert!(matches!(
        program.awbc_program_source(
            AwbcTaskPlanId(0),
            RuntimeTaskPlanSealLimits::default(),
            &mut meter()
        ),
        Err(RuntimeAwbcProgramSourceError::Mismatch {
            field: RuntimeAwbcProgramSourceField::TypeContext,
            ..
        })
    ));
}

#[test]
fn program_source_rejects_effect_permissions_not_owned_by_the_enclosing_invocation() {
    let mut code = executable();
    code.strings[1] = "wrong.permission".into();
    code.effect_sets.push(AwbcEffectSet {
        effects: vec![AwbcStringId(1)],
    });
    code.signatures[0].effects = AwbcEffectSetId(1);
    let program = admit(code);
    assert!(matches!(
        program.awbc_program_source(
            AwbcTaskPlanId(0),
            RuntimeTaskPlanSealLimits::default(),
            &mut meter()
        ),
        Err(RuntimeAwbcProgramSourceError::Mismatch {
            field: RuntimeAwbcProgramSourceField::Effects,
            ..
        })
    ));
}
