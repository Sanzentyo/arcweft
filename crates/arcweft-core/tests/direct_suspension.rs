use arcweft_core::{
    awbc::{
        fiber::{
            FiberAwaitTarget, FiberCursor, FiberResumeTarget, FiberScope, FiberScopeCleanup,
            FiberState, FiberStateError, FiberStatus, FiberSuspension, FiberSuspensionReason,
            FiberTerminalValue,
        },
        schema::{
            AwbcBlock, AwbcBlockId, AwbcConstant, AwbcConstantId, AwbcEffectKind, AwbcEffectPlan,
            AwbcEffectPlanId, AwbcEffectSetId, AwbcEntry, AwbcEntryId, AwbcEntryKind,
            AwbcEntryTarget, AwbcFlowBinding, AwbcFrameLayout, AwbcFrameLayoutId, AwbcFrameSlot,
            AwbcFrameSlotRole, AwbcFunction, AwbcFunctionFlags, AwbcFunctionId, AwbcFunctionKind,
            AwbcInstruction, AwbcProgram, AwbcRegisterId, AwbcResumePoint, AwbcResumePointId,
            AwbcRuntimeType, AwbcSafePointKind, AwbcScopeId, AwbcSignature, AwbcSignatureId,
            AwbcStringId, AwbcTableRange, AwbcTerminator, AwbcTrapCode, AwbcTypeId,
        },
        verify::{AwbcVerifyBudget, AwbcVerifyContext},
        vm::{self, VmExit, VmObservation, VmStepOptions},
    },
    entry::{EntryBindingIdentity, RuntimeEntryRoles},
    plan::{EntryRuntimeId, FlowRuntimeId},
    task::NeedId,
    value::RuntimeValue,
};

const CALLER: AwbcFunctionId = AwbcFunctionId(0);
const CALLEE: AwbcFunctionId = AwbcFunctionId(1);
const CALL_RETURN: AwbcResumePointId = AwbcResumePointId(0);
const AWAIT_RESUME: AwbcResumePointId = AwbcResumePointId(1);
const NEED_REGISTER: AwbcRegisterId = AwbcRegisterId(0);
const RETURN_REGISTER: AwbcRegisterId = AwbcRegisterId(1);
const CLEANUP_EFFECT: AwbcEffectPlanId = AwbcEffectPlanId(0);
const DROP_CLEANUP_EFFECT: AwbcEffectPlanId = AwbcEffectPlanId(1);

#[test]
fn direct_call_reaches_need_await_on_the_same_fiber() {
    let program = direct_suspension_program();
    program
        .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
        .expect("typed direct-suspension program verifies");
    let mut fiber = direct_suspension_fiber(&program);

    let entered = vm::step(
        &program,
        &mut fiber,
        VmStepOptions {
            max_instructions: 1,
        },
    )
    .expect("direct call enters its callee");

    assert_eq!(entered.exit, VmExit::Running);
    assert_eq!(fiber.frames.len(), 2);
    assert_eq!(
        fiber.cursor,
        FiberCursor {
            function: CALLEE,
            block: AwbcBlockId(2),
            instruction_offset: 0,
        }
    );
    assert!(entered.observations.iter().all(|observation| !matches!(
        observation,
        VmObservation::TaskStarted { .. } | VmObservation::FiberSpawned { .. }
    )));

    let suspended = vm::step(
        &program,
        &mut fiber,
        VmStepOptions {
            max_instructions: 1,
        },
    )
    .expect("callee reaches its unresolved Need");

    assert!(matches!(
        suspended.exit,
        VmExit::Suspended(FiberSuspensionReason::Await {
            target: FiberAwaitTarget::Need(NeedId(ref need)),
            binding: None,
            observer: None,
        }) if need == "need.profile"
    ));
    assert_eq!(fiber.status, FiberStatus::Suspended);
    assert_eq!(fiber.frames.len(), 2);
    assert_eq!(
        fiber
            .suspension
            .as_ref()
            .and_then(FiberSuspension::declared_resume),
        Some(AWAIT_RESUME)
    );
}

