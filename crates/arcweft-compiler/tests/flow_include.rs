//! Include invokes the original Flow function under both execution backends.

use std::sync::Arc;

use arcweft_compiler::source::compile_source;
use arcweft_core::{
    awbc::{
        codec::AwbcDecodeBudget,
        product_step::{AwbcProductExecutorSaveSnapshot, AwbcProductProgram},
        schema::AwbcProgram,
        verify::{AwbcVerifyBudget, AwbcVerifyContext},
    },
    engine::{FlowExit, FlowFiberStatus},
    executor::{
        ArcweftExecutionTier, ArcweftRuntimeExecutor, ArcweftRuntimeExecutorSnapshot,
        RuntimeExecutor,
    },
    plan::{RuntimeEntryTarget, RuntimeTaskSourceArtifactLimits},
    step::{RuntimeStepInput, RuntimeStepOptions},
};
use arcweft_runtime_plan::awbc_lower::AwbcLowerer;

#[derive(Clone, Copy, Debug)]
enum Backend {
    Native,
    Awbc,
    SelectedAwbc,
}

const BACKENDS: [Backend; 3] = [Backend::Native, Backend::Awbc, Backend::SelectedAwbc];

fn executor(source: &str, backend: Backend) -> ArcweftRuntimeExecutor {
    let compiled =
        compile_source(source).expect("Include source compiles through its checked target");
    let [entry] = compiled.plan.entries() else {
        panic!("fixture has one actual Entry")
    };
    let RuntimeEntryTarget::Flow(flow) = &entry.target else {
        panic!("fixture Entry owns a Flow")
    };
    let flow = flow.clone();
    if !matches!(backend, Backend::Native) {
        let lowerer = match backend {
            Backend::SelectedAwbc => AwbcLowerer::for_entry(
                &compiled.plan,
                &compiled.dialogue_content,
                "flow-include.arcw",
                &entry.id,
            ),
            Backend::Awbc => AwbcLowerer::new(
                &compiled.plan,
                &compiled.dialogue_content,
                "flow-include.arcw",
            ),
            Backend::Native => unreachable!(),
        };
        let lowered = lowerer.lower().expect("original Flow calls lower to AWBC");
        let decoded = AwbcProgram::decode_canonical(
            &lowered.program.encode_canonical().unwrap(),
            AwbcDecodeBudget::default(),
        )
        .unwrap();
        let program = AwbcProductProgram::admit(
            Arc::new(decoded),
            AwbcVerifyBudget::default(),
            AwbcVerifyContext::default(),
            RuntimeTaskSourceArtifactLimits::default(),
            None,
        )
        .unwrap();
        ArcweftRuntimeExecutor::from_admitted_awbc_product_flow_invocation(
            program,
            compiled.plan.seal_flow_invocation(flow, []).unwrap(),
        )
        .unwrap()
    } else {
        ArcweftRuntimeExecutor::from_runtime_flow_invocation(
            compiled.plan.seal_flow_invocation(flow, []).unwrap(),
            ArcweftExecutionTier::RuntimePlanVm,
        )
        .unwrap()
    }
}

fn returns(source: &str, expected: &str) {
    for backend in BACKENDS {
        let mut executor = executor(source, backend);
        let mut done = false;
        for step in 0..128 {
            let output = executor.step(RuntimeStepInput::default(), RuntimeStepOptions::default());
            assert!(output.stats.executed_ops <= 1);
            assert!(
                output.output.diagnostics.is_empty(),
                "backend={backend:?}, step={step}, output={:?}, fiber={:?}",
                output.output,
                executor.fiber()
            );
            if !matches!(backend, Backend::Native) {
                executor = restore_product(executor);
            }
            if matches!(
                executor.fiber().frame.status,
                FlowFiberStatus::Done(_) | FlowFiberStatus::Failed(_)
            ) {
                assert_eq!(
                    executor.fiber().frame.status,
                    FlowFiberStatus::Done(FlowExit::Return(expected.into()))
                );
                done = true;
                break;
            }
        }
        assert!(
            done,
            "Include did not resume its caller; backend={backend:?}"
        );
    }
}

