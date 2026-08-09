use crate::engine::{Engine, FlowFiberStatus};
use crate::entry::{
    CallableContractHash, EntryBindingIdentity, FlowContractHash, RootExecutionLimits,
    RuntimeCallableExecutable, RuntimeCallableExecutableCode, RuntimeCallableId,
    RuntimeCallableRole, RuntimeCommandConstructorId, RuntimeCommandContract, RuntimeCommandPolicy,
    RuntimeCommandTargetId, RuntimeEntryRoles, RuntimeFlowExecutable,
    RuntimeFlowExecutableParameter, RuntimeFlowParameterMode, RuntimeFlowRole, RuntimeNominalRole,
    RuntimeNominalTypeId, RuntimeStatefulEntryRoles, RuntimeTypeSchema,
};
use crate::pattern::{RuntimeSemanticTypeId, RuntimeVariantIdentity};
use crate::plan::{
    FlowOp, RuntimeEntryKind, RuntimeEntrySpec, RuntimeEntryTarget, RuntimeFlow, RuntimePlan,
    RuntimePlanError, RuntimePureHelper, RuntimePureHelperId, RuntimePureHelperOrigin,
    RuntimePureInputType, RuntimePureOutputType,
};
use crate::root::{
    RootCallableEvaluationError, RootCallableEvaluator, RootEventInput, RootRuntime,
    RootRuntimeError, RootStartupContract, RootTransitionOutcome, TransitionSequence,
};
use crate::step::{
    RuntimeDiagnosticCategory, RuntimeStepBudget, RuntimeStepInput, RuntimeStepMode,
    RuntimeStepOptions,
};
use crate::value::{
    RuntimeBinding, RuntimeExpr, RuntimeFieldValue, RuntimeFunctionValue, RuntimePayload,
    RuntimeSeq, RuntimeValue,
};

fn reducer_ok(state: RuntimeValue) -> RuntimeValue {
    reducer_ok_with_commands(state, Vec::new())
}

fn reducer_ok_with_commands(state: RuntimeValue, commands: Vec<RuntimeValue>) -> RuntimeValue {
    RuntimeValue::result_ok(RuntimeValue::Record(vec![
        RuntimeFieldValue {
            name: "state".to_owned(),
            value: state,
        },
        RuntimeFieldValue {
            name: "commands".to_owned(),
            value: RuntimeValue::Seq(RuntimeSeq::values(commands)),
        },
    ]))
}

fn nominal_variant(owner: &str, name: &str, payload: Option<RuntimeValue>) -> RuntimeValue {
    RuntimeValue::Variant {
        owner: RuntimeVariantIdentity::Nominal {
            nominal: RuntimeNominalTypeId::try_new(owner).expect("nominal identity"),
            semantic_identity: RuntimeSemanticTypeId::from_bytes([0x52; 32]),
        },
        ordinal: 0,
        name: name.to_owned(),
        payload: payload.map(Box::new),
    }
}

fn stateful_plan(flow_ops: Vec<FlowOp>) -> (RuntimePlan, crate::plan::EntryRuntimeId) {
    stateful_plan_with(
        flow_ops,
        RuntimeTypeSchema::I64,
        RuntimeTypeSchema::I64,
        RuntimeValue::i64(1),
        RuntimePureOutputType::I64,
        reducer_ok(RuntimeValue::i64(2)),
        RuntimeCommandPolicy::deny_all(RootExecutionLimits::engine_default()),
    )
}

fn root_helpers(
    initializer_value: RuntimeValue,
    initializer_output: RuntimePureOutputType,
    reducer_value: RuntimeValue,
) -> Vec<RuntimePureHelper> {
    vec![
        RuntimePureHelper {
            id: RuntimePureHelperId(0),
            name: "initial_state".to_owned(),
            input_names: Vec::new(),
            input_types: Vec::new(),
            output_type: initializer_output,
            expr: RuntimeExpr::Value(initializer_value),
            scalar_eval_supported: true,
            origin: RuntimePureHelperOrigin::Annotated,
        },
        RuntimePureHelper {
            id: RuntimePureHelperId(1),
            name: "reduce".to_owned(),
            input_names: vec!["state".to_owned(), "event".to_owned()],
            input_types: vec![RuntimePureInputType::Value, RuntimePureInputType::Value],
            output_type: RuntimePureOutputType::Value,
            expr: RuntimeExpr::Value(reducer_value),
            scalar_eval_supported: false,
            origin: RuntimePureHelperOrigin::Annotated,
        },
    ]
}

