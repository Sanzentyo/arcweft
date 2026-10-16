use arcweft_compiler::source::compile_source;
use arcweft_core::{
    effect::LineEffectRequest,
    engine::{Engine, FlowExit, FlowFiberStatus},
    step::{RuntimeStepBudget, RuntimeStepInput, RuntimeStepMode, RuntimeStepOptions},
};

const SOURCE: &str = r#"
fn score(base: i64, bonus: i64) -> i64 effects {} {
    return base * (bonus + 2i64)
}
entry cli @entry.main { goto @flow.main }
flow main() -> String effects { log.write } {
    let values: Vec<i64> = [1i64, 2i64, 3i64, 4i64]
    for item in values {
        let scored = score(item, 2i64)
        log.info(scored)
    }
    return "done"
}
"#;

#[test]
fn native_for_keeps_the_original_thirty_two_single_operation_steps() {
    let compiled = compile_source(SOURCE).unwrap();
    let entry = compiled.plan.entries()[0].id.clone();
    let mut engine = Engine::for_entry(compiled.plan, &entry).unwrap();
    let options = RuntimeStepOptions {
        mode: RuntimeStepMode::OneOp,
        budget: RuntimeStepBudget { max_ops: 1 },
        ..RuntimeStepOptions::default()
    };
    let mut messages = Vec::new();
    let mut pure_calls = 0;
    for _ in 0..32 {
        let result = engine.step(RuntimeStepInput::default(), options);
        assert!(result.output.diagnostics.is_empty(), "{result:?}");
        assert_eq!(result.stats.executed_ops, 1);
        pure_calls += result.stats.pure.pure_calls;
        messages.extend(result.output.effects.line.into_iter().filter_map(|effect| {
            if let LineEffectRequest::Log(log) = effect {
                Some(log.message)
            } else {
                None
            }
        }));
        if matches!(result.fiber_status, FlowFiberStatus::Done(_)) {
            break;
        }
    }
    assert_eq!(
        engine.fiber().frame.status,
        FlowFiberStatus::Done(FlowExit::Return("done".into()))
    );
    assert_eq!(messages, ["4", "8", "12", "16"]);
    assert_eq!(pure_calls, 4);
}
