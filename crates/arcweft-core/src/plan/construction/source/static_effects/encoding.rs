//! Counted source emission borrows the real request and its original live value.

use super::{Counts, DOMAIN, Error, Footprint, RuntimeStaticEffectSourceLimits, charge, limit};
use crate::effect::{
    LineEffectRequest as Effect, RuntimeAssertionProfile, RuntimeField, RuntimeWaitTarget,
};
use crate::entry::schema::value_encoding::ValueResourceAdmission;
use crate::task::semantic::TaskSemanticMeter;
use crate::value::RuntimeValue;

pub(in crate::plan) struct EncodedRuntimeStaticEffectSource {
    bytes: Vec<u8>,
    footprint: Footprint,
}
impl EncodedRuntimeStaticEffectSource {
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
        if self.bytes.len() > remaining {
            return Err(Error::OutputCapacity {
                required: self.bytes.len(),
                remaining,
            });
        }
        output.extend_from_slice(&self.bytes);
        Ok(())
    }
}
impl Effect {
    pub(in crate::plan) fn encode_static_source(
        &self,
        limits: RuntimeStaticEffectSourceLimits,
        resources: &ValueResourceAdmission,
        meter: &mut TaskSemanticMeter,
    ) -> Result<EncodedRuntimeStaticEffectSource, Error> {
        meter.status()?;
        let result = (|| {
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
            Ok(EncodedRuntimeStaticEffectSource {
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
    fn length(&mut self, n: usize) -> Result<(), Error> {
        self.length = self.length.checked_add(n).ok_or(Error::Arithmetic)?;
        limit("encoded bytes", self.length, self.maximum)
    }
    fn raw(&mut self, bytes: &[u8]) -> Result<(), Error> {
        self.length(bytes.len())?;
        if let Some(output) = &mut self.output {
            output.extend_from_slice(bytes);
        }
        Ok(())
    }
    fn byte(&mut self, n: u8) -> Result<(), Error> {
        self.raw(&[n])
    }
    fn count(&mut self, n: usize) -> Result<(), Error> {
        self.raw(
            &u32::try_from(n)
                .map_err(|_| Error::Arithmetic)?
                .to_le_bytes(),
        )
    }
    fn text(&mut self, text: &str, counts: &mut Counts) -> Result<(), Error> {
        counts.storage(text.len())?;
        self.count(text.len())?;
        self.raw(text.as_bytes())
    }
    fn optional_text(&mut self, text: Option<&str>, counts: &mut Counts) -> Result<(), Error> {
        self.byte(u8::from(text.is_some()))?;
        if let Some(text) = text {
            self.text(text, counts)?;
        }
        Ok(())
    }
    fn fields(&mut self, fields: &[RuntimeField], counts: &mut Counts) -> Result<(), Error> {
        counts.array::<RuntimeField>(fields.len())?;
        self.count(fields.len())?;
        for field in fields {
            self.text(&field.name, counts)?;
            self.text(&field.value, counts)?;
        }
        Ok(())
    }
    fn value(
        &mut self,
        value: &RuntimeValue,
        counts: &mut Counts,
        resources: &ValueResourceAdmission,
        meter: &mut TaskSemanticMeter,
    ) -> Result<(), Error> {
        let remaining = self.maximum - self.length;
        limit("encoded bytes", 4, remaining)?;
        let prepared = value.prepare_literal_source_encoding(
            counts.literal_limits(remaining - 4),
            resources,
            meter,
        )?;
        let length = prepared.encoded_bytes();
        counts.child(
            prepared.source_nodes(),
            prepared.elements(),
            prepared.storage_bytes(),
        )?;
        self.count(length)?;
        self.length(length)?;
        if let Some(output) = &mut self.output {
            prepared.append_to(output)?;
        }
        Ok(())
    }
}
fn emit(
    effect: &Effect,
    writer: &mut Writer<'_>,
    counts: &mut Counts,
    resources: &ValueResourceAdmission,
    meter: &mut TaskSemanticMeter,
) -> Result<(), Error> {
    writer.raw(DOMAIN)?;
    writer.byte(1)?;
    writer.byte(match effect {
        Effect::Wait(_) => 0,
        Effect::Audio(_) => {
            return Err(Error::Invalid {
                kind: "raw Audio expression carrier",
                offset: writer.length,
            });
        }
        Effect::Call(_) => 2,
        Effect::Log(_) => 3,
        Effect::SignalWrite(_) => 4,
        Effect::MetricWrite(_) => 5,
        Effect::EmitEvent(_) => 6,
        Effect::Out(_) => 7,
        Effect::Return(_) => 8,
        Effect::Goto(_) => 9,
        Effect::Panic(_) => 10,
        Effect::Fail(_) => 11,
        Effect::Bail(_) => 12,
        Effect::Ensure { .. } => 13,
        Effect::Assert(_) => 14,
        Effect::Close(_) => 15,
        Effect::Select(_) => 16,
        Effect::Break { .. } => 17,
        Effect::Continue { .. } => 18,
    })?;
    match effect {
        Effect::Wait(RuntimeWaitTarget::Duration(duration)) => {
            writer.raw(&duration.as_nanos().to_le_bytes())?
        }
        Effect::Audio(_) => unreachable!("raw Audio was rejected before metadata emission"),
        Effect::Call(call) => {
            writer.text(&call.callee, counts)?;
            counts.array::<String>(call.args.len())?;
            writer.count(call.args.len())?;
            for argument in &call.args {
                writer.text(argument, counts)?;
            }
        }
        Effect::Log(log) => {
            writer.text(&log.level, counts)?;
            writer.text(&log.message, counts)?;
            writer.fields(&log.fields, counts)?;
        }
        Effect::SignalWrite(value) | Effect::MetricWrite(value) => {
            writer.text(&value.target, counts)?;
            writer.value(value.value(), counts, resources, meter)?;
        }
        Effect::EmitEvent(event) => {
            writer.text(&event.event, counts)?;
            writer.fields(&event.fields, counts)?;
        }
        Effect::Out(out) => {
            writer.optional_text(out.label.as_deref(), counts)?;
            writer.text(&out.value, counts)?;
        }
        Effect::Return(text)
        | Effect::Goto(text)
        | Effect::Panic(text)
        | Effect::Fail(text)
        | Effect::Bail(text)
        | Effect::Close(text)
        | Effect::Select(text) => writer.text(text, counts)?,
        Effect::Ensure { condition, message } => {
            writer.text(condition, counts)?;
            writer.text(message, counts)?;
        }
        Effect::Assert(assertion) => {
            writer.raw(assertion.guard().as_bytes())?;
            writer.byte(match assertion.profile() {
                RuntimeAssertionProfile::Always => 0,
                RuntimeAssertionProfile::DebugOnly => 1,
            })?;
            writer.text(assertion.condition(), counts)?;
            writer.text(assertion.message(), counts)?;
        }
        Effect::Break { label, value } => {
            writer.optional_text(label.as_deref(), counts)?;
            writer.optional_text(value.as_deref(), counts)?;
        }
        Effect::Continue { label } => writer.optional_text(label.as_deref(), counts)?,
    }
    Ok(())
}
