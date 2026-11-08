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
    native_line_handle_execution(
        &owned_deferred_line_source(),
        2,
        false,
        false,
        &["body", "cleanup", "nested cleanup"],
    );
}

#[test]
fn saved_awbc_ordinary_flow_defer_reexports_an_actor_through_its_nested_line() {
    assert_saved_awbc_with_reveals(
        &owned_deferred_line_source(),
        2,
        &["body", "cleanup", "nested cleanup"],
    );
}

fn owned_deferred_line_source() -> String {
    FLOW_ACTOR_DEFER_SOURCE.replace(
        "defer { let _ = actor; log.info(\"cleanup\") }",
        r#"defer {
            let received = alice()[Cleanup[p]] with { out actor }
            defer { let _ = received; log.info("nested cleanup") }
            log.info("cleanup")
        }"#,
    )
}

fn copied_line_input_source() -> String {
    FLOW_ACTOR_DEFER_SOURCE.replace(
        "defer { let _ = actor; log.info(\"cleanup\") }",
        r#"let message = "kept"
        let received = alice()[Copy[p]] with { out message }
        log.info(message)
        log.info(received)
        defer { let _ = actor; log.info("cleanup") }"#,
    )
}

#[test]
fn native_initial_copy_line_input_remains_available_in_its_caller() {
    native_line_handle_execution(
        &copied_line_input_source(),
        2,
        false,
        false,
        &["kept", "kept", "body", "cleanup"],
    );
}

#[test]
fn saved_awbc_initial_copy_line_input_remains_available_in_its_caller() {
    assert_saved_awbc_with_reveals(
        &copied_line_input_source(),
        2,
        &["kept", "kept", "body", "cleanup"],
    );
}

#[test]
fn native_initial_owned_line_input_moves_into_the_line_root_defer() {
    for cancel in [false, true] {
        native_line_handle_execution(
            &initial_owned_line_root_defer_source(),
            2,
            false,
            cancel,
            &["body", "line cleanup", "cleanup"],
        );
    }
}

#[test]
fn saved_awbc_initial_owned_line_input_moves_into_the_line_root_defer() {
    for cancel in [false, true] {
        with_saved_awbc_steps(&initial_owned_line_root_defer_source(), |step| {
            execute_nested_line_handle_transfer(
                2,
                false,
                cancel,
                &["body", "line cleanup", "cleanup"],
                step,
            );
        });
    }
}

fn initial_owned_line_root_defer_source() -> String {
    FLOW_ACTOR_DEFER_SOURCE.replace(
        "defer { let _ = actor; log.info(\"cleanup\") }",
        r#"defer {
            let _ = alice()[Cleanup[mark @.later][p]] with {
                defer { let _ = actor; log.info("line cleanup") }
                cancel on input(.SkipLine) { out () }
            }
            log.info("cleanup")
        }"#,
    )
}

#[test]
fn native_initial_owned_line_root_defer_preserves_outcome_filters() {
    for outcome in ["completed", "cancelled"] {
        for cancel in [false, true] {
            let runs = (outcome == "cancelled") == cancel;
            native_line_handle_execution(
                &filtered_initial_owned_line_root_defer_source(outcome),
                2,
                false,
                cancel,
                if runs {
                    &["body", "line cleanup", "cleanup"]
                } else {
                    &["body", "cleanup"]
                },
            );
        }
    }
}

#[test]
fn saved_awbc_initial_owned_line_root_defer_preserves_outcome_filters() {
    for outcome in ["completed", "cancelled"] {
        for cancel in [false, true] {
            let runs = (outcome == "cancelled") == cancel;
            with_saved_awbc_steps(
                &filtered_initial_owned_line_root_defer_source(outcome),
                |step| {
                    execute_nested_line_handle_transfer(
                        2,
                        false,
                        cancel,
                        if runs {
                            &["body", "line cleanup", "cleanup"]
                        } else {
                            &["body", "cleanup"]
                        },
                        step,
                    );
                },
            );
        }
    }
}

fn filtered_initial_owned_line_root_defer_source(outcome: &str) -> String {
    initial_owned_line_root_defer_source().replace(
        "defer { let _ = actor; log.info(\"line cleanup\") }",
        &format!("defer on {outcome} {{ let _ = actor; log.info(\"line cleanup\") }}"),
    )
}

