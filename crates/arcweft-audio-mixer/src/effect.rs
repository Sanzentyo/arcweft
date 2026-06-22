#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::too_many_lines,
    clippy::unnested_or_patterns
)]

use arcweft_audio_core::{AudioEffectKind, PreparedEffect};
use arcweft_interaction_model::audio::AudioEffectParameter;

pub(crate) enum Effect {
    Biquad(Biquad),
    Compressor(Compressor),
    Delay(Delay),
    Reverb(Reverb),
    Limiter(Limiter),
}

impl Effect {
    pub(crate) fn new(definition: &PreparedEffect, sample_rate_hz: u32) -> Self {
        match &definition.kind {
            AudioEffectKind::LowPass {
                cutoff_milli_hz,
                q_milli,
            } => Self::Biquad(Biquad::new(
                BiquadMode::LowPass,
                *cutoff_milli_hz as f32 / 1_000.0,
                *q_milli as f32 / 1_000.0,
                sample_rate_hz,
            )),
            AudioEffectKind::HighPass {
                cutoff_milli_hz,
                q_milli,
            } => Self::Biquad(Biquad::new(
                BiquadMode::HighPass,
                *cutoff_milli_hz as f32 / 1_000.0,
                *q_milli as f32 / 1_000.0,
                sample_rate_hz,
            )),
            AudioEffectKind::Compressor {
                threshold_db_milli,
                ratio_milli,
                attack_micros,
                release_micros,
                makeup_db_milli,
            } => Self::Compressor(Compressor::new(
                *threshold_db_milli as f32 / 1_000.0,
                *ratio_milli as f32 / 1_000.0,
                *attack_micros as f32 / 1_000_000.0,
                *release_micros as f32 / 1_000_000.0,
                *makeup_db_milli as f32 / 1_000.0,
                sample_rate_hz,
            )),
            AudioEffectKind::Delay {
                time_millis,
                feedback_milli,
                wet_db_milli,
                dry_db_milli,
            } => Self::Delay(Delay::new(
                *time_millis,
                f32::from(*feedback_milli) / 1_000.0,
                db_to_linear(*wet_db_milli),
                db_to_linear(*dry_db_milli),
                sample_rate_hz,
            )),
            AudioEffectKind::Reverb {
                room_size_milli,
                damping_milli,
                wet_db_milli,
                dry_db_milli,
            } => Self::Reverb(Reverb::new(
                f32::from(*room_size_milli) / 1_000.0,
                f32::from(*damping_milli) / 1_000.0,
                db_to_linear(*wet_db_milli),
                db_to_linear(*dry_db_milli),
                sample_rate_hz,
            )),
            AudioEffectKind::Limiter {
                ceiling_db_milli,
                release_micros,
            } => Self::Limiter(Limiter::new(
                db_to_linear(*ceiling_db_milli),
                *release_micros as f32 / 1_000_000.0,
                sample_rate_hz,
            )),
        }
    }

    pub(crate) fn process(&mut self, stereo: &mut [f32]) {
        match self {
            Self::Biquad(effect) => effect.process(stereo),
            Self::Compressor(effect) => effect.process(stereo),
            Self::Delay(effect) => effect.process(stereo),
            Self::Reverb(effect) => effect.process(stereo),
            Self::Limiter(effect) => effect.process(stereo),
        }
    }

