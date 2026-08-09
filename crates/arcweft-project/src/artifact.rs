use crate::{
    fingerprint::{
        BuildDigest, NamedDigest, put_digest, put_named_digests, put_string, put_string_vec,
        put_u32,
    },
    incremental::QueryKind,
};
use serde::{Deserialize, Serialize};
use std::fmt::Display;
use thiserror::Error;

/// Compiler artifact family stored behind an incremental query key.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactKind {
    ParsedSyntax,
    InterfaceSummary,
    HirBody,
    TypeCheckReport,
    RuntimePlan,
    BytecodeUnit,
    AssetMetadata,
    AssetPayload,
    LinkPlan,
    BundleSection,
    BundleIndex,
}

/// Stable cache key for one build artifact.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct ArtifactKey(BuildDigest);

/// Opaque proof that an artifact key was derived for the one canonical
/// runtime-plan artifact family.
///
/// This wrapper has no raw-key constructor. Runtime diagnostic inventories
/// may bind only a key whose query, artifact kind, and logical item were
/// validated together before derivation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimePlanArtifactKey(ArtifactKey);

/// Canonical artifact-key input.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ArtifactKeyInput {
    pub compiler_build_id: String,
    pub query: QueryKind,
    pub artifact_kind: ArtifactKind,
    pub target_triple: String,
    pub target_features: Vec<String>,
    pub profile: String,
    pub package: String,
    pub logical_item: String,
    pub source_digest: BuildDigest,
    pub dependency_interface_digests: Vec<NamedDigest>,
    pub dependency_body_digests: Vec<NamedDigest>,
    pub adapter_environment_digest: BuildDigest,
    pub launch_profile_digest: BuildDigest,
    pub declared_environment_digest: BuildDigest,
    pub format_options_digest: BuildDigest,
}

/// Invalid attempt to derive the typed runtime-plan artifact key.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimePlanArtifactKeyError {
    #[error("runtime-plan artifact key requires the RuntimePlan query, got {actual:?}")]
    WrongQuery { actual: QueryKind },
    #[error("runtime-plan artifact key requires the RuntimePlan artifact kind, got {actual}")]
    WrongArtifactKind { actual: ArtifactKind },
    #[error("runtime-plan artifact key requires logical item `runtime-plan`, got `{actual}`")]
    WrongLogicalItem { actual: String },
}

impl ArtifactKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ParsedSyntax => "parsed_syntax",
            Self::InterfaceSummary => "interface_summary",
            Self::HirBody => "hir_body",
            Self::TypeCheckReport => "type_check_report",
            Self::RuntimePlan => "runtime_plan",
            Self::BytecodeUnit => "bytecode_unit",
            Self::AssetMetadata => "asset_metadata",
            Self::AssetPayload => "asset_payload",
            Self::LinkPlan => "link_plan",
            Self::BundleSection => "bundle_section",
            Self::BundleIndex => "bundle_index",
        }
    }
}

impl Display for ArtifactKind {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl ArtifactKey {
    /// Derives a stable key from canonical query inputs.
    pub fn derive(input: &ArtifactKeyInput) -> Self {
        let mut target_features = input.target_features.clone();
        target_features.sort();
        target_features.dedup();
        let dependency_interface_digests =
            NamedDigest::canonicalize(input.dependency_interface_digests.clone());
        let dependency_body_digests =
            NamedDigest::canonicalize(input.dependency_body_digests.clone());

        let mut bytes = Vec::new();
        put_u32(&mut bytes, crate::incremental::CACHE_SCHEMA_VERSION);
        put_string(&mut bytes, &input.compiler_build_id);
        put_string(&mut bytes, input.query.cache_namespace());
        put_string(&mut bytes, input.artifact_kind.as_str());
        put_string(&mut bytes, &input.target_triple);
        put_string_vec(&mut bytes, &target_features);
        put_string(&mut bytes, &input.profile);
        put_string(&mut bytes, &input.package);
        put_string(&mut bytes, &input.logical_item);
        put_digest(&mut bytes, input.source_digest);
        put_named_digests(&mut bytes, &dependency_interface_digests);
        if input.query.dependency_scope().requires_body_digests() {
            put_named_digests(&mut bytes, &dependency_body_digests);
        }
        put_digest(&mut bytes, input.adapter_environment_digest);
        put_digest(&mut bytes, input.launch_profile_digest);
        put_digest(&mut bytes, input.declared_environment_digest);
        put_digest(&mut bytes, input.format_options_digest);
        Self(BuildDigest::of(&bytes))
    }

    pub const fn digest(self) -> BuildDigest {
        self.0
    }
}

impl RuntimePlanArtifactKey {
    /// Validates the complete artifact family before deriving the opaque key.
    pub fn try_derive(input: &ArtifactKeyInput) -> Result<Self, RuntimePlanArtifactKeyError> {
        if input.query != QueryKind::RuntimePlan {
            return Err(RuntimePlanArtifactKeyError::WrongQuery {
                actual: input.query,
            });
        }
        if input.artifact_kind != ArtifactKind::RuntimePlan {
            return Err(RuntimePlanArtifactKeyError::WrongArtifactKind {
                actual: input.artifact_kind,
            });
        }
        if input.logical_item != "runtime-plan" {
            return Err(RuntimePlanArtifactKeyError::WrongLogicalItem {
                actual: input.logical_item.clone(),
            });
        }
        Ok(Self(ArtifactKey::derive(input)))
    }

