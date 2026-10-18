//! Counting and emission borrow the original immutable declaration tables.

use super::{
    Counts, DOMAIN, Error, Footprint, RuntimeContentDeclarationSourceLimits, charge, limit,
    reference,
};
use crate::plan::{RuntimeDialogueContentEffectTrigger, RuntimePlan};
use crate::task::semantic::TaskSemanticMeter;

pub(in crate::plan) struct PreparedRuntimeContentDeclarationEncoding<'a> {
    plan: &'a RuntimePlan,
    limits: RuntimeContentDeclarationSourceLimits,
    bytes: usize,
    footprint: Footprint,
}
impl PreparedRuntimeContentDeclarationEncoding<'_> {
    pub(in crate::plan) const fn encoded_bytes(&self) -> usize {
        self.bytes
    }
    pub(in crate::plan) const fn storage_bytes(&self) -> usize {
        self.footprint.storage
    }
    pub(in crate::plan) const fn elements(&self) -> usize {
        self.footprint.elements
    }
    pub(in crate::plan) fn append_to(self, output: &mut Vec<u8>) -> Result<(), Error> {
        let remaining = output.capacity() - output.len();
        if remaining < self.bytes {
            return Err(Error::OutputCapacity {
                required: self.bytes,
                remaining,
            });
        }
        let mut writer = Writer {
            output: Some(output),
            meter: None,
            bytes: 0,
            maximum: self.bytes,
        };
        let mut counts = Counts::new(self.limits);
        emit(self.plan, self.limits, &mut writer, &mut counts)?;
        debug_assert_eq!(writer.bytes, self.bytes);
        debug_assert_eq!(counts.footprint, self.footprint);
        Ok(())
    }
}
impl RuntimePlan {
    pub(in crate::plan) fn prepare_content_declaration_sources_encoding(
        &self,
        limits: RuntimeContentDeclarationSourceLimits,
        meter: &mut TaskSemanticMeter,
    ) -> Result<PreparedRuntimeContentDeclarationEncoding<'_>, Error> {
        meter.status()?;
        let result = (|| {
            if limits.type_count != self.type_table().len() {
                return Err(Error::Namespace {
                    kind: "type",
                    expected: limits.type_count,
                    actual: self.type_table().len(),
                });
            }
            let mut writer = Writer {
                output: None,
                meter: Some(&mut *meter),
                bytes: 0,
                maximum: limits.encoded_bytes,
            };
            let mut counts = Counts::new(limits);
            emit(self, limits, &mut writer, &mut counts)?;
            Ok(PreparedRuntimeContentDeclarationEncoding {
                plan: self,
                limits,
                bytes: writer.bytes,
                footprint: counts.footprint,
            })
        })();
        result.map_err(|error: Error| error.reject(meter))
    }
}
struct Writer<'a> {
    output: Option<&'a mut Vec<u8>>,
    meter: Option<&'a mut TaskSemanticMeter>,
    bytes: usize,
    maximum: usize,
}
impl Writer<'_> {
    fn bytes(&mut self, bytes: &[u8]) -> Result<(), Error> {
        let next = self
            .bytes
            .checked_add(bytes.len())
            .ok_or(Error::Arithmetic)?;
        limit("encoded bytes", next, self.maximum)?;
        if let Some(meter) = self.meter.as_deref_mut() {
            charge(meter, bytes.len())?;
        }
        if let Some(output) = self.output.as_deref_mut() {
            output.extend_from_slice(bytes);
        }
        self.bytes = next;
        Ok(())
    }
    fn byte(&mut self, n: u8) -> Result<(), Error> {
        self.bytes(&[n])
    }
    fn u32(&mut self, n: u32) -> Result<(), Error> {
        self.bytes(&n.to_le_bytes())
    }
    fn count(&mut self, n: usize) -> Result<(), Error> {
        self.u32(u32::try_from(n).map_err(|_| Error::Arithmetic)?)
    }
    fn u64(&mut self, n: u64) -> Result<(), Error> {
        self.bytes(&n.to_le_bytes())
    }
}
fn emit(
    plan: &RuntimePlan,
    limits: RuntimeContentDeclarationSourceLimits,
    writer: &mut Writer<'_>,
    counts: &mut Counts,
) -> Result<(), Error> {
    let templates = plan.dialogue_content_templates().rows();
    limit("templates", templates.len(), limits.templates as usize)?;
    limit(
        "formatters",
        plan.format_attempts().len(),
        limits.attempts as usize,
    )?;
    let plain = plan.dialogue_content().plain_text_context_template();
    if let Some(reference) = plain {
        let manifest = plan
            .dialogue_content_templates()
            .get(reference.id())
            .ok_or(Error::PlainTextPointer)?;
        if manifest.digest() != reference.digest() {
            return Err(Error::PlainTextPointer);
        }
    }
    writer.bytes(DOMAIN)?;
    writer.byte(1)?;
    writer.count(templates.len())?;
    writer.u32(plain.map_or(0, |reference| reference.id().get().get()))?;
    for (index, row) in templates.iter().enumerate() {
        let expected = u32::try_from(index + 1).map_err(|_| Error::Arithmetic)?;
        if row.id().get().get() != expected {
            return Err(Error::Ordinal {
                kind: "template",
                expected,
                actual: row.id().get().get(),
            });
        }
        counts.template(row.slots().len(), row.effects().len())?;
        writer.u32(expected)?;
        writer.bytes(row.digest().as_bytes())?;
        writer.count(row.slots().len())?;
        for (index, slot) in row.slots().iter().enumerate() {
            let expected = u32::try_from(index + 1).map_err(|_| Error::Arithmetic)?;
            if slot.slot().get().get() != expected {
                return Err(Error::Ordinal {
                    kind: "slot",
                    expected,
                    actual: slot.slot().get().get(),
                });
            }
            writer.u32(expected)?;
            writer.byte(slot.role().encoded())?;
            writer.bytes(slot.semantic_type().as_bytes())?;
        }
        writer.count(row.effects().len())?;
        for (index, effect) in row.effects().iter().enumerate() {
            let expected = u32::try_from(index + 1).map_err(|_| Error::Arithmetic)?;
            if effect.site().get().get() != expected {
                return Err(Error::Ordinal {
                    kind: "effect",
                    expected,
                    actual: effect.site().get().get(),
                });
            }
            counts.captures(effect.capture_types().len())?;
            writer.u32(expected)?;
            match effect.trigger() {
                RuntimeDialogueContentEffectTrigger::Content => writer.byte(0)?,
                RuntimeDialogueContentEffectTrigger::Delay { duration } => {
                    writer.byte(1)?;
                    writer.u64(duration.as_nanos())?;
                }
            }
            writer.count(effect.capture_types().len())?;
            for ty in effect.capture_types() {
                reference("capture type", ty.get().get(), limits.type_count)?;
                writer.u32(ty.get().get())?;
            }
        }
    }
    writer.count(plan.format_attempts().len())?;
    for (index, row) in plan.format_attempts().iter().enumerate() {
        let expected = u32::try_from(index + 1).map_err(|_| Error::Arithmetic)?;
        if row.id().get().get() != expected {
            return Err(Error::Ordinal {
                kind: "formatter",
                expected,
                actual: row.id().get().get(),
            });
        }
        reference(
            "formatter template",
            row.template().get().get(),
            templates.len(),
        )?;
        counts.attempt(row.operands().len())?;
        writer.u32(expected)?;
        writer.u32(row.template().get().get())?;
        writer.count(row.operands().len())?;
        for operand in row.operands() {
            reference("operand type", operand.ty().get().get(), limits.type_count)?;
            writer
                .byte(u8::try_from(operand.parameter().index()).map_err(|_| Error::Arithmetic)?)?;
            writer.u32(operand.ty().get().get())?;
        }
    }
    Ok(())
}
