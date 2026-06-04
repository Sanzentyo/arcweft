use arcweft_adapter_context::manifest::AdapterManifest;
use arcweft_runtime_host::RuntimeHostRunnerKind;
use arcweft_verify_lsp::{ArcweftLspContext, ArcweftLspProfileContextBuilder};

/// LSP-visible profile facts resolved outside the Sans I/O helper crate.
#[derive(Clone, Debug)]
pub struct LspProfile {
    adapter: AdapterManifest,
    runner: RuntimeHostRunnerKind,
}

impl LspProfile {
    /// Creates a profile from adapter metadata and a runner preset.
    pub const fn new(adapter: AdapterManifest, runner: RuntimeHostRunnerKind) -> Self {
        Self { adapter, runner }
    }

    /// Minimal built-in profile used before project metadata is loaded.
    pub fn default_for_runner(runner: RuntimeHostRunnerKind) -> Self {
        Self {
            adapter: AdapterManifest::new("arcweft-default", "Arcweft Default"),
            runner,
        }
    }

    /// Adapter manifest selected for this profile.
    pub const fn adapter(&self) -> &AdapterManifest {
        &self.adapter
    }

    /// Runtime runner selected for this profile.
    pub const fn runner(&self) -> RuntimeHostRunnerKind {
        self.runner
    }

    /// Builds a Sans I/O LSP context for helper calls.
    pub fn context(&self) -> ArcweftLspContext<'_> {
        ArcweftLspProfileContextBuilder::new(&self.adapter)
            .with_runner_kind(self.runner)
            .build()
    }
}
