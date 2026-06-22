//! Shared stereo Arcweft mixer. It owns no device handle or decoder.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::too_many_lines
)]

mod effect;

use arcweft_audio_core::{
    DecodedAudio, PreparedAudioCommand, PreparedAudioGraph, PreparedBus, PreparedSnapshot,
};
use arcweft_interaction_model::audio::{
    AudioDispatchId, AudioEvent, AudioFailure, AudioLoopMode, AudioPlaybackEndReason,
    AudioResourceId, AudioVoiceId, GainDbMilli,
};
use effect::Effect;
use std::sync::Arc;
use thiserror::Error;

pub struct Mixer {
    sample_rate_hz: u32,
    max_callback_frames: usize,
    resources: Vec<Option<ResourceSlot>>,
    voices: Vec<Option<Voice>>,
    buses: Vec<Bus>,
    snapshots: Vec<PreparedSnapshot>,
    scratch: Vec<Vec<f32>>,
    xrun_count: u64,
}

struct ResourceSlot {
    id: AudioResourceId,
    audio: Arc<DecodedAudio>,
}

struct Voice {
    playback: AudioDispatchId,
    id: AudioVoiceId,
    resource_slot: usize,
    bus_slot: usize,
    cursor: u64,
    loop_mode: AudioLoopMode,
    gain: SmoothedValue,
    pan: SmoothedValue,
    stop_reason: Option<AudioPlaybackEndReason>,
}

struct Bus {
    parent: Option<usize>,
    gain: SmoothedValue,
    muted: bool,
    effects: Vec<EffectSlot>,
}

struct EffectSlot {
    enabled: bool,
    effect: Effect,
}

#[derive(Clone, Copy)]
struct SmoothedValue {
    current: f32,
    target: f32,
    step: f32,
    remaining: u32,
}

impl SmoothedValue {
    fn immediate(value: f32) -> Self {
        Self {
            current: value,
            target: value,
            step: 0.0,
            remaining: 0,
        }
    }

    fn set_target(&mut self, target: f32, frames: u32) {
        self.target = target;
        if frames == 0 {
            self.current = target;
            self.step = 0.0;
            self.remaining = 0;
        } else {
            self.step = (target - self.current) / frames as f32;
            self.remaining = frames;
        }
    }

    fn next(&mut self) -> f32 {
        if self.remaining > 0 {
            self.current += self.step;
            self.remaining -= 1;
            if self.remaining == 0 {
                self.current = self.target;
            }
        }
        self.current
    }

    fn settled_at_zero(self) -> bool {
        self.remaining == 0 && self.current.abs() <= 0.000_001
    }
}

impl Mixer {
    pub fn new(
        graph: PreparedAudioGraph,
        sample_rate_hz: u32,
        max_callback_frames: usize,
    ) -> Result<Self, MixerError> {
        if sample_rate_hz == 0 || max_callback_frames == 0 {
            return Err(MixerError::InvalidConfiguration(
                "sample rate and callback frame capacity must be non-zero".to_owned(),
            ));
        }
        let buses = graph
            .buses
            .iter()
            .map(|bus| Bus::new(bus, sample_rate_hz))
            .collect::<Vec<_>>();
        let scratch = (0..buses.len())
            .map(|_| vec![0.0; max_callback_frames * 2])
            .collect();
        Ok(Self {
            sample_rate_hz,
            max_callback_frames,
            resources: (0..graph.resource_slots).map(|_| None).collect(),
            voices: (0..graph.max_voices).map(|_| None).collect(),
            buses,
            snapshots: graph.snapshots,
            scratch,
            xrun_count: 0,
        })
    }