fn stateful_plan_with(
    flow_ops: Vec<FlowOp>,
    state_schema: RuntimeTypeSchema,
    event_schema: RuntimeTypeSchema,
    initializer_value: RuntimeValue,
    initializer_output: RuntimePureOutputType,
    reducer_value: RuntimeValue,
    command_policy: RuntimeCommandPolicy,
) -> (RuntimePlan, crate::plan::EntryRuntimeId) {
    let entry = super::entry_id("entry.root_test");
    let flow = super::flow_id("flow.root_test");
    let state_layout = state_schema
        .try_layout_hash()
        .expect("state schema is valid");
    let event_layout = event_schema
        .try_layout_hash()
        .expect("event schema is valid");
    let initializer_id =
        RuntimeCallableId::try_new("root_test.initial_state").expect("callable ID is valid");
    let reducer_id = RuntimeCallableId::try_new("root_test.reduce").expect("callable ID is valid");
    let initializer_contract = CallableContractHash::from_bytes([1; 32]);
    let reducer_contract = CallableContractHash::from_bytes([2; 32]);
    let flow_contract = FlowContractHash::from_bytes([3; 32]);
    let plan = RuntimePlan::new(
        vec![RuntimeFlow {
            id: flow.clone(),
            ops: flow_ops,
        }],
        Vec::new(),
    )
    .expect("base plan is valid")
    .with_pure_helpers(root_helpers(
        initializer_value,
        initializer_output,
        reducer_value,
    ))
    .with_entries(vec![RuntimeEntrySpec {
        id: entry.clone(),
        kind: RuntimeEntryKind::Game,
        binding: EntryBindingIdentity::from_bytes([4; 32]),
        target: RuntimeEntryTarget::Flow(flow.clone()),
        roles: RuntimeEntryRoles::Stateful(Box::new(RuntimeStatefulEntryRoles {
            binding: EntryBindingIdentity::from_bytes([4; 32]),
            state: RuntimeNominalRole {
                identity: RuntimeNominalTypeId::try_new("RootState")
                    .expect("state identity is valid"),
                layout: state_layout,
                schema: state_schema,
            },
            initializer: RuntimeCallableRole {
                callable: initializer_id.clone(),
                contract: initializer_contract,
            },
            event: RuntimeNominalRole {
                identity: RuntimeNominalTypeId::try_new("RootEvent")
                    .expect("event identity is valid"),
                layout: event_layout,
                schema: event_schema,
            },
            reducer: RuntimeCallableRole {
                callable: reducer_id.clone(),
                contract: reducer_contract,
            },
            initial_flow: RuntimeFlowRole {
                flow: flow.clone(),
                contract: flow_contract,
            },
            command_policy,
        })),
    }])
    .with_entry_executables(
        vec![
            RuntimeCallableExecutable {
                callable: initializer_id,
                contract: initializer_contract,
                code: RuntimeCallableExecutableCode::PureHelper(RuntimePureHelperId(0)),
            },
            RuntimeCallableExecutable {
                callable: reducer_id,
                contract: reducer_contract,
                code: RuntimeCallableExecutableCode::PureHelper(RuntimePureHelperId(1)),
            },
        ],
        vec![RuntimeFlowExecutable {
            flow,
            contract: flow_contract,
            parameters: vec![RuntimeFlowExecutableParameter {
                position: 0,
                name: "flow_state".to_owned(),
                mode: RuntimeFlowParameterMode::Owned,
                nominal: RuntimeNominalTypeId::try_new("RootState")
                    .expect("state identity is valid"),
                layout: state_layout,
            }],
            controller: None,
        }],
    );
    plan.verify().expect("stateful plan verifies");
    (plan, entry)
}

fn reducer_rejection() -> RuntimeValue {
    RuntimeValue::result_err(nominal_variant(
        "ReducerError",
        "ReducerError",
        Some(RuntimeValue::Record(vec![
            RuntimeFieldValue {
                name: "code".to_owned(),
                value: RuntimeValue::String("not_allowed".to_owned()),
            },
            RuntimeFieldValue {
                name: "message".to_owned(),
                value: RuntimeValue::String("transition rejected".to_owned()),
            },
        ])),
    ))
}

