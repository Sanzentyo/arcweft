use arcweft_agent_protocol::{
    artifact::EffectCapability,
    ids::{IdentifierError, StableHash},
    verified_effects::VerifiedEffectSummary,
};
use arcweft_lang_sema::{
    check::{ClosedEffectRowReport, EffectRowCloseError},
    effect_model::CallableId,
    effects::EffectSet,
};
use thiserror::Error;

/// Current semantics of the transitive closure and artifact-boundary lowering.
pub const EFFECT_ANALYSIS_VERSION: u32 = 1;

/// Failure to construct a verified artifact effect proof.
#[derive(Debug, Error)]
pub enum VerifiedEffectBuildError {
    #[error("effect analysis report has no summary for `{callable}`")]
    MissingSummary { callable: CallableId },
    #[error("effect analysis row report contains unresolved rows: {source}")]
    InvalidRows {
        #[source]
        source: EffectRowCloseError,
    },
    #[error(transparent)]
    InvalidDigest(#[from] IdentifierError),
}

impl From<EffectRowCloseError> for VerifiedEffectBuildError {
    fn from(source: EffectRowCloseError) -> Self {
        Self::InvalidRows { source }
    }
}

/// Creates a schema-v2 effect proof from closed semantic row evidence.
///
/// The builder is fail-closed for a missing analyzed node. The legacy
/// `declared` slot is populated with the closed inferred row, not with the
/// source upper bound.
pub fn build_verified_effect_summary(
    callable: &CallableId,
    rows: &ClosedEffectRowReport,
) -> Result<VerifiedEffectSummary, VerifiedEffectBuildError> {
    let summary =
        rows.summary(callable)
            .ok_or_else(|| VerifiedEffectBuildError::MissingSummary {
                callable: callable.clone(),
            })?;
    let inferred = summary.inferred().clone();
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
    use super::*;
    use arcweft_lang_sema::check::ClosedEffectRowSummary;

    #[test]
    fn verified_effect_summary_uses_closed_inferred_row() {
        let callable = CallableId::new("agent.observe_smoke");
        let inferred = EffectSet::from_labels(["agent.observe"]).expect("valid inferred row");
        let upper_bound =
            EffectSet::from_labels(["agent.observe", "agent.capture"]).expect("valid upper row");
        let rows = ClosedEffectRowReport::new([ClosedEffectRowSummary::new(
            callable.clone(),
            inferred,
            Some(upper_bound),
            EffectSet::new(),
        )]);

        let summary =
            build_verified_effect_summary(&callable, &rows).expect("verified summary builds");

        assert_eq!(
            summary
                .declared
                .iter()
                .map(EffectCapability::as_str)
                .collect::<Vec<_>>(),
            vec!["agent.observe"]
        );
        assert_eq!(
            summary
                .inferred
                .iter()
                .map(EffectCapability::as_str)
                .collect::<Vec<_>>(),
            vec!["agent.observe"]
        );
        assert!(summary.digest.as_str().starts_with("blake3:"));
    }

    #[test]
    fn verified_effect_summary_rejects_missing_callable_row() {
        let rows = ClosedEffectRowReport::default();
        let callable = CallableId::new("agent.missing");

        let error =
            build_verified_effect_summary(&callable, &rows).expect_err("missing row is rejected");

        assert!(matches!(
            error,
            VerifiedEffectBuildError::MissingSummary { callable: missing }
                if missing == callable
        ));
    }
}
