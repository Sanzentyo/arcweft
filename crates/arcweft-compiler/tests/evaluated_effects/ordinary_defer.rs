use super::*;

const FLOW_ACTOR_DEFER_SOURCE: &str = r#"
pub character alice { display = "Alice" }
flow main() -> Unit effects { log.write } {
    let actor = alice()[Outer[p]] with {
        let actor = alice.stage.acquire(scope=line)
        out actor
    }
    defer { let _ = actor; log.info("cleanup") }
    log.info("body")
}
entry cli @entry.main { goto @flow.main }
"#;

#[test]
fn native_ordinary_flow_defer_keeps_returned_line_actor_until_its_cleanup() {
    native_line_handle_execution(
        FLOW_ACTOR_DEFER_SOURCE,
        1,
        false,
        false,
        &["body", "cleanup"],
    );
}

#[test]
fn native_ordinary_flow_defer_keeps_an_actor_inside_an_aggregate() {
    let source = FLOW_ACTOR_DEFER_SOURCE.replace(
        "defer { let _ = actor; log.info(\"cleanup\") }",
        "let captured = (actor, 7); defer { let _ = captured; log.info(\"cleanup\") }",
    );
    native_line_handle_execution(&source, 1, false, false, &["body", "cleanup"]);
}

#[test]
fn native_ordinary_flow_defer_releases_an_actor_when_its_filter_skips_cleanup() {
    let source = FLOW_ACTOR_DEFER_SOURCE.replace("defer {", "defer on failed {");
    native_line_handle_execution(&source, 1, false, false, &["body"]);
}

#[test]
fn native_ordinary_flow_defer_reexports_an_actor_through_its_nested_line() {
    let source = FLOW_ACTOR_DEFER_SOURCE.replace(
        "defer { let _ = actor; log.info(\"cleanup\") }",
        r#"defer {
            let received = alice()[Cleanup[p]] with { out actor }
            defer { let _ = received; log.info("nested cleanup") }
            log.info("cleanup")
        }"#,
    );
    native_line_handle_execution(
        &source,
        2,
        false,
        false,
        &["body", "cleanup", "nested cleanup"],
    );
}

#[test]
fn saved_awbc_ordinary_flow_defer_keeps_returned_line_actor_until_its_cleanup() {
    assert_saved_awbc(FLOW_ACTOR_DEFER_SOURCE, &["body", "cleanup"]);
}

#[test]
fn line_result_direct_acquisition_uses_its_owning_line_plan() {
    let source = r#"
pub character alice { display = "Alice" }
flow main() -> Unit effects { log.write } {
    let _ = alice()[Outer[p]] with {
        out alice.stage.acquire(scope=line)
    }
    log.info("body")
}
entry cli @entry.main { goto @flow.main }
"#;
    native_line_handle_execution(source, 1, false, false, &["body"]);
    assert_saved_awbc(source, &["body"]);
}

#[test]
fn line_result_effectful_call_keeps_its_value_continuation_and_source_order() {
    let source = r#"
pub character alice { display = "Alice" }
fn result_text() -> String effects { log.write } {
    log.info("value")
    return "result"
}
flow main() -> Unit effects { log.write } {
    let _ = alice()[Outer[p]] with {
        let _ = alice.stage.acquire(scope=line)
        out result_text()
    }
    log.info("body")
}
entry cli @entry.main { goto @flow.main }
"#;
    native_line_handle_execution(source, 1, false, false, &["value", "body"]);
    assert_saved_awbc(source, &["value", "body"]);
}

fn assert_saved_awbc(source: &str, expected: &[&str]) {
    let (compiled, awbc) = nested_awbc_from_compiled(
        with_attached_dialogue_project_and_visual(
            source,
            Some(LINE_HANDLE_CHARACTER_MANIFEST),
            compile_project,
        )
        .unwrap(),
    );
    let schema = bind_test_character_dialogue_schema(&compiled, awbc.program_owner());
    let mut backend = arcweft_core::pure::VmRuntimePureCallBackend::default().with_external_calls(
        arcweft_dialogue::CharacterDialogueRuntimeExternalCallBackend::new(&schema),
    );
    let mut awbc = Some(awbc);
    execute_nested_line_handle_transfer(1, false, false, expected, |input, options| {
        let mut current = awbc.take().unwrap();
        let result = current.step_with_pure_backend(input, options, &mut backend);
        awbc = Some(roundtrip_nested_awbc(current).0);
        result
    });
}
