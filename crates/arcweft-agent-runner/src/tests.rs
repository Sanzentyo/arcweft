use crate::{
    config::{AgentControllerRunConfig, AgentRunnerConfig},
    effect_policy::AgentEffectPolicyError,
    error::AgentRunError,
    host_request::{agent_host_request_from_call, agent_host_request_from_task},
    label_parse::parse_capture_format,
    policy::{RuntimeAgentCapability, RuntimeAgentPolicy},
    runner::AgentRunner,
    runtime_args::AGENT_NAMED_ARGS_VARIANT,
    runtime_payload::{
        runtime_payload_from_response, runtime_project_graph_symbol_payload,
        runtime_rag_context_payload, runtime_resource_payload,
    },
    runtime_value::{runtime_field, runtime_record_get, runtime_record_string},
    session::{AgentSession, NoopRagService, ReplayAgentSession, ReplayAgentSessionError},
};
use arcweft_agent_protocol::protocol::ActionResult;
use arcweft_agent_protocol::{
    action::{AgentActionDispatch, AgentActionKind, AgentActionTarget},
    artifact::{
        AgentArtifactManifest, AgentBundleKind, EffectCapability, ProjectBinding,
        ProjectBindingMode, RequiredEntity, RequiredEntitySourceAnchor,
        RequiredEntitySourcePosition,
    },
    ids::StableHash,
    ids::{AgentResourceUri, AgentRunId, CallableId, PublicId, SessionId},
    predicate::{CompareOp, DebugStatePath, ObservationFieldPath, Predicate, Probe},
    protocol::{
        AgentAction, AgentAssertionKind, AgentAssertionRequest, AgentHostRequest,
        AgentHostResponse, AgentProjectFlowControlSummary, AgentProjectGraph,
        AgentProjectGraphEdge, AgentProjectGraphSummary, AgentProjectGraphSymbol, AgentSessionInfo,
        CaptureFormat, CaptureRequest, CaptureResult, CaptureTarget, ObservationEnvelope,
        ObserveRequest, PointerButton, WaitRequest,
    },
    resource::{AgentResource, AgentResourceBody, AgentResourceKind},
    trace::{AgentTraceKind, AgentTraceRecord},
    value::AgentValue,
    verified_effects::VerifiedEffectSummary,
};
use arcweft_bundle::resource_codec::SourceMapSection;
use arcweft_bundle::{ArcweftBundle, BundleManifest, BundleRuntimeSummary};
use arcweft_core::{
    bytecode::BytecodeProgram,
    effect::{LineEffectRequest, RuntimeCall},
    engine::{FlowExit, FlowFiberStatus},
    entry::{
        AgentBudget, AgentPolicyHash, CallableContractHash, EntryBindingIdentity, FlowContractHash,
        RuntimeAgentEntryRoles, RuntimeCallableExecutable, RuntimeCallableExecutableCode,
        RuntimeCallableId, RuntimeCallableRole, RuntimeEntryRoles, RuntimeFlowExecutable,
    },
    pattern::RuntimePattern,
    plan::{
        EntryRuntimeId, FlowOp, FlowRuntimeId, RuntimeEntryKind, RuntimeEntrySpec,
        RuntimeEntryTarget, RuntimeFlow, RuntimePlan,
    },
    task::{
        AwaitTarget, HostTaskArgTemplate, HostTaskRequest, HostTaskRequestTemplate, NeedId, TaskId,
    },
    time::LogicalDuration,
    value::{RuntimeExpr, RuntimeFieldExpr, RuntimePayload, RuntimeValue},
};
use arcweft_debug_model::{
    event::{DebugEvent, DebugEventKind},
    sink::{DebugEventSink, NullDebugEventSink},
};
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};
use std::collections::BTreeMap;
use std::convert::Infallible;

fn flow_id(value: &str) -> FlowRuntimeId {
    FlowRuntimeId::from_runtime_target_value(value).expect("test flow ID is valid")
}

fn agent_entry_id(agent_id: &str) -> EntryRuntimeId {
    EntryRuntimeId::from_source_entity_body(&format!("entry.{agent_id}"))
        .expect("test Agent entry ID is valid")
}

fn agent_controller_program(
    flow: RuntimeFlow,
    agent_id: &str,
    budget: AgentBudget,
) -> BytecodeProgram {
    let entry = agent_entry_id(agent_id);
    let controller_flow = flow.id.clone();
    let binding = EntryBindingIdentity::from_bytes([1; 32]);
    let contract = CallableContractHash::from_bytes([2; 32]);
    let policy = AgentPolicyHash::from_bytes([3; 32]);
    let controller = RuntimeCallableRole {
        callable: RuntimeCallableId::try_new(format!("test::crate.{agent_id}"))
            .expect("test controller identity is valid"),
        contract,
    };
    let plan = RuntimePlan::new(vec![flow], Vec::new())
        .expect("runtime plan is valid")
        .with_entries(vec![RuntimeEntrySpec {
            id: entry,
            kind: RuntimeEntryKind::Agent,
            binding,
            target: RuntimeEntryTarget::Controller(controller_flow.clone()),
            roles: RuntimeEntryRoles::Agent(Box::new(RuntimeAgentEntryRoles {
                binding,
                controller: controller.clone(),
                policy,
                budget,
            })),
        }])
        .with_entry_executables(
            vec![RuntimeCallableExecutable {
                callable: controller.callable.clone(),
                contract,
                code: RuntimeCallableExecutableCode::ControllerFlow(controller_flow.clone()),
            }],
            vec![RuntimeFlowExecutable {
                flow: controller_flow,
                contract: FlowContractHash::from_bytes(*contract.as_bytes()),
                parameters: Vec::new(),
                controller: Some(controller),
            }],
        );
    plan.verify()
        .expect("Agent controller runtime plan verifies");
    BytecodeProgram::from_runtime_plan(plan)
}

#[derive(Default)]
struct TestSession {
    observations: Vec<ObservationEnvelope>,
}

struct MetadataSession {
    project_entities: Vec<RequiredEntity>,
    project_graph: AgentProjectGraph,
}

#[derive(Default)]
struct RecordingDebugSink {
    events: Vec<DebugEvent>,
}

fn project_neighbors_test_graph() -> AgentProjectGraph {
    AgentProjectGraph {
        symbols: vec![
            AgentProjectGraphSymbol {
                symbol_id: "project:summary".to_owned(),
                public_id: None,
                qualified_name: Some("project".to_owned()),
                kind: "project_summary".to_owned(),
                semantic_hash: None,
                flow_control: None,
                project_summary: Some(AgentProjectGraphSummary {
                    entity_count: 1,
                    agent_action_count: 0,
                    project_callable_count: 0,
                    relation_count: 1,
                    dependency_edge_count: 0,
                    dynamic_control_flow_count: 1,
                    debug_query_count: 0,
                }),
                summary: "Project".to_owned(),
            },
            AgentProjectGraphSymbol {
                symbol_id: "project:entity:flow.opening".to_owned(),
                public_id: Some(PublicId::new("flow.opening").expect("valid id")),
                qualified_name: None,
                kind: "flow".to_owned(),
                semantic_hash: Some("hir:flow:flow.opening:_".to_owned()),
                flow_control: Some(AgentProjectFlowControlSummary {
                    has_dynamic_control: true,
                    static_goto_count: 1,
                    dynamic_goto_count: 1,
                    branch_count: 0,
                    loop_count: 0,
                    await_count: 0,
                    thread_count: 0,
                    select_branch_count: 0,
                }),
                project_summary: None,
                summary: "Opening flow".to_owned(),
            },
        ],
        edges: vec![AgentProjectGraphEdge {
            from_symbol_id: "project:summary".to_owned(),
            to_symbol_id: "project:entity:flow.opening".to_owned(),
            edge_kind: "contains_entity".to_owned(),
        }],
    }
}

impl DebugEventSink for RecordingDebugSink {
    type Error = Infallible;

