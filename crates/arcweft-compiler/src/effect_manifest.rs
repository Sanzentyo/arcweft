use arcweft_agent_protocol::{
    artifact::EffectCapability,
    ids::{IdentifierError, StableHash},
    verified_effects::VerifiedEffectSummary,
};
use arcweft_lang_sema::{
    effect_analysis::EffectAnalysisReport, effect_model::CallableId, effects::EffectSet,
};
use thiserror::Error;

/// Current semantics of the transitive closure and public-boundary checks.
pub const EFFECT_ANALYSIS_VERSION: u32 = 1;

/// Failure to construct a verified artifact effect proof.
#[derive(Debug, Error)]
pub enum VerifiedEffectBuildError {
    #[error("effect analysis contains errors; refusing to build an effect proof")]
    AnalysisFailed,
    #[error("effect analysis report has no summary for `{callable}`")]
    MissingSummary { callable: CallableId },
    #[error("artifact boundary `{callable}` has no explicit effect declaration")]
    MissingDeclaration { callable: CallableId },
    #[error(
        "artifact boundary `{callable}` infers {inferred}, which exceeds declared upper bound {declared}"
    )]
    DeclarationViolation {
        callable: CallableId,
        inferred: EffectSet,
        declared: EffectSet,
    },
    #[error(transparent)]
    InvalidDigest(#[from] IdentifierError),
}

/// Creates a schema-v2 effect proof from a successful semantic report.
///
/// The builder is fail-closed: it accepts neither a report with diagnostics nor
/// a caller-supplied declaration that could disagree with the analyzed node.
pub fn build_verified_effect_summary(
    callable: CallableId,
    report: &EffectAnalysisReport,
) -> Result<VerifiedEffectSummary, VerifiedEffectBuildError> {
    if report.has_errors() {
        return Err(VerifiedEffectBuildError::AnalysisFailed);
    }
    let summary =
        report
            .summary(&callable)
            .ok_or_else(|| VerifiedEffectBuildError::MissingSummary {
                callable: callable.clone(),
            })?;
    let declared =
        summary
            .declared()
            .ok_or_else(|| VerifiedEffectBuildError::MissingDeclaration {
                callable: callable.clone(),
            })?;
    if !summary.inferred().is_subset(declared) {
        return Err(VerifiedEffectBuildError::DeclarationViolation {
            callable,
            inferred: summary.inferred().clone(),
            declared: declared.clone(),
        });
    }

    let declared = declared
        .iter()
        .map(|effect| EffectCapability::new(effect.as_str()))
        .collect();
    let inferred = summary
        .inferred()
        .iter()
        .map(|effect| EffectCapability::new(effect.as_str()))
        .collect::<Vec<_>>();
    let digest = effect_digest(EFFECT_ANALYSIS_VERSION, summary.inferred())?;

    Ok(VerifiedEffectSummary::new(
        EFFECT_ANALYSIS_VERSION,
        declared,
        inferred,
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
