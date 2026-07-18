use crate::native_task::internal_scheduler_manifest;
use arcweft_adapter_context::{
    manifest::{AdapterHostCallId, AdapterManifest},
    standard,
};
use std::collections::BTreeSet;

/// Runtime host preset selected by an embedding runner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeHostRunnerKind {
    /// Native CLI or native player runner.
    Native,
    /// Browser/Wasm runner without native filesystem access.
    BrowserWeb,
    /// Fully caller-supplied runner.
    Custom,
}

/// Runtime-host capabilities supplied by an embedding runner.
///
/// Adapter manifests describe the Arcweft-visible surface. This type describes
/// the concrete host calls that the selected native or web runner can actually
/// complete, so tooling can report a profile that type-checks but cannot run
/// with the selected host.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RuntimeHostCapabilities {
    host_calls: BTreeSet<AdapterHostCallId>,
}

/// Conformance report comparing adapter declarations with runner capabilities.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RuntimeHostConformanceReport {
    diagnostics: Vec<RuntimeHostConformanceDiagnostic>,
}

/// One runtime-host conformance issue.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeHostConformanceDiagnostic {
    kind: RuntimeHostConformanceDiagnosticKind,
    adapter_id: String,
    host_call: AdapterHostCallId,
}

/// Runtime-host conformance diagnostic kind.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeHostConformanceDiagnosticKind {
    /// Adapter manifest declares a host call not implemented by the selected runner.
    MissingHostCallImplementation,
}

impl RuntimeHostRunnerKind {
    /// Preset capabilities for this runner kind before adding custom adapters.
    pub fn capabilities(self) -> RuntimeHostCapabilities {
        match self {
            Self::Native => RuntimeHostCapabilities::standard_native(),
            Self::BrowserWeb => RuntimeHostCapabilities::browser_web(),
            Self::Custom => RuntimeHostCapabilities::new(),
        }
    }
}

impl RuntimeHostCapabilities {
    /// Creates an empty runtime-host capability set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a capability set from stable host-call ids.
    pub fn from_host_call_ids(ids: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            host_calls: ids
                .into_iter()
                .map(AdapterHostCallId::new)
                .collect::<BTreeSet<_>>(),
        }
    }

    /// Creates a capability set from adapter manifests implemented by a runner.
    pub fn from_adapter_manifests<'a>(
        manifests: impl IntoIterator<Item = &'a AdapterManifest>,
    ) -> Self {
        Self::from_host_call_ids(
            manifests
                .into_iter()
                .flat_map(AdapterManifest::host_calls)
                .map(|host_call| host_call.id().to_owned()),
        )
    }

    /// Native CLI/player runner capabilities implemented by `arcweft-runtime-host`.
    pub fn standard_native() -> Self {
        let manifests = [
            standard::native_file_manifest(),
            standard::system_info_manifest(),
            internal_scheduler_manifest(),
        ];
        Self::from_adapter_manifests(&manifests)
    }

    /// Browser runner capabilities for a web embedding without native filesystem access.
    ///
    /// Browser WebGPU/math acceleration is not represented as a host task here;
    /// web math backends should add their own implemented adapter manifests when
    /// they expose host-call surfaces.
    pub fn browser_web() -> Self {
        let manifests = [
            standard::system_info_manifest(),
            internal_scheduler_manifest(),
        ];
        Self::from_adapter_manifests(&manifests)
    }

    /// Returns a new capability set with host calls from one implemented manifest.
    #[must_use]
    pub fn with_adapter_manifest(mut self, manifest: &AdapterManifest) -> Self {
        self.host_calls.extend(
            manifest
                .host_calls()
                .iter()
                .map(|host_call| AdapterHostCallId::new(host_call.id())),
        );
        self
    }

    /// Returns a new capability set with host calls from implemented manifests.
    #[must_use]
    pub fn with_adapter_manifests<'a>(
        mut self,
        manifests: impl IntoIterator<Item = &'a AdapterManifest>,
    ) -> Self {
        self.host_calls.extend(
            manifests
                .into_iter()
                .flat_map(AdapterManifest::host_calls)
                .map(|host_call| AdapterHostCallId::new(host_call.id())),
        );
        self
    }

    /// Returns true when the runtime host implements this call id.
    pub fn has_host_call(&self, id: &AdapterHostCallId) -> bool {
        self.host_calls.contains(id)
    }

    /// Checks one adapter manifest against the selected runner capabilities.
    pub fn check_adapter_manifest(
        &self,
        manifest: &AdapterManifest,
    ) -> RuntimeHostConformanceReport {
        RuntimeHostConformanceReport {
            diagnostics: manifest
                .host_calls()
                .iter()
                .filter_map(|host_call| {
                    let id = AdapterHostCallId::new(host_call.id());
                    (!self.host_calls.contains(&id)).then(|| RuntimeHostConformanceDiagnostic {
                        kind: RuntimeHostConformanceDiagnosticKind::MissingHostCallImplementation,
                        adapter_id: manifest.id().as_str().to_owned(),
                        host_call: id,
                    })
                })
                .collect(),
        }
    }

    /// Checks several adapter manifests against the selected runner capabilities.
    pub fn check_adapter_manifests<'a>(
        &self,
        manifests: impl IntoIterator<Item = &'a AdapterManifest>,
    ) -> RuntimeHostConformanceReport {
        RuntimeHostConformanceReport {
            diagnostics: manifests
                .into_iter()
                .flat_map(|manifest| self.check_adapter_manifest(manifest).diagnostics)
                .collect(),
        }
    }

    /// Stable runtime-host call ids visible to tooling.
    pub fn host_call_ids(&self) -> impl Iterator<Item = &str> {
        self.host_calls.iter().map(AdapterHostCallId::as_str)
    }
}

