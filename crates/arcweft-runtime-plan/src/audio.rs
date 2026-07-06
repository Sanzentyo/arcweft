use std::collections::BTreeMap;

use arcweft_core::audio::RuntimeAudioCommand;
use arcweft_core::effect::LineEffectRequest;
use arcweft_core::value::{RuntimeExpr, RuntimeValue};
use arcweft_interaction_model::audio::{
    AudioEffectParameterKind, AudioLoopMode, MicrophoneConstraints,
};
use arcweft_lang_hir::syntax::expr::{CallArg, Expr, Literal};

use crate::expr::lower_runtime_expr_strict;
use crate::labels::expr_label;

/// Lowers the checked `audio.*` DSL family to typed runtime audio IR.
///
/// The semantic checker owns call availability and type diagnostics. This pass
/// still converts malformed recovered syntax to an explicit runtime failure
/// rather than silently preserving it as a string-dispatched generic call.
pub(crate) fn lower_audio_call(expr: &Expr) -> Option<LineEffectRequest> {
    let call = AudioCall::from_expr(expr)?;
    Some(match call.lower() {
        Ok(command) => LineEffectRequest::Audio(Box::new(command)),
        Err(message) => LineEffectRequest::Fail(message),
    })
}

struct AudioCall<'a> {
    callee: String,
    positional: Vec<&'a Expr>,
    named: BTreeMap<String, &'a Expr>,
    malformed: Option<String>,
}

impl<'a> AudioCall<'a> {
    fn from_expr(expr: &'a Expr) -> Option<Self> {
        let (callee, args) = match expr {
            Expr::Call { callee, args } => (expr_label(callee), args.as_slice()),
            Expr::MethodCall {
                receiver,
                method,
                args,
            } => {
                let method = method
                    .split_once('<')
                    .map_or(method.as_str(), |(name, _)| name);
                (
                    format!("{}.{method}", expr_label(receiver)),
                    args.as_slice(),
                )
            }
            Expr::Path(path) => (path.as_label().to_owned(), &[][..]),
            Expr::ShortVariant(name) => (format!(".{name}"), &[][..]),
            _ => return None,
        };
        if !callee.starts_with("audio.") {
            return None;
        }

        let mut call = Self {
            callee,
            positional: Vec::new(),
            named: BTreeMap::new(),
            malformed: None,
        };
        for arg in args {
            match arg {
                CallArg::Positional(value) => call.positional.push(value),
                CallArg::Named { name, value } => {
                    if call.named.insert(name.clone(), value).is_some() {
                        call.malformed = Some(format!("duplicate audio argument `{name}`"));
                    }
                }
                CallArg::Spread { .. } => {
                    call.malformed =
                        Some("audio commands do not accept spread arguments".to_owned());
                }
            }
        }
        Some(call)
    }

