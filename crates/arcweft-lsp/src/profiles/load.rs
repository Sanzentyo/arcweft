use super::{
    diagnostic::{LspProfileDiagnostic, LspProfileDiagnosticKind, LspProfileLoadError},
    environment::register_profile_environment,
    model::{LspProfile, ProfileSourceSelection},
    state::LspProfileState,
    uri::file_path_from_uri,
};
use arcweft_launch::{
    LaunchKeyPath, LaunchManifestSourceMap, LaunchProfileSelection, LaunchTokenPath,
};
use arcweft_project_loader::topology::LoadedProfileTopology;
use arcweft_runtime_host::RuntimeHostRunnerKind;
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

const DEFAULT_MANIFEST_NAME: &str = "arcw.toml";

/// Resolves LSP profile metadata from project manifests near opened documents.
#[derive(Clone, Debug)]
pub struct LspProfileResolver {
    runner: RuntimeHostRunnerKind,
    manifest_name: String,
    profile_id: Option<String>,
    arbitrary_expression_type_inlays: bool,
}

pub(crate) fn apply_registered_topology(
    profile: &mut LspProfile,
    topology: &LoadedProfileTopology,
    characters: arcweft_character::catalog::CharacterCatalog,
) {
    let selected = topology.selected_profile();
    let manifest = topology.loaded_project().manifest_document();
    profile.adapter = topology.adapter().clone();
    profile.declared_manifests = topology.registration_adapter_manifests().to_vec();
    profile.dialogue_defaults = selected.dialogue_defaults().map(str::to_owned);
    profile.dialogue_defaults_selection = selected.dialogue_defaults().and_then(|_| {
        dialogue_defaults_selection(
            topology.loaded_project().sources().manifest_path(),
            manifest.text(),
            selected.id().as_str(),
            topology.loaded_project().launch().source_map(),
        )
    });
    profile.characters = characters;
    profile.resolved_profile = Some(selected.clone());
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
        let accepted = state.current();
        let previous_profile = accepted
            .as_ref()
            .map(|environment| environment.profile().profile_id().as_str().to_owned());
        let selection = self.profile_id.as_deref().map_or(
            LaunchProfileSelection::Automatic {
                previous: previous_profile.as_deref(),
            },
            LaunchProfileSelection::Explicit,
        );
        let previous = accepted
            .as_ref()
            .map(|environment| environment.world().environment());
        let registered = register_profile_environment(
            &manifest_path,
            selection,
            &[],
            super::state::AcceptedOverlaySet::default(),
            previous,
        )
        .map_err(|error| LspProfileLoadError::Environment {
            profile_id: self.profile_id.clone(),
            source: Box::new(error),
        })?;
        Ok(self.profile_from_registered(registered, state))
    }

    fn find_manifest(&self, document_path: &Path) -> Option<PathBuf> {
        let start = document_path.parent().unwrap_or(document_path);
        start
            .ancestors()
            .map(|ancestor| ancestor.join(&self.manifest_name))
            .find(|candidate| candidate.is_file())
    }

    fn profile_from_registered(
        &self,
        registered: super::environment::RegisteredProfileCandidate,
        state: Arc<LspProfileState>,
    ) -> LspProfile {
        let (candidate, characters, topology) = registered.into_parts();
        state
            .replace_accepted(candidate)
            .expect("a fresh active profile state accepts generation one");
        let profile = topology.selected_profile();
        let manifest = topology.loaded_project().manifest_document();
        LspProfile {
            adapter: topology.adapter().clone(),
            declared_manifests: topology.registration_adapter_manifests().to_vec(),
            runner: self.runner,
            dialogue_defaults: profile.dialogue_defaults().map(str::to_owned),
            dialogue_defaults_selection: profile.dialogue_defaults().and_then(|_| {
                dialogue_defaults_selection(
                    topology.loaded_project().sources().manifest_path(),
                    manifest.text(),
                    profile.id().as_str(),
                    topology.loaded_project().launch().source_map(),
                )
            }),
            characters,
            resolved_profile: Some(profile.clone()),
            state,
            diagnostics: Vec::new(),
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