fn restore_product(executor: ArcweftRuntimeExecutor) -> ArcweftRuntimeExecutor {
    let ArcweftRuntimeExecutorSnapshot::AwbcProduct(snapshot) = executor.snapshot().unwrap();
    let saved =
        AwbcProductExecutorSaveSnapshot::from_live(&snapshot, &executor.program_owner()).unwrap();
    let bytes = serde_json::to_vec(&saved).unwrap();
    let decoded: AwbcProductExecutorSaveSnapshot = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(decoded, saved);
    let bound = executor.bind_product_save_snapshot(decoded).unwrap();
    executor
        .restore_snapshot(ArcweftRuntimeExecutorSnapshot::AwbcProduct(bound))
        .unwrap()
}

#[test]
fn forward_include_implicitly_completes_and_resumes_native_and_decoded_awbc() {
    returns(
        concat!(
            "flow main() -> String { include @flow.snippet; return \"after\" }\n",
            "flow snippet() {}\n",
            "entry cli @entry.main { goto @flow.main }\n",
        ),
        "after",
    );
}

#[test]
fn include_explicit_unit_and_value_returns_resume_native_and_decoded_awbc() {
    for snippet in [
        "flow snippet() { return () }",
        "flow snippet() -> String { return \"callee\" }",
    ] {
        returns(
            &format!(
                "flow main() -> String {{ include @flow.snippet; return \"after\" }}\n{snippet}\nentry cli @entry.main {{ goto @flow.main }}\n"
            ),
            "after",
        );
    }
}

#[test]
fn repeated_and_nested_includes_share_original_functions_and_resume_their_callers() {
    returns(
        concat!(
            "flow main() -> String { include @flow.first; include @flow.first; return \"after\" }\n",
            "flow first() { include @flow.second }\n",
            "flow second() {}\n",
            "entry cli @entry.main { goto @flow.main }\n",
        ),
        "after",
    );
}

#[test]
fn mutually_recursive_includes_execute_under_the_original_one_operation_budget() {
    let source = concat!(
        "flow main() { include @flow.first }\n",
        "flow first() { include @flow.second }\n",
        "flow second() { include @flow.first }\n",
        "entry cli @entry.main { goto @flow.main }\n",
    );
    for backend in BACKENDS {
        let mut executor = executor(source, backend);
        for _ in 0..16 {
            let output = executor.step(RuntimeStepInput::default(), RuntimeStepOptions::default());
            assert_eq!(output.stats.executed_ops, 1);
            assert!(
                output.output.diagnostics.is_empty(),
                "{:?}",
                output.output.diagnostics
            );
            assert!(!matches!(
                executor.fiber().frame.status,
                FlowFiberStatus::Done(_) | FlowFiberStatus::Failed(_)
            ));
            if !matches!(backend, Backend::Native) {
                executor = restore_product(executor);
            }
        }
    }
}

#[test]
fn goto_inside_an_include_replaces_the_actual_flow_without_returning_to_its_caller() {
    returns(
        concat!(
            "flow main() -> String { include @flow.snippet; return \"not-reached\" }\n",
            "flow snippet() -> Never { goto @flow.destination }\n",
            "flow destination() -> String { return \"destination\" }\n",
            "entry cli @entry.main { goto @flow.main }\n",
        ),
        "destination",
    );
}

#[test]
fn include_in_a_function_owned_thread_joins_before_its_flow_caller_returns() {
    returns(
        concat!(
            "fn reusable() -> ThreadHandle<Unit> effects { control.spawn } { thread { include @flow.snippet } }\n",
            "flow main() -> String { let child = reusable(); await child; return \"after-join\" }\n",
            "flow snippet() {}\n",
            "entry cli @entry.main { goto @flow.main }\n",
        ),
        "after-join",
    );
}

const NATIVE_THREAD_CAPTURE_SOURCE: &str = concat!(
    "fn launch(flag: bool) -> Unit effects { control.spawn } {\n",
    "    thread worker { if flag { include @flow.snippet } }\n",
    "    ()\n",
    "}\n",
    "flow main() -> String { launch(true); return \"after-thread\" }\n",
    "flow snippet() {}\n",
    "entry cli @entry.main { goto @flow.main }\n",
);

