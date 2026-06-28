use std::collections::BTreeMap;

use arcweft_agent_protocol::protocol::{AgentProjectGraph, AgentSessionInfo, ObservationEnvelope};
use arcweft_agent_repl::command::{
    BuiltinReplCommandHandler, ReplCommand, ReplCommandContext, ReplCommandDiagnosticCode,
    ReplCommandEvidence, ReplCommandHandler, ReplCommandHost, ReplCommandHostResult,
    ReplCommandStatus, ReplInput, ReplTracePolicy, parse_repl_command, parse_repl_input,
};
use arcweft_agent_repl::{ReplBaseSnapshot, ReplSession, ReplSessionOptions};
use arcweft_lang_sema::project_index::{ProgramHash, ProjectSemanticIndex};

fn test_session() -> ReplSession {
    ReplSession::new(
        ReplBaseSnapshot::from_project(
            "trace.base",
            ProjectSemanticIndex::new(ProgramHash::new("hash.trace.base")),
        ),
        ReplSessionOptions::default(),
    )
}

#[test]
fn repl_trace_rejects_cell_submission_before_execution() {
    let mut session = test_session();
    let input = match parse_repl_input("observe()").expect("input parses") {
        ReplInput::Cell(input) => input,
        other => panic!("expected cell input, got {other:?}"),
    };
    let mut context =
        ReplCommandContext::new(&mut session).with_trace_policy(ReplTracePolicy::ReadOnlyTrace);

    let result = context
        .reject_cell_submission_if_read_only(&input)
        .expect("read-only trace rejects cells");

    assert_eq!(result.status, ReplCommandStatus::Rejected);
    assert_eq!(
        result.diagnostics[0].code,
        ReplCommandDiagnosticCode::ReadOnlyTraceRejected
    );
    assert!(matches!(
        result.evidence,
        ReplCommandEvidence::CellSubmissionRejected(evidence)
            if evidence.source_len == "observe()".len()
                && evidence.policy == ReplTracePolicy::ReadOnlyTrace
    ));
}

#[test]
fn repl_trace_rejects_mutating_session_commands() {
    let mut session = test_session();
    let mut context =
        ReplCommandContext::new(&mut session).with_trace_policy(ReplTracePolicy::ReadOnlyTrace);
    let mut handler = BuiltinReplCommandHandler;

    let result = handler.handle(&mut context, parse_repl_command(":reset").unwrap());

    assert_eq!(result.status, ReplCommandStatus::Rejected);
    assert_eq!(
        result.diagnostics[0].code,
        ReplCommandDiagnosticCode::ReadOnlyTraceRejected
    );
}

#[test]
fn repl_trace_allows_read_only_inspection_commands() {
    let mut session = test_session();
    let mut context =
        ReplCommandContext::new(&mut session).with_trace_policy(ReplTracePolicy::ReadOnlyTrace);
    let mut handler = BuiltinReplCommandHandler;

    let result = handler.handle(
        &mut context,
        parse_repl_command(":generations --tiers").unwrap(),
    );

    assert_eq!(result.status, ReplCommandStatus::Ok);
    assert!(matches!(
        result.evidence,
        ReplCommandEvidence::Generations(evidence)
            if evidence.generation.committed_cells == 0 && evidence.tiers.is_some()
    ));
}

#[test]
fn repl_trace_allows_replay_host_reads_without_overlay_mutation() {
    let mut session = test_session();
    let before = session.generation_evidence();
    let mut host = ReplayLikeHost::default();
    let mut handler = BuiltinReplCommandHandler;
    let result = {
        let mut context = ReplCommandContext::new(&mut session)
            .with_trace_policy(ReplTracePolicy::ReadOnlyTrace)
            .with_host(&mut host);
        handler.handle(
            &mut context,
            ReplCommand::Observe(arcweft_agent_repl::command::ObserveCommand::default()),
        )
    };
    let after = session.generation_evidence();

    assert_eq!(result.status, ReplCommandStatus::Ok);
    assert_eq!(host.observations, 1);
    assert_eq!(before.committed_cells, after.committed_cells);
    assert_eq!(before.overlay_hash, after.overlay_hash);
}

#[derive(Default)]
struct ReplayLikeHost {
    observations: usize,
}

impl ReplCommandHost for ReplayLikeHost {
    fn session_info(&mut self) -> ReplCommandHostResult<AgentSessionInfo> {
        Ok(AgentSessionInfo {
            session_id: "trace.session".to_owned(),
            program_hash: "hash.trace.base".to_owned(),
            project_entities: Vec::new(),
            project_graph: AgentProjectGraph::default(),
            profile: Some("trace.replay".to_owned()),
            capabilities: vec!["observe".to_owned()],
        })
    }

    fn observe(
        &mut self,
        _command: &arcweft_agent_repl::command::ObserveCommand,
    ) -> ReplCommandHostResult<ObservationEnvelope> {
        self.observations += 1;
        Ok(observation(
            u64::try_from(self.observations).unwrap_or(u64::MAX),
        ))
    }

    fn step(
        &mut self,
        command: &arcweft_agent_repl::command::StepCommand,
    ) -> ReplCommandHostResult<ObservationEnvelope> {
        Ok(observation(u64::from(command.frames)))
    }
}

fn observation(tick: u64) -> ObservationEnvelope {
    ObservationEnvelope {
        tick,
        frame_id: format!("trace.frame.{tick}"),
        state_hash: format!("trace.state.{tick}"),
        render_hash: format!("trace.render.{tick}"),
        actions: Vec::new(),
        signals: BTreeMap::new(),
        payload: serde_json::json!({ "trace_tick": tick }),
    }
}