#[test]
fn saved_awbc_initial_line_root_defer_rejects_forged_capture_custody() {
    use arcweft_core::executor::ArcweftRuntimeExecutorSnapshot;
    let mut checked = false;
    assert_saved_awbc_checking(
        &initial_owned_line_root_defer_source(),
        2,
        &["body", "line cleanup", "cleanup"],
        |mut executor, save| {
            if checked {
                return executor;
            }
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
                    let Some(slot) = lease
                        .pointer("/owner/ParentFiber/slot")
                        .filter(|slot| slot["kind"] == "line_defer_capture")
                    else {
                        continue;
                    };
                    let before = executor.snapshot().unwrap();
                    let mut mutations = Vec::new();
                    for field in [
                        "execution",
                        "registration",
                        "ordinal",
                        "activation/owner_fiber",
                        "activation/content",
                        "activation/occurrence",
                    ] {
                        let mut forged = save.clone();
                        let value = forged["dialogues"]["entries"][entry_index]["PublishedHandles"]
                            ["handles"]["ledger"]["leases"][key]["owner"]["ParentFiber"]["slot"]
                            .pointer_mut(&format!("/{field}"))
                            .unwrap();
                        *value = if let Some(value) = value.as_str() {
                            (value.parse::<u64>().unwrap() + 1).to_string().into()
                        } else {
                            (value.as_u64().unwrap() + 1).into()
                        };
                        mutations.push(forged);
                    }
                    for duplicate in [false, true] {
                        let mut forged = save.clone();
                        let receiver = forged["dialogues"]["entries"]
                            .as_array_mut()
                            .unwrap()
                            .iter_mut()
                            .find(|entry| {
                                entry.pointer("/Active/activation") == Some(&slot["activation"])
                            })
                            .unwrap();
                        let packets = receiver["Active"]["line"]["deferred"]
                            .as_array_mut()
                            .unwrap();
                        let index = packets
                            .iter()
                            .position(|packet| packet["id"] == slot["registration"])
                            .unwrap();
                        if duplicate {
                            packets.push(packets[index].clone());
                        } else {
                            packets.remove(index);
                        }
                        mutations.push(forged);
                    }
                    for forged in mutations {
                        let forged = serde_json::from_value(forged).unwrap();
                        let error = match executor.bind_product_save_snapshot(forged) {
                            Err(error) => error.to_string(),
                            Ok(bound) => {
                                let Err(rejected) = executor.restore_snapshot(
                                    ArcweftRuntimeExecutorSnapshot::AwbcProduct(bound),
                                ) else {
                                    panic!("forged defer custody must be refused");
                                };
                                let (unchanged, error) = rejected.into_parts();
                                executor = unchanged;
                                error.to_string()
                            }
                        };
                        assert!(
                            error.contains("deferred")
                                || error.contains("owned by another runtime slot"),
                            "{error}"
                        );
                        assert_eq!(executor.snapshot().unwrap(), before);
                    }
                    checked = true;
                    return executor;
                }
            }
            executor
        },
    );
    assert!(
        checked,
        "the actual root defer capture was observed in a public save"
    );
}

#[test]
fn native_initial_owned_line_input_moves_into_an_init_defer() {
    native_line_handle_execution(
        &initial_owned_line_init_defer_source(),
        2,
        false,
        false,
        &["body", "init cleanup", "cleanup"],
    );
}

#[test]
fn saved_awbc_initial_owned_line_input_moves_into_an_init_defer() {
    with_saved_awbc_steps(&initial_owned_line_init_defer_source(), |step| {
        execute_nested_line_handle_transfer(
            2,
            false,
            false,
            &["body", "init cleanup", "cleanup"],
            step,
        );
    });
}

fn initial_owned_line_init_defer_source() -> String {
    FLOW_ACTOR_DEFER_SOURCE.replace(
        "defer { let _ = actor; log.info(\"cleanup\") }",
        r#"defer {
            let _ = alice()[Cleanup[p]] with {
                init {
                    defer { let _ = actor; log.info("init cleanup") }
                }
            }
            log.info("cleanup")
        }"#,
    )
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