#[test]
fn native_returned_thread_handle_delivers_its_actual_non_unit_value() {
    for returned in [false, true] {
        let source = format!(
            "fn reusable() -> ThreadHandle<bool> effects {{ control.spawn }} {{ thread {{ return {returned} }} }}\n\
             flow main() -> bool {{ let child = reusable(); return await child }}\n\
             entry cli @entry.main {{ goto @flow.main }}\n"
        );
        let mut runtime = executor(&source, Backend::Native);
        let mut tasks = Vec::new();
        let mut completions = Vec::new();
        let mut done = false;
        for step in 0..128 {
            let result = runtime.step(RuntimeStepInput::default(), RuntimeStepOptions::default());
            assert!(result.stats.executed_ops <= 1);
            assert!(
                result.output.diagnostics.is_empty(),
                "returned={returned}, step={step}: {:?}",
                result.output.diagnostics
            );
            for task in result.output.requests.tasks {
                assert!(matches!(
                    task.spec().request,
                    arcweft_core::task::TaskRequest::Program(_)
                ));
                tasks.push(task.handle().correlation);
            }
            completions.extend(result.output.requests.program_task_completions);
            match &runtime.fiber().frame.status {
                FlowFiberStatus::Done(exit) => {
                    assert_eq!(exit, &FlowExit::Return(returned.to_string()));
                    assert_eq!(tasks.len(), 1);
                    assert_eq!(completions.len(), 1);
                    assert_eq!(completions[0].handle().correlation, tasks[0]);
                    assert_eq!(
                        completions[0].terminal(),
                        &arcweft_core::task::ProgramTaskTerminal::Completed
                    );
                    done = true;
                    break;
                }
                FlowFiberStatus::Failed(error) => panic!("the returned child failed: {error}"),
                _ => {}
            }
        }
        assert!(
            done,
            "the returned handle must yield its actual child value"
        );
    }
}

#[test]
fn native_thread_retires_its_actual_non_unit_source_result_contract() {
    let source = concat!(
        "flow main() -> String { thread worker { return true }; return \"after-value\" }\n",
        "entry cli @entry.main { goto @flow.main }\n",
    );
    let mut runtime = executor(source, Backend::Native);
    let expected = arcweft_lang_sema::types::TypeKind::Bool
        .semantic_identity_digest()
        .unwrap();
    let mut submissions = Vec::new();
    let mut reports = Vec::new();
    for step in 0..128 {
        let result = runtime.step(RuntimeStepInput::default(), RuntimeStepOptions::default());
        assert!(result.stats.executed_ops <= 1);
        assert!(
            result.output.diagnostics.is_empty(),
            "step={step}: {:?}",
            result.output.diagnostics
        );
        for task in result.output.requests.tasks {
            assert_eq!(
                task.spec().outcome.payload_semantic_identity().as_bytes(),
                expected.as_bytes()
            );
            submissions.push(task.handle().correlation);
        }
        reports.extend(result.output.requests.program_task_completions);
        match &runtime.fiber().frame.status {
            FlowFiberStatus::Done(FlowExit::Return(value)) => {
                assert_eq!(value, "after-value");
                assert_eq!(submissions.len(), 1);
                assert_eq!(reports.len(), 1);
                assert_eq!(reports[0].handle().correlation, submissions[0]);
                assert_eq!(
                    reports[0].terminal(),
                    &arcweft_core::task::ProgramTaskTerminal::Completed
                );
                return;
            }
            FlowFiberStatus::Failed(error) => panic!("the source child failed: {error}"),
            _ => {}
        }
    }
    panic!("the actual source child must retire within the original one-operation steps");
}

