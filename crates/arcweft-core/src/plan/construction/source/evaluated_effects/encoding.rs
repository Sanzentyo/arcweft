//! Counted emission borrows the original effect and delegates child expressions.

use super::{Counts, DOMAIN, Error, Footprint, RuntimeEvaluatedEffectSourceLimits, charge, limit};
use crate::effect::{
    RuntimeAssertionProfile, RuntimeDropPolicyExpr, RuntimeEffectExpr as Effect,
    RuntimeEffectFieldExpr,
};
use crate::entry::schema::value_encoding::ValueResourceAdmission;
use crate::task::semantic::TaskSemanticMeter;
use crate::value::RuntimeExpr;

pub(in crate::plan) struct EncodedRuntimeEvaluatedEffectSource {
    bytes: Vec<u8>,
    footprint: Footprint,
}
impl EncodedRuntimeEvaluatedEffectSource {
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
impl Effect {
    pub(in crate::plan) fn encode_source(
        &self,
        limits: RuntimeEvaluatedEffectSourceLimits,
        resources: &ValueResourceAdmission,
        meter: &mut TaskSemanticMeter,
    ) -> Result<EncodedRuntimeEvaluatedEffectSource, Error> {
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
            Ok(EncodedRuntimeEvaluatedEffectSource {
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
        self.count(child.bytes().len())?;
        self.raw(child.bytes())
    }
    fn fields(
        &mut self,
        fields: &[RuntimeEffectFieldExpr],
        counts: &mut Counts,
        resources: &ValueResourceAdmission,
        meter: &mut TaskSemanticMeter,
    ) -> Result<(), Error> {
        counts.fields(fields.len())?;
        self.count(fields.len())?;
        for field in fields {
            self.text(&field.name, counts)?;
            self.expression(&field.value, counts, resources, meter)?;
        }
        Ok(())
    }

    fn registry_key(
        &mut self,
        key: &arcweft_id::LifetimeRegistryKey,
        counts: &mut Counts,
    ) -> Result<(), Error> {
        counts.array(
            key.segments().len(),
            size_of::<arcweft_id::DeclarationName>(),
        )?;
        let length = key.segments().iter().try_fold(
            8_usize
                .checked_add(key.scope().as_str().len())
                .ok_or(Error::Arithmetic)?,
            |length, name| {
                length
                    .checked_add(4)
                    .and_then(|n| n.checked_add(name.as_str().len()))
                    .ok_or(Error::Arithmetic)
            },
        )?;
        self.count(length)?;
        self.text(key.scope().as_str(), counts)?;
        self.count(key.segments().len())?;
        for name in key.segments() {
            self.text(name.as_str(), counts)?;
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
    match effect {
        Effect::Log {
            level,
            message,
            fields,
        } => {
            writer.byte(0)?;
            writer.text(level, counts)?;
            writer.expression(message, counts, resources, meter)?;
            writer.fields(fields, counts, resources, meter)?;
        }
        Effect::SignalWrite { target, value } | Effect::MetricWrite { target, value } => {
            writer.byte(if matches!(effect, Effect::SignalWrite { .. }) {
                1
            } else {
                2
            })?;
            writer.expression(target, counts, resources, meter)?;
            writer.expression(value, counts, resources, meter)?;
        }
        Effect::RegistryWrite { key, value } => {
            writer.byte(10)?;
            writer.registry_key(key, counts)?;
            writer.expression(value, counts, resources, meter)?;
        }
        Effect::EmitEvent { event, fields } => {
            writer.byte(3)?;
            writer.expression(event, counts, resources, meter)?;
            writer.fields(fields, counts, resources, meter)?;
        }
        Effect::Panic(message) | Effect::Fail(message) | Effect::Bail(message) => {
            writer.byte(match effect {
                Effect::Panic(_) => 4,
                Effect::Fail(_) => 5,
                _ => 6,
            })?;
            writer.expression(message, counts, resources, meter)?;
        }
        Effect::Ensure { condition, message } => {
            writer.byte(7)?;
            writer.expression(condition, counts, resources, meter)?;
            writer.expression(message, counts, resources, meter)?;
        }
        Effect::Drop { target, policy } => {
            writer.byte(8)?;
            writer.expression(target, counts, resources, meter)?;
            writer.byte(match policy {
                RuntimeDropPolicyExpr::Default => 0,
                RuntimeDropPolicyExpr::Cancel => 1,
                RuntimeDropPolicyExpr::Stop { .. } => 2,
                RuntimeDropPolicyExpr::Finish => 3,
                RuntimeDropPolicyExpr::Release => 4,
                RuntimeDropPolicyExpr::Detach => 5,
            })?;
            if let RuntimeDropPolicyExpr::Stop { fade } = policy {
                writer.expression(fade, counts, resources, meter)?;
            }
        }
        Effect::Assert {
            guard,
            condition,
            message,
            profile,
        } => {
            writer.byte(9)?;
            writer.raw(guard.as_bytes())?;
            writer.byte(match profile {
                RuntimeAssertionProfile::Always => 0,
                RuntimeAssertionProfile::DebugOnly => 1,
            })?;
            writer.text(message, counts)?;
            writer.expression(condition, counts, resources, meter)?;
        }
    }
    Ok(())
}