#[test]
fn nested_direct_await_snapshot_round_trips_and_resumes_exactly() {
    let program = direct_suspension_program();
    let mut fiber = direct_suspension_fiber(&program);
    vm::step(
        &program,
        &mut fiber,
        VmStepOptions {
            max_instructions: 16,
        },
    )
    .expect("callee reaches its unresolved Need");
    fiber
        .validate_for_program(&program)
        .expect("nested suspended stack validates");

    let encoded = serde_json::to_string(&fiber).expect("nested suspended stack serializes");
    let mut restored: FiberState =
        serde_json::from_str(&encoded).expect("nested suspended stack restores");
    assert_eq!(restored, fiber);
    restored
        .validate_for_program(&program)
        .expect("restored nested stack validates");

    restored
        .resume_at(&program, AWAIT_RESUME)
        .expect("Need resolution resumes the exact callee point");
    let resumed = vm::step(
        &program,
        &mut restored,
        VmStepOptions {
            max_instructions: 16,
        },
    )
    .expect("callee and caller return after resume");

    assert_eq!(resumed.exit, VmExit::Returned(None));
    assert_eq!(restored.status, FiberStatus::Returned);
    assert_eq!(restored.frames.len(), 1);
}

#[test]
fn direct_return_restores_destination_and_drains_each_frame_lifo() {
    let program = direct_return_program();
    program
        .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
        .expect("typed direct-return program verifies");
    let mut fiber = direct_suspension_fiber(&program);
    install_frame_cleanups(
        fiber.active_frame_mut().expect("caller frame"),
        "caller",
        AwbcScopeId(0),
    );

    vm::step(
        &program,
        &mut fiber,
        VmStepOptions {
            max_instructions: 1,
        },
    )
    .expect("direct call enters its callee");
    install_frame_cleanups(
        fiber.active_frame_mut().expect("callee frame"),
        "callee",
        AwbcScopeId(0),
    );

    let callee_step = vm::step(
        &program,
        &mut fiber,
        VmStepOptions {
            max_instructions: 1,
        },
    )
    .expect("callee returns to its caller");
    assert_eq!(callee_step.exit, VmExit::Running);
    assert_eq!(
        cleanup_observation_keys(&callee_step.observations),
        vec![
            "callee.scope.2",
            "callee.scope.1",
            "callee.root.2",
            "callee.root.1",
        ]
    );
    assert_eq!(
        fiber
            .active_frame()
            .expect("caller frame")
            .register(RETURN_REGISTER)
            .expect("callee return destination"),
        &RuntimeValue::String("need.profile".to_owned())
    );
    assert_eq!(
        fiber.frames[0]
            .root_cleanups
            .iter()
            .map(|cleanup| cleanup.key.as_str())
            .collect::<Vec<_>>(),
        vec!["caller.root.1", "caller.root.2"]
    );

    let root_step = vm::step(
        &program,
        &mut fiber,
        VmStepOptions {
            max_instructions: 1,
        },
    )
    .expect("caller returns");
    assert_eq!(root_step.exit, VmExit::Returned(None));
    assert_eq!(
        cleanup_observation_keys(&root_step.observations),
        vec![
            "caller.scope.2",
            "caller.scope.1",
            "caller.root.2",
            "caller.root.1",
        ]
    );
}

#[test]
fn suspended_snapshot_rejects_resume_point_owned_by_the_caller() {
    let program = direct_suspension_program();
    let mut fiber = direct_suspension_fiber(&program);
    vm::step(
        &program,
        &mut fiber,
        VmStepOptions {
            max_instructions: 16,
        },
    )
    .expect("callee reaches its unresolved Need");

    fiber.suspension.as_mut().expect("await suspension").resume =
        FiberResumeTarget::Declared(CALL_RETURN);
    assert!(matches!(
        fiber.validate_for_program(&program),
        Err(FiberStateError::ResumeFunctionMismatch {
            resume: 0,
            actual: 0,
            expected: 1,
        })
    ));
}

