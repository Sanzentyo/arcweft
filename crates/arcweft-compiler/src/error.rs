use crate::effect_manifest;
use thiserror::Error;

/// Source compiler diagnostics for the shared driver.
#[derive(Debug, Error)]
pub enum CompileSourceError {
    #[error("parse errors: {0:?}")]
    Parse(Vec<arcweft_lang_syntax::parser::recovery::ParseError>),
    #[error("HIR lowering errors: {0:?}")]
    Hir(Vec<arcweft_lang_hir::model::HirLowerError>),
    #[error("reference resolution errors: {0:?}")]
    Resolve(Vec<arcweft_lang_sema::resolve::NameResolutionError>),
    #[error("type-check readiness errors: {0:?}")]
    Readiness(Vec<arcweft_lang_sema::diagnostics::TypeCheckReadinessError>),
    #[error("type errors: {0:?}")]
    Type(Vec<arcweft_lang_sema::diagnostics::TypeCheckError>),
    #[error("runtime-plan lowering errors: {0:?}")]
    RuntimePlan(Vec<arcweft_runtime_plan::errors::RuntimePlanLowerError>),
}

/// Agent controller compiler diagnostics.
#[derive(Debug, Error)]
pub enum CompileAgentError {
    #[error("parse errors: {0:?}")]
    Parse(Vec<arcweft_lang_syntax::parser::recovery::ParseError>),
    #[error("HIR lowering errors: {0:?}")]
    Hir(Vec<arcweft_lang_hir::model::HirLowerError>),
    #[error("reference resolution errors: {0:?}")]
    Resolve(Vec<arcweft_lang_sema::resolve::NameResolutionError>),
    #[error("type-check readiness errors: {0:?}")]
    Readiness(Vec<arcweft_lang_sema::diagnostics::TypeCheckReadinessError>),
    #[error("type errors: {0:?}")]
    Type(Vec<arcweft_lang_sema::diagnostics::TypeCheckError>),
    #[error("agent source did not declare a top-level `agent` item")]
    MissingAgent,
    #[error("agent bundle compilation requires exactly one top-level `agent` item, found {0}")]
    MultipleAgents(usize),
    #[error("agent runtime-plan lowering errors: {0:?}")]
    RuntimePlan(Vec<arcweft_runtime_plan::errors::RuntimePlanLowerError>),
    #[error("agent artifact identifier error: {0}")]
    ArtifactIdentifier(#[from] arcweft_agent_protocol::ids::IdentifierError),
    #[error("agent verified effect manifest error: {0}")]
    EffectManifest(#[from] effect_manifest::VerifiedEffectBuildError),
    #[error("agent budget attribute error: {0}")]
    Budget(String),
}

/// HIR semantic validation diagnostics for the shared compiler driver.
#[derive(Debug, Error)]
pub enum ValidateHirError {
    #[error("reference resolution errors: {0:?}")]
    Resolve(Vec<arcweft_lang_sema::resolve::NameResolutionError>),
    #[error("type-check readiness errors: {0:?}")]
    Readiness(Vec<arcweft_lang_sema::diagnostics::TypeCheckReadinessError>),
    #[error("type errors: {0:?}")]
    Type(Vec<arcweft_lang_sema::diagnostics::TypeCheckError>),
}
