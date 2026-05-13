# Audio Manifest Schema Sketch

```rust
pub struct AudioManifest {
    pub buses: Vec<AudioBusDef>,
    pub cues: Vec<CueDef>,
    pub bgms: Vec<BgmDef>,
    pub voice_profiles: Vec<VoiceProfileDef>,
    pub mixer_snapshots: Vec<MixerSnapshotDef>,
}

pub struct CueDef {
    pub id: EntityId,
    pub public_id: PublicId,
    pub source: AudioSourceRef,
    pub bus: Ref<AudioBus>,
    pub loudness: Option<Lufs>,
    pub loop_points: Option<LoopPoints>,
    pub transcript: Option<String>,
}

pub struct BgmDef {
    pub id: EntityId,
    pub stems: Vec<StemDef>,
    pub sections: Vec<MusicSectionDef>,
    pub transitions: Vec<MusicTransitionDef>,
}
```

