use arcweft_runtime_host::RuntimeHostRunnerKind;

/// Static server configuration supplied by the embedding process.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LspConfig {
    runner: RuntimeHostRunnerKind,
}

impl Default for LspConfig {
    fn default() -> Self {
        Self {
            runner: RuntimeHostRunnerKind::Native,
        }
    }
}

impl LspConfig {
    /// Creates a config for one runtime-host runner preset.
    pub const fn new(runner: RuntimeHostRunnerKind) -> Self {
        Self { runner }
    }

    /// Runtime-host runner preset used for profile diagnostics and completion.
    pub const fn runner(self) -> RuntimeHostRunnerKind {
        self.runner
    }
}
