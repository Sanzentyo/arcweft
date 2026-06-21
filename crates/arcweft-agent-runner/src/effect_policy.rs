use std::collections::{BTreeMap, BTreeSet};

use arcweft_agent_protocol::{
    artifact::EffectCapability,
    protocol::{AgentAction, AgentHostRequest},
};
use thiserror::Error;

use crate::policy::{RuntimeAgentCapability, RuntimeAgentPolicy};

/// Strict exact-label mapping used before a controller starts.
#[derive(Clone, Debug, Default)]
pub struct AgentEffectRegistry {
    runtime: BTreeMap<String, RuntimeAgentCapability>,
    internal: BTreeSet<String>,
}

/// Exact verified labels plus the coarse least-privilege runner policy.
#[derive(Clone, Debug)]
pub struct AgentEffectAuthorization {
    verified: BTreeSet<String>,
    policy: RuntimeAgentPolicy,
}

/// Artifact/launch/request policy mismatch.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum AgentEffectPolicyError {
    #[error("verified Agent effect `{effect}` has no runtime or internal mapping")]
    UnmappedEffect { effect: String },
    #[error("launch policy does not grant required Agent capability `{capability}`")]
    MissingGrant { capability: &'static str },
    #[error("Agent host request requires effect `{effect}`, absent from the verified artifact")]
    UndeclaredRequestEffect { effect: &'static str },
}

impl AgentEffectRegistry {
    pub fn canonical() -> Self {
        let mut registry = Self::default();
        registry.register("agent.observe", RuntimeAgentCapability::Observe);
        registry.register("agent.wait", RuntimeAgentCapability::Observe);
        registry.register("agent.act.semantic", RuntimeAgentCapability::Act);
        registry.register("agent.act.physical", RuntimeAgentCapability::ActPhysical);
        registry.register("agent.capture", RuntimeAgentCapability::Capture);
        registry.register("agent.resource.read", RuntimeAgentCapability::ResourceRead);
        registry.register("debug.read", RuntimeAgentCapability::DebugRead);
        registry.register("debug.record", RuntimeAgentCapability::DebugRecord);
        registry.register("agent.rag.query", RuntimeAgentCapability::Rag);

        // Compiler-internal effects that do not grant a host boundary authority.
        registry.register_internal("control.suspend");
        registry.register_internal("control.spawn");
        registry.register_internal("control.detach");
        registry
    }

    pub fn register(&mut self, effect: impl Into<String>, capability: RuntimeAgentCapability) {
        let effect = effect.into();
        self.internal.remove(&effect);
        self.runtime.insert(effect, capability);
    }

    pub fn register_internal(&mut self, effect: impl Into<String>) {
        let effect = effect.into();
        self.runtime.remove(&effect);
        self.internal.insert(effect);
    }

    pub fn authorization_for_artifact(
        &self,
        verified_effects: &[EffectCapability],
        launch_grants: &RuntimeAgentPolicy,
    ) -> Result<AgentEffectAuthorization, AgentEffectPolicyError> {
        let verified = verified_effects
            .iter()
            .map(|effect| effect.as_str().to_owned())
            .collect::<BTreeSet<_>>();
        let required = verified
            .iter()
            .try_fold(BTreeSet::new(), |mut required, effect| {
                if let Some(capability) = self.runtime.get(effect) {
                    required.insert(*capability);
                    return Ok(required);
                }
                if self.internal.contains(effect) {
                    return Ok(required);
                }
                Err(AgentEffectPolicyError::UnmappedEffect {
                    effect: effect.clone(),
                })
            })?;

        if let Some(capability) = required
            .iter()
            .copied()
            .find(|capability| !launch_grants.allows(*capability))
        {
            return Err(AgentEffectPolicyError::MissingGrant {
                capability: capability.as_str(),
            });
        }

        Ok(AgentEffectAuthorization {
            verified,
            // Do not pass every launch grant through. The VM receives exactly
            // the authority proven necessary by this artifact.
            policy: RuntimeAgentPolicy::new(required),
        })
    }
}

impl AgentEffectAuthorization {
    pub const fn policy(&self) -> &RuntimeAgentPolicy {
        &self.policy
    }

    pub fn allows_effect(&self, effect: &str) -> bool {
        self.verified.contains(effect)
    }

    pub fn ensure_effect(&self, effect: &'static str) -> Result<(), AgentEffectPolicyError> {
        if self.allows_effect(effect) {
            Ok(())
        } else {
            Err(AgentEffectPolicyError::UndeclaredRequestEffect { effect })
        }
    }

    pub fn ensure_request(&self, request: &AgentHostRequest) -> Result<(), AgentEffectPolicyError> {
        required_effect_for_request(request).map_or(Ok(()), |effect| self.ensure_effect(effect))
    }
}

/// Exact DSL effect exercised by one host-boundary request.
///
/// Assertions are controller-local validation and therefore return `None`.
pub fn required_effect_for_request(request: &AgentHostRequest) -> Option<&'static str> {
    Some(match request {
        AgentHostRequest::Observe(_) => "agent.observe",
        AgentHostRequest::Wait(_) => "agent.wait",
        AgentHostRequest::Act(action) => match action.as_ref() {
            AgentAction::PointerClick { .. } => "agent.act.physical",
            AgentAction::AdvanceText
            | AgentAction::SelectChoice { .. }
            | AgentAction::Invoke(_) => "agent.act.semantic",
        },
        AgentHostRequest::Capture(_) => "agent.capture",
        AgentHostRequest::ReadResource { .. } => "agent.resource.read",
        AgentHostRequest::EntityMetadata { .. }
        | AgentHostRequest::ProjectGraphNeighborhood { .. } => "debug.read",
        AgentHostRequest::RagQuery(_) => "agent.rag.query",
        AgentHostRequest::Attach(_) | AgentHostRequest::Checkpoint { .. } => "debug.record",
        AgentHostRequest::Assert(_) => return None,
    })
}
