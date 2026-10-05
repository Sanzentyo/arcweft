//! Host-owned resolution of committed root commands.
//!
//! Core command contracts deliberately contain only stable constructor/target
//! identity and a replay-safe payload schema. This module owns the separate
//! host projection needed to publish those commands through the existing
//! runtime host-call boundary.

use super::{
    BundleSession, RootEventInput, RuntimeCommandEnvelope, RuntimeHostCallId,
    RuntimeHostCallRequest, RuntimeHostCallResult, RuntimePayload, RuntimeValue,
};
use arcweft_core::entry::{
    RuntimeCommandConstructorId, RuntimeCommandContract, RuntimeCommandTargetId,
    RuntimeSchemaLimits,
};
use arcweft_core::pattern::RuntimeSemanticTypeId;
use arcweft_core::step::{HostCallContractDigest, RuntimeHostCallMode};
use arcweft_core::task::{BoundTaskOutcome, RuntimeProgramOwner, TaskOutcomeContract};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

/// One explicit source for a host-call argument derived from an opaque command.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RootCommandHostArgument {
    Constructor,
    Target,
    Payload,
}

/// What to do with a result correlated to a published root command.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RootCommandHostResultRoute {
    /// Consume the result at the host boundary without producing a root event.
    Ignore,
    /// Treat a successful result payload as one complete typed root event.
    RootEventPayload,
}

/// Pending validation and routing authority for one published root command.
#[derive(Clone, Debug)]
pub(super) struct PendingRootCommandResult {
    pub(super) route: RootCommandHostResultRoute,
    pub(super) bound: BoundTaskOutcome,
}

/// Exact existing host-call endpoint selected by the embedding host.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RootCommandHostCallEndpoint {
    public_id: String,
    capability: String,
    operation: String,
    contract: HostCallContractDigest,
    mode: RuntimeHostCallMode,
    deterministic: bool,
}

impl RootCommandHostCallEndpoint {
    pub fn try_new(
        public_id: impl Into<String>,
        capability: impl Into<String>,
        operation: impl Into<String>,
        contract: HostCallContractDigest,
        mode: RuntimeHostCallMode,
        deterministic: bool,
    ) -> Result<Self, RootCommandHostCallCatalogError> {
        let public_id = validate_endpoint_field("public_id", public_id.into())?;
        let capability = validate_endpoint_field("capability", capability.into())?;
        let operation = validate_endpoint_field("operation", operation.into())?;
        if public_id != format!("{capability}.{operation}") {
            return Err(RootCommandHostCallCatalogError::EndpointIdentityMismatch {
                public_id,
                capability,
                operation,
            });
        }
        Ok(Self {
            public_id,
            capability,
            operation,
            contract,
            mode,
            deterministic,
        })
    }
}

/// One typed constructor/target projection owned by an embedding host.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RootCommandHostCallBinding {
    constructor: RuntimeCommandConstructorId,
    target: RuntimeCommandTargetId,
    endpoint: RootCommandHostCallEndpoint,
    arguments: Vec<RootCommandHostArgument>,
    result: RuntimeSemanticTypeId,
    result_route: RootCommandHostResultRoute,
    producer: arcweft_core::task::HostCallProducerDefinition,
}

impl RootCommandHostCallBinding {
    #[must_use]
    pub fn new(
        constructor: RuntimeCommandConstructorId,
        target: RuntimeCommandTargetId,
        endpoint: RootCommandHostCallEndpoint,
        arguments: impl IntoIterator<Item = RootCommandHostArgument>,
        result: RuntimeSemanticTypeId,
        result_route: RootCommandHostResultRoute,
    ) -> Self {
        let arguments = arguments.into_iter().collect::<Vec<_>>();
        let producer = Self::producer_definition(
            &constructor,
            &target,
            &endpoint,
            &arguments,
            result,
            result_route,
        );
        Self {
            constructor,
            target,
            endpoint,
            arguments,
            result,
            result_route,
            producer,
        }
    }