#[test]
fn deferred_dialogue_keeps_an_actor_in_another_waiting_cleanup_packet() {
    let source = deferred_dialogue_with_waiting_actor_source();
    let expected = &["body", "message", "released"];
    native_line_handle_execution(&source, 2, false, false, expected);
    with_saved_awbc_steps(&source, |step| {
        let mut reveals = 0;
        execute_nested_line_handle_transfer(2, false, false, expected, |input, options| {
            let result = step(input, options);
            for event in &result.output.flow_events {
                if let arcweft_core::plan::FlowEvent::DialogueLine { activation, .. } = event {
                    reveals += 1;
                    if reveals == 2 {
                        assert!(
                            matches!(
                                &result.fiber_status,
                                arcweft_core::engine::FlowFiberStatus::Dialogue(status)
                                    if status.waiting_presentation_activation() == Some(activation)
                            ),
                            "{:?}",
                            result.fiber_status
                        );
                    }
                }
            }
            result
        });
        assert_eq!(reveals, 2);
    });
}

#[test]
fn native_deferred_dialogue_cancellation_returns_to_the_owning_cleanup() {
    native_line_handle_execution(
        &deferred_dialogue_cancellation_source(),
        2,
        false,
        true,
        &["body", "cancelled", "released"],
    );
}

#[test]
fn saved_awbc_deferred_dialogue_cancellation_returns_to_the_owning_cleanup() {
    with_saved_awbc_steps(&deferred_dialogue_cancellation_source(), |step| {
        execute_nested_line_handle_transfer(
            2,
            false,
            true,
            &["body", "cancelled", "released"],
            step,
        );
    });
}

#[test]
fn saved_awbc_deferred_dialogue_rejects_missing_or_replaced_receiver() {
    let mut checked = false;
    assert_saved_awbc_checking(
        &deferred_dialogue_with_waiting_actor_source(),
        2,
        &["body", "message", "released"],
        |executor, save| {
            if checked {
                return executor;
            }
            let Some((index, _)) = save["child_fibers"]
                .as_array()
                .unwrap()
                .iter()
                .enumerate()
                .find(|(_, child)| {
                    child
                        .pointer("/fiber/suspension/reason/DialogueActive")
                        .is_some()
                })
            else {
                return executor;
            };
            for replace in [false, true] {
                let before = executor.snapshot().unwrap();
                let mut forged = save.clone();
                let fiber = &mut forged["child_fibers"][index]["fiber"];
                if replace {
                    let occurrence = &mut fiber["suspension"]["reason"]["DialogueActive"]["activation"]
                        ["occurrence"];
                    *occurrence = (occurrence.as_u64().unwrap() + 1).into();
                } else {
                    fiber["suspension"] = serde_json::Value::Null;
                    fiber["status"] = "Running".into();
                }
                let forged = serde_json::from_value(forged).unwrap();
                let error = executor.bind_product_save_snapshot(forged).unwrap_err();
                assert!(error.to_string().contains("matching receiver"), "{error}");
                assert_eq!(executor.snapshot().unwrap(), before);
            }
            checked = true;
            executor
        },
    );
    assert!(checked);
}

fn deferred_dialogue_with_waiting_actor_source() -> String {
    FLOW_ACTOR_DEFER_SOURCE.replace(
        "defer { let _ = actor; log.info(\"cleanup\") }",
        r#"defer {
            defer { let _ = actor; log.info("released") }
            let received = alice()[Cleanup[p]] with { out "message" }
            log.info(received)
        }"#,
    )
}

fn deferred_dialogue_cancellation_source() -> String {
    FLOW_ACTOR_DEFER_SOURCE.replace(
        "defer { let _ = actor; log.info(\"cleanup\") }",
        r#"defer {
            defer { let _ = actor; log.info("released") }
            let received = alice()[Cleanup[mark @.later][p]] with {
                on mark(@.later) { out "wrong" }
                cancel on input(.SkipLine) { out "cancelled" }
            }
            log.info(received)
        }"#,
    )
}

