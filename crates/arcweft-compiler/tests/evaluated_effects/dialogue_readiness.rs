use super::{bind_test_character_dialogue_schema, compile_attached_dialogue_project, log_messages};
use arcweft_compiler::project::CompiledProject;
use arcweft_core::{
    awbc::{
        codec::AwbcDecodeBudget,
        product_step::{AwbcProductExecutorSaveSnapshot, AwbcProductStepExecutor},
        schema::{AwbcEntryId, AwbcProgram},
    },
    engine::{DialogueRuntimePhase, Engine, FlowExit, FlowFiberStatus},
    executor::ArcweftRuntimeExecutor,
    plan::{EntryRuntimeId, FlowEvent},
    pure::VmRuntimePureCallBackend,
    runtime_id::DialogueActivationId,
    step::{
        RuntimeStepBudget, RuntimeStepInput, RuntimeStepMode, RuntimeStepOptions,
        RuntimeStepResult, RuntimeStepStopReason,
    },
    task::RuntimeProgramOwner,
    time::{LogicalDuration, TickId},
};
use arcweft_runtime_plan::awbc_lower::AwbcLowerer;

const INIT_SOURCE: &str = r#"
pub character alice { display = "Alice" }
flow main() -> Unit {
    alice()[Ready[p]] with {
        init {
            log.info("one")
            log.info("two")
            log.info("three")
        }
    }
}
entry cli @entry.native_test { goto @flow.main }
"#;

fn compiled_program(source: &str) -> (CompiledProject, AwbcProgram, Vec<u8>) {
    let compiled = compile_attached_dialogue_project(source).unwrap_or_else(|error| {
        let recovery = error
            .compilation_lease()
            .into_iter()
            .flat_map(|lease| lease.tooling_lease().modules())
            .map(|module| module.hir().diagnostics())
            .collect::<Vec<_>>();
        let diagnostics = error.diagnostics().iter()
            .map(|diagnostic| diagnostic.diagnostic().message())
            .collect::<Vec<_>>();
        panic!("readiness fixture compiles: {error}; project diagnostics: {diagnostics:?}; HIR diagnostics: {recovery:?}");
    });
    let runtime = compiled.runtime_plan();
    let report = AwbcLowerer::new(
        &runtime.plan,
        &runtime.dialogue_content_catalog,
        "dialogue_readiness.arcw",
    )
    .lower()
    .expect("source-owned dialogue lowers to verified AWBC");
    let bytes = report.program.encode_canonical().expect("canonical AWBC");
    let program = AwbcProgram::decode_canonical(&bytes, AwbcDecodeBudget::default())
        .expect("canonical AWBC decodes");
    assert_eq!(program, report.program);
    let entry = EntryRuntimeId::from_source_entity_body("entry.native_test")
        .expect("caller-authored entry identity");
    let [actual] = program.entries.as_slice() else {
        panic!("one explicit entry");
    };
    assert_eq!(actual.runtime_id, entry);
    (compiled, program, bytes)
}

fn with_both_executors(
    source: &str,
    mut check: impl FnMut(
        &str,
        &mut dyn FnMut(RuntimeStepInput, RuntimeStepOptions) -> RuntimeStepResult,
    ),
) {
    let (compiled, program, bytes) = compiled_program(source);
    let mut plan = compiled.runtime_plan().plan.clone();
    plan.bind_artifact(
        arcweft_core::effect::RuntimeArtifactFingerprint::try_from_bytes(
            *blake3::hash(&bytes).as_bytes(),
        )
        .expect("actual artifact fingerprint"),
    )
    .expect("bind the actual encoded artifact");
    let entry = EntryRuntimeId::from_source_entity_body("entry.native_test")
        .expect("caller-authored entry identity");
    let mut native = Engine::for_entry(plan, &entry).expect("exact native entry starts");
    let schema = bind_test_character_dialogue_schema(
        &compiled,
        RuntimeProgramOwner::Plan(native.program_plan()),
    );
    let mut backend = VmRuntimePureCallBackend::default().with_external_calls(
        arcweft_dialogue::CharacterDialogueRuntimeExternalCallBackend::new(&schema),
    );
    check("native", &mut |input, options| {
        native.step_with_pure_backend(input, options, &mut backend)
    });

    let mut awbc = ArcweftRuntimeExecutor::from_awbc_product(program, AwbcEntryId(0))
        .expect("exact decoded Product entry starts");
    let schema = bind_test_character_dialogue_schema(&compiled, awbc.program_owner());
    let mut backend = VmRuntimePureCallBackend::default().with_external_calls(
        arcweft_dialogue::CharacterDialogueRuntimeExternalCallBackend::new(&schema),
    );
    check("decoded-awbc", &mut |input, options| {
        awbc.step_with_pure_backend(input, options, &mut backend)
    });
}

