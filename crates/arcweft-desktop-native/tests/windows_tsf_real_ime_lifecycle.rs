use arcweft_desktop_native::text_input::windows_tsf::{
    TsfAcp, TsfAcpRange, TsfRangeError, TsfTextSnapshot, WindowsTsfAdapter, WindowsTsfEditAccess,
    WindowsTsfRuntimeFacts,
};
use arcweft_id::PublicId;
use arcweft_presentation::hit::HitRect;
use arcweft_presentation::input::InteractionTarget;
use arcweft_presentation::text_input::{
    CompositionEndReason, PlatformTextInputEvent, TextByteOffset, TextCommit,
    TextInputClientSnapshot, TextInputFocusGeneration, TextInputOperation, TextInputOptions,
    TextInputSerial, TextInputSessionId, TextRange, TextRevision,
};

fn target(name: &str) -> InteractionTarget {
    InteractionTarget::new(PublicId::try_new(name).unwrap())
}

fn snapshot(session: TextInputSessionId, target_name: &str) -> TextInputClientSnapshot {
    TextInputClientSnapshot::new(
        session,
        target(target_name),
        TextRevision(11),
        "かなabc",
        TextByteOffset(0),
        TextRange::new(TextByteOffset(0), TextByteOffset(0)),
        HitRect::new(0.0, 0.0, 240.0, 24.0),
        HitRect::new(0.0, 0.0, 1.0, 24.0),
        TextInputOptions::default(),
    )
}

#[test]
fn stale_snapshot_range_is_rejected_before_text_leaves_adapter() {
    let text = TsfTextSnapshot::plain(TextRevision(10), "かなabc");

    assert_eq!(
        TsfAcpRange::new(TsfAcp(0), TsfAcp(2)).to_canonical_byte_range(&text, TextRevision(11)),
        Err(TsfRangeError::StaleSnapshot {
            expected: TextRevision(11),
            actual: TextRevision(10),
        })
    );
}

#[test]
fn one_tsf_write_session_finishes_as_one_arcweft_batch() {
    let mut adapter =
        WindowsTsfAdapter::activate(WindowsTsfRuntimeFacts::default().with_runtime_ready())
            .0
            .with_first_serial(TextInputSerial(50));
    let event = adapter
        .begin_edit_session(
            &snapshot(TextInputSessionId(7), "textfield.windows.real"),
            TextInputFocusGeneration(3),
            WindowsTsfEditAccess::ReadWrite,
        )
        .with_operation(TextInputOperation::StartComposition)
        .with_operation(TextInputOperation::Commit(TextCommit::new("日本語")))
        .with_operation(TextInputOperation::EndComposition {
            reason: CompositionEndReason::Committed,
        })
        .finish()
        .expect("write session emits a batch");

    let PlatformTextInputEvent::Batch {
        context,
        operations,
    } = event
    else {
        panic!("TSF write session must use batch event");
    };
    assert_eq!(context.serial(), TextInputSerial(50));
    assert_eq!(context.generation(), TextInputFocusGeneration(3));
    assert_eq!(operations.len(), 3);
}

#[cfg(target_os = "windows")]
#[test]
fn windows_real_ime_owner_type_is_available() {
    fn assert_type<T>() {}
    assert_type::<arcweft_desktop_native::text_input::windows_tsf::real_ime::WindowsTsfImeBridge>();
}
