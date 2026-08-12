//! Host adapter dispatch primitives.
//!
//! This crate only owns typed policy and dispatch tables. Concrete I/O, GPU,
//! network, or OS integration belongs in adapter crates or application hosts.

use arcweft_adapter_context::manifest::AdapterManifest;
use arcweft_core::task::{HostTaskRequest, NamedHostTaskArg, TaskId, TaskSpec};
use arcweft_core::value::{RuntimePayload, RuntimeValue};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use thiserror::Error;

/// Concrete host-side implementation for one or more manifest host calls.
pub trait HostAdapter: Send + Sync + std::fmt::Debug {
    /// Manifest exported by this adapter implementation.
    fn manifest(&self) -> &AdapterManifest;

    /// Attempts to complete one synchronous task.
    ///
    /// Adapters that need asynchronous or host-main-thread work should
    /// override [`Self::submit`] instead.
    fn complete(&self, _task: &TaskSpec) -> Option<HostTaskOutcome> {
        None
    }

    /// Starts one task and reports whether it completed or remains pending.
    fn submit(&self, task: &TaskSpec) -> Option<HostTaskSubmission> {
        self.complete(task).map(HostTaskSubmission::Completed)
    }

    /// Drains adapter-owned completions produced since the last call.
    fn drain_completions(&self) -> Vec<HostAdapterCompletion> {
        Vec::new()
    }

    /// Requests cancellation of pending adapter-owned work.
    fn cancel(&self, _task_id: &TaskId) -> bool {
        false
    }

    /// Pumps work that is required to run on the embedding host's main thread.
    fn pump_main_thread(&self) -> Result<(), String> {
        Ok(())
    }

    /// Returns whether this task can be completed on a worker thread.
    fn can_complete_in_parallel(&self, request: &HostTaskRequest) -> bool;
}

/// Result of starting one host adapter task.
#[derive(Clone, Debug, PartialEq)]
pub enum HostTaskSubmission {
    Completed(HostTaskOutcome),
    Pending,
}

