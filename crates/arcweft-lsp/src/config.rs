use arcweft_runtime_host::RuntimeHostRunnerKind;

/// Static server configuration supplied by the embedding process.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LspConfig {
    runner: RuntimeHostRunnerKind,
    profile_id: Option<String>,
    arbitrary_expression_type_inlays: bool,
}

impl Default for LspConfig {
    fn default() -> Self {
        Self {
            runner: RuntimeHostRunnerKind::Native,
            profile_id: None,
            arbitrary_expression_type_inlays: false,
        }
    }
}

impl LspConfig {
    /// Creates a config for one runtime-host runner preset.
    pub fn new(runner: RuntimeHostRunnerKind) -> Self {
        Self {
            runner,
            profile_id: None,
            arbitrary_expression_type_inlays: false,
        }
    }

    /// Selects one launch profile id when project manifests contain several profiles.
    #[must_use]
    pub fn with_profile_id(mut self, profile_id: impl Into<String>) -> Self {
        self.profile_id = Some(profile_id.into());
        self
    }

    /// Enables expression type inlays outside let-binding sites.
    #[must_use]
    pub const fn with_arbitrary_expression_type_inlays(mut self, enabled: bool) -> Self {
        self.arbitrary_expression_type_inlays = enabled;
        self
    }

    /// Runtime-host runner preset used for profile diagnostics and completion.
    pub const fn runner(&self) -> RuntimeHostRunnerKind {
        self.runner
    }

    /// Optional launch profile id selected by the embedding process.
    pub fn profile_id(&self) -> Option<&str> {
        self.profile_id.as_deref()
    }

    /// Whether the editor explicitly requested expression-level type inlays.
    pub const fn arbitrary_expression_type_inlays(&self) -> bool {
        self.arbitrary_expression_type_inlays
    }
}
