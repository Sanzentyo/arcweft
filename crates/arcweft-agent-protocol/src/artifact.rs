use crate::ids::{PublicId, StableHash};
use serde::{Deserialize, Serialize};

/// Canonical capability name declared by a compiled Agent controller.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct EffectCapability(String);

/// How strictly a controller artifact is bound to a target program.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectBindingMode {
    Strict,
    Compatible,
}

/// One entity dependency that must remain compatible at runtime.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RequiredEntity {
    pub public_id: PublicId,
    pub kind: String,
    pub semantic_hash: StableHash,
    pub source_anchor: Option<RequiredEntitySourceAnchor>,
}

/// Source location recorded for an entity dependency in an Agent artifact.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RequiredEntitySourceAnchor {
    pub path: String,
    pub start_byte: u64,
    pub end_byte: u64,
    pub start: Option<RequiredEntitySourcePosition>,
    pub end: Option<RequiredEntitySourcePosition>,
}

/// One-based source position, when the compiler has line/column data.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RequiredEntitySourcePosition {
    pub line: u32,
    pub column: u32,
}

/// Compile-time target program binding stored in an Agent artifact.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ProjectBinding {
    pub program_hash: StableHash,
    pub mode: ProjectBindingMode,
    pub required_entities: Vec<RequiredEntity>,
}

/// Hard execution limits applied by the controller runner.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgentBudget {
    pub logical_timeout_millis: u64,
    pub max_vm_steps: u64,
    pub max_host_calls: u32,
    pub max_observations: u32,
    pub max_captures: u32,
    pub max_capture_bytes: u64,
    pub max_rag_queries: u32,
    pub max_context_bytes: u64,
}

/// Data-only manifest wrapping normal Arcweft bytecode in an Agent bundle.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgentArtifactManifest {
    pub schema_version: u32,
    pub bundle_kind: AgentBundleKind,
    pub agent_id: PublicId,
    pub source_hash: StableHash,
    pub compiler_version: String,
    pub project_binding: ProjectBinding,
    pub declared_effects: Vec<EffectCapability>,
    pub budget: AgentBudget,
    pub debug_map_hash: Option<StableHash>,
}

/// Bundle discriminator for a controller VM program.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentBundleKind {
    AgentController,
}

impl EffectCapability {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for AgentBudget {
    fn default() -> Self {
        Self {
            logical_timeout_millis: 30_000,
            max_vm_steps: 100_000,
            max_host_calls: 256,
            max_observations: 256,
            max_captures: 16,
            max_capture_bytes: 64 * 1024 * 1024,
            max_rag_queries: 8,
            max_context_bytes: 1024 * 1024,
        }
    }
}
