use std::collections::BTreeMap;

use arcweft_agent_protocol::protocol::{AgentProjectGraph, AgentSessionInfo, ObservationEnvelope};
use arcweft_agent_repl::command::{
    BuiltinReplCommandHandler, ObserveCommand, ReplCancelTarget, ReplCommandContext,
    ReplCommandEvidence, ReplCommandHandler, ReplCommandHost, ReplCommandHostResult,
    ReplCommandStatus, RuntimeTaskReplCommandHost, StepCommand, parse_repl_command,
};
use arcweft_agent_repl::{ReplBaseSnapshot, ReplSession, ReplSessionOptions};
use arcweft_lang_sema::project_index::ProgramHash;
use arcweft_runtime_driver::task::{
    RuntimeTaskCancelOutcome, RuntimeTaskCancelTarget, RuntimeTaskListOptions, RuntimeTaskOwner,
    RuntimeTaskRecord, RuntimeTaskStatus,
};

fn test_session() -> ReplSession {
    ReplSession::new(
        ReplBaseSnapshot::new(
            "runtime-task.base",
            &ProgramHash::new("hash.runtime-task.base"),
            std::sync::Arc::new(arcweft_lang_sema::env::TypeCheckEnv::standard()),
            [],
        ),
        ReplSessionOptions::default(),
    )
}

#[test]
fn repl_runtime_task_adapter_lists_runtime_owned_projection() {
    let mut session = test_session();
    let mut delegate = ReadOnlyHost;
    let mut tasks = FakeRuntimeTaskOwner {
        records: vec![
            task_record(
                "pending",
                2,
                11,
                0,
                "scope.pending",
                RuntimeTaskStatus::Pending,
            ),
            task_record("done", 1, 7, 1, "scope.done", RuntimeTaskStatus::Completed),
        ],
        ..FakeRuntimeTaskOwner::default()
    };
    let mut handler = BuiltinReplCommandHandler;

    let result = {
        let mut host = RuntimeTaskReplCommandHost::new(&mut delegate, &mut tasks);
        let mut context = ReplCommandContext::new(&mut session).with_host(&mut host);
        handler.handle(&mut context, parse_repl_command(":tasks --all").unwrap())
    };

    assert_eq!(result.status, ReplCommandStatus::Ok);
    assert!(matches!(
        result.evidence,
        ReplCommandEvidence::Tasks(evidence)
            if evidence.include_completed
                && evidence.tasks.tasks.len() == 2
                && evidence.tasks.tasks[0].dispatch == task_record("pending", 2, 11, 0, "scope.pending", RuntimeTaskStatus::Pending).dispatch
                && evidence.tasks.tasks[1].dispatch == task_record("done", 1, 7, 1, "scope.done", RuntimeTaskStatus::Completed).dispatch
    ));
}

#[test]
fn repl_runtime_task_adapter_cancels_through_runtime_owner() {
    let mut session = test_session();
    let mut delegate = ReadOnlyHost;
    let mut tasks = FakeRuntimeTaskOwner::default();
    let mut handler = BuiltinReplCommandHandler;

    let result = {
        let mut host = RuntimeTaskReplCommandHost::new(&mut delegate, &mut tasks);
        let mut context = ReplCommandContext::new(&mut session).with_host(&mut host);
        handler.handle(
            &mut context,
            parse_repl_command(":cancel scope scope.pending").unwrap(),
        )
    };

    assert_eq!(
        tasks.cancellations,
        vec![RuntimeTaskCancelTarget::Scope(
            arcweft_core::task::CancelScopeId("scope.pending".to_owned())
        )]
    );
    assert_eq!(result.status, ReplCommandStatus::Ok);
    assert!(matches!(
        result.evidence,
        ReplCommandEvidence::Cancel(evidence)
            if evidence.outcome.target == ReplCancelTarget::Scope("scope.pending".to_owned())
                && evidence.outcome.cancelled == 1
                && evidence.outcome.pending_after == 0
    ));
}

#[derive(Default)]
struct FakeRuntimeTaskOwner {
    records: Vec<RuntimeTaskRecord>,
    cancellations: Vec<RuntimeTaskCancelTarget>,
}