/// Completion emitted later by a pending host adapter task.
#[derive(Clone, Debug, PartialEq)]
pub struct HostAdapterCompletion {
    pub task_id: TaskId,
    pub outcome: HostTaskOutcome,
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

/// Shared typed view over generic custom host-call payloads.
pub struct HostCallArgs<'a> {
    positional: &'a [RuntimePayload],
    named: &'a [NamedHostTaskArg],
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
    #[error(
        "active adapter policy declares host calls without native implementations: {host_call_ids:?}"
    )]
    MissingHostCallImplementations { host_call_ids: Vec<String> },
    #[error("host-main-thread pump for adapter `{adapter}` failed: {message}")]
    Pump { adapter: String, message: String },
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

    /// Creates a policy from serialized host-call ids.
    pub fn from_host_call_ids<'a>(ids: impl IntoIterator<Item = &'a str>) -> Self {
        Self {
            ids: ids.into_iter().map(str::to_owned).collect(),
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

    /// Returns all manifest-authorized calls that lack concrete host adapters.
    pub fn missing_implementations(&self, registry: &HostAdapterRegistry) -> Vec<String> {
        self.ids
            .iter()
            .filter(|id| !registry.contains(id))
            .cloned()
            .collect()
    }

    /// Validates that every policy call has a concrete runtime implementation.
    ///
    /// This behavior belongs to the Arcweft-owned policy type. CLI and player
    /// hosts should not duplicate the comparison with local helper functions.
    pub fn ensure_implemented_by(
        &self,
        registry: &HostAdapterRegistry,
    ) -> Result<(), HostAdapterError> {
        let host_call_ids = self.missing_implementations(registry);
        if host_call_ids.is_empty() {
            Ok(())
        } else {
            Err(HostAdapterError::MissingHostCallImplementations { host_call_ids })
        }
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

    /// Starts a task through the concrete adapter registered for its host-call id.
    pub fn submit(&self, task: &TaskSpec) -> Option<HostTaskSubmission> {
        self.adapters
            .get(&task.request.host_call_id())
            .and_then(|adapter| adapter.submit(task))
    }

    /// Synchronous helper. Pending work returns `None`.
    pub fn dispatch(&self, task: &TaskSpec) -> Option<HostTaskOutcome> {
        match self.submit(task)? {
            HostTaskSubmission::Completed(outcome) => Some(outcome),
            HostTaskSubmission::Pending => None,
        }
    }

    /// Drains every registered adapter once, even when it owns multiple calls.
    pub fn drain_completions(&self) -> Vec<HostAdapterCompletion> {
        self.unique_adapters()
            .into_iter()
            .flat_map(|adapter| adapter.drain_completions())
            .collect()
    }

    /// Requests cancellation from every unique adapter until one owns the task.
    pub fn cancel(&self, task_id: &TaskId) -> bool {
        self.unique_adapters()
            .into_iter()
            .any(|adapter| adapter.cancel(task_id))
    }

    /// Pumps every unique adapter on the embedding host's main thread.
    pub fn pump_main_thread(&self) -> Result<(), HostAdapterError> {
        for adapter in self.unique_adapters() {
            adapter
                .pump_main_thread()
                .map_err(|message| HostAdapterError::Pump {
                    adapter: adapter.manifest().id().as_str().to_owned(),
                    message,
                })?;
        }
        Ok(())
    }

    fn unique_adapters(&self) -> Vec<Arc<dyn HostAdapter>> {
        let mut seen = BTreeSet::new();
        self.adapters
            .values()
            .filter(|adapter| {
                let identity = Arc::as_ptr(*adapter).cast::<()>();
                seen.insert(identity)
            })
            .cloned()
            .collect()
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

impl<'a> HostCallArgs<'a> {
    pub fn new(positional: &'a [RuntimePayload], named: &'a [NamedHostTaskArg]) -> Self {
        Self { positional, named }
    }

    pub fn from_custom_request(request: &'a HostTaskRequest) -> Option<Self> {
        let HostTaskRequest::Custom {
            args, named_args, ..
        } = request
        else {
            return None;
        };
        Some(Self::new(args, named_args))
    }

    pub fn expect_len(&self, len: usize) -> Result<(), String> {
        if self.positional.len() == len {
            Ok(())
        } else {
            Err(format!(
                "expected {len} positional host-call arguments, got {}",
                self.positional.len()
            ))
        }
    }

    pub fn string(&self, index: usize) -> Result<String, String> {
        match self.positional.get(index).map(|payload| &payload.0) {
            Some(RuntimeValue::String(value)) => Ok(value.clone()),
            Some(value) => Err(format!(
                "host-call argument #{index} must be String, got {}",
                runtime_value_kind(value)
            )),
            None => Err(format!("missing positional host-call argument #{index}")),
        }
    }

    pub fn bool(&self, index: usize) -> Result<bool, String> {
        match self.positional.get(index).map(|payload| &payload.0) {
            Some(RuntimeValue::Bool(value)) => Ok(*value),
            Some(value) => Err(format!(
                "host-call argument #{index} must be Bool, got {}",
                runtime_value_kind(value)
            )),
            None => Err(format!("missing positional host-call argument #{index}")),
        }
    }

    pub fn i32(&self, index: usize) -> Result<i32, String> {
        match self.positional.get(index).map(|payload| &payload.0) {
            Some(RuntimeValue::Int(value)) => value
                .try_into_i32()
                .ok_or_else(|| format!("host-call argument #{index} is outside the i32 range")),
            Some(value) => Err(format!(
                "host-call argument #{index} must be i32, got {}",
                runtime_value_kind(value)
            )),
            None => Err(format!("missing positional host-call argument #{index}")),
        }
    }

    pub fn u32(&self, index: usize) -> Result<u32, String> {
        match self.positional.get(index).map(|payload| &payload.0) {
            Some(RuntimeValue::UInt(value)) => value
                .try_into_u32()
                .ok_or_else(|| format!("host-call argument #{index} is outside the u32 range")),
            Some(RuntimeValue::Int(value)) => value
                .try_into_i64()
                .and_then(|value| u32::try_from(value).ok())
                .ok_or_else(|| format!("host-call argument #{index} is outside the u32 range")),
            Some(value) => Err(format!(
                "host-call argument #{index} must be u32, got {}",
                runtime_value_kind(value)
            )),
            None => Err(format!("missing positional host-call argument #{index}")),
        }
    }

    pub fn variant(&self, index: usize) -> Result<HostCallVariantArg<'a>, String> {
        match self.positional.get(index).map(|payload| &payload.0) {
            Some(RuntimeValue::Variant {
                owner,
                ordinal,
                name,
                payload,
            }) => Ok(HostCallVariantArg {
                owner,
                ordinal: *ordinal,
                name,
                payload: payload.as_deref(),
            }),
            Some(value) => Err(format!(
                "host-call argument #{index} must be Variant, got {}",
                runtime_value_kind(value)
            )),
            None => Err(format!("missing positional host-call argument #{index}")),
        }
    }

    pub fn named_string(&self, name: &str) -> Result<Option<String>, String> {
        self.named
            .iter()
            .find(|arg| arg.name == name)
            .map(|arg| match &arg.value.0 {
                RuntimeValue::String(value) => Ok(value.clone()),
                value => Err(format!(
                    "host-call argument `{name}` must be String, got {}",
                    runtime_value_kind(value)
                )),
            })
            .transpose()
    }
}

/// Borrowed enum-like variant argument decoded from a host-call payload.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HostCallVariantArg<'a> {
    pub owner: &'a arcweft_core::pattern::RuntimeVariantIdentity,
    pub ordinal: u32,
    pub name: &'a str,
    pub payload: Option<&'a RuntimeValue>,
}

fn runtime_value_kind(value: &RuntimeValue) -> &'static str {
    match value {
        RuntimeValue::Unit => "Unit",
        RuntimeValue::Bool(_) => "Bool",
        RuntimeValue::Int(_) => "Int",
        RuntimeValue::UInt(_) => "UInt",
        RuntimeValue::F32(_) => "F32",
        RuntimeValue::F64(_) => "F64",
        RuntimeValue::MatrixF32(_) => "MatrixF32",
        RuntimeValue::MatrixF64(_) => "MatrixF64",
        RuntimeValue::TensorF32(_) => "TensorF32",
        RuntimeValue::TensorF64(_) => "TensorF64",
        RuntimeValue::String(_) => "String",
        RuntimeValue::Char(_) => "Char",
        RuntimeValue::Duration(_) => "Duration",
        RuntimeValue::Range(_) => "Range",
        RuntimeValue::EntityRef(_) => "Ref",
        RuntimeValue::Tuple(_) => "Tuple",
        RuntimeValue::Seq(_) => "Seq",
        RuntimeValue::Record(_) => "Record",
        RuntimeValue::NominalRecord(_) => "NominalRecord",
        RuntimeValue::Opaque(_) => "Opaque",
        RuntimeValue::Function(_) => "Function",
        RuntimeValue::Variant { .. } => "Variant",
        RuntimeValue::Iterator(_) => "Iterator",
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

    #[test]
    fn policy_reports_all_missing_runtime_implementations_before_execution() {
        let policy = HostCallPolicy::from_host_call_ids(["fixture.first", "fixture.second"]);
        let error = policy
            .ensure_implemented_by(&HostAdapterRegistry::new())
            .expect_err("missing implementations are rejected");

        assert_eq!(
            error,
            HostAdapterError::MissingHostCallImplementations {
                host_call_ids: vec!["fixture.first".to_owned(), "fixture.second".to_owned()]
            }
        );
    }

    #[test]
    fn custom_host_call_args_preserve_positional_and_named_payloads() {
        let request = HostTaskRequest::custom_with_named_args(
            "fixture",
            "op",
            [RuntimePayload::from("title")],
            [("mode".to_owned(), RuntimePayload::from("fullscreen"))],
        );
        let args = HostCallArgs::from_custom_request(&request).expect("custom args");

        assert_eq!(args.string(0).expect("positional string"), "title");
        assert_eq!(
            args.named_string("mode").expect("named string"),
            Some("fullscreen".to_owned())
        );
        assert_eq!(args.named_string("missing").expect("missing named"), None);
    }

    #[test]
    fn custom_host_call_args_decode_variant_payloads() {
        let nominal = arcweft_core::entry::RuntimeNominalTypeId::try_new("WindowMode")
            .expect("test nominal identity");
        let owner = arcweft_core::pattern::RuntimeVariantIdentity::Nominal {
            nominal,
            semantic_identity: arcweft_core::pattern::RuntimeSemanticTypeId::from_bytes([7; 32]),
        };
        let request = HostTaskRequest::custom(
            "fixture",
            "op",
            [RuntimePayload(RuntimeValue::Variant {
                owner: owner.clone(),
                ordinal: 4,
                name: "Fullscreen".to_owned(),
                payload: None,
            })],
        );
        let args = HostCallArgs::from_custom_request(&request).expect("custom args");
        let variant = args.variant(0).expect("variant arg");

        assert_eq!(
            variant,
            HostCallVariantArg {
                owner: &owner,
                ordinal: 4,
                name: "Fullscreen",
                payload: None,
            }
        );
        assert!(
            args.string(0)
                .expect_err("variant is not string")
                .contains("Variant")
        );
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
