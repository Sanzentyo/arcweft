use super::*;
use arcweft_core::awbc::verify::{AwbcVerifyBudget, AwbcVerifyContext};
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
        (
            "Array<i64, 0>",
            "Array<i64, 0>",
            arcweft_core::value::runtime_sequence_values(vec![]),
            arcweft_core::value::runtime_sequence_values(vec![]),
            0,
        ),
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
fn retained_map_prefix_reuses_an_opaque_mapping_through_checked_ingress() {
    assert_map_source_order(
        "return twice(map(factory()))",
        "18",
        &["mapping", "values", "values"],
        6,
    );
}

#[test]
fn an_opaque_map_capture_does_not_invent_a_local_copy_guarantee() {
    let source = r"
#[pure]
fn increment(item: i64) -> i64 { item + 1i64 }
fn factory() -> (i64 -> i64 effects {}) { increment }
flow main() -> i64 {
    let mapper: Vec<i64> -> Vec<i64> effects {} = map(factory());
    let first = mapper([1i64, 2i64, 3i64]).sum();
    return first + mapper([1i64, 2i64, 3i64]).sum()
}
";
    let error = compile_source(source)
        .expect_err("an opaque capture needs its producer or checked ingress Copy evidence");
    assert!(
        matches!(
            error.project().semantic_error(),
            Some(
                arcweft_lang_sema::final_analysis::FinalSemanticAnalysisError::LocalUse(
                    arcweft_lang_sema::final_analysis::CheckedLocalUseError::Unavailable { .. }
                )
            )
        ),
        "{error:?}"
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
fn compiled_map_consumption_owns_source_aliases_and_preserves_one_op_execution() {
    use arcweft_core::plan::{FlowOp, RuntimeStandardMapCompletion};
    let source = r"
#[pure]
fn increment(item: i64) -> i64 { item + 1i64 }
flow main() -> i64 {
    let values: Vec<i64> = [1i64, 2i64, 3i64];
    let mapped = values.map(increment);
    let alias = mapped;
    return alias.sum()
}
";
    let compiled = compile_source(source).unwrap();
    let controls = compiled.plan.flows()[0]
        .body()
        .ops()
        .iter()
        .filter_map(|op| {
            if let FlowOp::StandardMap(control) = op {
                Some(control)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    let [control] = controls.as_slice() else {
        panic!("one compiled Map owner")
    };
    assert!(matches!(
        control.completion(),
        RuntimeStandardMapCompletion::Sum { .. }
    ));
    assert!(control.consuming_body().is_some());
    assert_eq!(compiled.runtime_plan_stats.sequence_map_sum_fusions, 1);
    assert_eq!(compiled.runtime_plan_stats.pure_call_exprs, 1);
    assert_map_source_order(
        "let mapped = values().map(increment); let alias = mapped; return alias.sum()",
        "9",
        &["values"],
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
fn opaque_map_mapping_requires_an_executed_ownership_guard() {
    for body in [
        "return values.map(factory())",
        "let mapper: Vec<i64> -> Vec<i64> effects {} = map(factory()); return mapper(values)",
    ] {
        let source = format!(
            r#"
#[pure]
fn increment(item: i64) -> i64 {{ item + 1i64 }}
fn factory() -> (i64 -> i64 effects {{}}) {{ log.info("mapping"); increment }}
pub fn root(values: Vec<i64>) -> Vec<i64> {{ {body} }}
entry cli @entry.main {{ goto @flow.main }}
flow main() -> i64 {{ return root([1i64, 2i64, 3i64]).sum() }}
"#
        );
        let compiled = compile_source(&source).unwrap();
        let mut lowered =
            AwbcLowerer::new(&compiled.plan, &compiled.dialogue_content, "map-guard.arcw")
                .lower()
                .unwrap()
                .program;
        let mut removed = 0;
        for instruction in &mut lowered.instructions {
            if matches!(
                instruction,
                arcweft_core::awbc::schema::AwbcInstruction::RequireUnrestricted { .. }
            ) {
                *instruction = arcweft_core::awbc::schema::AwbcInstruction::Nop;
                removed += 1;
            }
        }
        assert!(removed > 0);
        let error = lowered
            .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
            .expect_err("an opaque callback has no Copy proof without its runtime guard");
        assert!(
            error
                .to_string()
                .contains("producer-proven unrestricted CopyValue source"),
            "{error:?}"
        );
    }
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
fn twice(mapper: (Vec<i64> -> Vec<i64> effects {{}})) -> i64 {{
    let first = mapper(values()).sum();
    let second = mapper(values()).sum();
    return first + second
}}
flow main() -> i64 {{ {body} }}
"#
    );
    let compiled = compile_source(&source).unwrap();
    let [flow] = compiled.plan.flows() else {
        panic!("one flow");
    };
    let mut native = Engine::for_flow(compiled.plan.clone(), &flow.id).unwrap();
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
            "fn logging_increment(item: i64) -> i64 {{ log.info(item); item + 1i64 }}\npub fn root(values: Vec<i64>) -> Vec<i64> effects {{}} {{ {expression} }}"
        );
        let Err(error) = compile_source(&source) else {
            panic!("{expression}: Map must retain its callback effects");
        };
        assert!(
            matches!(error.project().semantic_error(), Some(
                arcweft_lang_sema::final_analysis::FinalSemanticAnalysisError::EffectUpperBoundExceeded { missing, .. }
            ) if missing.contains(&arcweft_lang_sema::effects::EffectId::log_write())),
            "{expression}: {error:?}"
        );
    }
}

#[test]
fn creating_a_map_prefix_keeps_its_callback_effects_latent() {
    let compiled = compile_source(
        "fn logging_increment(item: i64) -> i64 { log.info(item); item + 1i64 }\npub fn root() -> (Vec<i64> -> Vec<i64> effects { log.write }) effects {} { map(logging_increment) }",
    ).unwrap();
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
        .expect("creating a function does not invoke its callback");
    let mut engine =
        Engine::for_program_invocation(Arc::clone(selected.plan()), selected.program(), vec![])
            .unwrap();
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
        assert!(step.output.effects.line.is_empty());
        assert!(step.output.diagnostics.is_empty());
        if matches!(step.fiber_status, FlowFiberStatus::Done(_)) {
            break;
        }
        assert!(!matches!(step.fiber_status, FlowFiberStatus::Failed(_)));
    }
    assert!(matches!(
        engine.take_program_result().unwrap().unwrap().1,
        RuntimeValue::Callable(_)
    ));
    let lowered = AwbcLowerer::new(
        selected.plan(),
        &compiled.dialogue_content,
        "latent-map.arcw",
    )
    .lower()
    .unwrap()
    .program;
    let decoded = AwbcProgram::decode_canonical(
        &lowered.encode_canonical().unwrap(),
        AwbcDecodeBudget::default(),
    )
    .unwrap();
    let mut backend = VmRuntimePureCallBackend::default();
    let value = arcweft_core::awbc::product_step::evaluate_pure_program_with_backend(
        &Arc::new(decoded),
        selected.program(),
        &[],
        &mut backend,
    )
    .unwrap();
    assert!(matches!(value, RuntimeValue::Callable(_)));
    assert_eq!(backend.stats().awbc_pure_program_calls, 1);
    assert_eq!(backend.stats().pure_calls, 0);
}
