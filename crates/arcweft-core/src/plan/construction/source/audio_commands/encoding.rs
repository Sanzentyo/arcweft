//! Borrowed commands use the owning ABI traversal in both counted passes.

use super::{Counts, DOMAIN, Error, Footprint, RuntimeAudioCommandSourceLimits, charge, limit};
use crate::audio::RuntimeAudioCommand as Command;
use crate::entry::schema::value_encoding::ValueResourceAdmission;
use crate::task::semantic::TaskSemanticMeter;
use crate::value::RuntimeExpr;
use arcweft_interaction_model::audio::AudioLoopMode;

pub(in crate::plan) struct EncodedRuntimeAudioCommandSource {
    bytes: Vec<u8>,
    footprint: Footprint,
}
impl EncodedRuntimeAudioCommandSource {
    pub(in crate::plan) fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub(in crate::plan) fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
    pub(in crate::plan) const fn storage_bytes(&self) -> usize {
        self.footprint.storage
    }
    pub(in crate::plan) const fn elements(&self) -> usize {
        self.footprint.elements
    }
    pub(in crate::plan) const fn node_count(&self) -> usize {
        self.footprint.nodes
    }
    pub(in crate::plan) fn append_to(self, output: &mut Vec<u8>) -> Result<(), Error> {
        let remaining = output.capacity() - output.len();
        if remaining < self.bytes.len() {
            return Err(Error::OutputCapacity {
                required: self.bytes.len(),
                remaining,
            });
        }
        output.extend_from_slice(&self.bytes);
        Ok(())
    }
}
impl Command {
    pub(in crate::plan) fn encode_source(
        &self,
        limits: RuntimeAudioCommandSourceLimits,
        resources: &ValueResourceAdmission,
        meter: &mut TaskSemanticMeter,
    ) -> Result<EncodedRuntimeAudioCommandSource, Error> {
        meter.status()?;
        let result = (|| {
            self.validate_metadata()?;
            let mut counts = Counts::new(limits)?;
            let mut writer = Writer {
                output: None,
                length: 0,
                maximum: limits.encoded_bytes,
            };
            emit(self, &mut writer, &mut counts, resources, meter)?;
            let length = writer.length;
            charge(meter, length)?;
            let mut bytes = Vec::with_capacity(length);
            let mut emitted = Counts::new(limits)?;
            let mut writer = Writer {
                output: Some(&mut bytes),
                length: 0,
                maximum: length,
            };
            emit(self, &mut writer, &mut emitted, resources, meter)?;
            debug_assert_eq!(writer.length, length);
            debug_assert_eq!(counts.footprint, emitted.footprint);
            Ok(EncodedRuntimeAudioCommandSource {
                bytes,
                footprint: counts.footprint,
            })
        })();
        result.map_err(|error: Error| error.reject(meter))
    }
}
struct Writer<'a> {
    output: Option<&'a mut Vec<u8>>,
    length: usize,
    maximum: usize,
}
impl Writer<'_> {
    fn raw(&mut self, bytes: &[u8]) -> Result<(), Error> {
        self.length = self
            .length
            .checked_add(bytes.len())
            .ok_or(Error::Arithmetic)?;
        limit("encoded bytes", self.length, self.maximum)?;
        if let Some(output) = self.output.as_mut() {
            output.extend_from_slice(bytes);
        }
        Ok(())
    }
    fn byte(&mut self, n: u8) -> Result<(), Error> {
        self.raw(&[n])
    }
    fn u32(&mut self, n: u32) -> Result<(), Error> {
        self.raw(&n.to_le_bytes())
    }
    fn expression(
        &mut self,
        expr: &RuntimeExpr,
        counts: &mut Counts,
        resources: &ValueResourceAdmission,
        meter: &mut TaskSemanticMeter,
    ) -> Result<(), Error> {
        let remaining = self.maximum - self.length;
        limit("encoded bytes", 4, remaining)?;
        let child = expr.encode_sources(counts.expression(remaining - 4), resources, meter)?;
        counts.child(child.node_count(), child.elements(), child.storage_bytes())?;
        self.u32(u32::try_from(child.bytes().len()).map_err(|_| Error::Arithmetic)?)?;
        self.raw(child.bytes())
    }
    fn metadata(&mut self, command: &Command) -> Result<(), Error> {
        self.raw(DOMAIN)?;
        self.byte(1)?;
        match command {
            Command::Play { loop_mode, .. } => {
                self.byte(0)?;
                match loop_mode {
                    AudioLoopMode::None => self.byte(0)?,
                    AudioLoopMode::Whole => self.byte(1)?,
                    AudioLoopMode::Region {
                        start_frame,
                        end_frame,
                    } => {
                        self.byte(2)?;
                        self.raw(&start_frame.to_le_bytes())?;
                        self.raw(&end_frame.to_le_bytes())?;
                    }
                }
            }
            Command::Stop { .. } => self.byte(1)?,
            Command::StopAll { .. } => self.byte(2)?,
            Command::SetVoiceGain { .. } => self.byte(3)?,
            Command::SetVoicePan { .. } => self.byte(4)?,
            Command::SetBusGain { .. } => self.byte(5)?,
            Command::SetBusMute { .. } => self.byte(6)?,
            Command::SetEffectEnabled { .. } => self.byte(7)?,
            Command::SetEffectParameter { parameter, .. } => {
                self.byte(8)?;
                self.byte(parameter.semantic_tag())?;
            }
            Command::ApplySnapshot { .. } => self.byte(9)?,
            Command::RequestMicrophone { constraints, .. } => {
                self.byte(10)?;
                self.raw(&constraints.channels.to_le_bytes())?;
                self.byte(u8::from(constraints.preferred_sample_rate_hz.is_some()))?;
                if let Some(rate) = constraints.preferred_sample_rate_hz {
                    self.u32(rate)?;
                }
                self.byte(u8::from(constraints.echo_cancellation))?;
                self.byte(u8::from(constraints.noise_suppression))?;
                self.byte(u8::from(constraints.auto_gain_control))?;
            }
            Command::StopMicrophone { .. } => self.byte(11)?,
            Command::SetCaptureMonitor { bus, .. } => {
                self.byte(12)?;
                self.byte(u8::from(bus.is_some()))?;
            }
        }
        Ok(())
    }
}
fn emit(
    command: &Command,
    writer: &mut Writer<'_>,
    counts: &mut Counts,
    resources: &ValueResourceAdmission,
    meter: &mut TaskSemanticMeter,
) -> Result<(), Error> {
    writer.metadata(command)?;
    command
        .expression_refs()
        .try_map_expressions(|expr| writer.expression(expr, counts, resources, meter))?;
    Ok(())
}
