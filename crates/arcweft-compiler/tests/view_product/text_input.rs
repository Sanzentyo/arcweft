use super::*;
use arcweft_core::{pure::VmRuntimePureCallBackend, value::RuntimeFormatContext};
use arcweft_presentation::{
    input::InteractionTarget,
    text_input::{
        TextByteOffset, TextControlValue, TextControlWriteBack, TextInputSessionId, TextRange,
        TextRevision,
    },
};
use arcweft_runtime_driver::{
    clock::RuntimeClockStep,
    presentation_handles::{PresentationHandleRecord, PresentationResourceState},
    session::{BundleSession, BundleSessionOptions, BundleSessionStep, BundleStepInput},
    view_runtime::{BundleViewFrame, BundleViewTextValue},
};

const SOURCE: &str = r#"
entry cli @entry.main { goto @flow.main }
flow main() -> String { return "done" }
view Main() {
    local state mut caption: String = "first"
    local state mut submitted: String = "none"
    Column {
        TextField(caption, label = "Feedback").on_submit(|| { submitted = caption; () })
        Text(caption)
        Text(submitted)
    }
}
"#;

fn bundle(source: &str) -> ArcweftBundle {
    let fixture = project_view_fixture_with_entry(source, "arcweft-test://retained-text-input");
    let compiled = fixture.compile().expect("typed retained text input");
    let awbc = AwbcLowerer::new(
        &compiled.runtime_plan().plan,
        &compiled.runtime_plan().dialogue_content_catalog,
        "main.arcw",
    )
    .lower()
    .unwrap()
    .program;
    let bundle = ArcweftBundle::try_new(
        BundleManifest {
            profile_id: None,
            profile_kind: None,
            locale: compiled.locale().clone(),
            entry: Some("entry.main".to_owned()),
            adapter: None,
            adapter_manifest_ids: Vec::new(),
            required_host_calls: Vec::new(),
            runtime: BundleRuntimeSummary {
                artifact_fingerprint: RuntimeArtifactFingerprint::try_from_bytes([0x8a; 32])
                    .unwrap(),
                entry_flow: Some("flow.main".to_owned()),
                flows: compiled.runtime_plan().plan.flows().len(),
                bytecode_instructions: awbc.instructions.len(),
                line_task_groups: 0,
                stream_plans: 0,
            },
        },
        SourceMapSection::try_from_documents(&[fixture.document.as_ref()]).unwrap(),
        awbc,
        compiled.runtime_plan().dialogue_content_catalog.clone(),
    )
    .unwrap()
    .try_with_validated_view_product(compiled.view_product().product())
    .unwrap();
    let bundle = bundle.with_view_text(compiled.view_product().text().unwrap().clone());
    let bundle = if let Some(generation) = &compiled.runtime_plan().character_dialogue_generation {
        bundle.with_character_dialogue_generation(generation.as_ref().clone())
    } else {
        bundle
    };
    let bytes = bundle
        .to_format_bytes(BundleFormat::Awfb)
        .expect("retained input encodes");
    let decoded = ArcweftBundle::from_format_slice(BundleFormat::Awfb, &bytes)
        .expect("retained input decodes");
    assert_eq!(
        decoded.to_format_bytes(BundleFormat::Awfb).unwrap(),
        bytes,
        "canonical input/field-access/handler encoding"
    );
    decoded
}

fn runtime(source: &str) -> (BundleViewRuntime, PresentationHandleRecord) {
    let bundle = bundle(source);
    let product = ValidatedViewProduct::try_new(
        Some(bundle.source_map.clone()),
        bundle.view_program.clone(),
        bundle.view_style.clone(),
        ViewProductValidationLimits::default(),
    )
    .unwrap();
    (
        BundleViewRuntime::try_new_with_awbc(
            product,
            bundle.view_text.clone(),
            bundle.product_awbc().program_lease(),
        )
        .unwrap(),
        PresentationHandleRecord::new(
            PresentationHandleId::try_new("view.text.first").unwrap(),
            PresentationResourceKind::View,
            "view.Main".to_owned(),
            None,
            PresentationResourceState::Mounted,
            None,
            0,
        ),
    )
}

