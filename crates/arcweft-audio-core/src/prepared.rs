//! Decoded PCM and prepared command structures used by the realtime mixer.

use std::collections::BTreeMap;
use std::sync::Arc;

use arcweft_interaction_model::audio::{
    AudioBusId, AudioCommand, AudioDispatchId, AudioEffectId, AudioEffectParameter, AudioEvent,
    AudioFailure, AudioLoopMode, AudioResourceId, AudioSnapshotId, AudioVoiceId, GainDbMilli,
    PanMilli,
};

use crate::graph::{AudioEffectKind, AudioGraph, AudioGraphError};

#[derive(Clone, Debug)]
pub struct DecodedAudio {
    sample_rate_hz: u32,
    channels: u16,
    samples: Arc<[f32]>,
}

impl DecodedAudio {
    pub fn new(
        sample_rate_hz: u32,
        channels: u16,
        samples: impl Into<Arc<[f32]>>,
    ) -> Result<Self, AudioGraphError> {
        let samples = samples.into();
        if sample_rate_hz == 0 {
            return Err(AudioGraphError::InvalidDecodedAudio(
                "sample rate must be non-zero".to_owned(),
            ));
        }
        if channels == 0 || channels > 2 {
            return Err(AudioGraphError::InvalidDecodedAudio(format!(
                "decoded audio must be mono or stereo, found {channels} channels"
            )));
        }
        if samples.len() % usize::from(channels) != 0 {
            return Err(AudioGraphError::InvalidDecodedAudio(format!(
                "{} samples are not divisible by {channels} channels",
                samples.len()
            )));
        }
        Ok(Self {
            sample_rate_hz,
            channels,
            samples,
        })
    }

    pub const fn sample_rate_hz(&self) -> u32 {
        self.sample_rate_hz
    }

    pub const fn channels(&self) -> u16 {
        self.channels
    }

    pub fn samples(&self) -> &[f32] {
        &self.samples
    }

    pub fn frame_count(&self) -> u64 {
        (self.samples.len() / usize::from(self.channels)) as u64
    }
}

#[derive(Clone, Debug)]
pub struct AudioDispatch {
    pub id: AudioDispatchId,
    pub command: AudioCommand,
}

pub trait AudioHost {
    type Error;

    fn submit(&mut self, dispatch: AudioDispatch) -> Result<(), Self::Error>;
    fn drain_events(&mut self, events: &mut Vec<AudioEvent>);
}

#[derive(Clone, Debug)]
pub struct PreparedAudioGraph {
    pub buses: Vec<PreparedBus>,
    pub snapshots: Vec<PreparedSnapshot>,
    pub resource_slots: usize,
    pub max_voices: usize,
}

#[derive(Clone, Debug)]
pub struct PreparedBus {
    pub id: AudioBusId,
    pub parent: Option<usize>,
    pub gain: GainDbMilli,
    pub muted: bool,
    pub effects: Vec<PreparedEffect>,
}

#[derive(Clone, Debug)]
pub struct PreparedEffect {
    pub id: AudioEffectId,
    pub enabled: bool,
    pub kind: AudioEffectKind,
}

#[derive(Clone, Debug)]
pub struct PreparedSnapshot {
    pub id: AudioSnapshotId,
    pub bus_gains: Vec<(usize, GainDbMilli)>,
    pub effect_parameters: Vec<(usize, usize, AudioEffectParameter)>,
}