    /// Returns the generic key only for existing cache-store plumbing.
    pub const fn artifact_key(self) -> ArtifactKey {
        self.0
    }

    /// Returns the exact canonical digest copied into runtime diagnostics.
    pub const fn digest(self) -> BuildDigest {
        self.0.digest()
    }
}

impl Display for ArtifactKey {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.0, formatter)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ArtifactKey, ArtifactKeyInput, ArtifactKind, RuntimePlanArtifactKey,
        RuntimePlanArtifactKeyError,
    };
    use crate::{
        fingerprint::{BuildDigest, NamedDigest},
        incremental::QueryKind,
    };

    fn digest(label: &str) -> BuildDigest {
        BuildDigest::of(label.as_bytes())
    }

    fn input(query: QueryKind) -> ArtifactKeyInput {
        ArtifactKeyInput {
            compiler_build_id: "compiler".to_owned(),
            query,
            artifact_kind: query.artifact_kind(),
            target_triple: "native".to_owned(),
            target_features: vec!["simd".to_owned(), "base".to_owned()],
            profile: "dev".to_owned(),
            package: "pkg".to_owned(),
            logical_item: "crate::main".to_owned(),
            source_digest: digest("source"),
            dependency_interface_digests: vec![
                NamedDigest::new("b", digest("b-interface")),
                NamedDigest::new("a", digest("a-interface")),
            ],
            dependency_body_digests: vec![NamedDigest::new("a", digest("a-body"))],
            adapter_environment_digest: digest("adapter"),
            launch_profile_digest: digest("launch"),
            declared_environment_digest: digest("env"),
            format_options_digest: digest("options"),
        }
    }

    #[test]
    fn artifact_key_is_stable_for_sorted_features_and_dependencies() {
        let first = input(QueryKind::Interface);
        let mut second = input(QueryKind::Interface);
        second.target_features.reverse();
        second.dependency_interface_digests.reverse();

        assert_eq!(ArtifactKey::derive(&first), ArtifactKey::derive(&second));
    }

    #[test]
    fn body_digests_affect_body_dependent_queries_only() {
        let first = input(QueryKind::Interface);
        let mut second = input(QueryKind::Interface);
        second.dependency_body_digests = vec![NamedDigest::new("a", digest("changed"))];
        assert_eq!(ArtifactKey::derive(&first), ArtifactKey::derive(&second));

        let first = input(QueryKind::RuntimePlan);
        let mut second = input(QueryKind::RuntimePlan);
        second.dependency_body_digests = vec![NamedDigest::new("a", digest("changed"))];
        assert_ne!(ArtifactKey::derive(&first), ArtifactKey::derive(&second));
    }

    #[test]
    fn query_owns_its_expected_artifact_kind() {
        assert_eq!(
            QueryKind::BundleIndex.artifact_kind(),
            ArtifactKind::BundleIndex
        );
    }

    #[test]
    fn runtime_plan_artifact_key_rejects_other_artifact_families() {
        let mut wrong_query = input(QueryKind::BytecodeUnit);
        wrong_query.logical_item = "runtime-plan".to_owned();
        assert!(matches!(
            RuntimePlanArtifactKey::try_derive(&wrong_query),
            Err(RuntimePlanArtifactKeyError::WrongQuery {
                actual: QueryKind::BytecodeUnit
            })
        ));

        let mut wrong_kind = input(QueryKind::RuntimePlan);
        wrong_kind.artifact_kind = ArtifactKind::BytecodeUnit;
        wrong_kind.logical_item = "runtime-plan".to_owned();
        assert!(matches!(
            RuntimePlanArtifactKey::try_derive(&wrong_kind),
            Err(RuntimePlanArtifactKeyError::WrongArtifactKind {
                actual: ArtifactKind::BytecodeUnit
            })
        ));

        let mut wrong_item = input(QueryKind::RuntimePlan);
        wrong_item.logical_item = "other".to_owned();
        assert!(matches!(
            RuntimePlanArtifactKey::try_derive(&wrong_item),
            Err(RuntimePlanArtifactKeyError::WrongLogicalItem { .. })
        ));
    }

    #[test]
    fn runtime_plan_artifact_key_copies_the_canonical_generic_key() {
        let mut input = input(QueryKind::RuntimePlan);
        input.logical_item = "runtime-plan".to_owned();
        let typed = RuntimePlanArtifactKey::try_derive(&input).expect("runtime-plan key");
        assert_eq!(typed.artifact_key(), ArtifactKey::derive(&input));
        assert_eq!(typed.digest(), ArtifactKey::derive(&input).digest());
    }
}