fn frame(runtime: &mut BundleViewRuntime, handle: &PresentationHandleRecord) -> BundleViewFrame {
    let frame = runtime.evaluate(std::slice::from_ref(handle), &[], false);
    assert!(frame.diagnostics.is_empty(), "{frame:#?}");
    frame
}

fn write(
    control: &arcweft_bundle::resource_codec::ViewRuntimeTextControl,
    value: &str,
    revision: u64,
    submit: bool,
) -> TextControlWriteBack {
    let target = InteractionTarget::new(PublicId::try_new(&control.target).unwrap());
    let value = if control.kind.is_secure() {
        TextControlValue::sensitive(value)
    } else {
        TextControlValue::plain(value)
    };
    let range = TextRange::new(TextByteOffset(0), TextByteOffset(0));
    if submit {
        TextControlWriteBack::submit(
            target,
            TextInputSessionId(control.session),
            value,
            range,
            TextRevision(revision),
        )
    } else {
        TextControlWriteBack::change(
            target,
            TextInputSessionId(control.session),
            value,
            range,
            TextRevision(revision),
        )
    }
}

fn dispatch(
    runtime: &mut BundleViewRuntime,
    input: &TextControlWriteBack,
) -> Result<
    Option<arcweft_runtime_driver::dialogue::BundlePresentationInput>,
    arcweft_runtime_driver::view_runtime::BundleViewEventDispatchError,
> {
    runtime.dispatch_text_input_with_backend(
        input,
        &RuntimeFormatContext::default(),
        &mut VmRuntimePureCallBackend::default(),
    )
}

fn labels<F>(frame: &BundleViewFrame<F>) -> Vec<&str> {
    frame.mounts[0]
        .text
        .iter()
        .map(|text| match &text.value {
            BundleViewTextValue::Plain { value } => value.as_str(),
            _ => panic!("plain String"),
        })
        .collect()
}

#[test]
fn retained_text_input_updates_and_submits_the_current_value_across_cold_restore() {
    let (mut runtime, handle) = runtime(SOURCE);
    let first = frame(&mut runtime, &handle);
    let control = &first.mounts[0].text_inputs[0];
    assert_eq!(control.value, "first");
    assert_eq!(control.label.as_deref(), Some("Feedback"));
    assert_eq!(labels(&first), ["first", "none"]);
    dispatch(&mut runtime, &write(control, "東京", 1, false)).unwrap();
    let changed = frame(&mut runtime, &handle);
    assert_eq!(changed.mounts[0].text_inputs[0].value, "東京");
    assert_eq!(changed.mounts[0].text_inputs[0].selection.start, 0);
    assert_eq!(labels(&changed), ["東京", "none"]);
    let saved = runtime.snapshot().unwrap();
    runtime
        .restore(&saved, std::slice::from_ref(&handle))
        .unwrap();
    let restored = frame(&mut runtime, &handle);
    assert_eq!(labels(&restored), ["東京", "none"]);
    assert!(dispatch(&mut runtime, &write(control, "stale", 2, false)).is_err());
    dispatch(
        &mut runtime,
        &write(&restored.mounts[0].text_inputs[0], "京都", 1, true),
    )
    .unwrap();
    let submitted = frame(&mut runtime, &handle);
    assert_eq!(labels(&submitted), ["京都", "京都"]);
    assert_eq!(submitted.mounts[0].text_inputs[0].value, "京都");
}

#[test]
fn retained_text_input_handler_failure_keeps_value_routes_and_retry_intact() {
    let source = SOURCE.replace("submitted = caption; ()", "let divisor = match caption == \"bad\" { true => 0i64, false => 1i64 }; let quotient = 1i64 / divisor; submitted = caption; ()");
    let (mut runtime, handle) = runtime(&source);
    let initial = frame(&mut runtime, &handle);
    let control = &initial.mounts[0].text_inputs[0];
    let before = runtime.snapshot().unwrap();
    assert!(dispatch(&mut runtime, &write(control, "bad", 1, true)).is_err());
    assert_eq!(runtime.snapshot().unwrap(), before);
    // The original event seal and adapter revision remain usable after failure.
    dispatch(&mut runtime, &write(control, "good", 1, true)).unwrap();
    assert_eq!(labels(&frame(&mut runtime, &handle)), ["good", "good"]);
}

