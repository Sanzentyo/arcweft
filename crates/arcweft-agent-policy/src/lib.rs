//! Mandatory publication gate between raw Agent resources and MCP projections.
//!
//! Raw `AgentResource` values are accepted only at the trusted host boundary.
//! MCP adapters consume `PublishedAgentResource`, making accidental raw
//! publication a type error after the migration patch is applied.

mod decode;
mod gate;
mod publication;
mod published;
mod scene;

pub use gate::{AgentContentPolicyGate, AgentPolicyError};
pub use publication::AgentPublicationPolicy;
pub use published::{AgentPolicySummary, PublishedAgentResource};
pub use scene::{PublishedAgentScene, PublishedAgentSceneView};

#[cfg(test)]
mod tests;
