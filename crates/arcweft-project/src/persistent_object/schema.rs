use crate::{
    artifact::ArtifactKind,
    fingerprint::{
        BuildDigest, NamedDigest, put_digest, put_named_digests, put_string, put_string_vec,
        put_u32,
    },
    incremental::{CacheRecordStatus, InvalidationReason, QueryKind},
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const AWBO_MAGIC: [u8; 8] = *b"AWBO\r\n\x1a\n";
pub const AWBO_SCHEMA_VERSION: u32 = crate::incremental::CACHE_SCHEMA_VERSION;

/// Persistable compiler-private object family.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompilerObjectKind {
    ParsedSyntax,
    InterfaceSummary,
    HirBody,
    TypecheckGate,
    LineTaskEvidence,
    RuntimePlanUnit,
    BytecodeUnit,
    LinkPlan,
}

/// Whether an object may cross exact compiler identities.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompilerObjectStability {
    CrossCompiler,
    ExactCompilerIdentity,
}

/// Exact compiler identity recorded in compiler-private cache keys.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct CompilerBuildIdentity {
    pub package_version: String,
    pub git_commit: String,
    pub rustc: String,
    pub target: String,
    pub enabled_features: Vec<String>,
}

/// Canonical key material for a compiler object.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct CompilerObjectKey {
    pub kind: CompilerObjectKind,
    pub compiler: CompilerBuildIdentity,
    pub source_digest: BuildDigest,
    pub query_options_digest: BuildDigest,
    pub dependency_interface_digests: Vec<NamedDigest>,
    pub dependency_body_digests: Vec<NamedDigest>,
    pub environment_digest: BuildDigest,
}

/// Compiler namespace recorded inside compiler-private payloads.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompilerIdentityNamespaceObject {
    pub object_kind: CompilerObjectKind,
    pub cache_namespace: String,
    pub compiler: CompilerBuildIdentity,
}

/// Stable stage inputs copied into exact compiler-private payloads.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompilerStageInputsObject {
    pub query_options_digest: BuildDigest,
    pub dependency_interface_digests: Vec<NamedDigest>,
    pub dependency_body_digests: Vec<NamedDigest>,
    pub environment_digest: BuildDigest,
}