    pub fn apply(
        &mut self,
        command: PreparedAudioCommand,
        mut emit: impl FnMut(AudioEvent),
    ) -> Result<(), MixerError> {
        match command {
            PreparedAudioCommand::InstallResource {
                slot,
                resource,
                audio,
            } => {
                if audio.sample_rate_hz() != self.sample_rate_hz {
                    return Err(MixerError::InvalidConfiguration(format!(
                        "resource `{}` has {} Hz PCM but mixer runs at {} Hz",
                        resource.as_str(),
                        audio.sample_rate_hz(),
                        self.sample_rate_hz
                    )));
                }
                let Some(target) = self.resources.get_mut(slot) else {
                    return Err(MixerError::InvalidSlot("resource", slot));
                };
                *target = Some(ResourceSlot {
                    id: resource,
                    audio,
                });
            }
            PreparedAudioCommand::Play {
                dispatch,
                voice_slot,
                voice,
                resource_slot,
                bus_slot,
                gain,
                pan,
                loop_mode,
                start_frame,
                fade_in_millis,
            } => {
                let resource = self
                    .resources
                    .get(resource_slot)
                    .and_then(Option::as_ref)
                    .ok_or(MixerError::MissingResource(resource_slot))?;
                if bus_slot >= self.buses.len() {
                    return Err(MixerError::InvalidSlot("bus", bus_slot));
                }
                validate_play_range(resource, start_frame, loop_mode)?;
                let Some(target) = self.voices.get_mut(voice_slot) else {
                    return Err(MixerError::InvalidSlot("voice", voice_slot));
                };
                if let Some(previous) = target.take() {
                    emit(AudioEvent::PlaybackEnded {
                        playback: previous.playback,
                        voice: previous.id,
                        reason: AudioPlaybackEndReason::Replaced,
                    });
                }
                let target_gain = db_to_linear(gain);
                let mut smoothed_gain = SmoothedValue::immediate(if fade_in_millis == 0 {
                    target_gain
                } else {
                    0.0
                });
                smoothed_gain.set_target(
                    target_gain,
                    millis_to_frames(fade_in_millis, self.sample_rate_hz),
                );
                *target = Some(Voice {
                    playback: dispatch,
                    id: voice,
                    resource_slot,
                    bus_slot,
                    cursor: start_frame,
                    loop_mode,
                    gain: smoothed_gain,
                    pan: SmoothedValue::immediate(f32::from(pan.get()) / 1_000.0),
                    stop_reason: None,
                });
            }
            PreparedAudioCommand::Stop {
                voice_slot,
                fade_out_millis,
                ..
            } => self.stop_voice(
                voice_slot,
                fade_out_millis,
                AudioPlaybackEndReason::Stopped,
                &mut emit,
            )?,
            PreparedAudioCommand::StopAll {
                fade_out_millis, ..
            } => {
                for slot in 0..self.voices.len() {
                    self.stop_voice(
                        slot,
                        fade_out_millis,
                        AudioPlaybackEndReason::StopAll,
                        &mut emit,
                    )?;
                }
            }
            PreparedAudioCommand::SetVoiceGain {
                voice_slot,
                gain,
                transition_millis,
                ..
            } => {
                let sample_rate_hz = self.sample_rate_hz;
                let voice = self.voice_mut(voice_slot)?;
                voice.gain.set_target(
                    db_to_linear(gain),
                    millis_to_frames(transition_millis, sample_rate_hz),
                );
            }
            PreparedAudioCommand::SetVoicePan {
                voice_slot,
                pan,
                transition_millis,
                ..
            } => {
                let sample_rate_hz = self.sample_rate_hz;
                let voice = self.voice_mut(voice_slot)?;
                voice.pan.set_target(
                    f32::from(pan.get()) / 1_000.0,
                    millis_to_frames(transition_millis, sample_rate_hz),
                );
            }
            PreparedAudioCommand::SetBusGain {
                bus_slot,
                gain,
                transition_millis,
                ..
            } => {
                let sample_rate_hz = self.sample_rate_hz;
                let bus = self.bus_mut(bus_slot)?;
                bus.gain.set_target(
                    db_to_linear(gain),
                    millis_to_frames(transition_millis, sample_rate_hz),
                );
            }
            PreparedAudioCommand::SetBusMute {
                bus_slot, muted, ..
            } => self.bus_mut(bus_slot)?.muted = muted,
            PreparedAudioCommand::SetEffectEnabled {
                bus_slot,
                effect_slot,
                enabled,
                ..
            } => {
                let Some(effect) = self.bus_mut(bus_slot)?.effects.get_mut(effect_slot) else {
                    return Err(MixerError::InvalidSlot("effect", effect_slot));
                };
                effect.enabled = enabled;
            }
            PreparedAudioCommand::SetEffectParameter {
                bus_slot,
                effect_slot,
                parameter,
                transition_millis: _,
                ..
            } => {
                let sample_rate_hz = self.sample_rate_hz;
                let Some(effect) = self.bus_mut(bus_slot)?.effects.get_mut(effect_slot) else {
                    return Err(MixerError::InvalidSlot("effect", effect_slot));
                };
                if !effect.effect.set_parameter(parameter, sample_rate_hz) {
                    return Err(MixerError::InvalidEffectParameter);
                }
            }
            PreparedAudioCommand::ApplySnapshot {
                snapshot_slot,
                transition_millis,
                ..
            } => self.apply_snapshot(snapshot_slot, transition_millis)?,
        }
        Ok(())
    }

