use arcweft_interaction_model::{
    audio::{
        AudioDispatchId, AudioEvent, AudioResourceId, AudioVoiceId, HostEvent, HostEventBatch,
    },
    id::Identifier,
    input::{
        InputEpoch, InputEventKind, InputSequence, InteractionTarget, KeyCode, RoutedInputEvent,
    },
    payload::InteractionPayload,
};
use std::collections::BTreeMap;

#[test]
fn routed_input_roundtrip_preserves_routing_metadata() {
    let payload = InteractionPayload::Map(BTreeMap::from([(
        "source".to_owned(),
        InteractionPayload::Text("keyboard".to_owned()),
    )]));
    let event = RoutedInputEvent::new(
        InputEpoch::new(7),
        InputSequence::new(41),
        InteractionTarget::new("textbox.main").expect("target"),
        InputEventKind::KeyDown {
            key: KeyCode::new("Enter").expect("key"),
            repeat: false,
        },
    )
    .with_payload(payload);

    let encoded = serde_json::to_string(&event).expect("serialize event");
    let decoded: RoutedInputEvent = serde_json::from_str(&encoded).expect("deserialize event");

    assert_eq!(decoded, event);
    assert_eq!(decoded.epoch.get(), 7);
    assert_eq!(decoded.sequence.get(), 41);
    assert_eq!(decoded.target.as_str(), "textbox.main");
}

#[test]
fn audio_events_are_typed_and_roundtrip_without_string_dispatch() {
    let event = HostEvent::Audio {
        event: AudioEvent::PlaybackStarted {
            playback: AudioDispatchId::new(0, 1),
            voice: AudioVoiceId::new("voice.dialogue").expect("voice"),
            resource: AudioResourceId::new("audio.line.001").expect("resource"),
        },
    };
    let batch = HostEventBatch::new(vec![event]);

    let encoded = serde_json::to_value(&batch).expect("serialize batch");
    let decoded: HostEventBatch = serde_json::from_value(encoded).expect("deserialize batch");

    assert_eq!(decoded, batch);
}

#[test]
fn identifier_rejects_whitespace_only_input() {
    assert!(Identifier::new("   ").is_err());
}
