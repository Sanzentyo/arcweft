use super::*;
use crate::effect::RuntimeRegistryReadKind;
use arcweft_id::{DeclarationName, LifetimeRegistryKey, LifetimeScopeKind};

fn registry_program(scope: LifetimeScopeKind, path: &[&str], declared: &str) -> AwbcProgram {
    let mut program = minimal_program();
    program
        .runtime_types
        .push(runtime_type(1, AwbcRuntimeTypeShape::Bool));
    let effects = add_effect_set(&mut program, &[declared]);
    program.signatures.push(AwbcSignature {
        params: vec![AwbcTypeId(0)],
        result: None,
        effects,
    });
    let capability = AwbcStringId(u32::try_from(program.strings.len()).unwrap());
    program.strings.push("state.write".to_owned());
    program.effect_plans.push(AwbcEffectPlan {
        signature: AwbcSignatureId(1),
        capability: Some(capability),
        payload: AwbcEffectPayload::RegistryWrite {
            key: LifetimeRegistryKey::new(
                scope,
                path.iter()
                    .map(|name| DeclarationName::try_new(*name).unwrap())
                    .collect(),
            ),
            value_type: AwbcTypeId(0),
        },
        resources: Vec::new(),
    });
    program.canonicalize_string_table();
    program
}

#[test]
fn registry_effect_round_trip_preserves_root_unicode_and_named_keys() {
    for (scope, path, declared) in [
        (
            LifetimeScopeKind::Flow,
            vec!["flags", "seen"],
            "state.write('flow)",
        ),
        (
            LifetimeScopeKind::Persistent,
            vec![],
            "state.write('persistent)",
        ),
        (
            LifetimeScopeKind::parse("章").unwrap(),
            vec!["進行", "旗"],
            "state.write('章)",
        ),
    ] {
        let program = registry_program(scope, &path, declared);
        program
            .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
            .unwrap();
        let encoded = program.encode_canonical().unwrap();
        let decoded = AwbcProgram::decode_canonical(&encoded, AwbcDecodeBudget::default()).unwrap();
        assert_eq!(decoded.effect_plans, program.effect_plans);
        assert_eq!(
            decoded.effect_plans[0].kind(),
            AwbcEffectKind::RegistryWrite
        );
        decoded
            .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
            .unwrap();
        assert_eq!(decoded.encode_canonical().unwrap(), encoded);
    }
}

#[test]
fn registry_effect_requires_scoped_effect_coverage_on_segment_boundaries() {
    for declared in [
        "state.write('scene)",
        "state.write('flow.flag)",
        "log.write",
    ] {
        let program = registry_program(LifetimeScopeKind::Flow, &["flags", "seen"], declared);
        assert!(
            matches!(
                program.verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default()),
                Err(AwbcVerifyError::MalformedEffectPayload { effect: 0, .. })
            ),
            "accepted unrelated row {declared}"
        );
    }
    let program = registry_program(
        LifetimeScopeKind::Flow,
        &["flags", "seen"],
        "state.write('flow.flags)",
    );
    program
        .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
        .unwrap();
}

#[test]
fn registry_effect_rejects_missing_capability_wrong_signature_and_unknown_value_type() {
    let original = registry_program(LifetimeScopeKind::Flow, &["seen"], "state.write('flow)");
    for fault in 0..4 {
        let mut program = original.clone();
        match fault {
            0 => program.effect_plans[0].capability = None,
            1 => program.signatures[1].params.clear(),
            2 => program.signatures[1].result = Some(AwbcTypeId(0)),
            3 => {
                let AwbcEffectPayload::RegistryWrite { value_type, .. } =
                    &mut program.effect_plans[0].payload
                else {
                    unreachable!()
                };
                *value_type = AwbcTypeId(999);
            }
            _ => unreachable!(),
        }
        assert!(
            matches!(
                program.verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default()),
                Err(AwbcVerifyError::MalformedEffectPayload { effect: 0, .. })
                    | Err(AwbcVerifyError::IndexOutOfBounds {
                        table: "runtime_types",
                        index: 999,
                        ..
                    })
            ),
            "accepted invalid registry plan {fault}"
        );
    }
}

