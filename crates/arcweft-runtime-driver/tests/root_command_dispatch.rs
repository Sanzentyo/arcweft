use arcweft_bundle::resource_codec::SourceMapSection;
use arcweft_bundle::{ArcweftBundle, BundleManifest, BundleRuntimeSummary};
use arcweft_core::bytecode::BytecodeProgram;
use arcweft_core::entry::{
    CallableContractHash, EntryBindingIdentity, FlowContractHash, RootExecutionLimits,
    RuntimeCallableExecutable, RuntimeCallableExecutableCode, RuntimeCallableId,
    RuntimeCallableRole, RuntimeCommandConstructorId, RuntimeCommandContract, RuntimeCommandPolicy,
    RuntimeCommandTargetId, RuntimeEntryRoles, RuntimeFlowExecutable,
    RuntimeFlowExecutableParameter, RuntimeFlowParameterMode, RuntimeFlowRole, RuntimeNominalRole,
    RuntimeNominalTypeId, RuntimeStatefulEntryRoles, RuntimeTypeSchema, RuntimeValueDigest,
    TypeLayoutHash,
};
use arcweft_core::pattern::{
    RuntimeCheckedType, RuntimeCheckedVariantCase, RuntimePattern, RuntimeSemanticTypeId,
};
use arcweft_core::plan::{
    EntryRuntimeId, FlowOp, FlowRuntimeId, RuntimeEntryKind, RuntimeEntrySpec, RuntimeEntryTarget,
    RuntimeFlow, RuntimeHostCallTarget, RuntimePlan, RuntimePureHelper, RuntimePureHelperId,
    RuntimePureHelperOrigin, RuntimePureInputType, RuntimePureOutputType,
};
use arcweft_core::root::{RootEventInput, TransitionSequence};
use arcweft_core::step::{
    RuntimeHostCallError, RuntimeHostCallErrorKind, RuntimeHostCallMode, RuntimeHostCallResult,
};
use arcweft_core::value::{RuntimeExpr, RuntimeFieldExpr, RuntimePayload, RuntimeValue};
use arcweft_runtime_driver::clock::RuntimeClockStep;
use arcweft_runtime_driver::session::{
    BundleEntryStart, BundleHotSwapError, BundleSession, BundleSessionError, BundleSessionOptions,
    BundleStepInput, RecordedExternalOutcomePositionV1, RootCommandHostArgument,
    RootCommandHostCallBinding, RootCommandHostCallCatalog, RootCommandHostCallCatalogError,
    RootCommandHostCallEndpoint, RootCommandHostResultRoute, RootReplayError,
    RootReplayRecordingError,
};
use arcweft_runtime_driver::session_save::{BundleSessionPendingBlocker, BundleSessionSaveError};
use arcweft_runtime_plan::awbc_lower::AwbcLowerer;
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};
use arcweft_text_model::DialogueContentCatalog;

const ENTRY: &str = "entry.root_commands";
const FLOW: &str = "flow.root_commands";
const CONSTRUCTOR: &str = "command.save";
const TARGET: &str = "save.slot.primary";

fn fixture_runtime_artifact_fingerprint() -> arcweft_core::effect::RuntimeArtifactFingerprint {
    arcweft_core::effect::RuntimeArtifactFingerprint::try_from_bytes([0x6a; 32])
        .expect("fixture runtime artifact fingerprint is non-zero")
}

fn entry_id() -> EntryRuntimeId {
    EntryRuntimeId::from_source_entity_body(ENTRY).expect("entry ID")
}

fn flow_id() -> FlowRuntimeId {
    FlowRuntimeId::from_runtime_target_value(FLOW).expect("flow ID")
}

fn constructor_id() -> RuntimeCommandConstructorId {
    RuntimeCommandConstructorId::try_new(CONSTRUCTOR).expect("constructor ID")
}

fn target_id() -> RuntimeCommandTargetId {
    RuntimeCommandTargetId::try_new(TARGET).expect("target ID")
}

fn command_expr(payload: RuntimeValue) -> RuntimeExpr {
    RuntimeExpr::Variant {
        owner: nominal_variant_type("Command", "Command", "CommandPayload"),
        ordinal: 0,
        name: "Command".to_owned(),
        payload: Some(Box::new(RuntimeExpr::Record(vec![
            RuntimeFieldExpr {
                name: "constructor".to_owned(),
                value: RuntimeExpr::Value(RuntimeValue::EntityRef(CONSTRUCTOR.to_owned())),
            },
            RuntimeFieldExpr {
                name: "target".to_owned(),
                value: RuntimeExpr::Value(RuntimeValue::EntityRef(TARGET.to_owned())),
            },
            RuntimeFieldExpr {
                name: "payload".to_owned(),
                value: RuntimeExpr::Value(payload),
            },
        ]))),
    }
}

fn reducer_ok_with_command() -> RuntimeExpr {
    reducer_ok_with_commands([RuntimeValue::String("checkpoint".to_owned())])
}

fn reducer_ok_with_commands(payloads: impl IntoIterator<Item = RuntimeValue>) -> RuntimeExpr {
    let reduction = RuntimeExpr::Variant {
        owner: reduction_type(),
        ordinal: 0,
        name: "Reduction".to_owned(),
        payload: Some(Box::new(RuntimeExpr::Record(vec![
            RuntimeFieldExpr {
                name: "state".to_owned(),
                value: RuntimeExpr::Value(RuntimeValue::i64(2)),
            },
            RuntimeFieldExpr {
                name: "commands".to_owned(),
                value: RuntimeExpr::BracketSeq(payloads.into_iter().map(command_expr).collect()),
            },
        ]))),
    };
    RuntimeExpr::Variant {
        owner: reducer_result_type(),
        ordinal: 0,
        name: "Ok".to_owned(),
        payload: Some(Box::new(reduction)),
    }
}

fn command_bundle(include_flow_host_call: bool) -> ArcweftBundle {
    command_bundle_with_root(
        include_flow_host_call,
        RuntimeValue::i64(1),
        reducer_ok_with_command(),
    )
}

fn reducer_rejection() -> RuntimeExpr {
    let rejection = RuntimeExpr::Variant {
        owner: reducer_error_type(),
        ordinal: 0,
        name: "ReducerError".to_owned(),
        payload: Some(Box::new(RuntimeExpr::Record(vec![
            RuntimeFieldExpr {
                name: "code".to_owned(),
                value: RuntimeExpr::Value(RuntimeValue::String("not_allowed".to_owned())),
            },
            RuntimeFieldExpr {
                name: "message".to_owned(),
                value: RuntimeExpr::Value(RuntimeValue::String("transition rejected".to_owned())),
            },
        ]))),
    };
    RuntimeExpr::Variant {
        owner: reducer_result_type(),
        ordinal: 1,
        name: "Err".to_owned(),
        payload: Some(Box::new(rejection)),
    }
}

