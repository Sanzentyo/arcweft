use std::cell::{Cell, RefCell};

use arcweft_agent_mcp::model::{McpCallToolResult, McpContentBlock};
use arcweft_agent_mcp::repl_command::{
    McpReplCommandEndpoint, McpReplCommandRequest, McpReplTracePolicy,
};
use arcweft_agent_protocol::protocol::{AgentSessionInfo, ObservationEnvelope};
use arcweft_agent_repl::command::{
    ObserveCommand, ReplCommandHost, ReplCommandHostError, ReplCommandHostResult, StepCommand,
};
use arcweft_agent_repl::{
    ReplBaseSnapshot, ReplSession, ReplSessionOptions, ReplTierCommandHandler,
};
use arcweft_lang_sema::project_index::ProgramHash;
use arcweft_runtime_driver::task::{
    RuntimeTaskCancelOutcome, RuntimeTaskCancelTarget, RuntimeTaskListOptions, RuntimeTaskOwner,
    RuntimeTaskRecord, RuntimeTaskStatus,
};
use serde_json::Value;

fn test_session() -> ReplSession {
    ReplSession::new(
        ReplBaseSnapshot::new(
            "mcp.test.base",
            &ProgramHash::new("hash.mcp.test.base"),
            std::sync::Arc::new(arcweft_lang_sema::env::TypeCheckEnv::standard()),
            [],
        ),
        ReplSessionOptions::default(),
    )
}

fn request(input: &str) -> McpReplCommandRequest {
    McpReplCommandRequest {
        input: input.to_owned(),
        command_id: 7,
        trace_policy: McpReplTracePolicy::ReadWrite,
        max_items: 8,
        max_string_bytes: 80,
        include_diagnostics: true,
    }
}

fn result_json(result: &McpCallToolResult) -> Value {
    match result.content.as_slice() {
        [McpContentBlock::Text { text }] => serde_json::from_str(text).expect("tool text is JSON"),
        other => panic!("expected one text block, got {other:?}"),
    }
}

#[test]
fn mcp_repl_command_reports_host_unavailable_as_typed_result() {
    let mut session = test_session();
    let mut handler = ReplTierCommandHandler::default();
    let result =
        McpReplCommandEndpoint::new(&mut session, &mut handler).execute(&request(":tasks"));

    let json = result_json(&result);
    assert_eq!(json["status"], "error");
    assert_eq!(json["diagnostics"][0]["code"], "host_unavailable");
}

#[test]
fn mcp_repl_command_rejects_read_only_mutating_command() {
    let mut session = test_session();
    let mut handler = ReplTierCommandHandler::default();
    let mut host = FakeHost;
    let mut tasks =
        FakeRuntimeTaskOwner::with_tasks(vec![active_task("task.alpha", "scope.view", 0)]);
    let mut req = request(":cancel all");
    req.trace_policy = McpReplTracePolicy::ReadOnlyTrace;

    let result = McpReplCommandEndpoint::new(&mut session, &mut handler)
        .with_host(&mut host)
        .with_runtime_tasks(&mut tasks)
        .execute(&req);

    let json = result_json(&result);
    assert_eq!(json["status"], "rejected");
    assert_eq!(json["diagnostics"][0]["code"], "read_only_trace_rejected");
}

#[test]
fn mcp_repl_command_lists_runtime_tasks_through_existing_owner() {
    let mut session = test_session();
    let mut handler = ReplTierCommandHandler::default();
    let mut host = FakeHost;
    let mut tasks =
        FakeRuntimeTaskOwner::with_tasks(vec![active_task("task.alpha", "scope.view", 0)]);

    let result = McpReplCommandEndpoint::new(&mut session, &mut handler)
        .with_host(&mut host)
        .with_runtime_tasks(&mut tasks)
        .execute(&request(":tasks --all"));

    let json = result_json(&result);
    assert_eq!(json["status"], "ok");
    assert_eq!(json["evidence"]["kind"], "tasks");
    assert_eq!(json["evidence"]["include_completed"], true);
    assert_eq!(
        json["evidence"]["tasks"][0]["correlation"],
        serde_json::to_value(
            active_task("task.alpha", "scope.view", 0)
                .dispatch
                .correlation
        )
        .unwrap()
    );
    assert_eq!(json["evidence"]["tasks"][0]["dispatch_epoch"], 12);
    assert_eq!(json["evidence"]["tasks"][0]["dispatch_sequence"], 0);
    assert_eq!(json["evidence"]["tasks"][0]["status"], "running");
    assert_eq!(
        tasks.list_options.get(),
        Some(RuntimeTaskListOptions {
            include_completed: true
        })
    );
}

#[test]
fn mcp_repl_command_cancels_all_task_and_scope_targets() {
    let cases = [
        (
            ":cancel all".to_owned(),
            RuntimeTaskCancelTarget::All,
            "all",
        ),
        (
            format!(
                ":cancel task {}",
                active_task("task.alpha", "scope.view", 0)
                    .dispatch
                    .correlation
                    .task_id
            ),
            RuntimeTaskCancelTarget::Task(
                active_task("task.alpha", "scope.view", 0)
                    .dispatch
                    .correlation
                    .task_id,
            ),
            "task",
        ),
        (
            ":cancel scope scope.view".to_owned(),
            RuntimeTaskCancelTarget::Scope(arcweft_core::task::CancelScopeId(
                "scope.view".to_owned(),
            )),
            "scope",
        ),
    ];

    for (input, expected_target, expected_kind) in cases {
        let mut session = test_session();
        let mut handler = ReplTierCommandHandler::default();
        let mut host = FakeHost;
        let mut tasks =
            FakeRuntimeTaskOwner::with_tasks(vec![active_task("task.alpha", "scope.view", 0)]);

        let result = McpReplCommandEndpoint::new(&mut session, &mut handler)
            .with_host(&mut host)
            .with_runtime_tasks(&mut tasks)
            .execute(&request(&input));

        let json = result_json(&result);
        assert_eq!(json["status"], "ok");
        assert_eq!(json["evidence"]["kind"], "cancel");
        assert_eq!(json["evidence"]["outcome"]["target"]["kind"], expected_kind);
        assert_eq!(json["evidence"]["outcome"]["cancelled"], 1);
        assert_eq!(*tasks.cancelled_targets.borrow(), vec![expected_target]);
    }
}