    fn lower(&self) -> Result<RuntimeAudioCommand, String> {
        if let Some(message) = &self.malformed {
            return Err(message.clone());
        }
        match self.callee.as_str() {
            "audio.play" => Ok(RuntimeAudioCommand::Play {
                voice: self.lower_required(0, "voice")?,
                resource: self.lower_required(1, "resource")?,
                bus: self.lower_required(2, "bus")?,
                gain_db_milli: self.lower_or_i64(3, "gain_db_milli", 0)?,
                pan_milli: self.lower_or_i64(4, "pan_milli", 0)?,
                loop_mode: self.loop_mode()?,
                start_frame: self.lower_or_u64(5, "start_frame", 0)?,
                fade_in_millis: self.lower_or_u64(6, "fade_in_millis", 0)?,
            }),
            "audio.stop" => Ok(RuntimeAudioCommand::Stop {
                voice: self.lower_required(0, "voice")?,
                fade_out_millis: self.lower_or_u64(1, "fade_out_millis", 0)?,
            }),
            "audio.stop_all" => Ok(RuntimeAudioCommand::StopAll {
                fade_out_millis: self.lower_or_u64(0, "fade_out_millis", 0)?,
            }),
            "audio.voice_gain" => Ok(RuntimeAudioCommand::SetVoiceGain {
                voice: self.lower_required(0, "voice")?,
                gain_db_milli: self.lower_required(1, "gain_db_milli")?,
                transition_millis: self.lower_or_u64(2, "transition_millis", 0)?,
            }),
            "audio.voice_pan" => Ok(RuntimeAudioCommand::SetVoicePan {
                voice: self.lower_required(0, "voice")?,
                pan_milli: self.lower_required(1, "pan_milli")?,
                transition_millis: self.lower_or_u64(2, "transition_millis", 0)?,
            }),
            "audio.bus_gain" => Ok(RuntimeAudioCommand::SetBusGain {
                bus: self.lower_required(0, "bus")?,
                gain_db_milli: self.lower_required(1, "gain_db_milli")?,
                transition_millis: self.lower_or_u64(2, "transition_millis", 0)?,
            }),
            "audio.bus_mute" => Ok(RuntimeAudioCommand::SetBusMute {
                bus: self.lower_required(0, "bus")?,
                muted: self.lower_required(1, "muted")?,
            }),
            "audio.effect_enabled" => Ok(RuntimeAudioCommand::SetEffectEnabled {
                bus: self.lower_required(0, "bus")?,
                effect: self.lower_required(1, "effect")?,
                enabled: self.lower_required(2, "enabled")?,
            }),
            "audio.effect_parameter" => {
                let name = self.literal_string(2, "parameter")?;
                let Some(parameter) = AudioEffectParameterKind::from_name(name) else {
                    return Err(format!("unknown audio effect parameter `{name}`"));
                };
                Ok(RuntimeAudioCommand::SetEffectParameter {
                    bus: self.lower_required(0, "bus")?,
                    effect: self.lower_required(1, "effect")?,
                    parameter,
                    value: self.lower_required(3, "value")?,
                    transition_millis: self.lower_or_u64(4, "transition_millis", 0)?,
                })
            }
            "audio.snapshot" => Ok(RuntimeAudioCommand::ApplySnapshot {
                snapshot: self.lower_required(0, "snapshot")?,
                transition_millis: self.lower_or_u64(1, "transition_millis", 0)?,
            }),
            "audio.microphone" => Ok(RuntimeAudioCommand::RequestMicrophone {
                capture: self.lower_required(0, "capture")?,
                constraints: MicrophoneConstraints {
                    channels: self.literal_u16_or(1, "channels", 1)?,
                    preferred_sample_rate_hz: self
                        .optional(2, "sample_rate_hz")
                        .map(|value| self.literal_u32(value, "sample_rate_hz"))
                        .transpose()?,
                    echo_cancellation: self.literal_bool_or(3, "echo_cancellation", true)?,
                    noise_suppression: self.literal_bool_or(4, "noise_suppression", true)?,
                    auto_gain_control: self.literal_bool_or(5, "auto_gain_control", false)?,
                }
                .validate()
                .map_err(|error| error.to_string())?,
            }),
            "audio.microphone_stop" => Ok(RuntimeAudioCommand::StopMicrophone {
                capture: self.lower_required(0, "capture")?,
            }),
            "audio.capture_monitor" => Ok(RuntimeAudioCommand::SetCaptureMonitor {
                capture: self.lower_required(0, "capture")?,
                bus: self
                    .optional(1, "bus")
                    .map(lower_runtime_expr_strict)
                    .transpose()?,
                gain_db_milli: self.lower_or_i64(2, "gain_db_milli", 0)?,
            }),
            _ => Err(format!("unknown audio command `{}`", self.callee)),
        }
    }

