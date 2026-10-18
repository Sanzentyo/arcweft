//! Both counted passes borrow the actual admitted call site and child owners.

use super::{
    Counts, DOMAIN, Error, Footprint, RuntimeProjectCallSourceLimits, charge, limit, reference,
};
use crate::entry::schema::value_encoding::ValueResourceAdmission;
use crate::pattern::RuntimePattern;
use crate::plan::{
    RuntimeProjectCallAttachedPresence as Presence,
    RuntimeProjectCallOrdinaryMaterialization as Ordinary, RuntimeProjectCallSite,
};
use crate::task::semantic::TaskSemanticMeter;
use crate::value::RuntimeExpr;

pub(in crate::plan) struct EncodedRuntimeProjectCallSource {
    bytes: Vec<u8>,
    footprint: Footprint,
}
impl EncodedRuntimeProjectCallSource {
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
impl RuntimeProjectCallSite {
    pub(in crate::plan) fn encode_source(
        &self,
        limits: RuntimeProjectCallSourceLimits,
        resources: &ValueResourceAdmission,
        meter: &mut TaskSemanticMeter,
    ) -> Result<EncodedRuntimeProjectCallSource, Error> {
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
            Ok(EncodedRuntimeProjectCallSource {
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
    fn advance(&mut self, n: usize) -> Result<(), Error> {
        self.length = self.length.checked_add(n).ok_or(Error::Arithmetic)?;
        limit("encoded bytes", self.length, self.maximum)
    }
    fn raw(&mut self, bytes: &[u8]) -> Result<(), Error> {
        self.advance(bytes.len())?;
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
    fn pattern(
        &mut self,
        pattern: &RuntimePattern,
        counts: &mut Counts,
        resources: &ValueResourceAdmission,
        meter: &mut TaskSemanticMeter,
    ) -> Result<(), Error> {
        let remaining = self.maximum - self.length;
        limit("encoded bytes", 4, remaining)?;
        let child =
            pattern.prepare_sources_encoding(counts.pattern(remaining - 4), resources, meter)?;
        counts.child(child.node_count(), child.elements(), child.storage_bytes())?;
        self.count(child.encoded_bytes())?;
        self.advance(child.encoded_bytes())?;
        if let Some(output) = self.output.as_mut() {
            child.append_to(output)?;
        }
        Ok(())
    }
    fn ty(
        &mut self,
        id: crate::runtime_id::RuntimePlanTypeId,
        counts: &Counts,
    ) -> Result<(), Error> {
        reference("type", id.get().get(), counts.limits.expression.type_count)?;
        self.count(id.get().get() as usize)
    }
}
fn emit(
    site: &RuntimeProjectCallSite,
    writer: &mut Writer<'_>,
    counts: &mut Counts,
    resources: &ValueResourceAdmission,
    meter: &mut TaskSemanticMeter,
) -> Result<(), Error> {
    let call = site.plan();
    writer.raw(DOMAIN)?;
    writer.byte(1)?;
    reference(
        "callable state",
        call.state().get().get(),
        counts.limits.expression.state_count,
    )?;
    writer.count(call.state().get().get() as usize)?;
    writer.count(call.completed_group() as usize)?;
    writer.expression(call.callee(), counts, resources, meter)?;
    counts.operands(call.operands().len())?;
    writer.count(call.operands().len())?;
    for operand in call.operands() {
        writer.byte(match operand.mode() {
            crate::value::RuntimeCallArgumentMode::Value => 0,
            crate::value::RuntimeCallArgumentMode::Spread => 1,
        })?;
        writer.expression(operand.value(), counts, resources, meter)?;
    }
    counts.ordinary(call.ordinary().len())?;
    writer.count(call.ordinary().len())?;
    for row in call.ordinary() {
        writer.count(row.parameter() as usize)?;
        writer.ty(row.abi_ty(), counts)?;
        writer.ty(row.binding_ty(), counts)?;
        match row {
            Ordinary::Fixed(row) => {
                writer.byte(0)?;
                writer.count(row.source_index() as usize)?;
            }
            Ordinary::Rest(row) => {
                writer.byte(1)?;
                counts.sources(row.source_indices().len())?;
                writer.count(row.source_indices().len())?;
                for source in row.source_indices() {
                    writer.count(*source as usize)?;
                }
            }
        }
    }
    writer.byte(u8::from(call.attached().is_some()))?;
    if let Some(row) = call.attached() {
        writer.ty(row.abi_ty(), counts)?;
        writer.ty(row.binding_ty(), counts)?;
        writer.byte(match row.presence() {
            Presence::RequiredPresent => 0,
            Presence::OptionalPresent => 1,
            Presence::OptionalOmitted => 2,
            Presence::DefaultedPresent => 3,
            Presence::DefaultedOmitted => 4,
        })?;
        writer.byte(u8::from(row.source_index().is_some()))?;
        if let Some(source) = row.source_index() {
            writer.count(source as usize)?;
        }
    }
    writer.pattern(site.result(), counts, resources, meter)
}
