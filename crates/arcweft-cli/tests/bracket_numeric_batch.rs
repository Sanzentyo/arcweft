use arcweft_core::{
    effect::LineEffectRequest,
    engine::{Engine, FlowExit, FlowFiberStatus},
    pure::RuntimePureCallBackend,
    step::{
        RuntimePureCallStats, RuntimeStepBudget, RuntimeStepInput, RuntimeStepMode,
        RuntimeStepOptions,
    },
};
use arcweft_runtime_accelerator::{
    RuntimePureAccelerator, RuntimePureAcceleratorConfig, RuntimePureBackendMode,
    RuntimePureWorkerCount,
};

const SOURCE: &str = r#"
#[pure]
fn score(base: i64, bonus: i64) -> i64 { return base * (bonus + 2i64) }
flow main() -> String {
    let scores: Vec<i64> = [score(1i64, 2i64), score(2i64, 2i64), score(3i64, 2i64), score(4i64, 2i64)]
    for item in scores { log.info(item) }
    return "done"
}
"#;

#[test]
fn nary_bracket_calls_keep_one_op_results_and_ordinary_continuations() {
    let stats = execute(RuntimeStepMode::OneOp, 1);
    assert_eq!(stats.pure_calls, 4);
    assert_eq!(stats.vm_calls, 4);
    assert_eq!(stats.batch_calls, 0);
    assert_eq!(stats.jit_calls, 0);
}

#[test]
fn nary_bracket_calls_batch_four_complete_rows_with_the_original_eight_op_budget() {
    let stats = execute(RuntimeStepMode::Drain, 8);
    assert_eq!(stats.pure_calls, 4);
    assert_eq!(stats.batch_calls, 1);
    assert_eq!(stats.batch_items, 4);
    assert_eq!(stats.flat_batch_calls, 1);
    assert_eq!(stats.flat_batch_items, 4);
    assert_eq!(stats.flat_batch_bytes_borrowed, 64);
    assert_eq!(stats.flatten_materializations, 0);
    assert_eq!(stats.flatten_bytes_copied, 0);
    assert_eq!(stats.jit_calls, 4);
    assert_eq!(stats.vm_calls, 0);
    assert_eq!(stats.arg_stack_packs, 0);
    assert_eq!(stats.arg_vec_allocations, 0);
    assert_eq!(stats.arg_bytes_copied, 0);
}

fn execute(mode: RuntimeStepMode, max_ops: usize) -> RuntimePureCallStats {
    let compiled = arcweft_compiler::source::compile_source(SOURCE).unwrap();
    let [flow] = compiled.plan.flows() else {
        panic!("one real source flow")
    };
    let mut engine = Engine::for_flow(compiled.plan.clone(), &flow.id).unwrap();
    let plan = std::sync::Arc::new(compiled.plan);
    let mut backend = RuntimePureAccelerator::with_config(
        RuntimePureAcceleratorConfig {
            backend: RuntimePureBackendMode::Jit,
            workers: RuntimePureWorkerCount::Fixed(1),
            ..RuntimePureAcceleratorConfig::default()
        },
        &plan,
    );
    let options = RuntimeStepOptions {
        mode,
        budget: RuntimeStepBudget { max_ops },
        ..RuntimeStepOptions::default()
    };
    let mut messages = Vec::new();
    for _ in 0..128 {
        let step =
            engine.step_with_pure_backend(RuntimeStepInput::default(), options, &mut backend);
        assert!(step.stats.executed_ops <= max_ops);
        assert!(
            step.output.diagnostics.is_empty(),
            "{:?}",
            step.output.diagnostics
        );
        messages.extend(
            step.output
                .effects
                .line
                .into_iter()
                .filter_map(|effect| match effect {
                    LineEffectRequest::Log(log) => Some(log.message),
                    _ => None,
                }),
        );
        assert!(
            !matches!(step.fiber_status, FlowFiberStatus::Failed(_)),
            "{:?}",
            step.fiber_status
        );
        if matches!(step.fiber_status, FlowFiberStatus::Done(_)) {
            assert_eq!(
                step.fiber_status,
                FlowFiberStatus::Done(FlowExit::Return("done".into()))
            );
            assert_eq!(messages, ["4", "8", "12", "16"]);
            return backend.stats();
        }
    }
    panic!("the original bracket execution must finish within the retained bound");
}
