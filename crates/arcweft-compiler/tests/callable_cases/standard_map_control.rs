use super::*;

#[test]
fn seq_and_slice_preserve_owned_inputs_in_native_and_decoded_awbc() {
    use arcweft_core::awbc::{
        codec::AwbcDecodeBudget, product_step::AwbcProductStepExecutor, schema::AwbcProgram,
    };
    use arcweft_core::engine::{Engine, FlowFiberStatus};
    use arcweft_core::step::{RuntimeStepBudget, RuntimeStepMode, RuntimeStepOptions};
    use arcweft_lang_sema::final_analysis::{CheckedExecutionBodyOwner, CheckedExecutionSource};
    for family in ["Seq", "Slice"] {
        let source = format!(
            r#"
fn score(value: i64, bonus: i64) -> i64 {{ value + bonus }}
pub fn root(values: {family}<i64>, bonus: i64) -> i64 {{
    values.map(|item| score(item, bonus)).sum()
}}
flow main() -> String {{ return "unused" }}
"#
        );
        let compiled = compile_source(&source).unwrap();
        let declaration = compiled
            .analysis
            .final_analysis()
            .hir_topology()
            .modules()
            .iter()
            .flat_map(|module| module.entries())
            .filter_map(|entry| entry.body())
            .find(|body| body.declaration().name() == "root")
            .unwrap()
            .declaration()
            .clone();
        let selected = compiled
            .analysis
            .compile_deterministic_program(
                CheckedExecutionSource::InvokeBody(CheckedExecutionBodyOwner::Declaration {
                    declaration,
                    role: arcweft_lang_hir::project::HirDeclarationBodyRootRole::FunctionBody,
                }),
                None,
                &Default::default(),
            )
            .unwrap();
        let inputs = || {
            vec![
                arcweft_core::value::runtime_sequence_values(vec![
                    RuntimeValue::i64(1),
                    RuntimeValue::i64(2),
                    RuntimeValue::i64(3),
                ]),
                RuntimeValue::i64(2),
            ]
        };
        let mut engine = Engine::for_program_invocation(
            Arc::clone(selected.plan()),
            selected.program(),
            inputs(),
        )
        .unwrap();
        let lowered = AwbcLowerer::new(
            selected.plan(),
            &selected.lowering_report().dialogue_content_catalog,
            "map-input.arcw",
        )
        .lower()
        .unwrap()
        .program;
        let bytes = lowered.encode_canonical().unwrap();
        let decoded = AwbcProgram::decode_canonical(&bytes, AwbcDecodeBudget::default()).unwrap();
        assert_eq!(decoded, lowered);
        let mut awbc = AwbcProductStepExecutor::for_program_invocation(
            Arc::new(decoded),
            selected.program(),
            inputs(),
            arcweft_core::task::GenerationId::new(0),
            1,
        )
        .unwrap();
        let options = RuntimeStepOptions {
            mode: RuntimeStepMode::OneOp,
            budget: RuntimeStepBudget { max_ops: 1 },
            ..Default::default()
        };
        let mut backend = arcweft_core::pure::VmRuntimePureCallBackend::default();
        let mut native_done = false;
        let mut awbc_done = false;
        for _ in 0..4096 {
            if !native_done {
                let step = engine.step(Default::default(), options);
                assert!(step.stats.executed_ops <= 1);
                assert!(
                    step.output.diagnostics.is_empty(),
                    "{family}: {:?}",
                    step.output.diagnostics
                );
                assert!(!matches!(step.fiber_status, FlowFiberStatus::Failed(_)));
                native_done = matches!(step.fiber_status, FlowFiberStatus::Done(_));
            }
            if !awbc_done {
                let step = awbc.step_with_pure_backend(Default::default(), options, &mut backend);
                assert!(step.stats.executed_ops <= 1);
                assert!(
                    step.output.diagnostics.is_empty(),
                    "{family}: {:?}",
                    step.output.diagnostics
                );
                assert!(!matches!(step.fiber_status, FlowFiberStatus::Failed(_)));
                awbc_done = matches!(step.fiber_status, FlowFiberStatus::Done(_));
            }
            if native_done && awbc_done {
                break;
            }
        }
        assert!(
            native_done && awbc_done,
            "{family} must finish its bounded schedule"
        );
        assert_eq!(
            engine.take_program_result().unwrap().unwrap().1,
            RuntimeValue::i64(12)
        );
        assert_eq!(
            awbc.take_program_result().unwrap().unwrap().1,
            RuntimeValue::i64(12)
        );
    }
}

fn assert_native_return(source: &str, expected: &str) {
    use arcweft_core::engine::{Engine, FlowExit, FlowFiberStatus};
    use arcweft_core::plan::FlowOp;
    use arcweft_core::step::{RuntimeStepBudget, RuntimeStepMode, RuntimeStepOptions};
    let compiled = compile_source(source).expect("source Map compiles");
    let mut maps = 0;
    compiled.plan.visit_flow_ops(&mut |op| {
        if matches!(op, FlowOp::StandardMap(_)) {
            maps += 1;
        }
    });
    assert_eq!(maps, 1, "one final typed Map owner");
    let flow = compiled.plan.flows()[0].id.clone();
    let mut engine = Engine::for_flow(compiled.plan, &flow).unwrap();
    for _ in 0..512 {
        let step = engine.step(
            Default::default(),
            RuntimeStepOptions {
                mode: RuntimeStepMode::OneOp,
                budget: RuntimeStepBudget { max_ops: 1 },
                ..Default::default()
            },
        );
        assert!(step.stats.executed_ops <= 1);
        assert!(
            step.output.diagnostics.is_empty(),
            "{:?}",
            step.output.diagnostics
        );
        match step.fiber_status {
            FlowFiberStatus::Running => {}
            FlowFiberStatus::Done(exit) => {
                assert_eq!(exit, FlowExit::Return(expected.to_owned()));
                return;
            }
            status => panic!("Map stopped unexpectedly: {status:?}"),
        }
    }
    panic!("finite source Map must finish within its original bounded schedule");
}