    fn argument(&self, positional: usize, name: &str) -> Option<&'a Expr> {
        self.named
            .get(name)
            .copied()
            .or_else(|| self.positional.get(positional).copied())
    }

    fn optional(&self, positional: usize, name: &str) -> Option<&'a Expr> {
        self.argument(positional, name)
    }

    fn required(&self, positional: usize, name: &str) -> Result<&'a Expr, String> {
        self.argument(positional, name)
            .ok_or_else(|| format!("{} requires argument `{name}`", self.callee))
    }

    fn lower_required(&self, positional: usize, name: &str) -> Result<RuntimeExpr, String> {
        lower_runtime_expr_strict(self.required(positional, name)?)
    }

    fn lower_or_i64(
        &self,
        positional: usize,
        name: &str,
        default: i64,
    ) -> Result<RuntimeExpr, String> {
        self.optional(positional, name).map_or_else(
            || Ok(RuntimeExpr::Value(RuntimeValue::i64(default))),
            lower_runtime_expr_strict,
        )
    }

    fn lower_or_u64(
        &self,
        positional: usize,
        name: &str,
        default: u64,
    ) -> Result<RuntimeExpr, String> {
        self.optional(positional, name).map_or_else(
            || Ok(RuntimeExpr::Value(RuntimeValue::u64(default))),
            lower_runtime_expr_strict,
        )
    }

    fn loop_mode(&self) -> Result<AudioLoopMode, String> {
        let start = self.named.get("loop_start_frame").copied();
        let end = self.named.get("loop_end_frame").copied();
        match (start, end) {
            (Some(start), Some(end)) => AudioLoopMode::region(
                self.literal_u64(start, "loop_start_frame")?,
                self.literal_u64(end, "loop_end_frame")?,
            )
            .map_err(|error| error.to_string()),
            (None, None) => self
                .optional(7, "looped")
                .map(|value| self.literal_bool(value, "looped"))
                .transpose()
                .map(|looped| {
                    if looped.unwrap_or(false) {
                        AudioLoopMode::Whole
                    } else {
                        AudioLoopMode::None
                    }
                }),
            _ => Err("audio.play requires both `loop_start_frame` and `loop_end_frame`".to_owned()),
        }
    }

    fn literal_string(&self, positional: usize, name: &str) -> Result<&'a str, String> {
        match self.required(positional, name)? {
            Expr::Literal(Literal::String(value)) => Ok(value),
            value => Err(format!(
                "{} argument `{name}` must be a string literal, found {}",
                self.callee,
                expr_label(value)
            )),
        }
    }

    fn literal_bool_or(
        &self,
        positional: usize,
        name: &str,
        default: bool,
    ) -> Result<bool, String> {
        self.optional(positional, name)
            .map_or(Ok(default), |value| self.literal_bool(value, name))
    }

    fn literal_bool(&self, value: &Expr, name: &str) -> Result<bool, String> {
        match value {
            Expr::Literal(Literal::Bool(value)) => Ok(*value),
            _ => Err(format!(
                "{} argument `{name}` must be a bool literal",
                self.callee
            )),
        }
    }

    fn literal_u16_or(&self, positional: usize, name: &str, default: u16) -> Result<u16, String> {
        self.optional(positional, name)
            .map_or(Ok(default), |value| {
                self.literal_u64(value, name).and_then(|value| {
                    u16::try_from(value)
                        .map_err(|_| format!("{} argument `{name}` must fit u16", self.callee))
                })
            })
    }

    fn literal_u32(&self, value: &Expr, name: &str) -> Result<u32, String> {
        self.literal_u64(value, name).and_then(|value| {
            u32::try_from(value)
                .map_err(|_| format!("{} argument `{name}` must fit u32", self.callee))
        })
    }

    fn literal_u64(&self, value: &Expr, name: &str) -> Result<u64, String> {
        match value {
            Expr::Literal(Literal::Int { value, .. }) => u64::try_from(*value)
                .map_err(|_| format!("{} argument `{name}` must be non-negative", self.callee)),
            _ => Err(format!(
                "{} argument `{name}` must be an integer literal",
                self.callee
            )),
        }
    }
}