    fn producer_definition(
        constructor: &RuntimeCommandConstructorId,
        target: &RuntimeCommandTargetId,
        endpoint: &RootCommandHostCallEndpoint,
        arguments: &[RootCommandHostArgument],
        result: RuntimeSemanticTypeId,
        route: RootCommandHostResultRoute,
    ) -> arcweft_core::task::HostCallProducerDefinition {
        let mut site = blake3::Hasher::new();
        site.update(b"arcweft.root-command.host-site.v1\0");
        for identity in [constructor.as_str(), target.as_str()] {
            site.update(&(identity.len() as u64).to_le_bytes());
            site.update(identity.as_bytes());
        }
        let site = *site.finalize().as_bytes();
        let mut plan = blake3::Hasher::new();
        plan.update(b"arcweft.root-command.host-plan.v1\0");
        plan.update(&site);
        plan.update(endpoint.contract.as_bytes());
        for name in [&endpoint.capability, &endpoint.operation] {
            plan.update(&(name.len() as u64).to_le_bytes());
            plan.update(name.as_bytes());
        }
        plan.update(result.as_bytes());
        plan.update(&[
            match endpoint.mode {
                RuntimeHostCallMode::Immediate => 0,
                RuntimeHostCallMode::Suspend => 1,
            },
            u8::from(endpoint.deterministic),
            match route {
                RootCommandHostResultRoute::Ignore => 0,
                RootCommandHostResultRoute::RootEventPayload => 1,
            },
        ]);
        plan.update(&(arguments.len() as u64).to_le_bytes());
        for argument in arguments {
            plan.update(&[match argument {
                RootCommandHostArgument::Constructor => 0,
                RootCommandHostArgument::Target => 1,
                RootCommandHostArgument::Payload => 2,
            }]);
        }
        arcweft_core::task::HostCallProducerDefinition {
            contract: arcweft_core::task::NeedProducerContractDigest::from_bytes(
                *endpoint.contract.as_bytes(),
            ),
            plan: arcweft_core::task::TaskPlanSemanticDigest::from_bytes(
                *plan.finalize().as_bytes(),
            ),
            site: arcweft_core::task::NeedProducerSiteDigest::from_bytes(site),
        }
    }
    fn key(&self) -> RootCommandHostCallKey {
        RootCommandHostCallKey {
            constructor: self.constructor.clone(),
            target: self.target.clone(),
        }
    }
}

/// Complete, unique host projection for one selected entry command policy.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RootCommandHostCallCatalog {
    bindings: BTreeMap<RootCommandHostCallKey, RootCommandHostCallBinding>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct RootCommandHostCallKey {
    constructor: RuntimeCommandConstructorId,
    target: RuntimeCommandTargetId,
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum RootCommandHostCallCatalogError {
    #[error("root-command host endpoint `{public_id}` does not match `{capability}.{operation}`")]
    EndpointIdentityMismatch {
        public_id: String,
        capability: String,
        operation: String,
    },
    #[error("root-command host endpoint {field} cannot be empty")]
    EmptyEndpointField { field: &'static str },
    #[error("root-command host endpoint {field} contains a control character at byte {byte}")]
    InvalidEndpointField { field: &'static str, byte: usize },
    #[error(
        "duplicate root-command host binding for constructor `{constructor}` and target `{target}`"
    )]
    DuplicateBinding { constructor: String, target: String },
    #[error(
        "root-command host catalog is missing constructor `{constructor}` and target `{target}`"
    )]
    MissingBinding { constructor: String, target: String },
    #[error(
        "root-command host binding for constructor `{constructor}` and target `{target}` could not bind result semantic type {semantic_type:?} to the selected executable: {message}"
    )]
    ResultBinding {
        constructor: String,
        target: String,
        semantic_type: RuntimeSemanticTypeId,
        message: String,
    },
    #[error(
        "root-command host request identity `{request}` is already pending or duplicated in the publication batch"
    )]
    DuplicatePendingRequest { request: String },
    #[error("root-command host task admission failed: {message}")]
    TaskAdmission { message: String },
    #[error(
        "root-command host binding for constructor `{constructor}` and target `{target}` has result semantic type {semantic_type:?} that cannot be resolved in the selected AWBC program: {source}"
    )]
    UnresolvedResultType {
        constructor: String,
        target: String,
        semantic_type: RuntimeSemanticTypeId,
        #[source]
        source: arcweft_core::program_types::RuntimeProgramTypeError,
    },
    #[error(
        "root-command host binding for constructor `{constructor}` and target `{target}` routes result type {result_type:?} as a root event, but the selected entry has no stateful event role"
    )]
    RootEventRouteWithoutEventRole {
        constructor: String,
        target: String,
        result_type: RuntimeSemanticTypeId,
    },
    #[error(
        "root-command host binding for constructor `{constructor}` and target `{target}` routes result type {result_type:?} as a root event, but the selected entry event type is {event_type:?}"
    )]
    RootEventResultTypeMismatch {
        constructor: String,
        target: String,
        result_type: RuntimeSemanticTypeId,
        event_type: RuntimeSemanticTypeId,
    },
}