#[test]
fn retained_text_input_rejects_nonwritable_or_transient_value_sources() {
    for source in [
        SOURCE.replace("local state mut caption", "local state caption"),
        SOURCE.replace("local state mut caption", "let mut caption"),
        SOURCE.replace("TextField(caption,", "TextField(\"literal\","),
        SOURCE.replace("TextField(caption,", "Button(value = caption,"),
        SOURCE.replace("TextField(caption,", "TextField(caption, value = caption,"),
        SOURCE.replace(
            "TextField(caption, label = \"Feedback\")",
            "Button(\"not a text control\")",
        ),
    ] {
        assert!(
            project_view_fixture_with_entry(&source, "arcweft-test://invalid-text-input")
                .compile()
                .is_err(),
            "{source}"
        );
    }
}

#[test]
fn retained_text_input_keyed_occurrences_keep_distinct_targets_and_values() {
    use arcweft_core::value::{RuntimeBinding, RuntimeValue};
    let source = SOURCE.replace("view Main()", "view Main(items: Vec<String>)")
        .replace("local state mut caption: String = \"first\"", "")
        .replace("TextField(caption, label = \"Feedback\").on_submit(|| { submitted = caption; () })", "for item in items key = item { local state mut caption: String = item; TextField(caption, label = item) }")
        .replace("Text(caption)", "");
    let (mut runtime, handle) = runtime(&source);
    let evaluate = |runtime: &mut BundleViewRuntime, keys: &[&str]| {
        let inputs = [RuntimeBinding {
            name: "items".to_owned(),
            value: RuntimeValue::Seq(arcweft_core::value::RuntimeSeq::values(
                keys.iter()
                    .map(|value| RuntimeValue::String((*value).to_owned()))
                    .collect(),
            )),
        }];
        let frame = runtime.evaluate(std::slice::from_ref(&handle), &inputs, false);
        assert!(frame.diagnostics.is_empty(), "{frame:#?}");
        frame
    };
    let first = evaluate(&mut runtime, &["a", "b"]);
    let controls = &first.mounts[0].text_inputs;
    assert_eq!(controls.len(), 2);
    assert_ne!(controls[0].target, controls[1].target);
    dispatch(&mut runtime, &write(&controls[0], "edited a", 1, false)).unwrap();
    let reordered = evaluate(&mut runtime, &["b", "a"]);
    let controls = &reordered.mounts[0].text_inputs;
    assert_eq!(
        (controls[0].label.as_deref(), controls[0].value.as_str()),
        (Some("b"), "b")
    );
    assert_eq!(
        (controls[1].label.as_deref(), controls[1].value.as_str()),
        (Some("a"), "edited a")
    );
    let old = controls[1].clone();
    evaluate(&mut runtime, &["b"]);
    let reintroduced = evaluate(&mut runtime, &["b", "a"]);
    assert_eq!(reintroduced.mounts[0].text_inputs[1].value, "a");
    assert_ne!(reintroduced.mounts[0].text_inputs[1].session, old.session);
    assert!(dispatch(&mut runtime, &write(&old, "stale", 2, false)).is_err());
}