fn reducer_result_type() -> RuntimeCheckedType {
    RuntimeCheckedType::Result {
        ok: Box::new(reduction_type()),
        error: Box::new(reducer_error_type()),
    }
}

fn reduction_type() -> RuntimeCheckedType {
    nominal_variant_type("Reduction", "Reduction", "ReductionPayload")
}

fn reducer_error_type() -> RuntimeCheckedType {
    nominal_variant_type("ReducerError", "ReducerError", "ReducerErrorPayload")
}

fn nominal_variant_type(owner: &str, case: &str, payload: &str) -> RuntimeCheckedType {
    RuntimeCheckedType::Variant {
        nominal: RuntimeNominalTypeId::try_new(owner).expect("nominal variant owner"),
        semantic_identity: RuntimeSemanticTypeId::from_bytes([0x52; 32]),
        cases: vec![RuntimeCheckedVariantCase {
            name: case.to_owned(),
            payload: Some(Box::new(RuntimeCheckedType::Nominal {
                nominal: RuntimeNominalTypeId::try_new(payload).expect("nominal payload owner"),
                semantic_identity: RuntimeSemanticTypeId::from_bytes([0x53; 32]),
                layout: TypeLayoutHash::from_bytes([0x54; 32]),
            })),
        }],
    }
}

struct RootFixtureContract {
    state_schema: RuntimeTypeSchema,
    event_schema: RuntimeTypeSchema,
    command_schema: RuntimeTypeSchema,
    state_layout: TypeLayoutHash,
    event_layout: TypeLayoutHash,
    command_layout: TypeLayoutHash,
    initializer_id: RuntimeCallableId,
    reducer_id: RuntimeCallableId,
    initializer_contract: CallableContractHash,
    reducer_contract: CallableContractHash,
    flow_contract: FlowContractHash,
}

impl RootFixtureContract {
    fn new() -> Self {
        let state_schema = RuntimeTypeSchema::I64;
        let event_schema = RuntimeTypeSchema::I64;
        let command_schema = RuntimeTypeSchema::String;
        Self {
            state_layout: state_schema.try_layout_hash().expect("state layout"),
            event_layout: event_schema.try_layout_hash().expect("event layout"),
            command_layout: command_schema.try_layout_hash().expect("command layout"),
            state_schema,
            event_schema,
            command_schema,
            initializer_id: RuntimeCallableId::try_new("root_commands.initial")
                .expect("initializer ID"),
            reducer_id: RuntimeCallableId::try_new("root_commands.reduce").expect("reducer ID"),
            initializer_contract: CallableContractHash::from_bytes([1; 32]),
            reducer_contract: CallableContractHash::from_bytes([2; 32]),
            flow_contract: FlowContractHash::from_bytes([3; 32]),
        }
    }

    fn entry(&self) -> RuntimeEntrySpec {
        RuntimeEntrySpec {
            id: entry_id(),
            kind: RuntimeEntryKind::Game,
            binding: EntryBindingIdentity::from_bytes([4; 32]),
            target: RuntimeEntryTarget::Flow(flow_id()),
            roles: RuntimeEntryRoles::Stateful(Box::new(RuntimeStatefulEntryRoles {
                binding: EntryBindingIdentity::from_bytes([4; 32]),
                state: RuntimeNominalRole {
                    identity: RuntimeNominalTypeId::try_new("RootState").expect("state ID"),
                    layout: self.state_layout,
                    schema: self.state_schema.clone(),
                },
                initializer: RuntimeCallableRole {
                    callable: self.initializer_id.clone(),
                    contract: self.initializer_contract,
                },
                event: RuntimeNominalRole {
                    identity: RuntimeNominalTypeId::try_new("RootEvent").expect("event ID"),
                    layout: self.event_layout,
                    schema: self.event_schema.clone(),
                },
                reducer: RuntimeCallableRole {
                    callable: self.reducer_id.clone(),
                    contract: self.reducer_contract,
                },
                initial_flow: RuntimeFlowRole {
                    flow: flow_id(),
                    contract: self.flow_contract,
                },
                command_policy: RuntimeCommandPolicy::new(
                    [RuntimeCommandContract {
                        constructor: constructor_id(),
                        target: target_id(),
                        payload_layout: self.command_layout,
                        payload_schema: self.command_schema.clone(),
                    }],
                    RootExecutionLimits::engine_default(),
                ),
            })),
        }
    }

    fn callable_executables(&self) -> Vec<RuntimeCallableExecutable> {
        vec![
            RuntimeCallableExecutable {
                callable: self.initializer_id.clone(),
                contract: self.initializer_contract,
                code: RuntimeCallableExecutableCode::PureHelper(RuntimePureHelperId(0)),
            },
            RuntimeCallableExecutable {
                callable: self.reducer_id.clone(),
                contract: self.reducer_contract,
                code: RuntimeCallableExecutableCode::PureHelper(RuntimePureHelperId(1)),
            },
        ]
    }

    fn flow_executable(&self) -> RuntimeFlowExecutable {
        RuntimeFlowExecutable {
            flow: flow_id(),
            contract: self.flow_contract,
            parameters: vec![RuntimeFlowExecutableParameter {
                position: 0,
                name: "state".to_owned(),
                mode: RuntimeFlowParameterMode::Owned,
                nominal: RuntimeNominalTypeId::try_new("RootState").expect("state ID"),
                layout: self.state_layout,
            }],
            controller: None,
        }
    }
}