impl RootCommandHostCallCatalog {
    pub fn try_new(
        bindings: impl IntoIterator<Item = RootCommandHostCallBinding>,
    ) -> Result<Self, RootCommandHostCallCatalogError> {
        let mut catalog = Self::default();
        for binding in bindings {
            let key = binding.key();
            if catalog.bindings.insert(key.clone(), binding).is_some() {
                return Err(RootCommandHostCallCatalogError::DuplicateBinding {
                    constructor: key.constructor.as_str().to_owned(),
                    target: key.target.as_str().to_owned(),
                });
            }
        }
        Ok(catalog)
    }

    pub(crate) fn validate_policy(
        &self,
        contracts: &[RuntimeCommandContract],
    ) -> Result<(), RootCommandHostCallCatalogError> {
        let admitted = contracts
            .iter()
            .map(|contract| RootCommandHostCallKey {
                constructor: contract.constructor.clone(),
                target: contract.target.clone(),
            })
            .collect::<BTreeSet<_>>();
        for key in &admitted {
            if !self.bindings.contains_key(key) {
                return Err(RootCommandHostCallCatalogError::MissingBinding {
                    constructor: key.constructor.as_str().to_owned(),
                    target: key.target.as_str().to_owned(),
                });
            }
        }
        Ok(())
    }

    pub(crate) fn validate_for_program(
        &self,
        program: &arcweft_core::awbc::schema::AwbcProgram,
        contracts: &[RuntimeCommandContract],
        event_type: Option<RuntimeSemanticTypeId>,
    ) -> Result<(), RootCommandHostCallCatalogError> {
        self.validate_policy(contracts)?;

        let program_types = arcweft_core::program_types::RuntimeProgramTypes::Awbc(program);
        for binding in self.bindings.values() {
            program_types
                .require_type(binding.result)
                .map_err(
                    |source| RootCommandHostCallCatalogError::UnresolvedResultType {
                        constructor: binding.constructor.as_str().to_owned(),
                        target: binding.target.as_str().to_owned(),
                        semantic_type: binding.result,
                        source,
                    },
                )?;

            if binding.result_route == RootCommandHostResultRoute::RootEventPayload {
                let Some(event_type) = event_type else {
                    return Err(
                        RootCommandHostCallCatalogError::RootEventRouteWithoutEventRole {
                            constructor: binding.constructor.as_str().to_owned(),
                            target: binding.target.as_str().to_owned(),
                            result_type: binding.result,
                        },
                    );
                };
                if binding.result != event_type {
                    return Err(
                        RootCommandHostCallCatalogError::RootEventResultTypeMismatch {
                            constructor: binding.constructor.as_str().to_owned(),
                            target: binding.target.as_str().to_owned(),
                            result_type: binding.result,
                            event_type,
                        },
                    );
                }
            }
        }
        Ok(())
    }

    fn request(
        &self,
        envelope: &RuntimeCommandEnvelope,
        program: &RuntimeProgramOwner,
    ) -> Result<
        (
            arcweft_core::step::RuntimeHostCallStart,
            PendingRootCommandResult,
        ),
        RootCommandHostCallCatalogError,
    > {
        let key = RootCommandHostCallKey {
            constructor: envelope.command.constructor().clone(),
            target: envelope.command.target().clone(),
        };
        let binding = self.bindings.get(&key).ok_or_else(|| {
            RootCommandHostCallCatalogError::MissingBinding {
                constructor: key.constructor.as_str().to_owned(),
                target: key.target.as_str().to_owned(),
            }
        })?;
        let bound = TaskOutcomeContract::program(binding.result)
            .bind_program(program.clone(), RuntimeSchemaLimits::engine_default())
            .map_err(|error| RootCommandHostCallCatalogError::ResultBinding {
                constructor: binding.constructor.as_str().to_owned(),
                target: binding.target.as_str().to_owned(),
                semantic_type: binding.result,
                message: error.to_string(),
            })?;
        let args = binding
            .arguments
            .iter()
            .map(|argument| match argument {
                RootCommandHostArgument::Constructor => RuntimePayload(RuntimeValue::String(
                    envelope.command.constructor().as_str().to_owned(),
                )),
                RootCommandHostArgument::Target => RuntimePayload(RuntimeValue::String(
                    envelope.command.target().as_str().to_owned(),
                )),
                RootCommandHostArgument::Payload => envelope.command.payload().clone(),
            })
            .collect();
        Ok((
            arcweft_core::step::RuntimeHostCallStart {
                id: root_command_request_id(envelope),
                producer: binding.producer,
                request: arcweft_core::task::HostTaskRequest::Custom {
                    capability: arcweft_core::task::HostCapabilityId(
                        binding.endpoint.capability.clone(),
                    ),
                    operation: binding.endpoint.operation.clone(),
                    manifest_contract: Some(binding.endpoint.contract),
                    args,
                    named_args: Vec::new(),
                },
                result: binding.result,
                mode: binding.endpoint.mode,
                deterministic: binding.endpoint.deterministic,
            },
            PendingRootCommandResult {
                route: binding.result_route,
                bound,
            },
        ))
    }
}

