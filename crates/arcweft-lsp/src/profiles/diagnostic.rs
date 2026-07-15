use arcweft_launch::{LaunchDocumentError, LaunchProfileError};
use arcweft_source::SourceSpan;
use std::fmt;
use thiserror::Error;

/// One profile metadata diagnostic with exact source provenance when available.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LspProfileDiagnostic {
    kind: LspProfileDiagnosticKind,
    message: String,
    profile_id: Option<String>,
    resource: Option<String>,
    source: Option<SourceSpan>,
}

/// Stable profile diagnostic categories.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LspProfileDiagnosticKind {
    /// The document URI was not a local file URI.
    NonFileDocumentUri,
    /// No project manifest was found for an opened document.
    WorkspaceManifestNotFound,
    /// The project manifest could not be read.
    ManifestRead,
    /// The project manifest could not be parsed.
    ManifestParse,
    /// The selected profile could not be resolved.
    ProfileResolve,
    /// A project-local adapter manifest could not be read.
    AdapterManifestRead,
    /// A project-local adapter manifest could not be parsed.
    AdapterManifestParse,
    /// Rust ABI metadata could not be read.
    RustMetadataRead,
    /// Rust ABI metadata could not be parsed.
    RustMetadataParse,
    /// A character manifest could not be read.
    CharacterManifestRead,
    /// A character manifest could not be parsed or validated.
    CharacterManifestParse,
    /// Character manifests declared duplicate public character ids.
    CharacterCatalog,
}

#[derive(Debug, Error)]
pub(super) enum LspProfileLoadError {
    #[error("document URI is not a local file URI")]
    NonFileDocumentUri,
    #[error("no arcw.toml manifest was found for this document")]
    WorkspaceManifestNotFound,
    #[error("failed to read arcw.toml: {0}")]
    ManifestRead(std::io::Error),
    #[error("failed to bind arcw.toml source: {0}")]
    ManifestSource(String),
    #[error("{0}")]
    ManifestParse(LaunchDocumentError),
    #[error("{0}")]
    ProfileResolve(LaunchProfileError),
}

impl LspProfileDiagnostic {
    /// Creates a typed profile diagnostic.
    pub fn new(kind: LspProfileDiagnosticKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            profile_id: None,
            resource: None,
            source: None,
        }
    }

    /// Attaches the selected launch profile id without embedding host paths.
    #[must_use]
    pub fn with_profile_id(mut self, profile_id: impl Into<String>) -> Self {
        self.profile_id = Some(profile_id.into());
        self
    }

    /// Attaches a profile-relative resource label.
    #[must_use]
    pub fn with_resource(mut self, resource: impl Into<String>) -> Self {
        self.resource = Some(resource.into());
        self
    }

    /// Attaches the exact launch-manifest token that selected this resource.
    #[must_use]
    pub fn with_source(mut self, source: SourceSpan) -> Self {
        self.source = Some(source);
        self
    }

    /// Diagnostic category.
    pub const fn kind(&self) -> LspProfileDiagnosticKind {
        self.kind
    }

    /// Human-readable diagnostic message.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Optional launch profile id associated with this diagnostic.
    pub fn profile_id(&self) -> Option<&str> {
        self.profile_id.as_deref()
    }

    /// Optional profile-relative resource associated with this diagnostic.
    pub fn resource(&self) -> Option<&str> {
        self.resource.as_deref()
    }

    /// Exact revision-bound launch token associated with this diagnostic.
    pub const fn source(&self) -> Option<&SourceSpan> {
        self.source.as_ref()
    }
}

impl LspProfileDiagnosticKind {
    /// Stable code used in LSP diagnostics.
    pub const fn code(self) -> &'static str {
        match self {
            Self::NonFileDocumentUri => "profile.uri.non_file",
            Self::WorkspaceManifestNotFound => "profile.manifest.missing",
            Self::ManifestRead => "profile.manifest.read",
            Self::ManifestParse => "profile.manifest.parse",
            Self::ProfileResolve => "profile.resolve",
            Self::AdapterManifestRead => "profile.adapter_manifest.read",
            Self::AdapterManifestParse => "profile.adapter_manifest.parse",
            Self::RustMetadataRead => "profile.rust_metadata.read",
            Self::RustMetadataParse => "profile.rust_metadata.parse",
            Self::CharacterManifestRead => "profile.character_manifest.read",
            Self::CharacterManifestParse => "profile.character_manifest.parse",
            Self::CharacterCatalog => "profile.character_manifest.catalog",
        }
    }
}

impl LspProfileLoadError {
    pub(super) fn into_diagnostic(self) -> LspProfileDiagnostic {
        let kind = match self {
            Self::NonFileDocumentUri => LspProfileDiagnosticKind::NonFileDocumentUri,
            Self::WorkspaceManifestNotFound => LspProfileDiagnosticKind::WorkspaceManifestNotFound,
            Self::ManifestRead(_) => LspProfileDiagnosticKind::ManifestRead,
            Self::ManifestSource(_) | Self::ManifestParse(_) => {
                LspProfileDiagnosticKind::ManifestParse
            }
            Self::ProfileResolve(_) => LspProfileDiagnosticKind::ProfileResolve,
        };
        LspProfileDiagnostic::new(kind, self.to_string())
    }
}

impl fmt::Display for LspProfileDiagnosticKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}
