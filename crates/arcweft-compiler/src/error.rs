use crate::{effect_manifest, project::ProjectCompileError};
use thiserror::Error;

/// Source compiler diagnostics for the shared driver.
#[derive(Debug, Error)]
pub enum CompileSourceError {
    #[error(transparent)]
    Project(#[from] ProjectCompileError),
}

impl CompileSourceError {
    pub const fn project(&self) -> &ProjectCompileError {
        match self {
            Self::Project(error) => error,
        }
    }
}

/// Agent controller compiler diagnostics.
#[derive(Debug, Error)]
pub enum CompileAgentError {
    #[error("agent artifact identifier error: {0}")]
    ArtifactIdentifier(#[from] arcweft_agent_protocol::ids::IdentifierError),
    #[error("agent verified effect manifest error: {0}")]
    EffectManifest(#[from] effect_manifest::VerifiedEffectBuildError),
    #[error("agent product source-map error: {0}")]
    SourceMap(#[from] arcweft_bundle::resource_codec::SourceMapBuildError),
    #[error("selected source entry `{entry}` does not exist")]
    MissingSelectedEntry { entry: String },
    #[error("selected source entry `{entry}` is not an Agent entry")]
    SelectedEntryNotAgent { entry: String },
    #[error(
        "selected Agent controller `{controller}` resolved to {matches} exact ordinary function declarations"
    )]
    ControllerDeclarationCardinality { controller: String, matches: usize },
    #[error(
        "selected Agent controller `{controller}` does not match the exact HIR function accepted by project compilation"
    )]
    ControllerFunctionMismatch { controller: String },
    #[error("selected Agent entry `{entry}` has no checked runtime entry")]
    MissingRuntimeEntry { entry: String },
    #[error("selected Agent entry `{entry}` has an invalid runtime target or role catalog")]
    InvalidRuntimeEntry { entry: String },
    #[error("selected Agent entry `{entry}` does not match the supplied project semantic index")]
    ProjectIndexEntryMismatch { entry: String },
    #[error("compiled project module `{module}` has no bound source document")]
    MissingSourceDocument { module: String },
    #[error("selected Agent runtime plan failed verification: {0}")]
    RuntimePlanVerification(String),
}