#[test]
fn registry_drop_preserves_item_type_presence_and_policy_across_wire() {
    use crate::effect::{RuntimeDropPolicyKind, RuntimeRegistryDropTarget};

    let mut program = registry_program(
        LifetimeScopeKind::Flow,
        &["flags", "seen"],
        "state.write('flow.flags)",
    );
    let AwbcEffectPayload::RegistryWrite { key, .. } = &program.effect_plans[0].payload else {
        unreachable!()
    };
    let item_type = AwbcTypeId(u32::try_from(program.runtime_types.len() - 1).unwrap());
    let fade_type = AwbcTypeId(u32::try_from(program.runtime_types.len()).unwrap());
    program.effect_plans[0].payload = AwbcEffectPayload::RegistryDrop {
        target: RuntimeRegistryDropTarget::new(
            key.clone(),
            RuntimeRegistryReadKind::Optional,
            item_type,
        ),
        policy: RuntimeDropPolicyKind::Stop,
    };
    program
        .runtime_types
        .push(runtime_type(2, AwbcRuntimeTypeShape::Duration));
    program.signatures[1].params = vec![fade_type];
    let check = |program: &AwbcProgram| {
        program.verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
    };
    check(&program).unwrap();
    let encoded = program.encode_canonical().unwrap();
    let decoded = AwbcProgram::decode_canonical(&encoded, AwbcDecodeBudget::default()).unwrap();
    assert_eq!(decoded.effect_plans, program.effect_plans);
    check(&decoded).unwrap();
    assert_eq!(decoded.encode_canonical().unwrap(), encoded);

    program.signatures[1].params = vec![item_type];
    assert!(matches!(
        check(&program),
        Err(AwbcVerifyError::MalformedEffectPayload { effect: 0, .. })
    ));
}

fn registry_read_program(kind: RuntimeRegistryReadKind, write: bool) -> AwbcProgram {
    let mut program = registry_program(LifetimeScopeKind::Flow, &["進行"], "state.write('flow)");
    program.runtime_types = vec![AwbcRuntimeType::new(
        RuntimeCheckedType::Bool.semantic_identity_digest(),
        AwbcRuntimeTypeShape::Bool,
    )];
    let slot = |ty| AwbcFrameSlot {
        name: None,
        ty,
        role: AwbcFrameSlotRole::Temporary,
        scope_depth: 0,
    };
    let result = match kind {
        RuntimeRegistryReadKind::Required => AwbcTypeId(0),
        RuntimeRegistryReadKind::Optional => {
            let owner = RuntimeBuiltinVariantIdentity::Option;
            let cases = owner
                .cases()
                .iter()
                .map(|case| {
                    let name = AwbcStringId(program.strings.len().try_into().unwrap());
                    program.strings.push(case.name().to_owned());
                    AwbcVariantCase {
                        name,
                        payload: (case.name() == "Some").then_some(AwbcTypeId(1)),
                    }
                })
                .collect();
            program.runtime_types.extend([
                AwbcRuntimeType::new(
                    RuntimeCheckedType::Tuple(vec![RuntimeCheckedType::Bool])
                        .semantic_identity_digest(),
                    AwbcRuntimeTypeShape::Tuple(vec![AwbcTypeId(0)]),
                ),
                AwbcRuntimeType::new(
                    RuntimeCheckedType::Option(Box::new(RuntimeCheckedType::Bool))
                        .semantic_identity_digest(),
                    AwbcRuntimeTypeShape::Variant {
                        owner: AwbcVariantIdentity::Builtin(owner),
                        arguments: Vec::new(),
                        cases,
                    },
                ),
            ]);
            AwbcTypeId(2)
        }
    };
    program.signatures[0].effects = program.signatures[1].effects;
    program.signatures[0].result = Some(result);
    program.frame_layouts[0].slots = vec![slot(AwbcTypeId(0)), slot(result)];
    if write {
        program.constants.push(AwbcConstant::Bool(true));
        program.instructions.extend([
            AwbcInstruction::LoadConst {
                dst: AwbcRegisterId(0),
                constant: AwbcConstantId(0),
            },
            AwbcInstruction::EmitEffect {
                effect: AwbcEffectPlanId(0),
                args: vec![AwbcRegisterId(0)],
            },
        ]);
    }
    let AwbcEffectPayload::RegistryWrite { key, .. } = &program.effect_plans[0].payload else {
        unreachable!()
    };
    program.instructions.push(AwbcInstruction::ReadRegistry {
        dst: AwbcRegisterId(1),
        key: key.clone(),
        kind,
    });
    program.blocks[0].instructions =
        AwbcTableRange::new(0, program.instructions.len().try_into().unwrap());
    program.blocks[0].terminator = AwbcTerminator::Return {
        value: Some(AwbcRegisterId(1)),
    };
    program.canonicalize_string_table();
    program
}