fn command_value(payload: RuntimeValue) -> RuntimeValue {
    nominal_variant(
        "Command",
        "Command",
        Some(RuntimeValue::Record(vec![
            RuntimeFieldValue {
                name: "constructor".to_owned(),
                value: RuntimeValue::EntityRef("command.save".to_owned()),
            },
            RuntimeFieldValue {
                name: "target".to_owned(),
                value: RuntimeValue::EntityRef("save.primary".to_owned()),
            },
            RuntimeFieldValue {
                name: "payload".to_owned(),
                value: payload,
            },
        ])),
    )
}

fn string_command_policy(limits: RootExecutionLimits) -> RuntimeCommandPolicy {
    let payload_schema = RuntimeTypeSchema::String;
    RuntimeCommandPolicy::new(
        [RuntimeCommandContract {
            constructor: RuntimeCommandConstructorId::try_new("command.save")
                .expect("constructor identity"),
            target: RuntimeCommandTargetId::try_new("save.primary").expect("target identity"),
            payload_layout: payload_schema.try_layout_hash().expect("payload layout"),
            payload_schema,
        }],
        limits,
    )
}

struct FixedRootEvaluator {
    returned: RuntimeValue,
}

impl RootCallableEvaluator for FixedRootEvaluator {
    fn evaluate_root_callable(
        &mut self,
        _callable: &RuntimeCallableRole,
        _args: &[RuntimeValue],
    ) -> Result<RuntimeValue, RootCallableEvaluationError> {
        Ok(self.returned.clone())
    }
}

fn one_op() -> RuntimeStepOptions {
    RuntimeStepOptions {
        mode: RuntimeStepMode::OneOp,
        budget: RuntimeStepBudget { max_ops: 1 },
    }
}

#[test]
fn verified_plan_rejects_a_tampered_top_level_entry_binding() {
    let (mut plan, entry) = stateful_plan(vec![FlowOp::Noop]);
    plan.entries[0].binding = EntryBindingIdentity::from_bytes([5; 32]);

    assert_eq!(
        plan.verify(),
        Err(RuntimePlanError::EntryBindingMismatch {
            entry: entry.canonical_label(),
        })
    );
}

#[test]
fn run_013_verifier_rejects_missing_callable_schema_and_flow_roles() {
    let (plan, entry) = stateful_plan(vec![FlowOp::Noop]);

    let mut missing_callable = plan.clone();
    let missing_initializer = missing_callable.callable_executables.remove(0);
    assert_eq!(
        missing_callable.verify(),
        Err(RuntimePlanError::MissingCallable {
            entry: entry.canonical_label(),
            role: "initializer",
            callable: missing_initializer.callable.as_str().to_owned(),
        })
    );

    let mut mismatched_schema = plan.clone();
    let RuntimeEntryRoles::Stateful(roles) = &mut mismatched_schema.entries[0].roles else {
        panic!("fixture is stateful");
    };
    roles.state.schema = RuntimeTypeSchema::String;
    assert_eq!(
        mismatched_schema.verify(),
        Err(RuntimePlanError::StateLayoutMismatch {
            entry: entry.canonical_label(),
        })
    );

    let mut missing_flow = plan;
    missing_flow.flow_executables.clear();
    assert_eq!(
        missing_flow.verify(),
        Err(RuntimePlanError::InitialFlowContractMismatch {
            entry: entry.canonical_label(),
        })
    );
}