fn command_bundle_with_root(
    include_flow_host_call: bool,
    initializer_value: RuntimeValue,
    reducer_expr: RuntimeExpr,
) -> ArcweftBundle {
    let contract = RootFixtureContract::new();
    let flow_ops = if include_flow_host_call {
        vec![
            FlowOp::HostCall {
                binding: Some(RuntimePattern::Ident("probe".to_owned())),
                target: RuntimeHostCallTarget::new(
                    "flow.probe",
                    "flow",
                    "probe",
                    [RuntimeExpr::Local("state".to_owned())],
                    RuntimeHostCallMode::Suspend,
                    true,
                ),
            },
            FlowOp::ReturnExpr(RuntimeExpr::Local("probe".to_owned())),
        ]
    } else {
        vec![FlowOp::ReturnExpr(RuntimeExpr::Local("state".to_owned()))]
    };
    let plan = RuntimePlan::new(
        vec![RuntimeFlow {
            id: flow_id(),
            ops: flow_ops,
        }],
        Vec::new(),
    )
    .expect("base plan")
    .with_pure_helpers(vec![
        RuntimePureHelper {
            id: RuntimePureHelperId(0),
            name: "initial".to_owned(),
            input_names: Vec::new(),
            input_types: Vec::new(),
            output_type: RuntimePureOutputType::I64,
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
            expr: reducer_expr,
            scalar_eval_supported: false,
            origin: RuntimePureHelperOrigin::Annotated,
        },
    ])
    .with_entries(vec![contract.entry()])
    .with_entry_executables(
        contract.callable_executables(),
        vec![contract.flow_executable()],
    );
    plan.verify().expect("stateful plan verifies");
    let dialogue_content = DialogueContentCatalog::new();
    let product_awbc = AwbcLowerer::new(&plan, &dialogue_content, "root-commands.arcw")
        .lower()
        .expect("AWBC lowers")
        .program;
    let stats = BytecodeProgram::from_runtime_plan(plan.clone()).stats();
    ArcweftBundle::try_new(
        BundleManifest {
            profile_id: None,
            profile_kind: None,
            entry: Some(ENTRY.to_owned()),
            adapter: None,
            adapter_manifest_ids: Vec::new(),
            required_host_calls: Vec::new(),
            runtime: BundleRuntimeSummary {
                artifact_fingerprint: fixture_runtime_artifact_fingerprint(),
                entry_flow: Some(FLOW.to_owned()),
                flows: stats.flows,
                bytecode_instructions: stats.instructions,
                line_task_groups: stats.line_task_groups,
                stream_plans: stats.stream_plans,
                source_plans: stats.source_plans,
            },
        },
        source_map(),
        BytecodeProgram::from_runtime_plan(plan),
        dialogue_content,
    )
    .expect("stateful bundle source map accepts the generated standard Style source")
    .with_product_awbc(product_awbc)
}

fn non_stateful_bundle() -> (ArcweftBundle, EntryRuntimeId) {
    let entry = EntryRuntimeId::from_source_entity_body("entry.non_stateful").expect("entry ID");
    let flow = FlowRuntimeId::from_runtime_target_value("flow.non_stateful").expect("flow ID");
    let plan = RuntimePlan::new(
        vec![RuntimeFlow {
            id: flow.clone(),
            ops: vec![FlowOp::ReturnExpr(RuntimeExpr::Value(RuntimeValue::Unit))],
        }],
        Vec::new(),
    )
    .expect("base plan")
    .with_entries(vec![RuntimeEntrySpec {
        id: entry.clone(),
        kind: RuntimeEntryKind::Cli,
        binding: EntryBindingIdentity::from_bytes([54; 32]),
        target: RuntimeEntryTarget::Flow(flow),
        roles: RuntimeEntryRoles::None,
    }]);
    plan.verify().expect("non-stateful plan verifies");
    let dialogue_content = DialogueContentCatalog::new();
    let product_awbc = AwbcLowerer::new(&plan, &dialogue_content, "non-stateful.arcw")
        .lower()
        .expect("AWBC lowers")
        .program;
    let stats = BytecodeProgram::from_runtime_plan(plan.clone()).stats();
    let bundle = ArcweftBundle::try_new(
        BundleManifest {
            profile_id: None,
            profile_kind: None,
            entry: Some("entry.non_stateful".to_owned()),
            adapter: None,
            adapter_manifest_ids: Vec::new(),
            required_host_calls: Vec::new(),
            runtime: BundleRuntimeSummary {
                artifact_fingerprint: fixture_runtime_artifact_fingerprint(),
                entry_flow: Some("flow.non_stateful".to_owned()),
                flows: stats.flows,
                bytecode_instructions: stats.instructions,
                line_task_groups: stats.line_task_groups,
                stream_plans: stats.stream_plans,
                source_plans: stats.source_plans,
            },
        },
        source_map(),
        BytecodeProgram::from_runtime_plan(plan),
        dialogue_content,
    )
    .expect("non-stateful bundle source map accepts the generated standard Style source")
    .with_product_awbc(product_awbc);
    (bundle, entry)
}

fn source_map() -> SourceMapSection {
    let document = SourceDocument::try_new(
        SourceDocumentId::try_new("root-commands.arcw").expect("source ID"),
        SourceName::path("root-commands.arcw"),
        "",
    )
    .expect("source document");
    SourceMapSection::try_from_documents(&[&document]).expect("source map")
}

fn catalog(result_route: RootCommandHostResultRoute) -> RootCommandHostCallCatalog {
    RootCommandHostCallCatalog::try_new([RootCommandHostCallBinding::new(
        constructor_id(),
        target_id(),
        RootCommandHostCallEndpoint::try_new(
            "host.save",
            "save",
            "write",
            RuntimeHostCallMode::Immediate,
            false,
        )
        .expect("host endpoint"),
        [
            RootCommandHostArgument::Target,
            RootCommandHostArgument::Payload,
        ],
        result_route,
    )])
    .expect("catalog")
}

fn session(
    include_flow_host_call: bool,
    result_route: RootCommandHostResultRoute,
) -> BundleSession {
    BundleSession::new(
        &command_bundle(include_flow_host_call),
        BundleSessionOptions {
            entry: Some(entry_id()),
            root_command_host_calls: catalog(result_route),
            ..BundleSessionOptions::default()
        },
    )
    .expect("session starts")
}

fn step_with_event(
    session: &mut BundleSession,
    value: RuntimeValue,
) -> arcweft_runtime_driver::session::BundleSessionStep {
    session.step_with_clock(
        RuntimeClockStep::from_millis(1, 16).expect("clock"),
        BundleStepInput {
            root_events: vec![RootEventInput::new(RuntimePayload(value))],
            ..BundleStepInput::default()
        },
    )
}

#[test]
fn construction_rejects_missing_selected_command_projection() {
    let error = BundleSession::new(
        &command_bundle(false),
        BundleSessionOptions {
            entry: Some(entry_id()),
            ..BundleSessionOptions::default()
        },
    )
    .expect_err("selected policy requires a complete catalog");

    assert_eq!(
        error,
        BundleSessionError::RootCommandHostCatalog(
            RootCommandHostCallCatalogError::MissingBinding {
                constructor: CONSTRUCTOR.to_owned(),
                target: TARGET.to_owned(),
            }
        )
    );
}