impl BundleSession {
    pub(super) fn publish_and_acknowledge_root_commands(
        &mut self,
        commands: &[RuntimeCommandEnvelope],
        diagnostics: &mut Vec<String>,
    ) -> Vec<RuntimeHostCallRequest> {
        let requests = match self.publish_root_commands(commands) {
            Ok(requests) => requests,
            Err(error) => {
                diagnostics.push(format!("failed to publish root commands: {error}"));
                return Vec::new();
            }
        };
        if let Err(error) = self.executor.acknowledge_root_commands(commands) {
            for request in &requests {
                self.pending_root_command_results.remove(&request.id);
            }
            diagnostics.push(format!(
                "failed to acknowledge published root commands: {error}"
            ));
            return Vec::new();
        }
        requests
    }

    pub(super) fn publish_root_commands(
        &mut self,
        commands: &[RuntimeCommandEnvelope],
    ) -> Result<Vec<RuntimeHostCallRequest>, RootCommandHostCallCatalogError> {
        let program = self.executor.program_owner();
        let mut prepared = Vec::with_capacity(commands.len());
        let mut request_ids = BTreeSet::new();
        for command in commands {
            let (request, pending) = self
                .options
                .root_command_host_calls
                .request(command, &program)?;
            if !request_ids.insert(request.id.clone())
                || self.pending_root_command_results.contains_key(&request.id)
            {
                return Err(RootCommandHostCallCatalogError::DuplicatePendingRequest {
                    request: request.id.0,
                });
            }
            prepared.push((request, pending));
        }

        let mut requests = Vec::with_capacity(prepared.len());
        for (start, pending) in prepared {
            let request = self.executor.admit_host_call(start).map_err(|error| {
                RootCommandHostCallCatalogError::TaskAdmission {
                    message: error.to_string(),
                }
            })?;
            self.pending_root_command_results
                .insert(request.id.clone(), pending);
            requests.push(request);
        }
        Ok(requests)
    }

    pub(super) fn route_host_call_results(
        &mut self,
        results: Vec<RuntimeHostCallResult>,
        root_events: &mut Vec<RootEventInput>,
        diagnostics: &mut Vec<String>,
    ) -> Vec<RuntimeHostCallResult> {
        results
            .into_iter()
            .filter_map(|result| {
                let Some(pending) = self.pending_root_command_results.remove(&result.id) else {
                    return Some(result);
                };
                match result.outcome {
                    Ok(payload) => match pending.bound.try_payload(payload.0) {
                        Ok(payload) => match pending.route {
                            RootCommandHostResultRoute::Ignore => {}
                            RootCommandHostResultRoute::RootEventPayload => {
                                root_events.push(RootEventInput::new(payload));
                            }
                        },
                        Err(error) => diagnostics.push(format!(
                            "root command request `{}` returned a payload outside its declared result type: {error}",
                            result.id.0
                        )),
                    },
                    Err(error) => {
                        diagnostics.push(format!(
                            "root command request `{}` failed after root commit: {}",
                            result.id.0, error.message
                        ));
                    }
                }
                None
            })
            .collect()
    }
}

fn root_command_request_id(envelope: &RuntimeCommandEnvelope) -> RuntimeHostCallId {
    RuntimeHostCallId(format!(
        "arcweft.root-command.{}.{}",
        envelope.transition.get(),
        envelope.index
    ))
}

fn validate_endpoint_field(
    field: &'static str,
    value: String,
) -> Result<String, RootCommandHostCallCatalogError> {
    if value.is_empty() {
        return Err(RootCommandHostCallCatalogError::EmptyEndpointField { field });
    }
    if let Some((byte, _)) = value
        .char_indices()
        .find(|(_, character)| character.is_control())
    {
        return Err(RootCommandHostCallCatalogError::InvalidEndpointField { field, byte });
    }
    Ok(value)
}
