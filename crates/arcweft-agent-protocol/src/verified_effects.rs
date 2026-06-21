use serde::{Deserialize, Serialize};

use crate::{artifact::EffectCapability, ids::StableHash};

/// Compiler-verified transitive effect closure stored in schema-v2 artifacts.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct VerifiedEffectSummary {
    /// Version of the semantic closure rules, independent from the compiler version.
    pub analysis_version: u32,
    /// Source-declared upper bound at the public artifact boundary.
    pub declared: Vec<EffectCapability>,
    /// Compiler-inferred transitive closure after successful validation.
    pub inferred: Vec<EffectCapability>,
    /// Hash of canonical analysis version + sorted inferred effects.
    pub digest: StableHash,
}

impl VerifiedEffectSummary {
    pub fn new(
        analysis_version: u32,
        mut declared: Vec<EffectCapability>,
        mut inferred: Vec<EffectCapability>,
        digest: StableHash,
    ) -> Self {
        declared.sort();
        declared.dedup();
        inferred.sort();
        inferred.dedup();
        Self {
            analysis_version,
            declared,
            inferred,
            digest,
        }
    }
}