#[test]
fn committed_root_request_precedes_later_flow_host_request() {
    let mut session = session(true, RootCommandHostResultRoute::Ignore);
    let step = step_with_event(&mut session, RuntimeValue::i64(7));

    assert_eq!(step.root_commands.len(), 1, "{step:#?}");
    assert_eq!(step.requested_host_calls.len(), 2);
    assert_eq!(step.requested_host_calls[0].public_id, "host.save");
    assert_eq!(step.requested_host_calls[0].capability, "save");
    assert_eq!(step.requested_host_calls[0].operation, "write");
    assert_eq!(
        step.requested_host_calls[0].args,
        vec![
            RuntimePayload(RuntimeValue::EntityRef(TARGET.to_owned())),
            RuntimePayload(RuntimeValue::String("checkpoint".to_owned())),
        ]
    );
    assert_eq!(step.requested_host_calls[1].public_id, "flow.probe");
}

#[test]
fn root_result_is_not_delivered_to_suspended_flow_and_enters_next_root_batch() {
    let mut session = session(true, RootCommandHostResultRoute::RootEventPayload);
    let first = step_with_event(&mut session, RuntimeValue::i64(7));
    let root_request = first.requested_host_calls[0].id.clone();

    let second = session.step_with_clock(
        RuntimeClockStep::from_millis(2, 16).expect("clock"),
        BundleStepInput {
            host_call_results: vec![RuntimeHostCallResult {
                id: root_request,
                outcome: Ok(RuntimePayload(RuntimeValue::i64(8))),
            }],
            ..BundleStepInput::default()
        },
    );

    assert_eq!(second.root_transitions.len(), 1);
    assert_eq!(
        second.root_transitions[0].sequence(),
        TransitionSequence::from_u64(1)
    );
    assert!(matches!(
        second.fiber_status,
        arcweft_core::engine::FlowFiberStatus::HostCall(_)
    ));
}

#[test]
fn root_result_and_user_events_share_atomic_core_ingress_preflight() {
    let mut session = session(true, RootCommandHostResultRoute::RootEventPayload);
    let first = step_with_event(&mut session, RuntimeValue::i64(7));
    let root_request = first.requested_host_calls[0].id.clone();

    let rejected = session.step_with_clock(
        RuntimeClockStep::from_millis(2, 16).expect("clock"),
        BundleStepInput {
            root_events: vec![RootEventInput::new(RuntimePayload(RuntimeValue::String(
                "invalid".to_owned(),
            )))],
            host_call_results: vec![RuntimeHostCallResult {
                id: root_request,
                outcome: Ok(RuntimePayload(RuntimeValue::i64(8))),
            }],
            ..BundleStepInput::default()
        },
    );
    assert!(rejected.root_transitions.is_empty());

    let next = step_with_event(&mut session, RuntimeValue::i64(9));
    assert_eq!(
        next.root_transitions[0].sequence(),
        TransitionSequence::from_u64(1)
    );
}

#[test]
fn host_failure_is_observed_without_rolling_back_committed_root_state() {
    let mut session = session(true, RootCommandHostResultRoute::Ignore);
    let first = step_with_event(&mut session, RuntimeValue::i64(7));
    let root_request = first.requested_host_calls[0].id.clone();

    let next = session.step_with_clock(
        RuntimeClockStep::from_millis(2, 16).expect("clock"),
        BundleStepInput {
            root_events: vec![RootEventInput::new(RuntimePayload(RuntimeValue::i64(8)))],
            host_call_results: vec![RuntimeHostCallResult {
                id: root_request,
                outcome: Err(RuntimeHostCallError {
                    kind: RuntimeHostCallErrorKind::Failed,
                    message: "disk unavailable".to_owned(),
                }),
            }],
            ..BundleStepInput::default()
        },
    );

    assert_eq!(
        next.root_transitions[0].sequence(),
        TransitionSequence::from_u64(1)
    );
    assert!(
        next.diagnostics
            .iter()
            .any(|message| message.contains("failed after root commit"))
    );
}

#[test]
fn restarting_an_entry_intentionally_discards_old_result_correlation() {
    let mut session = session(false, RootCommandHostResultRoute::Ignore);
    let first = step_with_event(&mut session, RuntimeValue::i64(7));
    assert_eq!(first.requested_host_calls.len(), 1);
    assert!(matches!(
        session.snapshot_session(),
        Err(BundleSessionSaveError::NonQuiescent { blockers })
            if blockers
                == vec![BundleSessionPendingBlocker::PendingHostCallResults { count: 1 }]
    ));

    session
        .start_foreground_entry_on_current_generation(BundleEntryStart::SessionDefault)
        .expect("entry restart");

    session
        .snapshot_session()
        .expect("old result correlation is discarded at the explicit restart boundary");
}

fn recorded_single_transition(
    bundle: &ArcweftBundle,
    result_route: RootCommandHostResultRoute,
) -> arcweft_runtime_driver::session::RootReplayTraceV1 {
    let mut session = BundleSession::new(
        bundle,
        BundleSessionOptions {
            entry: Some(entry_id()),
            root_command_host_calls: catalog(result_route),
            ..BundleSessionOptions::default()
        },
    )
    .expect("recording session starts");
    let mut recorder = session
        .start_root_replay_recording()
        .expect("recorder starts");
    session
        .step_with_clock_recording(
            RuntimeClockStep::from_millis(1, 16).expect("clock"),
            BundleStepInput {
                root_events: vec![RootEventInput::new(RuntimePayload(RuntimeValue::i64(7)))],
                ..BundleStepInput::default()
            },
            &mut recorder,
        )
        .expect("recorded step");
    recorder.finish().expect("trace finishes")
}

fn replay_options(result_route: RootCommandHostResultRoute) -> BundleSessionOptions {
    BundleSessionOptions {
        entry: Some(entry_id()),
        root_command_host_calls: catalog(result_route),
        ..BundleSessionOptions::default()
    }
}

#[test]
fn rep_001_production_recording_replays_initializer_state_outcome_and_commands() {
    let bundle = command_bundle(false);
    let trace = recorded_single_transition(&bundle, RootCommandHostResultRoute::Ignore);

    let report = BundleSession::replay_root_trace(
        &bundle,
        replay_options(RootCommandHostResultRoute::Ignore),
        &trace,
    )
    .expect("recorded trace replays");

    assert_eq!(report.transitions_verified, 1);
    assert_eq!(report.external_outcomes_injected, 0);
    assert_eq!(report.suppressed_host_requests, 1);
    assert!(!report.terminal_trap);
}

