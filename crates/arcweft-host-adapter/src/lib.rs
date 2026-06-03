//! Host adapter dispatch primitives.
//!
//! This crate only owns typed policy and dispatch tables. Concrete I/O, GPU,
//! network, or OS integration belongs in adapter crates or application hosts.

use arcweft_adapter_context::manifest::AdapterManifest;
use arcweft_core::task::{HostTaskRequest, TaskSpec};
use arcweft_core::value::RuntimePayload;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use thiserror::Error;

/// Concrete host-side implementation for one or more manifest host calls.
pub trait HostAdapter: Send + Sync + std::fmt::Debug {
    /// Manifest exported by this adapter implementation.
    fn manifest(&self) -> &AdapterManifest;

    /// Attempts to complete one task.
    fn complete(&self, task: &TaskSpec) -> Option<HostTaskOutcome>;

    /// Returns whether this task can be completed on a worker thread.
    fn can_complete_in_parallel(&self, request: &HostTaskRequest) -> bool;
}

/// Manifest-derived allow-list for runtime host calls.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct HostCallPolicy {
    ids: BTreeSet<String>,
}

/// Registry of concrete host adapter implementations indexed by host-call id.
#[derive(Clone, Debug, Default)]
pub struct HostAdapterRegistry {
    adapters: BTreeMap<String, Arc<dyn HostAdapter>>,
}

/// Builder that rejects ambiguous host-call ownership.
#[derive(Clone, Debug, Default)]
pub struct HostAdapterRegistryBuilder {
    adapters: BTreeMap<String, Arc<dyn HostAdapter>>,
}

/// Result and accounting returned by one concrete adapter call.
#[derive(Clone, Debug, PartialEq)]
pub struct HostTaskOutcome {
    pub result: Result<RuntimePayload, String>,
    pub metrics: HostTaskMetrics,
}

/// Host-side work counters, aggregated by the embedding runtime.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct HostTaskMetrics {
    pub read_ops: usize,
    pub write_ops: usize,
    pub system_info_ops: usize,
    pub bytes_read: usize,
    pub bytes_written: usize,
}

/// Host adapter registration error.
#[derive(Debug, Error, Eq, PartialEq)]
pub enum HostAdapterError {
    #[error(
        "host call `{host_call_id}` is registered by both `{first_adapter}` and `{second_adapter}`"
    )]
    DuplicateHostCall {
        host_call_id: String,
        first_adapter: String,
        second_adapter: String,
    },
}

impl HostCallPolicy {
    /// Creates an empty policy.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a policy from one or more manifests.
    pub fn from_manifests(manifests: impl IntoIterator<Item = AdapterManifest>) -> Self {
        Self {
            ids: manifests
                .into_iter()
                .flat_map(|manifest| {
                    manifest
                        .host_calls()
                        .iter()
                        .map(|host_call| host_call.id().to_owned())
                        .collect::<Vec<_>>()
                })
                .collect(),
        }
    }

    /// Returns a new policy containing ids from both inputs.
    #[must_use]
    pub fn union(mut self, other: Self) -> Self {
        self.ids.extend(other.ids);
        self
    }

    /// Returns true when this host-call id is enabled by the active manifests.
    pub fn contains(&self, id: &str) -> bool {
        self.ids.contains(id)
    }

    /// Returns true when this task request is enabled by the active manifests.
    pub fn allows(&self, request: &HostTaskRequest) -> bool {
        self.contains(&request.host_call_id())
    }
}

impl HostAdapterRegistry {
    /// Creates an empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Starts building a registry.
    pub fn builder() -> HostAdapterRegistryBuilder {
        HostAdapterRegistryBuilder::new()
    }

    /// Returns true when a concrete adapter owns the host-call id.
    pub fn contains(&self, id: &str) -> bool {
        self.adapters.contains_key(id)
    }

    /// Dispatches a task to the concrete adapter registered for its host-call id.
    pub fn dispatch(&self, task: &TaskSpec) -> Option<HostTaskOutcome> {
        self.adapters
            .get(&task.request.host_call_id())
            .and_then(|adapter| adapter.complete(task))
    }

    /// Returns whether the registered adapter can complete this request in parallel.
    pub fn can_complete_in_parallel(&self, request: &HostTaskRequest) -> bool {
        self.adapters
            .get(&request.host_call_id())
            .is_some_and(|adapter| adapter.can_complete_in_parallel(request))
    }
}