    pub(crate) fn set_parameter(
        &mut self,
        parameter: AudioEffectParameter,
        sample_rate_hz: u32,
    ) -> bool {
        match (self, parameter) {
            (Self::Biquad(effect), AudioEffectParameter::BiquadCutoffMilliHz(value)) => {
                effect.cutoff_hz = value as f32 / 1_000.0;
                effect.update_coefficients(sample_rate_hz);
                true
            }
            (Self::Biquad(effect), AudioEffectParameter::BiquadQMilli(value)) => {
                effect.q = value as f32 / 1_000.0;
                effect.update_coefficients(sample_rate_hz);
                true
            }
            (Self::Compressor(effect), AudioEffectParameter::CompressorThresholdDbMilli(value)) => {
                effect.threshold_db = value as f32 / 1_000.0;
                true
            }
            (Self::Compressor(effect), AudioEffectParameter::CompressorRatioMilli(value)) => {
                effect.ratio = (value as f32 / 1_000.0).max(1.0);
                true
            }
            (Self::Compressor(effect), AudioEffectParameter::CompressorAttackMicros(value)) => {
                effect.attack = time_coefficient(value as f32 / 1_000_000.0, sample_rate_hz);
                true
            }
            (Self::Compressor(effect), AudioEffectParameter::CompressorReleaseMicros(value)) => {
                effect.release = time_coefficient(value as f32 / 1_000_000.0, sample_rate_hz);
                true
            }
            (Self::Compressor(effect), AudioEffectParameter::CompressorMakeupDbMilli(value)) => {
                effect.makeup = db_to_linear(value);
                true
            }
            (Self::Delay(effect), AudioEffectParameter::DelayFeedbackMilli(value)) => {
                effect.feedback = (f32::from(value) / 1_000.0).clamp(0.0, 0.999);
                true
            }
            (Self::Delay(effect), AudioEffectParameter::WetGainDbMilli(value)) => {
                effect.wet = db_to_linear(value);
                true
            }
            (Self::Delay(effect), AudioEffectParameter::DryGainDbMilli(value)) => {
                effect.dry = db_to_linear(value);
                true
            }
            (Self::Reverb(effect), AudioEffectParameter::ReverbRoomSizeMilli(value)) => {
                effect.set_room_size(f32::from(value) / 1_000.0);
                true
            }
            (Self::Reverb(effect), AudioEffectParameter::ReverbDampingMilli(value)) => {
                effect.set_damping(f32::from(value) / 1_000.0);
                true
            }
            (Self::Reverb(effect), AudioEffectParameter::WetGainDbMilli(value)) => {
                effect.wet = db_to_linear(value);
                true
            }
            (Self::Reverb(effect), AudioEffectParameter::DryGainDbMilli(value)) => {
                effect.dry = db_to_linear(value);
                true
            }
            (Self::Limiter(effect), AudioEffectParameter::LimiterCeilingDbMilli(value)) => {
                effect.ceiling = db_to_linear(value).max(0.000_001);
                true
            }
            (Self::Limiter(effect), AudioEffectParameter::LimiterReleaseMicros(value)) => {
                effect.release = time_coefficient(value as f32 / 1_000_000.0, sample_rate_hz);
                true
            }
            (_, AudioEffectParameter::DelayTimeMillis(_))
            | (_, AudioEffectParameter::BiquadCutoffMilliHz(_))
            | (_, AudioEffectParameter::BiquadQMilli(_))
            | (_, AudioEffectParameter::CompressorThresholdDbMilli(_))
            | (_, AudioEffectParameter::CompressorRatioMilli(_))
            | (_, AudioEffectParameter::CompressorAttackMicros(_))
            | (_, AudioEffectParameter::CompressorReleaseMicros(_))
            | (_, AudioEffectParameter::CompressorMakeupDbMilli(_))
            | (_, AudioEffectParameter::DelayFeedbackMilli(_))
            | (_, AudioEffectParameter::ReverbRoomSizeMilli(_))
            | (_, AudioEffectParameter::ReverbDampingMilli(_))
            | (_, AudioEffectParameter::WetGainDbMilli(_))
            | (_, AudioEffectParameter::DryGainDbMilli(_))
            | (_, AudioEffectParameter::LimiterCeilingDbMilli(_))
            | (_, AudioEffectParameter::LimiterReleaseMicros(_)) => false,
        }
    }
}

#[derive(Clone, Copy)]
enum BiquadMode {
    LowPass,
    HighPass,
}

pub(super) struct Biquad {
    mode: BiquadMode,
    cutoff_hz: f32,
    q: f32,
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    z1: [f32; 2],
    z2: [f32; 2],
}

impl Biquad {
    fn new(mode: BiquadMode, cutoff_hz: f32, q: f32, sample_rate_hz: u32) -> Self {
        let mut effect = Self {
            mode,
            cutoff_hz,
            q,
            b0: 1.0,
            b1: 0.0,
            b2: 0.0,
            a1: 0.0,
            a2: 0.0,
            z1: [0.0; 2],
            z2: [0.0; 2],
        };
        effect.update_coefficients(sample_rate_hz);
        effect
    }