#[test]
fn cancellation_unwinds_nested_frames_and_scopes_once_in_lifo_order() {
    let program = direct_suspension_program();
    let mut fiber = suspended_three_frame_fiber(&program);
    install_nested_cleanups(&mut fiber);
    fiber
        .validate_for_program(&program)
        .expect("nested suspended cleanup state validates");

    let cancellation = vm::cancel_fiber(&mut fiber);

    assert_eq!(cancellation.exit, VmExit::Cancelled);
    assert_eq!(fiber.status, FiberStatus::Cancelled);
    assert_eq!(fiber.terminal, Some(FiberTerminalValue::Cancelled));
    assert!(fiber.suspension.is_none());
    assert_eq!(
        cleanup_observation_keys(&cancellation.observations),
        expected_nested_cleanup_order()
    );
    assert!(fiber.frames.iter().all(|frame| {
        frame.root_cleanups.is_empty() && frame.scopes.iter().all(|scope| scope.cleanups.is_empty())
    }));
    fiber
        .validate_for_program(&program)
        .expect("cancelled stack remains a valid terminal snapshot");

    let encoded = serde_json::to_string(&fiber).expect("cancelled stack serializes");
    let restored: FiberState = serde_json::from_str(&encoded).expect("cancelled stack restores");
    assert_eq!(restored, fiber);
    restored
        .validate_for_program(&program)
        .expect("restored cancelled stack validates");

    let duplicate = vm::cancel_fiber(&mut fiber);
    assert_eq!(duplicate.exit, VmExit::Cancelled);
    assert!(duplicate.observations.is_empty());

    fiber.mark_trapped(arcweft_core::awbc::fiber::FiberTrap {
        code: AwbcTrapCode::InternalInvariant,
        message: Some("late trap".to_owned()),
        source_map: None,
    });
    assert_eq!(fiber.status, FiberStatus::Cancelled);
    assert_eq!(fiber.terminal, Some(FiberTerminalValue::Cancelled));
}

#[test]
fn cancelled_cleanup_registration_is_not_executed_during_unwind() {
    let program = cleanup_cancellation_program();
    program
        .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
        .expect("typed cleanup-cancellation program verifies");
    let mut fiber = direct_suspension_fiber(&program);
    vm::step(
        &program,
        &mut fiber,
        VmStepOptions {
            max_instructions: 16,
        },
    )
    .expect("callee registers, cancels, and reaches its unresolved Need");

    assert_eq!(fiber.status, FiberStatus::Suspended);
    assert_eq!(
        fiber
            .active_frame()
            .expect("suspended callee")
            .root_cleanups
            .iter()
            .map(|cleanup| cleanup.key.as_str())
            .collect::<Vec<_>>(),
        vec!["zz.zkeep"]
    );
    assert_eq!(
        vm::cancel_fiber(&mut fiber)
            .observations
            .iter()
            .filter(|observation| matches!(observation, VmObservation::Effect { .. }))
            .count(),
        1
    );
}

#[test]
fn suspended_owned_resource_cleanup_runs_exactly_once_on_cancellation() {
    let program = direct_suspension_program();
    let mut fiber = suspended_three_frame_fiber(&program);
    fiber
        .active_frame_mut()
        .expect("innermost suspended frame")
        .root_cleanups
        .push(FiberScopeCleanup {
            key: "resource.avatar".to_owned(),
            effect: DROP_CLEANUP_EFFECT,
            args: Vec::new(),
        });

    let cancellation = vm::cancel_fiber(&mut fiber);
    assert_eq!(
        cancellation
            .observations
            .iter()
            .filter(|observation| matches!(
                observation,
                VmObservation::Effect {
                    effect: DROP_CLEANUP_EFFECT,
                    args,
                } if args.is_empty()
            ))
            .count(),
        1
    );
    assert!(vm::cancel_fiber(&mut fiber).observations.is_empty());
}