#[test]
fn rep_002_recorded_rejection_replays_without_changing_state() {
    let bundle = command_bundle_with_root(false, RuntimeValue::i64(1), reducer_rejection());
    let trace = recorded_single_transition(&bundle, RootCommandHostResultRoute::Ignore);
    assert!(matches!(
        trace.transitions[0].outcome,
        arcweft_runtime_driver::session::RecordedRootOutcomeV1::Rejected { .. }
    ));

    let report = BundleSession::replay_root_trace(
        &bundle,
        replay_options(RootCommandHostResultRoute::Ignore),
        &trace,
    )
    .expect("rejection replays");

    assert_eq!(report.transitions_verified, 1);
    assert_eq!(report.suppressed_host_requests, 0);
}

#[test]
fn rep_003_artifact_and_binding_mismatch_fail_during_preflight() {
    let bundle = command_bundle(false);
    let trace = recorded_single_transition(&bundle, RootCommandHostResultRoute::Ignore);
    let other_bundle =
        command_bundle_with_root(false, RuntimeValue::i64(9), reducer_ok_with_command());

    assert_eq!(
        BundleSession::replay_root_trace(
            &other_bundle,
            replay_options(RootCommandHostResultRoute::Ignore),
            &trace,
        ),
        Err(RootReplayError::ArtifactMismatch)
    );

    let mut binding_mismatch = trace;
    binding_mismatch.binding = EntryBindingIdentity::from_bytes([99; 32]);
    assert_eq!(
        BundleSession::replay_root_trace(
            &bundle,
            replay_options(RootCommandHostResultRoute::Ignore),
            &binding_mismatch,
        ),
        Err(RootReplayError::BindingMismatch)
    );
}

#[test]
fn rep_004_initializer_digest_divergence_fails_before_transition() {
    let bundle = command_bundle(false);
    let mut trace = recorded_single_transition(&bundle, RootCommandHostResultRoute::Ignore);
    trace.initializer_state_digest = RuntimeValueDigest::from_bytes([44; 32]);

    assert_eq!(
        BundleSession::replay_root_trace(
            &bundle,
            replay_options(RootCommandHostResultRoute::Ignore),
            &trace,
        ),
        Err(RootReplayError::InitializerDigestMismatch)
    );
}

#[test]
fn rep_005_post_state_digest_divergence_reports_first_transition() {
    let bundle = command_bundle(false);
    let mut trace = recorded_single_transition(&bundle, RootCommandHostResultRoute::Ignore);
    let arcweft_runtime_driver::session::RecordedRootOutcomeV1::Committed { state_digest, .. } =
        &mut trace.transitions[0].outcome
    else {
        panic!("fixture commits");
    };
    *state_digest = RuntimeValueDigest::from_bytes([55; 32]);

    assert!(matches!(
        BundleSession::replay_root_trace(
            &bundle,
            replay_options(RootCommandHostResultRoute::Ignore),
            &trace,
        ),
        Err(RootReplayError::OutcomeDivergence { transition: 0, .. })
    ));
}

#[test]
fn rep_006_command_divergence_reports_first_command_index() {
    let bundle = command_bundle(false);
    let mut trace = recorded_single_transition(&bundle, RootCommandHostResultRoute::Ignore);
    let arcweft_runtime_driver::session::RecordedRootOutcomeV1::Committed {
        command_digests, ..
    } = &mut trace.transitions[0].outcome
    else {
        panic!("fixture commits");
    };
    command_digests[0] = RuntimeValueDigest::from_bytes([66; 32]);

    assert_eq!(
        BundleSession::replay_root_trace(
            &bundle,
            replay_options(RootCommandHostResultRoute::Ignore),
            &trace,
        ),
        Err(RootReplayError::CommandDivergence {
            entry: ENTRY.to_owned(),
            transition: 0,
            command_index: 0,
        })
    );

    let mut count_mismatch =
        recorded_single_transition(&bundle, RootCommandHostResultRoute::Ignore);
    let arcweft_runtime_driver::session::RecordedRootOutcomeV1::Committed {
        command_digests, ..
    } = &mut count_mismatch.transitions[0].outcome
    else {
        panic!("fixture commits");
    };
    command_digests.push(RuntimeValueDigest::from_bytes([77; 32]));
    assert!(matches!(
        BundleSession::replay_root_trace(
            &bundle,
            replay_options(RootCommandHostResultRoute::Ignore),
            &count_mismatch,
        ),
        Err(RootReplayError::CommandDivergence {
            transition: 0,
            command_index: 1,
            ..
        })
    ));

    let two_command_bundle = command_bundle_with_root(
        false,
        RuntimeValue::i64(1),
        reducer_ok_with_commands([
            RuntimeValue::String("first".to_owned()),
            RuntimeValue::String("second".to_owned()),
        ]),
    );
    let mut order_mismatch =
        recorded_single_transition(&two_command_bundle, RootCommandHostResultRoute::Ignore);
    let arcweft_runtime_driver::session::RecordedRootOutcomeV1::Committed {
        command_digests, ..
    } = &mut order_mismatch.transitions[0].outcome
    else {
        panic!("fixture commits");
    };
    command_digests.swap(0, 1);
    assert!(matches!(
        BundleSession::replay_root_trace(
            &two_command_bundle,
            replay_options(RootCommandHostResultRoute::Ignore),
            &order_mismatch,
        ),
        Err(RootReplayError::CommandDivergence {
            transition: 0,
            command_index: 0,
            ..
        })
    ));
}