fn assert_awbc_return(source: &str, expected: RuntimeValue) {
    use arcweft_core::awbc::{
        codec::AwbcDecodeBudget,
        product_step::AwbcProductStepExecutor,
        schema::{AwbcEntryId, AwbcProgram},
    };
    use arcweft_core::engine::{FlowExit, FlowFiberStatus};
    use arcweft_core::step::{RuntimeStepBudget, RuntimeStepMode, RuntimeStepOptions};
    let compiled = compile_source(source).unwrap();
    let lowered = AwbcLowerer::new(
        &compiled.plan,
        &compiled.dialogue_content,
        "map-control.arcw",
    )
    .lower()
    .unwrap()
    .program;
    let encoded = lowered.encode_canonical().unwrap();
    let decoded = AwbcProgram::decode_canonical(&encoded, AwbcDecodeBudget::default()).unwrap();
    assert_eq!(decoded, lowered);
    let mut executor = AwbcProductStepExecutor::for_entry(decoded, AwbcEntryId(0), 1).unwrap();
    let mut backend = arcweft_core::pure::VmRuntimePureCallBackend::default();
    for _ in 0..4096 {
        let step = executor.step_with_pure_backend(
            Default::default(),
            RuntimeStepOptions {
                mode: RuntimeStepMode::OneOp,
                budget: RuntimeStepBudget { max_ops: 1 },
                ..Default::default()
            },
            &mut backend,
        );
        assert!(step.stats.executed_ops <= 1);
        assert!(
            step.output.diagnostics.is_empty(),
            "{:?}",
            step.output.diagnostics
        );
        match step.fiber_status {
            FlowFiberStatus::Running => {}
            FlowFiberStatus::Done(exit) => {
                assert_eq!(
                    exit,
                    FlowExit::Return(expected.try_i64().unwrap().to_string())
                );
                return;
            }
            status => panic!("decoded AWBC Map stopped unexpectedly: {status:?}"),
        }
    }
    panic!("decoded finite Map must finish within its bounded schedule");
}

callable_case!(
    vec,
    r#"
fn score(value: i64, bonus: i64) -> i64 { value + bonus }
fn mapped(values: Vec<i64>, bonus: i64) -> Vec<i64> {
    values.map(|item| score(item, bonus))
}
flow main() -> i64 { return mapped([1i64, 2i64, 3i64], 2i64).sum() }
"#,
    RuntimeValue::i64(12),
    "12"
);

callable_case!(
    array,
    r#"
fn score(value: i64, bonus: i64) -> i64 { value + bonus }
fn mapped(values: Array<i64, 3>, bonus: i64) -> Array<i64, 3> {
    values.map(|item| score(item, bonus))
}
flow main() -> i64 { return mapped([1i64, 2i64, 3i64], 2i64).sum() }
"#,
    RuntimeValue::i64(12),
    "12"
);

callable_case!(
    option_some,
    r#"
fn score(value: i64, bonus: i64) -> i64 { value + bonus }
fn mapped(value: Option<i64>, bonus: i64) -> Option<i64> {
    value.map(|item| score(item, bonus))
}
flow main() -> i64 {
    return match mapped(Some(40i64), 2i64) { .Some(value) => value, .None => 0i64 }
}
"#,
    RuntimeValue::i64(42),
    "42"
);

callable_case!(
    option_none,
    r#"
fn fail(value: i64) -> i64 { value / (value - value) }
fn mapped(value: Option<i64>) -> Option<i64> { value.map(|item| fail(item)) }
flow main() -> i64 {
    return match mapped(None) { .Some(value) => value, .None => 42i64 }
}
"#,
    RuntimeValue::i64(42),
    "42"
);

callable_case!(
    result_ok,
    r#"
fn score(value: i64, bonus: i64) -> i64 { value + bonus }
fn mapped(value: Result<i64, String>, bonus: i64) -> Result<i64, String> {
    value.map(|item| score(item, bonus))
}
flow main() -> i64 {
    return match mapped(Ok(40i64), 2i64) { .Ok(value) => value, .Err(_) => 0i64 }
}
"#,
    RuntimeValue::i64(42),
    "42"
);

callable_case!(
    result_err,
    r#"
fn fail(value: i64) -> i64 { value / (value - value) }
fn mapped(value: Result<i64, i64>) -> Result<i64, i64> { value.map(|item| fail(item)) }
flow main() -> i64 {
    return match mapped(Err(42i64)) { .Ok(_) => 0i64, .Err(error) => error }
}
"#,
    RuntimeValue::i64(42),
    "42"
);