    pub fn render(&mut self, output: &mut [f32], mut emit: impl FnMut(AudioEvent)) {
        if !output.len().is_multiple_of(2) || output.len() / 2 > self.max_callback_frames {
            output.fill(0.0);
            self.xrun_count = self.xrun_count.saturating_add(1);
            emit(AudioEvent::Xrun {
                count: self.xrun_count,
            });
            return;
        }
        let samples = output.len();
        output.fill(0.0);
        for scratch in &mut self.scratch {
            scratch[..samples].fill(0.0);
        }

        for slot in 0..self.voices.len() {
            let Some(mut voice) = self.voices[slot].take() else {
                continue;
            };
            let end = self.render_voice(&mut voice, samples);
            if let Some(reason) = end {
                emit(AudioEvent::PlaybackEnded {
                    playback: voice.playback,
                    voice: voice.id,
                    reason,
                });
            } else {
                self.voices[slot] = Some(voice);
            }
        }

        for bus_index in (0..self.buses.len()).rev() {
            self.buses[bus_index].process(&mut self.scratch[bus_index][..samples]);
            if let Some(parent) = self.buses[bus_index].parent {
                let (parents, children) = self.scratch.split_at_mut(bus_index);
                let parent_buffer = &mut parents[parent][..samples];
                let child_buffer = &children[0][..samples];
                for (parent, child) in parent_buffer.iter_mut().zip(child_buffer) {
                    *parent += *child;
                }
            }
        }
        output.copy_from_slice(&self.scratch[0][..samples]);
    }

    fn render_voice(
        &mut self,
        voice: &mut Voice,
        samples: usize,
    ) -> Option<AudioPlaybackEndReason> {
        let resource = self.resources[voice.resource_slot]
            .as_ref()
            .expect("prepared voice retains installed resource");
        let destination = &mut self.scratch[voice.bus_slot][..samples];
        let frame_count = resource.audio.frame_count();
        let channels = usize::from(resource.audio.channels());
        let source = resource.audio.samples();

        for frame in destination.chunks_exact_mut(2) {
            if voice.cursor >= frame_count {
                if let Some(loop_start) = loop_start(voice.loop_mode, frame_count) {
                    voice.cursor = loop_start;
                } else {
                    return Some(AudioPlaybackEndReason::Finished);
                }
            }
            if let AudioLoopMode::Region {
                start_frame,
                end_frame,
            } = voice.loop_mode
                && voice.cursor >= end_frame
            {
                voice.cursor = start_frame;
            }
            let offset = voice.cursor as usize * channels;
            let (left, right) = if channels == 1 {
                (source[offset], source[offset])
            } else {
                (source[offset], source[offset + 1])
            };
            let gain = voice.gain.next();
            let pan = voice.pan.next().clamp(-1.0, 1.0);
            let angle = (pan + 1.0) * std::f32::consts::FRAC_PI_4;
            frame[0] += left * gain * angle.cos();
            frame[1] += right * gain * angle.sin();
            voice.cursor += 1;
        }
        if voice.stop_reason.is_some() && voice.gain.settled_at_zero() {
            voice.stop_reason
        } else {
            None
        }
    }

    fn stop_voice(
        &mut self,
        voice_slot: usize,
        fade_out_millis: u32,
        reason: AudioPlaybackEndReason,
        emit: &mut impl FnMut(AudioEvent),
    ) -> Result<(), MixerError> {
        let Some(target) = self.voices.get_mut(voice_slot) else {
            return Err(MixerError::InvalidSlot("voice", voice_slot));
        };
        if fade_out_millis == 0 {
            if let Some(voice) = target.take() {
                emit(AudioEvent::PlaybackEnded {
                    playback: voice.playback,
                    voice: voice.id,
                    reason,
                });
            }
        } else if let Some(voice) = target {
            voice.stop_reason = Some(reason);
            voice
                .gain
                .set_target(0.0, millis_to_frames(fade_out_millis, self.sample_rate_hz));
        }
        Ok(())
    }