#[test]
fn decoded_registry_read_requires_a_reached_row_and_preserves_optional_absence() {
    use crate::awbc::fiber::FiberTerminalValue;
    use crate::awbc::product_step::AwbcProductExecutorSaveSnapshot;
    use crate::engine::FlowFiberStatus;
    use crate::executor::{
        ArcweftRuntimeExecutor, ArcweftRuntimeExecutorSnapshot, RuntimeExecutor,
    };
    use crate::step::{RuntimeStepInput, RuntimeStepOptions};
    use crate::value::AwbcRuntimeValueSnapshot;

    for (kind, write, expected) in [
        (
            RuntimeRegistryReadKind::Required,
            true,
            Some(RuntimeValue::Bool(true)),
        ),
        (
            RuntimeRegistryReadKind::Optional,
            true,
            Some(RuntimeValue::option_some(RuntimeValue::Bool(true))),
        ),
        (
            RuntimeRegistryReadKind::Optional,
            false,
            Some(RuntimeValue::option_none()),
        ),
        (RuntimeRegistryReadKind::Required, false, None),
    ] {
        let original = registry_read_program(kind, write);
        let encoded = original.encode_canonical().unwrap();
        let decoded = AwbcProgram::decode_canonical(&encoded, AwbcDecodeBudget::default()).unwrap();
        assert_eq!(decoded.instructions, original.instructions);
        assert_eq!(decoded.encode_canonical().unwrap(), encoded);
        let mut executor =
            ArcweftRuntimeExecutor::from_awbc_product(decoded, AwbcEntryId(0)).unwrap();
        for _ in 0..8 {
            let step = executor.step(RuntimeStepInput::default(), RuntimeStepOptions::default());
            assert!(step.stats.executed_ops <= 1);
            match &step.fiber_status {
                FlowFiberStatus::Done(_) => {
                    let ArcweftRuntimeExecutorSnapshot::AwbcProduct(snapshot) =
                        executor.snapshot().unwrap();
                    let expected = AwbcRuntimeValueSnapshot::from_runtime_value_for_program(
                        expected
                            .as_ref()
                            .expect("a successful read returns a value"),
                        &executor.program_owner(),
                    )
                    .unwrap();
                    assert_eq!(
                        snapshot.fiber.terminal,
                        Some(FiberTerminalValue::Returned(Some(expected)))
                    );
                    assert!(step.output.diagnostics.is_empty());
                    break;
                }
                FlowFiberStatus::Failed(reason) => {
                    assert!(expected.is_none(), "{kind:?}: {reason}");
                    assert!(
                        reason.contains("required registry value is absent"),
                        "{reason}"
                    );
                    break;
                }
                _ => {
                    assert!(step.output.diagnostics.is_empty(), "{step:?}");
                }
            }
            let ArcweftRuntimeExecutorSnapshot::AwbcProduct(snapshot) =
                executor.snapshot().unwrap();
            let saved = AwbcProductExecutorSaveSnapshot::from_live(
                &snapshot,
                executor.admitted_product_awbc_program().unwrap(),
            )
            .unwrap();
            let decoded = serde_json::from_slice(&serde_json::to_vec(&saved).unwrap()).unwrap();
            let bound = executor.bind_product_save_snapshot(decoded).unwrap();
            executor = executor
                .restore_snapshot(ArcweftRuntimeExecutorSnapshot::AwbcProduct(bound))
                .unwrap();
        }
        assert!(matches!(
            executor.fiber().frame.status,
            FlowFiberStatus::Done(_) | FlowFiberStatus::Failed(_)
        ));
    }
}

#[test]
fn registry_read_rejects_a_reached_value_with_the_wrong_result_type() {
    use crate::executor::{ArcweftRuntimeExecutor, RuntimeExecutor};
    use crate::step::{RuntimeStepInput, RuntimeStepOptions};

    let mut program = registry_read_program(RuntimeRegistryReadKind::Required, true);
    let string = AwbcTypeId(1);
    program.runtime_types.push(AwbcRuntimeType::new(
        RuntimeCheckedType::String.semantic_identity_digest(),
        AwbcRuntimeTypeShape::String,
    ));
    program.signatures[0].result = Some(string);
    program.frame_layouts[0].slots[1].ty = string;
    let mut executor = ArcweftRuntimeExecutor::from_awbc_product(program, AwbcEntryId(0)).unwrap();
    for _ in 0..2 {
        let step = executor.step(RuntimeStepInput::default(), RuntimeStepOptions::default());
        assert!(step.output.diagnostics.is_empty());
    }
    let step = executor.step(RuntimeStepInput::default(), RuntimeStepOptions::default());
    assert!(
        matches!(step.fiber_status, crate::engine::FlowFiberStatus::Failed(_)),
        "{step:?}"
    );
    assert!(!step.output.diagnostics.is_empty());
}