#[test]
fn rep_007_recorded_external_outcome_is_injected_without_dispatch() {
    let bundle = command_bundle(false);
    let mut session = BundleSession::new(
        &bundle,
        replay_options(RootCommandHostResultRoute::RootEventPayload),
    )
    .expect("recording session starts");
    let mut recorder = session
        .start_root_replay_recording()
        .expect("recorder starts");
    let first = session
        .step_with_clock_recording(
            RuntimeClockStep::from_millis(1, 16).expect("clock"),
            BundleStepInput {
                root_events: vec![RootEventInput::new(RuntimePayload(RuntimeValue::i64(7)))],
                ..BundleStepInput::default()
            },
            &mut recorder,
        )
        .expect("first transition records");
    session
        .step_with_clock_recording(
            RuntimeClockStep::from_millis(2, 16).expect("clock"),
            BundleStepInput {
                host_call_results: vec![RuntimeHostCallResult {
                    id: first.requested_host_calls[0].id.clone(),
                    outcome: Ok(RuntimePayload(RuntimeValue::i64(8))),
                }],
                ..BundleStepInput::default()
            },
            &mut recorder,
        )
        .expect("external result transition records");
    let trace = recorder.finish().expect("trace finishes");
    assert_eq!(trace.transitions.len(), 2);
    assert_eq!(trace.external_outcomes.len(), 1);
    assert_eq!(
        trace.external_outcomes[0].position,
        RecordedExternalOutcomePositionV1::BeforeTransition(TransitionSequence::from_u64(1))
    );
    assert_eq!(
        trace.external_outcomes[0].root_event_sequence,
        Some(TransitionSequence::from_u64(1))
    );

    let report = BundleSession::replay_root_trace(
        &bundle,
        replay_options(RootCommandHostResultRoute::RootEventPayload),
        &trace,
    )
    .expect("external outcome replays");
    assert_eq!(report.transitions_verified, 2);
    assert_eq!(report.external_outcomes_injected, 1);
    assert_eq!(report.suppressed_host_requests, 2);

    let mut duplicate = trace;
    duplicate
        .external_outcomes
        .push(duplicate.external_outcomes[0].clone());
    assert!(matches!(
        BundleSession::replay_root_trace(
            &bundle,
            replay_options(RootCommandHostResultRoute::RootEventPayload),
            &duplicate,
        ),
        Err(RootReplayError::DuplicateExternalOutcome { .. })
    ));
}

#[test]
fn rep_008_sequence_gap_or_duplicate_is_rejected() {
    let bundle = command_bundle(false);
    let mut trace = recorded_single_transition(&bundle, RootCommandHostResultRoute::Ignore);
    trace.transitions[0].sequence = TransitionSequence::from_u64(1);

    assert!(matches!(
        BundleSession::replay_root_trace(
            &bundle,
            replay_options(RootCommandHostResultRoute::Ignore),
            &trace,
        ),
        Err(RootReplayError::SequenceMismatch {
            expected: 0,
            actual: 1,
            ..
        })
    ));

    let mut session =
        BundleSession::new(&bundle, replay_options(RootCommandHostResultRoute::Ignore))
            .expect("session starts");
    let mut recorder = session
        .start_root_replay_recording()
        .expect("recorder starts");
    for (tick, event) in [(1, 7), (2, 8)] {
        session
            .step_with_clock_recording(
                RuntimeClockStep::from_millis(tick, 16).expect("clock"),
                BundleStepInput {
                    root_events: vec![RootEventInput::new(RuntimePayload(RuntimeValue::i64(
                        event,
                    )))],
                    ..BundleStepInput::default()
                },
                &mut recorder,
            )
            .expect("transition records");
    }
    let mut duplicate = recorder.finish().expect("trace finishes");
    duplicate.transitions[1].sequence = TransitionSequence::ZERO;
    assert!(matches!(
        BundleSession::replay_root_trace(
            &bundle,
            replay_options(RootCommandHostResultRoute::Ignore),
            &duplicate,
        ),
        Err(RootReplayError::SequenceMismatch {
            expected: 1,
            actual: 0,
            ..
        })
    ));
}

#[test]
fn rep_009_recorded_trap_is_terminal_and_has_no_command_dispatch() {
    let bundle = command_bundle_with_root(
        false,
        RuntimeValue::i64(1),
        RuntimeExpr::Value(RuntimeValue::String("not a reducer result".to_owned())),
    );
    let trace = recorded_single_transition(&bundle, RootCommandHostResultRoute::Ignore);
    assert!(matches!(
        trace.transitions[0].outcome,
        arcweft_runtime_driver::session::RecordedRootOutcomeV1::Trapped { .. }
    ));

    let report = BundleSession::replay_root_trace(
        &bundle,
        replay_options(RootCommandHostResultRoute::Ignore),
        &trace,
    )
    .expect("trap replays deterministically");
    assert!(report.terminal_trap);
    assert_eq!(report.transitions_verified, 1);
    assert_eq!(report.suppressed_host_requests, 0);
}

#[test]
fn recorder_discards_an_atomically_rejected_input_batch() {
    let bundle = command_bundle(false);
    let mut session =
        BundleSession::new(&bundle, replay_options(RootCommandHostResultRoute::Ignore))
            .expect("session starts");
    let mut recorder = session
        .start_root_replay_recording()
        .expect("recorder starts");

    assert_eq!(
        session.step_with_clock_recording(
            RuntimeClockStep::from_millis(1, 16).expect("clock"),
            BundleStepInput {
                root_events: vec![RootEventInput::new(RuntimePayload(RuntimeValue::String(
                    "invalid".to_owned(),
                )))],
                ..BundleStepInput::default()
            },
            &mut recorder,
        ),
        Err(RootReplayRecordingError::RootIngressRejected)
    );

    session
        .step_with_clock_recording(
            RuntimeClockStep::from_millis(2, 16).expect("clock"),
            BundleStepInput {
                root_events: vec![RootEventInput::new(RuntimePayload(RuntimeValue::i64(7)))],
                ..BundleStepInput::default()
            },
            &mut recorder,
        )
        .expect("valid sequence zero still records");
    let trace = recorder.finish().expect("trace finishes");
    assert_eq!(trace.transitions.len(), 1);
    assert_eq!(trace.transitions[0].sequence, TransitionSequence::ZERO);
}

#[test]
fn run_009_011_typed_later_phase_event_is_deferred_to_next_step() {
    let mut session = session(false, RootCommandHostResultRoute::Ignore);
    let emitted = session.step_with_clock(
        RuntimeClockStep::from_millis(1, 16).expect("clock"),
        BundleStepInput {
            deferred_root_events: vec![RootEventInput::new(RuntimePayload(RuntimeValue::i64(7)))],
            ..BundleStepInput::default()
        },
    );

    assert!(emitted.root_transitions.is_empty());
    assert_eq!(emitted.deferred_root_events.len(), 1);

    let consumed = session.step_with_clock(
        RuntimeClockStep::from_millis(2, 16).expect("clock"),
        BundleStepInput::default(),
    );
    assert_eq!(consumed.root_transitions.len(), 1);
    assert_eq!(
        consumed.root_transitions[0].sequence(),
        TransitionSequence::ZERO
    );
}

