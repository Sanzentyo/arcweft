use arcweft_compiler::project::ProjectCompileDiagnostic;
use arcweft_source::SourceSpan;
use std::{fmt, sync::Arc};
use thiserror::Error;

/// One profile metadata diagnostic with exact source provenance when available.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LspProfileDiagnostic {
    kind: LspProfileDiagnosticKind,
    message: String,
    profile_id: Option<String>,
    resource: Option<String>,
    source: Option<SourceSpan>,
    project_compile_diagnostics: Arc<[ProjectCompileDiagnostic]>,
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
    /// A declared resource extension-manifest could not be read.
    ResourceTypeManifestRead,
    /// A declared resource extension-manifest could not be decoded or published.
    ResourceTypeManifestParse,
    /// The selected profile could not be resolved.
    ProfileResolve,
    /// A complete profile candidate could not be atomically published.
    ProfilePublication,
    /// An Arcweft project source could not be read.
    ProjectSourceRead,
    /// An Arcweft project source could not be parsed or linked.
    ProjectSourceParse,
    /// The exact loaded project was rejected by a later compiler stage.
    ProjectCompile,
    /// Generated external-module metadata could not be read.
    ExternalModuleMetadataRead,
    /// Generated external-module metadata could not be decoded or admitted.
    ExternalModuleMetadataParse,
    /// A character manifest could not be read.
    CharacterManifestRead,
    /// A character manifest could not be parsed or validated.
    CharacterManifestParse,
    /// A character layer payload could not be read.
    CharacterLayerPayloadRead,
    /// A character layer payload could not be decoded or validated.
    CharacterLayerPayloadParse,
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
            project_compile_diagnostics: Arc::from([]),
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

    /// Retains the compiler-owned, source-backed rejection evidence without
    /// flattening it into a profile-level string.
    #[must_use]
    pub(crate) fn with_project_compile_diagnostics(
        mut self,
        diagnostics: impl IntoIterator<Item = ProjectCompileDiagnostic>,
    ) -> Self {
        self.project_compile_diagnostics = diagnostics.into_iter().collect::<Vec<_>>().into();
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

    /// Exact compiler diagnostics retained for document-specific projection.
    pub(crate) fn project_compile_diagnostics(&self) -> &[ProjectCompileDiagnostic] {
        &self.project_compile_diagnostics
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
            Self::ResourceTypeManifestRead => "profile.resource_type_manifest.read",
            Self::ResourceTypeManifestParse => "profile.resource_type_manifest.parse",
            Self::ProfileResolve => "profile.resolve",
            Self::ProfilePublication => "profile.publication",
            Self::ProjectSourceRead => "profile.project_source.read",
            Self::ProjectSourceParse => "profile.project_source.parse",
            Self::ProjectCompile => "profile.project.compile",
            Self::ExternalModuleMetadataRead => "profile.external_module_metadata.read",
            Self::ExternalModuleMetadataParse => "profile.external_module_metadata.parse",
            Self::CharacterManifestRead => "profile.character_manifest.read",
            Self::CharacterManifestParse => "profile.character_manifest.parse",
            Self::CharacterLayerPayloadRead => "profile.character_layer_payload.read",
            Self::CharacterLayerPayloadParse => "profile.character_layer_payload.parse",
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
        super::environment::RegisterProfileEnvironmentError::Compile { details, source } => {
            LspProfileDiagnostic::new(
                LspProfileDiagnosticKind::ProjectCompile,
                format!("project compilation was rejected: {details}"),
            )
            .with_project_compile_diagnostics(source.diagnostics().iter().cloned())
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

    if error.resource_manifest_code().is_some() {
        return resource_type_manifest_topology_diagnostic(error);
    }

    match error {
        Error::ResourceRead { id, kind, .. } => {
            let resource = id.path().as_str();
            let diagnostic_kind =
                topology_resource_diagnostic_kind(kind, TopologyResourceFailure::Read);
            LspProfileDiagnostic::new(diagnostic_kind, format!("failed to read `{resource}`"))
                .with_resource(resource)
        }
        Error::ResourceUtf8 { id, kind, .. } => {
            let resource = id.path().as_str();
            let diagnostic_kind =
                topology_resource_diagnostic_kind(kind, TopologyResourceFailure::Parse);
            LspProfileDiagnostic::new(
                diagnostic_kind,
                format!("resource `{resource}` is not valid UTF-8"),
            )
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
        Error::ExternalModuleMetadataHash { import, id }
        | Error::ExternalModuleMetadataDecode { import, id, .. } => {
            let resource = id.path().as_str();
            LspProfileDiagnostic::new(
                LspProfileDiagnosticKind::ExternalModuleMetadataParse,
                format!("invalid generated metadata for import `{import}`"),
            )
            .with_resource(resource)
        }
        Error::ExternalModuleMetadataExpectation { import, .. }
        | Error::ExternalModuleFacts(
            arcweft_project_loader::topology::ExternalModuleFactsError::Symbol { import, .. }
            | arcweft_project_loader::topology::ExternalModuleFactsError::Callable { import, .. }
            | arcweft_project_loader::topology::ExternalModuleFactsError::TypeReference {
                import,
                ..
            }
            | arcweft_project_loader::topology::ExternalModuleFactsError::FunctionPurity {
                import,
                ..
            }
            | arcweft_project_loader::topology::ExternalModuleFactsError::ActivityExportMissing {
                import,
                ..
            }
            | arcweft_project_loader::topology::ExternalModuleFactsError::ActivityIdentityMismatch {
                import,
                ..
            },
        ) => LspProfileDiagnostic::new(
            LspProfileDiagnosticKind::ExternalModuleMetadataParse,
            format!("generated metadata import `{import}` was rejected"),
        )
        .with_resource(import.as_str()),
        Error::ExternalModuleFacts(
            arcweft_project_loader::topology::ExternalModuleFactsError::DuplicateMountedIdentity {
                identity,
            },
        ) => LspProfileDiagnostic::new(
            LspProfileDiagnosticKind::ExternalModuleMetadataParse,
            format!("generated mounted identity `{identity}` occurs more than once"),
        ),
        Error::Manifest { .. } => {
            LspProfileDiagnostic::new(LspProfileDiagnosticKind::ManifestParse, error.to_string())
        }
        Error::ModuleSyntax { id, .. } | Error::ModuleDeclaration { id, .. } => {
            let resource = id.path().as_str();
            LspProfileDiagnostic::new(
                LspProfileDiagnosticKind::ProjectSourceParse,
                error.to_string(),
            )
            .with_resource(resource)
        }
        Error::ModuleImport { path, .. } => LspProfileDiagnostic::new(
            LspProfileDiagnosticKind::ProjectSourceParse,
            error.to_string(),
        )
        .with_resource(path.display().to_string()),
        Error::ProfileSelection { .. } | Error::AdapterSelection { .. } => {
            LspProfileDiagnostic::new(LspProfileDiagnosticKind::ProfileResolve, error.to_string())
        }
        _ => LspProfileDiagnostic::new(
            LspProfileDiagnosticKind::CharacterCatalog,
            format!("exact profile topology was rejected ({:?})", error.code()),
        ),
    }
}

fn resource_type_manifest_topology_diagnostic(
    error: &arcweft_project_loader::topology::ProfileTopologyLoadError,
) -> LspProfileDiagnostic {
    use arcweft_project_loader::topology::ProfileTopologyLoadError as Error;

    let (resource, source) = match error {
        Error::ResourceTypeManifestUtf8 { id, .. }
        | Error::UnresolvedResourceTypePackage { id, .. } => (Some(id.path().as_str()), None),
        Error::ResourceTypeManifest { id, source, .. } => {
            (Some(id.path().as_str()), source.diagnostics().first())
        }
        Error::ResourceTypePublication { source } => (None, source.diagnostics().first()),
        _ => unreachable!("caller filters resource manifest topology diagnostics"),
    };
    let mut diagnostic = LspProfileDiagnostic::new(
        LspProfileDiagnosticKind::ResourceTypeManifestParse,
        error.to_string(),
    );
    if let Some(resource) = resource {
        diagnostic = diagnostic.with_resource(resource);
    }
    if let Some(source) = source {
        diagnostic = diagnostic.with_source(source.primary().clone());
    }
    diagnostic
}

#[derive(Clone, Copy)]
enum TopologyResourceFailure {
    Read,
    Parse,
}

fn topology_resource_diagnostic_kind(
    resource: &arcweft_project_loader::topology::ProfileTopologyResourceKind,
    failure: TopologyResourceFailure,
) -> LspProfileDiagnosticKind {
    use arcweft_project_loader::topology::ProfileTopologyResourceKind as Resource;

    match (resource, failure) {
        (Resource::Manifest, TopologyResourceFailure::Read) => {
            LspProfileDiagnosticKind::ManifestRead
        }
        (Resource::Manifest, TopologyResourceFailure::Parse) => {
            LspProfileDiagnosticKind::ManifestParse
        }
        (Resource::ResourceTypeManifest { .. }, TopologyResourceFailure::Read) => {
            LspProfileDiagnosticKind::ResourceTypeManifestRead
        }
        (Resource::ResourceTypeManifest { .. }, TopologyResourceFailure::Parse) => {
            LspProfileDiagnosticKind::ResourceTypeManifestParse
        }
        (Resource::ArcweftModule { .. }, TopologyResourceFailure::Read) => {
            LspProfileDiagnosticKind::ProjectSourceRead
        }
        (Resource::ArcweftModule { .. }, TopologyResourceFailure::Parse) => {
            LspProfileDiagnosticKind::ProjectSourceParse
        }
        (Resource::CharacterPackageManifest { .. }, TopologyResourceFailure::Read) => {
            LspProfileDiagnosticKind::CharacterManifestRead
        }
        (Resource::CharacterPackageManifest { .. }, TopologyResourceFailure::Parse) => {
            LspProfileDiagnosticKind::CharacterManifestParse
        }
        (Resource::CharacterLayerPayload { .. }, TopologyResourceFailure::Read) => {
            LspProfileDiagnosticKind::CharacterLayerPayloadRead
        }
        (Resource::CharacterLayerPayload { .. }, TopologyResourceFailure::Parse) => {
            LspProfileDiagnosticKind::CharacterLayerPayloadParse
        }
        (Resource::ExternalModuleMetadata { .. }, TopologyResourceFailure::Read) => {
            LspProfileDiagnosticKind::ExternalModuleMetadataRead
        }
        (Resource::ExternalModuleMetadata { .. }, TopologyResourceFailure::Parse) => {
            LspProfileDiagnosticKind::ExternalModuleMetadataParse
        }
    }
}

impl fmt::Display for LspProfileDiagnosticKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}
