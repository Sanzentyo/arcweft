//! Borrowed invocation inventories use the same counted writer for sizing and
//! reserved-capacity emission. No code table or identity payload is cloned.

use super::{
    Counts, DOMAIN, Error, Footprint, RuntimeInvocationSourceLimits, charge, limit, reference,
};
use crate::entry::{RuntimeCallableExecutableCode, RuntimeCallableRole};
use crate::plan::{FlowRuntimeId, RuntimePlan};
use crate::runtime_id::{RuntimeIdFamily, RuntimeIdPath};
use crate::task::semantic::TaskSemanticMeter;

pub(in crate::plan) struct PreparedRuntimeInvocationEncoding<'a> {
    plan: &'a RuntimePlan,
    limits: RuntimeInvocationSourceLimits,
    bytes: usize,
    footprint: Footprint,
}
impl PreparedRuntimeInvocationEncoding<'_> {
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
    pub(in crate::plan) fn prepare_invocation_sources_encoding(
        &self,
        limits: RuntimeInvocationSourceLimits,
        meter: &mut TaskSemanticMeter,
    ) -> Result<PreparedRuntimeInvocationEncoding<'_>, Error> {
        meter.status()?;
        let result = (|| {
            for (kind, expected, actual) in [
                ("type", limits.type_count, self.type_table().len()),
                (
                    "function",
                    limits.function_count,
                    self.function_sites().len(),
                ),
            ] {
                if expected != actual {
                    return Err(Error::Namespace {
                        kind,
                        expected,
                        actual,
                    });
                }
            }
            let mut writer = Writer {
                output: None,
                meter: Some(meter),
                bytes: 0,
                maximum: limits.encoded_bytes,
            };
            let mut counts = Counts::new(limits);
            emit(self, limits, &mut writer, &mut counts)?;
            Ok(PreparedRuntimeInvocationEncoding {
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
    fn raw(&mut self, bytes: &[u8]) -> Result<(), Error> {
        if let Some(meter) = self.meter.as_mut() {
            charge(meter, bytes.len())?;
        }
        self.bytes = self
            .bytes
            .checked_add(bytes.len())
            .ok_or(Error::Arithmetic)?;
        limit("encoded bytes", self.bytes, self.maximum)?;
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
    fn count(&mut self, n: usize) -> Result<(), Error> {
        self.u32(u32::try_from(n).map_err(|_| Error::Arithmetic)?)
    }
    fn text(&mut self, text: &str, counts: &mut Counts) -> Result<(), Error> {
        counts.text(text.len())?;
        self.count(text.len())?;
        self.raw(text.as_bytes())
    }
    fn function(
        &mut self,
        id: crate::runtime_id::RuntimeFunctionSiteId,
        limits: RuntimeInvocationSourceLimits,
    ) -> Result<(), Error> {
        reference("function", id.get().get(), limits.function_count)?;
        self.u32(id.get().get())
    }
    fn flow(&mut self, flow: &FlowRuntimeId, counts: &mut Counts) -> Result<(), Error> {
        let path = flow.path();
        let segments = path.segments();
        if segments.is_empty() {
            return Err(crate::runtime_id::RuntimeIdError::Empty {
                family: RuntimeIdFamily::Flow,
            }
            .into());
        }
        counts.segments(segments.len())?;
        self.count(segments.len())?;
        for (index, segment) in segments.iter().enumerate() {
            RuntimeIdPath::preflight_stored_source_segment(
                RuntimeIdFamily::Flow,
                index,
                segments.len(),
                segment.as_str(),
            )?;
            self.text(segment.as_str(), counts)?;
        }
        self.text(flow.public_label_ref().as_str(), counts)
    }
    fn role(&mut self, role: &RuntimeCallableRole, counts: &mut Counts) -> Result<(), Error> {
        self.role_fields(&role.callable, &role.contract, counts)
    }
    fn role_fields(
        &mut self,
        callable: &crate::entry::RuntimeCallableId,
        contract: &crate::entry::CallableContractHash,
        counts: &mut Counts,
    ) -> Result<(), Error> {
        crate::entry::RuntimeCallableId::validate_source(callable.as_str())?;
        self.text(callable.as_str(), counts)?;
        self.raw(contract.as_bytes())
    }
}
fn emit(
    plan: &RuntimePlan,
    limits: RuntimeInvocationSourceLimits,
    writer: &mut Writer<'_>,
    counts: &mut Counts,
) -> Result<(), Error> {
    writer.raw(DOMAIN)?;
    writer.byte(1)?;
    counts.roots(plan.flows().len())?;
    writer.count(plan.flows().len())?;
    for flow in plan.flows() {
        writer.flow(&flow.id, counts)?;
        writer.function(flow.function_site(), limits)?;
        counts.parameters(flow.params.len())?;
        writer.count(flow.params.len())?;
    }
    counts.schemas(plan.flow_schemas().len())?;
    writer.count(plan.flow_schemas().len())?;
    for schema in plan.flow_schemas() {
        writer.flow(&schema.flow, counts)?;
        counts.schema_parameters(schema.parameters.len())?;
        writer.count(schema.parameters.len())?;
        for parameter in &schema.parameters {
            writer.raw(parameter.identity.as_bytes())?;
            writer.u32(parameter.coordinate.position())?;
            writer.text(&parameter.name, counts)?;
            writer.byte(parameter.mode.semantic_tag())?;
            writer.byte(parameter.passing.semantic_tag())?;
            writer.raw(parameter.semantic_identity.as_bytes())?;
        }
    }
    counts.flow_executables(plan.flow_executables().len())?;
    writer.count(plan.flow_executables().len())?;
    for flow in plan.flow_executables() {
        writer.flow(&flow.flow, counts)?;
        writer.raw(flow.contract.as_bytes())?;
        writer.byte(u8::from(flow.controller.is_some()))?;
        if let Some(controller) = &flow.controller {
            writer.role(controller, counts)?;
        }
    }
    counts.callables(plan.callable_executables().len())?;
    writer.count(plan.callable_executables().len())?;
    for callable in plan.callable_executables() {
        writer.role_fields(&callable.callable, &callable.contract, counts)?;
        match &callable.code {
            RuntimeCallableExecutableCode::FunctionSite(site) => {
                writer.byte(1)?;
                writer.function(*site, limits)?;
            }
            RuntimeCallableExecutableCode::ControllerFlow(flow) => {
                writer.byte(2)?;
                writer.flow(flow, counts)?;
            }
        }
    }
    counts.defers(plan.defer_sites().len())?;
    writer.count(plan.defer_sites().len())?;
    for function in plan.defer_sites() {
        writer.function(*function, limits)?;
    }
    Ok(())
}
