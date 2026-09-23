use std::sync::Arc;

use arcweft_compiler::source::compile_source;
use arcweft_core::awbc::codec::AwbcDecodeBudget;
use arcweft_core::awbc::fiber::{AwbcFiberStateSnapshot, FiberState};
use arcweft_core::awbc::schema::{AwbcEntryId, AwbcProgram};
use arcweft_core::awbc::vm::{self, VmExit, VmStepOptions};
use arcweft_core::task::RuntimeProgramOwner;
use arcweft_core::value::RuntimeValue;
use arcweft_runtime_plan::awbc_lower::AwbcLowerer;

#[path = "support/execution.rs"]
mod execution;

fn assert_both(definitions: &str, body: &str, expected: &str) {
    let source = format!(
        "entry cli @entry.main {{ goto @flow.main }}\n{definitions}\nflow main() -> String {{ {body} }}"
    );
    execution::assert_native_return(&source, expected);
    execution::assert_awbc_return(&source, RuntimeValue::String(expected.to_owned()));
}

#[test]
fn nested_pure_scopes_join_their_own_success_type_before_callable_propagation() {
    let definitions = r#"
fn translate(input: Result<i64, String>) -> Result<String, String> {
    let number = scope outer {
        let item = scope inner { try input }
        item + 1i64
    }
    if number == 42i64 { Ok("done") } else { Ok("wrong") }
}
"#;
    assert_both(
        definitions,
        "return if let Ok(text) = translate(Ok(41i64)) { text } else { \"unexpected\" }",
        "done",
    );
    assert_both(
        definitions,
        "return if let Err(text) = translate(Err(\"stopped\")) { text } else { \"unexpected\" }",
        "stopped",
    );
}

#[test]
fn an_inner_carrier_remains_inside_the_named_pure_scope() {
    let definitions = r#"
fn settle(input: Option<i64>) -> String {
    scope outer {
        let caught = option { try input }
        if let Some(value) = caught { "present" } else { "empty" }
    }
}
"#;
    assert_both(definitions, "return settle(Some(4i64))", "present");
    assert_both(definitions, "return settle(None)", "empty");
}

fn flow_source(input: &str) -> String {
    format!(
        r#"
entry cli @entry.main {{ goto @flow.main }}
flow main() -> String {{
    let input: Result<i64, String> = {input}
    let outcome = result {{
        let number = scope outer {{ let value = try input
            value + 1i64 }}
        if number == 42i64 {{ "done" }} else {{ "wrong" }}
    }}
    return if let Ok(text) = outcome {{ text }} else {{ "stopped" }}
}}
"#
    )
}

#[test]
fn flow_scope_propagates_to_its_outer_carrier_after_leaving_the_frame() {
    for (input, expected) in [("Ok(41i64)", "done"), ("Err(\"stop\")", "stopped")] {
        let source = flow_source(input);
        execution::assert_native_return(&source, expected);
        execution::assert_awbc_return(&source, RuntimeValue::String(expected.to_owned()));
    }
}

#[test]
fn anonymous_statement_scope_propagation_skips_the_outer_success_continuation() {
    for (input, expected) in [("Some(1i64)", "done"), ("None", "empty")] {
        let body = format!(
            r#"
    let input: Option<i64> = {input}
    let outcome = option {{
        scope {{ let ignored = try input }}
        "done"
    }}
    return if let Some(text) = outcome {{ text }} else {{ "empty" }}
"#
        );
        assert_both("", &body, expected);
    }
}

#[test]
fn pure_scope_statements_join_before_the_remaining_function_body() {
    let definitions = r#"
fn finish(input: Option<i64>) -> Option<String> {
    scope named { let ignored = scope { try input } }
    Some("done")
}
"#;
    assert_both(
        definitions,
        "return if let Some(text) = finish(Some(1i64)) { text } else { \"empty\" }",
        "done",
    );
    assert_both(
        definitions,
        "return if let Some(text) = finish(None) { text } else { \"empty\" }",
        "empty",
    );
}

#[test]
fn nested_flow_scopes_unwind_only_the_frames_crossed_by_the_residual() {
    for (input, expected) in [("Some(41i64)", "done"), ("None", "empty")] {
        let body = format!(
            r#"
    let input: Option<i64> = {input}
    let outcome = option {{
        scope outer {{
            let number = scope inner {{ try input }}
            let ignored = number + 1i64
        }}
        "done"
    }}
    return if let Some(text) = outcome {{ text }} else {{ "empty" }}
"#
        );
        assert_both("", &body, expected);
    }
}

#[test]
fn scope_carrier_slots_restore_at_each_active_frame_instruction() {
    for (input, expected) in [("Ok(41i64)", "done"), ("Err(\"stop\")", "stopped")] {
        let compiled = compile_source(&flow_source(input)).expect("scope carrier compiles");
        let report = AwbcLowerer::new(&compiled.plan, &compiled.dialogue_content, "scope-try.arcw")
            .lower()
            .expect("scope carrier lowers");
        let bytes = report.program.encode_canonical().unwrap();
        let program =
            Arc::new(AwbcProgram::decode_canonical(&bytes, AwbcDecodeBudget::default()).unwrap());
        let owner = RuntimeProgramOwner::Awbc(Arc::clone(&program));
        let mut fiber = FiberState::for_entry(&program, AwbcEntryId(0), 1, 65_536).unwrap();
        let mut restored = 0;
        let mut returned = false;
        for _ in 0..256 {
            let output = vm::step(
                &program,
                &mut fiber,
                VmStepOptions {
                    max_instructions: 1,
                },
            )
            .unwrap();
            if let VmExit::Returned(value) = output.exit {
                assert_eq!(value, Some(RuntimeValue::String(expected.to_owned())));
                returned = true;
                break;
            }
            assert!(
                matches!(output.exit, VmExit::Running | VmExit::BudgetYield(_)),
                "{:?}",
                output.exit
            );
            if fiber
                .active_frame()
                .is_ok_and(|frame| !frame.scopes.is_empty())
            {
                let snapshot = AwbcFiberStateSnapshot::from_live(&fiber).unwrap();
                fiber = snapshot.into_live_for_program(&owner).unwrap();
                restored += 1;
            }
        }
        assert!(returned, "scope carrier exceeded its step bound");
        assert!(
            restored > 1,
            "saved state must cover the live scope computation"
        );
    }
}