#[test]
fn failed_deferred_dialogue_drains_child_and_caller_cleanup() {
    let source = r#"
pub character alice { display = "Alice" }
flow main() -> Unit effects { log.write } {
    defer on failed { log.info("outer failed") }
    scope region {
        defer {
            defer { log.info("child cleanup") }
            let _ = alice()[Cleanup[p]] with {
                let zero = 0i64
                log.info(1i64 / zero)
                out "wrong"
            }
            log.info("wrong callback continuation")
        }
        log.info("body")
    }
    log.info("wrong caller continuation")
}
entry cli @entry.main { goto @flow.main }
"#;
    with_saved_awbc_steps(source, |step| {
        let mut logs = Vec::new();
        let mut diagnosed = false;
        for _ in 0..512 {
            let result = step(Default::default(), Default::default());
            assert!(result.stats.executed_ops <= 1);
            diagnosed |= !result.output.diagnostics.is_empty();
            logs.extend(log_messages(&result.output.effects.line));
            if let arcweft_core::engine::FlowFiberStatus::Failed(message) = result.fiber_status {
                assert!(message.contains("division by zero"), "{message}");
                assert!(diagnosed);
                assert_eq!(logs, ["body", "child cleanup", "outer failed"]);
                return;
            }
        }
        panic!("failed deferred dialogue did not drain its actual owners: {logs:?}");
    });
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

#[test]
fn saved_awbc_initial_line_input_rejects_forged_packet_and_activation_slots() {
    let mut packet_seen = false;
    let mut activation_seen = false;
    assert_saved_awbc_checking(
        &owned_deferred_line_source(),
        2,
        &["body", "cleanup", "nested cleanup"],
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
                    let packet = slot["kind"] == "awbc_dialogue_operand";
                    let activation =
                        slot["kind"] == "awbc_register"
                            && save["dialogues"]["entries"].as_array().unwrap().iter().any(
                                |entry| {
                                    entry.pointer("/Active/frame/phase/Activating/fiber/instance")
                                        == Some(&slot["fiber"])
                                },
                            );
                    if !(packet && !packet_seen || activation && !activation_seen) {
                        continue;
                    }
                    let before = executor.snapshot().unwrap();
                    let fields: &[&str] = if packet {
                        &["execution", "fiber", "frame", "content"]
                    } else {
                        &["execution", "fiber", "frame", "register"]
                    };
                    for field in fields {
                        let mut forged = save.clone();
                        let value = &mut forged["dialogues"]["entries"][entry_index]["PublishedHandles"]
                            ["handles"]["ledger"]["leases"][key]["owner"]["ParentFiber"]["slot"]
                            [*field];
                        *value = if let Some(value) = value.as_str() {
                            (value.parse::<u64>().unwrap() + 1).to_string().into()
                        } else {
                            (value.as_u64().unwrap() + 1).into()
                        };
                        executor = reject_capture_save(executor, forged);
                        assert_eq!(executor.snapshot().unwrap(), before);
                    }
                    if packet {
                        assert_eq!(slot["operand"]["kind"], "initial_input");
                        let mut forged = save.clone();
                        let ordinal = &mut forged["dialogues"]["entries"][entry_index]["PublishedHandles"]
                            ["handles"]["ledger"]["leases"][key]["owner"]["ParentFiber"]["slot"]["operand"]
                            ["ordinal"];
                        *ordinal = (ordinal.as_u64().unwrap() + 1).into();
                        executor = reject_capture_save(executor, forged);
                        assert_eq!(executor.snapshot().unwrap(), before);
                        let mut forged = save.clone();
                        let child = forged["child_fibers"]
                            .as_array_mut()
                            .unwrap()
                            .iter_mut()
                            .find(|child| child["fiber"]["instance"] == slot["fiber"])
                            .unwrap();
                        child["fiber"]["suspension"]["reason"]["Dialogue"]["line_task_captures"] =
                            serde_json::json!([]);
                        let forged = serde_json::from_value(forged).unwrap();
                        if let Ok(bound) = executor.bind_product_save_snapshot(forged) {
                            let Err(rejected) = executor.restore_snapshot(
                                arcweft_core::executor::ArcweftRuntimeExecutorSnapshot::AwbcProduct(
                                    bound,
                                ),
                            ) else {
                                panic!("the original Dialogue input packet is required")
                            };
                            (executor, _) = rejected.into_parts();
                        }
                        assert_eq!(executor.snapshot().unwrap(), before);
                        packet_seen = true;
                    } else {
                        activation_seen = true
                    }
                }
            }
            executor
        },
    );
    assert!(
        packet_seen && activation_seen,
        "the actual input crossed both public save frontiers"
    );
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
    check: impl FnMut(
        arcweft_core::executor::ArcweftRuntimeExecutor,
        &serde_json::Value,
    ) -> arcweft_core::executor::ArcweftRuntimeExecutor,
) {
    with_saved_awbc_steps_checking(source, check, |step| {
        execute_nested_line_handle_transfer(reveals, false, false, expected, step);
    });
}

