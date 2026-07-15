use super::{
    cache::LspProfileState,
    diagnostic::{LspProfileDiagnostic, LspProfileDiagnosticKind, LspProfileLoadError},
    environment::register_profile_environment,
    model::{LspProfile, ProfileSourceSelection},
    uri::{file_path_from_uri, file_uri_from_path},
};
use arcweft_adapter_context::{
    manifest::AdapterRegistry,
    standard::{self, SANS_IO_ADAPTER_ID},
};
use arcweft_character::catalog::CharacterCatalog;
use arcweft_lang_sema::env::TypeCheckEnv;
use arcweft_launch::{
    LaunchKeyPath, LaunchManifestSourceMap, LaunchProfileError, LaunchTokenPath,
    ResolvedLaunchProfile, SourceBackedLaunchManifest,
};
use arcweft_runtime_host::RuntimeHostRunnerKind;
use arcweft_rust_abi::ArcweftRustManifest;
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName, SourceSpan};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

const DEFAULT_MANIFEST_NAME: &str = "arcw.toml";

pub(super) struct SourceBackedProfileResource {
    path: PathBuf,
    source: Option<SourceSpan>,
}

impl SourceBackedProfileResource {
    pub(super) fn new(path: PathBuf, source: Option<SourceSpan>) -> Self {
        Self { path, source }
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn bind(&self, diagnostic: LspProfileDiagnostic) -> LspProfileDiagnostic {
        match &self.source {
            Some(source) => diagnostic.with_source(source.clone()),
            None => diagnostic,
        }
    }
}

pub(super) fn read_adapter_manifests(
    resources: &[SourceBackedProfileResource],
    manifest_dir: &Path,
    profile_id: &str,
    registry: AdapterRegistry,
    diagnostics: &mut Vec<LspProfileDiagnostic>,
) -> AdapterRegistry {
    resources.iter().fold(registry, |registry, resource| {
        match arcweft_project_loader::adapter_manifest::load(resource.path()) {
            Ok(manifest) => registry.with_manifest(manifest),
            Err(error) => {
                diagnostics.push(resource.bind(adapter_manifest_diagnostic(
                    &error,
                    path_label(resource.path(), manifest_dir),
                    profile_id,
                )));
                registry
            }
        }
    })
}

pub(super) fn read_rust_metadata(
    resources: &[SourceBackedProfileResource],
    manifest_dir: &Path,
    profile_id: &str,
    diagnostics: &mut Vec<LspProfileDiagnostic>,
) -> Vec<ArcweftRustManifest> {
    resources
        .iter()
        .filter_map(
            |resource| match arcweft_project_loader::rust_metadata::load(resource.path()) {
                Ok(manifest) => Some(manifest),
                Err(error) => {
                    diagnostics.push(resource.bind(rust_metadata_diagnostic(
                        &error,
                        path_label(resource.path(), manifest_dir),
                        profile_id,
                    )));
                    None
                }
            },
        )
        .collect()
}

pub(super) fn read_character_manifests(
    resources: &[SourceBackedProfileResource],
    manifest_dir: &Path,
    profile_id: &str,
    diagnostics: &mut Vec<LspProfileDiagnostic>,
) -> CharacterCatalog {
    let mut manifests = Vec::new();
    for source_backed in resources {
        let resource = path_label(source_backed.path(), manifest_dir);
        match arcweft_project_loader::character_manifest::load(source_backed.path()) {
            Ok(manifest) => {
                manifests.push(manifest.manifest().manifest().clone());
            }
            Err(error) => {
                diagnostics.push(
                    source_backed.bind(character_manifest_diagnostic(&error, resource, profile_id)),
                );
            }
        }
    }
    CharacterCatalog::try_from_manifests(manifests).unwrap_or_else(|error| {
        diagnostics.push(
            LspProfileDiagnostic::new(
                LspProfileDiagnosticKind::CharacterCatalog,
                error.to_string(),
            )
            .with_profile_id(profile_id),
        );
        CharacterCatalog::default()
    })
}

fn character_manifest_diagnostic(
    error: &arcweft_project_loader::character_manifest::LoadError,
    resource: String,
    profile_id: &str,
) -> LspProfileDiagnostic {
    let kind = match error {
        arcweft_project_loader::character_manifest::LoadError::Read(_) => {
            LspProfileDiagnosticKind::CharacterManifestRead
        }
        arcweft_project_loader::character_manifest::LoadError::Parse(_)
        | arcweft_project_loader::character_manifest::LoadError::DocumentId(_)
        | arcweft_project_loader::character_manifest::LoadError::Document(_)
        | arcweft_project_loader::character_manifest::LoadError::ProjectDocument(_) => {
            LspProfileDiagnosticKind::CharacterManifestParse
        }
    };
    LspProfileDiagnostic::new(kind, format!("{error} `{resource}`"))
        .with_profile_id(profile_id)
        .with_resource(resource)
}

fn adapter_manifest_diagnostic(
    error: &arcweft_project_loader::adapter_manifest::LoadError,
    resource: String,
    profile_id: &str,
) -> LspProfileDiagnostic {
    let kind = match error {
        arcweft_project_loader::adapter_manifest::LoadError::Read(_) => {
            LspProfileDiagnosticKind::AdapterManifestRead
        }
        arcweft_project_loader::adapter_manifest::LoadError::Parse(_) => {
            LspProfileDiagnosticKind::AdapterManifestParse
        }
    };
    LspProfileDiagnostic::new(kind, format!("{error} `{resource}`"))
        .with_profile_id(profile_id)
        .with_resource(resource)
}

fn rust_metadata_diagnostic(
    error: &arcweft_project_loader::rust_metadata::LoadError,
    resource: String,
    profile_id: &str,
) -> LspProfileDiagnostic {
    let kind = match error {
        arcweft_project_loader::rust_metadata::LoadError::Read(_) => {
            LspProfileDiagnosticKind::RustMetadataRead
        }
        arcweft_project_loader::rust_metadata::LoadError::Parse(_) => {
            LspProfileDiagnosticKind::RustMetadataParse
        }
    };
    LspProfileDiagnostic::new(kind, format!("{error} `{resource}`"))
        .with_profile_id(profile_id)
        .with_resource(resource)
}

fn path_label(path: &Path, manifest_dir: &Path) -> String {
    let display_path = path.strip_prefix(manifest_dir).unwrap_or(path);
    let components = display_path
        .components()
        .filter_map(|component| match component {
            std::path::Component::Normal(value) => Some(value.to_string_lossy()),
            _ => None,
        })
        .collect::<Vec<_>>();
    if components.is_empty() {
        path.file_name().map_or_else(
            || "metadata".to_owned(),
            |name| name.to_string_lossy().into_owned(),
        )
    } else {
        components.join("/")
    }
}

/// Resolves LSP profile metadata from project manifests near opened documents.
#[derive(Clone, Debug)]
pub struct LspProfileResolver {
    runner: RuntimeHostRunnerKind,
    manifest_name: String,
    profile_id: Option<String>,
    arbitrary_expression_type_inlays: bool,
}

impl LspProfileResolver {
    /// Creates a resolver for one runner preset and optional explicit profile id.
    pub fn new(runner: RuntimeHostRunnerKind, profile_id: Option<String>) -> Self {
        Self {
            runner,
            manifest_name: DEFAULT_MANIFEST_NAME.to_owned(),
            profile_id,
            arbitrary_expression_type_inlays: false,
        }
    }