#[derive(Clone, Debug)]
pub enum PreparedAudioCommand {
    InstallResource {
        slot: usize,
        resource: AudioResourceId,
        audio: Arc<DecodedAudio>,
    },
    Play {
        dispatch: AudioDispatchId,
        voice_slot: usize,
        voice: AudioVoiceId,
        resource_slot: usize,
        bus_slot: usize,
        gain: GainDbMilli,
        pan: PanMilli,
        loop_mode: AudioLoopMode,
        start_frame: u64,
        fade_in_millis: u32,
    },
    Stop {
        dispatch: AudioDispatchId,
        voice_slot: usize,
        fade_out_millis: u32,
    },
    StopAll {
        dispatch: AudioDispatchId,
        fade_out_millis: u32,
    },
    SetVoiceGain {
        dispatch: AudioDispatchId,
        voice_slot: usize,
        gain: GainDbMilli,
        transition_millis: u32,
    },
    SetVoicePan {
        dispatch: AudioDispatchId,
        voice_slot: usize,
        pan: PanMilli,
        transition_millis: u32,
    },
    SetBusGain {
        dispatch: AudioDispatchId,
        bus_slot: usize,
        gain: GainDbMilli,
        transition_millis: u32,
    },
    SetBusMute {
        dispatch: AudioDispatchId,
        bus_slot: usize,
        muted: bool,
    },
    SetEffectEnabled {
        dispatch: AudioDispatchId,
        bus_slot: usize,
        effect_slot: usize,
        enabled: bool,
    },
    SetEffectParameter {
        dispatch: AudioDispatchId,
        bus_slot: usize,
        effect_slot: usize,
        parameter: AudioEffectParameter,
        transition_millis: u32,
    },
    ApplySnapshot {
        dispatch: AudioDispatchId,
        snapshot_slot: usize,
        transition_millis: u32,
    },
}

impl PreparedAudioCommand {
    pub const fn dispatch(&self) -> Option<AudioDispatchId> {
        match self {
            Self::InstallResource { .. } => None,
            Self::Play { dispatch, .. }
            | Self::Stop { dispatch, .. }
            | Self::StopAll { dispatch, .. }
            | Self::SetVoiceGain { dispatch, .. }
            | Self::SetVoicePan { dispatch, .. }
            | Self::SetBusGain { dispatch, .. }
            | Self::SetBusMute { dispatch, .. }
            | Self::SetEffectEnabled { dispatch, .. }
            | Self::SetEffectParameter { dispatch, .. }
            | Self::ApplySnapshot { dispatch, .. } => Some(*dispatch),
        }
    }
}

pub struct AudioCommandPreparer {
    resources: BTreeMap<AudioResourceId, usize>,
    buses: BTreeMap<AudioBusId, usize>,
    effects: BTreeMap<(AudioBusId, AudioEffectId), usize>,
    snapshots: BTreeMap<AudioSnapshotId, usize>,
    voices: BTreeMap<AudioVoiceId, usize>,
    free_voice_slots: Vec<usize>,
    max_voices: usize,
}

