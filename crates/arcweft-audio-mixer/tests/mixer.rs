use arcweft_audio_core::{
    AudioAsset, AudioBusDef, AudioDecodeStrategy, AudioDispatch, AudioFormat, AudioGraph,
    DEFAULT_MAX_VOICES, DecodedAudio,
};
use arcweft_audio_mixer::Mixer;
use arcweft_interaction_model::audio::{
    AudioBusId, AudioCommand, AudioDispatchId, AudioLoopMode, AudioMillis, AudioResourceId,
    AudioVoiceId, GainDbMilli, PanMilli,
};
use std::sync::Arc;

#[test]
fn mixer_renders_prepared_mono_resource_to_stereo() {
    let resource = AudioResourceId::new("audio.test").expect("resource");
    let bus = AudioBusId::new("bus.master").expect("bus");
    let graph = AudioGraph {
        master_bus: bus.clone(),
        assets: vec![AudioAsset {
            id: resource.clone(),
            path: "audio/test.wav".to_owned(),
            format: AudioFormat::Wav,
            strategy: AudioDecodeStrategy::Preload,
            default_loop: AudioLoopMode::None,
        }],
        buses: vec![AudioBusDef {
            id: bus.clone(),
            parent: None,
            gain: GainDbMilli::UNITY,
            muted: false,
            effects: Vec::new(),
        }],
        snapshots: Vec::new(),
    };
    let (prepared, mut commands) = graph.prepare(DEFAULT_MAX_VOICES).expect("graph");
    let mut mixer = Mixer::new(prepared, 48_000, 64).expect("mixer");
    let decoded = Arc::new(DecodedAudio::new(48_000, 1, vec![1.0, 0.5, 0.0]).expect("pcm"));
    mixer
        .apply(
            commands
                .install_resource(&resource, decoded)
                .expect("install"),
            |_| {},
        )
        .expect("apply install");
    mixer
        .apply(
            commands
                .prepare(AudioDispatch {
                    id: AudioDispatchId {
                        logical_epoch: 1,
                        sequence: 1,
                    },
                    command: AudioCommand::Play {
                        voice: AudioVoiceId::new("voice.test").expect("voice"),
                        resource,
                        bus,
                        gain: GainDbMilli::UNITY,
                        pan: PanMilli::CENTER,
                        loop_mode: AudioLoopMode::None,
                        start_frame: 0,
                        fade_in: AudioMillis::ZERO,
                    },
                })
                .expect("play"),
            |_| {},
        )
        .expect("apply play");

    let mut output = [0.0; 6];
    mixer.render(&mut output, |_| {});
    assert!(output[0] > 0.69 && output[1] > 0.69);
    assert!(output[2] > 0.34 && output[3] > 0.34);
    assert!(output[4].abs() <= f32::EPSILON);
    assert!(output[5].abs() <= f32::EPSILON);
}
