use crate::{effect_manifest, project::ProjectCompileError};
use thiserror::Error;

/// Source compiler diagnostics for the shared driver.
#[derive(Debug, Error)]
pub enum CompileSourceError {
    #[error(transparent)]
    SyntaxDatabase(#[from] arcweft_lang_syntax::incremental::SyntaxDatabaseCreateError),
    #[error(transparent)]
    SyntaxParse(#[from] arcweft_lang_syntax::incremental::ParseFailure),
    #[error(transparent)]
    HirDatabase(#[from] arcweft_lang_hir::identity::HirDatabaseCreateError),
    #[error(transparent)]
    Project(#[from] ProjectCompileError),
}

impl CompileSourceError {
    /// Returns the project-compilation failure.
    ///
    /// # Panics
    ///
    /// Panics when source compilation failed before reaching project
    /// compilation.
    pub const fn project(&self) -> &ProjectCompileError {
        match self {
            Self::Project(error) => error,
            Self::SyntaxDatabase(_) | Self::SyntaxParse(_) | Self::HirDatabase(_) => {
                panic!("source compiler failed before project compilation")
            }
        }
    }
}

/// Agent controller compiler diagnostics.
#[derive(Debug, Error)]
pub enum CompileAgentError {
    #[error("agent runtime diagnostic identity error: {0}")]
    RuntimeDiagnosticIdentity(Box<crate::runtime_diagnostics::ExecutionDiagnosticContextError>),
    #[error("agent artifact identifier error: {0}")]
    ArtifactIdentifier(Box<arcweft_agent_protocol::ids::IdentifierError>),
    #[error("agent verified effect manifest error: {0}")]
    EffectManifest(Box<effect_manifest::VerifiedEffectBuildError>),
    #[error("agent product source-map error: {0}")]
    SourceMap(Box<arcweft_bundle::resource_codec::SourceMapBuildError>),
    #[error("selected source entry `{entry}` does not exist")]
    MissingSelectedEntry { entry: String },
    #[error("selected source entry `{entry}` is not an Agent entry")]
    SelectedEntryNotAgent { entry: String },
    #[error(
        "selected Agent controller `{controller}` resolved to {matches} exact ordinary function declarations"
    )]
    ControllerDeclarationCardinality { controller: String, matches: usize },
    #[error("selected Agent controller `{controller}` has no exact checked callable facts")]
    MissingControllerSemanticFacts { controller: String },
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

impl From<crate::runtime_diagnostics::ExecutionDiagnosticContextError> for CompileAgentError {
    fn from(error: crate::runtime_diagnostics::ExecutionDiagnosticContextError) -> Self {
        Self::RuntimeDiagnosticIdentity(Box::new(error))
    }
}

impl From<arcweft_agent_protocol::ids::IdentifierError> for CompileAgentError {
    fn from(error: arcweft_agent_protocol::ids::IdentifierError) -> Self {
        Self::ArtifactIdentifier(Box::new(error))
    }
}

impl From<effect_manifest::VerifiedEffectBuildError> for CompileAgentError {
    fn from(error: effect_manifest::VerifiedEffectBuildError) -> Self {
        Self::EffectManifest(Box::new(error))
    }
}

impl From<arcweft_bundle::resource_codec::SourceMapBuildError> for CompileAgentError {
    fn from(error: arcweft_bundle::resource_codec::SourceMapBuildError) -> Self {
        Self::SourceMap(Box::new(error))
    }
}
