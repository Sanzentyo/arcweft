use super::*;
use arcweft_core::awbc::{codec::AwbcDecodeBudget, schema::AwbcProgram};
use arcweft_core::effect::LineEffectRequest;
use arcweft_core::engine::{Engine, FlowFiberStatus};
use arcweft_core::pure::{RuntimePureCallBackend, VmRuntimePureCallBackend};
use arcweft_core::step::{RuntimeStepBudget, RuntimeStepMode, RuntimeStepOptions};
use arcweft_lang_sema::final_analysis::{CheckedExecutionBodyOwner, CheckedExecutionSource};

#[test]
fn every_map_family_uses_a_control_program_with_exact_values_and_callback_count() {
    let sequence = || {
        arcweft_core::value::runtime_sequence_values(vec![
            RuntimeValue::i64(1),
            RuntimeValue::i64(2),
            RuntimeValue::i64(3),
        ])
    };
    let mapped = || {
        arcweft_core::value::runtime_sequence_values(vec![
            RuntimeValue::i64(2),
            RuntimeValue::i64(3),
            RuntimeValue::i64(4),
        ])
    };
    let cases = [
        ("Vec<i64>", "Vec<i64>", sequence(), mapped(), 3),
        ("Seq<i64>", "Seq<i64>", sequence(), mapped(), 3),
        ("Array<i64, 3>", "Array<i64, 3>", sequence(), mapped(), 3),
        ("Slice<i64>", "Vec<i64>", sequence(), mapped(), 3),
        (
            "Option<i64>",
            "Option<i64>",
            RuntimeValue::option_some(RuntimeValue::i64(7)),
            RuntimeValue::option_some(RuntimeValue::i64(8)),
            1,
        ),
        (
            "Option<i64>",
            "Option<i64>",
            RuntimeValue::option_none(),
            RuntimeValue::option_none(),
            0,
        ),
        (
            "Result<i64, String>",
            "Result<i64, String>",
            RuntimeValue::result_ok(RuntimeValue::i64(9)),
            RuntimeValue::result_ok(RuntimeValue::i64(10)),
            1,
        ),
        (
            "Result<i64, String>",
            "Result<i64, String>",
            RuntimeValue::result_err(RuntimeValue::String("preserve".to_owned())),
            RuntimeValue::result_err(RuntimeValue::String("preserve".to_owned())),
            0,
        ),
    ];
    for (input_type, output_type, input, expected, callbacks) in cases {
        for (annotation, expression) in [
            ("", "values.map(increment)".to_owned()),
            ("#[pure]", "map(increment)(values)".to_owned()),
            (
                "#[pure]",
                format!(
                    "{{ let mapper: {input_type} -> {output_type} effects {{}} = map(increment); mapper(values) }}"
                ),
            ),
        ] {
            let source = format!(
                r#"
#[pure]
fn increment(item: i64) -> i64 {{ return item + 1i64 }}
{annotation}
pub fn root(values: {input_type}) -> {output_type} {{ {expression} }}
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
            let mut maps = 0;
            selected.plan().visit_flow_ops(&mut |op| {
                if matches!(op, arcweft_core::plan::FlowOp::StandardMap(_)) {
                    maps += 1;
                }
            });
            assert_eq!(maps, 1, "{input_type}: {expression}");
            let mut native = Engine::for_program_invocation(
                Arc::clone(selected.plan()),
                selected.program(),
                vec![input.clone()],
            )
            .unwrap();
            let mut backend = VmRuntimePureCallBackend::default();
            for _ in 0..512 {
                let step = native.step_with_pure_backend(
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
                assert!(!matches!(step.fiber_status, FlowFiberStatus::Failed(_)));
                if matches!(step.fiber_status, FlowFiberStatus::Done(_)) {
                    break;
                }
            }
            assert_eq!(native.take_program_result().unwrap().unwrap().1, expected);
            assert_eq!(
                backend.stats().pure_calls,
                callbacks,
                "{input_type}: {expression}"
            );
            let lowered = AwbcLowerer::new(
                selected.plan(),
                &selected.lowering_report().dialogue_content_catalog,
                "map-program.arcw",
            )
            .lower()
            .unwrap()
            .program;
            let bytes = lowered.encode_canonical().unwrap();
            let decoded =
                AwbcProgram::decode_canonical(&bytes, AwbcDecodeBudget::default()).unwrap();
            assert_eq!(decoded, lowered);
            let mut product_backend = VmRuntimePureCallBackend::default();
            let product = arcweft_core::awbc::product_step::evaluate_pure_program_with_backend(
                &Arc::new(decoded),
                selected.program(),
                &[input.clone()],
                &mut product_backend,
            )
            .unwrap();
            assert_eq!(product, expected, "{input_type}: {expression}");
            assert_eq!(product_backend.stats().awbc_pure_program_calls, 1);
        }
    }
}

#[test]
fn retained_map_prefix_can_reuse_a_copy_mapping() {
    assert_map_source_order(
        "let mapper: Vec<i64> -> Vec<i64> effects {} = map(increment); let first = mapper(values()).sum(); let second = mapper(values()).sum(); return first + second",
        "18",
        &["values", "values"],
        6,
    );
}

#[test]
fn retained_map_prefix_evaluates_mapping_once_and_keeps_source_order() {
    assert_map_source_order(
        "let mapper: Vec<i64> -> Vec<i64> effects {} = map(factory()); return mapper(values()).sum()",
        "9",
        &["mapping", "values"],
        3,
    );
}

#[test]
fn direct_map_evaluates_receiver_before_mapping() {
    assert_map_source_order(
        "return values().map(factory()).sum()",
        "9",
        &["values", "mapping"],
        3,
    );
}

#[test]
fn retained_map_prefix_invokes_each_callback_with_its_declared_effects() {
    assert_map_source_order(
        "let mapper: Vec<i64> -> Vec<i64> effects { log.write } = map(logging_increment); return mapper(values()).sum()",
        "9",
        &["values", "1", "2", "3"],
        0,
    );
}

fn assert_map_source_order(body: &str, expected: &str, messages: &[&str], pure_calls: usize) {
    let source = format!(
        r#"
entry cli @entry.main {{ goto @flow.main }}
#[pure]
fn increment(item: i64) -> i64 {{ item + 1i64 }}
fn logging_increment(item: i64) -> i64 {{ log.info(item); item + 1i64 }}
fn factory() -> (i64 -> i64 effects {{}}) {{ log.info("mapping"); increment }}
fn values() -> Vec<i64> {{ log.info("values"); [1i64, 2i64, 3i64] }}
flow main() -> i64 {{ {body} }}
"#
    );
    let compiled = compile_source(&source).unwrap();
    let [flow] = compiled.plan.flows() else {
        panic!("one flow");
    };
    let mut native = Engine::for_flow(compiled.plan.clone(), &flow.id).unwrap();
    let lowered = AwbcLowerer::new(&compiled.plan, &compiled.dialogue_content, "map-order.arcw")
        .lower()
        .unwrap()
        .program;
    let bytes = lowered.encode_canonical().unwrap();
    let decoded = AwbcProgram::decode_canonical(&bytes, AwbcDecodeBudget::default()).unwrap();
    let mut awbc = arcweft_core::executor::ArcweftRuntimeExecutor::from_awbc_product(
        decoded,
        arcweft_core::awbc::schema::AwbcEntryId(0),
    )
    .unwrap();
    let options = RuntimeStepOptions {
        mode: RuntimeStepMode::OneOp,
        budget: RuntimeStepBudget { max_ops: 1 },
        ..Default::default()
    };
    let run =
        |step: &mut dyn FnMut() -> arcweft_core::step::RuntimeStepResult| {
            let mut actual = Vec::new();
            for _ in 0..2048 {
                let step = step();
                assert!(step.stats.executed_ops <= 1);
                assert!(
                    step.output.diagnostics.is_empty(),
                    "{:?}",
                    step.output.diagnostics
                );
                actual.extend(step.output.effects.line.into_iter().filter_map(
                    |effect| match effect {
                        LineEffectRequest::Log(log) => Some(log.message),
                        _ => None,
                    },
                ));
                match step.fiber_status {
                    FlowFiberStatus::Done(arcweft_core::engine::FlowExit::Return(value)) => {
                        assert_eq!(value, expected);
                        return actual;
                    }
                    FlowFiberStatus::Failed(failure) => panic!("Map failed: {failure:?}"),
                    _ => {}
                }
            }
            panic!("Map did not finish its bounded schedule");
        };
    let mut native_backend = VmRuntimePureCallBackend::default();
    let mut awbc_backend = VmRuntimePureCallBackend::default();
    assert_eq!(
        run(&mut || native.step_with_pure_backend(
            Default::default(),
            options,
            &mut native_backend
        )),
        messages
    );
    assert_eq!(native_backend.stats().pure_calls, pure_calls);
    assert_eq!(
        run(&mut || awbc.step_with_pure_backend(Default::default(), options, &mut awbc_backend)),
        messages
    );
}

#[test]
fn map_callback_effects_cannot_enter_a_pure_invocation() {
    for expression in [
        "values.map(logging_increment)",
        "map(logging_increment)(values)",
        "{ let mapper: Vec<i64> -> Vec<i64> effects { log.write } = map(logging_increment); mapper(values) }",
    ] {
        let source = format!(
            "fn logging_increment(item: i64) -> i64 {{ log.info(item); item + 1i64 }}\n#[pure]\npub fn root(values: Vec<i64>) -> Vec<i64> {{ {expression} }}"
        );
        let error = compile_source(&source).expect_err("Map must retain its callback effects");
        assert!(
            error.project().diagnostics().iter().any(|diagnostic| {
                diagnostic
                    .diagnostic()
                    .code()
                    .is_some_and(|code| code.as_str() == "AWF-EFX-003")
            }),
            "{expression}: {error:?}"
        );
    }
}
