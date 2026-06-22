use arcweft_audio_core::{
    AudioBusDef, AudioEffectDef, AudioEffectKind, AudioGraph, DEFAULT_MAX_VOICES,
};
use arcweft_interaction_model::audio::{AudioBusId, AudioEffectId, GainDbMilli};

#[test]
fn graph_preparation_keeps_parent_before_child() {
    let master_bus = AudioBusId::new("bus.master").expect("master");
    let graph = AudioGraph {
        master_bus: master_bus.clone(),
        assets: Vec::new(),
        buses: vec![
            AudioBusDef {
                id: master_bus.clone(),
                parent: None,
                gain: GainDbMilli::UNITY,
                muted: false,
                effects: vec![AudioEffectDef {
                    id: AudioEffectId::new("effect.master.limit").expect("effect"),
                    enabled: true,
                    kind: AudioEffectKind::Limiter {
                        ceiling_db_milli: -500,
                        release_micros: 80_000,
                    },
                }],
            },
            AudioBusDef {
                id: AudioBusId::new("bus.bgm").expect("bgm"),
                parent: Some(master_bus),
                gain: GainDbMilli::new(-4_000).expect("valid gain"),
                muted: false,
                effects: Vec::new(),
            },
        ],
        snapshots: Vec::new(),
    };

    let (prepared, _) = graph.prepare(DEFAULT_MAX_VOICES).expect("prepare graph");
    assert_eq!(prepared.buses[1].parent, Some(0));
}
