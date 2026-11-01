use super::presentation_factories::{program_from_source, run_selected};
use arcweft_core::value::RuntimeValue;

fn run_view_value(native: bool, constructor: &str) {
    let source = format!(
        r#"
pub view Card(caption: String, enabled: bool = true) {{
    Text(caption)
}}
pub fn root() -> bool {{
    let handle = mount({constructor});
    handle.unmount();
    true
}}
flow main() -> i64 {{ return 0i64 }}
"#
    );
    run_selected(program_from_source(&source), native, |value| {
        assert_eq!(value, &RuntimeValue::Bool(true));
    });
}

#[test]
fn source_view_value_mounts_directly_in_native() {
    run_view_value(true, r#"Card("owned", enabled = false)"#);
}

#[test]
fn source_view_value_mounts_directly_in_decoded_awbc() {
    run_view_value(false, r#"Card("owned", enabled = false)"#);
}

#[test]
fn source_view_value_mounts_through_a_local_alias_in_native() {
    run_view_value(
        true,
        r#"{ let selected = Card; selected("owned", enabled = false) }"#,
    );
}

#[test]
fn source_view_value_mounts_through_a_local_alias_in_decoded_awbc() {
    run_view_value(
        false,
        r#"{ let selected = Card; selected("owned", enabled = false) }"#,
    );
}

#[test]
fn source_view_value_mounts_with_an_omitted_default_in_native() {
    run_view_value(true, r#"Card("owned")"#);
}

#[test]
fn source_view_value_mounts_with_an_omitted_default_in_decoded_awbc() {
    run_view_value(false, r#"Card("owned")"#);
}

fn supplied_arguments_follow_source_order(native: bool) {
    let source = r#"
pub view Card(caption: String, enabled: bool = true) { Text(caption) }
pub fn root() -> i64 {
    let mut order = 0i64;
    let value = Card(enabled = scope { order = order * 10i64 + 1i64; false },
                     caption = scope { order = order * 10i64 + 2i64; "owned" });
    let handle = mount(value);
    handle.unmount();
    order
}
flow main() -> i64 { return 0i64 }
"#;
    run_selected(program_from_source(source), native, |value| {
        assert_eq!(
            value,
            &RuntimeValue::Int(arcweft_core::value::RuntimeInt::I64(12))
        )
    });
}

#[test]
fn source_view_value_named_captures_evaluate_once_in_source_order_in_native() {
    supplied_arguments_follow_source_order(true);
}

#[test]
fn source_view_value_named_captures_evaluate_once_in_source_order_in_decoded_awbc() {
    supplied_arguments_follow_source_order(false);
}

fn semantic_callee_prefix(native: bool, statements: &str) {
    let source = format!(
        r#"
pub view Card(caption: String, enabled: bool = true) {{ Text(caption) }}
pub fn root() -> i64 {{
    let mut order = 0i64;
    {statements}
    let handle = mount(value);
    handle.unmount();
    order
}}
flow main() -> i64 {{ return 0i64 }}
"#
    );
    run_selected(program_from_source(&source), native, |value| {
        assert_eq!(
            value,
            &RuntimeValue::Int(arcweft_core::value::RuntimeInt::I64(12))
        )
    });
}

#[test]
fn source_view_alias_keeps_initializer_prefix_effects_in_native() {
    semantic_callee_prefix(
        true,
        r#"let selected = { let digit = 1i64; order = order * 10i64 + digit; Card };
        let value = selected(caption = scope { order = order * 10i64 + 2i64; "owned" });"#,
    );
}

#[test]
fn source_view_alias_keeps_initializer_prefix_effects_in_decoded_awbc() {
    semantic_callee_prefix(
        false,
        r#"let selected = { let digit = 1i64; order = order * 10i64 + digit; Card };
        let value = selected(caption = scope { order = order * 10i64 + 2i64; "owned" });"#,
    );
}

#[test]
fn source_view_computed_callee_keeps_prefix_effects_in_native() {
    semantic_callee_prefix(
        true,
        r#"let value = ({ let digit = 1i64; order = order * 10i64 + digit; Card })(
        caption = scope { order = order * 10i64 + 2i64; "owned" });"#,
    );
}

#[test]
fn source_view_computed_callee_keeps_prefix_effects_in_decoded_awbc() {
    semantic_callee_prefix(
        false,
        r#"let value = ({ let digit = 1i64; order = order * 10i64 + digit; Card })(
        caption = scope { order = order * 10i64 + 2i64; "owned" });"#,
    );
}

fn run_flow(native: bool, body: &str, expected: &str) {
    use arcweft_core::{
        awbc::{
            codec::AwbcDecodeBudget,
            product_step::AwbcProductStepExecutor,
            schema::{AwbcEntryId, AwbcProgram},
        },
        engine::{Engine, FlowExit, FlowFiberStatus},
        step::{RuntimeStepBudget, RuntimeStepMode, RuntimeStepOptions, RuntimeStepResult},
    };
    use std::sync::Arc;

    let source = format!(
        r#"
pub view Card(caption: String) {{ Text(caption) }}
flow main() -> i64 {{ {body} }}
entry cli @entry.main {{ goto @flow.main }}
"#
    );
    let compiled = super::compile_source(&source)
        .expect("semantic View aliases compile in their original Flow");
    let code =
        super::AwbcLowerer::new(&compiled.plan, &compiled.dialogue_content, "view-flow.arcw")
            .lower()
            .unwrap()
            .program;
    let bytes = code.encode_canonical().unwrap();
    let decoded = AwbcProgram::decode_canonical(&bytes, AwbcDecodeBudget::default()).unwrap();
    assert_eq!(decoded, code);
    let options = RuntimeStepOptions {
        mode: RuntimeStepMode::OneOp,
        budget: RuntimeStepBudget { max_ops: 1 },
        ..Default::default()
    };
    let drive = |mut step: Box<dyn FnMut() -> RuntimeStepResult + '_>| {
        for _ in 0..256 {
            let result = step();
            assert!(result.stats.executed_ops <= 1);
            assert!(
                result.output.diagnostics.is_empty(),
                "{:?}",
                result.output.diagnostics
            );
            assert!(result.output.requests.tasks.is_empty());
            if let FlowFiberStatus::Done(exit) = result.fiber_status {
                assert_eq!(exit, FlowExit::Return(expected.to_owned()));
                return;
            }
            assert!(
                !matches!(result.fiber_status, FlowFiberStatus::Failed(_)),
                "{:?}",
                result.fiber_status
            );
        }
        panic!("View Flow exceeded its deterministic step limit");
    };
    if native {
        let mut plan = compiled.plan;
        plan.bind_artifact(
            arcweft_core::effect::RuntimeArtifactFingerprint::try_from_bytes(
                *blake3::hash(&bytes).as_bytes(),
            )
            .unwrap(),
        )
        .unwrap();
        let [flow] = plan.flows() else {
            panic!("one original Flow")
        };
        let flow = flow.id.clone();
        let mut engine = Engine::for_flow(plan, &flow).unwrap();
        drive(Box::new(|| engine.step(Default::default(), options)));
    } else {
        let mut executor = Some(
            AwbcProductStepExecutor::for_entry_arc(Arc::new(decoded), AwbcEntryId(0), 1).unwrap(),
        );
        let mut backend = arcweft_core::pure::VmRuntimePureCallBackend::default();
        drive(Box::new(|| {
            let mut current = executor.take().unwrap();
            let result = current.step_with_pure_backend(Default::default(), options, &mut backend);
            let snapshot = current
                .snapshot()
                .expect("public save retains the original Flow and View owner");
            executor = Some(
                current
                    .restore_snapshot_owned(snapshot)
                    .unwrap_or_else(|(_, error)| panic!("owned restore: {error}")),
            );
            result
        }));
    }
}

const FLOW_ALIAS: &str = r#"
    let selected = Card;
    let handle = mount(selected("owned"));
    handle.unmount();
    return 42i64
"#;

const FLOW_ALIAS_PREFIX: &str = r#"
    let mut order = 0i64;
    let selected = scope { order = order * 10i64 + 1i64; Card };
    let handle = mount(selected(caption = scope { order = order * 10i64 + 2i64; "owned" }));
    handle.unmount();
    return order
"#;

const FLOW_CALLEE_PREFIX: &str = r#"
    let mut order = 0i64;
    let handle = mount((scope { order = order * 10i64 + 1i64; Card })(caption = scope { order = order * 10i64 + 2i64; "owned" }));
    handle.unmount();
    return order
"#;

#[test]
fn source_view_alias_executes_in_its_original_native_flow() {
    run_flow(true, FLOW_ALIAS, "42");
}

#[test]
fn source_view_alias_executes_in_its_original_decoded_awbc_flow() {
    run_flow(false, FLOW_ALIAS, "42");
}

#[test]
fn source_view_alias_keeps_scoped_prefix_effects_in_native_flow() {
    run_flow(true, FLOW_ALIAS_PREFIX, "12");
}

#[test]
fn source_view_alias_keeps_scoped_prefix_effects_in_decoded_awbc_flow() {
    run_flow(false, FLOW_ALIAS_PREFIX, "12");
}

#[test]
fn source_view_computed_callee_keeps_scoped_prefix_effects_in_native_flow() {
    run_flow(true, FLOW_CALLEE_PREFIX, "12");
}

#[test]
fn source_view_computed_callee_keeps_scoped_prefix_effects_in_decoded_awbc_flow() {
    run_flow(false, FLOW_CALLEE_PREFIX, "12");
}

fn run_flow_try_prefix(native: bool, computed: bool, success: bool) {
    let input = if success { "Some(2i64)" } else { "None" };
    let prefix = "scope { let digit = try input; order = order * 10i64 + digit; Card }";
    let call = if computed {
        format!("({prefix})(caption = scope {{ order = order * 10i64 + 3i64; \"owned\" }})")
    } else {
        "selected(caption = scope { order = order * 10i64 + 3i64; \"owned\" })".to_owned()
    };
    let binding = if computed {
        String::new()
    } else {
        format!("let selected = {prefix};")
    };
    let body = format!(
        r#"
        let input: Option<i64> = {input};
        let mut order = 1i64;
        let outcome = option {{
            {binding}
            let handle = mount({call});
            handle.unmount();
            order
        }};
        return if let Some(value) = outcome {{ value }} else {{ order }}
    "#
    );
    run_flow(native, &body, if success { "123" } else { "1" });
}

#[test]
fn source_view_alias_prefix_try_succeeds_in_native_flow() {
    run_flow_try_prefix(true, false, true);
}

#[test]
fn source_view_alias_prefix_try_succeeds_in_decoded_awbc_flow() {
    run_flow_try_prefix(false, false, true);
}

#[test]
fn source_view_alias_prefix_try_propagates_in_native_flow() {
    run_flow_try_prefix(true, false, false);
}

#[test]
fn source_view_alias_prefix_try_propagates_in_decoded_awbc_flow() {
    run_flow_try_prefix(false, false, false);
}

#[test]
fn source_view_computed_callee_prefix_try_succeeds_in_native_flow() {
    run_flow_try_prefix(true, true, true);
}

#[test]
fn source_view_computed_callee_prefix_try_succeeds_in_decoded_awbc_flow() {
    run_flow_try_prefix(false, true, true);
}

#[test]
fn source_view_computed_callee_prefix_try_propagates_in_native_flow() {
    run_flow_try_prefix(true, true, false);
}

#[test]
fn source_view_computed_callee_prefix_try_propagates_in_decoded_awbc_flow() {
    run_flow_try_prefix(false, true, false);
}

const FLOW_ESCAPING_VIEW: &str = r#"
    let value = scope {
        let caption = "owned";
        let selected = Card;
        selected(caption)
    };
    let handle = mount(value);
    handle.unmount();
    return 42i64
"#;

#[test]
fn source_view_value_leaves_its_construction_scope_before_mount_in_native_flow() {
    run_flow(true, FLOW_ESCAPING_VIEW, "42");
}

#[test]
fn source_view_value_leaves_its_construction_scope_before_mount_in_decoded_awbc_flow() {
    run_flow(false, FLOW_ESCAPING_VIEW, "42");
}