/// `.awbo` validation and binary codec error.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum AwboError {
    #[error("AWBO magic does not match")]
    BadMagic,
    #[error("unsupported AWBO schema version {actual}; expected {expected}")]
    UnsupportedSchema { actual: u32, expected: u32 },
    #[error("AWBO wire tag {tag} is not valid for {domain}")]
    UnsupportedWireTag { domain: &'static str, tag: u8 },
    #[error("AWBO payload kind {payload:?} does not match key kind {key:?}")]
    KindMismatch {
        key: CompilerObjectKind,
        payload: CompilerObjectKind,
    },
    #[error("AWBO stability {actual:?} does not match {expected:?} for {kind:?}")]
    StabilityMismatch {
        kind: CompilerObjectKind,
        actual: CompilerObjectStability,
        expected: CompilerObjectStability,
    },
    #[error("AWBO key digest mismatch")]
    KeyDigestMismatch,
    #[error("AWBO payload digest mismatch")]
    PayloadDigestMismatch,
    #[error("AWBO payload length mismatch: expected {expected}, actual {actual}")]
    PayloadLengthMismatch { expected: u64, actual: u64 },
    #[error("AWBO {field} is too large to encode")]
    PayloadTooLarge { field: &'static str },
    #[error("AWBO payload schema version {actual} does not match expected {expected}")]
    PayloadSchemaMismatch { actual: u32, expected: u32 },
    #[error("AWBO payload key input mismatch in {field}")]
    PayloadKeyInputMismatch { field: &'static str },
    #[error("malformed AWBO payload: {reason}")]
    MalformedPayload { reason: String },
}

impl CompilerObjectKind {
    pub const fn stability(self) -> CompilerObjectStability {
        match self {
            Self::InterfaceSummary => CompilerObjectStability::CrossCompiler,
            Self::ParsedSyntax
            | Self::HirBody
            | Self::TypecheckGate
            | Self::LineTaskEvidence
            | Self::RuntimePlanUnit
            | Self::BytecodeUnit
            | Self::LinkPlan => CompilerObjectStability::ExactCompilerIdentity,
        }
    }

    pub const fn cache_namespace(self) -> &'static str {
        match self {
            Self::ParsedSyntax => "parsed-syntax",
            Self::InterfaceSummary => "interface-summary",
            Self::HirBody => "hir-body",
            Self::TypecheckGate => "typecheck-gate",
            Self::LineTaskEvidence => "line-task-evidence",
            Self::RuntimePlanUnit => "runtime-plan-unit",
            Self::BytecodeUnit => "bytecode-unit",
            Self::LinkPlan => "link-plan",
        }
    }

    /// Query family allowed for adapter-owned read-through.
    ///
    /// Object families must opt in only after their payload has a stable
    /// validation contract and read/write-through tests.
    pub const fn safe_read_through_query_kind(self) -> Option<QueryKind> {
        match self {
            Self::ParsedSyntax => Some(QueryKind::Parse),
            Self::InterfaceSummary => Some(QueryKind::Interface),
            Self::HirBody => Some(QueryKind::HirBody),
            Self::TypecheckGate => Some(QueryKind::TypeCheck),
            Self::LineTaskEvidence
            | Self::RuntimePlanUnit
            | Self::BytecodeUnit
            | Self::LinkPlan => None,
        }
    }

    /// Artifact family allowed for adapter-owned read-through.
    pub const fn safe_read_through_artifact_kind(self) -> Option<ArtifactKind> {
        match self {
            Self::ParsedSyntax => Some(ArtifactKind::ParsedSyntax),
            Self::InterfaceSummary => Some(ArtifactKind::InterfaceSummary),
            Self::HirBody => Some(ArtifactKind::HirBody),
            Self::TypecheckGate => Some(ArtifactKind::TypeCheckReport),
            Self::LineTaskEvidence
            | Self::RuntimePlanUnit
            | Self::BytecodeUnit
            | Self::LinkPlan => None,
        }
    }

    /// Reverse mapping for object families enabled for safe read-through.
    pub const fn from_safe_read_through_artifact_kind(artifact_kind: ArtifactKind) -> Option<Self> {
        match artifact_kind {
            ArtifactKind::ParsedSyntax => Some(Self::ParsedSyntax),
            ArtifactKind::InterfaceSummary => Some(Self::InterfaceSummary),
            ArtifactKind::HirBody => Some(Self::HirBody),
            ArtifactKind::TypeCheckReport => Some(Self::TypecheckGate),
            ArtifactKind::RuntimePlan
            | ArtifactKind::BytecodeUnit
            | ArtifactKind::AssetMetadata
            | ArtifactKind::AssetPayload
            | ArtifactKind::LinkPlan
            | ArtifactKind::BundleSection
            | ArtifactKind::BundleIndex => None,
        }
    }

    pub const TYPECHECK_GATE_CONSERVATIVE_POLICY: &'static str =
        "typecheck-gate-valid-but-linked-sema-rebuilt";

    pub fn read_through_hit_status(self) -> CacheRecordStatus {
        if self.read_through_hit_requires_rebuild() {
            CacheRecordStatus::HitThenRebuilt {
                reason: InvalidationReason::ConservativeInvalidation {
                    policy: Self::TYPECHECK_GATE_CONSERVATIVE_POLICY.to_owned(),
                },
            }
        } else {
            CacheRecordStatus::Hit
        }
    }

    pub const fn read_through_hit_requires_rebuild(self) -> bool {
        matches!(self, Self::TypecheckGate)
    }

    pub const fn conservative_read_through_policy(self) -> Option<&'static str> {
        match self {
            Self::TypecheckGate => Some(Self::TYPECHECK_GATE_CONSERVATIVE_POLICY),
            Self::ParsedSyntax
            | Self::InterfaceSummary
            | Self::HirBody
            | Self::LineTaskEvidence
            | Self::RuntimePlanUnit
            | Self::BytecodeUnit
            | Self::LinkPlan => None,
        }
    }

    pub const fn wire_tag(self) -> u8 {
        match self {
            Self::ParsedSyntax => 0,
            Self::InterfaceSummary => 1,
            Self::HirBody => 2,
            Self::TypecheckGate => 7,
            Self::LineTaskEvidence => 3,
            Self::RuntimePlanUnit => 4,
            Self::BytecodeUnit => 5,
            Self::LinkPlan => 6,
        }
    }

    pub const fn from_wire_tag(tag: u8) -> Result<Self, AwboError> {
        match tag {
            0 => Ok(Self::ParsedSyntax),
            1 => Ok(Self::InterfaceSummary),
            2 => Ok(Self::HirBody),
            7 => Ok(Self::TypecheckGate),
            3 => Ok(Self::LineTaskEvidence),
            4 => Ok(Self::RuntimePlanUnit),
            5 => Ok(Self::BytecodeUnit),
            6 => Ok(Self::LinkPlan),
            _ => Err(AwboError::UnsupportedWireTag {
                domain: "compiler object kind",
                tag,
            }),
        }
    }
}

impl CompilerObjectStability {
    pub const fn wire_tag(self) -> u8 {
        match self {
            Self::CrossCompiler => 0,
            Self::ExactCompilerIdentity => 1,
        }
    }

