use std::collections::BTreeMap;
use std::convert::Infallible;

use arcweft_agent_protocol::protocol::{
    ActionResult, AgentAction, AgentProjectGraph, AgentSessionInfo, CaptureRequest, CaptureResult,
    ObservationEnvelope, ObserveRequest,
};
use arcweft_agent_protocol::resource::AgentResource;
use arcweft_agent_repl::command::{
    BuiltinReplCommandHandler, CellsCommand, ReplBackgroundRequest, ReplBackgroundRequestId,
    ReplBackgroundRequestSink, ReplCommand, ReplCommandContext, ReplCommandDiagnosticCode,
    ReplCommandEvidence, ReplCommandHandler, ReplCommandStatus, ReplCommandTarget, ReplInput,
    WarmCommand, parse_repl_command, parse_repl_input,
};
use arcweft_agent_repl::{ReplBaseSnapshot, ReplSession, ReplSessionOptions};
use arcweft_agent_runner::policy::RuntimeAgentCapability;
use arcweft_agent_runner::session::AgentSession;
use arcweft_lang_sema::project_index::ProgramHash;

fn test_session() -> ReplSession {
    ReplSession::new(
        ReplBaseSnapshot::new(
            "test.base",
            &ProgramHash::new("hash.test.base"),
            std::sync::Arc::new(arcweft_lang_sema::env::TypeCheckEnv::standard()),
            [],
        ),
        ReplSessionOptions::default(),
    )
}

#[test]
fn repl_command_parse_distinguishes_commands_from_cells() {
    let command = parse_repl_input("  :cells --all").expect("command parses");
    let cell = parse_repl_input("signal(@signal.ready)").expect("cell parses");

    assert!(matches!(
        command,
        ReplInput::Command(ReplCommand::Cells(CellsCommand {
            include_invalidated: true,
        }))
    ));
    match cell {
        ReplInput::Cell(input) => assert_eq!(input.source_text(), "signal(@signal.ready)"),
        other => panic!("expected cell input, got {other:?}"),
    }
}

#[test]
fn repl_command_parse_reports_stable_unknown_command_error() {
    let error = parse_repl_command(":legacy").expect_err("unknown command is rejected");

    assert_eq!(error.code, ReplCommandDiagnosticCode::UnknownCommand);
    assert_eq!(error.command.as_deref(), Some(":legacy"));
}

#[test]
fn repl_command_parse_reserves_typed_warm_and_codegen_targets() {
    let warm = parse_repl_command(":warm cell.7").expect("warm parses");
    let codegen = parse_repl_command(":codegen latest").expect("codegen parses");

    assert!(matches!(
        warm,
        ReplCommand::Warm(WarmCommand {
            target: ReplCommandTarget::Cell(_),
        })
    ));
    assert!(matches!(
        codegen,
        ReplCommand::Codegen(command) if command.target == ReplCommandTarget::Latest
    ));
}

#[test]
fn repl_command_cells_and_capabilities_are_deterministic_read_only_results() {
    let mut session = test_session();
    let mut context = ReplCommandContext::new(&mut session);
    let mut handler = BuiltinReplCommandHandler;

    let cells = handler.handle(
        &mut context,
        ReplCommand::Cells(CellsCommand {
            include_invalidated: true,
        }),
    );
    let capabilities = handler.handle(&mut context, parse_repl_command(":capabilities").unwrap());

    assert_eq!(cells.command_id.as_u64(), 1);
    assert_eq!(cells.status, ReplCommandStatus::Ok);
    assert!(matches!(
        cells.evidence,
        ReplCommandEvidence::Cells(list) if list.cells.is_empty()
    ));
    assert_eq!(capabilities.command_id.as_u64(), 2);
    assert_eq!(capabilities.status, ReplCommandStatus::Ok);
    assert!(matches!(
        capabilities.evidence,
        ReplCommandEvidence::Capabilities(report)
            if report.allows(RuntimeAgentCapability::Observe)
    ));
}

#[test]
fn repl_command_undo_empty_session_reports_typed_session_error() {
    let mut session = test_session();
    let mut context = ReplCommandContext::new(&mut session);
    let mut handler = BuiltinReplCommandHandler;

    let result = handler.handle(&mut context, parse_repl_command(":undo").unwrap());

    assert_eq!(result.status, ReplCommandStatus::Error);
    assert_eq!(
        result.diagnostics[0].code,
        ReplCommandDiagnosticCode::SessionError
    );
}