impl HostAdapterRegistryBuilder {
    /// Creates an empty registry builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers one concrete adapter for every host call in its manifest.
    ///
    /// # Errors
    ///
    /// Returns [`HostAdapterError::DuplicateHostCall`] if two adapters export
    /// the same runtime host-call id.
    pub fn register<A>(mut self, adapter: A) -> Result<Self, HostAdapterError>
    where
        A: HostAdapter + 'static,
    {
        let adapter = Arc::new(adapter);
        let adapter_id = adapter.manifest().id().as_str().to_owned();
        for host_call in adapter.manifest().host_calls() {
            let host_call_id = host_call.id().to_owned();
            if let Some(existing) = self.adapters.get(&host_call_id) {
                return Err(HostAdapterError::DuplicateHostCall {
                    host_call_id,
                    first_adapter: existing.manifest().id().as_str().to_owned(),
                    second_adapter: adapter_id,
                });
            }
            self.adapters.insert(host_call_id, adapter.clone());
        }
        Ok(self)
    }

    /// Builds the immutable registry.
    pub fn build(self) -> HostAdapterRegistry {
        HostAdapterRegistry {
            adapters: self.adapters,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_adapter_context::manifest::{AdapterHostCall, AdapterManifest};
    use arcweft_core::task::{
        CancelScopeId, HostTaskRequest, TaskClass, TaskId, TaskKey, TaskPolicy, TaskPriority,
    };

    #[derive(Debug)]
    struct StaticAdapter {
        manifest: AdapterManifest,
        host_call_id: String,
        result: RuntimePayload,
        parallel: bool,
    }

    impl HostAdapter for StaticAdapter {
        fn manifest(&self) -> &AdapterManifest {
            &self.manifest
        }

        fn complete(&self, task: &TaskSpec) -> Option<HostTaskOutcome> {
            (task.request.host_call_id() == self.host_call_id).then(|| HostTaskOutcome {
                result: Ok(self.result.clone()),
                metrics: HostTaskMetrics::default(),
            })
        }

        fn can_complete_in_parallel(&self, _request: &HostTaskRequest) -> bool {
            self.parallel
        }
    }

    #[test]
    fn registry_dispatches_by_manifest_host_call() {
        let registry = HostAdapterRegistry::builder()
            .register(StaticAdapter {
                manifest: manifest("fixture", "fixture.echo"),
                host_call_id: "fixture.echo".to_owned(),
                result: RuntimePayload::from("ok"),
                parallel: true,
            })
            .expect("adapter is unique")
            .build();

        let task = task("fixture", "echo");
        let outcome = registry.dispatch(&task).expect("adapter handles task");

        assert_eq!(outcome.result.expect("task succeeds").label(), "ok");
        assert!(registry.can_complete_in_parallel(&task.request));
    }

    #[test]
    fn registry_rejects_duplicate_host_call_ids() {
        let error = HostAdapterRegistry::builder()
            .register(StaticAdapter {
                manifest: manifest("first", "fixture.echo"),
                host_call_id: "fixture.echo".to_owned(),
                result: RuntimePayload::from("first"),
                parallel: false,
            })
            .expect("first adapter is unique")
            .register(StaticAdapter {
                manifest: manifest("second", "fixture.echo"),
                host_call_id: "fixture.echo".to_owned(),
                result: RuntimePayload::from("second"),
                parallel: false,
            })
            .expect_err("duplicate host call is rejected");

        assert!(matches!(
            error,
            HostAdapterError::DuplicateHostCall {
                host_call_id,
                first_adapter,
                second_adapter
            } if host_call_id == "fixture.echo"
                && first_adapter == "first"
                && second_adapter == "second"
        ));
    }

    #[test]
    fn policy_is_manifest_derived() {
        let policy = HostCallPolicy::from_manifests([manifest("fixture", "fixture.echo")]);

        assert!(policy.contains("fixture.echo"));
        assert!(policy.allows(&task("fixture", "echo").request));
        assert!(!policy.contains("fixture.missing"));
    }

    fn manifest(id: &str, host_call_id: &str) -> AdapterManifest {
        AdapterManifest::new(id, id)
            .with_host_call(AdapterHostCall::new(host_call_id.to_owned(), []))
    }

    fn task(capability: &str, operation: &str) -> TaskSpec {
        TaskSpec::new(
            TaskId(format!("{capability}.{operation}")),
            TaskKey(format!("{capability}.{operation}")),
            TaskClass::Background,
            TaskPriority(0),
            CancelScopeId("test".to_owned()),
            TaskPolicy::JoinSameKey,
            HostTaskRequest::custom(capability, operation, []),
        )
    }
}
