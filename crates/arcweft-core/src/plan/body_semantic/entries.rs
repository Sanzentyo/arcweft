//! Actual Entry role/dispatch metadata. Bodies stay on their code owners.
//! Command schemas use the layout leaf checked by aggregate structural admission.

use super::{RuntimeBodySemanticContext, RuntimeBodySemanticError};
use crate::entry::{
    AgentBudget, RootExecutionLimits, RuntimeCallableRole, RuntimeCommandPolicy, RuntimeEntryRoles,
    RuntimeFlowRole, RuntimeNominalRole, RuntimeSchemaLimits,
};
use crate::plan::{
    RuntimeEntryTarget, RuntimeRouteBindingSource, RuntimeRoutePathSegment, RuntimeRouteSpec,
};
use crate::task::semantic::{TaskSemanticEncoder, TaskSemanticMeter};

impl RuntimeBodySemanticContext<'_> {
    /// This preparation visitor runs after the candidate's structural check.
    /// It does not issue a public task-plan seal or trust caller digest bytes.
    pub(crate) fn entry_row_digest(
        &self,
        meter: &mut TaskSemanticMeter,
        ordinal: usize,
    ) -> Result<blake3::Hash, RuntimeBodySemanticError> {
        meter.status()?;
        let row = self.plan.entries().get(ordinal).ok_or_else(|| {
            meter.reject_owner();
            RuntimeBodySemanticError::MissingRow {
                table: "entries",
                ordinal,
            }
        })?;
        let mut encoder =
            TaskSemanticEncoder::new(b"arcweft.runtime-plan.executable-row.v1\0", meter);
        encoder.tag(6);
        encoder.tag(row.kind.canonical_tag());
        encoder.count(row.id.path().segments().len());
        for segment in row.id.path().segments() {
            encoder.enter_element();
            encoder.string(segment.as_str());
        }
        encoder.tag(u8::from(row.kind.custom_payload().is_some()));
        if let Some(custom) = row.kind.custom_payload() {
            encoder.string(custom);
        }
        encoder.digest(row.binding.as_bytes());
        match &row.target {
            RuntimeEntryTarget::Flow(flow) => {
                encoder.tag(0);
                self.write_entry_flow(&mut encoder, flow)?;
            }
            RuntimeEntryTarget::Routes(routes) => {
                encoder.tag(1);
                encoder.count(routes.len());
                for (ordinal, route) in routes.iter().enumerate() {
                    encoder.enter_element();
                    encoder.count(ordinal);
                    self.write_entry_route(&mut encoder, route)?;
                }
            }
            RuntimeEntryTarget::Controller(flow) => {
                encoder.tag(2);
                self.write_entry_flow(&mut encoder, flow)?;
            }
        }
        match &row.roles {
            RuntimeEntryRoles::None => encoder.tag(0),
            RuntimeEntryRoles::Stateful(roles) => {
                encoder.tag(1);
                encoder.digest(roles.binding.as_bytes());
                roles.state.write_executable_role(&mut encoder);
                roles.initializer.write_executable_role(&mut encoder);
                roles.event.write_executable_role(&mut encoder);
                roles.reducer.write_executable_role(&mut encoder);
                roles.initial_flow.write_executable_role(&mut encoder);
                roles.command_policy.write_executable_policy(&mut encoder);
            }
            RuntimeEntryRoles::Agent(roles) => {
                encoder.tag(2);
                encoder.digest(roles.binding.as_bytes());
                roles.controller.write_executable_role(&mut encoder);
                encoder.digest(roles.policy.as_bytes());
                roles.budget.write_executable_policy(&mut encoder);
            }
        }
        encoder.finish().map_err(Into::into)
    }

    fn write_entry_route(
        &self,
        encoder: &mut TaskSemanticEncoder<'_>,
        route: &RuntimeRouteSpec,
    ) -> Result<(), RuntimeBodySemanticError> {
        encoder.tag(route.method.semantic_tag());
        encoder.count(route.path.segments().len());
        for (ordinal, segment) in route.path.segments().iter().enumerate() {
            encoder.enter_element();
            encoder.count(ordinal);
            match segment {
                RuntimeRoutePathSegment::Literal(value) => {
                    encoder.tag(0);
                    encoder.string(value);
                }
                RuntimeRoutePathSegment::Capture(coordinate) => {
                    encoder.tag(1);
                    encoder.ordinal(coordinate.position());
                }
            }
        }
        self.write_entry_flow(encoder, &route.target)?;
        encoder.count(route.bindings.len());
        for (ordinal, binding) in route.bindings.iter().enumerate() {
            encoder.enter_element();
            encoder.count(ordinal);
            encoder.ordinal(binding.parameter.position());
            match binding.source {
                RuntimeRouteBindingSource::PathCapture(coordinate) => {
                    encoder.tag(0);
                    encoder.ordinal(coordinate.position());
                }
            }
        }
        encoder.status().map_err(Into::into)
    }

    fn write_entry_flow(
        &self,
        encoder: &mut TaskSemanticEncoder<'_>,
        identity: &crate::plan::FlowRuntimeId,
    ) -> Result<(), RuntimeBodySemanticError> {
        encoder.status()?;
        let flow = self.plan.flows.flow(identity).ok_or_else(|| {
            encoder.reject_owner();
            RuntimeBodySemanticError::MissingRow {
                table: "entry Flow targets",
                ordinal: self.plan.flows().len(),
            }
        })?;
        let schema = self.plan.flows.schema(identity).ok_or_else(|| {
            encoder.reject_owner();
            RuntimeBodySemanticError::MissingRow {
                table: "entry Flow schemas",
                ordinal: self.plan.flow_schemas().len(),
            }
        })?;
        identity.write_executable_identity(encoder);
        encoder.digest(flow.definition().as_bytes());
        schema.write_executable_parameters(encoder);
        self.write_function_signature(encoder, flow.function_site())
    }
}