    fn apply_snapshot(
        &mut self,
        snapshot_slot: usize,
        transition_millis: u32,
    ) -> Result<(), MixerError> {
        let Some(snapshot) = self.snapshots.get(snapshot_slot).cloned() else {
            return Err(MixerError::InvalidSlot("snapshot", snapshot_slot));
        };
        let frames = millis_to_frames(transition_millis, self.sample_rate_hz);
        for (bus_slot, gain) in snapshot.bus_gains {
            self.bus_mut(bus_slot)?
                .gain
                .set_target(db_to_linear(gain), frames);
        }
        for (bus_slot, effect_slot, parameter) in snapshot.effect_parameters {
            let sample_rate_hz = self.sample_rate_hz;
            let Some(effect) = self.bus_mut(bus_slot)?.effects.get_mut(effect_slot) else {
                return Err(MixerError::InvalidSlot("effect", effect_slot));
            };
            if !effect.effect.set_parameter(parameter, sample_rate_hz) {
                return Err(MixerError::InvalidEffectParameter);
            }
        }
        Ok(())
    }

    fn voice_mut(&mut self, slot: usize) -> Result<&mut Voice, MixerError> {
        self.voices
            .get_mut(slot)
            .and_then(Option::as_mut)
            .ok_or(MixerError::MissingVoice(slot))
    }

    fn bus_mut(&mut self, slot: usize) -> Result<&mut Bus, MixerError> {
        self.buses
            .get_mut(slot)
            .ok_or(MixerError::InvalidSlot("bus", slot))
    }
}

impl Bus {
    fn new(definition: &PreparedBus, sample_rate_hz: u32) -> Self {
        Self {
            parent: definition.parent,
            gain: SmoothedValue::immediate(db_to_linear(definition.gain)),
            muted: definition.muted,
            effects: definition
                .effects
                .iter()
                .map(|effect| EffectSlot {
                    enabled: effect.enabled,
                    effect: Effect::new(effect, sample_rate_hz),
                })
                .collect(),
        }
    }

    fn process(&mut self, stereo: &mut [f32]) {
        for effect in &mut self.effects {
            if effect.enabled {
                effect.effect.process(stereo);
            }
        }
        for frame in stereo.chunks_exact_mut(2) {
            let gain = if self.muted { 0.0 } else { self.gain.next() };
            frame[0] *= gain;
            frame[1] *= gain;
        }
    }
}

fn validate_play_range(
    resource: &ResourceSlot,
    start_frame: u64,
    loop_mode: AudioLoopMode,
) -> Result<(), MixerError> {
    let frames = resource.audio.frame_count();
    if start_frame >= frames {
        return Err(MixerError::InvalidPosition {
            resource: resource.id.as_str().to_owned(),
            frame: start_frame,
            frames,
        });
    }
    if let AudioLoopMode::Region {
        start_frame,
        end_frame,
    } = loop_mode
        && (end_frame <= start_frame || end_frame > frames)
    {
        return Err(MixerError::InvalidLoop {
            resource: resource.id.as_str().to_owned(),
            start: start_frame,
            end: end_frame,
            frames,
        });
    }
    Ok(())
}

fn loop_start(loop_mode: AudioLoopMode, frame_count: u64) -> Option<u64> {
    match loop_mode {
        AudioLoopMode::None => None,
        AudioLoopMode::Whole => (frame_count > 0).then_some(0),
        AudioLoopMode::Region { start_frame, .. } => Some(start_frame),
    }
}

fn db_to_linear(gain: GainDbMilli) -> f32 {
    if gain.get() <= -120_000 {
        0.0
    } else {
        10.0_f32.powf(gain.get() as f32 / 20_000.0)
    }
}

fn millis_to_frames(millis: u32, sample_rate_hz: u32) -> u32 {
    ((u64::from(millis) * u64::from(sample_rate_hz)) / 1_000).min(u64::from(u32::MAX)) as u32
}

#[derive(Debug, Error)]
pub enum MixerError {
    #[error("invalid mixer configuration: {0}")]
    InvalidConfiguration(String),
    #[error("invalid {0} slot {1}")]
    InvalidSlot(&'static str, usize),
    #[error("resource slot {0} has not been installed")]
    MissingResource(usize),
    #[error("voice slot {0} is not active")]
    MissingVoice(usize),
    #[error("resource `{resource}` frame {frame} is outside 0..{frames}")]
    InvalidPosition {
        resource: String,
        frame: u64,
        frames: u64,
    },
    #[error("resource `{resource}` loop {start}..{end} is invalid for {frames} frames")]
    InvalidLoop {
        resource: String,
        start: u64,
        end: u64,
        frames: u64,
    },
    #[error("the effect does not accept that typed parameter")]
    InvalidEffectParameter,
}

impl From<MixerError> for AudioFailure {
    fn from(error: MixerError) -> Self {
        Self::Backend {
            message: error.to_string(),
        }
    }
}