impl AudioGraph {
    pub fn prepare(
        &self,
        max_voices: usize,
    ) -> Result<(PreparedAudioGraph, AudioCommandPreparer), AudioGraphError> {
        self.validate()?;
        if max_voices == 0 {
            return Err(AudioGraphError::InvalidVoiceLimit);
        }

        let bus_slots = self
            .buses
            .iter()
            .enumerate()
            .map(|(index, bus)| (bus.id.clone(), index))
            .collect::<BTreeMap<_, _>>();
        let buses = self
            .buses
            .iter()
            .map(|bus| PreparedBus {
                id: bus.id.clone(),
                parent: bus.parent.as_ref().map(|parent| bus_slots[parent]),
                gain: bus.gain,
                muted: bus.muted,
                effects: bus
                    .effects
                    .iter()
                    .map(|effect| PreparedEffect {
                        id: effect.id.clone(),
                        enabled: effect.enabled,
                        kind: effect.kind.clone(),
                    })
                    .collect(),
            })
            .collect::<Vec<_>>();

        let effects = self
            .buses
            .iter()
            .flat_map(|bus| {
                bus.effects
                    .iter()
                    .enumerate()
                    .map(|(index, effect)| ((bus.id.clone(), effect.id.clone()), index))
            })
            .collect::<BTreeMap<_, _>>();
        let snapshots = self
            .snapshots
            .iter()
            .map(|snapshot| PreparedSnapshot {
                id: snapshot.id.clone(),
                bus_gains: snapshot
                    .bus_gains
                    .iter()
                    .map(|entry| (bus_slots[&entry.bus], entry.gain))
                    .collect(),
                effect_parameters: snapshot
                    .effect_parameters
                    .iter()
                    .map(|entry| {
                        (
                            bus_slots[&entry.bus],
                            effects[&(entry.bus.clone(), entry.effect.clone())],
                            entry.parameter,
                        )
                    })
                    .collect(),
            })
            .collect::<Vec<_>>();
        let resources = self
            .assets
            .iter()
            .enumerate()
            .map(|(index, asset)| (asset.id.clone(), index))
            .collect::<BTreeMap<_, _>>();
        let snapshot_slots = self
            .snapshots
            .iter()
            .enumerate()
            .map(|(index, snapshot)| (snapshot.id.clone(), index))
            .collect::<BTreeMap<_, _>>();

        Ok((
            PreparedAudioGraph {
                buses,
                snapshots,
                resource_slots: self.assets.len(),
                max_voices,
            },
            AudioCommandPreparer {
                resources,
                buses: bus_slots,
                effects,
                snapshots: snapshot_slots,
                voices: BTreeMap::new(),
                free_voice_slots: (0..max_voices).rev().collect(),
                max_voices,
            },
        ))
    }
}

impl AudioCommandPreparer {
    pub fn install_resource(
        &self,
        resource: &AudioResourceId,
        audio: Arc<DecodedAudio>,
    ) -> Result<PreparedAudioCommand, AudioFailure> {
        self.resources
            .get(resource)
            .copied()
            .map(|slot| PreparedAudioCommand::InstallResource {
                slot,
                resource: resource.clone(),
                audio,
            })
            .ok_or_else(|| AudioFailure::UnknownResource {
                resource: resource.clone(),
            })
    }