impl RuntimeNominalRole {
    fn write_executable_role(&self, encoder: &mut TaskSemanticEncoder<'_>) {
        // Aggregate admission resolves this exact identity/layout in the
        // original nominal owner. Accepted nominal references are leaves.
        encoder.string(self.identity.as_str());
        encoder.digest(self.semantic_identity.as_bytes());
        encoder.digest(self.layout.as_bytes());
    }
}

impl RuntimeCallableRole {
    pub(super) fn write_executable_role(&self, encoder: &mut TaskSemanticEncoder<'_>) {
        encoder.string(self.callable.as_str());
        encoder.digest(self.contract.as_bytes());
    }
}

impl RuntimeFlowRole {
    fn write_executable_role(&self, encoder: &mut TaskSemanticEncoder<'_>) {
        self.flow.write_executable_identity(encoder);
        encoder.digest(self.contract.as_bytes());
    }
}

impl RuntimeCommandPolicy {
    fn write_executable_policy(&self, encoder: &mut TaskSemanticEncoder<'_>) {
        encoder.count(self.admitted.len());
        for (ordinal, command) in self.admitted.iter().enumerate() {
            encoder.enter_element();
            encoder.count(ordinal);
            encoder.string(command.constructor.as_str());
            encoder.string(command.target.as_str());
            // Structural admission recomputes this exact schema layout and
            // rejects a mismatching payload_schema before semantic encoding.
            encoder.digest(command.payload_layout.as_bytes());
        }
        self.root_limits.write_executable_policy(encoder);
    }
}

impl AgentBudget {
    fn write_executable_policy(self, encoder: &mut TaskSemanticEncoder<'_>) {
        encoder.scalar_u64(self.logical_timeout_millis);
        encoder.scalar_u64(self.max_vm_steps);
        encoder.ordinal(self.max_host_calls);
        encoder.ordinal(self.max_observations);
        encoder.ordinal(self.max_captures);
        encoder.scalar_u64(self.max_capture_bytes);
        encoder.ordinal(self.max_rag_queries);
        encoder.scalar_u64(self.max_context_bytes);
    }
}

impl RootExecutionLimits {
    fn write_executable_policy(self, encoder: &mut TaskSemanticEncoder<'_>) {
        self.schema.write_executable_policy(encoder);
        encoder.ordinal(self.max_commands_per_transition);
        encoder.scalar_u64(self.max_command_bytes_per_transition);
        encoder.ordinal(self.max_pending_events);
        encoder.ordinal(self.max_pending_commands);
    }
}

impl RuntimeSchemaLimits {
    fn write_executable_policy(self, encoder: &mut TaskSemanticEncoder<'_>) {
        encoder.ordinal(self.max_depth);
        encoder.ordinal(self.max_nodes);
        encoder.ordinal(self.max_sequence_items);
        encoder.scalar_u64(self.max_string_bytes);
        encoder.scalar_u64(self.max_encoded_bytes);
        encoder.scalar_u64(self.max_validation_work);
    }
}

impl crate::plan::RuntimeHttpMethod {
    pub(crate) const fn semantic_tag(self) -> u8 {
        match self {
            Self::Get => 0,
            Self::Post => 1,
            Self::Put => 2,
            Self::Patch => 3,
            Self::Delete => 4,
            Self::Head => 5,
            Self::Options => 6,
        }
    }
}

#[cfg(test)]
mod tests;
