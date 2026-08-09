use arcweft_agent_protocol::ids::SessionId;
use arcweft_agent_protocol::protocol::{
    ActionResult, AgentAction, AgentProjectGraph, AgentSessionInfo, CaptureRequest, CaptureResult,
    ObservationEnvelope, ObserveRequest,
};
use arcweft_agent_protocol::resource::AgentResource;
use arcweft_agent_repl::command::{
    ReplCommandEvidence, ReplCommandHandler, ReplCommandStatus, parse_repl_command,
};
use arcweft_agent_repl::{
    ReplBaseSnapshot, ReplCellFilter, ReplCellInput, ReplSession, ReplSessionOptions,
    ReplTierBackendStatus, ReplTierCommandHandler, ReplTierFallback, ReplWarmUnsupportedReason,
};
use arcweft_agent_runner::config::{AgentControllerRunConfig, AgentRunnerConfig};
use arcweft_agent_runner::session::{AgentSession, NoopRagService};
use arcweft_debug_model::sink::NullDebugEventSink;
use arcweft_lang_sema::project_index::ProgramHash;

#[test]
fn repl_tiering_warm_without_backend_is_deterministic_vm_fallback() {
    let mut repl = test_repl("test.program.tiering.warm");
    let mut context = arcweft_agent_repl::command::ReplCommandContext::new(&mut repl);
    let mut handler = ReplTierCommandHandler::default();

    let result = handler.handle(&mut context, parse_repl_command(":warm latest").unwrap());

    assert_eq!(result.status, ReplCommandStatus::Ok);
    match result.evidence {
        ReplCommandEvidence::Warm(outcome) => {
            assert!(outcome.requested);
            assert!(!outcome.started_background_job);
            assert_eq!(outcome.backend_status, ReplTierBackendStatus::Unsupported);
            assert_eq!(outcome.fallback, ReplTierFallback::BytecodeVm);
            assert_eq!(
                outcome.reason,
                Some(ReplWarmUnsupportedReason::FullScriptBackendNotAvailable)
            );
            assert!(outcome.warmed_cells.is_empty());
        }
        other => panic!("expected warm outcome, got {other:?}"),
    }
    assert_eq!(context.session().tier_status().records.len(), 1);
}

#[test]
fn repl_tiering_codegen_reports_status_only_surface() {
    let mut repl = test_repl("test.program.tiering.codegen");
    let mut context = arcweft_agent_repl::command::ReplCommandContext::new(&mut repl);
    let mut handler = ReplTierCommandHandler::default();

    let result = handler.handle(&mut context, parse_repl_command(":codegen").unwrap());

    assert_eq!(result.status, ReplCommandStatus::Ok);
    match result.evidence {
        ReplCommandEvidence::Codegen(status) => {
            assert!(status.requested);
            assert_eq!(status.backend_status, ReplTierBackendStatus::Unsupported);
            assert_eq!(status.fallback, ReplTierFallback::BytecodeVm);
            assert!(status.enabled_backends.is_empty());
            assert!(status.pending_jobs.is_empty());
            assert!(
                status
                    .failures
                    .iter()
                    .any(|failure| failure.code.as_str() == "full_script_backend_not_available")
            );
        }
        other => panic!("expected codegen status, got {other:?}"),
    }
    assert_eq!(context.session().tier_status().records.len(), 1);
}

#[test]
fn repl_tiering_immediate_vm_execution_remains_available_after_status_only_requests() {
    let mut repl = test_repl("test.program.tiering.execution");
    let mut host = StaticAgentSession::new("test.program.tiering.execution");
    let mut debug = NullDebugEventSink;
    let mut rag = NoopRagService;
    let first = repl
        .evaluate_cell(
            &ReplCellInput::statement("let before_warm = \"vm\""),
            test_runtime(&mut host, &mut debug, &mut rag),
        )
        .expect("first cell executes through VM");
    assert!(first.committed);

    {
        let mut context = arcweft_agent_repl::command::ReplCommandContext::new(&mut repl);
        let mut handler = ReplTierCommandHandler::default();
        assert_eq!(
            handler
                .handle(&mut context, parse_repl_command(":warm latest").unwrap())
                .status,
            ReplCommandStatus::Ok
        );
        assert_eq!(
            handler
                .handle(&mut context, parse_repl_command(":codegen").unwrap())
                .status,
            ReplCommandStatus::Ok
        );
    }

    let second = repl
        .evaluate_cell(
            &ReplCellInput::statement("let after_warm = \"vm\""),
            test_runtime(&mut host, &mut debug, &mut rag),
        )
        .expect("second cell still executes through VM");
    assert!(second.committed);
    assert_eq!(repl.cells(ReplCellFilter::default()).cells.len(), 2);
}

fn test_repl(program_hash: &str) -> ReplSession {
    ReplSession::new(
        ReplBaseSnapshot::new(
            "tiering.test",
            &ProgramHash::new(program_hash),
            std::sync::Arc::new(arcweft_lang_sema::env::TypeCheckEnv::standard()),
            [],
        ),
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
        AgentRunnerConfig::new(SessionId::new("session.repl.tiering.test").unwrap()),
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
            session_id: "session.repl.tiering.test".to_owned(),
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
