use crate::{
    ids::{CallableId, PublicId, StableHash},
    verified_effects::VerifiedEffectSummary,
};
use arcweft_core::entry::AgentBudget;
use serde::{Deserialize, Deserializer, Serialize, de};

/// Canonical capability name required by a compiled Agent controller.
///
/// In the schema-v1 `AgentArtifactManifest::declared_effects` field this value
/// is the compiler-lowered closed first-order effect row. The field name is
/// retained for the serialized artifact contract, but the value is not the
/// source `effects` upper bound.
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

/// Data-only manifest wrapping normal Arcweft bytecode in an Agent bundle.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AgentArtifactManifest {
    #[serde(deserialize_with = "deserialize_schema_v1")]
    pub schema_version: u32,
    pub bundle_kind: AgentBundleKind,
    /// Exact source entry selected for this artifact.
    pub entry_id: PublicId,
    /// Ordinary callable declaration selected by `entry agent`.
    pub controller_id: CallableId,
    /// Checked entry binding identity, including controller policy and budget.
    pub entry_binding_hash: StableHash,
    /// Checked ordinary callable contract identity.
    pub controller_contract_hash: StableHash,
    /// Closed Agent policy and effective budget identity.
    pub policy_hash: StableHash,
    pub source_hash: StableHash,
    pub compiler_version: String,
    pub project_binding: ProjectBinding,
    /// Closed first-order effect row inferred by the compiler.
    ///
    /// The serialized field name is historical. Unused members of a source
    /// `effects { ... }` upper bound must not be emitted here.
    pub declared_effects: Vec<EffectCapability>,
    pub verified_effects: VerifiedEffectSummary,
    pub budget: AgentBudget,
    pub debug_map_hash: Option<StableHash>,
}

fn deserialize_schema_v1<'de, D>(deserializer: D) -> Result<u32, D::Error>
where
    D: Deserializer<'de>,
{
    let version = u32::deserialize(deserializer)?;
    if version == 1 {
        Ok(version)
    } else {
        Err(de::Error::custom(format!(
            "unsupported Agent artifact manifest schema version {version}; expected 1"
        )))
    }
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

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;

    fn final_manifest_json() -> Value {
        json!({
            "schema_version": 1,
            "bundle_kind": "agent_controller",
            "entry_id": "entry.agent.smoke",
            "controller_id": "game::crate.smoke",
            "entry_binding_hash": "blake3:entry",
            "controller_contract_hash": "blake3:contract",
            "policy_hash": "blake3:policy",
            "source_hash": "blake3:source",
            "compiler_version": "arcweft-compiler/test",
            "project_binding": {
                "program_hash": "blake3:program",
                "mode": "strict",
                "required_entities": []
            },
            "declared_effects": [],
            "verified_effects": {
                "analysis_version": 1,
                "declared": [],
                "inferred": [],
                "digest": "blake3:effects"
            },
            "budget": {
                "logical_timeout_millis": 30000,
                "max_vm_steps": 100_000,
                "max_host_calls": 256,
                "max_observations": 256,
                "max_captures": 16,
                "max_capture_bytes": 67_108_864,
                "max_rag_queries": 8,
                "max_context_bytes": 1_048_576
            },
            "debug_map_hash": null
        })
    }

    #[test]
    fn final_entry_bound_schema_v1_round_trips() {
        let value = final_manifest_json();
        let manifest: AgentArtifactManifest =
            serde_json::from_value(value.clone()).expect("final schema-v1 manifest decodes");
        assert_eq!(manifest.entry_id.as_str(), "entry.agent.smoke");
        assert_eq!(manifest.controller_id.as_str(), "game::crate.smoke");
        assert_eq!(
            serde_json::to_value(manifest).expect("final schema-v1 manifest encodes"),
            value
        );
    }

    #[test]
    fn predecessor_agent_item_schema_v1_is_rejected() {
        let mut value = final_manifest_json();
        let object = value.as_object_mut().expect("manifest is an object");
        object.insert("agent_id".to_owned(), json!("agent.smoke"));
        object.remove("entry_id");
        object.remove("controller_id");
        object.remove("entry_binding_hash");
        object.remove("controller_contract_hash");
        object.remove("policy_hash");

        assert!(serde_json::from_value::<AgentArtifactManifest>(value).is_err());
    }

    #[test]
    fn mixed_predecessor_and_final_schema_v1_is_rejected() {
        let mut value = final_manifest_json();
        value
            .as_object_mut()
            .expect("manifest is an object")
            .insert("agent_id".to_owned(), json!("agent.smoke"));

        assert!(serde_json::from_value::<AgentArtifactManifest>(value).is_err());
    }

    #[test]
    fn unsupported_manifest_version_is_rejected() {
        let mut value = final_manifest_json();
        value["schema_version"] = json!(2);

        assert!(serde_json::from_value::<AgentArtifactManifest>(value).is_err());
    }
}
