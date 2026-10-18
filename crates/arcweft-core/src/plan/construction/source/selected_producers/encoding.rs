//! Counted emission delegates each value graph to its owning Source codec.

use super::{Counts, DOMAIN, Error, Footprint, RuntimeSelectedProducerSourceLimits, charge, limit};
use crate::entry::schema::value_encoding::ValueResourceAdmission;
use crate::plan::{FlowOp, RuntimePlan};
use crate::task::semantic::TaskSemanticMeter;
use crate::task::{
    AssetLoadKind, HostRestartPolicy, NeedProducerRequestProjection, RuntimeHostArgumentTemplate,
    TaskHost,
};

pub(in crate::plan) struct EncodedRuntimeSelectedProducerSource {
    bytes: Vec<u8>,
    footprint: Footprint,
}
impl EncodedRuntimeSelectedProducerSource {
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
impl FlowOp {
    pub(in crate::plan) fn encode_selected_producer_source(
        &self,
        plan: &RuntimePlan,
        limits: RuntimeSelectedProducerSourceLimits,
        resources: &ValueResourceAdmission,
        meter: &mut TaskSemanticMeter,
    ) -> Result<EncodedRuntimeSelectedProducerSource, Error> {
        meter.status()?;
        let result = (|| {
            if limits.expression.type_count != plan.type_table().len()
                || limits.task_count != plan.task_plans().len()
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
            Ok(EncodedRuntimeSelectedProducerSource {
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
    fn advance(&mut self, bytes: usize) -> Result<(), Error> {
        self.length = self.length.checked_add(bytes).ok_or(Error::Arithmetic)?;
        limit("encoded bytes", self.length, self.maximum)
    }
    fn raw(&mut self, bytes: &[u8]) -> Result<(), Error> {
        self.advance(bytes.len())?;
        if let Some(output) = self.output.as_mut() {
            output.extend_from_slice(bytes);
        }
        Ok(())
    }
    fn byte(&mut self, byte: u8) -> Result<(), Error> {
        self.raw(&[byte])
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
    fn ty(
        &mut self,
        ty: crate::pattern::RuntimeSemanticTypeId,
        plan: &RuntimePlan,
        meter: &mut TaskSemanticMeter,
    ) -> Result<(), Error> {
        meter
            .charge_work(u64::try_from(plan.type_table().len()).map_err(|_| Error::Arithmetic)?)?;
        let id = plan
            .type_table()
            .id_for_semantic(ty)
            .ok_or(Error::Invalid {
                kind: "original type",
                offset: self.length,
            })?;
        self.u32(id.get().get())
    }
    fn request(
        &mut self,
        request: &NeedProducerRequestProjection,
        counts: &mut Counts,
    ) -> Result<(), Error> {
        match request {
            NeedProducerRequestProjection::AssetLoad {
                kind,
                argument_name,
            } => {
                self.byte(match kind {
                    AssetLoadKind::Image => 0,
                    AssetLoadKind::Voice => 1,
                })?;
                self.text(argument_name, counts)
            }
            NeedProducerRequestProjection::ExternCapability {
                capability,
                operation,
                contract,
                argument_names,
            } => {
                self.byte(2)?;
                self.text(&capability.0, counts)?;
                self.text(operation, counts)?;
                self.raw(contract.as_bytes())?;
                counts.array(argument_names.len(), 2 * size_of::<Option<String>>())?;
                self.count(argument_names.len())?;
                for name in argument_names {
                    self.byte(u8::from(name.is_some()))?;
                    if let Some(name) = name {
                        self.text(name, counts)?;
                    }
                }
                Ok(())
            }
        }
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
    let FlowOp::StartNeedProducer { binding, target } = op else {
        return Err(Error::Operation);
    };
    if plan.task_plan(target.task_plan()).is_none() {
        return Err(Error::TaskCoordinate);
    }
    writer.raw(DOMAIN)?;
    writer.byte(1)?;
    writer.u32(target.task_plan().ordinal())?;
    let schema = target.plan();
    writer.raw(schema.contract().as_bytes())?;
    writer.raw(schema.site().as_bytes())?;
    writer.byte(schema.policy().semantic_tag())?;
    writer.byte(match schema.restart() {
        HostRestartPolicy::MustBeQuiescent => 0,
        HostRestartPolicy::Restartable => 1,
    })?;
    writer.byte(schema.class().semantic_tag())?;
    writer.u32(schema.priority().0.cast_unsigned())?;
    writer.text(&schema.cancel_scope().0, counts)?;
    writer.request(schema.request(), counts)?;
    writer.ty(schema.payload_type(), plan, meter)?;
    counts.array(
        schema.argument_types().len(),
        2 * size_of::<crate::pattern::RuntimeSemanticTypeId>(),
    )?;
    writer.count(schema.argument_types().len())?;
    for ty in schema.argument_types() {
        writer.ty(*ty, plan, meter)?;
    }
    // Account for the complete argument array and all names before preparing
    // the first nested value graph or retaining a binding-source index.
    counts.array(
        target.arguments().len(),
        size_of::<crate::plan::RuntimeHostArgumentSeed>()
            + size_of::<RuntimeHostArgumentTemplate>(),
    )?;
    for argument in target.arguments() {
        if let Some(name) = argument.name() {
            counts.text(name.len())?;
        }
    }
    let pattern = binding.prepare_sources_encoding(
        counts.pattern(writer.frame_capacity()?),
        resources,
        meter,
    )?;
    counts.child(
        pattern.node_count(),
        pattern.elements(),
        pattern.storage_bytes(),
    )?;
    writer.count(pattern.encoded_bytes())?;
    writer.advance(pattern.encoded_bytes())?;
    if let Some(output) = writer.output.as_mut() {
        pattern.append_to(output)?;
    }
    writer.count(target.arguments().len())?;
    for argument in target.arguments() {
        writer.raw(argument.identity().as_bytes())?;
        match argument {
            RuntimeHostArgumentTemplate::Positional(..) => writer.byte(0)?,
            RuntimeHostArgumentTemplate::Named(_, named) => {
                writer.byte(1)?;
                writer.frame(named.name.as_bytes())?;
            }
            RuntimeHostArgumentTemplate::Spread(..) => {
                return Err(Error::Invalid {
                    kind: "selected producer spread",
                    offset: writer.length,
                });
            }
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