    fn append(&mut self, event: &DebugEvent) -> Result<(), Self::Error> {
        self.events.push(event.clone());
        Ok(())
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl AgentSession for TestSession {
    type Error = Infallible;

    fn info(&mut self) -> Result<AgentSessionInfo, Self::Error> {
        Ok(AgentSessionInfo {
            session_id: "session.test".to_owned(),
            program_hash: "hash".to_owned(),
            project_entities: Vec::new(),
            project_graph: AgentProjectGraph::default(),
            profile: None,
            capabilities: Vec::new(),
        })
    }

    fn observe(&mut self, _request: ObserveRequest) -> Result<ObservationEnvelope, Self::Error> {
        Ok(self.observations.remove(0))
    }

    fn act(&mut self, _action: AgentAction) -> Result<ActionResult, Self::Error> {
        Ok(ActionResult {
            accepted: true,
            before_tick: 1,
            after_tick: 2,
            before_state_hash: "a".to_owned(),
            after_state_hash: "b".to_owned(),
        })
    }

    fn capture(&mut self, _request: CaptureRequest) -> Result<CaptureResult, Self::Error> {
        Ok(CaptureResult {
            uri: AgentResourceUri::new("agent://capture/test").expect("valid uri"),
            content_hash: "hash".to_owned(),
            media_type: "image/png".to_owned(),
            byte_len: 4,
        })
    }

    fn read_resource(&mut self, uri: &str) -> Result<AgentResource, Self::Error> {
        Ok(AgentResource {
            uri: AgentResourceUri::new(uri).expect("test resource URI is nonempty"),
            kind: AgentResourceKind::ObservationLatest,
            mime_type: "application/json".to_owned(),
            hash: "resource.hash".to_owned(),
            image: None,
            body: AgentResourceBody::Json(serde_json::json!({ "uri": uri })),
        })
    }

    fn step_frames(&mut self, _count: u32) -> Result<ObservationEnvelope, Self::Error> {
        Ok(self.observations.remove(0))
    }
}

impl AgentSession for MetadataSession {
    type Error = Infallible;

    fn info(&mut self) -> Result<AgentSessionInfo, Self::Error> {
        Ok(AgentSessionInfo {
            session_id: "session.metadata".to_owned(),
            program_hash: "hash".to_owned(),
            project_entities: self.project_entities.clone(),
            project_graph: self.project_graph.clone(),
            profile: None,
            capabilities: vec!["debug.read".to_owned()],
        })
    }

    fn observe(&mut self, _request: ObserveRequest) -> Result<ObservationEnvelope, Self::Error> {
        unreachable!("metadata session only serves AgentSessionInfo")
    }

    fn act(&mut self, _action: AgentAction) -> Result<ActionResult, Self::Error> {
        unreachable!("metadata session only serves AgentSessionInfo")
    }

    fn capture(&mut self, _request: CaptureRequest) -> Result<CaptureResult, Self::Error> {
        unreachable!("metadata session only serves AgentSessionInfo")
    }

    fn read_resource(&mut self, _uri: &str) -> Result<AgentResource, Self::Error> {
        unreachable!("metadata session only serves AgentSessionInfo")
    }

    fn step_frames(&mut self, _count: u32) -> Result<ObservationEnvelope, Self::Error> {
        unreachable!("metadata session only serves AgentSessionInfo")
    }
}

fn observation(tick: u64, ready: bool) -> ObservationEnvelope {
    ObservationEnvelope {
        tick,
        frame_id: format!("frame.{tick}"),
        state_hash: format!("state.{tick}"),
        render_hash: format!("render.{tick}"),
        actions: Vec::new(),
        signals: BTreeMap::from([("signal.ready".to_owned(), AgentValue::Bool(ready))]),
        payload: serde_json::json!({}),
    }
}

#[test]
fn capture_format_parser_accepts_only_raster_formats() {
    assert_eq!(
        parse_capture_format(".png").expect("png is accepted"),
        CaptureFormat::Png
    );
    assert_eq!(
        parse_capture_format(".raw_rgba").expect("raw rgba is accepted"),
        CaptureFormat::RawRgba
    );
    assert_eq!(
        parse_capture_format(".raw").expect("raw shorthand is accepted"),
        CaptureFormat::RawRgba
    );

    let error = parse_capture_format(".svg").expect_err("svg capture is not an Agent format");
    assert!(error.contains("unsupported capture format `.svg`"));
}

fn observation_with_signal(
    tick: u64,
    signal: &'static str,
    value: AgentValue,
) -> ObservationEnvelope {
    ObservationEnvelope {
        tick,
        frame_id: format!("frame.{tick}"),
        state_hash: format!("state.{tick}"),
        render_hash: format!("render.{tick}"),
        actions: Vec::new(),
        signals: BTreeMap::from([(signal.to_owned(), value)]),
        payload: serde_json::json!({}),
    }
}

fn observation_with_action_target(
    tick: u64,
    target: &'static str,
    enabled: bool,
) -> ObservationEnvelope {
    ObservationEnvelope {
        tick,
        frame_id: format!("frame.{tick}"),
        state_hash: format!("state.{tick}"),
        render_hash: format!("render.{tick}"),
        actions: vec![AgentActionTarget {
            id: format!("action.select_choice.{target}"),
            target: target.to_owned(),
            action: AgentActionKind::SelectChoice,
            kind: AgentActionDispatch::Semantic,
            enabled,
        }],
        signals: BTreeMap::new(),
        payload: serde_json::json!({}),
    }
}

#[test]
fn replay_agent_session_replays_recorded_host_responses_in_order() {
    let observation = observation(7, true);
    let action = ActionResult {
        accepted: true,
        before_tick: 7,
        after_tick: 8,
        before_state_hash: "state.7".to_owned(),
        after_state_hash: "state.8".to_owned(),
    };
    let capture = CaptureResult {
        uri: AgentResourceUri::new("agent://capture/replay").expect("valid capture uri"),
        content_hash: "blake3:capture".to_owned(),
        media_type: "image/png".to_owned(),
        byte_len: 12,
    };
    let resource = AgentResource {
        uri: AgentResourceUri::new("agent://resource/replay")
            .expect("test resource URI is nonempty"),
        kind: AgentResourceKind::ObservationLatest,
        mime_type: "application/json".to_owned(),
        hash: "state.replay".to_owned(),
        image: None,
        body: AgentResourceBody::Json(serde_json::json!({ "ok": true })),
    };
    let mut session = ReplayAgentSession::from_trace_records(vec![
        replay_trace_record(AgentTraceKind::RunStarted, 0, serde_json::json!({})),
        replay_trace_record(
            AgentTraceKind::ObservationReceived,
            1,
            serde_json::to_value(&observation).expect("observation serializes"),
        ),
        replay_trace_record(
            AgentTraceKind::ActionCompleted,
            2,
            serde_json::to_value(&action).expect("action result serializes"),
        ),
        replay_trace_record(
            AgentTraceKind::CaptureStored,
            3,
            serde_json::to_value(&capture).expect("capture result serializes"),
        ),
        replay_trace_record(
            AgentTraceKind::ResourceReadCompleted,
            4,
            serde_json::to_value(&resource).expect("resource serializes"),
        ),
        replay_trace_record(AgentTraceKind::RunFinished, 5, serde_json::json!({})),
    ]);

    assert_eq!(
        session.info().expect("replay info").session_id,
        "session.test"
    );
    assert_eq!(
        session
            .observe(ObserveRequest::default())
            .expect("replay observe"),
        observation
    );
    assert_eq!(
        session
            .act(AgentAction::AdvanceText)
            .expect("replay action"),
        action
    );
    assert_eq!(
        session
            .capture(CaptureRequest {
                target: CaptureTarget::Viewport,
                format: CaptureFormat::Png,
                capture_kind: "color".to_owned(),
                name: "viewport".to_owned(),
            })
            .expect("replay capture"),
        capture
    );
    assert_eq!(
        session
            .read_resource("agent://resource/replay")
            .expect("replay resource read"),
        resource
    );
}

#[test]
fn replay_agent_session_rejects_out_of_order_host_response() {
    let action = ActionResult {
        accepted: true,
        before_tick: 1,
        after_tick: 2,
        before_state_hash: "state.1".to_owned(),
        after_state_hash: "state.2".to_owned(),
    };
    let mut session = ReplayAgentSession::from_trace_records(vec![replay_trace_record(
        AgentTraceKind::ActionCompleted,
        9,
        serde_json::to_value(action).expect("action result serializes"),
    )]);

    assert_eq!(
        session.observe(ObserveRequest::default()),
        Err(ReplayAgentSessionError::UnexpectedRecordKind {
            expected: AgentTraceKind::ObservationReceived,
            found: AgentTraceKind::ActionCompleted,
            sequence: 9,
        })
    );
}

fn replay_trace_record(
    kind: AgentTraceKind,
    sequence: u64,
    payload: serde_json::Value,
) -> AgentTraceRecord {
    AgentTraceRecord {
        schema_version: 1,
        run_id: AgentRunId::new("run.replay").expect("valid run id"),
        session_id: Some(SessionId::new("session.test").expect("valid session id")),
        sequence,
        tick: None,
        kind,
        payload_hash: StableHash::new(format!("payload.{sequence}")).expect("valid hash"),
        payload,
        blob_refs: Vec::new(),
    }
}

fn observe_checkpoint_program() -> BytecodeProgram {
    agent_controller_program(
        RuntimeFlow {
            id: flow_id("agent.observe_smoke"),
            ops: vec![
                FlowOp::Effect(LineEffectRequest::Call(RuntimeCall {
                    callee: "observe".to_owned(),
                    args: vec!["include_objects = true".to_owned()],
                })),
                FlowOp::Effect(LineEffectRequest::Call(RuntimeCall {
                    callee: "checkpoint".to_owned(),
                    args: vec!["\"after-observe\"".to_owned()],
                })),
                FlowOp::Return("done".to_owned()),
            ],
        },
        "agent.observe_smoke",
        AgentBudget::default(),
    )
}

fn observe_checkpoint_bundle() -> ArcweftBundle {
    let program = observe_checkpoint_program();
    agent_controller_test_bundle(
        &program,
        "agent.observe_smoke",
        "agent.observe_smoke.awfagent",
        "fn observe_smoke() -> Result<Unit, AgentError> effects { agent.observe, debug.record } { observe() }\nentry agent @entry.agent.observe_smoke { controller = observe_smoke }",
        &["agent.observe", "debug.record"],
        AgentBudget::default(),
    )
}

fn capture_binding_bundle_with_budget(budget: AgentBudget) -> ArcweftBundle {
    agent_controller_test_bundle(
        &capture_binding_program_with_budget(budget),
        "agent.capture_binding",
        "agent.capture_binding.awfagent",
        "fn capture_binding() -> Result<Unit, AgentError> effects { agent.capture } { let shot = try capture(viewport()) }\nentry agent @entry.agent.capture_binding { controller = capture_binding }",
        &["agent.capture"],
        budget,
    )
}

fn agent_controller_test_bundle(
    program: &BytecodeProgram,
    agent_id: &str,
    source_label: &str,
    source_text: &str,
    effects: &[&str],
    budget: AgentBudget,
) -> ArcweftBundle {
    let stats = program.stats();
    let [entry] = program.entries.as_slice() else {
        panic!("test Agent artifact has exactly one entry");
    };
    assert_eq!(entry.id, agent_entry_id(agent_id));
    let RuntimeEntryTarget::Controller(controller_flow) = &entry.target else {
        panic!("test Agent entry targets a controller");
    };
    let roles = entry.roles.agent().expect("test Agent roles exist");
    let display = arcweft_render_text::LineDisplayCatalog::default();
    let declared_effects = effects
        .iter()
        .copied()
        .map(EffectCapability::new)
        .collect::<Vec<_>>();
    let verified_effects = VerifiedEffectSummary::new(
        1,
        declared_effects.clone(),
        declared_effects.clone(),
        StableHash::new(format!("blake3:test-effects-{agent_id}")).expect("valid effect hash"),
    );
    ArcweftBundle::try_new(
        BundleManifest {
            profile_id: None,
            profile_kind: None,
            entry: Some(format!("entry.{agent_id}")),
            adapter: None,
            adapter_manifest_ids: Vec::new(),
            required_host_calls: Vec::new(),
            runtime: BundleRuntimeSummary {
                entry_flow: Some(controller_flow.public_label().into_string()),
                flows: stats.flows,
                bytecode_instructions: stats.instructions,
                line_task_groups: stats.line_task_groups,
                stream_plans: stats.stream_plans,
                source_plans: stats.source_plans,
            },
        },
        source_map(source_label, source_text),
        program.clone(),
        display,
    )
    .expect("standard dialogue source joins source map")
    .with_agent_manifest(AgentArtifactManifest {
        schema_version: 1,
        bundle_kind: AgentBundleKind::AgentController,
        entry_id: PublicId::new(format!("entry.{agent_id}")).expect("valid entry id"),
        controller_id: CallableId::new(roles.controller.callable.as_str())
            .expect("valid controller id"),
        entry_binding_hash: StableHash::from_blake3_bytes(*entry.binding.as_bytes()),
        controller_contract_hash: StableHash::from_blake3_bytes(
            *roles.controller.contract.as_bytes(),
        ),
        policy_hash: StableHash::from_blake3_bytes(*roles.policy.as_bytes()),
        source_hash: StableHash::new("blake3:test").expect("valid source hash"),
        compiler_version: "test".to_owned(),
        project_binding: ProjectBinding {
            program_hash: StableHash::new("program-test").expect("valid program hash"),
            mode: ProjectBindingMode::Compatible,
            required_entities: Vec::new(),
        },
        declared_effects,
        verified_effects,
        budget,
        debug_map_hash: None,
    })
}

fn source_map(label: &str, text: &str) -> SourceMapSection {
    let document = SourceDocument::try_new(
        SourceDocumentId::try_new(label).expect("source ID"),
        SourceName::path(label),
        text,
    )
    .expect("source document");
    SourceMapSection::try_from_documents(&[&document]).expect("source map")
}

fn capture_binding_program() -> BytecodeProgram {
    capture_binding_program_with_budget(AgentBudget::default())
}

fn capture_binding_program_with_budget(budget: AgentBudget) -> BytecodeProgram {
    agent_controller_program(
        RuntimeFlow {
            id: flow_id("agent.capture_binding"),
            ops: vec![
                FlowOp::Await {
                    binding: Some(RuntimePattern::Ident("shot".to_owned())),
                    target: AwaitTarget::new(
                        NeedId("need.agent.capture".to_owned()),
                        TaskId("task.agent.capture".to_owned()),
                        HostTaskRequestTemplate::new(
                            "agent",
                            "capture",
                            [
                                HostTaskArgTemplate::positional(RuntimeExpr::Value(
                                    RuntimeValue::String("viewport()".to_owned()),
                                )),
                                HostTaskArgTemplate::positional(RuntimeExpr::Record(vec![
                                    RuntimeFieldExpr {
                                        name: "format".to_owned(),
                                        value: RuntimeExpr::Value(RuntimeValue::String(
                                            ".png".to_owned(),
                                        )),
                                    },
                                    RuntimeFieldExpr {
                                        name: "name".to_owned(),
                                        value: RuntimeExpr::Value(RuntimeValue::String(
                                            "viewport".to_owned(),
                                        )),
                                    },
                                ])),
                            ],
                        ),
                    ),
                    pending: Vec::new(),
                },
                FlowOp::ReturnExpr(RuntimeExpr::Field {
                    target: Box::new(RuntimeExpr::Local("shot".to_owned())),
                    field: "uri".to_owned(),
                }),
            ],
        },
        "agent.capture_binding",
        budget,
    )
}

fn read_resource_binding_program() -> BytecodeProgram {
    agent_controller_program(
        RuntimeFlow {
            id: flow_id("agent.read_resource_binding"),
            ops: vec![
                FlowOp::Await {
                    binding: Some(RuntimePattern::Ident("resource".to_owned())),
                    target: AwaitTarget::new(
                        NeedId("need.agent.read_resource".to_owned()),
                        TaskId("task.agent.read_resource".to_owned()),
                        HostTaskRequestTemplate::new(
                            "agent",
                            "read_resource",
                            [HostTaskArgTemplate::positional(RuntimeExpr::Value(
                                RuntimeValue::String("agent://resource/test".to_owned()),
                            ))],
                        ),
                    ),
                    pending: Vec::new(),
                },
                FlowOp::ReturnExpr(RuntimeExpr::Field {
                    target: Box::new(RuntimeExpr::Field {
                        target: Box::new(RuntimeExpr::Local("resource".to_owned())),
                        field: "body".to_owned(),
                    }),
                    field: "json".to_owned(),
                }),
            ],
        },
        "agent.read_resource_binding",
        AgentBudget::default(),
    )
}

fn entity_metadata_binding_program() -> BytecodeProgram {
    agent_controller_program(
        RuntimeFlow {
            id: flow_id("agent.entity_metadata_binding"),
            ops: vec![
                FlowOp::Await {
                    binding: Some(RuntimePattern::Ident("meta".to_owned())),
                    target: AwaitTarget::new(
                        NeedId("need.agent.entity_meta".to_owned()),
                        TaskId("task.agent.entity_meta".to_owned()),
                        HostTaskRequestTemplate::new(
                            "agent",
                            "entity_meta",
                            [HostTaskArgTemplate::positional(RuntimeExpr::EntityRef(
                                "flow.opening".to_owned(),
                            ))],
                        ),
                    ),
                    pending: Vec::new(),
                },
                FlowOp::ReturnExpr(RuntimeExpr::Field {
                    target: Box::new(RuntimeExpr::Local("meta".to_owned())),
                    field: "semantic_hash".to_owned(),
                }),
            ],
        },
        "agent.entity_metadata_binding",
        AgentBudget::default(),
    )
}

fn project_neighbors_binding_program() -> BytecodeProgram {
    agent_controller_program(
        RuntimeFlow {
            id: flow_id("agent.project_neighbors_binding"),
            ops: vec![
                FlowOp::Await {
                    binding: Some(RuntimePattern::Ident("graph".to_owned())),
                    target: AwaitTarget::new(
                        NeedId("need.agent.project_neighbors".to_owned()),
                        TaskId("task.agent.project_neighbors".to_owned()),
                        HostTaskRequestTemplate::new(
                            "agent",
                            "project_neighbors",
                            [
                                HostTaskArgTemplate::positional(RuntimeExpr::EntityRef(
                                    "flow.opening".to_owned(),
                                )),
                                HostTaskArgTemplate::positional(RuntimeExpr::Variant {
                                    path: Some("agent".to_owned()),
                                    name: AGENT_NAMED_ARGS_VARIANT.to_owned(),
                                    payload: Some(Box::new(RuntimeExpr::Record(vec![
                                        RuntimeFieldExpr {
                                            name: "depth".to_owned(),
                                            value: RuntimeExpr::Value(RuntimeValue::u32(1)),
                                        },
                                    ]))),
                                }),
                            ],
                        ),
                    ),
                    pending: Vec::new(),
                },
                FlowOp::ReturnExpr(RuntimeExpr::Field {
                    target: Box::new(RuntimeExpr::Local("graph".to_owned())),
                    field: "edge_count".to_owned(),
                }),
            ],
        },
        "agent.project_neighbors_binding",
        AgentBudget::default(),
    )
}

fn wait_binding_program() -> BytecodeProgram {
    agent_controller_program(
        RuntimeFlow {
            id: flow_id("agent.wait_binding"),
            ops: vec![
                FlowOp::Await {
                    binding: Some(RuntimePattern::Ident("obs".to_owned())),
                    target: AwaitTarget::new(
                        NeedId("need.agent.wait".to_owned()),
                        TaskId("task.agent.wait".to_owned()),
                        HostTaskRequestTemplate::new(
                            "agent",
                            "wait",
                            [
                                HostTaskArgTemplate::positional(RuntimeExpr::Record(vec![
                                    RuntimeFieldExpr {
                                        name: "kind".to_owned(),
                                        value: RuntimeExpr::Value(RuntimeValue::String(
                                            "compare".to_owned(),
                                        )),
                                    },
                                    RuntimeFieldExpr {
                                        name: "probe".to_owned(),
                                        value: RuntimeExpr::Record(vec![
                                            RuntimeFieldExpr {
                                                name: "kind".to_owned(),
                                                value: RuntimeExpr::Value(RuntimeValue::String(
                                                    "signal".to_owned(),
                                                )),
                                            },
                                            RuntimeFieldExpr {
                                                name: "target".to_owned(),
                                                value: RuntimeExpr::Value(RuntimeValue::String(
                                                    "signal.ready".to_owned(),
                                                )),
                                            },
                                        ]),
                                    },
                                    RuntimeFieldExpr {
                                        name: "op".to_owned(),
                                        value: RuntimeExpr::Value(RuntimeValue::String(
                                            "eq".to_owned(),
                                        )),
                                    },
                                    RuntimeFieldExpr {
                                        name: "value".to_owned(),
                                        value: RuntimeExpr::Value(RuntimeValue::Bool(true)),
                                    },
                                ])),
                                HostTaskArgTemplate::positional(RuntimeExpr::Variant {
                                    path: Some("agent".to_owned()),
                                    name: "named_args".to_owned(),
                                    payload: Some(Box::new(RuntimeExpr::Record(vec![
                                        RuntimeFieldExpr {
                                            name: "timeout".to_owned(),
                                            value: RuntimeExpr::Value(RuntimeValue::Duration(
                                                LogicalDuration::from_nanos(5_000_000),
                                            )),
                                        },
                                        RuntimeFieldExpr {
                                            name: "stable_frames".to_owned(),
                                            value: RuntimeExpr::Value(RuntimeValue::u32(2)),
                                        },
                                        RuntimeFieldExpr {
                                            name: "poll_frames".to_owned(),
                                            value: RuntimeExpr::Value(RuntimeValue::u32(1)),
                                        },
                                    ]))),
                                }),
                            ],
                        ),
                    ),
                    pending: Vec::new(),
                },
                FlowOp::ReturnExpr(RuntimeExpr::Field {
                    target: Box::new(RuntimeExpr::Local("obs".to_owned())),
                    field: "tick".to_owned(),
                }),
            ],
        },
        "agent.wait_binding",
        AgentBudget::default(),
    )
}

#[test]
fn wait_requires_stable_predicate_matches() {
    let session = TestSession {
        observations: vec![
            observation(1, false),
            observation(2, true),
            observation(3, true),
        ],
    };
    let mut runner = AgentRunner::new(
        session,
        NullDebugEventSink,
        NoopRagService,
        RuntimeAgentPolicy::new([RuntimeAgentCapability::Observe]),
        AgentRunnerConfig::new(SessionId::new("session.test").expect("valid session id")),
    );

    let report = runner
        .handle_host_request(AgentHostRequest::Wait(Box::new(WaitRequest {
            predicate: Predicate::Compare {
                probe: Probe::Signal {
                    target: PublicId::new("signal.ready").expect("valid public id"),
                },
                op: CompareOp::Eq,
                value: Box::new(AgentValue::Bool(true)),
            },
            timeout_millis: 5,
            stable_frames: 2,
            poll_frames: 1,
        })))
        .expect("wait succeeds");

    assert!(matches!(
        report.response,
        AgentHostResponse::Observation(observation) if observation.tick == 3
    ));
}

#[test]
fn wait_matches_entity_probe_against_string_observation_id() {
    let session = TestSession {
        observations: vec![observation_with_signal(
            1,
            "signal.current_flow",
            AgentValue::String("flow.opening".to_owned()),
        )],
    };
    let mut runner = AgentRunner::new(
        session,
        NullDebugEventSink,
        NoopRagService,
        RuntimeAgentPolicy::new([RuntimeAgentCapability::Observe]),
        AgentRunnerConfig::new(SessionId::new("session.test").expect("valid session id")),
    );

    let report = runner
        .handle_host_request(AgentHostRequest::Wait(Box::new(WaitRequest {
            predicate: Predicate::Compare {
                probe: Probe::Signal {
                    target: PublicId::new("signal.current_flow").expect("valid public id"),
                },
                op: CompareOp::Eq,
                value: Box::new(AgentValue::Entity(
                    PublicId::new("flow.opening").expect("valid public id"),
                )),
            },
            timeout_millis: 5,
            stable_frames: 1,
            poll_frames: 1,
        })))
        .expect("wait succeeds");

    assert!(matches!(
        report.response,
        AgentHostResponse::Observation(observation) if observation.tick == 1
    ));
}

#[test]
fn wait_matches_enabled_action_target_predicate() {
    let session = TestSession {
        observations: vec![
            observation_with_action_target(1, "choice.opening.listen", false),
            observation_with_action_target(2, "choice.opening.listen", true),
        ],
    };
    let mut runner = AgentRunner::new(
        session,
        NullDebugEventSink,
        NoopRagService,
        RuntimeAgentPolicy::new([RuntimeAgentCapability::Observe]),
        AgentRunnerConfig::new(SessionId::new("session.test").expect("valid session id")),
    );

    let report = runner
        .handle_host_request(AgentHostRequest::Wait(Box::new(WaitRequest {
            predicate: Predicate::ActionEnabled {
                target: PublicId::new("choice.opening.listen").expect("valid public id"),
            },
            timeout_millis: 5,
            stable_frames: 1,
            poll_frames: 1,
        })))
        .expect("wait succeeds when target action becomes enabled");

    assert!(matches!(
        report.response,
        AgentHostResponse::Observation(observation) if observation.tick == 2
    ));
}

#[test]
fn effect_form_wait_call_lowers_to_host_wait_request() {
    let request = agent_host_request_from_call(&RuntimeCall {
        callee: "wait".to_owned(),
        args: vec![
            "signal(@signal.current_flow).eq(@flow.opening)".to_owned(),
            "timeout = 5s".to_owned(),
            "stable_frames = 2u32".to_owned(),
            "poll_frames = 1u32".to_owned(),
        ],
    })
    .expect("effect-form wait lowers");

    let AgentHostRequest::Wait(request) = request else {
        panic!("expected wait host request");
    };
    assert_eq!(request.timeout_millis, 5_000);
    assert_eq!(request.stable_frames, 2);
    assert_eq!(request.poll_frames, 1);
    assert!(matches!(
        request.predicate,
        Predicate::Compare {
            probe: Probe::Signal { ref target },
            op: CompareOp::Eq,
            ref value,
        } if target.as_str() == "signal.current_flow"
            && matches!(
                value.as_ref(),
                AgentValue::Entity(value) if value.as_str() == "flow.opening"
        )
    ));
}

#[test]
fn effect_form_wait_call_lowers_action_enabled_predicate() {
    let request = agent_host_request_from_call(&RuntimeCall {
        callee: "wait".to_owned(),
        args: vec![
            "action_enabled(@choice.opening.listen)".to_owned(),
            "timeout = 5s".to_owned(),
        ],
    })
    .expect("effect-form action-enabled wait lowers");

    let AgentHostRequest::Wait(request) = request else {
        panic!("expected wait host request");
    };
    assert!(matches!(
        request.predicate,
        Predicate::ActionEnabled { ref target } if target.as_str() == "choice.opening.listen"
    ));
}

#[test]
fn effect_form_observe_defaults_to_object_payloads() {
    let request = agent_host_request_from_call(&RuntimeCall {
        callee: "observe".to_owned(),
        args: Vec::new(),
    })
    .expect("effect-form observe lowers");

    let AgentHostRequest::Observe(request) = request else {
        panic!("expected observe host request");
    };
    assert!(request.include_objects);
    assert!(!request.include_images);
    assert!(!request.include_logs);
}

#[test]
fn effect_form_advance_text_call_lowers_to_host_action() {
    let request = agent_host_request_from_call(&RuntimeCall {
        callee: "advance_text".to_owned(),
        args: Vec::new(),
    })
    .expect("effect-form advance_text lowers");

    assert!(matches!(
        request,
        AgentHostRequest::Act(action) if matches!(*action, AgentAction::AdvanceText)
    ));
}

#[test]
fn effect_form_invoke_call_lowers_to_host_action() {
    let request = agent_host_request_from_call(&RuntimeCall {
        callee: "invoke".to_owned(),
        args: vec![
            "@activity.inventory".to_owned(),
            ".open".to_owned(),
            r#"{ label = "main", index = 7u32, focused = true }"#.to_owned(),
        ],
    })
    .expect("effect-form invoke lowers");

    let AgentHostRequest::Act(action) = request else {
        panic!("expected action host request");
    };
    let AgentAction::Invoke(invoke) = *action else {
        panic!("expected invoke action");
    };
    assert_eq!(invoke.target.as_str(), "activity.inventory");
    assert_eq!(invoke.action, "open");
    assert_eq!(
        invoke.args.get("label"),
        Some(&AgentValue::String("main".to_owned()))
    );
    assert_eq!(invoke.args.get("index"), Some(&AgentValue::U64(7)));
    assert_eq!(invoke.args.get("focused"), Some(&AgentValue::Bool(true)));
}

#[test]
fn effect_form_pointer_click_lowers_to_physical_action() {
    let request = agent_host_request_from_call(&RuntimeCall {
        callee: "pointer.click".to_owned(),
        args: vec![
            "viewport_point(12u32, 34u32)".to_owned(),
            "button = .secondary".to_owned(),
        ],
    })
    .expect("effect-form pointer.click lowers");

    let AgentHostRequest::Act(action) = request else {
        panic!("expected action host request");
    };
    assert!(matches!(
        *action,
        AgentAction::PointerClick {
            x: 12,
            y: 34,
            button: PointerButton::Secondary
        }
    ));
}

#[test]
fn physical_pointer_click_requires_runtime_policy_grant() {
    let request = AgentHostRequest::Act(Box::new(AgentAction::PointerClick {
        x: 12,
        y: 34,
        button: PointerButton::Primary,
    }));
    let mut denied = AgentRunner::new(
        TestSession::default(),
        NullDebugEventSink,
        NoopRagService,
        RuntimeAgentPolicy::new([RuntimeAgentCapability::Act]),
        AgentRunnerConfig::new(SessionId::new("session.test").expect("valid session id")),
    );
    let error = denied
        .handle_host_request(request.clone())
        .expect_err("physical action is denied without physical policy");
    assert!(matches!(
        error,
        AgentRunError::PolicyDenied("agent.act.physical")
    ));

    let mut granted = AgentRunner::new(
        TestSession::default(),
        NullDebugEventSink,
        NoopRagService,
        RuntimeAgentPolicy::new([RuntimeAgentCapability::ActPhysical]),
        AgentRunnerConfig::new(SessionId::new("session.test").expect("valid session id")),
    );
    let report = granted
        .handle_host_request(request)
        .expect("physical policy allows pointer.click host action");
    assert!(matches!(report.response, AgentHostResponse::Action(_)));
}

#[test]
fn custom_task_attach_records_runtime_resource_payload() {
    let request = HostTaskRequest::Custom {
        capability: arcweft_core::task::HostCapabilityId("agent".to_owned()),
        operation: "attach".to_owned(),
        args: vec![RuntimePayload::new(RuntimeValue::Record(vec![
            runtime_field(
                "uri",
                RuntimeValue::String("arcweft://session/cli/observation/latest.json".to_owned()),
            ),
            runtime_field(
                "kind",
                RuntimeValue::String("observation_latest".to_owned()),
            ),
        ]))],
        named_args: Vec::new(),
    };
    let request = agent_host_request_from_task(&request).expect("attach task lowers");
    let mut runner = AgentRunner::new(
        TestSession::default(),
        RecordingDebugSink::default(),
        NoopRagService,
        RuntimeAgentPolicy::new([RuntimeAgentCapability::DebugRecord]),
        AgentRunnerConfig::new(SessionId::new("session.test").expect("valid session id")),
    );

    let report = runner
        .handle_host_request(request)
        .expect("debug record policy allows attach");

    assert!(matches!(report.response, AgentHostResponse::Unit));
    assert!(
        runner
            .debug_mut()
            .events
            .iter()
            .any(|event| event.payload["attachment"]["uri"]
                == "arcweft://session/cli/observation/latest.json")
    );
}

#[test]
fn observation_payload_exposes_action_targets_for_contains_checks() {
    let response = AgentHostResponse::Observation(Box::new(ObservationEnvelope {
        tick: 7,
        frame_id: "frame.7".to_owned(),
        state_hash: "state.7".to_owned(),
        render_hash: "render.7".to_owned(),
        actions: vec![AgentActionTarget {
            id: "action.select_choice.choice.opening.listen".to_owned(),
            target: "choice.opening.listen".to_owned(),
            action: AgentActionKind::SelectChoice,
            kind: AgentActionDispatch::Semantic,
            enabled: true,
        }],
        signals: BTreeMap::new(),
        payload: serde_json::json!({}),
    }));

    let RuntimeValue::Record(fields) = runtime_payload_from_response(&response).0 else {
        panic!("observation payload is a record");
    };
    let RuntimeValue::Seq(actions) =
        &runtime_record_get(&fields, "actions").expect("actions field exists")
    else {
        panic!("actions field is a sequence");
    };

    assert_eq!(actions.len(), 1);
    assert!(matches!(
        actions.value_at(0),
        RuntimeValue::Record(ref fields)
            if runtime_record_get(fields, "target")
                == Ok(&RuntimeValue::String("choice.opening.listen".to_owned()))
    ));
}

#[test]
fn observation_payload_exposes_observed_objects_for_visual_regression_scripts() {
    let response = AgentHostResponse::Observation(Box::new(ObservationEnvelope {
        tick: 8,
        frame_id: "frame.8".to_owned(),
        state_hash: "state.8".to_owned(),
        render_hash: "render.8".to_owned(),
        actions: Vec::new(),
        signals: BTreeMap::new(),
        payload: serde_json::json!({
            "objects": [
                {
                    "id": "object.dialogue.0.0",
                    "parent_id": "object.dialogue.0",
                    "entity": "dialogue.main",
                    "layer": "dialogue.rich_text",
                    "role": "dialogue_view",
                    "visible": true,
                    "enabled": true,
                    "bbox": {
                        "space": "viewport",
                        "x": 24,
                        "y": 384,
                        "width": 752,
                        "height": 168
                    },
                    "text": "Hello"
                }
            ]
        }),
    }));

    let RuntimeValue::Record(fields) = runtime_payload_from_response(&response).0 else {
        panic!("observation payload is a record");
    };
    let RuntimeValue::Seq(objects) =
        &runtime_record_get(&fields, "objects").expect("objects field exists")
    else {
        panic!("objects field is a sequence");
    };
    let RuntimeValue::Record(object_fields) = objects.value_at(0) else {
        panic!("object is a record");
    };
    let RuntimeValue::Record(bbox_fields) =
        runtime_record_get(&object_fields, "bbox").expect("bbox field exists")
    else {
        panic!("bbox field is a record");
    };

    assert_eq!(
        runtime_record_get(&object_fields, "role"),
        Ok(&RuntimeValue::String("dialogue_view".to_owned()))
    );
    assert_eq!(
        runtime_record_get(bbox_fields, "width"),
        Ok(&RuntimeValue::u32(752))
    );
    assert_eq!(
        runtime_record_get(bbox_fields, "height"),
        Ok(&RuntimeValue::u32(168))
    );
}

#[test]
fn effect_form_wait_call_lowers_composite_predicate() {
    let request = agent_host_request_from_call(&RuntimeCall {
        callee: "wait".to_owned(),
        args: vec![
            "all(exists(signal(@signal.ready)), not(metric(@metric.fps).lt(30.0f32)))".to_owned(),
            "timeout = 5s".to_owned(),
        ],
    })
    .expect("effect-form composite wait lowers");

    let AgentHostRequest::Wait(request) = request else {
        panic!("expected wait host request");
    };
    assert!(
        matches!(request.predicate, Predicate::All { ref predicates } if predicates.len() == 2)
    );
}

#[test]
fn wait_matches_composite_float_predicate() {
    let session = TestSession {
        observations: vec![ObservationEnvelope {
            tick: 1,
            frame_id: "frame.1".to_owned(),
            state_hash: "state.1".to_owned(),
            render_hash: "render.1".to_owned(),
            actions: Vec::new(),
            signals: BTreeMap::from([
                ("signal.ready".to_owned(), AgentValue::Bool(true)),
                ("metric.fps".to_owned(), AgentValue::F64(60.0)),
            ]),
            payload: serde_json::json!({}),
        }],
    };
    let mut runner = AgentRunner::new(
        session,
        NullDebugEventSink,
        NoopRagService,
        RuntimeAgentPolicy::new([RuntimeAgentCapability::Observe]),
        AgentRunnerConfig::new(SessionId::new("session.test").expect("valid session id")),
    );

    let report = runner
        .handle_host_request(AgentHostRequest::Wait(Box::new(WaitRequest {
            predicate: Predicate::All {
                predicates: vec![
                    Predicate::Exists {
                        probe: Probe::Signal {
                            target: PublicId::new("signal.ready").expect("valid public id"),
                        },
                    },
                    Predicate::Not {
                        predicate: Box::new(Predicate::Compare {
                            probe: Probe::Metric {
                                target: PublicId::new("metric.fps").expect("valid public id"),
                            },
                            op: CompareOp::Less,
                            value: Box::new(AgentValue::F64(30.0)),
                        }),
                    },
                ],
            },
            timeout_millis: 5,
            stable_frames: 1,
            poll_frames: 1,
        })))
        .expect("composite wait succeeds");

    assert!(matches!(
        report.response,
        AgentHostResponse::Observation(observation) if observation.tick == 1
    ));
}

#[test]
fn wait_matches_state_and_observation_field_predicates() {
    let session = TestSession {
        observations: vec![ObservationEnvelope {
            tick: 2,
            frame_id: "frame.2".to_owned(),
            state_hash: "state.2".to_owned(),
            render_hash: "render.2".to_owned(),
            actions: Vec::new(),
            signals: BTreeMap::new(),
            payload: serde_json::json!({
                "state": {
                    "route.phase": "opening"
                }
            }),
        }],
    };
    let mut runner = AgentRunner::new(
        session,
        NullDebugEventSink,
        NoopRagService,
        RuntimeAgentPolicy::default(),
        AgentRunnerConfig::new(SessionId::new("session.test").expect("valid session id")),
    );

    let report = runner
        .handle_host_request(AgentHostRequest::Wait(Box::new(WaitRequest {
            predicate: Predicate::All {
                predicates: vec![
                    Predicate::Compare {
                        probe: Probe::StatePath {
                            path: DebugStatePath::new("route.phase").expect("valid state path"),
                        },
                        op: CompareOp::Eq,
                        value: Box::new(AgentValue::String("opening".to_owned())),
                    },
                    Predicate::Compare {
                        probe: Probe::ObservationField {
                            path: ObservationFieldPath::new("tick")
                                .expect("valid observation field path"),
                        },
                        op: CompareOp::GreaterOrEqual,
                        value: Box::new(AgentValue::I64(2)),
                    },
                ],
            },
            timeout_millis: 5,
            stable_frames: 1,
            poll_frames: 1,
        })))
        .expect("state and observation wait succeeds");

    assert!(matches!(
        report.response,
        AgentHostResponse::Observation(observation) if observation.tick == 2
    ));
}

#[test]
fn wait_matches_diagnostics_has_error_predicate() {
    let session = TestSession {
        observations: vec![
            ObservationEnvelope {
                tick: 1,
                frame_id: "frame.1".to_owned(),
                state_hash: "state.1".to_owned(),
                render_hash: "render.1".to_owned(),
                actions: Vec::new(),
                signals: BTreeMap::new(),
                payload: serde_json::json!({
                    "diagnostics": [
                        { "severity": "warning", "message": "not fatal" }
                    ]
                }),
            },
            ObservationEnvelope {
                tick: 2,
                frame_id: "frame.2".to_owned(),
                state_hash: "state.2".to_owned(),
                render_hash: "render.2".to_owned(),
                actions: Vec::new(),
                signals: BTreeMap::new(),
                payload: serde_json::json!({
                    "diagnostics": [
                        { "severity": "error", "message": "render mismatch" }
                    ]
                }),
            },
        ],
    };
    let mut runner = AgentRunner::new(
        session,
        NullDebugEventSink,
        NoopRagService,
        RuntimeAgentPolicy::new([RuntimeAgentCapability::Observe]),
        AgentRunnerConfig::new(SessionId::new("session.test").expect("valid session id")),
    );

    let report = runner
        .handle_host_request(AgentHostRequest::Wait(Box::new(WaitRequest {
            predicate: Predicate::DiagnosticsHasError,
            timeout_millis: 5,
            stable_frames: 1,
            poll_frames: 1,
        })))
        .expect("diagnostic wait succeeds");

    assert!(matches!(
        report.response,
        AgentHostResponse::Observation(observation) if observation.tick == 2
    ));
}

#[test]
fn assertion_host_request_records_passed_expect() {
    let mut runner = AgentRunner::new(
        TestSession::default(),
        RecordingDebugSink::default(),
        NoopRagService,
        RuntimeAgentPolicy::default(),
        AgentRunnerConfig::new(SessionId::new("session.test").expect("valid session id")),
    );

    let report = runner
        .handle_host_request(AgentHostRequest::Assert(Box::new(AgentAssertionRequest {
            kind: AgentAssertionKind::Expect,
            condition: true,
            message: "accepted should be true".to_owned(),
        })))
        .expect("passing assertion succeeds");

    assert!(matches!(report.response, AgentHostResponse::Unit));
    assert!(runner.debug_mut().events.iter().any(|event| {
        event.kind == DebugEventKind::Assertion
            && event.payload["kind"] == "expect"
            && event.payload["passed"] == serde_json::json!(true)
    }));
}

#[test]
fn assertion_host_request_fails_deny_with_structured_event() {
    let mut runner = AgentRunner::new(
        TestSession::default(),
        RecordingDebugSink::default(),
        NoopRagService,
        RuntimeAgentPolicy::default(),
        AgentRunnerConfig::new(SessionId::new("session.test").expect("valid session id")),
    );

    let error = runner
        .handle_host_request(AgentHostRequest::Assert(Box::new(AgentAssertionRequest {
            kind: AgentAssertionKind::Deny,
            condition: true,
            message: "route should not be open".to_owned(),
        })))
        .expect_err("failing deny stops the controller");

    assert!(matches!(
        error,
        AgentRunError::AssertionFailed {
            kind: AgentAssertionKind::Deny,
            ref message,
        } if message == "route should not be open"
    ));
    assert!(runner.debug_mut().events.iter().any(|event| {
        event.kind == DebugEventKind::Assertion
            && event.payload["kind"] == "deny"
            && event.payload["passed"] == serde_json::json!(false)
    }));
}

#[test]
fn capture_requires_policy_capability() {
    let mut runner = AgentRunner::new(
        TestSession::default(),
        NullDebugEventSink,
        NoopRagService,
        RuntimeAgentPolicy::default(),
        AgentRunnerConfig::new(SessionId::new("session.test").expect("valid session id")),
    );

    let error = runner
        .handle_host_request(AgentHostRequest::Capture(Box::new(CaptureRequest {
            target: CaptureTarget::Viewport,
            format: CaptureFormat::Png,
            capture_kind: "color".to_owned(),
            name: "viewport".to_owned(),
        })))
        .expect_err("capture is denied");

    assert!(matches!(
        error,
        AgentRunError::PolicyDenied("agent.capture")
    ));
}

#[test]
fn controller_bytecode_dispatches_effect_calls_to_runner_host_boundary() {
    let session = TestSession {
        observations: vec![observation(1, true)],
    };
    let mut runner = AgentRunner::new(
        session,
        NullDebugEventSink,
        NoopRagService,
        RuntimeAgentPolicy::new([
            RuntimeAgentCapability::Observe,
            RuntimeAgentCapability::DebugRecord,
        ]),
        AgentRunnerConfig::new(SessionId::new("session.test").expect("valid session id")),
    );
    let program = observe_checkpoint_program();
    let entry = program.entries[0].id.clone();

    let report = runner
        .run_controller_bytecode(program, &entry, AgentControllerRunConfig::default())
        .expect("controller bytecode runs");

    assert_eq!(report.steps, 1);
    assert_eq!(report.host_calls, 2);
    assert_eq!(report.responses.len(), 2);
    assert!(matches!(
        &report.responses[0],
        AgentHostResponse::Observation(observation) if observation.tick == 1
    ));
    assert!(matches!(report.responses[1], AgentHostResponse::Unit));
}

#[test]
fn controller_bundle_runs_through_bytecode_host_boundary() {
    let session = TestSession {
        observations: vec![observation(1, true)],
    };
    let mut runner = AgentRunner::new(
        session,
        NullDebugEventSink,
        NoopRagService,
        RuntimeAgentPolicy::new([
            RuntimeAgentCapability::Observe,
            RuntimeAgentCapability::DebugRecord,
        ]),
        AgentRunnerConfig::new(SessionId::new("session.test").expect("valid session id")),
    );
    let bundle = observe_checkpoint_bundle();

    let report = runner
        .run_controller_bundle(&bundle, AgentControllerRunConfig::default())
        .expect("controller bundle runs");

    assert_eq!(report.host_calls, 2);
    assert!(matches!(
        &report.responses[0],
        AgentHostResponse::Observation(observation) if observation.tick == 1
    ));
}

fn assert_agent_artifact_mismatch(bundle: &ArcweftBundle) {
    let mut runner = AgentRunner::new(
        TestSession {
            observations: vec![observation(1, true)],
        },
        NullDebugEventSink,
        NoopRagService,
        RuntimeAgentPolicy::new([
            RuntimeAgentCapability::Observe,
            RuntimeAgentCapability::DebugRecord,
        ]),
        AgentRunnerConfig::new(SessionId::new("session.test").expect("valid session id")),
    );

    assert!(matches!(
        runner.run_controller_bundle(bundle, AgentControllerRunConfig::default()),
        Err(AgentRunError::AgentArtifactMismatch { .. })
    ));
}

#[test]
fn controller_bundle_rejects_tampered_entry_bound_manifest_fields() {
    let original = observe_checkpoint_bundle();

    let mut entry = original.clone();
    entry.agent.as_mut().unwrap().entry_id =
        PublicId::new("entry.agent.other").expect("valid tampered entry");
    assert_agent_artifact_mismatch(&entry);

    let mut controller = original.clone();
    controller.agent.as_mut().unwrap().controller_id =
        CallableId::new("test::crate.other").expect("valid tampered controller");
    assert_agent_artifact_mismatch(&controller);

    let mut binding = original.clone();
    binding.agent.as_mut().unwrap().entry_binding_hash = StableHash::from_blake3_bytes([9; 32]);
    assert_agent_artifact_mismatch(&binding);

    let mut contract = original.clone();
    contract.agent.as_mut().unwrap().controller_contract_hash =
        StableHash::from_blake3_bytes([9; 32]);
    assert_agent_artifact_mismatch(&contract);

    let mut policy = original.clone();
    policy.agent.as_mut().unwrap().policy_hash = StableHash::from_blake3_bytes([9; 32]);
    assert_agent_artifact_mismatch(&policy);

    let mut budget = original;
    budget.agent.as_mut().unwrap().budget.max_vm_steps += 1;
    assert_agent_artifact_mismatch(&budget);
}

#[test]
fn controller_bytecode_rejects_explicit_non_agent_entry_before_execution() {
    let mut program = observe_checkpoint_program();
    program.entries[0].kind = RuntimeEntryKind::Game;
    let entry = program.entries[0].id.clone();
    let mut runner = AgentRunner::new(
        TestSession::default(),
        NullDebugEventSink,
        NoopRagService,
        RuntimeAgentPolicy::default(),
        AgentRunnerConfig::new(SessionId::new("session.test").expect("valid session id")),
    );

    assert!(matches!(
        runner.run_controller_bytecode(program, &entry, AgentControllerRunConfig::default()),
        Err(AgentRunError::InvalidControllerEntry { .. })
    ));
}

#[test]
fn controller_bundle_rejects_strict_project_binding_mismatch_before_execution() {
    let session = TestSession {
        observations: vec![observation(1, true)],
    };
    let mut runner = AgentRunner::new(
        session,
        RecordingDebugSink::default(),
        NoopRagService,
        RuntimeAgentPolicy::new([
            RuntimeAgentCapability::Observe,
            RuntimeAgentCapability::DebugRecord,
        ]),
        AgentRunnerConfig::new(SessionId::new("session.test").expect("valid session id")),
    );
    let mut bundle = observe_checkpoint_bundle();
    let manifest = bundle.agent.as_mut().expect("agent manifest exists");
    manifest.project_binding.mode = ProjectBindingMode::Strict;
    manifest.project_binding.program_hash =
        StableHash::new("different-program").expect("valid program hash");

    let error = runner
        .run_controller_bundle(&bundle, AgentControllerRunConfig::default())
        .expect_err("strict binding mismatch is rejected");

    assert!(matches!(
        error,
        AgentRunError::ProjectBindingMismatch {
            expected_program_hash,
            actual_program_hash,
            mode: ProjectBindingMode::Strict,
            detail,
        } if expected_program_hash == "different-program"
            && actual_program_hash == "hash"
            && detail == "strict program hash mismatch"
    ));
    assert_eq!(runner.session_mut().observations.len(), 1);
    assert!(runner.debug_mut().events.is_empty());
}

#[test]
fn controller_bundle_rejects_compatible_project_entity_mismatch_before_execution() {
    let session = TestSession {
        observations: vec![observation(1, true)],
    };
    let mut runner = AgentRunner::new(
        session,
        RecordingDebugSink::default(),
        NoopRagService,
        RuntimeAgentPolicy::new([
            RuntimeAgentCapability::Observe,
            RuntimeAgentCapability::DebugRecord,
        ]),
        AgentRunnerConfig::new(SessionId::new("session.test").expect("valid session id")),
    );
    let mut bundle = observe_checkpoint_bundle();
    let manifest = bundle.agent.as_mut().expect("agent manifest exists");
    manifest.project_binding.required_entities = vec![RequiredEntity {
        public_id: PublicId::new("signal.ready").expect("valid public id"),
        kind: "signal".to_owned(),
        semantic_hash: StableHash::new("shape.signal.ready.v1").expect("valid semantic hash"),
        source_anchor: None,
    }];

    let error = runner
        .run_controller_bundle(&bundle, AgentControllerRunConfig::default())
        .expect_err("compatible entity mismatch is rejected");

    assert!(matches!(
        error,
        AgentRunError::ProjectBindingMismatch {
            expected_program_hash,
            actual_program_hash,
            mode: ProjectBindingMode::Compatible,
            detail,
        } if expected_program_hash == "program-test"
            && actual_program_hash == "hash"
            && detail == "required entity signal.ready is missing"
    ));
    assert_eq!(runner.session_mut().observations.len(), 1);
    assert!(runner.debug_mut().events.is_empty());
}

#[test]
fn controller_bundle_requires_launch_grant_for_verified_effects_before_execution() {
    let mut runner = AgentRunner::new(
        TestSession::default(),
        RecordingDebugSink::default(),
        NoopRagService,
        RuntimeAgentPolicy::default(),
        AgentRunnerConfig::new(SessionId::new("session.test").expect("valid session id")),
    );
    let bundle = capture_binding_bundle_with_budget(AgentBudget::default());

    let error = runner
        .run_controller_bundle(&bundle, AgentControllerRunConfig::default())
        .expect_err("verified capture effect requires launch grant");

    assert!(matches!(
        error,
        AgentRunError::EffectPolicy(AgentEffectPolicyError::MissingGrant {
            capability: "agent.capture",
        })
    ));
    assert!(runner.debug_mut().events.is_empty());
}

#[test]
fn controller_bundle_rejects_host_request_absent_from_verified_effects() {
    let session = TestSession {
        observations: vec![observation(1, true)],
    };
    let mut runner = AgentRunner::new(
        session,
        RecordingDebugSink::default(),
        NoopRagService,
        RuntimeAgentPolicy::new([
            RuntimeAgentCapability::Observe,
            RuntimeAgentCapability::DebugRecord,
        ]),
        AgentRunnerConfig::new(SessionId::new("session.test").expect("valid session id")),
    );
    let mut bundle = observe_checkpoint_bundle();
    let manifest = bundle.agent.as_mut().expect("agent manifest exists");
    manifest.verified_effects = VerifiedEffectSummary::new(
        1,
        vec![EffectCapability::new("agent.observe")],
        vec![EffectCapability::new("agent.observe")],
        StableHash::new("blake3:test-observe-only").expect("valid effect hash"),
    );

    let error = runner
        .run_controller_bundle(&bundle, AgentControllerRunConfig::default())
        .expect_err("checkpoint requires a verified debug.record effect");

    assert!(matches!(
        error,
        AgentRunError::EffectPolicy(AgentEffectPolicyError::UndeclaredRequestEffect {
            effect: "debug.record",
        })
    ));
}

#[test]
fn controller_bytecode_resumes_bound_capture_response() {
    let mut runner = AgentRunner::new(
        TestSession::default(),
        NullDebugEventSink,
        NoopRagService,
        RuntimeAgentPolicy::new([
            RuntimeAgentCapability::Observe,
            RuntimeAgentCapability::Capture,
        ]),
        AgentRunnerConfig::new(SessionId::new("session.test").expect("valid session id")),
    );
    let program = capture_binding_program();
    let entry = program.entries[0].id.clone();

    let report = runner
        .run_controller_bytecode(program, &entry, AgentControllerRunConfig::default())
        .expect("controller bytecode runs");

    assert_eq!(report.host_calls, 1);
    assert!(matches!(
        &report.responses[0],
        AgentHostResponse::Capture(result) if result.uri.as_str() == "agent://capture/test"
    ));
    assert!(matches!(
        report.final_status,
        Some(FlowFiberStatus::Done(FlowExit::Return(ref value)))
            if value == "agent://capture/test"
    ));
}

#[test]
fn controller_bundle_enforces_agent_manifest_capture_budget() {
    let budget = AgentBudget {
        max_captures: 0,
        ..AgentBudget::default()
    };
    let bundle = capture_binding_bundle_with_budget(budget);
    let mut runner = AgentRunner::new(
        TestSession::default(),
        NullDebugEventSink,
        NoopRagService,
        RuntimeAgentPolicy::new([
            RuntimeAgentCapability::Observe,
            RuntimeAgentCapability::Capture,
        ]),
        AgentRunnerConfig::new(SessionId::new("session.test").expect("valid session id")),
    );

    let error = runner
        .run_controller_bundle(&bundle, AgentControllerRunConfig::default())
        .expect_err("capture budget stops controller bundle");

    assert!(matches!(
        error,
        AgentRunError::ControllerResourceBudgetExceeded {
            kind: "capture",
            limit: 0,
            attempted: 1,
        }
    ));
}

#[test]
fn controller_bytecode_resumes_bound_resource_response_fields() {
    let mut runner = AgentRunner::new(
        TestSession::default(),
        NullDebugEventSink,
        NoopRagService,
        RuntimeAgentPolicy::new([RuntimeAgentCapability::ResourceRead]),
        AgentRunnerConfig::new(SessionId::new("session.test").expect("valid session id")),
    );
    let program = read_resource_binding_program();
    let entry = program.entries[0].id.clone();

    let report = runner
        .run_controller_bytecode(program, &entry, AgentControllerRunConfig::default())
        .expect("controller bytecode runs");

    assert_eq!(report.host_calls, 1);
    assert!(matches!(
        &report.responses[0],
        AgentHostResponse::Resource(resource) if resource["uri"] == "agent://resource/test"
    ));
    assert!(matches!(
        report.final_status,
        Some(FlowFiberStatus::Done(FlowExit::Return(ref value)))
            if value == "{\"uri\":\"agent://resource/test\"}"
    ));
}

#[test]
fn controller_bytecode_resumes_bound_entity_metadata_response_fields() {
    let mut runner = AgentRunner::new(
        MetadataSession {
            project_entities: vec![RequiredEntity {
                public_id: PublicId::new("flow.opening").expect("valid public id"),
                kind: "flow".to_owned(),
                semantic_hash: StableHash::new("hir:flow:flow.opening:_")
                    .expect("valid semantic hash"),
                source_anchor: Some(RequiredEntitySourceAnchor {
                    path: "game.arcw".to_owned(),
                    start_byte: 8,
                    end_byte: 21,
                    start: Some(RequiredEntitySourcePosition { line: 2, column: 1 }),
                    end: Some(RequiredEntitySourcePosition {
                        line: 2,
                        column: 14,
                    }),
                }),
            }],
            project_graph: AgentProjectGraph::default(),
        },
        NullDebugEventSink,
        NoopRagService,
        RuntimeAgentPolicy::new([RuntimeAgentCapability::DebugRead]),
        AgentRunnerConfig::new(SessionId::new("session.test").expect("valid session id")),
    );
    let program = entity_metadata_binding_program();
    let entry = program.entries[0].id.clone();

    let report = runner
        .run_controller_bytecode(program, &entry, AgentControllerRunConfig::default())
        .expect("controller bytecode runs");

    assert_eq!(report.host_calls, 1);
    assert!(matches!(
        &report.responses[0],
        AgentHostResponse::EntityMetadata(metadata)
            if metadata.public_id.as_str() == "flow.opening"
                && metadata.semantic_hash.as_str() == "hir:flow:flow.opening:_"
                && metadata
                    .source_anchor
                    .as_ref()
                    .is_some_and(|source| source.path == "game.arcw")
    ));
    assert!(matches!(
        report.final_status,
        Some(FlowFiberStatus::Done(FlowExit::Return(ref value)))
            if value == "hir:flow:flow.opening:_"
    ));
}

#[test]
fn controller_bytecode_resumes_project_graph_neighborhood_fields() {
    let mut runner = AgentRunner::new(
        MetadataSession {
            project_entities: Vec::new(),
            project_graph: project_neighbors_test_graph(),
        },
        NullDebugEventSink,
        NoopRagService,
        RuntimeAgentPolicy::new([RuntimeAgentCapability::DebugRead]),
        AgentRunnerConfig::new(SessionId::new("session.test").expect("valid session id")),
    );
    let program = project_neighbors_binding_program();
    let entry = program.entries[0].id.clone();

    let report = runner
        .run_controller_bytecode(program, &entry, AgentControllerRunConfig::default())
        .expect("controller bytecode runs");

    assert_eq!(report.host_calls, 1);
    assert!(matches!(
        &report.responses[0],
        AgentHostResponse::ProjectGraphNeighborhood(neighborhood)
            if neighborhood.root.as_str() == "flow.opening"
                && neighborhood.symbols.len() == 2
                && neighborhood.edges.len() == 1
                && neighborhood.edges[0].edge_kind == "contains_entity"
                && neighborhood.symbols.iter().any(|symbol| symbol.public_id.as_ref().is_some_and(|id| id.as_str() == "flow.opening")
                    && symbol.flow_control.is_some_and(|summary| summary.dynamic_goto_count == 1 && summary.has_dynamic_control))
    ));
    let flow_symbol = match &report.responses[0] {
        AgentHostResponse::ProjectGraphNeighborhood(neighborhood) => neighborhood
            .symbols
            .iter()
            .find(|symbol| {
                symbol
                    .public_id
                    .as_ref()
                    .is_some_and(|id| id.as_str() == "flow.opening")
            })
            .expect("flow symbol exists"),
        _ => panic!("project graph response"),
    };
    let RuntimeValue::Record(fields) = runtime_project_graph_symbol_payload(flow_symbol) else {
        panic!("symbol payload is a record");
    };
    assert!(matches!(
        runtime_record_get(&fields, "has_dynamic_control"),
        Ok(RuntimeValue::Bool(true))
    ));
    assert!(matches!(
        runtime_record_get(&fields, "dynamic_goto_count"),
        Ok(RuntimeValue::UInt(arcweft_core::value::RuntimeUInt::U32(1)))
    ));
    let summary_symbol = match &report.responses[0] {
        AgentHostResponse::ProjectGraphNeighborhood(neighborhood) => neighborhood
            .symbols
            .iter()
            .find(|symbol| symbol.kind == "project_summary")
            .expect("project summary symbol exists"),
        _ => panic!("project graph response"),
    };
    let RuntimeValue::Record(summary_fields) = runtime_project_graph_symbol_payload(summary_symbol)
    else {
        panic!("summary symbol payload is a record");
    };
    assert!(matches!(
        runtime_record_get(&summary_fields, "has_project_summary"),
        Ok(RuntimeValue::Bool(true))
    ));
    assert!(matches!(
        runtime_record_get(&summary_fields, "relation_count"),
        Ok(RuntimeValue::UInt(arcweft_core::value::RuntimeUInt::U32(1)))
    ));
    assert!(matches!(
        report.final_status,
        Some(FlowFiberStatus::Done(FlowExit::Return(_)))
    ));
}

#[test]
fn resource_runtime_payload_preserves_json_body_value() {
    let json_payload = runtime_resource_payload(&serde_json::json!({
        "uri": "agent://resource/json",
        "kind": "observation_latest",
        "mime_type": "application/json",
        "hash": "json.hash",
        "body": {
            "body_kind": "json",
            "body": {
                "uri": "agent://resource/json",
                "tick": 3,
                "matched": true
            }
        }
    }));
    let RuntimeValue::Record(resource_fields) = json_payload else {
        panic!("resource payload is a record");
    };
    let RuntimeValue::Record(body_fields) =
        runtime_record_get(&resource_fields, "body").expect("body field exists")
    else {
        panic!("body payload is a record");
    };
    assert_eq!(
        runtime_record_string(body_fields, "kind").expect("body kind is a string"),
        "json"
    );
    assert_eq!(
        runtime_record_string(body_fields, "json").expect("body json is a string"),
        "{\"matched\":true,\"tick\":3,\"uri\":\"agent://resource/json\"}"
    );
    let RuntimeValue::Record(value_fields) =
        runtime_record_get(body_fields, "value").expect("body value exists")
    else {
        panic!("json body value is a record");
    };
    assert_eq!(
        runtime_record_string(value_fields, "uri").expect("json uri is a string"),
        "agent://resource/json"
    );
    assert!(matches!(
        runtime_record_get(value_fields, "matched").expect("matched field exists"),
        RuntimeValue::Bool(true)
    ));
}

#[test]
fn resource_runtime_payload_preserves_text_body_value() {
    let text_payload = runtime_resource_payload(&serde_json::json!({
        "uri": "agent://resource/text",
        "kind": "logs",
        "mime_type": "text/plain",
        "hash": "text.hash",
        "body": {
            "body_kind": "text",
            "body": "hello"
        }
    }));
    let RuntimeValue::Record(resource_fields) = text_payload else {
        panic!("resource payload is a record");
    };
    let RuntimeValue::Record(body_fields) =
        runtime_record_get(&resource_fields, "body").expect("body field exists")
    else {
        panic!("body payload is a record");
    };
    assert_eq!(
        runtime_record_string(body_fields, "kind").expect("body kind is a string"),
        "text"
    );
    assert_eq!(
        runtime_record_string(body_fields, "text").expect("body text is a string"),
        "hello"
    );
    assert_eq!(
        runtime_record_string(body_fields, "value").expect("body value is a string"),
        "hello"
    );
}

#[test]
fn resource_runtime_payload_preserves_bytes_body_value() {
    let bytes_payload = runtime_resource_payload(&serde_json::json!({
        "uri": "agent://resource/image",
        "kind": "image",
        "mime_type": "image/png",
        "hash": "image.hash",
        "body": {
            "body_kind": "bytes_base64",
            "body": {
                "encoding": "base64",
                "data": "aGVsbG8="
            }
        }
    }));
    let RuntimeValue::Record(resource_fields) = bytes_payload else {
        panic!("resource payload is a record");
    };
    let RuntimeValue::Record(body_fields) =
        runtime_record_get(&resource_fields, "body").expect("body field exists")
    else {
        panic!("body payload is a record");
    };
    assert_eq!(
        runtime_record_string(body_fields, "kind").expect("body kind is a string"),
        "bytes_base64"
    );
    assert_eq!(
        runtime_record_string(body_fields, "base64").expect("body data is a string"),
        "aGVsbG8="
    );
    assert_eq!(
        runtime_record_string(body_fields, "encoding").expect("body encoding is a string"),
        "base64"
    );
    let RuntimeValue::Record(value_fields) =
        runtime_record_get(body_fields, "value").expect("body value exists")
    else {
        panic!("bytes body value is a record");
    };
    assert_eq!(
        runtime_record_string(value_fields, "data").expect("body value data is a string"),
        "aGVsbG8="
    );
}

#[test]
fn rag_context_runtime_payload_exposes_summary_fields() {
    let rag_payload = runtime_rag_context_payload(&serde_json::json!({
        "query": {
            "text": "why did opening flow stall?"
        },
        "items": [
            { "id": "item.1" },
            { "id": "item.2" }
        ],
        "truncated": true
    }));
    let RuntimeValue::Record(fields) = rag_payload else {
        panic!("RAG context payload is a record");
    };

    assert_eq!(
        runtime_record_string(&fields, "summary").expect("summary is a string"),
        "2 RAG context item(s) for `why did opening flow stall?`"
    );
    assert_eq!(
        runtime_record_get(&fields, "item_count").expect("item_count exists"),
        &RuntimeValue::usize(2)
    );
    assert!(matches!(
        runtime_record_get(&fields, "truncated").expect("truncated exists"),
        RuntimeValue::Bool(true)
    ));
    assert_eq!(
        runtime_record_string(&fields, "json").expect("json is a string"),
        "{\"items\":[{\"id\":\"item.1\"},{\"id\":\"item.2\"}],\"query\":{\"text\":\"why did opening flow stall?\"},\"truncated\":true}"
    );
}

#[test]
fn controller_bytecode_resumes_bound_wait_response() {
    let session = TestSession {
        observations: vec![
            observation(1, false),
            observation(2, true),
            observation(3, true),
        ],
    };
    let mut runner = AgentRunner::new(
        session,
        NullDebugEventSink,
        NoopRagService,
        RuntimeAgentPolicy::default(),
        AgentRunnerConfig::new(SessionId::new("session.test").expect("valid session id")),
    );
    let program = wait_binding_program();
    let entry = program.entries[0].id.clone();

    let report = runner
        .run_controller_bytecode(program, &entry, AgentControllerRunConfig::default())
        .expect("controller bytecode runs");

    assert_eq!(report.host_calls, 1);
    assert!(matches!(
        &report.responses[0],
        AgentHostResponse::Observation(observation) if observation.tick == 3
    ));
    assert!(matches!(
        report.final_status,
        Some(FlowFiberStatus::Done(FlowExit::Return(ref value))) if value == "3"
    ));
}
