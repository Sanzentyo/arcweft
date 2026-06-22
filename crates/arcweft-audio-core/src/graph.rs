use std::collections::{BTreeMap, BTreeSet};

use arcweft_interaction_model::audio::{
    AudioBusId, AudioEffectId, AudioEffectParameter, AudioLoopMode, AudioResourceId,
    AudioSnapshotId, GainDbMilli,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const MAX_BUSES: usize = 64;
pub const MAX_EFFECTS_PER_BUS: usize = 16;
pub const DEFAULT_MAX_VOICES: usize = 256;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioFormat {
    Wav,
    Flac,
    OggVorbis,
    Mp3,
    AacMp4,
}

impl AudioFormat {
    #[must_use]
    pub const fn extension_hint(self) -> &'static str {
        match self {
            Self::Wav => "wav",
            Self::Flac => "flac",
            Self::OggVorbis => "ogg",
            Self::Mp3 => "mp3",
            Self::AacMp4 => "m4a",
        }
    }

    #[must_use]
    pub fn from_extension(extension: &str) -> Option<Self> {
        match extension
            .trim_start_matches('.')
            .to_ascii_lowercase()
            .as_str()
        {
            "wav" | "wave" => Some(Self::Wav),
            "flac" => Some(Self::Flac),
            "ogg" | "oga" => Some(Self::OggVorbis),
            "mp3" => Some(Self::Mp3),
            "aac" | "m4a" | "mp4" => Some(Self::AacMp4),
            _ => None,
        }
    }

    #[must_use]
    pub const fn supports_sample_accurate_loop(self) -> bool {
        matches!(self, Self::Wav | Self::Flac | Self::OggVorbis)
    }

    #[must_use]
    pub const fn mime_type(self) -> &'static str {
        match self {
            Self::Wav => "audio/wav",
            Self::Flac => "audio/flac",
            Self::OggVorbis => "audio/ogg",
            Self::Mp3 => "audio/mpeg",
            Self::AacMp4 => "audio/mp4",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioDecodeStrategy {
    Preload,
    Stream,
}

impl AudioDecodeStrategy {
    #[must_use]
    pub const fn is_streaming(self) -> bool {
        matches!(self, Self::Stream)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AudioAsset {
    pub id: AudioResourceId,
    pub path: String,
    pub format: AudioFormat,
    pub strategy: AudioDecodeStrategy,
    pub default_loop: AudioLoopMode,
}

impl AudioAsset {
    pub fn validate(&self) -> Result<(), AudioGraphError> {
        if self.path.trim().is_empty() {
            return Err(AudioGraphError::InvalidAsset {
                asset: self.id.clone(),
                message: "path must not be empty".to_owned(),
            });
        }
        if !matches!(self.default_loop, AudioLoopMode::None)
            && !self.format.supports_sample_accurate_loop()
        {
            return Err(AudioGraphError::InvalidAsset {
                asset: self.id.clone(),
                message: format!(
                    "{} requires a post-decode trim manifest before sample-accurate looping",
                    self.format.extension_hint()
                ),
            });
        }
        if self.strategy.is_streaming() && !matches!(self.default_loop, AudioLoopMode::None) {
            return Err(AudioGraphError::InvalidAsset {
                asset: self.id.clone(),
                message: "streamed assets must use a host-managed section transition instead of a PCM loop".to_owned(),
            });
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AudioBusDef {
    pub id: AudioBusId,
    pub parent: Option<AudioBusId>,
    pub gain: GainDbMilli,
    pub muted: bool,
    pub effects: Vec<AudioEffectDef>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AudioEffectDef {
    pub id: AudioEffectId,
    pub enabled: bool,
    pub kind: AudioEffectKind,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AudioEffectKind {
    LowPass {
        cutoff_milli_hz: u64,
        q_milli: u32,
    },
    HighPass {
        cutoff_milli_hz: u64,
        q_milli: u32,
    },
    Compressor {
        threshold_db_milli: i32,
        ratio_milli: u32,
        attack_micros: u32,
        release_micros: u32,
        makeup_db_milli: i32,
    },
    Delay {
        time_millis: u32,
        feedback_milli: u16,
        wet_db_milli: i32,
        dry_db_milli: i32,
    },
    Reverb {
        room_size_milli: u16,
        damping_milli: u16,
        wet_db_milli: i32,
        dry_db_milli: i32,
    },
    Limiter {
        ceiling_db_milli: i32,
        release_micros: u32,
    },
}

impl AudioEffectKind {
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::LowPass { .. } => "low_pass",
            Self::HighPass { .. } => "high_pass",
            Self::Compressor { .. } => "compressor",
            Self::Delay { .. } => "delay",
            Self::Reverb { .. } => "reverb",
            Self::Limiter { .. } => "limiter",
        }
    }

    #[allow(clippy::too_many_lines)]
    pub fn validate(&self) -> Result<(), AudioGraphError> {
        match self {
            Self::LowPass {
                cutoff_milli_hz,
                q_milli,
            }
            | Self::HighPass {
                cutoff_milli_hz,
                q_milli,
            } => {
                if *cutoff_milli_hz < 1_000 || *q_milli == 0 {
                    return Err(AudioGraphError::InvalidEffect {
                        effect: self.name(),
                        message: "cutoff and Q must be positive".to_owned(),
                    });
                }
            }
            Self::Compressor {
                threshold_db_milli,
                ratio_milli,
                attack_micros,
                release_micros,
                makeup_db_milli,
            } => {
                GainDbMilli::new(*threshold_db_milli).map_err(|error| {
                    AudioGraphError::InvalidEffect {
                        effect: self.name(),
                        message: error.to_string(),
                    }
                })?;
                GainDbMilli::new(*makeup_db_milli).map_err(|error| {
                    AudioGraphError::InvalidEffect {
                        effect: self.name(),
                        message: error.to_string(),
                    }
                })?;
                if *ratio_milli < 1_000 || *attack_micros == 0 || *release_micros == 0 {
                    return Err(AudioGraphError::InvalidEffect {
                        effect: self.name(),
                        message: "ratio must be >= 1000 and attack/release must be non-zero"
                            .to_owned(),
                    });
                }
            }
            Self::Delay {
                time_millis,
                feedback_milli,
                wet_db_milli,
                dry_db_milli,
            } => {
                GainDbMilli::new(*wet_db_milli).map_err(|error| {
                    AudioGraphError::InvalidEffect {
                        effect: self.name(),
                        message: error.to_string(),
                    }
                })?;
                GainDbMilli::new(*dry_db_milli).map_err(|error| {
                    AudioGraphError::InvalidEffect {
                        effect: self.name(),
                        message: error.to_string(),
                    }
                })?;
                if *time_millis == 0 || *feedback_milli >= 1_000 {
                    return Err(AudioGraphError::InvalidEffect {
                        effect: self.name(),
                        message: "time must be non-zero and feedback must be below 1000".to_owned(),
                    });
                }
            }
            Self::Reverb {
                room_size_milli,
                damping_milli,
                wet_db_milli,
                dry_db_milli,
            } => {
                GainDbMilli::new(*wet_db_milli).map_err(|error| {
                    AudioGraphError::InvalidEffect {
                        effect: self.name(),
                        message: error.to_string(),
                    }
                })?;
                GainDbMilli::new(*dry_db_milli).map_err(|error| {
                    AudioGraphError::InvalidEffect {
                        effect: self.name(),
                        message: error.to_string(),
                    }
                })?;
                if *room_size_milli > 1_000 || *damping_milli > 1_000 {
                    return Err(AudioGraphError::InvalidEffect {
                        effect: self.name(),
                        message: "room size and damping must be <= 1000".to_owned(),
                    });
                }
            }
            Self::Limiter {
                ceiling_db_milli,
                release_micros,
            } => {
                GainDbMilli::new(*ceiling_db_milli).map_err(|error| {
                    AudioGraphError::InvalidEffect {
                        effect: self.name(),
                        message: error.to_string(),
                    }
                })?;
                if *release_micros == 0 {
                    return Err(AudioGraphError::InvalidEffect {
                        effect: self.name(),
                        message: "release must be non-zero".to_owned(),
                    });
                }
            }
        }
        Ok(())
    }

    #[must_use]
    pub const fn supports_parameter(&self, parameter: AudioEffectParameter) -> bool {
        match self {
            Self::LowPass { .. } | Self::HighPass { .. } => matches!(
                parameter,
                AudioEffectParameter::BiquadCutoffMilliHz(_)
                    | AudioEffectParameter::BiquadQMilli(_)
            ),
            Self::Compressor { .. } => matches!(
                parameter,
                AudioEffectParameter::CompressorThresholdDbMilli(_)
                    | AudioEffectParameter::CompressorRatioMilli(_)
                    | AudioEffectParameter::CompressorAttackMicros(_)
                    | AudioEffectParameter::CompressorReleaseMicros(_)
                    | AudioEffectParameter::CompressorMakeupDbMilli(_)
            ),
            Self::Delay { .. } => matches!(
                parameter,
                AudioEffectParameter::DelayTimeMillis(_)
                    | AudioEffectParameter::DelayFeedbackMilli(_)
                    | AudioEffectParameter::WetGainDbMilli(_)
                    | AudioEffectParameter::DryGainDbMilli(_)
            ),
            Self::Reverb { .. } => matches!(
                parameter,
                AudioEffectParameter::ReverbRoomSizeMilli(_)
                    | AudioEffectParameter::ReverbDampingMilli(_)
                    | AudioEffectParameter::WetGainDbMilli(_)
                    | AudioEffectParameter::DryGainDbMilli(_)
            ),
            Self::Limiter { .. } => matches!(
                parameter,
                AudioEffectParameter::LimiterCeilingDbMilli(_)
                    | AudioEffectParameter::LimiterReleaseMicros(_)
            ),
        }
    }

    #[allow(clippy::too_many_lines)]
    pub fn apply_parameter(
        &mut self,
        parameter: AudioEffectParameter,
    ) -> Result<(), AudioGraphError> {
        let parameter = parameter
            .validate()
            .map_err(|error| AudioGraphError::InvalidEffect {
                effect: self.name(),
                message: error.to_string(),
            })?;
        if !self.supports_parameter(parameter) {
            return Err(AudioGraphError::UnsupportedEffectParameter {
                effect: self.name(),
                parameter: parameter.name(),
            });
        }
        match (&mut *self, parameter) {
            (
                Self::LowPass {
                    cutoff_milli_hz, ..
                }
                | Self::HighPass {
                    cutoff_milli_hz, ..
                },
                AudioEffectParameter::BiquadCutoffMilliHz(value),
            ) => {
                *cutoff_milli_hz = value;
            }
            (
                Self::LowPass { q_milli, .. } | Self::HighPass { q_milli, .. },
                AudioEffectParameter::BiquadQMilli(value),
            ) => {
                *q_milli = value;
            }
            (
                Self::Compressor {
                    threshold_db_milli, ..
                },
                AudioEffectParameter::CompressorThresholdDbMilli(value),
            ) => {
                *threshold_db_milli = value;
            }
            (
                Self::Compressor { ratio_milli, .. },
                AudioEffectParameter::CompressorRatioMilli(value),
            ) => {
                *ratio_milli = value;
            }
            (
                Self::Compressor { attack_micros, .. },
                AudioEffectParameter::CompressorAttackMicros(value),
            ) => {
                *attack_micros = value;
            }
            (
                Self::Compressor { release_micros, .. },
                AudioEffectParameter::CompressorReleaseMicros(value),
            )
            | (
                Self::Limiter { release_micros, .. },
                AudioEffectParameter::LimiterReleaseMicros(value),
            ) => {
                *release_micros = value;
            }
            (
                Self::Compressor {
                    makeup_db_milli, ..
                },
                AudioEffectParameter::CompressorMakeupDbMilli(value),
            ) => {
                *makeup_db_milli = value;
            }
            (Self::Delay { time_millis, .. }, AudioEffectParameter::DelayTimeMillis(value)) => {
                *time_millis = value;
            }
            (
                Self::Delay { feedback_milli, .. },
                AudioEffectParameter::DelayFeedbackMilli(value),
            ) => {
                *feedback_milli = value;
            }
            (
                Self::Reverb {
                    room_size_milli, ..
                },
                AudioEffectParameter::ReverbRoomSizeMilli(value),
            ) => {
                *room_size_milli = value;
            }
            (
                Self::Reverb { damping_milli, .. },
                AudioEffectParameter::ReverbDampingMilli(value),
            ) => {
                *damping_milli = value;
            }
            (
                Self::Delay { wet_db_milli, .. } | Self::Reverb { wet_db_milli, .. },
                AudioEffectParameter::WetGainDbMilli(value),
            ) => {
                *wet_db_milli = value;
            }
            (
                Self::Delay { dry_db_milli, .. } | Self::Reverb { dry_db_milli, .. },
                AudioEffectParameter::DryGainDbMilli(value),
            ) => {
                *dry_db_milli = value;
            }
            (
                Self::Limiter {
                    ceiling_db_milli, ..
                },
                AudioEffectParameter::LimiterCeilingDbMilli(value),
            ) => {
                *ceiling_db_milli = value;
            }
            _ => unreachable!("parameter support was checked above"),
        }
        self.validate()
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AudioSnapshotDef {
    pub id: AudioSnapshotId,
    pub bus_gains: Vec<SnapshotBusGain>,
    pub effect_parameters: Vec<SnapshotEffectParameter>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SnapshotBusGain {
    pub bus: AudioBusId,
    pub gain: GainDbMilli,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SnapshotEffectParameter {
    pub bus: AudioBusId,
    pub effect: AudioEffectId,
    pub parameter: AudioEffectParameter,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AudioGraph {
    pub master_bus: AudioBusId,
    pub assets: Vec<AudioAsset>,
    pub buses: Vec<AudioBusDef>,
    pub snapshots: Vec<AudioSnapshotDef>,
}

impl AudioGraph {
    #[allow(clippy::too_many_lines)]
    pub fn validate(&self) -> Result<(), AudioGraphError> {
        if self.buses.is_empty() {
            return Err(AudioGraphError::MissingMasterBus);
        }
        if self.buses.len() > MAX_BUSES {
            return Err(AudioGraphError::BusLimit {
                actual: self.buses.len(),
                maximum: MAX_BUSES,
            });
        }

        let mut asset_ids = BTreeSet::new();
        for asset in &self.assets {
            if !asset_ids.insert(asset.id.clone()) {
                return Err(AudioGraphError::DuplicateResource(asset.id.clone()));
            }
            asset.validate()?;
        }

        let mut bus_slots = BTreeMap::new();
        for (slot, bus) in self.buses.iter().enumerate() {
            if bus_slots.insert(bus.id.clone(), slot).is_some() {
                return Err(AudioGraphError::DuplicateBus(bus.id.clone()));
            }
            if bus.effects.len() > MAX_EFFECTS_PER_BUS {
                return Err(AudioGraphError::EffectLimit {
                    bus: bus.id.clone(),
                    actual: bus.effects.len(),
                    maximum: MAX_EFFECTS_PER_BUS,
                });
            }
            let mut effect_ids = BTreeSet::new();
            for effect in &bus.effects {
                if !effect_ids.insert(effect.id.clone()) {
                    return Err(AudioGraphError::DuplicateEffect {
                        bus: bus.id.clone(),
                        effect: effect.id.clone(),
                    });
                }
                effect.kind.validate()?;
            }
        }

        let Some(&master_slot) = bus_slots.get(&self.master_bus) else {
            return Err(AudioGraphError::UnknownMasterBus(self.master_bus.clone()));
        };
        if self.buses[master_slot].parent.is_some() {
            return Err(AudioGraphError::MasterHasParent(self.master_bus.clone()));
        }
        for (slot, bus) in self.buses.iter().enumerate() {
            if let Some(parent) = &bus.parent {
                let Some(&parent_slot) = bus_slots.get(parent) else {
                    return Err(AudioGraphError::UnknownParent {
                        bus: bus.id.clone(),
                        parent: parent.clone(),
                    });
                };
                if parent_slot >= slot {
                    return Err(AudioGraphError::NonTopologicalBus {
                        bus: bus.id.clone(),
                        parent: parent.clone(),
                    });
                }
            } else if slot != master_slot {
                return Err(AudioGraphError::DisconnectedBus(bus.id.clone()));
            }
        }

        let mut snapshot_ids = BTreeSet::new();
        for snapshot in &self.snapshots {
            if !snapshot_ids.insert(snapshot.id.clone()) {
                return Err(AudioGraphError::DuplicateSnapshot(snapshot.id.clone()));
            }
            for entry in &snapshot.bus_gains {
                if !bus_slots.contains_key(&entry.bus) {
                    return Err(AudioGraphError::UnknownSnapshotBus {
                        snapshot: snapshot.id.clone(),
                        bus: entry.bus.clone(),
                    });
                }
            }
            for entry in &snapshot.effect_parameters {
                let Some(bus) = self.buses.iter().find(|bus| bus.id == entry.bus) else {
                    return Err(AudioGraphError::UnknownSnapshotBus {
                        snapshot: snapshot.id.clone(),
                        bus: entry.bus.clone(),
                    });
                };
                let Some(effect) = bus.effects.iter().find(|effect| effect.id == entry.effect)
                else {
                    return Err(AudioGraphError::UnknownSnapshotEffect {
                        snapshot: snapshot.id.clone(),
                        bus: entry.bus.clone(),
                        effect: entry.effect.clone(),
                    });
                };
                let mut kind = effect.kind.clone();
                kind.apply_parameter(entry.parameter)?;
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum AudioGraphError {
    #[error("audio graph requires a master bus")]
    MissingMasterBus,
    #[error("audio graph contains {actual} buses; maximum is {maximum}")]
    BusLimit { actual: usize, maximum: usize },
    #[error("audio mixer voice capacity must be greater than zero")]
    InvalidVoiceLimit,
    #[error("audio bus `{bus}` contains {actual} effects; maximum is {maximum}", bus = bus.as_str())]
    EffectLimit {
        bus: AudioBusId,
        actual: usize,
        maximum: usize,
    },
    #[error("duplicate audio resource `{resource}`", resource = .0.as_str())]
    DuplicateResource(AudioResourceId),
    #[error("duplicate audio bus `{bus}`", bus = .0.as_str())]
    DuplicateBus(AudioBusId),
    #[error("duplicate effect `{effect}` on bus `{bus}`", bus = bus.as_str(), effect = effect.as_str())]
    DuplicateEffect {
        bus: AudioBusId,
        effect: AudioEffectId,
    },
    #[error("duplicate audio snapshot `{snapshot}`", snapshot = .0.as_str())]
    DuplicateSnapshot(AudioSnapshotId),
    #[error("audio asset `{asset}` is invalid: {message}", asset = asset.as_str())]
    InvalidAsset {
        asset: AudioResourceId,
        message: String,
    },
    #[error("audio effect `{effect}` is invalid: {message}")]
    InvalidEffect {
        effect: &'static str,
        message: String,
    },
    #[error("invalid decoded audio: {0}")]
    InvalidDecodedAudio(String),
    #[error("effect `{effect}` does not support parameter `{parameter}`")]
    UnsupportedEffectParameter {
        effect: &'static str,
        parameter: &'static str,
    },
    #[error("unknown master audio bus `{bus}`", bus = .0.as_str())]
    UnknownMasterBus(AudioBusId),
    #[error("master audio bus `{bus}` must not have a parent", bus = .0.as_str())]
    MasterHasParent(AudioBusId),
    #[error("audio bus `{bus}` is disconnected from the master", bus = .0.as_str())]
    DisconnectedBus(AudioBusId),
    #[error("audio bus `{bus}` references unknown parent `{parent}`", bus = bus.as_str(), parent = parent.as_str())]
    UnknownParent { bus: AudioBusId, parent: AudioBusId },
    #[error("audio bus `{bus}` must appear after its parent `{parent}`", bus = bus.as_str(), parent = parent.as_str())]
    NonTopologicalBus { bus: AudioBusId, parent: AudioBusId },
    #[error("snapshot `{snapshot}` references unknown bus `{bus}`", snapshot = snapshot.as_str(), bus = bus.as_str())]
    UnknownSnapshotBus {
        snapshot: AudioSnapshotId,
        bus: AudioBusId,
    },
    #[error("snapshot `{snapshot}` references unknown effect `{effect}` on bus `{bus}`", snapshot = snapshot.as_str(), bus = bus.as_str(), effect = effect.as_str())]
    UnknownSnapshotEffect {
        snapshot: AudioSnapshotId,
        bus: AudioBusId,
        effect: AudioEffectId,
    },
}
