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
    presentation_handles::{PresentationHandleRecord, PresentationResourceState},
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

fn runtime(source: &str) -> (BundleViewRuntime, PresentationHandleRecord) {
    let compiled = project_view_fixture_with_entry(source, "arcweft-test://retained-text-input")
        .compile()
        .expect("typed retained text input");
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
    (
        BundleViewRuntime::try_new_with_awbc(
            compiled.view_product().product().as_ref().clone(),
            compiled.view_product().text().cloned(),
            awbc,
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

fn labels(frame: &BundleViewFrame) -> Vec<&str> {
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
