use crate::native_task::internal_scheduler_manifest;
use arcweft_adapter_context::{
    manifest::{AdapterHostCallId, AdapterManifest},
    standard,
};
use std::collections::BTreeSet;

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

    /// Stable runtime-host call ids visible to tooling.
    pub fn host_call_ids(&self) -> impl Iterator<Item = &str> {
        self.host_calls.iter().map(AdapterHostCallId::as_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_adapter_context::manifest::AdapterHostCall;

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
}