#[test]
fn native_thread_capture_source_and_child_resume_under_one_operation_steps() {
    let mut runtime = executor(NATIVE_THREAD_CAPTURE_SOURCE, Backend::Native);
    let mut submissions = 0;
    let mut observed_child = false;
    for step in 0..128 {
        let result = runtime.step(RuntimeStepInput::default(), RuntimeStepOptions::default());
        assert!(result.stats.executed_ops <= 1);
        assert!(
            result.output.diagnostics.is_empty(),
            "step={step}: {:?}",
            result.output.diagnostics
        );
        for task in &result.output.requests.tasks {
            assert_eq!(task.spec().class, arcweft_core::task::TaskClass::Cpu);
            assert_eq!(task.spec().debug_label, "worker");
            assert_eq!(result.stats.child_fibers, 1);
            submissions += 1;
        }
        observed_child |= result.stats.child_fibers > 0;
        match &runtime.fiber().frame.status {
            FlowFiberStatus::Done(exit) => {
                assert_eq!(exit, &FlowExit::Return("after-thread".into()));
                assert_eq!(submissions, 1);
                assert!(observed_child);
                return;
            }
            FlowFiberStatus::Failed(message) => panic!("Thread failed: {message}"),
            _ => {}
        }
    }
    panic!("Thread capture execution did not complete");
}

#[test]
fn native_thread_quota_rejection_publishes_no_child_or_task() {
    let mut runtime = executor(NATIVE_THREAD_CAPTURE_SOURCE, Backend::Native);
    for step in 0..128 {
        let result = runtime.step(
            RuntimeStepInput::default(),
            RuntimeStepOptions {
                max_new_task_requests: 0,
                ..RuntimeStepOptions::default()
            },
        );
        assert!(result.stats.executed_ops <= 1);
        assert_eq!(result.stats.child_fibers, 0);
        assert!(result.output.requests.tasks.is_empty());
        if !result.output.diagnostics.is_empty() {
            assert!(
                result.output.diagnostics.iter().any(|diagnostic| {
                    diagnostic.category == arcweft_core::step::RuntimeDiagnosticCategory::Budget
                        && diagnostic.message.contains("Thread start")
                }),
                "step={step}: {:?}",
                result.output.diagnostics
            );
            return;
        }
        assert!(!matches!(
            runtime.fiber().frame.status,
            FlowFiberStatus::Done(_)
        ));
    }
    panic!("zero Thread task quota was not enforced");
}

#[test]
fn native_sibling_thread_includes_return_to_their_callers_without_mutual_join() {
    let source = concat!(
        "flow main() -> String {\n",
        "    thread first { include @flow.child }\n",
        "    thread second { include @flow.child }\n",
        "    return \"joined\"\n",
        "}\n",
        "flow child() -> Unit { return () }\n",
        "entry cli @entry.main { goto @flow.main }\n",
    );
    let mut runtime = executor(source, Backend::Native);
    let mut submissions = 0;
    for step in 0..128 {
        let result = runtime.step(RuntimeStepInput::default(), RuntimeStepOptions::default());
        assert!(result.stats.executed_ops <= 1);
        assert!(
            result.output.diagnostics.is_empty(),
            "step={step}: {:?}",
            result.output.diagnostics
        );
        submissions += result.output.requests.tasks.len();
        if let FlowFiberStatus::Done(exit) = &runtime.fiber().frame.status {
            assert_eq!(exit, &FlowExit::Return("joined".into()));
            assert_eq!(submissions, 2);
            assert_eq!(result.stats.child_fibers, 0);
            return;
        }
    }
    panic!("sibling Include returns joined each other's tasks");
}

