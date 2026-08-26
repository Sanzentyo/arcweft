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
};
use arcweft_core::pattern::RuntimeCheckedType;
use arcweft_core::step::RuntimeHostCallMode;
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

/// Exact existing host-call endpoint selected by the embedding host.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RootCommandHostCallEndpoint {
    public_id: String,
    capability: String,
    operation: String,
    mode: RuntimeHostCallMode,
    deterministic: bool,
}

impl RootCommandHostCallEndpoint {
    pub fn try_new(
        public_id: impl Into<String>,
        capability: impl Into<String>,
        operation: impl Into<String>,
        mode: RuntimeHostCallMode,
        deterministic: bool,
    ) -> Result<Self, RootCommandHostCallCatalogError> {
        Ok(Self {
            public_id: validate_endpoint_field("public_id", public_id.into())?,
            capability: validate_endpoint_field("capability", capability.into())?,
            operation: validate_endpoint_field("operation", operation.into())?,
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
    result: RuntimeCheckedType,
    result_route: RootCommandHostResultRoute,
}

impl RootCommandHostCallBinding {
    #[must_use]
    pub fn new(
        constructor: RuntimeCommandConstructorId,
        target: RuntimeCommandTargetId,
        endpoint: RootCommandHostCallEndpoint,
        arguments: impl IntoIterator<Item = RootCommandHostArgument>,
        result: RuntimeCheckedType,
        result_route: RootCommandHostResultRoute,
    ) -> Self {
        Self {
            constructor,
            target,
            endpoint,
            arguments: arguments.into_iter().collect(),
            result,
            result_route,
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

    fn request(
        &self,
        envelope: &RuntimeCommandEnvelope,
    ) -> (RuntimeHostCallRequest, RootCommandHostResultRoute) {
        let key = RootCommandHostCallKey {
            constructor: envelope.command.constructor().clone(),
            target: envelope.command.target().clone(),
        };
        let binding = self
            .bindings
            .get(&key)
            .expect("session construction proved the selected command policy complete");
        let args = binding
            .arguments
            .iter()
            .map(|argument| match argument {
                RootCommandHostArgument::Constructor => RuntimePayload(RuntimeValue::EntityRef(
                    envelope.command.constructor().as_str().to_owned(),
                )),
                RootCommandHostArgument::Target => RuntimePayload(RuntimeValue::EntityRef(
                    envelope.command.target().as_str().to_owned(),
                )),
                RootCommandHostArgument::Payload => envelope.command.payload().clone(),
            })
            .collect();
        (
            RuntimeHostCallRequest {
                id: root_command_request_id(envelope),
                public_id: binding.endpoint.public_id.clone(),
                capability: binding.endpoint.capability.clone(),
                operation: binding.endpoint.operation.clone(),
                contract: None,
                args,
                named_args: Vec::new(),
                result: binding.result.clone(),
                mode: binding.endpoint.mode,
                deterministic: binding.endpoint.deterministic,
            },
            binding.result_route,
        )
    }
}

impl BundleSession {
    pub(super) fn publish_and_acknowledge_root_commands(
        &mut self,
        commands: &[RuntimeCommandEnvelope],
        diagnostics: &mut Vec<String>,
    ) -> Vec<RuntimeHostCallRequest> {
        let requests = self.publish_root_commands(commands);
        if let Err(error) = self.executor.acknowledge_root_commands(commands) {
            diagnostics.push(format!(
                "failed to acknowledge published root commands: {error}"
            ));
        }
        requests
    }

    pub(super) fn publish_root_commands(
        &mut self,
        commands: &[RuntimeCommandEnvelope],
    ) -> Vec<RuntimeHostCallRequest> {
        commands
            .iter()
            .map(|command| {
                let (request, result_route) = self.options.root_command_host_calls.request(command);
                let replaced = self
                    .pending_root_command_results
                    .insert(request.id.clone(), result_route);
                debug_assert!(
                    replaced.is_none(),
                    "transition/index root request identities are unique within a session"
                );
                request
            })
            .collect()
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
                let Some(route) = self.pending_root_command_results.remove(&result.id) else {
                    return Some(result);
                };
                match (route, result.outcome) {
                    (RootCommandHostResultRoute::Ignore, Ok(_)) => {}
                    (
                        RootCommandHostResultRoute::Ignore
                        | RootCommandHostResultRoute::RootEventPayload,
                        Err(error),
                    ) => {
                        diagnostics.push(format!(
                            "root command request `{}` failed after root commit: {}",
                            result.id.0, error.message
                        ));
                    }
                    (RootCommandHostResultRoute::RootEventPayload, Ok(payload)) => {
                        root_events.push(RootEventInput::new(payload));
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
