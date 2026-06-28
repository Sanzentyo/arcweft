use arcweft_desktop_native::text_input::windows_tsf::{
    WindowsTsfAdapter, WindowsTsfEditAccess, WindowsTsfRuntimeFacts,
};
use arcweft_id::PublicId;
use arcweft_presentation::hit::HitRect;
use arcweft_presentation::input::InteractionTarget;
use arcweft_presentation::text_input::{
    CompositionEndReason, PlatformTextInputEvent, TextByteOffset, TextCommit,
    TextCompositionUpdate, TextInputClientSnapshot, TextInputFocusGeneration, TextInputOperation,
    TextInputOptions, TextInputSerial, TextInputSessionId, TextRange, TextRevision,
};

fn main() {
    let target = InteractionTarget::new(
        PublicId::try_new("sample.ime.textfield").expect("sample target id is valid"),
    );
    let snapshot = TextInputClientSnapshot::new(
        TextInputSessionId(1),
        target,
        TextRevision(1),
        "",
        TextByteOffset(0),
        TextRange::new(TextByteOffset(0), TextByteOffset(0)),
        HitRect::new(64.0, 72.0, 620.0, 72.0),
        HitRect::new(76.0, 88.0, 2.0, 42.0),
        TextInputOptions::default(),
    );
    let facts = WindowsTsfRuntimeFacts::default()
        .with_runtime_ready()
        .with_reconversion_function_available()
        .with_mapped_display_attributes()
        .with_layout_available();
    let (mut adapter, activation) = WindowsTsfAdapter::activate(facts);
    adapter = adapter.with_first_serial(TextInputSerial(1));
    let preedit = "かな";
    let preedit_bytes = u32::try_from(preedit.len()).expect("sample preedit fits u32");
    let event = adapter
        .begin_edit_session(
            &snapshot,
            TextInputFocusGeneration(1),
            WindowsTsfEditAccess::ReadWrite,
        )
        .with_operation(TextInputOperation::StartComposition)
        .with_operation(TextInputOperation::SetComposition(
            TextCompositionUpdate::new(
                preedit,
                TextRange::new(TextByteOffset(0), TextByteOffset(preedit_bytes)),
            ),
        ))
        .with_operation(TextInputOperation::Commit(TextCommit::new("仮名")))
        .with_operation(TextInputOperation::EndComposition {
            reason: CompositionEndReason::Committed,
        })
        .finish()
        .expect("write edit session emits an Arcweft text-input event");

    println!("Arcweft native IME adapter contract sample");
    println!("capability diagnostics: {}", activation.diagnostics().len());
    print_event(&event);
}

fn print_event(event: &PlatformTextInputEvent) {
    match event {
        PlatformTextInputEvent::Batch {
            context,
            operations,
        } => {
            println!(
                "batch adapter={:?} session={:?} generation={:?} serial={:?}",
                context.adapter(),
                context.session(),
                context.generation(),
                context.serial()
            );
            for operation in operations {
                println!("operation {operation:?}");
            }
        }
        other => println!("event {other:?}"),
    }
}