impl RuntimeTaskOwner for FakeRuntimeTaskOwner {
    fn runtime_tasks(&self, options: RuntimeTaskListOptions) -> Vec<RuntimeTaskRecord> {
        self.records
            .iter()
            .filter(|record| options.include_completed || record.status.is_active())
            .cloned()
            .collect()
    }

    fn cancel_runtime_tasks(
        &mut self,
        target: RuntimeTaskCancelTarget,
    ) -> RuntimeTaskCancelOutcome {
        self.cancellations.push(target);
        RuntimeTaskCancelOutcome {
            cancelled: 1,
            pending_after: 0,
        }
    }
}

#[derive(Default)]
struct ReadOnlyHost;

impl ReplCommandHost for ReadOnlyHost {
    fn session_info(&mut self) -> ReplCommandHostResult<AgentSessionInfo> {
        Ok(AgentSessionInfo {
            session_id: "runtime-task.session".to_owned(),
            program_hash: "hash.runtime-task.base".to_owned(),
            project_entities: Vec::new(),
            project_graph: AgentProjectGraph::default(),
            profile: Some("runtime-task".to_owned()),
            capabilities: vec!["observe".to_owned(), "step_frames".to_owned()],
        })
    }

    fn observe(&mut self, _command: &ObserveCommand) -> ReplCommandHostResult<ObservationEnvelope> {
        Ok(observation(1))
    }

    fn step(&mut self, command: &StepCommand) -> ReplCommandHostResult<ObservationEnvelope> {
        Ok(observation(u64::from(command.frames)))
    }
}

fn observation(tick: u64) -> ObservationEnvelope {
    ObservationEnvelope {
        tick,
        frame_id: format!("runtime-task.frame.{tick}"),
        state_hash: format!("runtime-task.state.{tick}"),
        render_hash: format!("runtime-task.render.{tick}"),
        actions: Vec::new(),
        signals: BTreeMap::new(),
        payload: serde_json::json!({ "tick": tick }),
    }
}

fn task_record(
    argument: &str,
    generation: u64,
    epoch: u64,
    sequence: u64,
    scope: &str,
    status: RuntimeTaskStatus,
) -> RuntimeTaskRecord {
    use arcweft_core::{pattern::RuntimeCheckedType, task::*, value::RuntimeValue};
    let outcome = TaskOutcomeContract::new(RuntimeCheckedType::Unit);
    let arguments = RuntimeValue::Tuple(vec![RuntimeValue::String(argument.into())]);
    let producer = NeedProducerSpec::new(
        NeedProducerFamily::HostAdapterTask,
        NeedProducerContractDigest::from_bytes([1; 32]),
        TaskPlanSemanticDigest::from_bytes([2; 32]),
        NeedProducerSiteDigest::from_bytes([3; 32]),
        RuntimeTypeSemanticDigest::from_bytes(*outcome.payload_semantic_identity().as_bytes()),
        arguments.try_digest(1024).unwrap(),
    );
    let spec = TaskSpec {
        generation: GenerationId::new(generation),
        producer: NeedProducerInstance::try_from(&producer).unwrap(),
        class: TaskClass::Background,
        priority: TaskPriority(0),
        cancel_scope: CancelScopeId(scope.into()),
        policy: TaskPolicy::JoinSameKey,
        outcome,
        request: HostTaskRequest::custom(
            "fixture",
            "record",
            [RuntimeValue::String(argument.into()).into()],
        ),
        debug_label: argument.into(),
    };
    let mut journal = TaskAdmissionJournal::default();
    let handle = journal.ensure_task(spec).unwrap();
    RuntimeTaskRecord {
        dispatch: TaskDispatchIdentity::new(
            handle.correlation,
            LogicalEpoch(epoch),
            TaskSequence(sequence),
        ),
        status,
        cursor: status.is_terminal().then_some(TaskPublicationCursor {
            logical_epoch: LogicalEpoch(epoch),
            sequence: TaskSequence(3),
        }),
        cancel_scope: CancelScopeId(scope.into()),
    }
}