    fn update_coefficients(&mut self, sample_rate_hz: u32) {
        let nyquist = sample_rate_hz as f32 * 0.5;
        let cutoff = self.cutoff_hz.clamp(1.0, nyquist * 0.99);
        let q = self.q.max(0.001);
        let omega = 2.0 * std::f32::consts::PI * cutoff / sample_rate_hz as f32;
        let cosine = omega.cos();
        let alpha = omega.sin() / (2.0 * q);
        let (b0, b1, b2) = match self.mode {
            BiquadMode::LowPass => ((1.0 - cosine) * 0.5, 1.0 - cosine, (1.0 - cosine) * 0.5),
            BiquadMode::HighPass => ((1.0 + cosine) * 0.5, -(1.0 + cosine), (1.0 + cosine) * 0.5),
        };
        let a0 = 1.0 + alpha;
        self.b0 = b0 / a0;
        self.b1 = b1 / a0;
        self.b2 = b2 / a0;
        self.a1 = -2.0 * cosine / a0;
        self.a2 = (1.0 - alpha) / a0;
    }

    fn process(&mut self, stereo: &mut [f32]) {
        for frame in stereo.chunks_exact_mut(2) {
            for (channel, sample) in frame.iter_mut().enumerate() {
                let input = *sample;
                let output = self.b0 * input + self.z1[channel];
                self.z1[channel] = self.b1 * input - self.a1 * output + self.z2[channel];
                self.z2[channel] = self.b2 * input - self.a2 * output;
                *sample = output;
            }
        }
    }
}

pub(super) struct Compressor {
    threshold_db: f32,
    ratio: f32,
    attack: f32,
    release: f32,
    makeup: f32,
    envelope: f32,
}

impl Compressor {
    fn new(
        threshold_db: f32,
        ratio: f32,
        attack_seconds: f32,
        release_seconds: f32,
        makeup_db: f32,
        sample_rate_hz: u32,
    ) -> Self {
        Self {
            threshold_db,
            ratio: ratio.max(1.0),
            attack: time_coefficient(attack_seconds, sample_rate_hz),
            release: time_coefficient(release_seconds, sample_rate_hz),
            makeup: 10.0_f32.powf(makeup_db / 20.0),
            envelope: 0.0,
        }
    }

    fn process(&mut self, stereo: &mut [f32]) {
        for frame in stereo.chunks_exact_mut(2) {
            let level = frame[0].abs().max(frame[1].abs());
            let coefficient = if level > self.envelope {
                self.attack
            } else {
                self.release
            };
            self.envelope = coefficient * self.envelope + (1.0 - coefficient) * level;
            let level_db = 20.0 * self.envelope.max(0.000_000_1).log10();
            let reduction_db = if level_db > self.threshold_db {
                let compressed = self.threshold_db + (level_db - self.threshold_db) / self.ratio;
                compressed - level_db
            } else {
                0.0
            };
            let gain = 10.0_f32.powf(reduction_db / 20.0) * self.makeup;
            frame[0] *= gain;
            frame[1] *= gain;
        }
    }
}

pub(super) struct Delay {
    buffer: Vec<f32>,
    write: usize,
    feedback: f32,
    wet: f32,
    dry: f32,
}

impl Delay {
    fn new(time_millis: u32, feedback: f32, wet: f32, dry: f32, sample_rate_hz: u32) -> Self {
        let frames = ((u64::from(sample_rate_hz) * u64::from(time_millis)) / 1_000).max(1);
        Self {
            buffer: vec![0.0; frames as usize * 2],
            write: 0,
            feedback: feedback.clamp(0.0, 0.999),
            wet,
            dry,
        }
    }

    fn process(&mut self, stereo: &mut [f32]) {
        for sample in stereo {
            let delayed = self.buffer[self.write];
            let input = *sample;
            self.buffer[self.write] = input + delayed * self.feedback;
            *sample = input * self.dry + delayed * self.wet;
            self.write += 1;
            if self.write == self.buffer.len() {
                self.write = 0;
            }
        }
    }
}

struct Comb {
    buffer: Vec<f32>,
    index: usize,
    feedback: f32,
    damping: f32,
    filtered: f32,
}