#[test]
fn deferred_and_live_events_share_one_atomic_next_step_ingress_batch() {
    let mut session = session(false, RootCommandHostResultRoute::Ignore);
    session.step_with_clock(
        RuntimeClockStep::from_millis(1, 16).expect("clock"),
        BundleStepInput {
            deferred_root_events: vec![RootEventInput::new(RuntimePayload(RuntimeValue::String(
                "invalid".to_owned(),
            )))],
            ..BundleStepInput::default()
        },
    );

    let rejected = session.step_with_clock(
        RuntimeClockStep::from_millis(2, 16).expect("clock"),
        BundleStepInput {
            root_events: vec![RootEventInput::new(RuntimePayload(RuntimeValue::i64(7)))],
            ..BundleStepInput::default()
        },
    );
    assert!(rejected.root_transitions.is_empty());

    let next = step_with_event(&mut session, RuntimeValue::i64(8));
    assert_eq!(
        next.root_transitions[0].sequence(),
        TransitionSequence::ZERO
    );
}

#[test]
fn hot_009_code_compatible_swap_preserves_root_state_and_sequence_without_reinitializing() {
    let initial_bundle = command_bundle_with_root(
        false,
        RuntimeValue::i64(1),
        reducer_ok_with_commands(Vec::<RuntimeValue>::new()),
    );
    let mut session = BundleSession::new(
        &initial_bundle,
        replay_options(RootCommandHostResultRoute::Ignore),
    )
    .expect("session starts");
    let first = step_with_event(&mut session, RuntimeValue::i64(7));
    assert_eq!(
        first.root_transitions[0].sequence(),
        TransitionSequence::ZERO
    );
    let before = session
        .snapshot_session()
        .expect("quiescent snapshot")
        .root
        .expect("stateful root");

    let replacement = command_bundle_with_root(
        false,
        RuntimeValue::i64(99),
        reducer_ok_with_commands(Vec::<RuntimeValue>::new()),
    );
    let report = session
        .hot_swap_bundle(&replacement)
        .expect("compatible swap");
    assert_eq!(
        report.compatibility,
        arcweft_runtime_driver::swap::SwapCompatibility::CodeCompatible
    );

    let after = session
        .snapshot_session()
        .expect("quiescent snapshot")
        .root
        .expect("stateful root");
    assert_eq!(after, before);

    let second = step_with_event(&mut session, RuntimeValue::i64(8));
    assert_eq!(
        second.root_transitions[0].sequence(),
        TransitionSequence::from_u64(1)
    );
}

#[test]
fn hot_008_pending_deferred_root_event_blocks_swap_without_losing_the_event() {
    let initial_bundle = command_bundle(false);
    let mut session = BundleSession::new(
        &initial_bundle,
        replay_options(RootCommandHostResultRoute::Ignore),
    )
    .expect("session starts");
    session.step_with_clock(
        RuntimeClockStep::from_millis(1, 16).expect("clock"),
        BundleStepInput {
            deferred_root_events: vec![RootEventInput::new(RuntimePayload(RuntimeValue::i64(7)))],
            ..BundleStepInput::default()
        },
    );
    let replacement =
        command_bundle_with_root(false, RuntimeValue::i64(99), reducer_ok_with_command());

    assert!(matches!(
        session.hot_swap_bundle(&replacement),
        Err(BundleHotSwapError::PendingRootWork {
            reducer_active: false,
            pending_events: 1,
            pending_commands: 0,
            pending_command_results: 0,
        })
    ));

    let consumed = session.step_with_clock(
        RuntimeClockStep::from_millis(2, 16).expect("clock"),
        BundleStepInput::default(),
    );
    assert_eq!(consumed.root_transitions.len(), 1);
    assert_eq!(
        consumed.root_transitions[0].sequence(),
        TransitionSequence::ZERO
    );
}

#[test]
fn hot_008_pending_root_command_result_correlation_blocks_swap() {
    let initial_bundle = command_bundle(false);
    let mut session = BundleSession::new(
        &initial_bundle,
        replay_options(RootCommandHostResultRoute::RootEventPayload),
    )
    .expect("session starts");
    let first = step_with_event(&mut session, RuntimeValue::i64(7));
    assert_eq!(first.requested_host_calls.len(), 1);
    let replacement =
        command_bundle_with_root(false, RuntimeValue::i64(99), reducer_ok_with_command());

    assert!(matches!(
        session.hot_swap_bundle(&replacement),
        Err(BundleHotSwapError::PendingRootWork {
            reducer_active: false,
            pending_events: 0,
            pending_commands: 0,
            pending_command_results: 1,
        })
    ));
}

#[test]
fn hot_008_reducer_retained_root_event_blocks_swap() {
    let initial_bundle = command_bundle_with_root(false, RuntimeValue::i64(1), reducer_rejection());
    let mut session = BundleSession::new(
        &initial_bundle,
        replay_options(RootCommandHostResultRoute::Ignore),
    )
    .expect("session starts");
    let rejected = session.step_with_clock(
        RuntimeClockStep::from_millis(1, 16).expect("clock"),
        BundleStepInput {
            root_events: vec![
                RootEventInput::new(RuntimePayload(RuntimeValue::i64(7))),
                RootEventInput::new(RuntimePayload(RuntimeValue::i64(8))),
            ],
            ..BundleStepInput::default()
        },
    );
    assert_eq!(rejected.root_transitions.len(), 1);
    let replacement = command_bundle_with_root(false, RuntimeValue::i64(99), reducer_rejection());

    assert!(matches!(
        session.hot_swap_bundle(&replacement),
        Err(BundleHotSwapError::PendingRootWork {
            reducer_active: false,
            pending_events: 1,
            pending_commands: 0,
            pending_command_results: 0,
        })
    ));
}

#[test]
fn save_001_stateful_root_round_trip_preserves_value_sequence_and_entry_contract() {
    let bundle = command_bundle_with_root(
        false,
        RuntimeValue::i64(1),
        reducer_ok_with_commands(Vec::<RuntimeValue>::new()),
    );
    let mut session =
        BundleSession::new(&bundle, replay_options(RootCommandHostResultRoute::Ignore))
            .expect("session starts");
    session.step_with_clock(
        RuntimeClockStep::from_millis(1, 16).expect("clock"),
        BundleStepInput {
            root_events: vec![
                RootEventInput::new(RuntimePayload(RuntimeValue::i64(7))),
                RootEventInput::new(RuntimePayload(RuntimeValue::i64(8))),
            ],
            ..BundleStepInput::default()
        },
    );
    let expected = session.snapshot_session().expect("snapshot exports");
    assert_eq!(
        expected.root.as_ref().expect("stateful root").next_sequence,
        TransitionSequence::from_u64(2)
    );
    let bytes = session.export_session_save_bytes().expect("save encodes");
    let mut restored =
        BundleSession::new(&bundle, replay_options(RootCommandHostResultRoute::Ignore))
            .expect("replacement session starts");

    restored
        .import_session_save_bytes(&bytes, &arcweft_save::SaveDecodeOptions::default())
        .expect("save restores");

    assert_eq!(
        restored.snapshot_session().expect("restored snapshot"),
        expected
    );
}

