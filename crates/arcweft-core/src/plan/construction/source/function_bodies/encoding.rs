use super::{
    Counts, DOMAIN, Error, Footprint, RuntimeFunctionBodySourceLimits, charge, limit, namespace,
    reading,
};
use crate::entry::schema::value_encoding::ValueResourceAdmission;
use crate::plan::{RuntimeFunctionSiteBody, RuntimePlan};
use crate::task::semantic::TaskSemanticMeter;

pub(in crate::plan) struct EncodedRuntimeFunctionBodySources {
    bytes: Vec<u8>,
    footprint: Footprint,
}
impl EncodedRuntimeFunctionBodySources {
    pub(in crate::plan) fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub(in crate::plan) const fn node_count(&self) -> usize {
        self.footprint.nodes
    }
    pub(in crate::plan) const fn elements(&self) -> usize {
        self.footprint.elements
    }
    pub(in crate::plan) const fn storage_bytes(&self) -> usize {
        self.footprint.storage
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
impl RuntimePlan {
    pub(in crate::plan) fn encode_function_body_sources(
        &self,
        limits: RuntimeFunctionBodySourceLimits,
        resources: &ValueResourceAdmission,
        meter: &mut TaskSemanticMeter,
    ) -> Result<EncodedRuntimeFunctionBodySources, Error> {
        meter.status()?;
        let result = (|| {
            let rows = self.function_sites().len();
            namespace("function declarations", limits.functions, rows)?;
            let mut counts = Counts::new(limits, rows)?;
            let mut length = DOMAIN.len().checked_add(5).ok_or(Error::Arithmetic)?;
            limit("encoded bytes", length, limits.encoded_bytes)?;
            for (index, function) in self.function_sites().iter().enumerate() {
                let packet = encode_row(
                    self,
                    index,
                    function,
                    &counts,
                    limits.encoded_bytes - length,
                    resources,
                    meter,
                )?;
                let mut cursor = reading::Cursor::new(&packet);
                cursor.row(index, &mut counts, resources, meter)?;
                cursor.finish()?;
                length = length.checked_add(packet.len()).ok_or(Error::Arithmetic)?;
                limit("encoded bytes", length, limits.encoded_bytes)?;
            }
            charge(meter, length)?;
            let mut output = Writer {
                bytes: Vec::with_capacity(length),
                maximum: length,
            };
            output.raw(DOMAIN)?;
            output.byte(1)?;
            output.count(rows)?;
            let mut emitted = Counts::new(limits, rows)?;
            for (index, function) in self.function_sites().iter().enumerate() {
                let packet = encode_row(
                    self,
                    index,
                    function,
                    &emitted,
                    limits.encoded_bytes - output.bytes.len(),
                    resources,
                    meter,
                )?;
                let mut cursor = reading::Cursor::new(&packet);
                cursor.row(index, &mut emitted, resources, meter)?;
                cursor.finish()?;
                output.raw(&packet)?;
            }
            debug_assert_eq!(output.bytes.len(), length);
            debug_assert_eq!(emitted.footprint, counts.footprint);
            Ok(EncodedRuntimeFunctionBodySources {
                bytes: output.bytes,
                footprint: counts.footprint,
            })
        })();
        result.map_err(|error: Error| error.reject(meter))
    }
}
fn encode_row(
    plan: &RuntimePlan,
    index: usize,
    function: &crate::plan::RuntimeFunctionSite,
    counts: &Counts,
    maximum: usize,
    resources: &ValueResourceAdmission,
    meter: &mut TaskSemanticMeter,
) -> Result<Vec<u8>, Error> {
    let mut counts = *counts;
    if let RuntimeFunctionSiteBody::Executable(body) = function.body() {
        let bytes = body.effects().iter().try_fold(0_usize, |total, effect| {
            total
                .checked_add(effect.as_str().len())
                .ok_or(Error::Arithmetic)
        })?;
        counts.effects(body.effects().len(), bytes)?;
    }
    let mut output = Writer {
        bytes: Vec::new(),
        maximum,
    };
    output.count(index.checked_add(1).ok_or(Error::Arithmetic)?)?;
    output.raw(function.definition().as_bytes())?;
    match function.body() {
        RuntimeFunctionSiteBody::Expression(expr) => {
            output.byte(0)?;
            output.count(0)?;
            let child = expr.encode_sources(
                counts.expression(output.frame_capacity()?),
                resources,
                meter,
            )?;
            output.frame(child.bytes())?;
        }
        RuntimeFunctionSiteBody::Executable(body) => {
            output.byte(1)?;
            output.count(body.effects().len())?;
            for effect in body.effects().iter() {
                output.frame(effect.as_str().as_bytes())?;
            }
            let child = plan.encode_flow_source(
                body.ops(),
                counts.flow(output.frame_capacity()?),
                resources,
                meter,
            )?;
            output.frame(child.bytes())?;
        }
    }
    Ok(output.bytes)
}
struct Writer {
    bytes: Vec<u8>,
    maximum: usize,
}
impl Writer {
    fn frame_capacity(&self) -> Result<usize, Error> {
        self.maximum
            .checked_sub(self.bytes.len())
            .and_then(|remaining| remaining.checked_sub(4))
            .ok_or(Error::Limit {
                kind: "encoded bytes",
                actual: self.bytes.len().checked_add(4).ok_or(Error::Arithmetic)?,
                maximum: self.maximum,
            })
    }
    fn raw(&mut self, bytes: &[u8]) -> Result<(), Error> {
        let length = self
            .bytes
            .len()
            .checked_add(bytes.len())
            .ok_or(Error::Arithmetic)?;
        limit("encoded bytes", length, self.maximum)?;
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }
    fn byte(&mut self, byte: u8) -> Result<(), Error> {
        self.raw(&[byte])
    }
    fn count(&mut self, count: usize) -> Result<(), Error> {
        self.raw(
            &u32::try_from(count)
                .map_err(|_| Error::Arithmetic)?
                .to_le_bytes(),
        )
    }
    fn frame(&mut self, bytes: &[u8]) -> Result<(), Error> {
        self.count(bytes.len())?;
        self.raw(bytes)
    }
}