#[test]
fn run_014_engine_never_chooses_the_first_flow_without_explicit_selection() {
    let first_flow = super::flow_id("flow.first");
    let second_flow = super::flow_id("flow.second");
    let first_entry = super::entry_id("entry.first");
    let second_entry = super::entry_id("entry.second");
    let plan = RuntimePlan::new(
        vec![
            RuntimeFlow {
                id: first_flow.clone(),
                ops: vec![FlowOp::Noop],
            },
            RuntimeFlow {
                id: second_flow.clone(),
                ops: vec![FlowOp::Return("second".to_owned())],
            },
        ],
        Vec::new(),
    )
    .expect("base plan")
    .with_entries(vec![
        RuntimeEntrySpec {
            id: first_entry,
            kind: RuntimeEntryKind::Cli,
            binding: EntryBindingIdentity::from_bytes([10; 32]),
            target: RuntimeEntryTarget::Flow(first_flow),
            roles: RuntimeEntryRoles::None,
        },
        RuntimeEntrySpec {
            id: second_entry.clone(),
            kind: RuntimeEntryKind::Cli,
            binding: EntryBindingIdentity::from_bytes([11; 32]),
            target: RuntimeEntryTarget::Flow(second_flow),
            roles: RuntimeEntryRoles::None,
        },
    ]);
    plan.verify().expect("explicit multi-entry plan verifies");

    let mut engine = Engine::new(plan);
    assert_eq!(engine.fiber().cursor, None);

    engine
        .start_entry(&second_entry)
        .expect("the exact second entry is selected");
    assert_eq!(
        engine.fiber().cursor,
        Some(crate::engine::FlowCursor {
            flow_index: 1,
            op_index: 0,
        })
    );
}

#[test]
fn invalid_root_event_rejects_the_step_without_failing_the_runtime() {
    let (plan, entry) = stateful_plan(vec![FlowOp::Noop, FlowOp::Noop]);
    let mut engine = Engine::for_entry(plan, &entry).expect("stateful entry starts");
    let before_cursor = engine.fiber().cursor;

    let rejected = engine.step(
        RuntimeStepInput {
            root_events: vec![RootEventInput::new(RuntimePayload(RuntimeValue::String(
                "wrong event shape".to_owned(),
            )))],
            ..RuntimeStepInput::default()
        },
        one_op(),
    );

    assert_eq!(engine.fiber().status, FlowFiberStatus::Running);
    assert_eq!(engine.fiber().cursor, before_cursor);
    assert_eq!(
        engine
            .root()
            .expect("root remains active")
            .active()
            .next_sequence
            .get(),
        0
    );
    assert!(
        rejected
            .output
            .diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.category == RuntimeDiagnosticCategory::Input })
    );

    let resumed = engine.step(RuntimeStepInput::default(), one_op());
    assert_eq!(resumed.stats.executed_ops, 1);
    assert_eq!(engine.fiber().status, FlowFiberStatus::Running);
}

#[test]
fn non_stateful_entry_rejects_root_events_without_poisoning_the_fiber() {
    let plan = super::runtime_plan(
        Some(super::flow_id("flow.main")),
        vec![RuntimeFlow {
            id: super::flow_id("flow.main"),
            ops: vec![FlowOp::Noop, FlowOp::Noop],
        }],
        Vec::new(),
    )
    .expect("non-stateful plan is valid");
    let mut engine = super::engine_for_test_plan(plan);
    let before_cursor = engine.fiber().cursor;

    let rejected = engine.step(
        RuntimeStepInput {
            root_events: vec![RootEventInput::new(RuntimePayload(RuntimeValue::i64(1)))],
            ..RuntimeStepInput::default()
        },
        one_op(),
    );

    assert_eq!(engine.fiber().status, FlowFiberStatus::Running);
    assert_eq!(engine.fiber().cursor, before_cursor);
    assert!(
        rejected
            .output
            .diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.category == RuntimeDiagnosticCategory::Input })
    );
    assert_eq!(
        engine
            .step(RuntimeStepInput::default(), one_op())
            .stats
            .executed_ops,
        1
    );
}

#[test]
fn initial_flow_owns_a_value_copy_independent_from_durable_root_state() {
    let (plan, entry) = stateful_plan(vec![
        FlowOp::Bind(vec![RuntimeBinding {
            name: "flow_state".to_owned(),
            value: RuntimeValue::i64(99),
        }]),
        FlowOp::Noop,
    ]);
    let mut engine = Engine::for_entry(plan, &entry).expect("stateful entry starts");

    engine.step(RuntimeStepInput::default(), one_op());
    assert_eq!(
        engine.fiber().env.get("flow_state"),
        Some(&RuntimeValue::i64(99))
    );
    assert_eq!(
        engine.root().expect("root is active").active().value,
        RuntimePayload(RuntimeValue::i64(1))
    );

    let reduced = engine.step(
        RuntimeStepInput {
            root_events: vec![RootEventInput::new(RuntimePayload(RuntimeValue::i64(7)))],
            ..RuntimeStepInput::default()
        },
        one_op(),
    );

    assert_eq!(reduced.output.root_transitions.len(), 1);
    assert_eq!(
        engine.fiber().env.get("flow_state"),
        Some(&RuntimeValue::i64(99))
    );
    assert_eq!(
        engine.root().expect("root is active").active().value,
        RuntimePayload(RuntimeValue::i64(2))
    );
}

