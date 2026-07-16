use arcweft_source::SourceSpan;
use std::{fmt, path::Path};
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
    #[error("{source}")]
    Environment {
        profile_id: Option<String>,
        #[source]
        source: Box<super::environment::RegisterProfileEnvironmentError>,
    },
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
            Self::Environment { profile_id, source } => {
                return environment_diagnostic(*source, profile_id);
            }
        };
        LspProfileDiagnostic::new(kind, self.to_string())
    }
}

fn environment_diagnostic(
    error: super::environment::RegisterProfileEnvironmentError,
    profile_id: Option<String>,
) -> LspProfileDiagnostic {
    let mut diagnostic = match error {
        super::environment::RegisterProfileEnvironmentError::Topology(error) => {
            topology_diagnostic(error.as_ref())
        }
        super::environment::RegisterProfileEnvironmentError::RegistrationLoad(error) => {
            LspProfileDiagnostic::new(
                LspProfileDiagnosticKind::CharacterCatalog,
                error.to_string(),
            )
        }
        error => LspProfileDiagnostic::new(
            LspProfileDiagnosticKind::CharacterCatalog,
            error.to_string(),
        ),
    };
    if let Some(profile_id) = profile_id {
        diagnostic = diagnostic.with_profile_id(profile_id);
    }
    diagnostic
}

fn topology_diagnostic(
    error: &arcweft_project_loader::topology::ProfileTopologyLoadError,
) -> LspProfileDiagnostic {
    use arcweft_project_loader::topology::ProfileTopologyLoadError as Error;

    match error {
        Error::ResourceRead { id, .. } => {
            let resource = id.path().as_str();
            let kind = if resource == "arcw.toml" {
                LspProfileDiagnosticKind::ManifestRead
            } else if resource.contains(".awchar") {
                LspProfileDiagnosticKind::CharacterManifestRead
            } else if Path::new(resource)
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
            {
                LspProfileDiagnosticKind::RustMetadataRead
            } else {
                LspProfileDiagnosticKind::AdapterManifestRead
            };
            LspProfileDiagnostic::new(kind, format!("failed to read `{resource}`"))
                .with_resource(resource)
        }
        Error::CharacterManifest { id, source, .. } => {
            let resource = id.path().as_str();
            LspProfileDiagnostic::new(
                LspProfileDiagnosticKind::CharacterManifestParse,
                format!("invalid character manifest `{resource}`: {source}"),
            )
            .with_resource(resource)
        }
        Error::AdapterManifest { id, source, .. } => {
            let resource = id.path().as_str();
            LspProfileDiagnostic::new(
                LspProfileDiagnosticKind::AdapterManifestParse,
                format!("invalid adapter manifest `{resource}`: {source}"),
            )
            .with_resource(resource)
        }
        Error::RustMetadata { id, source, .. } => {
            let resource = id.path().as_str();
            LspProfileDiagnostic::new(
                LspProfileDiagnosticKind::RustMetadataParse,
                format!("invalid Rust metadata `{resource}`: {source}"),
            )
            .with_resource(resource)
        }
        Error::ProjectManifest { .. } | Error::LaunchManifest { .. } => {
            LspProfileDiagnostic::new(LspProfileDiagnosticKind::ManifestParse, error.to_string())
        }
        Error::ProfileSelection { .. } | Error::AdapterSelection { .. } => {
            LspProfileDiagnostic::new(LspProfileDiagnosticKind::ProfileResolve, error.to_string())
        }
        _ => LspProfileDiagnostic::new(
            LspProfileDiagnosticKind::CharacterCatalog,
            format!("exact profile topology was rejected ({:?})", error.code()),
        ),
    }
}

impl fmt::Display for LspProfileDiagnosticKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}
