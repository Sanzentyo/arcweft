use arcweft_runtime_host::RuntimeHostRunnerKind;

/// Static server configuration supplied by the embedding process.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LspConfig {
    runner: RuntimeHostRunnerKind,
    profile_id: Option<String>,
}

impl Default for LspConfig {
    fn default() -> Self {
        Self {
            runner: RuntimeHostRunnerKind::Native,
            profile_id: None,
        }
    }
}

impl LspConfig {
    /// Creates a config for one runtime-host runner preset.
    pub fn new(runner: RuntimeHostRunnerKind) -> Self {
        Self {
            runner,
            profile_id: None,
        }
    }

    /// Selects one launch profile id when project manifests contain several profiles.
    #[must_use]
    pub fn with_profile_id(mut self, profile_id: impl Into<String>) -> Self {
        self.profile_id = Some(profile_id.into());
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
}