fn with_saved_awbc_steps<R>(
    source: &str,
    run: impl FnOnce(
        &mut dyn FnMut(
            arcweft_core::step::RuntimeStepInput,
            arcweft_core::step::RuntimeStepOptions,
        ) -> arcweft_core::step::RuntimeStepResult,
    ) -> R,
) -> R {
    with_saved_awbc_steps_checking(source, |executor, _| executor, run)
}

fn with_saved_awbc_steps_checking<R>(
    source: &str,
    mut check: impl FnMut(
        arcweft_core::executor::ArcweftRuntimeExecutor,
        &serde_json::Value,
    ) -> arcweft_core::executor::ArcweftRuntimeExecutor,
    run: impl FnOnce(
        &mut dyn FnMut(
            arcweft_core::step::RuntimeStepInput,
            arcweft_core::step::RuntimeStepOptions,
        ) -> arcweft_core::step::RuntimeStepResult,
    ) -> R,
) -> R {
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
    run(&mut |input, options| {
        let mut current = awbc.take().unwrap();
        let result = current.step_with_pure_backend(input, options, &mut backend);
        awbc = Some(roundtrip_nested_awbc_checking(current, &mut check).0);
        result
    })
}

#[test]
fn native_initial_owned_line_init_defer_preserves_nested_scopes() {
    let [(source, expected), _] = initial_owned_line_scoped_defer_sources();
    native_line_handle_execution(&source, 2, false, false, expected);
}

#[test]
fn saved_awbc_initial_owned_line_init_defer_preserves_nested_scopes() {
    let [(source, expected), _] = initial_owned_line_scoped_defer_sources();
    assert_saved_awbc_with_reveals(&source, 2, expected);
}

#[test]
fn native_initial_owned_line_init_defer_preserves_explicit_scope() {
    let [_, (source, expected)] = initial_owned_line_scoped_defer_sources();
    native_line_handle_execution(&source, 2, false, false, expected);
}

#[test]
fn saved_awbc_initial_owned_line_init_defer_preserves_explicit_scope() {
    let [_, (source, expected)] = initial_owned_line_scoped_defer_sources();
    assert_saved_awbc_with_reveals(&source, 2, expected);
}

fn initial_owned_line_scoped_defer_sources() -> [(String, &'static [&'static str]); 2] {
    let source = initial_owned_line_init_defer_source();
    [
        (
            source.replace(
                "defer { let _ = actor; log.info(\"init cleanup\") }",
                r#"defer {
                    defer { let _ = actor; log.info("init cleanup") }
                    log.info("nested body")
                }"#,
            ),
            &["body", "nested body", "init cleanup", "cleanup"],
        ),
        (
            source.replace(
                "defer { let _ = actor; log.info(\"init cleanup\") }",
                r#"scope cleanup {
                    defer { let _ = actor; log.info("init cleanup") }
                    log.info("scope body")
                }"#,
            ),
            &["body", "scope body", "init cleanup", "cleanup"],
        ),
    ]
}

#[test]
fn native_initial_owned_line_init_defer_preserves_outcome_filters() {
    for outcome in ["completed", "failed"] {
        let source = initial_owned_line_init_defer_source().replace(
            "defer { let _ = actor;",
            &format!("defer on {outcome} {{ let _ = actor;"),
        );
        let expected: &[&str] = if outcome == "completed" {
            &["body", "init cleanup", "cleanup"]
        } else {
            &["body", "cleanup"]
        };
        native_line_handle_execution(&source, 2, false, false, expected);
    }
}

#[test]
fn saved_awbc_initial_owned_line_init_defer_preserves_outcome_filters() {
    for outcome in ["completed", "failed"] {
        let source = initial_owned_line_init_defer_source().replace(
            "defer { let _ = actor;",
            &format!("defer on {outcome} {{ let _ = actor;"),
        );
        let expected: &[&str] = if outcome == "completed" {
            &["body", "init cleanup", "cleanup"]
        } else {
            &["body", "cleanup"]
        };
        assert_saved_awbc_with_reveals(&source, 2, expected);
    }
}