#[test]
fn run_001_initializer_state_is_installed_before_initial_flow_execution() {
    let (plan, entry) = stateful_plan(vec![FlowOp::Noop]);
    let mut engine = Engine::for_entry(plan, &entry).expect("stateful entry starts");

    assert_eq!(
        engine.root().expect("root is active").active().value,
        RuntimePayload(RuntimeValue::i64(1))
    );
    assert_eq!(
        engine.fiber().env.get("flow_state"),
        Some(&RuntimeValue::i64(1))
    );
    assert_eq!(
        engine
            .root()
            .expect("root is active")
            .active()
            .next_sequence,
        TransitionSequence::ZERO
    );

    engine.step(RuntimeStepInput::default(), one_op());
    assert_eq!(
        engine.root().expect("root is active").active().value,
        RuntimePayload(RuntimeValue::i64(1))
    );
}

#[test]
fn run_002_invalid_initializer_value_aborts_entry_start() {
    let (plan, entry) = stateful_plan_with(
        vec![FlowOp::Noop],
        RuntimeTypeSchema::I64,
        RuntimeTypeSchema::I64,
        RuntimeValue::String("not an i64 state".to_owned()),
        RuntimePureOutputType::Value,
        reducer_ok(RuntimeValue::i64(2)),
        RuntimeCommandPolicy::deny_all(RootExecutionLimits::engine_default()),
    );

    let error = Engine::for_entry(plan, &entry).expect_err("invalid initializer must abort");
    assert!(error.to_string().contains("initial"));
}

#[test]
fn run_003_same_ordered_batch_has_identical_outcomes_and_final_state() {
    let (plan, entry) = stateful_plan(vec![FlowOp::Noop]);
    let events = vec![
        RootEventInput::new(RuntimePayload(RuntimeValue::i64(10))),
        RootEventInput::new(RuntimePayload(RuntimeValue::i64(11))),
        RootEventInput::new(RuntimePayload(RuntimeValue::i64(12))),
    ];
    let mut first = Engine::for_entry(plan.clone(), &entry).expect("first entry starts");
    let mut second = Engine::for_entry(plan, &entry).expect("second entry starts");

    let first_step = first.step(
        RuntimeStepInput {
            root_events: events.clone(),
            ..RuntimeStepInput::default()
        },
        one_op(),
    );
    let second_step = second.step(
        RuntimeStepInput {
            root_events: events,
            ..RuntimeStepInput::default()
        },
        one_op(),
    );

    assert_eq!(
        first_step.output.root_transitions,
        second_step.output.root_transitions
    );
    assert_eq!(
        first.root().expect("first root").snapshot_state(),
        second.root().expect("second root").snapshot_state()
    );
    assert_eq!(
        first_step
            .output
            .root_transitions
            .iter()
            .map(RootTransitionOutcome::sequence)
            .collect::<Vec<_>>(),
        vec![
            TransitionSequence::ZERO,
            TransitionSequence::from_u64(1),
            TransitionSequence::from_u64(2),
        ]
    );
}

