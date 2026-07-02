//! Typed producer identity evidence for bytecode/link persistent reuse.
//!
//! This module is a data-contract boundary.  CLI/build/runtime adapters may
//! attach producer evidence, but stable family names, classifications, and
//! conservative reasons are owned here so cache explain output is typed and
//! auditable.

use crate::fingerprint::BuildDigest;
use serde::{Deserialize, Serialize};

/// Every audited family that can produce, package, synthesize, or consume
/// bytecode/link-related artifacts.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BytecodeLinkProducerFamily {
    FullBuild,
    FullBuildWatch,
    DirectBundle,
    SingleSourceCompile,
    PatchBundle,
    AgentScript,
    RuntimeDriver,
    FixtureRegeneration,
    PersistentCacheTestBuilder,
}

pub const AUDITED_BYTECODE_LINK_PRODUCER_FAMILIES: &[BytecodeLinkProducerFamily] = &[
    BytecodeLinkProducerFamily::FullBuild,
    BytecodeLinkProducerFamily::FullBuildWatch,
    BytecodeLinkProducerFamily::DirectBundle,
    BytecodeLinkProducerFamily::SingleSourceCompile,
    BytecodeLinkProducerFamily::PatchBundle,
    BytecodeLinkProducerFamily::AgentScript,
    BytecodeLinkProducerFamily::RuntimeDriver,
    BytecodeLinkProducerFamily::FixtureRegeneration,
    BytecodeLinkProducerFamily::PersistentCacheTestBuilder,
];

/// Request-level classification for a bytecode/link producer family.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BytecodeLinkProducerClassification {
    ActualReusableReady,
    ActualReusableAfterIdentityWork,
    ConservativeRequired,
    NotABytecodeLinkProducer,
}

/// The owned boundary type that provides, or must provide, identity for a family.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BytecodeLinkIdentityOwner {
    FullBuildPersistentArtifactContext,
    FullBuildBytecodeUnitArtifact,
    FullBuildLinkPlanArtifact,
    DirectBundleProducerIdentity,
    CompileEmit,
    PatchTargetMaterializationIdentity,
    AgentScriptProducerIdentity,
    BundleRunnerRuntimeProgram,
    FixtureRegenerationProducerIdentity,
    PersistentCacheTestBuilder,
}

/// Typed conservative continuation reason for bytecode/link reuse.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BytecodeLinkConservativeReason {
    TypecheckGateLinkedSemaUnavailable,
    FullBuildMultiModuleProductAwbcNotNarrowed,
    DirectBundlePersistentIdentityUnavailable,
    PatchTargetProductIdentityUnavailable,
    AgentScriptProducerIdentityUnavailable,
    RuntimeDriverConsumesOnly,
    SingleSourceCompileEmitsPlanOnly,
    FixtureOnlySyntheticIdentity,
}

/// Cache-explain-safe producer identity evidence.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BytecodeLinkProducerEvidence {
    pub family: BytecodeLinkProducerFamily,
    pub classification: BytecodeLinkProducerClassification,
    pub identity_owner: BytecodeLinkIdentityOwner,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actual_identity: Option<BuildDigest>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conservative_reason: Option<BytecodeLinkConservativeReason>,
}

impl BytecodeLinkProducerFamily {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::FullBuild => "full_build",
            Self::FullBuildWatch => "full_build_watch",
            Self::DirectBundle => "direct_bundle",
            Self::SingleSourceCompile => "single_source_compile",
            Self::PatchBundle => "patch_bundle",
            Self::AgentScript => "agent_script",
            Self::RuntimeDriver => "runtime_driver",
            Self::FixtureRegeneration => "fixture_regeneration",
            Self::PersistentCacheTestBuilder => "persistent_cache_test_builder",
        }
    }

    pub const fn default_classification(self) -> BytecodeLinkProducerClassification {
        match self {
            Self::FullBuild | Self::FullBuildWatch | Self::PersistentCacheTestBuilder => {
                BytecodeLinkProducerClassification::ActualReusableReady
            }
            Self::DirectBundle | Self::AgentScript => {
                BytecodeLinkProducerClassification::ActualReusableAfterIdentityWork
            }
            Self::PatchBundle | Self::FixtureRegeneration => {
                BytecodeLinkProducerClassification::ConservativeRequired
            }
            Self::SingleSourceCompile | Self::RuntimeDriver => {
                BytecodeLinkProducerClassification::NotABytecodeLinkProducer
            }
        }
    }
}

impl BytecodeLinkProducerClassification {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ActualReusableReady => "actual_reusable_ready",
            Self::ActualReusableAfterIdentityWork => "actual_reusable_after_identity_work",
            Self::ConservativeRequired => "conservative_required",
            Self::NotABytecodeLinkProducer => "not_a_bytecode_link_producer",
        }
    }
}