#[test]
fn save_003_retained_root_event_reports_exact_non_quiescent_count() {
    let bundle = command_bundle_with_root(false, RuntimeValue::i64(1), reducer_rejection());
    let mut session =
        BundleSession::new(&bundle, replay_options(RootCommandHostResultRoute::Ignore))
            .expect("session starts");
    session.step_with_clock(
        RuntimeClockStep::from_millis(1, 16).expect("clock"),
        BundleStepInput {
            root_events: vec![
                RootEventInput::new(RuntimePayload(RuntimeValue::i64(7))),
                RootEventInput::new(RuntimePayload(RuntimeValue::i64(8))),
            ],
            ..BundleStepInput::default()
        },
    );

    assert_eq!(
        session.snapshot_session(),
        Err(BundleSessionSaveError::NonQuiescent {
            blockers: vec![BundleSessionPendingBlocker::PendingRootEvents { count: 1 }],
        })
    );
}

#[test]
fn save_004_active_entry_tampering_is_rejected_without_mutation() {
    let bundle = command_bundle_with_root(
        false,
        RuntimeValue::i64(1),
        reducer_ok_with_commands(Vec::<RuntimeValue>::new()),
    );
    let mut session =
        BundleSession::new(&bundle, replay_options(RootCommandHostResultRoute::Ignore))
            .expect("session starts");
    let before = session.snapshot_session().expect("snapshot exports");
    let mut wrong_binding = before.clone();
    wrong_binding.active_entry.binding = EntryBindingIdentity::from_bytes([44; 32]);
    let mut wrong_id = before.clone();
    wrong_id.active_entry.id =
        EntryRuntimeId::from_source_entity_body("entry.other").expect("other entry ID");
    let mut wrong_kind = before.clone();
    wrong_kind.active_entry.kind = RuntimeEntryKind::Editor;

    for invalid in [wrong_binding, wrong_id, wrong_kind] {
        assert!(matches!(
            session.restore_session_snapshot(invalid),
            Err(BundleSessionSaveError::Root { .. })
        ));
        assert_eq!(
            session
                .snapshot_session()
                .expect("rejected restore leaves session valid"),
            before
        );
    }
}

#[test]
fn save_005_state_and_event_role_tampering_is_rejected_without_mutation() {
    let bundle = command_bundle_with_root(
        false,
        RuntimeValue::i64(1),
        reducer_ok_with_commands(Vec::<RuntimeValue>::new()),
    );
    let mut session =
        BundleSession::new(&bundle, replay_options(RootCommandHostResultRoute::Ignore))
            .expect("session starts");
    let before = session.snapshot_session().expect("snapshot exports");
    let mut invalid = Vec::new();

    let mut state_identity = before.clone();
    state_identity
        .root
        .as_mut()
        .expect("stateful root")
        .state_identity = RuntimeNominalTypeId::try_new("OtherState").expect("identity");
    invalid.push(state_identity);
    let mut state_layout = before.clone();
    state_layout
        .root
        .as_mut()
        .expect("stateful root")
        .state_layout = TypeLayoutHash::from_bytes([45; 32]);
    invalid.push(state_layout);
    let mut event_identity = before.clone();
    event_identity
        .root
        .as_mut()
        .expect("stateful root")
        .event_identity = RuntimeNominalTypeId::try_new("OtherEvent").expect("identity");
    invalid.push(event_identity);
    let mut event_layout = before.clone();
    event_layout
        .root
        .as_mut()
        .expect("stateful root")
        .event_layout = TypeLayoutHash::from_bytes([46; 32]);
    invalid.push(event_layout);

    for snapshot in invalid {
        assert!(matches!(
            session.restore_session_snapshot(snapshot),
            Err(BundleSessionSaveError::Root { .. })
        ));
        assert_eq!(
            session
                .snapshot_session()
                .expect("rejected restore leaves session valid"),
            before
        );
    }
}

#[test]
fn save_006_invalid_root_value_or_presence_is_rejected_without_mutation() {
    let bundle = command_bundle_with_root(
        false,
        RuntimeValue::i64(1),
        reducer_ok_with_commands(Vec::<RuntimeValue>::new()),
    );
    let mut session =
        BundleSession::new(&bundle, replay_options(RootCommandHostResultRoute::Ignore))
            .expect("session starts");
    let before = session.snapshot_session().expect("snapshot exports");
    let mut invalid_value = before.clone();
    invalid_value.root.as_mut().expect("stateful root").value =
        RuntimePayload(RuntimeValue::String("invalid state".to_owned()));
    let mut missing_root = before.clone();
    missing_root.root = None;

    for invalid in [invalid_value, missing_root] {
        assert!(matches!(
            session.restore_session_snapshot(invalid),
            Err(BundleSessionSaveError::Root { .. })
        ));
        assert_eq!(
            session
                .snapshot_session()
                .expect("rejected restore leaves session valid"),
            before
        );
    }
}

#[test]
fn save_009_non_stateful_entry_rejects_injected_root_without_mutation() {
    let stateful_bundle = command_bundle_with_root(
        false,
        RuntimeValue::i64(1),
        reducer_ok_with_commands(Vec::<RuntimeValue>::new()),
    );
    let stateful = BundleSession::new(
        &stateful_bundle,
        replay_options(RootCommandHostResultRoute::Ignore),
    )
    .expect("stateful session starts");
    let injected_root = stateful
        .snapshot_session()
        .expect("stateful snapshot")
        .root
        .expect("stateful root");
    let (bundle, entry) = non_stateful_bundle();
    let mut session = BundleSession::new(
        &bundle,
        BundleSessionOptions {
            entry: Some(entry),
            ..BundleSessionOptions::default()
        },
    )
    .expect("non-stateful session starts");
    let before = session.snapshot_session().expect("snapshot exports");
    assert!(before.root.is_none());
    let mut invalid = before.clone();
    invalid.root = Some(injected_root);

    assert!(matches!(
        session.restore_session_snapshot(invalid),
        Err(BundleSessionSaveError::Root { .. })
    ));
    assert_eq!(
        session
            .snapshot_session()
            .expect("rejected restore leaves session valid"),
        before
    );
}