#[test]
fn run_004_committed_commands_preserve_reducer_vector_order() {
    let limits = RootExecutionLimits::engine_default();
    let reducer = reducer_ok_with_commands(
        RuntimeValue::i64(2),
        vec![
            command_value(RuntimeValue::String("first".to_owned())),
            command_value(RuntimeValue::String("second".to_owned())),
        ],
    );
    let (plan, entry) = stateful_plan_with(
        vec![FlowOp::Noop],
        RuntimeTypeSchema::I64,
        RuntimeTypeSchema::I64,
        RuntimeValue::i64(1),
        RuntimePureOutputType::I64,
        reducer,
        string_command_policy(limits),
    );
    let mut engine = Engine::for_entry(plan, &entry).expect("stateful entry starts");

    let step = engine.step(
        RuntimeStepInput {
            root_events: vec![
                RootEventInput::new(RuntimePayload(RuntimeValue::i64(7))),
                RootEventInput::new(RuntimePayload(RuntimeValue::i64(8))),
            ],
            ..RuntimeStepInput::default()
        },
        one_op(),
    );

    assert_eq!(
        step.output
            .root_commands
            .iter()
            .map(|command| (command.transition, command.index))
            .collect::<Vec<_>>(),
        vec![
            (TransitionSequence::ZERO, 0),
            (TransitionSequence::ZERO, 1),
            (TransitionSequence::from_u64(1), 0),
            (TransitionSequence::from_u64(1), 1),
        ]
    );
    assert_eq!(
        step.output.root_commands[0].command.payload(),
        &RuntimePayload(RuntimeValue::String("first".to_owned()))
    );
    assert_eq!(
        step.output.root_commands[1].command.payload(),
        &RuntimePayload(RuntimeValue::String("second".to_owned()))
    );
    let RootTransitionOutcome::Committed {
        command_digests, ..
    } = &step.output.root_transitions[0]
    else {
        panic!("reducer commits");
    };
    assert_eq!(command_digests.len(), 2);
    let RootTransitionOutcome::Committed {
        command_digests, ..
    } = &step.output.root_transitions[1]
    else {
        panic!("second reducer commits");
    };
    assert_eq!(command_digests.len(), 2);
    assert_eq!(
        engine.root().expect("root remains active").save_blockers(),
        crate::root::RootSaveBlockers {
            reducer_active: false,
            pending_events: 0,
            pending_commands: 4,
        }
    );
}

#[test]
fn run_006_018_rejection_rolls_back_consumes_one_sequence_and_preserves_later_event() {
    let (plan, entry) = stateful_plan_with(
        vec![FlowOp::Noop, FlowOp::Noop],
        RuntimeTypeSchema::I64,
        RuntimeTypeSchema::I64,
        RuntimeValue::i64(1),
        RuntimePureOutputType::I64,
        reducer_rejection(),
        RuntimeCommandPolicy::deny_all(RootExecutionLimits::engine_default()),
    );
    let mut engine = Engine::for_entry(plan, &entry).expect("stateful entry starts");

    let first = engine.step(
        RuntimeStepInput {
            root_events: vec![
                RootEventInput::new(RuntimePayload(RuntimeValue::i64(7))),
                RootEventInput::new(RuntimePayload(RuntimeValue::i64(8))),
            ],
            ..RuntimeStepInput::default()
        },
        one_op(),
    );

    assert!(matches!(
        first.output.root_transitions.as_slice(),
        [RootTransitionOutcome::Rejected { sequence, .. }]
            if *sequence == TransitionSequence::ZERO
    ));
    assert_eq!(first.stats.executed_ops, 1);
    let root = engine.root().expect("root remains active");
    assert_eq!(root.active().value, RuntimePayload(RuntimeValue::i64(1)));
    assert_eq!(root.active().next_sequence, TransitionSequence::from_u64(1));
    assert_eq!(root.save_blockers().pending_events, 1);

    let second = engine.step(RuntimeStepInput::default(), one_op());
    assert!(matches!(
        second.output.root_transitions.as_slice(),
        [RootTransitionOutcome::Rejected { sequence, .. }]
            if *sequence == TransitionSequence::from_u64(1)
    ));
    assert_eq!(second.stats.executed_ops, 1);
    assert_eq!(
        engine
            .root()
            .expect("root remains active")
            .save_blockers()
            .pending_events,
        0
    );
}

