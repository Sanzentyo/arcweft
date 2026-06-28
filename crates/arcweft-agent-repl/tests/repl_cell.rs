use arcweft_agent_protocol::ids::SessionId;
use arcweft_agent_protocol::protocol::{
    ActionResult, AgentAction, AgentProjectGraph, AgentSessionInfo, CaptureRequest, CaptureResult,
    ObservationEnvelope, ObserveRequest,
};
use arcweft_agent_protocol::resource::AgentResource;
use arcweft_agent_repl::{
    ReplBaseSnapshot, ReplBindingSnapshotKind, ReplCellFilter, ReplCellInput, ReplCellKind,
    ReplSession, ReplSessionOptions,
};
use arcweft_agent_runner::config::{AgentControllerRunConfig, AgentRunnerConfig};
use arcweft_agent_runner::session::{AgentSession, NoopRagService};
use arcweft_debug_model::sink::NullDebugEventSink;
use arcweft_lang_sema::project_index::{ProgramHash, ProjectSemanticIndex};

#[test]
fn repl_cell_command_input_is_not_committed() {
    let mut repl = test_repl("test.program.command");
    let before = repl.cells(ReplCellFilter::default()).cells.len();
    let mut host = StaticAgentSession::new("test.program.command");
    let mut debug = NullDebugEventSink;
    let mut rag = NoopRagService;
    let error = repl
        .evaluate_cell(
            &ReplCellInput::source(":history"),
            test_runtime(&mut host, &mut debug, &mut rag),
        )
        .expect_err("command-only input is delegated to seq05.2");
    assert_eq!(
        error.phase(),
        arcweft_agent_repl::ReplTransactionPhase::ClassifyParse
    );
    assert_eq!(repl.cells(ReplCellFilter::default()).cells.len(), before);
}

#[test]
fn repl_cell_records_literal_binding_evidence() {
    let mut repl = test_repl("test.program.binding");
    let mut host = StaticAgentSession::new("test.program.binding");
    let mut debug = NullDebugEventSink;
    let mut rag = NoopRagService;
    let outcome = repl
        .evaluate_cell(
            &ReplCellInput::statement("let greeting = \"hello\""),
            test_runtime(&mut host, &mut debug, &mut rag),
        )
        .expect("literal statement cell should compile, commit, and execute");
    assert_eq!(outcome.record.kind, ReplCellKind::Statement);
    assert!(outcome.record.bindings.iter().any(|binding| {
        binding.name == "greeting" && binding.snapshot_kind == ReplBindingSnapshotKind::Literal
    }));
}

fn test_repl(program_hash: &str) -> ReplSession {
    let project = ProjectSemanticIndex::new(ProgramHash::new(program_hash));
    ReplSession::new(
        ReplBaseSnapshot::from_project("test", project),
        ReplSessionOptions::default(),
    )
}

fn test_runtime<'a>(
    host: &'a mut StaticAgentSession,
    debug: &'a mut NullDebugEventSink,
    rag: &'a mut NoopRagService,
) -> arcweft_agent_repl::ReplEvaluationRuntime<
    'a,
    StaticAgentSession,
    NullDebugEventSink,
    NoopRagService,
> {
    arcweft_agent_repl::ReplEvaluationRuntime::new(
        host,
        debug,
        rag,
        AgentRunnerConfig::new(SessionId::new("session.repl.test").unwrap()),
    )
    .with_run_config(AgentControllerRunConfig {
        max_steps: 16,
        max_ops_per_step: 128,
    })
}

#[derive(Clone, Debug)]
struct StaticAgentSession {
    program_hash: String,
}

impl StaticAgentSession {
    fn new(program_hash: &str) -> Self {
        Self {
            program_hash: program_hash.to_owned(),
        }
    }
}

impl AgentSession for StaticAgentSession {
    type Error = std::convert::Infallible;

    fn info(&mut self) -> Result<AgentSessionInfo, Self::Error> {
        Ok(AgentSessionInfo {
            session_id: "session.repl.test".to_owned(),
            program_hash: self.program_hash.clone(),
            project_entities: Vec::new(),
            project_graph: AgentProjectGraph::default(),
            profile: None,
            capabilities: Vec::new(),
        })
    }

    fn observe(&mut self, _request: ObserveRequest) -> Result<ObservationEnvelope, Self::Error> {
        unreachable!("test cells do not request observations")
    }

    fn act(&mut self, _action: AgentAction) -> Result<ActionResult, Self::Error> {
        unreachable!("test cells do not request actions")
    }

    fn capture(&mut self, _request: CaptureRequest) -> Result<CaptureResult, Self::Error> {
        unreachable!("test cells do not request captures")
    }

    fn read_resource(&mut self, _uri: &str) -> Result<AgentResource, Self::Error> {
        unreachable!("test cells do not request resource reads")
    }

    fn step_frames(&mut self, _count: u32) -> Result<ObservationEnvelope, Self::Error> {
        unreachable!("test cells do not wait")
    }
}