fn options(mode: RuntimeStepMode, max_ops: usize) -> RuntimeStepOptions {
    RuntimeStepOptions {
        mode,
        budget: RuntimeStepBudget { max_ops },
        ..RuntimeStepOptions::default()
    }
}

fn input(tick: u64) -> RuntimeStepInput {
    RuntimeStepInput {
        tick: TickId(tick),
        dt: LogicalDuration::from_nanos(16_000_000),
        ..RuntimeStepInput::default()
    }
}

fn ready(result: &RuntimeStepResult, backend: &str) -> DialogueActivationId {
    let FlowFiberStatus::Dialogue(status) = &result.fiber_status else {
        panic!("{backend}: expected actual dialogue owner: {result:?}");
    };
    assert_eq!(
        status.phase(),
        DialogueRuntimePhase::Ready,
        "{backend}: {result:?}"
    );
    assert_eq!(
        status.waiting_presentation_activation(),
        Some(status.activation())
    );
    status.activation().clone()
}

fn reveal_owners(result: &RuntimeStepResult) -> Vec<DialogueActivationId> {
    result
        .output
        .flow_events
        .iter()
        .filter_map(|event| match event {
            FlowEvent::DialogueLine { activation, .. } => Some(activation.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn authored_capture_steps_reveal_the_same_dialogue_before_native_and_awbc_capture() {
    for (source, steps) in [
        (
            r#"pub character alice { display = "Alice" }
flow main {
    alice: Step pinned capture[p]
}
entry cli @entry.native_test { goto @flow.main }
"#,
            3,
        ),
        (
            r#"pub character alice { display = "Alice" }
flow main {
    alice: #layout(.vertical_rl)[#fx(typewriter(characters_per_second=1))[吾輩]][p]
}
entry cli @entry.native_test { goto @flow.main }
"#,
            2,
        ),
    ] {
        with_both_executors(source, |backend, step| {
            let mut revealed = Vec::new();
            let mut last = None;
            for tick in 0..steps {
                let result = step(input(tick), options(RuntimeStepMode::Drain, 64));
                assert!(
                    result.output.diagnostics.is_empty(),
                    "{backend}: {result:?}"
                );
                assert!(result.stats.executed_ops <= 64);
                revealed.extend(reveal_owners(&result));
                last = Some(result);
            }
            let last = last.expect("authored capture step is nonzero");
            assert_eq!(revealed, [ready(&last, backend)], "{backend}: {last:?}");
            assert_eq!(last.stop_reason, RuntimeStepStopReason::Blocked);
        });
    }
}

#[test]
fn dialogue_drain_reaches_ready_without_spending_host_steps_on_internal_activation() {
    with_both_executors(INIT_SOURCE, |backend, step| {
        let result = step(input(0), options(RuntimeStepMode::Drain, 64));
        assert!(
            result.output.diagnostics.is_empty(),
            "{backend}: {result:?}"
        );
        assert_eq!(reveal_owners(&result), [ready(&result, backend)]);
        assert_eq!(
            log_messages(&result.output.effects.line),
            ["one", "two", "three"]
        );
        assert!(result.stats.executed_ops > 1 && result.stats.executed_ops <= 64);
        assert_eq!(result.stop_reason, RuntimeStepStopReason::Output);

        let waiting = step(input(1), options(RuntimeStepMode::Drain, 64));
        assert_eq!(ready(&waiting, backend), ready(&result, backend));
        assert_eq!(waiting.stop_reason, RuntimeStepStopReason::Blocked);
        assert!(waiting.output.flow_events.is_empty());
        assert!(waiting.output.effects.line.is_empty());
        assert!(waiting.output.requests.host_calls.is_empty());
        assert!(waiting.output.requests.line_commands.is_empty());
        assert!(
            waiting.stats.executed_ops <= 1,
            "a real wait is not a busy drain"
        );
    });
}

#[test]
fn dialogue_duration_wait_blocks_reveal_until_later_logical_time() {
    let source = INIT_SOURCE.replace("log.info(\"two\")", "wait(0.1s)");
    with_both_executors(&source, |backend, step| {
        let first = step(
            RuntimeStepInput {
                dt: LogicalDuration::from_nanos(u64::MAX),
                ..Default::default()
            },
            options(RuntimeStepMode::Drain, 64),
        );
        assert!(first.output.diagnostics.is_empty(), "{backend}: {first:?}");
        assert!(reveal_owners(&first).is_empty(), "{backend}: {first:?}");
        assert_eq!(log_messages(&first.output.effects.line), ["one"]);
        let FlowFiberStatus::Dialogue(status) = &first.fiber_status else {
            panic!("{backend}: {first:?}")
        };
        let activation = status.activation().clone();
        assert_eq!(status.phase(), DialogueRuntimePhase::Activating);
        for dt in [0, 99_999_999] {
            let pending = step(
                RuntimeStepInput {
                    dt: LogicalDuration::from_nanos(dt),
                    ..Default::default()
                },
                options(RuntimeStepMode::Drain, 64),
            );
            assert!(
                pending.output.diagnostics.is_empty(),
                "{backend}: {pending:?}"
            );
            assert!(reveal_owners(&pending).is_empty());
            assert!(log_messages(&pending.output.effects.line).is_empty());
            assert!(
                pending.stats.executed_ops <= 1,
                "a blocked activation must not busy drain: {backend}: {pending:?}"
            );
        }
        let finished = step(
            RuntimeStepInput {
                dt: LogicalDuration::from_nanos(1),
                ..Default::default()
            },
            options(RuntimeStepMode::Drain, 64),
        );
        assert!(
            finished.output.diagnostics.is_empty(),
            "{backend}: {finished:?}"
        );
        assert_eq!(ready(&finished, backend), activation);
        assert_eq!(reveal_owners(&finished), [activation]);
        assert_eq!(log_messages(&finished.output.effects.line), ["three"]);
    });
}

#[test]
fn decoded_awbc_dialogue_duration_wait_public_save_resumes_the_same_activation_once() {
    let source = INIT_SOURCE.replace("log.info(\"two\")", "wait(0.1s)");
    let (compiled, program, _) = compiled_program(&source);
    let mut original = AwbcProductStepExecutor::for_entry(program, AwbcEntryId(0), 65_536).unwrap();
    let schema = bind_test_character_dialogue_schema(
        &compiled,
        RuntimeProgramOwner::Awbc(original.program_arc()),
    );
    let mut backend = VmRuntimePureCallBackend::default().with_external_calls(
        arcweft_dialogue::CharacterDialogueRuntimeExternalCallBackend::new(&schema),
    );
    let first = original.step_with_pure_backend(
        Default::default(),
        options(RuntimeStepMode::Drain, 64),
        &mut backend,
    );
    assert!(first.output.diagnostics.is_empty(), "{first:?}");
    assert!(reveal_owners(&first).is_empty());
    assert_eq!(log_messages(&first.output.effects.line), ["one"]);
    let FlowFiberStatus::Dialogue(status) = &first.fiber_status else {
        panic!("{first:?}")
    };
    let activation = status.activation().clone();
    let partial = original.step_with_pure_backend(
        RuntimeStepInput {
            dt: LogicalDuration::from_nanos(40_000_000),
            ..Default::default()
        },
        options(RuntimeStepMode::Drain, 64),
        &mut backend,
    );
    assert!(partial.output.diagnostics.is_empty(), "{partial:?}");
    assert!(reveal_owners(&partial).is_empty());
    let snapshot = original.snapshot().unwrap();
    let inert =
        AwbcProductExecutorSaveSnapshot::from_live(&snapshot, original.admitted_program()).unwrap();
    let bytes = serde_json::to_vec(&inert).unwrap();
    let decoded: AwbcProductExecutorSaveSnapshot = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(decoded, inert);
    let program = original.program_arc();
    drop(original);
    let fresh = AwbcProductStepExecutor::for_entry_arc(program, AwbcEntryId(0), 65_536).unwrap();
    let bound = decoded
        .into_live_for_program(fresh.admitted_program())
        .unwrap();
    let mut restored = fresh.restore_snapshot_owned(bound).unwrap();
    let schema = bind_test_character_dialogue_schema(
        &compiled,
        RuntimeProgramOwner::Awbc(restored.program_arc()),
    );
    let mut backend = VmRuntimePureCallBackend::default().with_external_calls(
        arcweft_dialogue::CharacterDialogueRuntimeExternalCallBackend::new(&schema),
    );
    let completed = restored.step_with_pure_backend(
        RuntimeStepInput {
            dt: LogicalDuration::from_nanos(60_000_000),
            ..Default::default()
        },
        options(RuntimeStepMode::Drain, 64),
        &mut backend,
    );
    assert!(completed.output.diagnostics.is_empty(), "{completed:?}");
    assert_eq!(ready(&completed, "restored-awbc"), activation);
    assert_eq!(reveal_owners(&completed), [activation]);
    assert_eq!(log_messages(&completed.output.effects.line), ["three"]);
    let waiting = restored.step_with_pure_backend(
        Default::default(),
        options(RuntimeStepMode::Drain, 64),
        &mut backend,
    );
    assert!(waiting.output.diagnostics.is_empty(), "{waiting:?}");
    assert!(reveal_owners(&waiting).is_empty());
    assert!(log_messages(&waiting.output.effects.line).is_empty());
}

#[test]
fn dialogue_one_op_and_drain_budget_yield_keep_the_actual_activation_owner() {
    with_both_executors(INIT_SOURCE, |backend, step| {
        let (owner, tick) = (0..64)
            .find_map(|tick| {
                let result = step(input(tick), options(RuntimeStepMode::OneOp, 64));
                assert!(
                    result.output.diagnostics.is_empty(),
                    "{backend}: {result:?}"
                );
                assert_eq!(result.stats.executed_ops, 1, "{backend}: {result:?}");
                assert_eq!(result.stop_reason, RuntimeStepStopReason::OneOp);
                assert!(reveal_owners(&result).is_empty());
                match result.fiber_status {
                    FlowFiberStatus::Dialogue(status) => {
                        assert_eq!(status.phase(), DialogueRuntimePhase::Activating);
                        assert!(status.waiting_presentation_activation().is_none());
                        Some((status.activation().clone(), tick + 1))
                    }
                    FlowFiberStatus::Running => None,
                    status => panic!("{backend}: unexpected pre-reveal status {status:?}"),
                }
            })
            .expect("the checked entry reaches its activation within a fixed bound");

        let zero = step(input(tick), options(RuntimeStepMode::Drain, 0));
        assert_eq!(zero.stats.executed_ops, 0);
        assert_eq!(zero.stop_reason, RuntimeStepStopReason::BudgetExhausted);
        assert!(reveal_owners(&zero).is_empty());
        let FlowFiberStatus::Dialogue(status) = &zero.fiber_status else {
            panic!("{backend}: zero budget lost the actual activation owner");
        };
        assert_eq!(status.phase(), DialogueRuntimePhase::Activating);
        assert_eq!(status.activation(), &owner);

        let exhausted = step(input(tick + 1), options(RuntimeStepMode::Drain, 1));
        assert_eq!(exhausted.stats.executed_ops, 1);
        assert_eq!(
            exhausted.stop_reason,
            RuntimeStepStopReason::BudgetExhausted,
            "{backend}: {exhausted:?}"
        );
        assert!(reveal_owners(&exhausted).is_empty());
        let FlowFiberStatus::Dialogue(status) = &exhausted.fiber_status else {
            panic!("{backend}: activation owner was lost");
        };
        assert_eq!(status.phase(), DialogueRuntimePhase::Activating);
        assert_eq!(status.activation(), &owner);

        let resumed = step(input(tick + 2), options(RuntimeStepMode::Drain, 64));
        assert!(
            resumed.output.diagnostics.is_empty(),
            "{backend}: {resumed:?}"
        );
        assert_eq!(ready(&resumed, backend), owner);
        assert_eq!(reveal_owners(&resumed), [owner]);
        assert!(resumed.stats.executed_ops <= 64);
    });
}

#[test]
fn dialogue_game_returns_on_reveal_after_non_presentation_init_observations() {
    with_both_executors(INIT_SOURCE, |backend, step| {
        let result = step(input(0), options(RuntimeStepMode::Game, 64));
        assert!(
            result.output.diagnostics.is_empty(),
            "{backend}: {result:?}"
        );
        assert_eq!(result.stop_reason, RuntimeStepStopReason::Output);
        assert_eq!(reveal_owners(&result), [ready(&result, backend)]);
        assert_eq!(
            log_messages(&result.output.effects.line),
            ["one", "two", "three"]
        );
        assert!(result.stats.executed_ops > 1 && result.stats.executed_ops <= 64);
    });
}

#[test]
fn dialogue_drain_finishes_owned_scoped_init_defer_before_reveal() {
    with_both_executors(
        r#"
pub character alice { display = "Alice" }
flow main() -> Unit {
    alice()[Ready[p]] with {
        init {
            defer { log.info("scoped cleanup") }
            log.info("init")
        }
    }
}
entry cli @entry.native_test { goto @flow.main }
"#,
        |backend, step| {
            let result = step(input(0), options(RuntimeStepMode::Drain, 64));
            assert!(
                result.output.diagnostics.is_empty(),
                "{backend}: {result:?}"
            );
            assert_eq!(reveal_owners(&result), [ready(&result, backend)]);
            assert_eq!(
                log_messages(&result.output.effects.line),
                ["init", "scoped cleanup"]
            );
            assert_eq!(result.stats.child_fibers, 0);
            assert!(result.stats.executed_ops <= 64);
        },
    );
}

#[test]
fn failed_dialogue_activation_never_publishes_the_reveal_frame() {
    with_both_executors(
        r#"
fn divisor() -> i64 { 0i64 }
pub character alice { display = "Alice" }
flow main() -> Unit {
    alice()[Must not be published[p]] with {
        init {
            let failure = 1i64 / divisor()
            log.info("unreachable")
        }
    }
}
entry cli @entry.native_test { goto @flow.main }
"#,
        |backend, step| {
            let failed = step(input(0), options(RuntimeStepMode::Drain, 64));
            assert_eq!(
                failed.stop_reason,
                RuntimeStepStopReason::Failed,
                "{backend}: {failed:?}"
            );
            assert!(
                matches!(&failed.fiber_status, FlowFiberStatus::Failed(message) if message.contains("division by zero")),
                "{backend}: {failed:?}"
            );
            assert!(reveal_owners(&failed).is_empty());
            assert!(log_messages(&failed.output.effects.line).is_empty());
            assert_eq!(failed.stats.child_fibers, 0);
            assert!(failed.stats.executed_ops <= 64);
            let repeated = step(input(1), options(RuntimeStepMode::Drain, 64));
            assert_eq!(repeated.stop_reason, RuntimeStepStopReason::Failed);
            assert_eq!(repeated.stats.executed_ops, 0);
            assert!(reveal_owners(&repeated).is_empty());
        },
    );
}

#[test]
fn dialogue_init_recursive_function_value_preserves_capture_and_one_op_budget() {
    with_both_executors(
        r#"
fn count(value: i64) -> i64 {
    if value == 0i64 { 0i64 } else { count(value - 1i64) + 1i64 }
}
pub character alice { display = "Alice" }
flow main() -> Unit {
    alice()[Ready[p]] with {
        init {
            let offset = 40i64
            let compute = |value: i64| count(value) + offset
            let answer = compute(2i64)
            if answer == 42i64 {
                log.info("answer=42")
            } else {
                log.info("wrong answer")
            }
        }
    }
}
entry cli @entry.native_test { goto @flow.main }
"#,
        |backend, step| {
            let mut messages = Vec::new();
            let mut activation_steps = 0;
            let mut completed = false;
            for tick in 0..256 {
                let result = step(input(tick), options(RuntimeStepMode::OneOp, 64));
                assert!(
                    result.output.diagnostics.is_empty(),
                    "{backend}: {result:?}"
                );
                assert_eq!(result.stats.executed_ops, 1, "{backend}: {result:?}");
                messages.extend(log_messages(&result.output.effects.line));
                if let FlowFiberStatus::Dialogue(status) = &result.fiber_status {
                    if status.phase() == DialogueRuntimePhase::Activating {
                        activation_steps += 1;
                        assert!(reveal_owners(&result).is_empty());
                    }
                    if status.phase() == DialogueRuntimePhase::Ready {
                        assert_eq!(reveal_owners(&result), [ready(&result, backend)]);
                        assert_eq!(messages, ["answer=42"]);
                        assert!(
                            activation_steps >= 5,
                            "{backend}: activation stays budgeted"
                        );
                        completed = true;
                        break;
                    }
                }
            }
            assert!(completed, "{backend}: recursive activation reaches reveal");
        },
    );
}

#[test]
fn decoded_awbc_activation_quantum_and_portable_restore_resume_once_at_the_next_step() {
    let (compiled, program, _) = compiled_program(INIT_SOURCE);
    let mut original = AwbcProductStepExecutor::for_entry(program, AwbcEntryId(0), 2)
        .expect("small actual VM quantum");
    let schema = bind_test_character_dialogue_schema(
        &compiled,
        RuntimeProgramOwner::Awbc(original.program_arc()),
    );
    let mut backend = VmRuntimePureCallBackend::default().with_external_calls(
        arcweft_dialogue::CharacterDialogueRuntimeExternalCallBackend::new(&schema),
    );
    let (activation, next_tick) = (0..64)
        .find_map(|tick| {
            let result = original.step_with_pure_backend(
                input(tick),
                options(RuntimeStepMode::Drain, 64),
                &mut backend,
            );
            assert!(result.output.diagnostics.is_empty(), "{result:?}");
            assert!(
                reveal_owners(&result).is_empty(),
                "the first activation quantum precedes reveal"
            );
            match result.fiber_status {
                FlowFiberStatus::Dialogue(status) => {
                    assert_eq!(status.phase(), DialogueRuntimePhase::Activating);
                    assert_eq!(result.stop_reason, RuntimeStepStopReason::BudgetExhausted);
                    assert!(
                        result.stats.executed_ops < 64,
                        "the actual VM quantum is the earlier boundary"
                    );
                    Some((status.activation().clone(), tick + 1))
                }
                FlowFiberStatus::Running => None,
                status => panic!("unexpected small-quantum state: {status:?}"),
            }
        })
        .expect("actual activation quantum yields within a fixed bound");
    let snapshot = original.snapshot().expect("preempted owner snapshots");
    let inert = AwbcProductExecutorSaveSnapshot::from_live(&snapshot, original.admitted_program())
        .expect("portable activation");
    let bytes = serde_json::to_vec(&inert).expect("version1 save DTO serializes");
    let decoded: AwbcProductExecutorSaveSnapshot =
        serde_json::from_slice(&bytes).expect("save DTO decodes");
    assert_eq!(decoded, inert);
    let live = decoded
        .into_live_for_program(original.admitted_program())
        .expect("current program reconstructs the owner");
    let mut restored =
        AwbcProductStepExecutor::for_entry_arc(original.program_arc(), AwbcEntryId(0), 2)
            .expect("same admitted program starts");
    restored = restored
        .restore_snapshot_owned(live)
        .expect("actual activation/cursor/quantum restores");
    let restored_schema = bind_test_character_dialogue_schema(
        &compiled,
        RuntimeProgramOwner::Awbc(restored.program_arc()),
    );
    let mut restored_backend = VmRuntimePureCallBackend::default().with_external_calls(
        arcweft_dialogue::CharacterDialogueRuntimeExternalCallBackend::new(&restored_schema),
    );
    let mut effects = Vec::new();
    let mut reveals = Vec::new();
    for tick in next_tick..next_tick + 64 {
        let actual = original.step_with_pure_backend(
            input(tick),
            options(RuntimeStepMode::Drain, 64),
            &mut backend,
        );
        let rebound = restored.step_with_pure_backend(
            input(tick),
            options(RuntimeStepMode::Drain, 64),
            &mut restored_backend,
        );
        assert_eq!(
            actual, rebound,
            "restore preserves the same step/cursor/budget result"
        );
        assert!(actual.output.diagnostics.is_empty(), "{actual:?}");
        effects.extend(actual.output.effects.line);
        reveals.extend(reveal_owners(&rebound));
        if matches!(&rebound.fiber_status, FlowFiberStatus::Dialogue(status) if status.phase() == DialogueRuntimePhase::Ready)
        {
            assert_eq!(ready(&rebound, "restored decoded-awbc"), activation);
            assert_eq!(reveals, [activation]);
            assert_eq!(log_messages(&effects), ["one", "two", "three"]);
            return;
        }
        assert_eq!(actual.stop_reason, RuntimeStepStopReason::BudgetExhausted);
    }
    panic!("restored activation exceeded its deterministic continuation bound");
}

#[test]
fn dialogue_line_output_and_owned_child_resume_once_after_budgeted_activation() {
    const SOURCE: &str = r#"
enum Outcome { Done }
pub character alice { display = "Alice" }
flow main() -> i64 {
    let outcome = alice()[Ready[p]] with {
        thread motion { log.info("motion") }
        out Outcome.Done
    }
    return match outcome { .Done => 42i64 }
}
entry cli @entry.native_test { goto @flow.main }
"#;
    with_both_executors(SOURCE, |backend, step| {
        let mut next_input = input(0);
        let mut revealed = Vec::new();
        let mut messages = Vec::new();
        let mut advances = 0;
        let mut observed_child = false;
        for tick in 0..256 {
            let result = step(next_input, options(RuntimeStepMode::OneOp, 1));
            assert!(result.stats.executed_ops <= 1, "{backend}: {result:?}");
            assert!(
                result.output.diagnostics.is_empty(),
                "{backend}: {result:?}"
            );
            assert!(
                result.output.requests.tasks.is_empty(),
                "{backend}: {result:?}"
            );
            assert!(
                result.output.requests.host_calls.is_empty(),
                "{backend}: {result:?}"
            );
            assert!(
                result.output.requests.program_task_completions.is_empty(),
                "{backend}: {result:?}"
            );
            assert!(
                result.output.requests.line_commands.is_empty(),
                "{backend}: {result:?}"
            );
            if tick < 3 {
                assert_eq!(result.stats.executed_ops, 1, "{backend}: {result:?}");
                assert!(reveal_owners(&result).is_empty(), "{backend}: {result:?}");
                assert!(
                    result.output.effects.line.is_empty(),
                    "{backend}: {result:?}"
                );
            }
            revealed.extend(reveal_owners(&result));
            messages.extend(log_messages(&result.output.effects.line));
            observed_child |= result.stats.child_fibers > 0;
            next_input = input(tick + 1);
            if let FlowFiberStatus::Dialogue(status) = &result.fiber_status
                && status.phase() == DialogueRuntimePhase::Ready
                && advances == 0
            {
                let activation = ready(&result, backend);
                assert_eq!(revealed, [activation.clone()], "{backend}: {result:?}");
                next_input.dialogue_advances.push(activation);
                advances += 1;
            }
            if let FlowFiberStatus::Done(exit) = &result.fiber_status {
                assert_eq!(
                    exit,
                    &FlowExit::Return("42".to_owned()),
                    "{backend}: {result:?}"
                );
                assert_eq!(revealed.len(), 1, "{backend}: {result:?}");
                assert_eq!(advances, 1, "{backend}: {result:?}");
                assert_eq!(messages, ["motion"], "{backend}: {result:?}");
                assert!(observed_child, "{backend}: the original child must execute");
                assert_eq!(result.stats.child_fibers, 0, "{backend}: {result:?}");
                return;
            }
            assert!(
                !matches!(result.fiber_status, FlowFiberStatus::Failed(_)),
                "{backend}: {result:?}"
            );
        }
        panic!("{backend}: the owned child and typed line output did not finish");
    });
}