#[test]
fn run_007_reducer_trap_has_no_partial_commit_and_skips_later_phases() {
    let (plan, entry) = stateful_plan_with(
        vec![FlowOp::Noop],
        RuntimeTypeSchema::I64,
        RuntimeTypeSchema::I64,
        RuntimeValue::i64(1),
        RuntimePureOutputType::I64,
        RuntimeValue::String("malformed reducer result".to_owned()),
        RuntimeCommandPolicy::deny_all(RootExecutionLimits::engine_default()),
    );
    let mut engine = Engine::for_entry(plan, &entry).expect("stateful entry starts");
    let before_cursor = engine.fiber().cursor;

    let trapped = engine.step(
        RuntimeStepInput {
            root_events: vec![
                RootEventInput::new(RuntimePayload(RuntimeValue::i64(7))),
                RootEventInput::new(RuntimePayload(RuntimeValue::i64(8))),
            ],
            deferred_root_events: vec![RootEventInput::new(RuntimePayload(RuntimeValue::i64(9)))],
            ..RuntimeStepInput::default()
        },
        one_op(),
    );

    assert!(matches!(
        trapped.output.root_transitions.as_slice(),
        [RootTransitionOutcome::Trapped { sequence, .. }]
            if *sequence == TransitionSequence::ZERO
    ));
    assert!(trapped.output.root_commands.is_empty());
    assert!(trapped.output.requests.root_events_next_step.is_empty());
    assert_eq!(trapped.stats.executed_ops, 0);
    assert_eq!(engine.fiber().cursor, before_cursor);
    assert!(matches!(engine.fiber().status, FlowFiberStatus::Failed(_)));
    let root = engine.root().expect("failed root remains inspectable");
    assert_eq!(root.active().value, RuntimePayload(RuntimeValue::i64(1)));
    assert_eq!(root.active().next_sequence, TransitionSequence::ZERO);
    assert_eq!(root.save_blockers().pending_events, 0);
    assert_eq!(root.save_blockers().pending_commands, 0);
}

#[test]
fn run_008_non_finite_state_and_runtime_handle_trap_without_partial_commit() {
    let cases = [
        (
            RuntimeTypeSchema::F64,
            RuntimeValue::F64(1.0),
            RuntimePureOutputType::F64,
            RuntimeValue::F64(f64::NAN),
        ),
        (
            RuntimeTypeSchema::I64,
            RuntimeValue::i64(1),
            RuntimePureOutputType::I64,
            RuntimeValue::Function(RuntimeFunctionValue::new(
                Vec::new(),
                RuntimeExpr::Value(RuntimeValue::Unit),
                Vec::new(),
            )),
        ),
    ];

    for (state_schema, initial_state, initializer_output, invalid_state) in cases {
        let expected_state = RuntimePayload(initial_state.clone());
        let (plan, entry) = stateful_plan_with(
            vec![FlowOp::Noop],
            state_schema,
            RuntimeTypeSchema::I64,
            initial_state,
            initializer_output,
            reducer_ok(invalid_state),
            RuntimeCommandPolicy::deny_all(RootExecutionLimits::engine_default()),
        );
        let mut engine = Engine::for_entry(plan, &entry).expect("stateful entry starts");
        let trapped = engine.step(
            RuntimeStepInput {
                root_events: vec![RootEventInput::new(RuntimePayload(RuntimeValue::i64(7)))],
                ..RuntimeStepInput::default()
            },
            one_op(),
        );

        assert!(matches!(
            trapped.output.root_transitions.as_slice(),
            [RootTransitionOutcome::Trapped { .. }]
        ));
        let root = engine.root().expect("root remains inspectable");
        assert_eq!(root.active().value, expected_state);
        assert_eq!(root.active().next_sequence, TransitionSequence::ZERO);
        assert_eq!(root.save_blockers().pending_commands, 0);
    }
}

#[test]
fn run_010_mutable_initial_flow_state_parameter_is_rejected_by_verification() {
    let (mut plan, entry) = stateful_plan(vec![FlowOp::Noop]);
    plan.flow_executables[0].parameters[0].mode = RuntimeFlowParameterMode::Mutable;

    assert_eq!(
        plan.verify(),
        Err(RuntimePlanError::InvalidInitialFlowStateParameter {
            entry: entry.canonical_label(),
        })
    );
}