#[test]
fn mcp_repl_command_preserves_warm_and_codegen_unsupported_backend_status() {
    let mut session = test_session();
    let mut handler = ReplTierCommandHandler::default();

    let warm =
        McpReplCommandEndpoint::new(&mut session, &mut handler).execute(&request(":warm latest"));
    let warm_json = result_json(&warm);
    assert_eq!(warm_json["status"], "ok");
    assert_eq!(warm_json["evidence"]["kind"], "warm");
    assert_eq!(warm_json["evidence"]["backend_status"], "unsupported");
    assert_eq!(
        warm_json["evidence"]["reason"],
        "full_script_backend_not_available"
    );

    let codegen = McpReplCommandEndpoint::new(&mut session, &mut handler)
        .execute(&request(":codegen latest"));
    let codegen_json = result_json(&codegen);
    assert_eq!(codegen_json["status"], "ok");
    assert_eq!(codegen_json["evidence"]["kind"], "codegen");
    assert_eq!(codegen_json["evidence"]["backend_status"], "unsupported");
    assert_eq!(
        codegen_json["evidence"]["failures"][0]["code"],
        "full_script_backend_not_available"
    );
}

#[derive(Default)]
struct FakeRuntimeTaskOwner {
    tasks: Vec<RuntimeTaskRecord>,
    list_options: Cell<Option<RuntimeTaskListOptions>>,
    cancelled_targets: RefCell<Vec<RuntimeTaskCancelTarget>>,
}

impl FakeRuntimeTaskOwner {
    fn with_tasks(tasks: Vec<RuntimeTaskRecord>) -> Self {
        Self {
            tasks,
            ..Self::default()
        }
    }
}

impl RuntimeTaskOwner for FakeRuntimeTaskOwner {
    fn runtime_tasks(&self, options: RuntimeTaskListOptions) -> Vec<RuntimeTaskRecord> {
        self.list_options.set(Some(options));
        self.tasks.clone()
    }

    fn cancel_runtime_tasks(
        &mut self,
        target: RuntimeTaskCancelTarget,
    ) -> RuntimeTaskCancelOutcome {
        self.cancelled_targets.borrow_mut().push(target);
        RuntimeTaskCancelOutcome {
            cancelled: 1,
            pending_after: 0,
        }
    }
}

struct FakeHost;

impl ReplCommandHost for FakeHost {
    fn session_info(&mut self) -> ReplCommandHostResult<AgentSessionInfo> {
        Err(ReplCommandHostError::unsupported(
            "test host does not expose session_info",
        ))
    }

    fn observe(&mut self, _command: &ObserveCommand) -> ReplCommandHostResult<ObservationEnvelope> {
        Err(ReplCommandHostError::unsupported(
            "test host does not expose observe",
        ))
    }

    fn step(&mut self, _command: &StepCommand) -> ReplCommandHostResult<ObservationEnvelope> {
        Err(ReplCommandHostError::unsupported(
            "test host does not expose step",
        ))
    }
}

fn active_task(argument: &str, scope: &str, sequence: u64) -> RuntimeTaskRecord {
    use arcweft_core::{
        pattern::RuntimeCheckedType,
        task::{
            CancelScopeId, GenerationId, HostTaskRequest, LogicalEpoch, NeedProducerContractDigest,
            NeedProducerFamily, NeedProducerInstance, NeedProducerSiteDigest, NeedProducerSpec,
            RuntimeTypeSemanticDigest, TaskAdmissionJournal, TaskClass, TaskDispatchIdentity,
            TaskOutcomeContract, TaskPlanSemanticDigest, TaskPolicy, TaskPriority,
            TaskPublicationCursor, TaskSequence, TaskSpec,
        },
        value::RuntimeValue,
    };
    let outcome = TaskOutcomeContract::new(RuntimeCheckedType::Unit);
    let values = RuntimeValue::Tuple(vec![RuntimeValue::String(argument.into())]);
    let producer = NeedProducerSpec::new(
        NeedProducerFamily::HostAdapterTask,
        NeedProducerContractDigest::from_bytes([1; 32]),
        TaskPlanSemanticDigest::from_bytes([2; 32]),
        NeedProducerSiteDigest::from_bytes([3; 32]),
        RuntimeTypeSemanticDigest::from_bytes(*outcome.payload_semantic_identity().as_bytes()),
        values.try_digest(1024).unwrap(),
    );
    let spec = TaskSpec {
        generation: GenerationId::new(4),
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
            LogicalEpoch(12),
            TaskSequence(sequence),
        ),
        status: RuntimeTaskStatus::Running,
        cursor: Some(TaskPublicationCursor {
            logical_epoch: LogicalEpoch(12),
            sequence: TaskSequence(2),
        }),
        cancel_scope: CancelScopeId(scope.into()),
    }
}
