use crate::{
    id::{Identifier, IdentifierError},
    payload::InteractionPayload,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Stable identifier of one encoded or decoded audio resource.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct AudioResourceId(Identifier);

impl AudioResourceId {
    pub fn new(value: impl Into<String>) -> Result<Self, IdentifierError> {
        Identifier::new(value).map(Self)
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

/// Stable identifier of one logical playback voice.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct AudioVoiceId(Identifier);

impl AudioVoiceId {
    pub fn new(value: impl Into<String>) -> Result<Self, IdentifierError> {
        Identifier::new(value).map(Self)
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

/// Stable identifier of one mixer bus.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct AudioBusId(Identifier);

impl AudioBusId {
    pub fn new(value: impl Into<String>) -> Result<Self, IdentifierError> {
        Identifier::new(value).map(Self)
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

/// Stable identifier of one effect instance in a bus chain.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct AudioEffectId(Identifier);

impl AudioEffectId {
    pub fn new(value: impl Into<String>) -> Result<Self, IdentifierError> {
        Identifier::new(value).map(Self)
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

/// Stable identifier of one mixer snapshot.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct AudioSnapshotId(Identifier);

impl AudioSnapshotId {
    pub fn new(value: impl Into<String>) -> Result<Self, IdentifierError> {
        Identifier::new(value).map(Self)
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

/// Stable identifier of one permissioned microphone capture stream.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct AudioCaptureId(Identifier);

impl AudioCaptureId {
    pub fn new(value: impl Into<String>) -> Result<Self, IdentifierError> {
        Identifier::new(value).map(Self)
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

/// Deterministic identity assigned to one runtime-to-host audio command.
#[derive(
    Clone, Copy, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
)]
pub struct AudioDispatchId {
    pub logical_epoch: u64,
    pub sequence: u64,
}

impl AudioDispatchId {
    #[must_use]
    pub const fn new(logical_epoch: u64, sequence: u64) -> Self {
        Self {
            logical_epoch,
            sequence,
        }
    }
}

/// Signed gain in thousandths of one decibel.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(try_from = "i32", into = "i32")]
pub struct GainDbMilli(i32);

impl GainDbMilli {
    pub const MIN: Self = Self(-120_000);
    pub const UNITY: Self = Self(0);
    pub const MAX: Self = Self(24_000);

    pub fn new(value: i32) -> Result<Self, AudioValueError> {
        if (Self::MIN.0..=Self::MAX.0).contains(&value) {
            Ok(Self(value))
        } else {
            Err(AudioValueError::GainOutOfRange(value))
        }
    }

    #[must_use]
    pub const fn get(self) -> i32 {
        self.0
    }

    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn linear_amplitude(self) -> f32 {
        if self.0 <= Self::MIN.0 {
            0.0
        } else {
            10.0_f32.powf(self.0 as f32 / 20_000.0)
        }
    }
}

impl Default for GainDbMilli {
    fn default() -> Self {
        Self::UNITY
    }
}

impl TryFrom<i32> for GainDbMilli {
    type Error = AudioValueError;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<GainDbMilli> for i32 {
    fn from(value: GainDbMilli) -> Self {
        value.0
    }
}

/// Stereo pan in thousandths: `-1000` is left, `0` center, `1000` right.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(try_from = "i16", into = "i16")]
pub struct PanMilli(i16);

impl PanMilli {
    pub const LEFT: Self = Self(-1_000);
    pub const CENTER: Self = Self(0);
    pub const RIGHT: Self = Self(1_000);

    pub fn new(value: i16) -> Result<Self, AudioValueError> {
        if (Self::LEFT.0..=Self::RIGHT.0).contains(&value) {
            Ok(Self(value))
        } else {
            Err(AudioValueError::PanOutOfRange(value))
        }
    }

    #[must_use]
    pub const fn get(self) -> i16 {
        self.0
    }

    #[must_use]
    pub fn normalized(self) -> f32 {
        f32::from(self.0) / 1_000.0
    }

    /// Returns equal-power left/right factors.
    #[must_use]
    pub fn stereo_gains(self) -> (f32, f32) {
        let pan = self.normalized();
        (((1.0 - pan) * 0.5).sqrt(), ((1.0 + pan) * 0.5).sqrt())
    }
}

impl Default for PanMilli {
    fn default() -> Self {
        Self::CENTER
    }
}

impl TryFrom<i16> for PanMilli {
    type Error = AudioValueError;

    fn try_from(value: i16) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<PanMilli> for i16 {
    fn from(value: PanMilli) -> Self {
        value.0
    }
}

/// Non-negative audio control duration in milliseconds.
#[derive(
    Clone, Copy, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
)]
#[serde(transparent)]
pub struct AudioMillis(u32);

impl AudioMillis {
    pub const ZERO: Self = Self(0);

    #[must_use]
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }

    #[must_use]
    pub fn frames_at(self, sample_rate_hz: u32) -> u32 {
        let frames = u64::from(self.0)
            .saturating_mul(u64::from(sample_rate_hz))
            .saturating_add(999)
            / 1_000;
        u32::try_from(frames).unwrap_or(u32::MAX)
    }
}

/// Loop policy after a resource has been decoded to PCM frames.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AudioLoopMode {
    #[default]
    None,
    Whole,
    Region {
        start_frame: u64,
        end_frame: u64,
    },
}

impl AudioLoopMode {
    pub fn region(start_frame: u64, end_frame: u64) -> Result<Self, AudioValueError> {
        if start_frame < end_frame {
            Ok(Self::Region {
                start_frame,
                end_frame,
            })
        } else {
            Err(AudioValueError::InvalidLoopRegion {
                start_frame,
                end_frame,
            })
        }
    }

    pub fn validate_for_frames(self, frame_count: u64) -> Result<(), AudioValueError> {
        match self {
            Self::None | Self::Whole if frame_count > 0 => Ok(()),
            Self::Whole | Self::None => Err(AudioValueError::LoopOutsideResource {
                start_frame: 0,
                end_frame: frame_count,
                frame_count,
            }),
            Self::Region {
                start_frame,
                end_frame,
            } if start_frame < end_frame && end_frame <= frame_count => Ok(()),
            Self::Region {
                start_frame,
                end_frame,
            } => Err(AudioValueError::LoopOutsideResource {
                start_frame,
                end_frame,
                frame_count,
            }),
        }
    }

    /// Returns the next frame, or `None` when non-looped playback is complete.
    #[must_use]
    pub const fn next_frame(self, current_frame: u64, frame_count: u64) -> Option<u64> {
        let next = current_frame.saturating_add(1);
        match self {
            Self::None => {
                if next < frame_count {
                    Some(next)
                } else {
                    None
                }
            }
            Self::Whole => {
                if next < frame_count {
                    Some(next)
                } else {
                    Some(0)
                }
            }
            Self::Region {
                start_frame,
                end_frame,
            } => {
                if next < end_frame {
                    Some(next)
                } else {
                    Some(start_frame)
                }
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioEffectParameterKind {
    BiquadCutoffMilliHz,
    BiquadQMilli,
    CompressorThresholdDbMilli,
    CompressorRatioMilli,
    CompressorAttackMicros,
    CompressorReleaseMicros,
    CompressorMakeupDbMilli,
    DelayTimeMillis,
    DelayFeedbackMilli,
    ReverbRoomSizeMilli,
    ReverbDampingMilli,
    WetGainDbMilli,
    DryGainDbMilli,
    LimiterCeilingDbMilli,
    LimiterReleaseMicros,
}

impl AudioEffectParameterKind {
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "biquad_cutoff_milli_hz" => Some(Self::BiquadCutoffMilliHz),
            "biquad_q_milli" => Some(Self::BiquadQMilli),
            "compressor_threshold_db_milli" => Some(Self::CompressorThresholdDbMilli),
            "compressor_ratio_milli" => Some(Self::CompressorRatioMilli),
            "compressor_attack_micros" => Some(Self::CompressorAttackMicros),
            "compressor_release_micros" => Some(Self::CompressorReleaseMicros),
            "compressor_makeup_db_milli" => Some(Self::CompressorMakeupDbMilli),
            "delay_time_millis" => Some(Self::DelayTimeMillis),
            "delay_feedback_milli" => Some(Self::DelayFeedbackMilli),
            "reverb_room_size_milli" => Some(Self::ReverbRoomSizeMilli),
            "reverb_damping_milli" => Some(Self::ReverbDampingMilli),
            "wet_gain_db_milli" => Some(Self::WetGainDbMilli),
            "dry_gain_db_milli" => Some(Self::DryGainDbMilli),
            "limiter_ceiling_db_milli" => Some(Self::LimiterCeilingDbMilli),
            "limiter_release_micros" => Some(Self::LimiterReleaseMicros),
            _ => None,
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::BiquadCutoffMilliHz => "biquad_cutoff_milli_hz",
            Self::BiquadQMilli => "biquad_q_milli",
            Self::CompressorThresholdDbMilli => "compressor_threshold_db_milli",
            Self::CompressorRatioMilli => "compressor_ratio_milli",
            Self::CompressorAttackMicros => "compressor_attack_micros",
            Self::CompressorReleaseMicros => "compressor_release_micros",
            Self::CompressorMakeupDbMilli => "compressor_makeup_db_milli",
            Self::DelayTimeMillis => "delay_time_millis",
            Self::DelayFeedbackMilli => "delay_feedback_milli",
            Self::ReverbRoomSizeMilli => "reverb_room_size_milli",
            Self::ReverbDampingMilli => "reverb_damping_milli",
            Self::WetGainDbMilli => "wet_gain_db_milli",
            Self::DryGainDbMilli => "dry_gain_db_milli",
            Self::LimiterCeilingDbMilli => "limiter_ceiling_db_milli",
            Self::LimiterReleaseMicros => "limiter_release_micros",
        }
    }

    /// Converts one checked runtime integer to the typed parameter variant.
    pub fn from_i64(self, value: i64) -> Result<AudioEffectParameter, AudioValueError> {
        let parameter = match self {
            Self::BiquadCutoffMilliHz => {
                AudioEffectParameter::BiquadCutoffMilliHz(self.u64(value)?)
            }
            Self::BiquadQMilli => AudioEffectParameter::BiquadQMilli(self.u32(value)?),
            Self::CompressorThresholdDbMilli => {
                AudioEffectParameter::CompressorThresholdDbMilli(self.i32(value)?)
            }
            Self::CompressorRatioMilli => {
                AudioEffectParameter::CompressorRatioMilli(self.u32(value)?)
            }
            Self::CompressorAttackMicros => {
                AudioEffectParameter::CompressorAttackMicros(self.u32(value)?)
            }
            Self::CompressorReleaseMicros => {
                AudioEffectParameter::CompressorReleaseMicros(self.u32(value)?)
            }
            Self::CompressorMakeupDbMilli => {
                AudioEffectParameter::CompressorMakeupDbMilli(self.i32(value)?)
            }
            Self::DelayTimeMillis => AudioEffectParameter::DelayTimeMillis(self.u32(value)?),
            Self::DelayFeedbackMilli => AudioEffectParameter::DelayFeedbackMilli(self.u16(value)?),
            Self::ReverbRoomSizeMilli => {
                AudioEffectParameter::ReverbRoomSizeMilli(self.u16(value)?)
            }
            Self::ReverbDampingMilli => AudioEffectParameter::ReverbDampingMilli(self.u16(value)?),
            Self::WetGainDbMilli => AudioEffectParameter::WetGainDbMilli(self.i32(value)?),
            Self::DryGainDbMilli => AudioEffectParameter::DryGainDbMilli(self.i32(value)?),
            Self::LimiterCeilingDbMilli => {
                AudioEffectParameter::LimiterCeilingDbMilli(self.i32(value)?)
            }
            Self::LimiterReleaseMicros => {
                AudioEffectParameter::LimiterReleaseMicros(self.u32(value)?)
            }
        };
        parameter.validate()
    }

    fn u64(self, value: i64) -> Result<u64, AudioValueError> {
        u64::try_from(value).map_err(|_| self.invalid(value, "must fit u64"))
    }

    fn u32(self, value: i64) -> Result<u32, AudioValueError> {
        u32::try_from(value).map_err(|_| self.invalid(value, "must fit u32"))
    }

    fn u16(self, value: i64) -> Result<u16, AudioValueError> {
        u16::try_from(value).map_err(|_| self.invalid(value, "must fit u16"))
    }

    fn i32(self, value: i64) -> Result<i32, AudioValueError> {
        i32::try_from(value).map_err(|_| self.invalid(value, "must fit i32"))
    }

    const fn invalid(self, value: i64, requirement: &'static str) -> AudioValueError {
        AudioValueError::InvalidEffectParameter {
            parameter: self.as_str(),
            value,
            requirement,
        }
    }
}

/// Typed effect parameter update. Each variant owns its wire name and unit.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum AudioEffectParameter {
    BiquadCutoffMilliHz(u64),
    BiquadQMilli(u32),
    CompressorThresholdDbMilli(i32),
    CompressorRatioMilli(u32),
    CompressorAttackMicros(u32),
    CompressorReleaseMicros(u32),
    CompressorMakeupDbMilli(i32),
    DelayTimeMillis(u32),
    DelayFeedbackMilli(u16),
    ReverbRoomSizeMilli(u16),
    ReverbDampingMilli(u16),
    WetGainDbMilli(i32),
    DryGainDbMilli(i32),
    LimiterCeilingDbMilli(i32),
    LimiterReleaseMicros(u32),
}

impl AudioEffectParameter {
    #[must_use]
    pub const fn kind(self) -> AudioEffectParameterKind {
        match self {
            Self::BiquadCutoffMilliHz(_) => AudioEffectParameterKind::BiquadCutoffMilliHz,
            Self::BiquadQMilli(_) => AudioEffectParameterKind::BiquadQMilli,
            Self::CompressorThresholdDbMilli(_) => {
                AudioEffectParameterKind::CompressorThresholdDbMilli
            }
            Self::CompressorRatioMilli(_) => AudioEffectParameterKind::CompressorRatioMilli,
            Self::CompressorAttackMicros(_) => AudioEffectParameterKind::CompressorAttackMicros,
            Self::CompressorReleaseMicros(_) => AudioEffectParameterKind::CompressorReleaseMicros,
            Self::CompressorMakeupDbMilli(_) => AudioEffectParameterKind::CompressorMakeupDbMilli,
            Self::DelayTimeMillis(_) => AudioEffectParameterKind::DelayTimeMillis,
            Self::DelayFeedbackMilli(_) => AudioEffectParameterKind::DelayFeedbackMilli,
            Self::ReverbRoomSizeMilli(_) => AudioEffectParameterKind::ReverbRoomSizeMilli,
            Self::ReverbDampingMilli(_) => AudioEffectParameterKind::ReverbDampingMilli,
            Self::WetGainDbMilli(_) => AudioEffectParameterKind::WetGainDbMilli,
            Self::DryGainDbMilli(_) => AudioEffectParameterKind::DryGainDbMilli,
            Self::LimiterCeilingDbMilli(_) => AudioEffectParameterKind::LimiterCeilingDbMilli,
            Self::LimiterReleaseMicros(_) => AudioEffectParameterKind::LimiterReleaseMicros,
        }
    }

    #[must_use]
    pub const fn name(self) -> &'static str {
        self.kind().as_str()
    }

    pub fn validate(self) -> Result<Self, AudioValueError> {
        let invalid = |value, requirement| AudioValueError::InvalidEffectParameter {
            parameter: self.name(),
            value,
            requirement,
        };
        match self {
            Self::BiquadCutoffMilliHz(0) | Self::BiquadQMilli(0) | Self::DelayTimeMillis(0) => {
                Err(invalid(0, "must be non-zero"))
            }
            Self::CompressorRatioMilli(value) if value < 1_000 => {
                Err(invalid(i64::from(value), "must be at least 1000"))
            }
            Self::CompressorAttackMicros(value)
            | Self::CompressorReleaseMicros(value)
            | Self::LimiterReleaseMicros(value)
                if value == 0 =>
            {
                Err(invalid(0, "must be non-zero"))
            }
            Self::DelayFeedbackMilli(value) if value >= 1_000 => {
                Err(invalid(i64::from(value), "must be below 1000"))
            }
            Self::ReverbRoomSizeMilli(value) | Self::ReverbDampingMilli(value) if value > 1_000 => {
                Err(invalid(i64::from(value), "must not exceed 1000"))
            }
            Self::CompressorThresholdDbMilli(value)
            | Self::CompressorMakeupDbMilli(value)
            | Self::WetGainDbMilli(value)
            | Self::DryGainDbMilli(value)
            | Self::LimiterCeilingDbMilli(value) => GainDbMilli::new(value).map(|_| self),
            _ => Ok(self),
        }
    }
}

/// Browser/native microphone constraints resolved from a checked declaration.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MicrophoneConstraints {
    pub channels: u16,
    pub preferred_sample_rate_hz: Option<u32>,
    pub echo_cancellation: bool,
    pub noise_suppression: bool,
    pub auto_gain_control: bool,
}

impl MicrophoneConstraints {
    pub fn validate(self) -> Result<Self, AudioValueError> {
        if !(1..=2).contains(&self.channels) {
            return Err(AudioValueError::MicrophoneChannels(self.channels));
        }
        if self.preferred_sample_rate_hz == Some(0) {
            return Err(AudioValueError::MicrophoneSampleRate(0));
        }
        Ok(self)
    }
}

/// Runtime-to-host audio request. No device, path, browser or CPAL handle crosses this boundary.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AudioCommand {
    Play {
        voice: AudioVoiceId,
        resource: AudioResourceId,
        bus: AudioBusId,
        gain: GainDbMilli,
        pan: PanMilli,
        loop_mode: AudioLoopMode,
        start_frame: u64,
        fade_in: AudioMillis,
    },
    Stop {
        voice: AudioVoiceId,
        fade_out: AudioMillis,
    },
    StopAll {
        fade_out: AudioMillis,
    },
    SetVoiceGain {
        voice: AudioVoiceId,
        gain: GainDbMilli,
        transition: AudioMillis,
    },
    SetVoicePan {
        voice: AudioVoiceId,
        pan: PanMilli,
        transition: AudioMillis,
    },
    SetBusGain {
        bus: AudioBusId,
        gain: GainDbMilli,
        transition: AudioMillis,
    },
    SetBusMute {
        bus: AudioBusId,
        muted: bool,
    },
    SetEffectEnabled {
        bus: AudioBusId,
        effect: AudioEffectId,
        enabled: bool,
    },
    SetEffectParameter {
        bus: AudioBusId,
        effect: AudioEffectId,
        parameter: AudioEffectParameter,
        transition: AudioMillis,
    },
    ApplySnapshot {
        snapshot: AudioSnapshotId,
        transition: AudioMillis,
    },
    RequestMicrophone {
        capture: AudioCaptureId,
        constraints: MicrophoneConstraints,
    },
    StopMicrophone {
        capture: AudioCaptureId,
    },
    SetCaptureMonitor {
        capture: AudioCaptureId,
        bus: Option<AudioBusId>,
        gain: GainDbMilli,
    },
}

/// Runtime boundary envelope preserving deterministic command identity.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AudioCommandEnvelope {
    pub dispatch: AudioDispatchId,
    pub command: AudioCommand,
}

impl AudioCommandEnvelope {
    #[must_use]
    pub const fn new(dispatch: AudioDispatchId, command: AudioCommand) -> Self {
        Self { dispatch, command }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioPlaybackEndReason {
    Finished,
    Stopped,
    Replaced,
    StopAll,
    ResourceUnloaded,
}

impl AudioPlaybackEndReason {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Finished => "finished",
            Self::Stopped => "stopped",
            Self::Replaced => "replaced",
            Self::StopAll => "stop_all",
            Self::ResourceUnloaded => "resource_unloaded",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioCaptureState {
    PermissionPrompted,
    PermissionGranted,
    PermissionDenied,
    Started,
    Stopped,
    DeviceLost,
}

impl AudioCaptureState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PermissionPrompted => "permission_prompted",
            Self::PermissionGranted => "permission_granted",
            Self::PermissionDenied => "permission_denied",
            Self::Started => "started",
            Self::Stopped => "stopped",
            Self::DeviceLost => "device_lost",
        }
    }
}

/// Mixer-only invariant failure. Command validation should normally prevent these.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioMixerFailure {
    ResourceSlot,
    VoiceSlot,
    BusSlot,
    EffectSlot,
    SnapshotSlot,
    CallbackCapacity,
}

impl AudioMixerFailure {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ResourceSlot => "resource_slot",
            Self::VoiceSlot => "voice_slot",
            Self::BusSlot => "bus_slot",
            Self::EffectSlot => "effect_slot",
            Self::SnapshotSlot => "snapshot_slot",
            Self::CallbackCapacity => "callback_capacity",
        }
    }
}

/// Structured host failure returned at the next deterministic step boundary.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AudioFailure {
    UnknownResource {
        resource: AudioResourceId,
    },
    ResourceNotInstalled {
        resource: AudioResourceId,
    },
    UnknownVoice {
        voice: AudioVoiceId,
    },
    UnknownBus {
        bus: AudioBusId,
    },
    UnknownEffect {
        bus: AudioBusId,
        effect: AudioEffectId,
    },
    UnsupportedEffectParameter {
        bus: AudioBusId,
        effect: AudioEffectId,
        parameter: String,
    },
    UnknownSnapshot {
        snapshot: AudioSnapshotId,
    },
    UnknownCapture {
        capture: AudioCaptureId,
    },
    InvalidLoop {
        start_frame: u64,
        end_frame: u64,
        frame_count: u64,
    },
    VoiceLimit {
        maximum: usize,
    },
    PermissionDenied {
        capture: AudioCaptureId,
    },
    UnsupportedFormat {
        format: String,
    },
    QueueFull,
    MixerInvariant {
        failure: AudioMixerFailure,
    },
    Backend {
        message: String,
    },
}

/// Host-to-runtime audio completion, capture status, metric or failure event.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AudioEvent {
    OutputReady {
        sample_rate_hz: u32,
        channels: u16,
    },
    PlaybackStarted {
        playback: AudioDispatchId,
        voice: AudioVoiceId,
        resource: AudioResourceId,
    },
    PlaybackEnded {
        playback: AudioDispatchId,
        voice: AudioVoiceId,
        reason: AudioPlaybackEndReason,
    },
    CaptureStateChanged {
        dispatch: AudioDispatchId,
        capture: AudioCaptureId,
        state: AudioCaptureState,
        sample_rate_hz: Option<u32>,
        channels: Option<u16>,
    },
    CaptureLevel {
        capture: AudioCaptureId,
        sequence: u64,
        rms: f32,
        peak: f32,
        dropped_samples: u64,
    },
    CommandFailed {
        dispatch: AudioDispatchId,
        failure: AudioFailure,
    },
    Xrun {
        count: u64,
    },
    DecodeUnderrun {
        resource: AudioResourceId,
        count: u64,
    },
}

/// Typed host event family consumed at the runtime step boundary.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum HostEvent {
    Audio {
        event: AudioEvent,
    },
    Signal {
        name: Identifier,
        value: InteractionPayload,
    },
    Metric {
        name: Identifier,
        value: InteractionPayload,
    },
    Custom {
        name: Identifier,
        payload: InteractionPayload,
    },
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(transparent)]
pub struct HostEventBatch(Vec<HostEvent>);

