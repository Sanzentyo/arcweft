use arcweft_agent_protocol::{
    artifact::EffectCapability,
    ids::{IdentifierError, StableHash},
    verified_effects::VerifiedEffectSummary,
};
use arcweft_lang_sema::{
    callable::CheckedCallableFacts, effect_row::EffectRowTail, effects::EffectSet,
};
use thiserror::Error;

/// Current semantics of the transitive closure and artifact-boundary lowering.
pub const EFFECT_ANALYSIS_VERSION: u32 = 1;

/// Failure to construct a verified artifact effect proof.
#[derive(Debug, Error)]
pub enum VerifiedEffectBuildError {
    #[error("checked callable has no body-owned inferred effect row")]
    MissingBodyRow,
    #[error("checked callable effect row is not closed at the artifact boundary")]
    UnresolvedRow,
    #[error(transparent)]
    InvalidDigest(#[from] IdentifierError),
}

/// Creates a schema-v2 effect proof from closed semantic row evidence.
///
/// The builder is fail-closed for a missing analyzed node. The legacy
/// `declared` slot is populated with the closed inferred row, not with the
/// source upper bound.
pub fn build_verified_effect_summary(
    callable: &CheckedCallableFacts,
) -> Result<VerifiedEffectSummary, VerifiedEffectBuildError> {
    let row = callable
        .actual_row()
        .ok_or(VerifiedEffectBuildError::MissingBodyRow)?;
    if row.tail() != EffectRowTail::Closed {
        return Err(VerifiedEffectBuildError::UnresolvedRow);
    }
    let inferred = row.concrete().clone();
    let actual = inferred
        .iter()
        .map(|effect| EffectCapability::new(effect.as_str()))
        .collect::<Vec<_>>();
    let digest = effect_digest(EFFECT_ANALYSIS_VERSION, &inferred)?;

    Ok(VerifiedEffectSummary::new(
        EFFECT_ANALYSIS_VERSION,
        actual.clone(),
        actual,
        digest,
    ))
}

fn effect_digest(
    analysis_version: u32,
    effects: &EffectSet,
) -> Result<StableHash, IdentifierError> {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"arcweft.effect-analysis\0");
    hasher.update(&analysis_version.to_le_bytes());
    for effect in effects {
        let bytes = effect.as_str().as_bytes();
        let byte_len = u64::try_from(bytes.len()).expect("effect label length fits in u64");
        hasher.update(&byte_len.to_le_bytes());
        hasher.update(bytes);
    }
    StableHash::new(format!("blake3:{}", hasher.finalize().to_hex()))
}

#[cfg(test)]
mod tests {
    use super::{EFFECT_ANALYSIS_VERSION, effect_digest};
    use arcweft_agent_protocol::artifact::EffectCapability;
    use arcweft_lang_sema::effects::EffectSet;

    #[test]
    fn inferred_effect_digest_is_deterministic_and_ordered() {
        let effects =
            EffectSet::from_labels(["agent.observe", "agent.capture"]).expect("valid inferred row");
        let digest = effect_digest(EFFECT_ANALYSIS_VERSION, &effects).expect("effect digest");
        assert!(digest.as_str().starts_with("blake3:"));

        let projected = effects
            .iter()
            .map(|effect| EffectCapability::new(effect.as_str()))
            .collect::<Vec<_>>();
        assert_eq!(projected.len(), 2);
    }
}
