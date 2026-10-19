//! One borrowed counted writer for sizing and reserved-capacity emission.

use super::{Counts, DOMAIN, Error, Footprint, RuntimeEntrySourceLimits, charge, limit};
use crate::entry::{
    AgentBudget, RootExecutionLimits, RuntimeAgentEntryRoles, RuntimeCallableId,
    RuntimeCallableRole, RuntimeCommandConstructorId, RuntimeCommandContract,
    RuntimeCommandTargetId, RuntimeEntryRoles, RuntimeNominalRole, RuntimeNominalTypeId,
    RuntimeStatefulEntryRoles,
};
use crate::plan::{
    FlowRuntimeId, RuntimeEntryKind, RuntimeEntrySpec, RuntimeEntryTarget, RuntimePlan,
    RuntimeRouteBinding, RuntimeRouteBindingSource, RuntimeRoutePath, RuntimeRoutePathSegment,
    RuntimeRouteSpec,
};
use crate::runtime_id::{RuntimeIdFamily, RuntimeIdPath, RuntimeIdSegment};
use crate::task::semantic::TaskSemanticMeter;

pub(in crate::plan) struct PreparedRuntimeEntrySourceEncoding<'a> {
    entries: &'a [RuntimeEntrySpec],
    limits: RuntimeEntrySourceLimits,
    bytes: usize,
    footprint: Footprint,
}
impl PreparedRuntimeEntrySourceEncoding<'_> {
    pub(in crate::plan) const fn encoded_bytes(&self) -> usize {
        self.bytes
    }
    pub(in crate::plan) const fn elements(&self) -> usize {
        self.footprint.elements
    }
    pub(in crate::plan) const fn storage_bytes(&self) -> usize {
        self.footprint.storage
    }
    pub(in crate::plan) const fn node_count(&self) -> usize {
        self.footprint.schemas
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
            limits: self.limits,
            counts: Counts::new(self.limits),
        };
        writer.emit(self.entries)?;
        debug_assert_eq!(writer.bytes, self.bytes);
        debug_assert_eq!(writer.counts.footprint, self.footprint);
        Ok(())
    }
}
impl RuntimePlan {
    pub(in crate::plan) fn prepare_entry_sources_encoding(
        &self,
        limits: RuntimeEntrySourceLimits,
        meter: &mut TaskSemanticMeter,
    ) -> Result<PreparedRuntimeEntrySourceEncoding<'_>, Error> {
        meter.status()?;
        let result = (|| {
            let mut writer = Writer {
                output: None,
                meter: Some(&mut *meter),
                bytes: 0,
                limits,
                counts: Counts::new(limits),
            };
            writer.emit(self.entries())?;
            Ok(PreparedRuntimeEntrySourceEncoding {
                entries: self.entries(),
                limits,
                bytes: writer.bytes,
                footprint: writer.counts.footprint,
            })
        })();
        result.map_err(|error: Error| error.reject(meter))
    }
}
struct Writer<'a> {
    output: Option<&'a mut Vec<u8>>,
    meter: Option<&'a mut TaskSemanticMeter>,
    bytes: usize,
    limits: RuntimeEntrySourceLimits,
    counts: Counts,
}
impl Writer<'_> {
    fn extent(&mut self, bytes: usize) -> Result<(), Error> {
        self.bytes = self.bytes.checked_add(bytes).ok_or(Error::Arithmetic)?;
        limit("encoded bytes", self.bytes, self.limits.encoded_bytes)
    }
    fn raw(&mut self, bytes: &[u8]) -> Result<(), Error> {
        self.extent(bytes.len())?;
        if let Some(meter) = self.meter.as_mut() {
            charge(meter, bytes.len())?;
        }
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
    fn u64(&mut self, value: u64) -> Result<(), Error> {
        self.raw(&value.to_le_bytes())
    }
    fn count(&mut self, value: usize) -> Result<(), Error> {
        self.u32(u32::try_from(value).map_err(|_| Error::Arithmetic)?)
    }
    fn text(&mut self, value: &str) -> Result<(), Error> {
        self.counts.text(value)?;
        self.count(value.len())?;
        self.raw(value.as_bytes())
    }
    fn path(
        &mut self,
        family: RuntimeIdFamily,
        path: &RuntimeIdPath,
        label: Option<&str>,
    ) -> Result<(), Error> {
        let segments = path.segments();
        if segments.is_empty() {
            return Err(crate::runtime_id::RuntimeIdError::Empty { family }.into());
        }
        self.counts.array::<RuntimeIdSegment>(segments.len())?;
        self.count(segments.len())?;
        for (index, segment) in segments.iter().enumerate() {
            RuntimeIdPath::preflight_stored_source_segment(
                family,
                index,
                segments.len(),
                segment.as_str(),
            )?;
            self.text(segment.as_str())?;
        }
        if let Some(label) = label {
            self.text(label)?;
        }
        Ok(())
    }
    fn flow(&mut self, flow: &FlowRuntimeId) -> Result<(), Error> {
        self.path(
            RuntimeIdFamily::Flow,
            flow.path(),
            Some(flow.public_label_ref().as_str()),
        )
    }
    fn nominal(&mut self, nominal: &RuntimeNominalRole) -> Result<(), Error> {
        RuntimeNominalTypeId::validate_source(nominal.identity.as_str())?;
        self.text(nominal.identity.as_str())?;
        self.raw(nominal.semantic_identity.as_bytes())?;
        self.raw(nominal.layout.as_bytes())
    }
    fn callable(&mut self, callable: &RuntimeCallableRole) -> Result<(), Error> {
        RuntimeCallableId::validate_source(callable.callable.as_str())?;
        self.text(callable.callable.as_str())?;
        self.raw(callable.contract.as_bytes())
    }
    fn emit(&mut self, entries: &[RuntimeEntrySpec]) -> Result<(), Error> {
        self.raw(DOMAIN)?;
        self.byte(1)?;
        self.counts.entries(entries.len())?;
        self.count(entries.len())?;
        for entry in entries {
            self.entry(entry)?;
        }
        Ok(())
    }
    fn entry(&mut self, entry: &RuntimeEntrySpec) -> Result<(), Error> {
        self.path(RuntimeIdFamily::Entry, entry.id.path(), None)?;
        match &entry.kind {
            RuntimeEntryKind::Game => self.byte(0)?,
            RuntimeEntryKind::Editor => self.byte(1)?,
            RuntimeEntryKind::Cli => self.byte(2)?,
            RuntimeEntryKind::Server => self.byte(3)?,
            RuntimeEntryKind::Activity => self.byte(4)?,
            RuntimeEntryKind::Test => self.byte(5)?,
            RuntimeEntryKind::Bench => self.byte(6)?,
            RuntimeEntryKind::Agent => self.byte(7)?,
            RuntimeEntryKind::Custom(name) => {
                self.byte(8)?;
                self.text(name)?;
            }
        }
        self.raw(entry.binding.as_bytes())?;
        match &entry.target {
            RuntimeEntryTarget::Flow(flow) => {
                self.byte(0)?;
                self.flow(flow)?;
            }
            RuntimeEntryTarget::Routes(routes) => {
                self.byte(1)?;
                self.counts.routes(routes.len())?;
                self.count(routes.len())?;
                for route in routes {
                    self.route(route)?;
                }
            }
            RuntimeEntryTarget::Controller(flow) => {
                self.byte(2)?;
                self.flow(flow)?;
            }
        }
        match &entry.roles {
            RuntimeEntryRoles::None => self.byte(0)?,
            RuntimeEntryRoles::Stateful(role) => {
                self.byte(1)?;
                self.counts
                    .storage(size_of::<RuntimeStatefulEntryRoles>())?;
                self.raw(role.binding.as_bytes())?;
                self.nominal(&role.state)?;
                self.nominal(&role.event)?;
                self.callable(&role.initializer)?;
                self.callable(&role.reducer)?;
                self.flow(&role.initial_flow.flow)?;
                self.raw(role.initial_flow.contract.as_bytes())?;
                self.counts.commands(role.command_policy.admitted.len())?;
                self.count(role.command_policy.admitted.len())?;
                for command in &role.command_policy.admitted {
                    self.command(command)?;
                }
                self.root_limits(role.command_policy.root_limits)?;
            }
            RuntimeEntryRoles::Agent(role) => {
                self.byte(2)?;
                self.counts.storage(size_of::<RuntimeAgentEntryRoles>())?;
                self.raw(role.binding.as_bytes())?;
                self.callable(&role.controller)?;
                self.raw(role.policy.as_bytes())?;
                self.agent_budget(role.budget)?;
            }
        }
        Ok(())
    }
    fn route(&mut self, route: &RuntimeRouteSpec) -> Result<(), Error> {
        self.byte(route.method.semantic_tag())?;
        let segments = route.path.segments();
        self.counts
            .array::<RuntimeRoutePathSegment>(segments.len())?;
        self.count(segments.len())?;
        let mut captures = 0usize;
        for segment in segments {
            match segment {
                RuntimeRoutePathSegment::Literal(literal) => {
                    RuntimeRoutePath::validate_source_literal(literal)?;
                    self.byte(0)?;
                    self.text(literal)?;
                }
                RuntimeRoutePathSegment::Capture(capture) => {
                    RuntimeRoutePath::validate_source_capture(*capture, captures)?;
                    captures = captures.checked_add(1).ok_or(Error::Arithmetic)?;
                    self.byte(1)?;
                    self.u32(capture.position())?;
                }
            }
        }
        self.flow(&route.target)?;
        self.counts
            .array::<RuntimeRouteBinding>(route.bindings.len())?;
        self.count(route.bindings.len())?;
        for binding in &route.bindings {
            self.u32(binding.parameter.position())?;
            match binding.source {
                RuntimeRouteBindingSource::PathCapture(capture) => {
                    self.byte(0)?;
                    self.u32(capture.position())?;
                }
            }
        }
        Ok(())
    }
    fn command(&mut self, command: &RuntimeCommandContract) -> Result<(), Error> {
        RuntimeCommandConstructorId::validate_source(command.constructor.as_str())?;
        RuntimeCommandTargetId::validate_source(command.target.as_str())?;
        self.text(command.constructor.as_str())?;
        self.text(command.target.as_str())?;
        self.raw(command.payload_layout.as_bytes())?;
        let limits = self.counts.schema_limits()?;
        let prepared = if let Some(meter) = self.meter.as_mut() {
            command
                .payload_schema
                .prepare_source_encoding(limits, meter)?
        } else {
            command.payload_schema.prepare_source_in_frame(limits)?
        };
        self.counts.schema(
            prepared.nodes(),
            prepared.elements(),
            prepared.storage_bytes(),
        )?;
        self.count(prepared.encoded_bytes())?;
        self.extent(prepared.encoded_bytes())?;
        if let Some(output) = self.output.as_mut() {
            prepared.append_to(output)?;
        }
        Ok(())
    }
    fn root_limits(&mut self, limits: RootExecutionLimits) -> Result<(), Error> {
        self.u32(limits.schema.max_depth)?;
        self.u32(limits.schema.max_nodes)?;
        self.u32(limits.schema.max_sequence_items)?;
        self.u64(limits.schema.max_string_bytes)?;
        self.u64(limits.schema.max_encoded_bytes)?;
        self.u64(limits.schema.max_validation_work)?;
        self.u32(limits.max_commands_per_transition)?;
        self.u64(limits.max_command_bytes_per_transition)?;
        self.u32(limits.max_pending_events)?;
        self.u32(limits.max_pending_commands)
    }
    fn agent_budget(&mut self, budget: AgentBudget) -> Result<(), Error> {
        self.u64(budget.logical_timeout_millis)?;
        self.u64(budget.max_vm_steps)?;
        self.u32(budget.max_host_calls)?;
        self.u32(budget.max_observations)?;
        self.u32(budget.max_captures)?;
        self.u64(budget.max_capture_bytes)?;
        self.u32(budget.max_rag_queries)?;
        self.u64(budget.max_context_bytes)
    }
}