#[test]
fn trap_below_suspended_callers_unwinds_without_becoming_cancellation() {
    let mut program = direct_suspension_program();
    program.blocks[3].terminator = AwbcTerminator::Trap {
        code: AwbcTrapCode::ExplicitPanic,
        message: None,
    };
    program
        .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
        .expect("typed trapping direct-suspension program verifies");
    let mut fiber = suspended_three_frame_fiber(&program);
    install_nested_cleanups(&mut fiber);

    fiber
        .resume_at(&program, AWAIT_RESUME)
        .expect("Need resolution resumes the innermost callee");
    let trapped = vm::step(
        &program,
        &mut fiber,
        VmStepOptions {
            max_instructions: 1,
        },
    )
    .expect("resumed callee reaches its trap");

    assert!(matches!(
        trapped.exit,
        VmExit::Trapped(ref trap) if trap.code == AwbcTrapCode::ExplicitPanic
    ));
    assert_eq!(fiber.status, FiberStatus::Trapped);
    assert_eq!(
        cleanup_observation_keys(&trapped.observations),
        expected_nested_cleanup_order()
    );
    assert!(matches!(
        trapped.observations.last(),
        Some(VmObservation::Trap(trap)) if trap.code == AwbcTrapCode::ExplicitPanic
    ));

    let duplicate = vm::step(
        &program,
        &mut fiber,
        VmStepOptions {
            max_instructions: 1,
        },
    )
    .expect("terminal trap remains stable");
    assert!(duplicate.observations.is_empty());
}

fn direct_suspension_fiber(program: &AwbcProgram) -> FiberState {
    let mut fiber =
        FiberState::for_entry(program, AwbcEntryId(0), 7, 64).expect("entry fiber initializes");
    fiber
        .active_frame_mut()
        .expect("caller frame")
        .set_register(
            NEED_REGISTER,
            RuntimeValue::String("need.profile".to_owned()),
        )
        .expect("bind typed Need handle");
    fiber
}

fn suspended_three_frame_fiber(program: &AwbcProgram) -> FiberState {
    let mut fiber = direct_suspension_fiber(program);
    let need = [RuntimeValue::String("need.profile".to_owned())];
    fiber
        .push_call_frame_with_args(program, CALLEE, CALL_RETURN, None, &need)
        .expect("caller enters first callee");
    fiber
        .push_call_frame_with_args(program, CALLEE, AWAIT_RESUME, None, &need)
        .expect("callee recursively enters innermost callee");
    fiber
        .suspend(FiberSuspension {
            resume: FiberResumeTarget::Declared(AWAIT_RESUME),
            reason: FiberSuspensionReason::Await {
                target: FiberAwaitTarget::Need(NeedId("need.profile".to_owned())),
                binding: None,
                observer: None,
            },
        })
        .expect("innermost callee suspends");
    fiber
}

fn install_nested_cleanups(fiber: &mut FiberState) {
    for (frame_index, frame) in fiber.frames.iter_mut().enumerate() {
        let owner = ["caller", "middle", "inner"][frame_index];
        install_frame_cleanups(
            frame,
            owner,
            AwbcScopeId(u32::try_from(frame_index).expect("fixture scope index fits u32")),
        );
    }
}

fn install_frame_cleanups(
    frame: &mut arcweft_core::awbc::fiber::FiberFrame,
    owner: &str,
    scope: AwbcScopeId,
) {
    frame.root_cleanups = vec![
        cleanup(format!("{owner}.root.1")),
        cleanup(format!("{owner}.root.2")),
    ];
    frame.scopes.push(FiberScope {
        id: scope,
        depth: 1,
        cleanups: vec![
            cleanup(format!("{owner}.scope.1")),
            cleanup(format!("{owner}.scope.2")),
        ],
    });
}

fn cleanup(key: String) -> FiberScopeCleanup {
    FiberScopeCleanup {
        key: key.clone(),
        effect: CLEANUP_EFFECT,
        args: vec![RuntimeValue::String(key)],
    }
}

fn cleanup_observation_keys(observations: &[VmObservation]) -> Vec<&str> {
    observations
        .iter()
        .filter_map(|observation| {
            let VmObservation::Effect {
                effect: CLEANUP_EFFECT,
                args,
            } = observation
            else {
                return None;
            };
            let [RuntimeValue::String(key)] = args.as_slice() else {
                panic!("cleanup observation has an unexpected payload: {args:?}");
            };
            Some(key.as_str())
        })
        .collect()
}

fn expected_nested_cleanup_order() -> Vec<&'static str> {
    vec![
        "inner.scope.2",
        "inner.scope.1",
        "inner.root.2",
        "inner.root.1",
        "middle.scope.2",
        "middle.scope.1",
        "middle.root.2",
        "middle.root.1",
        "caller.scope.2",
        "caller.scope.1",
        "caller.root.2",
        "caller.root.1",
    ]
}

