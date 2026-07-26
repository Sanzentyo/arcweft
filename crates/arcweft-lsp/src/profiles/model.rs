use super::{
    diagnostic::LspProfileDiagnostic,
    state::{AcceptedProfileEnvironment, LspProfileState},
    uri::file_uri_from_path,
};
use arcweft_adapter_context::{manifest::AdapterManifest, standard};
use arcweft_adapter_sema::registration::AdapterSemanticRegistration;
use arcweft_character::catalog::CharacterCatalog;
use arcweft_lang_sema::env::TypeCheckEnv;
use arcweft_launch::resolve::ResolvedLaunchProfile;
use arcweft_runtime_host::RuntimeHostRunnerKind;
use arcweft_source::{SourceDocument, SourceDocumentIdentity};
use arcweft_verify_lsp::{ArcweftLspContext, ArcweftLspProfileContextBuilder};
use lsp_types::Uri;
use std::{
    ops::Range,
    path::{Path, PathBuf},
    sync::Arc,
};

/// LSP-visible profile facts resolved outside the Sans I/O helper crate.
#[derive(Clone, Debug)]
pub struct LspProfile {
    pub(super) adapter: AdapterManifest,
    pub(super) declared_manifests: Vec<AdapterManifest>,
    pub(super) runner: RuntimeHostRunnerKind,
    pub(super) entry_selection: Option<ProfileSourceSelection>,
    pub(super) entry_selections: Vec<(String, ProfileSourceSelection)>,
    pub(super) characters: CharacterCatalog,
    pub(super) resolved_profile: Option<ResolvedLaunchProfile>,
    pub(super) state: Arc<LspProfileState>,
    pub(super) diagnostics: Vec<LspProfileDiagnostic>,
    pub(super) arbitrary_expression_type_inlays: bool,
}

/// Source location of a launch-profile-selected setting.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProfileSourceSelection {
    pub(super) path: PathBuf,
    pub(super) document: Arc<SourceDocument>,
    pub(super) value_range: Range<usize>,
}

impl LspProfile {
    /// Creates a profile from adapter metadata and a runner preset.
    pub fn new(adapter: AdapterManifest, runner: RuntimeHostRunnerKind) -> Self {
        Self {
            adapter,
            declared_manifests: Vec::new(),
            runner,
            entry_selection: None,
            entry_selections: Vec::new(),
            characters: CharacterCatalog::default(),
            resolved_profile: None,
            state: Arc::new(LspProfileState::new()),
            diagnostics: Vec::new(),
            arbitrary_expression_type_inlays: false,
        }
    }

    /// Minimal built-in profile used before project metadata is loaded.
    pub fn default_for_runner(runner: RuntimeHostRunnerKind) -> Self {
        Self {
            adapter: standard::sans_io_manifest(),
            declared_manifests: Vec::new(),
            runner,
            entry_selection: None,
            entry_selections: Vec::new(),
            characters: CharacterCatalog::default(),
            resolved_profile: None,
            state: Arc::new(LspProfileState::new()),
            diagnostics: Vec::new(),
            arbitrary_expression_type_inlays: false,
        }
    }

    /// Enables or disables expression-level type inlays for this profile.
    #[must_use]
    pub const fn with_arbitrary_expression_type_inlays(mut self, enabled: bool) -> Self {
        self.arbitrary_expression_type_inlays = enabled;
        self
    }

    /// Adapter manifest selected for this profile.
    pub const fn adapter(&self) -> &AdapterManifest {
        &self.adapter
    }

    /// Runtime runner selected for this profile.
    pub const fn runner(&self) -> RuntimeHostRunnerKind {
        self.runner
    }

    /// Adapter manifests declared by the selected profile.
    pub fn declared_manifests(&self) -> &[AdapterManifest] {
        &self.declared_manifests
    }

    /// Profile-loading diagnostics that should be surfaced in the editor.
    pub fn diagnostics(&self) -> &[LspProfileDiagnostic] {
        &self.diagnostics
    }

    /// Character manifests selected by the active launch profile.
    pub const fn characters(&self) -> &CharacterCatalog {
        &self.characters
    }

    /// Source-backed launch profile used to construct registration facts.
    pub const fn resolved_profile(&self) -> Option<&ResolvedLaunchProfile> {
        self.resolved_profile.as_ref()
    }

    /// Current atomically accepted semantic environment, if project registration succeeded.
    pub fn accepted_environment(&self) -> Option<Arc<AcceptedProfileEnvironment>> {
        self.state.current()
    }

    /// Shared accepted-environment state used by profile rebuilds.
    pub fn state(&self) -> &Arc<LspProfileState> {
        &self.state
    }

    /// Source location of the launch profile's canonical `entry` value.
    pub fn entry_selection(&self) -> Option<&ProfileSourceSelection> {
        self.entry_selection.as_ref()
    }

    /// Every source-backed profile `entry` token in the current manifest.
    pub fn entry_selections(&self) -> &[(String, ProfileSourceSelection)] {
        &self.entry_selections
    }

    /// Whether expression-level type inlays are enabled for this profile.
    pub const fn arbitrary_expression_type_inlays(&self) -> bool {
        self.arbitrary_expression_type_inlays
    }

    /// Builds a Sans I/O LSP context for helper calls.
    pub fn context(&self) -> ArcweftLspContext<'_> {
        ArcweftLspProfileContextBuilder::new(&self.adapter)
            .with_runner_kind(self.runner)
            .build()
    }

    /// Builds the semantic environment selected by this profile.
    pub fn typecheck_env(&self) -> TypeCheckEnv {
        AdapterSemanticRegistration::new(&self.adapter).declare_effects(TypeCheckEnv::standard())
    }

    pub(crate) fn replace_diagnostics(&mut self, diagnostic: LspProfileDiagnostic) {
        self.diagnostics.clear();
        self.diagnostics.push(diagnostic);
    }
}

impl ProfileSourceSelection {
    /// Manifest path containing the selected setting.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Manifest source text used to compute `value_range`.
    pub fn source(&self) -> &str {
        self.document.text()
    }

    /// Exact manifest revision that owns this token.
    pub fn source_identity(&self) -> &SourceDocumentIdentity {
        self.document.identity()
    }

    /// Byte range of the selected value inside `source`.
    pub fn value_range(&self) -> Range<usize> {
        self.value_range.clone()
    }

    /// File URI for the manifest source.
    pub fn uri(&self) -> Option<Uri> {
        file_uri_from_path(&self.path)
    }
}
