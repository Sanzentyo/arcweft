//! The original audio grammar itself is the borrowed source view.

use super::{DOMAIN, Error};
use crate::audio::RuntimeAudioCommand as Command;
use arcweft_interaction_model::audio::{
    AudioEffectParameterKind, AudioLoopMode, MicrophoneConstraints,
};

pub(super) struct Cursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}
impl<'a> Cursor<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }
    fn invalid(&self, kind: &'static str) -> Error {
        Error::Invalid {
            kind,
            offset: self.offset,
        }
    }
    fn raw(&mut self, n: usize) -> Result<&'a [u8], Error> {
        let end = self.offset.checked_add(n).ok_or(Error::Arithmetic)?;
        let result = self.bytes.get(self.offset..end).ok_or(Error::Truncated {
            offset: self.offset,
        })?;
        self.offset = end;
        Ok(result)
    }
    fn byte(&mut self) -> Result<u8, Error> {
        Ok(self.raw(1)?[0])
    }
    fn flag(&mut self) -> Result<bool, Error> {
        match self.byte()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(self.invalid("flag")),
        }
    }
    fn u16(&mut self) -> Result<u16, Error> {
        Ok(u16::from_le_bytes(
            self.raw(2)?.try_into().expect("source u16 width"),
        ))
    }
    fn u32(&mut self) -> Result<u32, Error> {
        Ok(u32::from_le_bytes(
            self.raw(4)?.try_into().expect("source u32 width"),
        ))
    }
    fn u64(&mut self) -> Result<u64, Error> {
        Ok(u64::from_le_bytes(
            self.raw(8)?.try_into().expect("source u64 width"),
        ))
    }
    fn frame(&mut self) -> Result<&'a [u8], Error> {
        let n = self.u32()? as usize;
        self.raw(n)
    }
    fn loop_mode(&mut self) -> Result<AudioLoopMode, Error> {
        match self.byte()? {
            0 => Ok(AudioLoopMode::None),
            1 => Ok(AudioLoopMode::Whole),
            2 => Ok(AudioLoopMode::Region {
                start_frame: self.u64()?,
                end_frame: self.u64()?,
            }),
            _ => Err(self.invalid("loop mode")),
        }
    }
    fn microphone(&mut self) -> Result<MicrophoneConstraints, Error> {
        Ok(MicrophoneConstraints {
            channels: self.u16()?,
            preferred_sample_rate_hz: if self.flag()? {
                Some(self.u32()?)
            } else {
                None
            },
            echo_cancellation: self.flag()?,
            noise_suppression: self.flag()?,
            auto_gain_control: self.flag()?,
        })
    }
    pub fn command(&mut self) -> Result<Command<&'a [u8]>, Error> {
        if self.raw(DOMAIN.len())? != DOMAIN {
            return Err(self.invalid("domain"));
        }
        if self.byte()? != 1 {
            return Err(self.invalid("version"));
        }
        let command = match self.byte()? {
            0 => {
                let loop_mode = self.loop_mode()?;
                Command::Play {
                    voice: self.frame()?,
                    resource: self.frame()?,
                    bus: self.frame()?,
                    gain_db_milli: self.frame()?,
                    pan_milli: self.frame()?,
                    loop_mode,
                    start_frame: self.frame()?,
                    fade_in_millis: self.frame()?,
                }
            }
            1 => Command::Stop {
                voice: self.frame()?,
                fade_out_millis: self.frame()?,
            },
            2 => Command::StopAll {
                fade_out_millis: self.frame()?,
            },
            3 => Command::SetVoiceGain {
                voice: self.frame()?,
                gain_db_milli: self.frame()?,
                transition_millis: self.frame()?,
            },
            4 => Command::SetVoicePan {
                voice: self.frame()?,
                pan_milli: self.frame()?,
                transition_millis: self.frame()?,
            },
            5 => Command::SetBusGain {
                bus: self.frame()?,
                gain_db_milli: self.frame()?,
                transition_millis: self.frame()?,
            },
            6 => Command::SetBusMute {
                bus: self.frame()?,
                muted: self.frame()?,
            },
            7 => Command::SetEffectEnabled {
                bus: self.frame()?,
                effect: self.frame()?,
                enabled: self.frame()?,
            },
            8 => {
                let parameter = AudioEffectParameterKind::from_semantic_tag(self.byte()?)
                    .ok_or_else(|| self.invalid("effect parameter"))?;
                Command::SetEffectParameter {
                    bus: self.frame()?,
                    effect: self.frame()?,
                    parameter,
                    value: self.frame()?,
                    transition_millis: self.frame()?,
                }
            }
            9 => Command::ApplySnapshot {
                snapshot: self.frame()?,
                transition_millis: self.frame()?,
            },
            10 => {
                let constraints = self.microphone()?;
                Command::RequestMicrophone {
                    capture: self.frame()?,
                    constraints,
                }
            }
            11 => Command::StopMicrophone {
                capture: self.frame()?,
            },
            12 => {
                let bus = self.flag()?;
                Command::SetCaptureMonitor {
                    capture: self.frame()?,
                    bus: if bus { Some(self.frame()?) } else { None },
                    gain_db_milli: self.frame()?,
                }
            }
            _ => return Err(self.invalid("command tag")),
        };
        command.validate_metadata()?;
        Ok(command)
    }
    pub fn finish(&self) -> Result<(), Error> {
        if self.offset != self.bytes.len() {
            return Err(Error::Trailing {
                offset: self.offset,
            });
        }
        Ok(())
    }
}