fn direct_return_program() -> AwbcProgram {
    let mut program = direct_suspension_program();
    program.frame_layouts[0].slots.push(AwbcFrameSlot {
        name: None,
        ty: AwbcTypeId(2),
        role: AwbcFrameSlotRole::ReturnValue,
        scope_depth: 0,
    });
    program.signatures[1].result = Some(AwbcTypeId(2));
    program.blocks[0].terminator = AwbcTerminator::CallFunction {
        function: CALLEE,
        args: vec![NEED_REGISTER],
        dst: Some(RETURN_REGISTER),
        resume: CALL_RETURN,
    };
    program.blocks[2].terminator = AwbcTerminator::Return {
        value: Some(NEED_REGISTER),
    };
    program.functions[1].blocks = AwbcTableRange::new(2, 1);
    program.functions[1].flags = AwbcFunctionFlags(AwbcFunctionFlags::DETERMINISTIC);
    program.blocks.truncate(3);
    program.resume_points.truncate(1);
    program
}

fn cleanup_cancellation_program() -> AwbcProgram {
    let mut program = direct_suspension_program();
    let keep =
        AwbcStringId(u32::try_from(program.strings.len()).expect("fixture string table fits u32"));
    program.strings.push("zz.zkeep".to_owned());
    let removed =
        AwbcStringId(u32::try_from(program.strings.len()).expect("fixture string table fits u32"));
    program.strings.push("zz.zremoved".to_owned());
    program.instructions = vec![
        AwbcInstruction::RegisterCleanup {
            key: keep,
            effect: CLEANUP_EFFECT,
            args: vec![NEED_REGISTER],
        },
        AwbcInstruction::RegisterCleanup {
            key: removed,
            effect: CLEANUP_EFFECT,
            args: vec![NEED_REGISTER],
        },
        AwbcInstruction::CancelCleanup { key: removed },
    ];
    program.blocks[2].instructions = AwbcTableRange::new(0, 3);
    program
}

fn direct_suspension_program() -> AwbcProgram {
    let need_ty = AwbcTypeId(2);
    AwbcProgram {
        strings: vec![
            "flow.main".to_owned(),
            "need".to_owned(),
            "zz.cleanup".to_owned(),
            "zz.message".to_owned(),
        ],
        runtime_types: vec![
            AwbcRuntimeType::Unit,
            AwbcRuntimeType::Dynamic,
            AwbcRuntimeType::NeedHandle,
        ],
        constants: vec![
            AwbcConstant::String(AwbcStringId(2)),
            AwbcConstant::String(AwbcStringId(3)),
        ],
        signatures: direct_suspension_signatures(need_ty),
        frame_layouts: vec![need_frame_layout(need_ty), need_frame_layout(need_ty)],
        functions: direct_suspension_functions(),
        flow_bindings: vec![AwbcFlowBinding {
            flow: FlowRuntimeId::from_checked_declaration_digest([0xa6; 32], "flow.main")
                .expect("test checked Flow identity"),
            function: CALLER,
        }],
        blocks: direct_suspension_blocks(),
        resume_points: direct_suspension_resume_points(),
        effect_plans: direct_suspension_effect_plans(),
        entries: vec![direct_suspension_entry()],
        ..AwbcProgram::default()
    }
}

fn direct_suspension_signatures(need_ty: AwbcTypeId) -> Vec<AwbcSignature> {
    vec![
        AwbcSignature {
            params: vec![need_ty],
            result: None,
            effects: AwbcEffectSetId(0),
        },
        AwbcSignature {
            params: vec![need_ty],
            result: None,
            effects: AwbcEffectSetId(0),
        },
        AwbcSignature {
            params: Vec::new(),
            result: None,
            effects: AwbcEffectSetId(0),
        },
    ]
}

