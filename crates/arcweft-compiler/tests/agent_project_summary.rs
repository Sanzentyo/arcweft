use arcweft_compiler::source::compile_source;
use arcweft_core::{
    awbc::{codec::AwbcDecodeBudget, product_step::AwbcProductStepExecutor, schema::AwbcProgram},
    engine::{Engine, FlowFiberStatus},
    step::{RuntimeStepBudget, RuntimeStepMode, RuntimeStepOptions},
    value::{RuntimeSeq, RuntimeValue},
};
use arcweft_lang_sema::final_analysis::{CheckedExecutionBodyOwner, CheckedExecutionSource};
use arcweft_runtime_plan::awbc_lower::AwbcLowerer;
use std::sync::Arc;

fn counts(rows: &[(&str, u32)]) -> RuntimeValue {
    RuntimeValue::Seq(RuntimeSeq::values(
        rows.iter()
            .map(|(kind, count)| {
                RuntimeValue::Tuple(vec![
                    RuntimeValue::String((*kind).to_owned()),
                    RuntimeValue::u32(*count),
                ])
            })
            .collect(),
    ))
}

fn summary() -> RuntimeValue {
    RuntimeValue::try_record(vec![
        ("entity_count".to_owned(), RuntimeValue::u32(3)),
        ("agent_action_count".to_owned(), RuntimeValue::u32(0)),
        ("project_callable_count".to_owned(), RuntimeValue::u32(0)),
        ("relation_count".to_owned(), RuntimeValue::u32(4)),
        ("dependency_edge_count".to_owned(), RuntimeValue::u32(0)),
        (
            "dynamic_control_flow_count".to_owned(),
            RuntimeValue::u32(1),
        ),
        ("debug_query_count".to_owned(), RuntimeValue::u32(0)),
        (
            "entity_kind_counts".to_owned(),
            counts(&[("entry", 1), ("flow", 2)]),
        ),
        (
            "relation_kind_counts".to_owned(),
            counts(&[("entry_goto", 1), ("flow_goto", 3)]),
        ),
        ("dependency_edge_kind_counts".to_owned(), counts(&[])),
    ])
    .unwrap()
}

#[test]
fn project_summary_count_maps_execute_from_source_in_native_and_canonical_awbc() {
    for (field, rows) in [
        ("entity_kind_counts", vec![("entry", 1), ("flow", 2)]),
        (
            "relation_kind_counts",
            vec![("entry_goto", 1), ("flow_goto", 3)],
        ),
        ("dependency_edge_kind_counts", vec![]),
    ] {
        let source = format!(
            "pub fn root(summary: AgentProjectGraphSummary) -> BTreeMap<String, u32> {{ summary.{field} }}\nflow main() -> String {{ return \"unused\" }}"
        );
        let compiled = compile_source(&source).expect("typed summary field compiles");
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
        let mut native = Engine::for_program_invocation(
            Arc::clone(selected.plan()),
            selected.program(),
            vec![summary()],
        )
        .unwrap();
        let lowered = AwbcLowerer::new(
            selected.plan(),
            &selected.lowering_report().dialogue_content_catalog,
            "summary.arcw",
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
            vec![summary()],
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
        let (mut native_done, mut awbc_done) = (false, false);
        for _ in 0..64 {
            if !native_done {
                let step = native.step(Default::default(), options);
                assert!(step.stats.executed_ops <= 1);
                assert!(
                    step.output.diagnostics.is_empty(),
                    "{field}: {:?}",
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
                    "{field}: {:?}",
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
            "{field}: bounded execution finishes"
        );
        assert_eq!(
            native.take_program_result().unwrap().unwrap().1,
            counts(&rows)
        );
        assert_eq!(
            awbc.take_program_result().unwrap().unwrap().1,
            counts(&rows)
        );
        let wrong = source.replace("BTreeMap<String, u32>", "BTreeMap<String, u64>");
        assert!(
            compile_source(&wrong).is_err(),
            "{field}: field count width is exact"
        );
        for wrong in [
            source.replace("BTreeMap<String, u32>", "OrderedMap<String, u32>"),
            source.replace("BTreeMap<String, u32>", "BTreeMap<u32, u32>"),
        ] {
            assert!(
                compile_source(&wrong).is_err(),
                "{field}: map kind and key type are exact"
            );
        }
    }
}
