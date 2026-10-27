use super::*;

fn assert_labelled_loop_return(body: &str) {
    let source =
        format!("flow main() -> i64 {{ {body} }}\nentry cli @entry.main {{ goto @flow.main }}\n");
    assert_native_return(&source, "42");
    assert_awbc_return(&source, RuntimeValue::i64(42));
    // Loop intrinsics run through the actual Product host, including its
    // owned iterator state, rather than the dialogue-only scalar test host.
    let compiled = compile_source(&source).expect("loop source compiles");
    let lowered = AwbcLowerer::new(
        &compiled.plan,
        &compiled.dialogue_content,
        "loop-control.arcw",
    )
    .lower()
    .expect("loop source lowers to verified AWBC");
    let encoded = lowered.program.encode_canonical().unwrap();
    let program = arcweft_core::awbc::schema::AwbcProgram::decode_canonical(
        &encoded,
        arcweft_core::awbc::codec::AwbcDecodeBudget::default(),
    )
    .unwrap();
    assert_eq!(program, lowered.program);
    let mut executor = arcweft_core::awbc::product_step::AwbcProductStepExecutor::for_entry_arc(
        Arc::new(program),
        arcweft_core::awbc::schema::AwbcEntryId(0),
        1,
    )
    .unwrap();
    let mut backend = arcweft_core::pure::VmRuntimePureCallBackend::default();
    for _ in 0..256 {
        let step =
            executor.step_with_pure_backend(Default::default(), Default::default(), &mut backend);
        assert!(step.stats.executed_ops <= 1);
        assert!(
            step.output.diagnostics.is_empty(),
            "{:?}",
            step.output.diagnostics
        );
        assert!(step.output.requests.tasks.is_empty());
        let snapshot = executor.inert_rollback_image().unwrap();
        executor = executor
            .restore_rollback_image(snapshot.clone())
            .unwrap_or_else(|(_, error)| panic!("loop Product rollback: {error}"));
        assert_eq!(executor.inert_rollback_image().unwrap(), snapshot);
        match &executor.fiber().frame.status {
            arcweft_core::engine::FlowFiberStatus::Running => {}
            arcweft_core::engine::FlowFiberStatus::Done(exit) => {
                assert_eq!(
                    exit,
                    &arcweft_core::engine::FlowExit::Return("42".to_owned())
                );
                // Product has delivered the owned value to its Flow result
                // consumer. The typed VM result is checked separately above.
                assert!(matches!(
                    executor.compact_fiber().terminal,
                    Some(arcweft_core::awbc::fiber::FiberTerminalValue::ReturnedTransferred)
                ));
                assert_eq!(
                    executor.compact_fiber().return_summary.as_deref(),
                    Some("42")
                );
                return;
            }
            status => panic!("loop Product stopped unexpectedly: {status:?}"),
        }
    }
    panic!("loop Product exceeded the unchanged deterministic step limit");
}

#[test]
fn labelled_loop_break_crosses_nested_value_loop() {
    assert_labelled_loop_return(
        "let value = 'outer: loop { loop { break 'outer 42i64 } }; return value",
    );
}

#[test]
fn labelled_loop_continue_crosses_nested_value_loop() {
    assert_labelled_loop_return(
        "let mut count = 0i64; let value = 'outer: loop { count = count + 1i64; loop { if count < 3i64 { continue 'outer }; break 'outer 42i64 } }; return value",
    );
}

#[test]
fn labelled_loop_shadowing_selects_the_nearest_same_name() {
    assert_labelled_loop_return(
        "let value = 'same: loop { let inner = 'same: loop { break 'same 21i64 }; break 'same inner + 21i64 }; return value",
    );
}

#[test]
fn labelled_loop_break_crosses_statement_while() {
    assert_labelled_loop_return(
        "let value = 'outer: loop { while true { break 'outer 42i64 } }; return value",
    );
}

#[test]
fn labelled_loop_break_crosses_statement_while_let() {
    assert_labelled_loop_return(
        "let value = 'outer: loop { while let .Some(item) = Some(42i64) { break 'outer item } }; return value",
    );
}

#[test]
fn labelled_loop_enclosing_while_preserves_unlabelled_break_and_continue() {
    assert_labelled_loop_return(
        "let mut count = 0i64; let value = 'outer: loop { while count < 3i64 { count = count + 1i64; if count < 3i64 { continue }; break }; break 'outer count + 39i64 }; return value",
    );
}

#[test]
fn labelled_loop_break_crosses_statement_for() {
    assert_labelled_loop_return(
        "let value = 'outer: loop { for item in [1i64] { break 'outer item + 41i64 } }; return value",
    );
}

#[test]
fn labelled_loop_continue_crosses_statement_for() {
    assert_labelled_loop_return(
        "let mut count = 0i64; let value = 'outer: loop { count = count + 1i64; for item in [1i64] { if count < 3i64 { continue 'outer }; break 'outer item + 41i64 } }; return value",
    );
}

#[test]
fn labelled_loop_enclosing_iterator_preserves_unlabelled_break_and_continue() {
    assert_labelled_loop_return(
        "let value = 'outer: loop { for item in [1i64, 2i64, 3i64] { if item == 1i64 { continue }; break }; break 'outer 42i64 }; return value",
    );
}

#[test]
fn control_branch_context_runs_both_orders_in_native_and_decoded_awbc() {
    for expression in [
        "if flag { [] } else { values }",
        "if flag { values } else { [] }",
        "if let .Some(items) = maybe { [] } else { values }",
        "if let .Some(items) = maybe { items } else { [] }",
        "match flag { true => [], false => values }",
        "match flag { true => values, false => [] }",
        "match flag { _ when flag => [], _ => values }",
        "if flag { match flag { true => [], false => values } } else { values }",
    ] {
        let source = format!(
            r"
fn joined(flag: bool, values: Vec<i64>, maybe: Option<Vec<i64>>) -> i64 {{
    let selected = {expression}
    match selected {{ [] => 0i64, [head, ..] => head }}
}}
flow main() -> i64 {{
    return joined(true, [42i64], Some([42i64])) + joined(false, [42i64], None)
}}
entry cli @entry.main {{ goto @flow.main }}
"
        );
        assert_native_return(&source, "42");
        assert_awbc_return(&source, RuntimeValue::i64(42));
    }
}

#[test]
fn control_branch_generic_capture_runs_native_and_decoded_awbc() {
    const SOURCE: &str = r"
fn identity<T>(value: T) -> T { value }
fn joined<T>(flag: bool, values: Vec<T>) -> Vec<T> {
    let choose = |condition: bool| match condition {
        true => identity([])
        false => values
    }
    choose(flag)
}
fn head(values: Vec<i64>) -> i64 {
    match values { [] => 0i64, [value, ..] => value }
}
flow main() -> i64 {
    return head(joined(false, [42i64])) + head(joined(true, [21i64]))
}
entry cli @entry.main { goto @flow.main }
";
    assert_native_return(SOURCE, "42");
    assert_awbc_return(SOURCE, RuntimeValue::i64(42));
}