impl RuntimeHostConformanceReport {
    /// Creates a report from diagnostics.
    pub fn from_diagnostics(
        diagnostics: impl IntoIterator<Item = RuntimeHostConformanceDiagnostic>,
    ) -> Self {
        Self {
            diagnostics: diagnostics.into_iter().collect(),
        }
    }

    /// Returns all conformance diagnostics.
    pub fn diagnostics(&self) -> &[RuntimeHostConformanceDiagnostic] {
        &self.diagnostics
    }

    /// Returns true when no conformance issue was found.
    pub fn is_success(&self) -> bool {
        self.diagnostics.is_empty()
    }
}

impl RuntimeHostConformanceDiagnostic {
    /// Diagnostic kind.
    pub const fn kind(&self) -> RuntimeHostConformanceDiagnosticKind {
        self.kind
    }

    /// Adapter manifest id that declared the unsupported host call.
    pub fn adapter_id(&self) -> &str {
        &self.adapter_id
    }

    /// Unsupported host call declared by the adapter manifest.
    pub const fn host_call(&self) -> &AdapterHostCallId {
        &self.host_call
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_adapter_context::manifest::{AdapterEffectCapability, AdapterHostCall};

    #[test]
    fn standard_native_includes_file_system_info_and_scheduler_calls() {
        let capabilities = RuntimeHostCapabilities::standard_native();

        for id in [
            "fs.read_text",
            "system.core_count",
            "system.available_parallelism",
            "flow_thread.run_child",
        ] {
            assert!(
                capabilities
                    .host_call_ids()
                    .any(|candidate| candidate == id)
            );
        }
    }

    #[test]
    fn browser_web_excludes_native_file_calls() {
        let capabilities = RuntimeHostCapabilities::browser_web();

        assert!(
            capabilities
                .host_call_ids()
                .any(|candidate| candidate == "system.core_count")
        );
        assert!(
            !capabilities
                .host_call_ids()
                .any(|candidate| candidate == "fs.read_text")
        );
    }

    #[test]
    fn implemented_adapter_manifests_extend_capabilities() {
        let custom = AdapterManifest::new("custom", "Custom")
            .with_host_call(AdapterHostCall::new("custom.read", []));
        let capabilities = RuntimeHostCapabilities::browser_web().with_adapter_manifest(&custom);

        assert!(
            capabilities
                .host_call_ids()
                .any(|candidate| candidate == "custom.read")
        );
    }

    #[test]
    fn runner_kind_builds_expected_presets() {
        let native = RuntimeHostRunnerKind::Native.capabilities();
        let browser = RuntimeHostRunnerKind::BrowserWeb.capabilities();
        let custom = RuntimeHostRunnerKind::Custom.capabilities();

        assert!(native.host_call_ids().any(|id| id == "fs.read_text"));
        assert!(!browser.host_call_ids().any(|id| id == "fs.read_text"));
        assert_eq!(custom.host_call_ids().count(), 0);
    }

    #[test]
    fn conformance_reports_declared_calls_missing_from_runner() {
        let manifest = AdapterManifest::new("custom", "Custom")
            .with_host_call(AdapterHostCall::new("custom.read", []));
        let report = RuntimeHostRunnerKind::BrowserWeb
            .capabilities()
            .check_adapter_manifest(&manifest);

        assert!(!report.is_success());
        assert_eq!(report.diagnostics().len(), 1);
        let diagnostic = &report.diagnostics()[0];
        assert_eq!(
            diagnostic.kind(),
            RuntimeHostConformanceDiagnosticKind::MissingHostCallImplementation
        );
        assert_eq!(diagnostic.adapter_id(), "custom");
        assert_eq!(diagnostic.host_call().as_str(), "custom.read");
    }

    #[test]
    fn conformance_accepts_calls_added_by_implemented_manifest() {
        let manifest = AdapterManifest::new("custom", "Custom")
            .with_host_call(AdapterHostCall::new("custom.read", []));
        let capabilities = RuntimeHostRunnerKind::BrowserWeb
            .capabilities()
            .with_adapter_manifest(&manifest);

        assert!(capabilities.check_adapter_manifest(&manifest).is_success());
    }

    #[test]
    fn effect_availability_does_not_substitute_for_runner_host_call_support() {
        let manifest = AdapterManifest::new("custom", "Custom")
            .with_effect(AdapterEffectCapability::new("fs.read"))
            .with_host_call(AdapterHostCall::new("custom.read", []));
        let report = RuntimeHostCapabilities::new().check_adapter_manifest(&manifest);

        assert_eq!(
            manifest
                .effects()
                .iter()
                .map(AdapterEffectCapability::as_str)
                .collect::<Vec<_>>(),
            ["fs.read"]
        );
        assert_eq!(report.diagnostics().len(), 1);
        assert_eq!(
            report.diagnostics()[0].kind(),
            RuntimeHostConformanceDiagnosticKind::MissingHostCallImplementation
        );
        assert_eq!(report.diagnostics()[0].adapter_id(), "custom");
        assert_eq!(report.diagnostics()[0].host_call().as_str(), "custom.read");
    }
}