fn direct_suspension_functions() -> Vec<AwbcFunction> {
    vec![
        AwbcFunction {
            public_id: Some(AwbcStringId(0)),
            kind: AwbcFunctionKind::Flow,
            signature: AwbcSignatureId(0),
            frame_layout: AwbcFrameLayoutId(0),
            blocks: AwbcTableRange::new(0, 2),
            entry_block: AwbcBlockId(0),
            flags: AwbcFunctionFlags(AwbcFunctionFlags::DETERMINISTIC),
        },
        AwbcFunction {
            public_id: None,
            kind: AwbcFunctionKind::Synthetic,
            signature: AwbcSignatureId(1),
            frame_layout: AwbcFrameLayoutId(1),
            blocks: AwbcTableRange::new(2, 2),
            entry_block: AwbcBlockId(2),
            flags: AwbcFunctionFlags(
                AwbcFunctionFlags::DETERMINISTIC | AwbcFunctionFlags::MAY_SUSPEND,
            ),
        },
    ]
}

fn direct_suspension_blocks() -> Vec<AwbcBlock> {
    vec![
        AwbcBlock {
            owner: CALLER,
            instructions: AwbcTableRange::new(0, 0),
            terminator: AwbcTerminator::CallFunction {
                function: CALLEE,
                args: vec![NEED_REGISTER],
                dst: None,
                resume: CALL_RETURN,
            },
            safe_point: AwbcSafePointKind::FlowEntry,
            source_map: None,
        },
        AwbcBlock {
            owner: CALLER,
            instructions: AwbcTableRange::new(0, 0),
            terminator: AwbcTerminator::Return { value: None },
            safe_point: AwbcSafePointKind::CallableBoundary,
            source_map: None,
        },
        AwbcBlock {
            owner: CALLEE,
            instructions: AwbcTableRange::new(0, 0),
            terminator: AwbcTerminator::Await {
                handle: NEED_REGISTER,
                binding: None,
                observer: None,
                resume: AWAIT_RESUME,
            },
            safe_point: AwbcSafePointKind::CallableBoundary,
            source_map: None,
        },
        AwbcBlock {
            owner: CALLEE,
            instructions: AwbcTableRange::new(0, 0),
            terminator: AwbcTerminator::Return { value: None },
            safe_point: AwbcSafePointKind::None,
            source_map: None,
        },
    ]
}

fn direct_suspension_resume_points() -> Vec<AwbcResumePoint> {
    vec![
        AwbcResumePoint {
            function: CALLER,
            block: AwbcBlockId(1),
            frame_layout: AwbcFrameLayoutId(0),
            kind: AwbcSafePointKind::CallableBoundary,
        },
        AwbcResumePoint {
            function: CALLEE,
            block: AwbcBlockId(3),
            frame_layout: AwbcFrameLayoutId(1),
            kind: AwbcSafePointKind::Await,
        },
    ]
}

fn direct_suspension_effect_plans() -> Vec<AwbcEffectPlan> {
    vec![
        AwbcEffectPlan {
            kind: AwbcEffectKind::Log,
            signature: AwbcSignatureId(0),
            capability: None,
            audio: None,
            static_args: vec![AwbcConstantId(0), AwbcConstantId(1)],
            resources: Vec::new(),
        },
        AwbcEffectPlan {
            kind: AwbcEffectKind::DropHandle,
            signature: AwbcSignatureId(2),
            capability: None,
            audio: None,
            static_args: vec![AwbcConstantId(0)],
            resources: Vec::new(),
        },
    ]
}

fn direct_suspension_entry() -> AwbcEntry {
    AwbcEntry {
        runtime_id: EntryRuntimeId::canonical("main")
            .expect("test entry runtime identity is valid"),
        binding: EntryBindingIdentity::from_bytes([1; 32]),
        public_id: AwbcStringId(0),
        kind: AwbcEntryKind::Cli,
        signature: AwbcSignatureId(0),
        target: AwbcEntryTarget::Function(CALLER),
        roles: RuntimeEntryRoles::None,
    }
}

fn need_frame_layout(need_ty: AwbcTypeId) -> AwbcFrameLayout {
    AwbcFrameLayout {
        slots: vec![AwbcFrameSlot {
            name: Some(AwbcStringId(1)),
            ty: need_ty,
            role: AwbcFrameSlotRole::Parameter,
            scope_depth: 0,
        }],
        max_scope_depth: 1,
    }
}