#[test]
fn run_016_mixed_valid_and_invalid_ingress_batch_is_rejected_atomically() {
    let (plan, entry) = stateful_plan(vec![FlowOp::Noop]);
    let mut engine = Engine::for_entry(plan, &entry).expect("stateful entry starts");
    let before_cursor = engine.fiber().cursor;

    let rejected = engine.step(
        RuntimeStepInput {
            root_events: vec![
                RootEventInput::new(RuntimePayload(RuntimeValue::i64(7))),
                RootEventInput::new(RuntimePayload(RuntimeValue::String("invalid".to_owned()))),
            ],
            ..RuntimeStepInput::default()
        },
        one_op(),
    );

    assert!(rejected.output.root_transitions.is_empty());
    assert_eq!(rejected.stats.executed_ops, 0);
    assert_eq!(engine.fiber().cursor, before_cursor);
    assert_eq!(engine.fiber().status, FlowFiberStatus::Running);
    let root = engine.root().expect("root remains active");
    assert_eq!(root.active().value, RuntimePayload(RuntimeValue::i64(1)));
    assert_eq!(root.active().next_sequence, TransitionSequence::ZERO);
    assert_eq!(root.save_blockers().pending_events, 0);
}

#[test]
fn run_017_queue_limit_is_an_atomic_input_rejection() {
    let limits = RootExecutionLimits {
        max_pending_events: 2,
        ..RootExecutionLimits::engine_default()
    };
    let (plan, entry) = stateful_plan_with(
        vec![FlowOp::Noop],
        RuntimeTypeSchema::I64,
        RuntimeTypeSchema::I64,
        RuntimeValue::i64(1),
        RuntimePureOutputType::I64,
        reducer_ok(RuntimeValue::i64(2)),
        RuntimeCommandPolicy::deny_all(limits),
    );
    let mut engine = Engine::for_entry(plan, &entry).expect("stateful entry starts");
    let before_cursor = engine.fiber().cursor;

    let rejected = engine.step(
        RuntimeStepInput {
            root_events: vec![
                RootEventInput::new(RuntimePayload(RuntimeValue::i64(7))),
                RootEventInput::new(RuntimePayload(RuntimeValue::i64(8))),
                RootEventInput::new(RuntimePayload(RuntimeValue::i64(9))),
            ],
            ..RuntimeStepInput::default()
        },
        one_op(),
    );

    assert!(rejected.output.root_transitions.is_empty());
    assert_eq!(rejected.stats.executed_ops, 0);
    assert_eq!(engine.fiber().cursor, before_cursor);
    assert_eq!(engine.fiber().status, FlowFiberStatus::Running);
    let root = engine.root().expect("root remains active");
    assert_eq!(root.active().next_sequence, TransitionSequence::ZERO);
    assert_eq!(root.save_blockers().pending_events, 0);
}

#[test]
fn run_012_017_transition_sequence_exhaustion_is_atomic_and_not_caller_controlled() {
    let (plan, entry) = stateful_plan(vec![FlowOp::Noop]);
    let contract =
        RootStartupContract::from_runtime_plan(&plan, &entry).expect("startup contract verifies");
    let engine = Engine::for_entry(plan, &entry).expect("stateful entry starts");
    let mut snapshot = engine.root().expect("root is active").snapshot_state();
    snapshot.next_sequence = TransitionSequence::from_u64(u64::MAX - 1);
    let mut root = RootRuntime::from_snapshot(contract, snapshot).expect("snapshot restores");
    let mut evaluator = FixedRootEvaluator {
        returned: reducer_ok(RuntimeValue::i64(2)),
    };

    assert_eq!(
        root.step(
            vec![
                RootEventInput::new(RuntimePayload(RuntimeValue::i64(7))),
                RootEventInput::new(RuntimePayload(RuntimeValue::i64(8))),
            ],
            &mut evaluator,
        ),
        Err(RootRuntimeError::TransitionSequenceExhausted)
    );
    assert_eq!(
        root.active().next_sequence,
        TransitionSequence::from_u64(u64::MAX - 1)
    );
    assert_eq!(root.save_blockers().pending_events, 0);

    let final_admissible = root
        .step(
            vec![RootEventInput::new(RuntimePayload(RuntimeValue::i64(9)))],
            &mut evaluator,
        )
        .expect("penultimate sequence is admitted");
    assert_eq!(
        final_admissible.outcomes[0].sequence(),
        TransitionSequence::from_u64(u64::MAX - 1)
    );
    assert_eq!(root.active().next_sequence, TransitionSequence::TERMINAL);
    assert_eq!(
        root.step(
            vec![RootEventInput::new(RuntimePayload(RuntimeValue::i64(10)))],
            &mut evaluator,
        ),
        Err(RootRuntimeError::TransitionSequenceExhausted)
    );
}