impl BytecodeLinkConservativeReason {
    pub const fn policy(self) -> &'static str {
        match self {
            Self::TypecheckGateLinkedSemaUnavailable => {
                "typecheck-gate-valid-but-linked-sema-rebuilt"
            }
            Self::FullBuildMultiModuleProductAwbcNotNarrowed => {
                "full-build-product-awbc-not-narrowed-to-reusable-unit"
            }
            Self::DirectBundlePersistentIdentityUnavailable => {
                "direct-bundle-persistent-producer-identity-unavailable"
            }
            Self::PatchTargetProductIdentityUnavailable => {
                "patch-target-product-identity-unavailable"
            }
            Self::AgentScriptProducerIdentityUnavailable => {
                "agent-script-producer-identity-unavailable"
            }
            Self::RuntimeDriverConsumesOnly => "runtime-driver-consumes-product-bytecode-only",
            Self::SingleSourceCompileEmitsPlanOnly => "single-source-compile-emits-plan-only",
            Self::FixtureOnlySyntheticIdentity => "fixture-only-synthetic-bytecode-link-identity",
        }
    }

    pub const fn missing_identity(self) -> &'static str {
        match self {
            Self::TypecheckGateLinkedSemaUnavailable => {
                "linked semantic/typecheck report identity is not persisted as a reusable boundary"
            }
            Self::FullBuildMultiModuleProductAwbcNotNarrowed => {
                "per-module or per-SCC runtime-plan-unit digest plus narrowed canonical AWBC unit bytes"
            }
            Self::DirectBundlePersistentIdentityUnavailable => {
                "DirectBundleProducerIdentity covering source selection, include spaces, bundle format, adapter manifests, and product AWBC section identity"
            }
            Self::PatchTargetProductIdentityUnavailable => {
                "PatchTargetMaterializationIdentity covering target product identity and section compatibility matrix"
            }
            Self::AgentScriptProducerIdentityUnavailable => {
                "AgentScriptProducerIdentity covering script source, agent manifest, controller bundle schema, host calls, and lowering policy"
            }
            Self::RuntimeDriverConsumesOnly => {
                "no producer identity is required because this family only consumes decoded product bytecode"
            }
            Self::SingleSourceCompileEmitsPlanOnly => {
                "no bytecode/link identity is required because arcw compile emits check/HIR/plan only"
            }
            Self::FixtureOnlySyntheticIdentity => {
                "fixture generator identity must remain under fixture/test-only ownership"
            }
        }
    }

    pub const fn consumer_boundary(self) -> &'static str {
        match self {
            Self::TypecheckGateLinkedSemaUnavailable => {
                "typecheck read-through may validate facts but linked semantic consumers rebuild"
            }
            Self::FullBuildMultiModuleProductAwbcNotNarrowed => {
                "ordinary build/watch still relinks linked product AWBC for multi-module projects"
            }
            Self::DirectBundlePersistentIdentityUnavailable => {
                "direct bundle writes bundle bytes but has no persistent bytecode/link consumer contract yet"
            }
            Self::PatchTargetProductIdentityUnavailable => {
                "patch materialization consumes base/target AWFB bytes without a reusable target bytecode/link record"
            }
            Self::AgentScriptProducerIdentityUnavailable => {
                "agent script packaging has not declared a bytecode/link persistent consumer boundary"
            }
            Self::RuntimeDriverConsumesOnly => {
                "runtime driver selects and executes product bytecode but does not create reusable records"
            }
            Self::SingleSourceCompileEmitsPlanOnly => {
                "single-source compile stops before bytecode/link production"
            }
            Self::FixtureOnlySyntheticIdentity => {
                "synthetic actual identities are allowed only for fixtures and tests"
            }
        }
    }

    pub const fn follow_up_sequence(self) -> Option<&'static str> {
        match self {
            Self::FullBuildMultiModuleProductAwbcNotNarrowed => Some("seq04.8.3.2"),
            Self::DirectBundlePersistentIdentityUnavailable => Some("seq04.8.3.1"),
            Self::PatchTargetProductIdentityUnavailable => Some("seq04.8.3.4"),
            Self::AgentScriptProducerIdentityUnavailable => Some("seq04.8.3.3"),
            Self::TypecheckGateLinkedSemaUnavailable
            | Self::RuntimeDriverConsumesOnly
            | Self::SingleSourceCompileEmitsPlanOnly
            | Self::FixtureOnlySyntheticIdentity => None,
        }
    }
}

impl BytecodeLinkProducerEvidence {
    pub const fn actual(
        family: BytecodeLinkProducerFamily,
        identity_owner: BytecodeLinkIdentityOwner,
        actual_identity: BuildDigest,
    ) -> Self {
        Self {
            family,
            classification: BytecodeLinkProducerClassification::ActualReusableReady,
            identity_owner,
            actual_identity: Some(actual_identity),
            conservative_reason: None,
        }
    }

    pub const fn conservative(
        family: BytecodeLinkProducerFamily,
        identity_owner: BytecodeLinkIdentityOwner,
        reason: BytecodeLinkConservativeReason,
    ) -> Self {
        Self {
            family,
            classification: BytecodeLinkProducerClassification::ConservativeRequired,
            identity_owner,
            actual_identity: None,
            conservative_reason: Some(reason),
        }
    }

    pub const fn excluded(
        family: BytecodeLinkProducerFamily,
        identity_owner: BytecodeLinkIdentityOwner,
        reason: BytecodeLinkConservativeReason,
    ) -> Self {
        Self {
            family,
            classification: BytecodeLinkProducerClassification::NotABytecodeLinkProducer,
            identity_owner,
            actual_identity: None,
            conservative_reason: Some(reason),
        }
    }
}
