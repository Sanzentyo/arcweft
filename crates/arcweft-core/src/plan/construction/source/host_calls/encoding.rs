//! Counted Host source emission delegates values and bindings to their owners.

use super::{Counts, DOMAIN, Error, Footprint, RuntimeHostCallSourceLimits, charge, limit};
use crate::entry::schema::value_encoding::ValueResourceAdmission;
use crate::plan::{FlowOp, RuntimePlan};
use crate::step::RuntimeHostCallMode;
use crate::task::RuntimeHostArgumentTemplate;
use crate::task::semantic::TaskSemanticMeter;

pub(in crate::plan) struct EncodedRuntimeHostCallSource {
    bytes: Vec<u8>,
    footprint: Footprint,
}
impl EncodedRuntimeHostCallSource {
    pub(in crate::plan) fn bytes(&self) -> &[u8] {
        &self.bytes
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
impl FlowOp {
    pub(in crate::plan) fn encode_host_call_source(
        &self,
        plan: &RuntimePlan,
        limits: RuntimeHostCallSourceLimits,
        resources: &ValueResourceAdmission,
        meter: &mut TaskSemanticMeter,
    ) -> Result<EncodedRuntimeHostCallSource, Error> {
        meter.status()?;
        let result = (|| {
            if limits.task_count != plan.task_plans().len()
                || limits.expression.type_count != plan.type_table().len()
                || limits.expression.local_count != plan.local_declarations().len()
            {
                return Err(Error::TaskCoordinate);
            }
            let mut counts = Counts::new(limits)?;
            let mut writer = Writer {
                output: None,
                length: 0,
                maximum: limits.encoded_bytes,
            };
            emit(self, plan, &mut writer, &mut counts, resources, meter)?;
            let length = writer.length;
            charge(meter, length)?;
            let mut bytes = Vec::with_capacity(length);
            let mut emitted = Counts::new(limits)?;
            let mut writer = Writer {
                output: Some(&mut bytes),
                length: 0,
                maximum: length,
            };
            emit(self, plan, &mut writer, &mut emitted, resources, meter)?;
            debug_assert_eq!(writer.length, length);
            debug_assert_eq!(counts.footprint, emitted.footprint);
            Ok(EncodedRuntimeHostCallSource {
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
    fn advance(&mut self, count: usize) -> Result<(), Error> {
        self.length = self.length.checked_add(count).ok_or(Error::Arithmetic)?;
        limit("encoded bytes", self.length, self.maximum)
    }
    fn raw(&mut self, bytes: &[u8]) -> Result<(), Error> {
        self.advance(bytes.len())?;
        if let Some(output) = self.output.as_mut() {
            output.extend_from_slice(bytes);
        }
        Ok(())
    }
    fn byte(&mut self, value: u8) -> Result<(), Error> {
        self.raw(&[value])
    }
    fn u32(&mut self, value: u32) -> Result<(), Error> {
        self.raw(&value.to_le_bytes())
    }
    fn count(&mut self, value: usize) -> Result<(), Error> {
        self.u32(u32::try_from(value).map_err(|_| Error::Arithmetic)?)
    }
    fn frame(&mut self, bytes: &[u8]) -> Result<(), Error> {
        self.count(bytes.len())?;
        self.raw(bytes)
    }
    fn text(&mut self, text: &str, counts: &mut Counts) -> Result<(), Error> {
        counts.text(text.len())?;
        self.frame(text.as_bytes())
    }
    fn frame_capacity(&self) -> Result<usize, Error> {
        let remaining = self.maximum - self.length;
        limit("encoded bytes", 4, remaining)?;
        Ok(remaining - 4)
    }
}
fn emit(
    op: &FlowOp,
    plan: &RuntimePlan,
    writer: &mut Writer<'_>,
    counts: &mut Counts,
    resources: &ValueResourceAdmission,
    meter: &mut TaskSemanticMeter,
) -> Result<(), Error> {
    let FlowOp::HostCall { binding, target } = op else {
        return Err(Error::Operation);
    };
    if plan.task_plan(&target.producer.plan).is_none() {
        return Err(Error::TaskCoordinate);
    }
    if plan.type_table().get(target.result).is_none() {
        return Err(Error::Reference {
            kind: "result type",
            actual: target.result.get().get(),
            maximum: plan.type_table().len(),
        });
    }
    writer.raw(DOMAIN)?;
    writer.byte(1)?;
    writer.u32(target.producer.plan.ordinal())?;
    writer.raw(target.producer.contract.as_bytes())?;
    writer.raw(target.producer.site.as_bytes())?;
    writer.text(&target.public_id, counts)?;
    writer.text(&target.capability, counts)?;
    writer.text(&target.operation, counts)?;
    writer.byte(u8::from(target.contract.is_some()))?;
    if let Some(contract) = target.contract {
        writer.raw(contract.as_bytes())?;
    }
    writer.u32(target.result.get().get())?;
    writer.byte(match target.mode {
        RuntimeHostCallMode::Immediate => 0,
        RuntimeHostCallMode::Suspend => 1,
    })?;
    writer.byte(u8::from(target.deterministic))?;
    // Every argument slot and name is counted before preparing the first value.
    counts.array(
        target.args.len(),
        size_of::<crate::plan::RuntimeHostArgumentSeed>()
            + size_of::<RuntimeHostArgumentTemplate>(),
    )?;
    for argument in &target.args {
        if let RuntimeHostArgumentTemplate::Named(_, named) = argument {
            counts.text(named.name.len())?;
        }
    }
    writer.byte(u8::from(binding.is_some()))?;
    if let Some(binding) = binding {
        let child = binding.prepare_sources_encoding(
            counts.pattern(writer.frame_capacity()?),
            resources,
            meter,
        )?;
        counts.child(child.node_count(), child.elements(), child.storage_bytes())?;
        writer.count(child.encoded_bytes())?;
        writer.advance(child.encoded_bytes())?;
        if let Some(output) = writer.output.as_mut() {
            child.append_to(output)?;
        }
    }
    writer.count(target.args.len())?;
    for argument in &target.args {
        writer.raw(argument.identity().as_bytes())?;
        match argument {
            RuntimeHostArgumentTemplate::Positional(..) => writer.byte(0)?,
            RuntimeHostArgumentTemplate::Named(_, named) => {
                writer.byte(1)?;
                writer.frame(named.name.as_bytes())?;
            }
            RuntimeHostArgumentTemplate::Spread(..) => writer.byte(2)?,
        }
        let child = argument.value().encode_sources(
            counts.expression(writer.frame_capacity()?),
            resources,
            meter,
        )?;
        counts.child(child.node_count(), child.elements(), child.storage_bytes())?;
        writer.frame(child.bytes())?;
    }
    Ok(())
}