#[test]
fn retained_text_input_kinds_enforce_privacy_selection_revision_and_capacity() {
    use arcweft_bundle::resource_codec::view::ViewInputKind;
    for (widget, expected) in [
        ("TextField", ViewInputKind::TextField),
        ("TextArea", ViewInputKind::TextArea),
        ("SecureField", ViewInputKind::SecureField),
    ] {
        let (mut runtime, handle) = runtime(&SOURCE.replace("TextField", widget));
        let initial = frame(&mut runtime, &handle);
        let control = &initial.mounts[0].text_inputs[0];
        assert_eq!(control.kind, expected);
        assert_eq!(control.options.multiline, expected.is_multiline());
        let before = runtime.snapshot().unwrap();
        let wrong_privacy = TextControlWriteBack::change(
            InteractionTarget::new(PublicId::try_new(&control.target).unwrap()),
            TextInputSessionId(control.session),
            if expected.is_secure() {
                TextControlValue::plain("private")
            } else {
                TextControlValue::sensitive("private")
            },
            TextRange::new(TextByteOffset(0), TextByteOffset(0)),
            TextRevision(1),
        );
        assert!(dispatch(&mut runtime, &wrong_privacy).is_err());
        let bad_selection = TextControlWriteBack::change(
            InteractionTarget::new(PublicId::try_new(&control.target).unwrap()),
            TextInputSessionId(control.session),
            if expected.is_secure() {
                TextControlValue::sensitive("東京")
            } else {
                TextControlValue::plain("東京")
            },
            TextRange::new(TextByteOffset(1), TextByteOffset(1)),
            TextRevision(1),
        );
        assert!(dispatch(&mut runtime, &bad_selection).is_err());
        let huge = "x".repeat(
            arcweft_core::entry::RuntimeSchemaLimits::engine_default().max_encoded_bytes as usize
                + 1,
        );
        assert!(dispatch(&mut runtime, &write(control, &huge, 1, false)).is_err());
        assert_eq!(runtime.snapshot().unwrap(), before);
        dispatch(&mut runtime, &write(control, "accepted", 1, false)).unwrap();
        assert!(dispatch(&mut runtime, &write(control, "replay", 1, false)).is_err());
        assert_eq!(
            frame(&mut runtime, &handle).mounts[0].text_inputs[0].value,
            "accepted"
        );
    }
}

#[test]
fn retained_text_input_bundle_rejects_redirecting_writes_to_readonly_state() {
    let source = SOURCE.replace(
        "Column {",
        "local state protected: String = \"protected\"; Column {",
    );
    let compiled = project_view_fixture_with_entry(&source, "arcweft-test://text-input-authority")
        .compile()
        .unwrap();
    let product = compiled.view_product().product();
    let mut changed = product.program().unwrap().resource().clone();
    let readonly = changed
        .instructions
        .iter()
        .filter_map(|instruction| match instruction {
            ViewProgramInstruction::BindLocal { program, .. } => {
                program.outputs.first().map(|output| output.coordinate)
            }
            _ => None,
        })
        .nth(2)
        .unwrap();
    let ViewProgramInstruction::BindTextInput { value, .. } = changed
        .instructions
        .iter_mut()
        .find(|instruction| matches!(instruction, ViewProgramInstruction::BindTextInput { .. }))
        .unwrap()
    else {
        unreachable!()
    };
    value.source = arcweft_view::ViewExecutionInputSource::Local(readonly);
    let accepted = ValidatedViewProduct::try_new(
        Some(product.source_map().clone()),
        Some(changed),
        product.style().map(|style| style.resource().clone()),
        ViewProductValidationLimits::default(),
    );
    let awbc = Arc::new(
        arcweft_bundle::standard_view::install_dialogue_handler_awbc(
            AwbcLowerer::new(
                &compiled.runtime_plan().plan,
                &compiled.runtime_plan().dialogue_content_catalog,
                "main.arcw",
            )
            .lower()
            .unwrap()
            .program,
        )
        .unwrap(),
    );
    assert!(
        accepted.map_or(true, |product| BundleViewRuntime::try_new_with_awbc(
            product,
            compiled.view_product().text().cloned(),
            awbc
        )
        .is_err()),
        "a typed input must not acquire write authority over readonly state"
    );
}

#[test]
fn retained_text_input_player_batch_changes_then_submits_without_losing_the_handler() {
    let (mut session, initial) = player_session(SOURCE);
    assert_eq!(initial.presentation.text_inputs.len(), 1);
    let control = &initial.presentation.text_inputs[0];
    session
        .queue_text_control_write_backs([
            write(control, "東京", 1, false),
            write(control, "京都", 2, true),
        ])
        .expect("player edit and submit batch");
    let updated = session.step_with_clock(
        RuntimeClockStep::from_millis(2, 16).unwrap(),
        BundleStepInput::default(),
    );
    assert!(updated.diagnostics.is_empty(), "{:?}", updated.diagnostics);
    assert_eq!(labels(&updated.presentation.view), ["京都", "京都"]);
    assert_eq!(updated.text_control_write_backs.len(), 2);
}

