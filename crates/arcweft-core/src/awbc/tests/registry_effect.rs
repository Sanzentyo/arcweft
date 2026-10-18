use super::*;
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
    assert!(fiber.checkpoint(&owner).is_ok());
    fiber.active_frame_mut().unwrap().root_cleanups.push(live);
    assert!(
        fiber.checkpoint(&owner).is_err(),
        "an untrusted cleanup cannot enter an inert fiber image"
    );
    assert_eq!(fiber.active_frame().unwrap().root_cleanups.len(), 1);
}