#[test]
fn saved_awbc_initial_line_init_defer_rejects_forged_capture_custody() {
    use arcweft_core::executor::ArcweftRuntimeExecutorSnapshot;
    let mut checked = false;
    assert_saved_awbc_checking(
        &initial_owned_line_init_defer_source(),
        2,
        &["body", "init cleanup", "cleanup"],
        |mut executor, save| {
            if checked {
                return executor;
            }
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
                    let Some(slot) = lease
                        .pointer("/owner/ParentFiber/slot")
                        .filter(|slot| slot["kind"] == "awbc_defer_capture")
                    else {
                        continue;
                    };
                    if !save["dialogues"]["entries"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|entry| {
                            entry.pointer("/Active/frame/phase/Activating/fiber/instance")
                                == Some(&slot["fiber"])
                        })
                    {
                        continue;
                    }
                    let before = executor.snapshot().unwrap();
                    let mut mutations = Vec::new();
                    for field in [
                        "execution",
                        "fiber",
                        "frame",
                        "scope",
                        "registration",
                        "ordinal",
                    ] {
                        let mut forged = save.clone();
                        let value = &mut forged["dialogues"]["entries"][entry_index]["PublishedHandles"]
                            ["handles"]["ledger"]["leases"][key]["owner"]["ParentFiber"]["slot"]
                            [field];
                        *value = if field == "scope" {
                            if value.is_null() {
                                0.into()
                            } else {
                                serde_json::Value::Null
                            }
                        } else if let Some(value) = value.as_str() {
                            (value.parse::<u64>().unwrap() + 1).to_string().into()
                        } else {
                            (value.as_u64().unwrap() + 1).into()
                        };
                        mutations.push(forged);
                    }
                    for duplicate in [false, true] {
                        let mut forged = save.clone();
                        let receiver = forged["dialogues"]["entries"]
                            .as_array_mut()
                            .unwrap()
                            .iter_mut()
                            .find(|entry| {
                                entry.pointer("/Active/frame/phase/Activating/fiber/instance")
                                    == Some(&slot["fiber"])
                            })
                            .unwrap();
                        let frame =
                            receiver["Active"]["frame"]["phase"]["Activating"]["fiber"]["frames"]
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
                        if duplicate {
                            packets.push(packets[index].clone());
                        } else {
                            packets.remove(index);
                        }
                        mutations.push(forged);
                    }
                    for forged in mutations {
                        let forged = serde_json::from_value(forged).unwrap();
                        match executor.bind_product_save_snapshot(forged) {
                            Err(_) => {}
                            Ok(bound) => {
                                let Err(rejected) = executor.restore_snapshot(
                                    ArcweftRuntimeExecutorSnapshot::AwbcProduct(bound),
                                ) else {
                                    panic!("forged scoped custody must be refused");
                                };
                                let (unchanged, _) = rejected.into_parts();
                                executor = unchanged;
                            }
                        }
                        assert_eq!(executor.snapshot().unwrap(), before);
                    }
                    checked = true;
                    return executor;
                }
            }
            executor
        },
    );
    assert!(
        checked,
        "the received init-defer packet reached a public save"
    );
}

#[test]
fn native_initial_owned_line_init_defer_retains_its_packet_while_a_child_dialogue_waits() {
    for cancel in [false, true] {
        native_line_handle_execution_at(
            &initial_owned_line_waiting_init_defer_source(),
            3,
            false,
            cancel.then_some(2),
            &[
                "body",
                if cancel { "cancelled" } else { "nested" },
                "init cleanup",
                "cleanup",
            ],
        );
    }
}

#[test]
fn saved_awbc_initial_owned_line_init_defer_retains_its_packet_while_a_child_dialogue_waits() {
    for cancel in [false, true] {
        with_saved_awbc_steps(&initial_owned_line_waiting_init_defer_source(), |step| {
            execute_nested_line_handle_transfer_at(
                3,
                false,
                cancel.then_some(2),
                &[
                    "body",
                    if cancel { "cancelled" } else { "nested" },
                    "init cleanup",
                    "cleanup",
                ],
                step,
            );
        });
    }
}

fn initial_owned_line_waiting_init_defer_source() -> String {
    initial_owned_line_init_defer_source().replace(
        "defer { let _ = actor; log.info(\"init cleanup\") }",
        r#"defer {
            defer { let _ = actor; log.info("init cleanup") }
            let received = alice()[Inner[p]] with {
                cancel on input(.SkipLine) { out "cancelled" }
                out "nested"
            }
            log.info(received)
        }"#,
    )
}