#[test]
fn registry_read_verifies_scope_and_optional_destination() {
    let original = registry_read_program(RuntimeRegistryReadKind::Required, true);
    for (scope, kind) in [
        (LifetimeScopeKind::Global, RuntimeRegistryReadKind::Required),
        (LifetimeScopeKind::Flow, RuntimeRegistryReadKind::Optional),
    ] {
        let mut program = original.clone();
        program.instructions[2] = AwbcInstruction::ReadRegistry {
            dst: AwbcRegisterId(1),
            key: LifetimeRegistryKey::new(scope, Box::new([])),
            kind,
        };
        assert!(
            program
                .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
                .is_err()
        );
    }
    let mut program = original;
    program.instructions.push(program.instructions[2].clone());
    program.blocks[0].instructions = AwbcTableRange::new(0, 4);
    assert!(
        program
            .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
            .is_err()
    );
}

fn inactive_registry_write_program() -> AwbcProgram {
    let mut program = registry_program(
        LifetimeScopeKind::parse("unavailable").unwrap(),
        &["flags"],
        "state.write('unavailable)",
    );
    program.runtime_types[0] = AwbcRuntimeType::new(
        RuntimeCheckedType::Bool.semantic_identity_digest(),
        AwbcRuntimeTypeShape::Bool,
    );
    program.signatures[0].effects = program.signatures[1].effects;
    program.frame_layouts[0].slots = vec![AwbcFrameSlot {
        name: None,
        ty: AwbcTypeId(0),
        role: AwbcFrameSlotRole::Temporary,
        scope_depth: 0,
    }];
    program.constants = vec![AwbcConstant::Bool(true), AwbcConstant::Bool(false)];
    program.instructions = vec![
        AwbcInstruction::LoadConst {
            dst: AwbcRegisterId(0),
            constant: AwbcConstantId(0),
        },
        AwbcInstruction::EmitEffect {
            effect: AwbcEffectPlanId(0),
            args: vec![AwbcRegisterId(0)],
        },
        AwbcInstruction::LoadConst {
            dst: AwbcRegisterId(0),
            constant: AwbcConstantId(1),
        },
    ];
    program.blocks[0].instructions = AwbcTableRange::new(0, 3);
    program
        .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
        .unwrap();
    program
}

#[test]
fn registry_write_without_scope_traps_before_taking_the_operand_or_running_the_suffix() {
    use crate::awbc::vm::{VmExit, VmStepOptions, step};
    let program = inactive_registry_write_program();
    let mut fiber = FiberState::for_entry(&program, AwbcEntryId(0), 0, 64).unwrap();
    let output = step(
        &program,
        &mut fiber,
        VmStepOptions {
            max_instructions: 64,
        },
    )
    .unwrap();
    assert!(matches!(output.exit, VmExit::Trapped(_)), "{output:?}");
    assert_eq!(fiber.status, FiberStatus::Trapped);
    assert_eq!(fiber.cursor.instruction_offset, 1);
    assert_eq!(
        fiber
            .active_frame()
            .unwrap()
            .register(AwbcRegisterId(0))
            .unwrap(),
        &RuntimeValue::Bool(true)
    );
    assert!(
        output.observations.iter().all(|observation| !matches!(
            observation,
            crate::awbc::vm::VmObservation::Effect { .. }
        ))
    );
}

#[test]
fn decoded_product_registry_rejection_reports_failed_instead_of_done() {
    use crate::engine::FlowFiberStatus;
    use crate::executor::{ArcweftRuntimeExecutor, RuntimeExecutor};
    use crate::step::{
        RuntimeStepBudget, RuntimeStepInput, RuntimeStepMode, RuntimeStepOptions,
        RuntimeStepStopReason,
    };
    let original = inactive_registry_write_program();
    let decoded = AwbcProgram::decode_canonical(
        &original.encode_canonical().unwrap(),
        AwbcDecodeBudget::default(),
    )
    .unwrap();
    let mut executor = ArcweftRuntimeExecutor::from_awbc_product(decoded, AwbcEntryId(0)).unwrap();
    let result = executor.step(
        RuntimeStepInput::default(),
        RuntimeStepOptions {
            mode: RuntimeStepMode::Drain,
            budget: RuntimeStepBudget { max_ops: 64 },
            ..RuntimeStepOptions::default()
        },
    );
    assert!(
        matches!(executor.fiber().frame.status, FlowFiberStatus::Failed(_)),
        "{result:?}"
    );
    assert_eq!(result.stop_reason, RuntimeStepStopReason::Failed);
    assert!(!result.output.diagnostics.is_empty());
    assert!(result.output.effects.line.is_empty());
    assert!(result.output.flow_events.iter().all(|event| !matches!(
        event,
        crate::plan::FlowEvent::Done | crate::plan::FlowEvent::Return { .. }
    )));
}