    /// Carries editor-selected inlay policy into every resolved profile.
    #[must_use]
    pub const fn with_arbitrary_expression_type_inlays(mut self, enabled: bool) -> Self {
        self.arbitrary_expression_type_inlays = enabled;
        self
    }

    /// Minimal built-in profile used when no document-specific metadata is cached.
    pub fn default_profile(&self) -> LspProfile {
        LspProfile::default_for_runner(self.runner)
            .with_arbitrary_expression_type_inlays(self.arbitrary_expression_type_inlays)
    }

    /// Resolves a profile for one LSP document URI.
    pub fn resolve_for_uri(&self, uri: &lsp_types::Uri) -> LspProfile {
        self.resolve_for_uri_with_state(uri, Arc::new(LspProfileState::new()))
    }

    pub(crate) fn resolve_for_uri_with_state(
        &self,
        uri: &lsp_types::Uri,
        state: Arc<LspProfileState>,
    ) -> LspProfile {
        match file_path_from_uri(uri) {
            Some(path) => self.resolve_for_document_path_with_state(&path, state),
            None => self.default_with_diagnostic_and_state(
                LspProfileDiagnostic::new(
                    LspProfileDiagnosticKind::NonFileDocumentUri,
                    LspProfileLoadError::NonFileDocumentUri.to_string(),
                ),
                state,
            ),
        }
    }