impl HostEventBatch {
    #[must_use]
    pub fn new(events: Vec<HostEvent>) -> Self {
        Self(events)
    }

    #[must_use]
    pub fn as_slice(&self) -> &[HostEvent] {
        &self.0
    }

    #[must_use]
    pub fn iter(&self) -> impl ExactSizeIterator<Item = &HostEvent> {
        self.0.iter()
    }

    #[must_use]
    pub fn into_vec(self) -> Vec<HostEvent> {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum AudioValueError {
    #[error("audio gain {0} milli-dB is outside -120000..=24000")]
    GainOutOfRange(i32),
    #[error("audio pan {0} is outside -1000..=1000")]
    PanOutOfRange(i16),
    #[error("audio loop start {start_frame} must be before end {end_frame}")]
    InvalidLoopRegion { start_frame: u64, end_frame: u64 },
    #[error("audio loop {start_frame}..{end_frame} is outside resource frame count {frame_count}")]
    LoopOutsideResource {
        start_frame: u64,
        end_frame: u64,
        frame_count: u64,
    },
    #[error("microphone channel count {0} is unsupported; expected 1 or 2")]
    MicrophoneChannels(u16),
    #[error("microphone preferred sample rate must be non-zero")]
    MicrophoneSampleRate(u32),
    #[error("audio effect parameter `{parameter}` value {value} is invalid: {requirement}")]
    InvalidEffectParameter {
        parameter: &'static str,
        value: i64,
        requirement: &'static str,
    },
}