    pub const fn from_wire_tag(tag: u8) -> Result<Self, AwboError> {
        match tag {
            0 => Ok(Self::CrossCompiler),
            1 => Ok(Self::ExactCompilerIdentity),
            _ => Err(AwboError::UnsupportedWireTag {
                domain: "compiler object stability",
                tag,
            }),
        }
    }
}

impl CompilerBuildIdentity {
    #[must_use]
    pub fn canonicalized(mut self) -> Self {
        self.enabled_features.sort();
        self.enabled_features.dedup();
        self
    }
}

impl CompilerObjectKey {
    #[must_use]
    pub fn canonicalized(mut self) -> Self {
        self.compiler = self.compiler.canonicalized();
        self.dependency_interface_digests =
            NamedDigest::canonicalize(self.dependency_interface_digests);
        self.dependency_body_digests = NamedDigest::canonicalize(self.dependency_body_digests);
        self
    }

    pub fn digest(&self) -> BuildDigest {
        let key = self.clone().canonicalized();
        let mut bytes = Vec::new();
        put_u32(&mut bytes, AWBO_SCHEMA_VERSION);
        put_string(&mut bytes, key.kind.cache_namespace());
        put_string(&mut bytes, &key.compiler.package_version);
        put_string(&mut bytes, &key.compiler.git_commit);
        put_string(&mut bytes, &key.compiler.rustc);
        put_string(&mut bytes, &key.compiler.target);
        put_string_vec(&mut bytes, &key.compiler.enabled_features);
        put_digest(&mut bytes, key.source_digest);
        put_digest(&mut bytes, key.query_options_digest);
        put_named_digests(&mut bytes, &key.dependency_interface_digests);
        put_named_digests(&mut bytes, &key.dependency_body_digests);
        put_digest(&mut bytes, key.environment_digest);
        BuildDigest::of(&bytes)
    }

    pub fn identity_namespace(&self) -> CompilerIdentityNamespaceObject {
        CompilerIdentityNamespaceObject::from_key(self)
    }

    pub fn stage_inputs(&self) -> CompilerStageInputsObject {
        CompilerStageInputsObject::from_key(self)
    }
}

impl CompilerIdentityNamespaceObject {
    pub fn from_key(key: &CompilerObjectKey) -> Self {
        Self {
            object_kind: key.kind,
            cache_namespace: key.kind.cache_namespace().to_owned(),
            compiler: key.compiler.clone().canonicalized(),
        }
    }

    #[must_use]
    pub fn canonicalized(mut self) -> Self {
        self.compiler = self.compiler.canonicalized();
        self
    }

    pub fn validate_for_key(&self, key: &CompilerObjectKey) -> Result<(), AwboError> {
        let actual = self.clone().canonicalized();
        let expected = Self::from_key(key);
        if actual.object_kind != expected.object_kind {
            return Err(AwboError::PayloadKeyInputMismatch {
                field: "compiler_namespace.object_kind",
            });
        }
        if actual.cache_namespace != expected.cache_namespace {
            return Err(AwboError::PayloadKeyInputMismatch {
                field: "compiler_namespace.cache_namespace",
            });
        }
        if actual.compiler != expected.compiler {
            return Err(AwboError::PayloadKeyInputMismatch {
                field: "compiler_namespace.compiler",
            });
        }
        Ok(())
    }
}

impl CompilerStageInputsObject {
    pub fn from_key(key: &CompilerObjectKey) -> Self {
        Self {
            query_options_digest: key.query_options_digest,
            dependency_interface_digests: NamedDigest::canonicalize(
                key.dependency_interface_digests.clone(),
            ),
            dependency_body_digests: NamedDigest::canonicalize(key.dependency_body_digests.clone()),
            environment_digest: key.environment_digest,
        }
    }

    #[must_use]
    pub fn canonicalized(mut self) -> Self {
        self.dependency_interface_digests =
            NamedDigest::canonicalize(self.dependency_interface_digests);
        self.dependency_body_digests = NamedDigest::canonicalize(self.dependency_body_digests);
        self
    }

    pub fn validate_for_key(&self, key: &CompilerObjectKey) -> Result<(), AwboError> {
        if self.clone().canonicalized() != Self::from_key(key) {
            return Err(AwboError::PayloadKeyInputMismatch {
                field: "stage_inputs",
            });
        }
        Ok(())
    }

    pub fn dependency_interface_digest_root(&self) -> BuildDigest {
        let mut bytes = Vec::new();
        put_u32(&mut bytes, AWBO_SCHEMA_VERSION);
        put_string(&mut bytes, "dependency-interface-digests");
        let values = NamedDigest::canonicalize(self.dependency_interface_digests.clone());
        put_named_digests(&mut bytes, &values);
        BuildDigest::of(&bytes)
    }
}