    /// Resolves a profile for one local document path.
    pub fn resolve_for_document_path(&self, document_path: &Path) -> LspProfile {
        self.resolve_for_document_path_with_state(document_path, Arc::new(LspProfileState::new()))
    }

    pub(super) fn resolve_for_document_path_with_state(
        &self,
        document_path: &Path,
        state: Arc<LspProfileState>,
    ) -> LspProfile {
        self.try_resolve_for_document_path(document_path, Arc::clone(&state))
            .unwrap_or_else(|error| {
                self.default_with_diagnostic_and_state(error.into_diagnostic(), state)
            })
    }

    fn try_resolve_for_document_path(
        &self,
        document_path: &Path,
        state: Arc<LspProfileState>,
    ) -> Result<LspProfile, LspProfileLoadError> {
        let manifest_path = self
            .find_manifest(document_path)
            .ok_or(LspProfileLoadError::WorkspaceManifestNotFound)?;
        let manifest_dir = manifest_path.parent().unwrap_or_else(|| Path::new("."));
        let source =
            fs::read_to_string(&manifest_path).map_err(LspProfileLoadError::ManifestRead)?;
        let document_id = file_uri_from_path(&manifest_path).map_or_else(
            || format!("arcweft-lsp://{}", manifest_path.display()),
            |uri| uri.as_str().to_owned(),
        );
        let document_id = SourceDocumentId::try_new(document_id)
            .map_err(|error| LspProfileLoadError::ManifestSource(error.to_string()))?;
        let document = SourceDocument::try_new(
            document_id,
            SourceName::path(manifest_path.display().to_string()),
            source.clone(),
        )
        .map_err(|error| LspProfileLoadError::ManifestSource(error.to_string()))?;
        let sourced_manifest = SourceBackedLaunchManifest::parse_document(&document)
            .map_err(LspProfileLoadError::ManifestParse)?;
        let manifest = sourced_manifest.manifest();
        let profile_id = self
            .profile_id
            .as_deref()
            .or_else(|| manifest.profiles().keys().next().map(String::as_str))
            .ok_or_else(|| {
                LspProfileLoadError::ProfileResolve(LaunchProfileError::MissingProfile(
                    "<default>".to_owned(),
                ))
            })?;
        let standard_registry = standard::standard_registry();
        let profile = manifest
            .resolve_profile_with_adapters(
                profile_id,
                manifest_dir,
                &standard_registry.adapter_ids(),
            )
            .map_err(LspProfileLoadError::ProfileResolve)?;
        Ok(self.profile_from_resolved(
            &profile,
            manifest_dir,
            &manifest_path,
            &source,
            profile_id,
            standard_registry,
            state,
            sourced_manifest.source_map(),
        ))
    }