#[test]
fn repl_command_reset_empty_session_reports_return_to_base_evidence() {
    let mut session = test_session();
    let mut context = ReplCommandContext::new(&mut session);
    let mut handler = BuiltinReplCommandHandler;

    let result = handler.handle(&mut context, parse_repl_command(":reset").unwrap());

    assert_eq!(result.status, ReplCommandStatus::Ok);
    match result.evidence {
        ReplCommandEvidence::Reset(evidence) => {
            assert_eq!(evidence.summary.removed_cells, 0);
            assert!(evidence.binding_evidence_after.bindings.is_empty());
            assert_eq!(evidence.generation_evidence_after.committed_cells, 0);
            assert_eq!(evidence.tier_invalidations.len(), 1);
        }
        other => panic!("expected reset evidence, got {other:?}"),
    }
}

#[test]
fn repl_command_background_sink_receives_typed_warm_request() {
    #[derive(Default)]
    struct Sink {
        requests: Vec<ReplBackgroundRequest>,
    }

    impl ReplBackgroundRequestSink for Sink {
        fn enqueue(&mut self, request: ReplBackgroundRequest) -> ReplBackgroundRequestId {
            self.requests.push(request);
            ReplBackgroundRequestId::new(u64::try_from(self.requests.len()).unwrap_or(u64::MAX))
        }
    }

    let mut session = test_session();
    let mut sink = Sink::default();
    let mut handler = BuiltinReplCommandHandler;
    let result = {
        let mut context = ReplCommandContext::new(&mut session).with_background(&mut sink);
        handler.handle(&mut context, parse_repl_command(":warm latest").unwrap())
    };

    assert_eq!(result.status, ReplCommandStatus::Queued);
    assert_eq!(sink.requests.len(), 1);
    assert!(matches!(
        result.evidence,
        ReplCommandEvidence::BackgroundQueued(evidence)
            if evidence.request_id.as_u64() == 1
    ));
}

#[test]
fn repl_command_agent_session_adapter_reuses_existing_observe_boundary() {
    let mut session = test_session();
    let mut host_session = FakeAgentSession::default();
    let mut handler = BuiltinReplCommandHandler;
    let result = {
        let mut host =
            arcweft_agent_repl::command::AgentSessionReplCommandHost::new(&mut host_session);
        let mut context = ReplCommandContext::new(&mut session).with_host(&mut host);
        handler.handle(
            &mut context,
            parse_repl_command(":observe images --no-objects").unwrap(),
        )
    };

    assert_eq!(result.status, ReplCommandStatus::Ok);
    assert_eq!(host_session.observes, 1);
    assert!(matches!(
        result.evidence,
        ReplCommandEvidence::Observation(evidence)
            if evidence.request.include_images && !evidence.request.include_objects
    ));
}

#[derive(Default)]
struct FakeAgentSession {
    observes: usize,
}

impl AgentSession for FakeAgentSession {
    type Error = Infallible;

    fn info(&mut self) -> Result<AgentSessionInfo, Self::Error> {
        Ok(AgentSessionInfo {
            session_id: "session.test".to_owned(),
            program_hash: "hash.test.base".to_owned(),
            project_entities: Vec::new(),
            project_graph: AgentProjectGraph::default(),
            profile: Some("test".to_owned()),
            capabilities: vec!["observe".to_owned(), "step_frames".to_owned()],
        })
    }

    fn observe(&mut self, _request: ObserveRequest) -> Result<ObservationEnvelope, Self::Error> {
        self.observes += 1;
        Ok(test_observation(
            u64::try_from(self.observes).unwrap_or(u64::MAX),
        ))
    }

    fn act(&mut self, _action: AgentAction) -> Result<ActionResult, Self::Error> {
        unreachable!("act is not used by command adapter tests")
    }

    fn capture(&mut self, _request: CaptureRequest) -> Result<CaptureResult, Self::Error> {
        unreachable!("capture is not used by command adapter tests")
    }

    fn read_resource(&mut self, _uri: &str) -> Result<AgentResource, Self::Error> {
        unreachable!("resource reads are not used by command adapter tests")
    }

    fn step_frames(&mut self, count: u32) -> Result<ObservationEnvelope, Self::Error> {
        Ok(test_observation(u64::from(count)))
    }
}

fn test_observation(tick: u64) -> ObservationEnvelope {
    ObservationEnvelope {
        tick,
        frame_id: format!("frame.{tick}"),
        state_hash: format!("state.{tick}"),
        render_hash: format!("render.{tick}"),
        actions: Vec::new(),
        signals: BTreeMap::new(),
        payload: serde_json::json!({ "tick": tick }),
    }
}
