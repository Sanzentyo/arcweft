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
fn native_nested_deferred_line_uses_the_innermost_capture_frame() {
    native_line_handle_execution(
        &nested_deferred_line_source(),
        2,
        false,
        false,
        &["body", "cleanup", "captured"],
    );
}

#[test]
fn saved_awbc_nested_deferred_line_uses_the_innermost_capture_frame() {
    assert_saved_awbc_with_reveals(
        &nested_deferred_line_source(),
        2,
        &["body", "cleanup", "captured"],
    );
}

fn nested_deferred_line_source() -> String {
    FLOW_ACTOR_DEFER_SOURCE.replace(
        "defer { let _ = actor; log.info(\"cleanup\") }",
        r#"let _ = actor
        let message = "captured"
        defer {
            defer {
                let received = alice()[Cleanup[p]] with { out message }
                log.info(received)
            }
            log.info("cleanup")
        }"#,
    )
}

#[test]
fn native_generic_deferred_line_uses_each_closed_capture_frame() {
    native_line_handle_execution(
        &generic_deferred_line_source(),
        3,
        false,
        false,
        &["number", "text", "body"],
    );
}

#[test]
fn saved_awbc_generic_deferred_line_uses_each_closed_capture_frame() {
    assert_saved_awbc_with_reveals(
        &generic_deferred_line_source(),
        3,
        &["number", "text", "body"],
    );
}

fn generic_deferred_line_source() -> String {
    FLOW_ACTOR_DEFER_SOURCE
        .replace(
            "flow main()",
            r"fn cleanup<T>(value: T, message: String) -> Unit effects { log.write } {
                defer {
                    let _ = value
                    let received = alice()[Cleanup[p]] with { out message }
                    log.info(received)
                }
            }
            flow main()",
        )
        .replace(
            "defer { let _ = actor; log.info(\"cleanup\") }",
            r#"let _ = actor
            cleanup(7, "number")
            cleanup("value", "text")"#,
        )
}

#[test]
fn saved_awbc_ordinary_flow_defer_keeps_returned_line_actor_until_its_cleanup() {
    assert_saved_awbc(FLOW_ACTOR_DEFER_SOURCE, &["body", "cleanup"]);
}

#[test]
fn saved_awbc_ordinary_flow_defer_keeps_an_actor_inside_an_aggregate() {
    let source = FLOW_ACTOR_DEFER_SOURCE.replace(
        "defer { let _ = actor; log.info(\"cleanup\") }",
        "let captured = (actor, 7); defer { let _ = captured; log.info(\"cleanup\") }",
    );
    assert_saved_awbc(&source, &["body", "cleanup"]);
}

#[test]
fn saved_awbc_ordinary_flow_defer_releases_an_actor_when_its_filter_skips_cleanup() {
    let source = FLOW_ACTOR_DEFER_SOURCE.replace("defer {", "defer on failed {");
    assert_saved_awbc(&source, &["body"]);
}