    fn find_manifest(&self, document_path: &Path) -> Option<PathBuf> {
        let start = document_path.parent().unwrap_or(document_path);
        start
            .ancestors()
            .map(|ancestor| ancestor.join(&self.manifest_name))
            .find(|candidate| candidate.is_file())
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "profile construction keeps the resolved launch, exact manifest provenance, registry, and publication state explicit"
    )]
    fn profile_from_resolved(
        &self,
        profile: &ResolvedLaunchProfile,
        manifest_dir: &Path,
        manifest_path: &Path,
        manifest_source: &str,
        profile_id: &str,
        standard_registry: AdapterRegistry,
        state: Arc<LspProfileState>,
        source_map: &LaunchManifestSourceMap,
    ) -> LspProfile {
        let mut diagnostics = Vec::new();
        let adapter_resources = profile_resources(
            profile.adapter_manifests(),
            source_map,
            profile_id,
            "adapter_manifests",
        );
        let registry = read_adapter_manifests(
            &adapter_resources,
            manifest_dir,
            profile_id,
            standard_registry,
            &mut diagnostics,
        );
        let mut adapter = registry
            .get(profile.adapter().unwrap_or(SANS_IO_ADAPTER_ID))
            .cloned()
            .unwrap_or_else(standard::sans_io_manifest);
        let rust_resources = profile_resources(
            profile.rust_metadata(),
            source_map,
            profile_id,
            "rust_metadata",
        );
        for rust_manifest in
            read_rust_metadata(&rust_resources, manifest_dir, profile_id, &mut diagnostics)
        {
            adapter = adapter.with_rust_manifest(&rust_manifest);
        }
        let character_resources = profile_resources(
            profile.character_manifests(),
            source_map,
            profile_id,
            "character_manifests",
        );
        let characters = read_character_manifests(
            &character_resources,
            manifest_dir,
            profile_id,
            &mut diagnostics,
        );
        let base = adapter.apply_to_env(TypeCheckEnv::standard());
        let accepted = state.current();
        let previous = accepted
            .as_ref()
            .map(|environment| environment.world().environment());
        match register_profile_environment(manifest_path, profile, &adapter, base, previous) {
            Ok(world) => {
                state
                    .replace_accepted(world)
                    .expect("a fresh active profile state accepts generation one");
            }
            Err(error) => diagnostics.push(
                LspProfileDiagnostic::new(LspProfileDiagnosticKind::CharacterCatalog, error)
                    .with_profile_id(profile_id),
            ),
        }
        let declared_manifests = vec![adapter.clone()];
        LspProfile {
            adapter,
            declared_manifests,
            runner: self.runner,
            dialogue_defaults: profile.dialogue_defaults().map(str::to_owned),
            dialogue_defaults_selection: profile.dialogue_defaults().and_then(|_| {
                dialogue_defaults_selection(manifest_path, manifest_source, profile_id, source_map)
            }),
            characters,
            resolved_profile: Some(profile.clone()),
            state,
            diagnostics,
            arbitrary_expression_type_inlays: self.arbitrary_expression_type_inlays,
        }
    }

    fn default_with_diagnostic_and_state(
        &self,
        diagnostic: LspProfileDiagnostic,
        state: Arc<LspProfileState>,
    ) -> LspProfile {
        let mut profile = LspProfile::default_for_runner(self.runner);
        profile.state = state;
        profile.diagnostics.push(diagnostic);
        profile.with_arbitrary_expression_type_inlays(self.arbitrary_expression_type_inlays)
    }
}

fn dialogue_defaults_selection(
    manifest_path: &Path,
    source: &str,
    profile_id: &str,
    source_map: &LaunchManifestSourceMap,
) -> Option<ProfileSourceSelection> {
    let value_range = source_map
        .token(&LaunchTokenPath::Key {
            path: LaunchKeyPath::new(vec![
                "profiles".to_owned(),
                profile_id.to_owned(),
                "dialogue_defaults".to_owned(),
            ]),
            occurrence: 0,
        })?
        .string_content()?
        .range()
        .as_range();
    Some(ProfileSourceSelection {
        path: manifest_path.to_path_buf(),
        source: source.to_owned(),
        value_range,
    })
}

fn profile_resources(
    paths: &[PathBuf],
    source_map: &LaunchManifestSourceMap,
    profile_id: &str,
    key: &str,
) -> Vec<SourceBackedProfileResource> {
    let path = LaunchKeyPath::new(vec![
        "profiles".to_owned(),
        profile_id.to_owned(),
        key.to_owned(),
    ]);
    paths
        .iter()
        .enumerate()
        .map(|(index, resource)| {
            let source = source_map
                .token(&LaunchTokenPath::ArrayElement {
                    path: path.clone(),
                    occurrence: 0,
                    index,
                })
                .and_then(|token| token.value())
                .cloned();
            SourceBackedProfileResource::new(resource.clone(), source)
        })
        .collect()
}