fn player_session(source: &str) -> (BundleSession, BundleSessionStep) {
    let source = source.replace("flow main() -> String { return \"done\" }", "pub action hold(value: String)\nflow main() -> String { let panel = mount(Main()); let event = await receive(hold); return event.value }");
    let bundle = bundle(&source);
    let mut session = BundleSession::new(&bundle, BundleSessionOptions::default()).unwrap();
    let initial = session.step_with_clock(
        RuntimeClockStep::from_millis(1, 16).unwrap(),
        BundleStepInput::default(),
    );
    assert!(initial.diagnostics.is_empty(), "{:?}", initial.diagnostics);
    (session, initial)
}

#[test]
fn retained_text_input_player_orders_click_change_and_submit_before_the_next_step() {
    let source = SOURCE.replace("Column {", "Column { Button(\"click\").on_click(|| { submitted = \"clicked\"; () })")
        .replace(".on_submit(|| { submitted = caption; () })", ".on_change(|| { submitted = match submitted == \"clicked\" { true => \"changed\", false => \"missing click\" }; () }).on_submit(|| { submitted = match submitted == \"changed\" { true => \"both\", false => \"missing change\" }; () })");
    let (mut session, initial) = player_session(&source);
    let before_click = session.snapshot_session().unwrap();
    let click = initial.presentation.view.mounts[0]
        .events
        .iter()
        .find(|event| event.event() == EventKind::Activate)
        .unwrap();
    session
        .queue_view_handler_invocation(&retained_transition_invocation(click))
        .unwrap();
    let after_click = session.snapshot_session().unwrap();
    assert_eq!(
        after_click.runtime, before_click.runtime,
        "UI refresh does not advance session clocks or sequences"
    );
    assert_eq!(
        after_click.executor, before_click.executor,
        "UI refresh executes no flow operations"
    );
    assert_eq!(labels(&after_click.presentation.view), ["first", "clicked"]);
    let control = &initial.presentation.text_inputs[0];
    session
        .queue_text_control_write_backs([
            write(control, "東京", 1, false),
            write(control, "京都", 2, true),
        ])
        .unwrap();
    let updated = session.step_with_clock(
        RuntimeClockStep::from_millis(2, 16).unwrap(),
        BundleStepInput::default(),
    );
    assert!(updated.diagnostics.is_empty(), "{:?}", updated.diagnostics);
    assert_eq!(labels(&updated.presentation.view), ["京都", "both"]);
    assert_eq!(updated.text_control_write_backs.len(), 2);
}

#[test]
fn retained_text_input_player_handler_failure_preserves_the_complete_session() {
    let source = SOURCE.replace("submitted = caption; ()", "let divisor = match caption == \"bad\" { true => 0i64, false => 1i64 }; let quotient = 1i64 / divisor; submitted = caption; ()");
    let (mut session, initial) = player_session(&source);
    let control = &initial.presentation.text_inputs[0];
    let before = session.snapshot_session().unwrap();
    assert!(
        session
            .queue_text_control_write_back(&write(control, "bad", 1, true))
            .is_err()
    );
    assert_eq!(
        session.snapshot_session().unwrap(),
        before,
        "rejected input changes no executor, View, presentation, queue, or clock"
    );
    session
        .queue_text_control_write_back(&write(control, "good", 1, true))
        .unwrap();
    let updated = session.step_with_clock(
        RuntimeClockStep::from_millis(2, 16).unwrap(),
        BundleStepInput::default(),
    );
    assert!(updated.diagnostics.is_empty(), "{:?}", updated.diagnostics);
    assert_eq!(labels(&updated.presentation.view), ["good", "good"]);
    assert_eq!(updated.text_control_write_backs.len(), 1);
}