    #[allow(clippy::too_many_lines)]
    pub fn prepare(
        &mut self,
        dispatch: AudioDispatch,
    ) -> Result<PreparedAudioCommand, AudioFailure> {
        match dispatch.command {
            AudioCommand::Play {
                voice,
                resource,
                bus,
                gain,
                pan,
                loop_mode,
                start_frame,
                fade_in,
            } => {
                let resource_slot = self.resources.get(&resource).copied().ok_or_else(|| {
                    AudioFailure::UnknownResource {
                        resource: resource.clone(),
                    }
                })?;
                let bus_slot = self
                    .buses
                    .get(&bus)
                    .copied()
                    .ok_or_else(|| AudioFailure::UnknownBus { bus: bus.clone() })?;
                let voice_slot = if let Some(slot) = self.voices.get(&voice).copied() {
                    slot
                } else {
                    let Some(slot) = self.free_voice_slots.pop() else {
                        return Err(AudioFailure::VoiceLimit {
                            maximum: self.max_voices,
                        });
                    };
                    self.voices.insert(voice.clone(), slot);
                    slot
                };
                Ok(PreparedAudioCommand::Play {
                    dispatch: dispatch.id,
                    voice_slot,
                    voice,
                    resource_slot,
                    bus_slot,
                    gain,
                    pan,
                    loop_mode,
                    start_frame,
                    fade_in_millis: fade_in.get(),
                })
            }
            AudioCommand::Stop { voice, fade_out } => self
                .voices
                .get(&voice)
                .copied()
                .map(|voice_slot| PreparedAudioCommand::Stop {
                    dispatch: dispatch.id,
                    voice_slot,
                    fade_out_millis: fade_out.get(),
                })
                .ok_or(AudioFailure::UnknownVoice { voice }),
            AudioCommand::StopAll { fade_out } => Ok(PreparedAudioCommand::StopAll {
                dispatch: dispatch.id,
                fade_out_millis: fade_out.get(),
            }),
            AudioCommand::SetVoiceGain {
                voice,
                gain,
                transition,
            } => self
                .voices
                .get(&voice)
                .copied()
                .map(|voice_slot| PreparedAudioCommand::SetVoiceGain {
                    dispatch: dispatch.id,
                    voice_slot,
                    gain,
                    transition_millis: transition.get(),
                })
                .ok_or(AudioFailure::UnknownVoice { voice }),
            AudioCommand::SetVoicePan {
                voice,
                pan,
                transition,
            } => self
                .voices
                .get(&voice)
                .copied()
                .map(|voice_slot| PreparedAudioCommand::SetVoicePan {
                    dispatch: dispatch.id,
                    voice_slot,
                    pan,
                    transition_millis: transition.get(),
                })
                .ok_or(AudioFailure::UnknownVoice { voice }),
            AudioCommand::SetBusGain {
                bus,
                gain,
                transition,
            } => self
                .buses
                .get(&bus)
                .copied()
                .map(|bus_slot| PreparedAudioCommand::SetBusGain {
                    dispatch: dispatch.id,
                    bus_slot,
                    gain,
                    transition_millis: transition.get(),
                })
                .ok_or(AudioFailure::UnknownBus { bus }),
            AudioCommand::SetBusMute { bus, muted } => self
                .buses
                .get(&bus)
                .copied()
                .map(|bus_slot| PreparedAudioCommand::SetBusMute {
                    dispatch: dispatch.id,
                    bus_slot,
                    muted,
                })
                .ok_or(AudioFailure::UnknownBus { bus }),
            AudioCommand::SetEffectEnabled {
                bus,
                effect,
                enabled,
            } => self.prepare_effect(bus, effect, |bus_slot, effect_slot| {
                PreparedAudioCommand::SetEffectEnabled {
                    dispatch: dispatch.id,
                    bus_slot,
                    effect_slot,
                    enabled,
                }
            }),
            AudioCommand::SetEffectParameter {
                bus,
                effect,
                parameter,
                transition,
            } => self.prepare_effect(bus, effect, |bus_slot, effect_slot| {
                PreparedAudioCommand::SetEffectParameter {
                    dispatch: dispatch.id,
                    bus_slot,
                    effect_slot,
                    parameter,
                    transition_millis: transition.get(),
                }
            }),
            AudioCommand::ApplySnapshot {
                snapshot,
                transition,
            } => self
                .snapshots
                .get(&snapshot)
                .copied()
                .map(|snapshot_slot| PreparedAudioCommand::ApplySnapshot {
                    dispatch: dispatch.id,
                    snapshot_slot,
                    transition_millis: transition.get(),
                })
                .ok_or(AudioFailure::UnknownSnapshot { snapshot }),
            AudioCommand::RequestMicrophone { .. }
            | AudioCommand::StopMicrophone { .. }
            | AudioCommand::SetCaptureMonitor { .. } => Err(AudioFailure::Backend {
                message: "microphone command must be handled by the device coordinator".to_owned(),
            }),
        }
    }

    pub fn observe_event(&mut self, event: &AudioEvent) {
        if let AudioEvent::PlaybackEnded { voice, .. } = event
            && let Some(slot) = self.voices.remove(voice)
        {
            self.free_voice_slots.push(slot);
        }
    }

    fn prepare_effect(
        &self,
        bus: AudioBusId,
        effect: AudioEffectId,
        make: impl FnOnce(usize, usize) -> PreparedAudioCommand,
    ) -> Result<PreparedAudioCommand, AudioFailure> {
        let Some(bus_slot) = self.buses.get(&bus).copied() else {
            return Err(AudioFailure::UnknownBus { bus });
        };
        self.effects
            .get(&(bus.clone(), effect.clone()))
            .copied()
            .map(|effect_slot| make(bus_slot, effect_slot))
            .ok_or(AudioFailure::UnknownEffect { bus, effect })
    }
}