fn inactive_registry_cleanup_program() -> AwbcProgram {
    let mut program = inactive_registry_write_program();
    let key = AwbcStringId(u32::try_from(program.strings.len()).unwrap());
    program.strings.push("registry.cleanup".to_owned());
    program.instructions[1] = AwbcInstruction::RegisterCleanup {
        key,
        effect: AwbcEffectPlanId(0),
        args: vec![AwbcRegisterId(0)],
    };
    program.canonicalize_string_table();
    program
        .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
        .unwrap();
    program
}

#[test]
fn registry_cleanup_registration_rejects_before_transferring_its_operand() {
    use crate::awbc::vm::{VmExit, VmStepOptions, step};
    let program = inactive_registry_cleanup_program();
    let mut fiber = FiberState::for_entry(&program, AwbcEntryId(0), 0, 64).unwrap();
    let output = step(
        &program,
        &mut fiber,
        VmStepOptions {
            max_instructions: 64,
        },
    )
    .unwrap();
    assert!(matches!(output.exit, VmExit::Trapped(_)), "{output:?}");
    assert_eq!(fiber.cursor.instruction_offset, 1);
    let frame = fiber.active_frame().unwrap();
    assert!(frame.root_cleanups.is_empty());
    assert_eq!(
        frame.register(AwbcRegisterId(0)).unwrap(),
        &RuntimeValue::Bool(true)
    );
}

#[test]
fn registry_static_cleanup_rejects_the_same_source_in_live_and_inert_phases() {
    use crate::awbc::fiber::{FiberCursor, FiberStateError};
    let program = inactive_registry_cleanup_program();
    let source = FiberCursor {
        function: AwbcFunctionId(0),
        block: AwbcBlockId(0),
        instruction_offset: 1,
    };
    let live = FiberScopeCleanup {
        source,
        args: vec![RuntimeValue::Bool(true)],
    };
    let inert = FiberScopeCleanup {
        source,
        args: vec![crate::value::AwbcRuntimeValueSnapshot::Bool(true)],
    };
    let expected = FiberStateError::EffectMaterialization(
        crate::effect::RuntimeEffectMaterializeError::RegistryContextRequired,
    );
    assert_eq!(live.key(&program), Err(expected.clone()));
    assert_eq!(inert.key(&program), Err(expected.clone()));
    let decoded: FiberScopeCleanup<crate::value::AwbcRuntimeValueSnapshot> =
        serde_json::from_slice(&serde_json::to_vec(&inert).unwrap()).unwrap();
    assert_eq!(decoded.key(&program), Err(expected));
    assert_eq!(live.args, vec![RuntimeValue::Bool(true)]);
    assert_eq!(
        inert.args,
        vec![crate::value::AwbcRuntimeValueSnapshot::Bool(true)]
    );
    let mut fiber = FiberState::for_entry(&program, AwbcEntryId(0), 0, 64).unwrap();
    fiber.cursor.instruction_offset = 2;
    let owner = RuntimeProgramOwner::Awbc(std::sync::Arc::new(program));
    assert!(
        fiber
            .checkpoint(
                &mut crate::value::RuntimeRetainedValueAdmission::for_program(
                    (owner).clone(),
                    crate::entry::RuntimeSchemaLimits::engine_default()
                )
            )
            .is_ok()
    );
    fiber.active_frame_mut().unwrap().root_cleanups.push(live);
    assert!(
        fiber
            .checkpoint(
                &mut crate::value::RuntimeRetainedValueAdmission::for_program(
                    (owner).clone(),
                    crate::entry::RuntimeSchemaLimits::engine_default()
                )
            )
            .is_err(),
        "an untrusted cleanup cannot enter an inert fiber image"
    );
    assert_eq!(fiber.active_frame().unwrap().root_cleanups.len(), 1);
}