impl Comb {
    fn process(&mut self, input: f32) -> f32 {
        let output = self.buffer[self.index];
        self.filtered = output * (1.0 - self.damping) + self.filtered * self.damping;
        self.buffer[self.index] = input + self.filtered * self.feedback;
        self.index += 1;
        if self.index == self.buffer.len() {
            self.index = 0;
        }
        output
    }
}

pub(super) struct Reverb {
    left: Vec<Comb>,
    right: Vec<Comb>,
    wet: f32,
    dry: f32,
}

impl Reverb {
    fn new(room_size: f32, damping: f32, wet: f32, dry: f32, sample_rate_hz: u32) -> Self {
        let scale = sample_rate_hz as f32 / 44_100.0;
        let mut left = [1116_u16, 1188, 1277, 1356]
            .into_iter()
            .map(|length| Comb {
                buffer: vec![0.0; (f32::from(length) * scale).round().max(1.0) as usize],
                index: 0,
                feedback: 0.7,
                damping: 0.2,
                filtered: 0.0,
            })
            .collect::<Vec<_>>();
        let mut right = [1139_u16, 1211, 1300, 1379]
            .into_iter()
            .map(|length| Comb {
                buffer: vec![0.0; (f32::from(length) * scale).round().max(1.0) as usize],
                index: 0,
                feedback: 0.7,
                damping: 0.2,
                filtered: 0.0,
            })
            .collect::<Vec<_>>();
        let feedback = 0.7 + room_size.clamp(0.0, 1.0) * 0.28;
        for comb in left.iter_mut().chain(right.iter_mut()) {
            comb.feedback = feedback;
            comb.damping = damping.clamp(0.0, 1.0);
        }
        Self {
            left,
            right,
            wet,
            dry,
        }
    }

    fn set_room_size(&mut self, room_size: f32) {
        let feedback = 0.7 + room_size.clamp(0.0, 1.0) * 0.28;
        for comb in self.left.iter_mut().chain(self.right.iter_mut()) {
            comb.feedback = feedback;
        }
    }

    fn set_damping(&mut self, damping: f32) {
        for comb in self.left.iter_mut().chain(self.right.iter_mut()) {
            comb.damping = damping.clamp(0.0, 1.0);
        }
    }

    fn process(&mut self, stereo: &mut [f32]) {
        for frame in stereo.chunks_exact_mut(2) {
            let input = (frame[0] + frame[1]) * 0.25;
            let left = self
                .left
                .iter_mut()
                .map(|comb| comb.process(input))
                .sum::<f32>();
            let right = self
                .right
                .iter_mut()
                .map(|comb| comb.process(input))
                .sum::<f32>();
            frame[0] = frame[0] * self.dry + left * self.wet;
            frame[1] = frame[1] * self.dry + right * self.wet;
        }
    }
}

pub(super) struct Limiter {
    ceiling: f32,
    release: f32,
    gain: f32,
}

impl Limiter {
    fn new(ceiling: f32, release_seconds: f32, sample_rate_hz: u32) -> Self {
        Self {
            ceiling: ceiling.max(0.000_001),
            release: time_coefficient(release_seconds, sample_rate_hz),
            gain: 1.0,
        }
    }

    fn process(&mut self, stereo: &mut [f32]) {
        for frame in stereo.chunks_exact_mut(2) {
            let peak = frame[0].abs().max(frame[1].abs());
            let required = if peak > self.ceiling {
                self.ceiling / peak
            } else {
                1.0
            };
            self.gain = if required < self.gain {
                required
            } else {
                self.release * self.gain + (1.0 - self.release)
            };
            frame[0] = (frame[0] * self.gain).clamp(-self.ceiling, self.ceiling);
            frame[1] = (frame[1] * self.gain).clamp(-self.ceiling, self.ceiling);
        }
    }
}

fn db_to_linear(db_milli: i32) -> f32 {
    if db_milli <= -120_000 {
        0.0
    } else {
        10.0_f32.powf(db_milli as f32 / 20_000.0)
    }
}

fn time_coefficient(seconds: f32, sample_rate_hz: u32) -> f32 {
    (-1.0 / (seconds.max(0.000_001) * sample_rate_hz as f32)).exp()
}