#[test]
fn saved_awbc_ordinary_flow_defer_moves_an_actor_into_a_nested_registration() {
    let source = FLOW_ACTOR_DEFER_SOURCE.replace(
        "defer { let _ = actor; log.info(\"cleanup\") }",
        r#"defer {
            defer { let _ = actor; log.info("nested") }
            log.info("cleanup")
        }"#,
    );
    assert_saved_awbc(&source, &["body", "cleanup", "nested"]);
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one saved execution audits every packet and live-child custody coordinate before completing the original callback"
)]
fn saved_awbc_ordinary_flow_defer_rejects_forged_packet_and_child_slots() {
    let mut packet_seen = false;
    let mut child_seen = false;
    assert_saved_awbc_checking(
        FLOW_ACTOR_DEFER_SOURCE,
        1,
        &["body", "cleanup"],
        |mut executor, save| {
            for (entry_index, entry) in save["dialogues"]["entries"]
                .as_array()
                .unwrap()
                .iter()
                .enumerate()
            {
                let Some(leases) = entry
                    .pointer("/PublishedHandles/handles/ledger/leases")
                    .and_then(serde_json::Value::as_object)
                else {
                    continue;
                };
                for (key, lease) in leases {
                    let Some(slot) = lease.pointer("/owner/ParentFiber/slot") else {
                        continue;
                    };
                    let packet = slot["kind"] == "awbc_defer_capture";
                    let child = slot["kind"] == "awbc_register"
                        && slot["fiber"] != save["fiber"]["instance"];
                    if (packet && !packet_seen) || (child && !child_seen) {
                        for field in ["execution", "fiber", "frame"] {
                            let mut forged = save.clone();
                            let value = &mut forged["dialogues"]["entries"][entry_index]["PublishedHandles"]
                                ["handles"]["ledger"]["leases"][key]["owner"]["ParentFiber"]["slot"]
                                [field];
                            *value = (value.as_str().unwrap().parse::<u64>().unwrap() + 1)
                                .to_string()
                                .into();
                            executor = reject_capture_save(executor, forged);
                        }
                        if packet {
                            for field in ["registration", "ordinal", "scope"] {
                                let mut forged = save.clone();
                                let value = &mut forged["dialogues"]["entries"][entry_index]["PublishedHandles"]
                                    ["handles"]["ledger"]["leases"][key]["owner"]["ParentFiber"]["slot"]
                                    [field];
                                *value = match field {
                                    "registration" => {
                                        (value.as_str().unwrap().parse::<u64>().unwrap() + 1)
                                            .to_string()
                                            .into()
                                    }
                                    "ordinal" => (value.as_u64().unwrap() + 1).into(),
                                    "scope" if value.is_null() => 0.into(),
                                    "scope" => serde_json::Value::Null,
                                    _ => unreachable!(),
                                };
                                executor = reject_capture_save(executor, forged);
                            }
                            let mut forged = save.clone();
                            let frame = forged["fiber"]["frames"]
                                .as_array_mut()
                                .unwrap()
                                .iter_mut()
                                .find(|frame| frame["instance"] == slot["frame"])
                                .unwrap();
                            let deferred = if slot["scope"].is_null() {
                                &mut frame["root_deferred"]
                            } else {
                                &mut frame["scopes"]
                                    .as_array_mut()
                                    .unwrap()
                                    .iter_mut()
                                    .find(|scope| scope["id"] == slot["scope"])
                                    .unwrap()["deferred"]
                            };
                            let packets = deferred["defers"].as_array_mut().unwrap();
                            let index = packets
                                .iter()
                                .position(|packet| packet["id"] == slot["registration"])
                                .unwrap();
                            packets.remove(index);
                            executor = reject_capture_save(executor, forged);
                            packet_seen = true;
                        } else {
                            child_seen = true;
                        }
                    }
                }
            }
            executor
        },
    );
    assert!(packet_seen && child_seen);
}

fn reject_capture_save(
    executor: arcweft_core::executor::ArcweftRuntimeExecutor,
    forged: serde_json::Value,
) -> arcweft_core::executor::ArcweftRuntimeExecutor {
    use arcweft_core::executor::ArcweftRuntimeExecutorSnapshot;
    let saved = serde_json::from_value(forged).unwrap();
    let saved = executor.bind_product_save_snapshot(saved).unwrap();
    let Err(rejected) =
        executor.restore_snapshot(ArcweftRuntimeExecutorSnapshot::AwbcProduct(saved))
    else {
        panic!("the published lease must match its actual packet or child slot");
    };
    let (unchanged, error) = rejected.into_parts();
    assert!(
        error.to_string().contains("owned by another runtime slot"),
        "{error}"
    );
    unchanged
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
    assert_saved_awbc_with_reveals(source, 1, expected);
}

fn assert_saved_awbc_with_reveals(source: &str, reveals: usize, expected: &[&str]) {
    assert_saved_awbc_checking(source, reveals, expected, |executor, _| executor);
}

fn assert_saved_awbc_checking(
    source: &str,
    reveals: usize,
    expected: &[&str],
    mut check: impl FnMut(
        arcweft_core::executor::ArcweftRuntimeExecutor,
        &serde_json::Value,
    ) -> arcweft_core::executor::ArcweftRuntimeExecutor,
) {
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
    execute_nested_line_handle_transfer(reveals, false, false, expected, |input, options| {
        let mut current = awbc.take().unwrap();
        let result = current.step_with_pure_backend(input, options, &mut backend);
        awbc = Some(roundtrip_nested_awbc_checking(current, &mut check).0);
        result
    });
}
