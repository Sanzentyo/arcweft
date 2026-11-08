use super::*;

#[test]
fn readonly_string_methods_preserve_the_receiver_and_count_unicode_scalars() {
    for (text, expected) in [("  Aé🦀é  ", 5), ("", 0), ("👩‍💻", 3)] {
        let source = format!(
            r#"
entry cli @entry.main {{ goto @flow.main }}
flow main() -> usize {{
    let original: String = "{text}"
    let original_copy = original.to_string()
    let trimmed = original.trim()
    let copied = trimmed.to_string()
    let counter = || copied.scalar_count()
    let count = counter()
    if original != "{text}" {{ return 99usize }}
    if original_copy != original {{ return 97usize }}
    if trimmed != copied {{ return 98usize }}
    if count != copied.scalar_count() {{ return 96usize }}
    return count
}}
"#
        );
        assert_native_return(&source, &expected.to_string());
        assert_awbc_return(&source, RuntimeValue::usize(expected));
    }
}

#[test]
fn string_operations_and_captures_survive_each_awbc_step_public_cold_restore() {
    use arcweft_core::{
        awbc::{
            product_step::{AwbcProductExecutorSaveSnapshot, AwbcProductProgram},
            schema::AwbcProgram,
        },
        engine::{FlowExit, FlowFiberStatus},
        executor::{ArcweftRuntimeExecutor, ArcweftRuntimeExecutorSnapshot, RuntimeExecutor},
        plan::RuntimeEntryTarget,
        step::{RuntimeStepBudget, RuntimeStepInput, RuntimeStepMode, RuntimeStepOptions},
    };

    let compiled = compile_source(
        r#"
entry cli @entry.main { goto @flow.main }
flow main() -> usize {
    let original = "  Aé🦀é  "
    let trimmed = original.trim()
    let copied = trimmed.to_string()
    let counter = || copied.scalar_count()
    let count = counter()
    if original != "  Aé🦀é  " { return 99usize }
    if trimmed != copied { return 98usize }
    return count
}
"#,
    )
    .unwrap();
    let [entry] = compiled.plan.entries() else {
        panic!("one original entry");
    };
    let RuntimeEntryTarget::Flow(flow) = &entry.target else {
        panic!("original Flow entry");
    };
    let bytes = AwbcLowerer::for_entry(
        &compiled.plan,
        &compiled.dialogue_content,
        "string-operations.arcw",
        &entry.id,
    )
    .lower()
    .unwrap()
    .program
    .encode_canonical()
    .unwrap();
    let fresh = || {
        let decoded = AwbcProgram::decode_canonical(&bytes, Default::default()).unwrap();
        let admitted = AwbcProductProgram::admit(
            Arc::new(decoded),
            Default::default(),
            Default::default(),
            Default::default(),
            None,
        )
        .unwrap();
        ArcweftRuntimeExecutor::from_admitted_awbc_product_flow_invocation(
            admitted,
            compiled
                .plan
                .clone()
                .seal_flow_invocation(flow.clone(), [])
                .unwrap(),
        )
        .unwrap()
    };
    let mut runtime = fresh();
    let mut public_saves = 0;
    for _ in 0..256 {
        let result = runtime.step(
            RuntimeStepInput::default(),
            RuntimeStepOptions {
                mode: RuntimeStepMode::OneOp,
                budget: RuntimeStepBudget { max_ops: 1 },
                ..Default::default()
            },
        );
        assert!(result.output.diagnostics.is_empty(), "{:?}", result.output);
        assert!(result.stats.executed_ops <= 1);
        let ArcweftRuntimeExecutorSnapshot::AwbcProduct(snapshot) = runtime.snapshot().unwrap();
        let saved = AwbcProductExecutorSaveSnapshot::from_live(
            &snapshot,
            runtime.admitted_product_awbc_program().unwrap(),
        )
        .unwrap();
        let decoded = serde_json::from_slice(&serde_json::to_vec(&saved).unwrap()).unwrap();
        assert_eq!(saved, decoded);
        drop(runtime);
        let cold = fresh();
        let bound = cold.bind_product_save_snapshot(decoded).unwrap();
        runtime = cold
            .restore_snapshot(ArcweftRuntimeExecutorSnapshot::AwbcProduct(bound))
            .unwrap();
        public_saves += 1;
        if let FlowFiberStatus::Done(exit) = &runtime.fiber().frame.status {
            assert_eq!(exit, &FlowExit::Return("5".into()));
            assert!(public_saves > 1, "execution must cross saved frontiers");
            return;
        }
    }
    panic!("cold-restored String execution did not finish");
}

#[test]
fn string_scalar_count_rejects_wrong_receivers_arity_and_result_kind() {
    for source in [
        "flow main() -> usize { let value = 42i64; return value.scalar_count() }",
        "flow main() -> usize { let value = [1i64]; return value.scalar_count() }",
        "flow main() -> usize { return \"text\".scalar_count(1usize) }",
        "flow main() -> usize { return \"text\".scalar_count(extra = 1usize) }",
        "flow main() -> u64 { return \"text\".scalar_count() }",
        "flow main() -> usize { return \"text\".len() }",
    ] {
        let source = format!("entry cli @entry.main {{ goto @flow.main }}\n{source}");
        assert!(compile_source(&source).is_err(), "{source}");
    }
}