#[test]
fn native_spawn_function_returns_before_its_child_finishes() {
    use arcweft_core::effect::LineEffectRequest;

    let source = concat!(
        "fn launch() -> Unit effects { control.spawn, log.write } {\n",
        "    thread worker {\n",
        "        log.info(\"child-start\")\n",
        "        log.info(\"child-1\")\n",
        "        log.info(\"child-2\")\n",
        "        log.info(\"child-3\")\n",
        "        log.info(\"child-4\")\n",
        "        log.info(\"child-finished\")\n",
        "    }\n",
        "    ()\n",
        "}\n",
        "flow main() -> String { launch(); log.info(\"parent-resumed\"); return \"joined\" }\n",
        "entry cli @entry.main { goto @flow.main }\n",
    );
    let mut runtime = executor(source, Backend::Native);
    let mut logs = Vec::new();
    for step in 0..128 {
        let result = runtime.step(RuntimeStepInput::default(), RuntimeStepOptions::default());
        assert!(result.stats.executed_ops <= 1);
        assert!(
            result.output.diagnostics.is_empty(),
            "step={step}: {:?}",
            result.output.diagnostics
        );
        logs.extend(
            result
                .output
                .effects
                .line
                .into_iter()
                .filter_map(|effect| match effect {
                    LineEffectRequest::Log(log) => Some(log.message),
                    _ => None,
                }),
        );
        if let FlowFiberStatus::Done(exit) = &runtime.fiber().frame.status {
            assert_eq!(exit, &FlowExit::Return("joined".into()));
            assert_eq!(result.stats.child_fibers, 0);
            let resumed = logs
                .iter()
                .position(|message| message == "parent-resumed")
                .unwrap();
            let finished = logs
                .iter()
                .position(|message| message == "child-finished")
                .unwrap();
            assert!(
                resumed < finished,
                "ordinary return waited for its spawned child: {logs:?}"
            );
            return;
        }
    }
    panic!("parent task did not finish after its child");
}

#[test]
fn native_nested_thread_finishes_its_own_descendants_before_the_outer_flow() {
    use arcweft_core::effect::LineEffectRequest;

    let source = concat!(
        "flow main() -> String {\n",
        "    thread parent {\n",
        "        thread descendant {\n",
        "            log.info(\"descendant-start\")\n",
        "            log.info(\"descendant-middle\")\n",
        "            log.info(\"descendant-finish\")\n",
        "        }\n",
        "        include @flow.child\n",
        "    }\n",
        "    thread sibling { include @flow.child }\n",
        "    return \"joined\"\n",
        "}\n",
        "flow child() -> Unit { return () }\n",
        "entry cli @entry.main { goto @flow.main }\n",
    );
    let mut runtime = executor(source, Backend::Native);
    let mut submissions = 0;
    let mut descendant_finished = false;
    for step in 0..128 {
        let result = runtime.step(RuntimeStepInput::default(), RuntimeStepOptions::default());
        assert!(result.stats.executed_ops <= 1);
        assert!(
            result.output.diagnostics.is_empty(),
            "step={step}: {:?}",
            result.output.diagnostics
        );
        submissions += result.output.requests.tasks.len();
        descendant_finished |= result.output.effects.line.iter().any(|effect| {
            matches!(effect, LineEffectRequest::Log(log) if log.message == "descendant-finish")
        });
        if let FlowFiberStatus::Done(exit) = &runtime.fiber().frame.status {
            assert_eq!(exit, &FlowExit::Return("joined".into()));
            assert_eq!(submissions, 3);
            assert!(descendant_finished);
            assert_eq!(result.stats.child_fibers, 0);
            return;
        }
    }
    panic!("nested Thread did not join its actual descendants");
}

#[test]
fn include_rejects_a_parameterized_flow_before_executable_publication() {
    use arcweft_compiler::project::ProjectCompileStage;

    let declaration = "flow snippet(value: String) {}\n";
    compile_source(&format!(
        "{declaration}flow main() {{}}\nentry cli @entry.main {{ goto @flow.main }}\n"
    ))
    .expect("the parameterized Flow itself is valid");
    let error = compile_source(&format!(
        "{declaration}flow main() {{ include @flow.snippet }}\nentry cli @entry.main {{ goto @flow.main }}\n"
    ))
    .expect_err("Include cannot fabricate the required input");
    assert_eq!(
        error.project().stage(),
        ProjectCompileStage::RuntimePlanLower.as_str()
    );
    assert!(
        error.project().diagnostics().iter().any(|diagnostic| {
            diagnostic.stage() == ProjectCompileStage::RuntimePlanLower
                && diagnostic
                    .diagnostic()
                    .message()
                    .contains("zero-input invocation ABI")
        }),
        "{error:?}"
    );
}
