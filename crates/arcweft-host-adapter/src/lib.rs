//! Host adapter dispatch primitives.
//!
//! This crate only owns typed policy and dispatch tables. Concrete I/O, GPU,
//! network, or OS integration belongs in adapter crates or application hosts.

use arcweft_adapter_context::manifest::{
    AdapterHostCall, AdapterManifest, AdapterNominalOwner, AdapterTypeKind,
};
use arcweft_core::pattern::{
    RuntimeCheckedType, RuntimeSemanticTypeId, RuntimeSemanticTypeIdentityEncoder,
};
use arcweft_core::step::RuntimeHostCallMode;
use arcweft_core::task::{BoundTaskOutcome, HostTaskRequest, NamedHostArg, TaskId, TaskSpec};
use arcweft_core::value::{
    RuntimePayload, RuntimeSignedIntWidth, RuntimeUnsignedIntWidth, RuntimeValue,
};
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
    fn complete(&self, _task: &TaskSpec, _outcome: &BoundTaskOutcome) -> Option<HostTaskOutcome> {
        None
    }

    /// Starts one task and reports whether it completed or remains pending.
    fn submit(&self, task: &TaskSpec, outcome: &BoundTaskOutcome) -> Option<HostTaskSubmission> {
        self.complete(task, outcome)
            .map(HostTaskSubmission::Completed)
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
    calls: BTreeMap<String, RegisteredHostCall>,
}

/// Builder that rejects ambiguous host-call ownership.
#[derive(Clone, Debug, Default)]
pub struct HostAdapterRegistryBuilder {
    calls: BTreeMap<String, RegisteredHostCall>,
}

#[derive(Clone, Debug)]
struct RegisteredHostCall {
    adapter: Arc<dyn HostAdapter>,
    contract: RegisteredHostCallContract,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RegisteredHostCallContract {
    digest: arcweft_adapter_context::manifest::HostCallContractDigest,
    mode: RuntimeHostCallMode,
    result: RuntimeSemanticTypeId,
}

/// Result and accounting returned by one concrete adapter call.
#[derive(Clone, Debug, PartialEq)]
pub struct HostTaskOutcome {
    pub completion: HostTaskCompletion,
    pub metrics: HostTaskMetrics,
}

/// Typed terminal result of a host task.
///
/// `Ready` carries the complete payload selected by the task's
/// [`arcweft_core::task::TaskOutcomeContract`]. A domain error is therefore a
/// `Result::Err` value inside `Ready`. `Failed` is reserved for a host or
/// adapter failure that cannot be represented by that contract.
#[derive(Clone, Debug, PartialEq)]
pub enum HostTaskCompletion {
    Ready(RuntimePayload),
    Failed(String),
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
    named: &'a [NamedHostArg<RuntimePayload>],
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
    #[error(
        "host call `{host_call_id}` from adapter `{adapter}` has an invalid result contract: {error}"
    )]
    InvalidHostCallResultContract {
        adapter: String,
        host_call_id: String,
        error: HostCallRuntimeTypeError,
    },
    #[error("host-main-thread pump for adapter `{adapter}` failed: {message}")]
    Pump { adapter: String, message: String },
}

/// Invalid result shape in a manifest host-call contract.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum HostCallRuntimeTypeError {
    #[error("Need may appear only as the outer host-call result")]
    NestedNeed,
}

impl HostCallPolicy {
    /// Creates an empty policy.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a policy from one or more manifests.
    pub fn from_manifests(manifests: impl IntoIterator<Item = AdapterManifest>) -> Self {
        Self::from_selected_calls(manifests, |_| true)
    }

    /// Creates an implicit embedding policy without non-returning host calls.
    ///
    /// A host call returning `Never` can take control away from the embedding
    /// process. Such calls require explicit adapter selection with
    /// [`Self::from_manifests`].
    pub fn from_returning_manifests(manifests: impl IntoIterator<Item = AdapterManifest>) -> Self {
        Self::from_selected_calls(manifests, |host_call| {
            !matches!(host_call.signature().return_type(), AdapterTypeKind::Never)
        })
    }

    fn from_selected_calls(
        manifests: impl IntoIterator<Item = AdapterManifest>,
        include: impl Fn(&AdapterHostCall) -> bool,
    ) -> Self {
        let mut policy = Self::new();
        for manifest in manifests {
            policy.ids.extend(
                manifest
                    .host_calls()
                    .iter()
                    .filter(|host_call| include(host_call))
                    .map(|host_call| host_call.id().to_owned()),
            );
        }
        policy
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
        self.calls.contains_key(id)
    }

    /// Returns the exact manifest-owned ABI identity for one registered call.
    pub fn host_call_contract(
        &self,
        id: &str,
    ) -> Option<arcweft_adapter_context::manifest::HostCallContractDigest> {
        self.calls.get(id).map(|call| call.contract.digest)
    }

    /// Returns the manifest-owned semantic identity of one host-call result.
    pub fn host_call_result_type(&self, id: &str) -> Option<RuntimeSemanticTypeId> {
        self.calls.get(id).map(|call| call.contract.result)
    }

    /// Checks the runtime result predicate against the exact selected manifest
    /// signature before dispatch. The result identity was sealed once when its
    /// owning adapter was registered.
    pub fn host_call_accepts_runtime_result(
        &self,
        id: &str,
        mode: RuntimeHostCallMode,
        result: RuntimeSemanticTypeId,
    ) -> bool {
        self.calls
            .get(id)
            .is_some_and(|call| call.contract.mode == mode && call.contract.result == result)
    }

    /// Starts a task through the concrete adapter registered for its host-call id.
    pub fn submit(
        &self,
        task: &TaskSpec,
        outcome: &BoundTaskOutcome,
    ) -> Option<HostTaskSubmission> {
        self.calls
            .get(&task.request.host_call_id())
            .and_then(|call| call.adapter.submit(task, outcome))
    }

    /// Synchronous helper. Pending work returns `None`.
    pub fn dispatch(&self, task: &TaskSpec, outcome: &BoundTaskOutcome) -> Option<HostTaskOutcome> {
        match self.submit(task, outcome)? {
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
        self.calls
            .values()
            .filter(|call| {
                let identity = Arc::as_ptr(&call.adapter).cast::<()>();
                seen.insert(identity)
            })
            .map(|call| call.adapter.clone())
            .collect()
    }

    /// Returns whether the registered adapter can complete this request in parallel.
    pub fn can_complete_in_parallel(&self, request: &HostTaskRequest) -> bool {
        self.calls
            .get(&request.host_call_id())
            .is_some_and(|call| call.adapter.can_complete_in_parallel(request))
    }
}

impl RegisteredHostCallContract {
    fn seal(call: &AdapterHostCall) -> Result<Self, HostCallRuntimeTypeError> {
        let (mode, result) = match call.signature().return_type() {
            AdapterTypeKind::Need { item } => (RuntimeHostCallMode::Suspend, item.as_ref()),
            result => (RuntimeHostCallMode::Immediate, result),
        };
        ensure_no_nested_need(result)?;
        Ok(Self {
            digest: call.contract_digest(),
            mode,
            result: adapter_type_semantic_identity(result),
        })
    }
}

fn ensure_no_nested_need(ty: &AdapterTypeKind) -> Result<(), HostCallRuntimeTypeError> {
    match ty {
        AdapterTypeKind::Need { .. } => Err(HostCallRuntimeTypeError::NestedNeed),
        AdapterTypeKind::Vec { item }
        | AdapterTypeKind::Seq { item }
        | AdapterTypeKind::Option { item } => ensure_no_nested_need(item),
        AdapterTypeKind::Result { ok, error } => {
            ensure_no_nested_need(ok)?;
            ensure_no_nested_need(error)
        }
        AdapterTypeKind::Tuple { items } => items.iter().try_for_each(ensure_no_nested_need),
        AdapterTypeKind::Nominal { nominal } => nominal
            .arguments()
            .iter()
            .try_for_each(ensure_no_nested_need),
        AdapterTypeKind::Unit
        | AdapterTypeKind::Never
        | AdapterTypeKind::Bool
        | AdapterTypeKind::I8
        | AdapterTypeKind::I16
        | AdapterTypeKind::I32
        | AdapterTypeKind::I64
        | AdapterTypeKind::I128
        | AdapterTypeKind::ISize
        | AdapterTypeKind::U8
        | AdapterTypeKind::U16
        | AdapterTypeKind::U32
        | AdapterTypeKind::U64
        | AdapterTypeKind::U128
        | AdapterTypeKind::USize
        | AdapterTypeKind::F32
        | AdapterTypeKind::F64
        | AdapterTypeKind::String
        | AdapterTypeKind::Char
        | AdapterTypeKind::Bytes => Ok(()),
    }
}

fn adapter_type_semantic_identity(ty: &AdapterTypeKind) -> RuntimeSemanticTypeId {
    let mut encoder = RuntimeSemanticTypeIdentityEncoder::new();
    encode_adapter_semantic_type(&mut encoder, ty);
    encoder.finish()
}

fn encode_adapter_semantic_type(
    encoder: &mut RuntimeSemanticTypeIdentityEncoder,
    ty: &AdapterTypeKind,
) {
    match ty {
        AdapterTypeKind::Unit => RuntimeCheckedType::Unit.encode_semantic_identity(encoder),
        AdapterTypeKind::Never => RuntimeCheckedType::Never.encode_semantic_identity(encoder),
        AdapterTypeKind::Bool => RuntimeCheckedType::Bool.encode_semantic_identity(encoder),
        AdapterTypeKind::I8 => {
            RuntimeCheckedType::Signed(RuntimeSignedIntWidth::I8).encode_semantic_identity(encoder)
        }
        AdapterTypeKind::I16 => {
            RuntimeCheckedType::Signed(RuntimeSignedIntWidth::I16).encode_semantic_identity(encoder)
        }
        AdapterTypeKind::I32 => {
            RuntimeCheckedType::Signed(RuntimeSignedIntWidth::I32).encode_semantic_identity(encoder)
        }
        AdapterTypeKind::I64 => {
            RuntimeCheckedType::Signed(RuntimeSignedIntWidth::I64).encode_semantic_identity(encoder)
        }
        AdapterTypeKind::I128 => RuntimeCheckedType::Signed(RuntimeSignedIntWidth::I128)
            .encode_semantic_identity(encoder),
        AdapterTypeKind::ISize => RuntimeCheckedType::Signed(RuntimeSignedIntWidth::ISize)
            .encode_semantic_identity(encoder),
        AdapterTypeKind::U8 => RuntimeCheckedType::Unsigned(RuntimeUnsignedIntWidth::U8)
            .encode_semantic_identity(encoder),
        AdapterTypeKind::U16 => RuntimeCheckedType::Unsigned(RuntimeUnsignedIntWidth::U16)
            .encode_semantic_identity(encoder),
        AdapterTypeKind::U32 => RuntimeCheckedType::Unsigned(RuntimeUnsignedIntWidth::U32)
            .encode_semantic_identity(encoder),
        AdapterTypeKind::U64 => RuntimeCheckedType::Unsigned(RuntimeUnsignedIntWidth::U64)
            .encode_semantic_identity(encoder),
        AdapterTypeKind::U128 => RuntimeCheckedType::Unsigned(RuntimeUnsignedIntWidth::U128)
            .encode_semantic_identity(encoder),
        AdapterTypeKind::USize => RuntimeCheckedType::Unsigned(RuntimeUnsignedIntWidth::USize)
            .encode_semantic_identity(encoder),
        AdapterTypeKind::F32 => RuntimeCheckedType::F32.encode_semantic_identity(encoder),
        AdapterTypeKind::F64 => RuntimeCheckedType::F64.encode_semantic_identity(encoder),
        AdapterTypeKind::String => RuntimeCheckedType::String.encode_semantic_identity(encoder),
        AdapterTypeKind::Char => RuntimeCheckedType::Char.encode_semantic_identity(encoder),
        AdapterTypeKind::Bytes => RuntimeCheckedType::Bytes.encode_semantic_identity(encoder),
        AdapterTypeKind::Vec { item } => {
            encoder.write_tag(48);
            encode_adapter_semantic_type(encoder, item);
        }
        AdapterTypeKind::Seq { item } => {
            encoder.write_tag(51);
            encode_adapter_semantic_type(encoder, item);
        }
        AdapterTypeKind::Need { item } => {
            encoder.write_tag(54);
            encode_adapter_semantic_type(encoder, item);
        }
        AdapterTypeKind::Result { ok, error } => {
            encoder.write_tag(57);
            encode_adapter_semantic_type(encoder, ok);
            encode_adapter_semantic_type(encoder, error);
        }
        AdapterTypeKind::Option { item } => {
            encoder.write_tag(58);
            encode_adapter_semantic_type(encoder, item);
        }
        AdapterTypeKind::Nominal { nominal } => {
            encoder.write_tag(65);
            match nominal.owner() {
                AdapterNominalOwner::Standard => encoder.write_u8(0),
                AdapterNominalOwner::Environment { owner } => {
                    encoder.write_u8(1);
                    encoder.write_str(owner.as_str());
                }
                AdapterNominalOwner::RustPackage { package } => {
                    encoder.write_u8(2);
                    encoder.write_str(package.as_str());
                }
            }
            encoder.write_u8(0);
            encoder.write_len(nominal.path().segments().len());
            for segment in nominal.path().segments() {
                encoder.write_str(segment.as_str());
            }
            encoder.write_len(nominal.arguments().len());
            for argument in nominal.arguments() {
                encode_adapter_semantic_type(encoder, argument);
            }
        }
        AdapterTypeKind::Tuple { items } => {
            encoder.write_tag(75);
            encoder.write_len(items.len());
            for item in items {
                encode_adapter_semantic_type(encoder, item);
            }
        }
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
            if let Some(existing) = self.calls.get(&host_call_id) {
                return Err(HostAdapterError::DuplicateHostCall {
                    host_call_id,
                    first_adapter: existing.adapter.manifest().id().as_str().to_owned(),
                    second_adapter: adapter_id,
                });
            }
            let contract = RegisteredHostCallContract::seal(host_call).map_err(|error| {
                HostAdapterError::InvalidHostCallResultContract {
                    adapter: adapter_id.clone(),
                    host_call_id: host_call_id.clone(),
                    error,
                }
            })?;
            self.calls.insert(
                host_call_id,
                RegisteredHostCall {
                    adapter: adapter.clone(),
                    contract,
                },
            );
        }
        Ok(self)
    }

    /// Builds the immutable registry.
    pub fn build(self) -> HostAdapterRegistry {
        HostAdapterRegistry { calls: self.calls }
    }
}

impl<'a> HostCallArgs<'a> {
    pub fn new(
        positional: &'a [RuntimePayload],
        named: &'a [NamedHostArg<RuntimePayload>],
    ) -> Self {
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

    /// Reads one required positional-or-named parameter, rejecting extra arguments.
    pub fn single(&self, name: &str) -> Result<&'a RuntimeValue, String> {
        match (self.positional, self.named) {
            ([value], []) => Ok(value.value()),
            ([], [argument]) if argument.name == name => Ok(argument.value.value()),
            _ => Err(format!(
                "expected exactly one positional argument or named `{name}` argument"
            )),
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
        RuntimeValue::Need(_) => "Need",
        RuntimeValue::Char(_) => "Char",
        RuntimeValue::Duration(_) => "Duration",
        RuntimeValue::Progress(_) => "Progress",
        RuntimeValue::Range(_) => "Range",
        RuntimeValue::EntityRef(_) => "Ref",
        RuntimeValue::Tuple(_) => "Tuple",
        RuntimeValue::Seq(_) => "Seq",
        RuntimeValue::Record(_) => "Record",
        RuntimeValue::NominalRecord(_) => "NominalRecord",
        RuntimeValue::Opaque(_) => "Opaque",
        RuntimeValue::Agent(value) => value.label(),
        RuntimeValue::Reduction(_) => "Reduction",
        RuntimeValue::Callable(_) => "Function",
        RuntimeValue::Variant { .. } => "Variant",
        RuntimeValue::Iterator(_) => "Iterator",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_adapter_context::{
        manifest::{
            AdapterCallableGroupIndex, AdapterFunctionSignature, AdapterHostCall, AdapterManifest,
            AdapterParameterGroup,
        },
        standard,
    };
    use arcweft_core::pattern::RuntimeCheckedType;
    use arcweft_core::task::{
        CancelScopeId, HostTaskRequest, TaskClass, TaskId, TaskKey, TaskOutcomeContract,
        TaskPolicy, TaskPriority,
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

        fn complete(&self, task: &TaskSpec, outcome: &BoundTaskOutcome) -> Option<HostTaskOutcome> {
            (task.request.host_call_id() == self.host_call_id).then(|| {
                let completion = outcome
                    .try_payload(self.result.value().clone())
                    .map_or_else(
                        |error| HostTaskCompletion::Failed(error.to_string()),
                        HostTaskCompletion::Ready,
                    );
                HostTaskOutcome {
                    completion,
                    metrics: HostTaskMetrics::default(),
                }
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
        let bound = task
            .outcome
            .bind_standalone()
            .expect("test task has a standalone outcome contract");
        let outcome = registry
            .dispatch(&task, &bound)
            .expect("adapter handles task");

        let HostTaskCompletion::Ready(value) = outcome.completion else {
            panic!("task succeeds");
        };
        assert_eq!(value.label(), "ok");
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
            layout: arcweft_core::entry::TypeLayoutHash::from_bytes([8; 32]),
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

    #[test]
    fn registered_host_call_contract_uses_digest_mode_and_result_semantic_identity() {
        let manifest = standard::native_file_manifest();
        let call = &manifest.host_calls()[0];
        let (expected_mode, result_type) = match call.signature().return_type() {
            AdapterTypeKind::Need { item } => (RuntimeHostCallMode::Suspend, item.as_ref()),
            result => (RuntimeHostCallMode::Immediate, result),
        };
        let expected_digest = call.contract_digest();
        let expected_result = adapter_type_semantic_identity(result_type);
        let need_identity = adapter_type_semantic_identity(call.signature().return_type());
        assert_ne!(expected_result, need_identity);

        let sealed = RegisteredHostCallContract::seal(call)
            .expect("standard file result contract has no nested Need");
        assert_eq!(sealed.digest, expected_digest);
        assert_eq!(sealed.mode, RuntimeHostCallMode::Suspend);
        assert_eq!(sealed.result, expected_result);

        let registry = HostAdapterRegistry::builder()
            .register(StaticAdapter {
                manifest,
                host_call_id: "fs.read_text".to_owned(),
                result: RuntimePayload::from("unused"),
                parallel: false,
            })
            .expect("standard manifest registers")
            .build();

        assert_eq!(
            registry.host_call_contract("fs.read_text"),
            Some(expected_digest)
        );
        assert_eq!(
            registry.host_call_result_type("fs.read_text"),
            Some(expected_result)
        );
        assert!(registry.host_call_accepts_runtime_result(
            "fs.read_text",
            expected_mode,
            expected_result,
        ));
        assert!(!registry.host_call_accepts_runtime_result(
            "fs.read_text",
            RuntimeHostCallMode::Immediate,
            expected_result,
        ));
        assert!(!registry.host_call_accepts_runtime_result(
            "fs.read_text",
            expected_mode,
            need_identity,
        ));
    }

    #[test]
    fn primitive_host_results_share_the_checked_runtime_type_authority() {
        for (adapter, checked) in [
            (AdapterTypeKind::Unit, RuntimeCheckedType::Unit),
            (AdapterTypeKind::Never, RuntimeCheckedType::Never),
            (AdapterTypeKind::Bool, RuntimeCheckedType::Bool),
            (
                AdapterTypeKind::I32,
                RuntimeCheckedType::Signed(RuntimeSignedIntWidth::I32),
            ),
            (
                AdapterTypeKind::U8,
                RuntimeCheckedType::Unsigned(RuntimeUnsignedIntWidth::U8),
            ),
            (AdapterTypeKind::String, RuntimeCheckedType::String),
            (AdapterTypeKind::Bytes, RuntimeCheckedType::Bytes),
        ] {
            assert_eq!(
                adapter_type_semantic_identity(&adapter),
                checked.semantic_identity_digest()
            );
        }
    }

    #[test]
    fn registry_rejects_nested_need_in_manifest_host_call_result() {
        let signature = AdapterFunctionSignature::try_new(
            vec![
                AdapterParameterGroup::try_new(
                    AdapterCallableGroupIndex::try_from_usize(0).expect("initial group index fits"),
                    Vec::new(),
                )
                .expect("empty initial group is valid"),
            ],
            AdapterTypeKind::Option {
                item: Box::new(AdapterTypeKind::Need {
                    item: Box::new(AdapterTypeKind::String),
                }),
            },
        )
        .expect("signature shape is valid");
        let manifest = AdapterManifest::new("fixture", "Fixture").with_host_call(
            AdapterHostCall::with_signature("fixture.nested", signature, []),
        );
        let error = HostAdapterRegistry::builder()
            .register(StaticAdapter {
                manifest,
                host_call_id: "fixture.nested".to_owned(),
                result: RuntimePayload::from("unused"),
                parallel: false,
            })
            .expect_err("Need remains an outer host-call modality");

        assert_eq!(
            error,
            HostAdapterError::InvalidHostCallResultContract {
                adapter: "fixture".to_owned(),
                host_call_id: "fixture.nested".to_owned(),
                error: HostCallRuntimeTypeError::NestedNeed,
            }
        );
    }

    fn manifest(id: &str, host_call_id: &str) -> AdapterManifest {
        let signature = AdapterFunctionSignature::try_new(
            vec![
                AdapterParameterGroup::try_new(
                    AdapterCallableGroupIndex::try_from_usize(0).expect("initial group index fits"),
                    Vec::new(),
                )
                .expect("empty initial group is valid"),
            ],
            AdapterTypeKind::String,
        )
        .expect("fixture signature is valid");
        AdapterManifest::new(id, id).with_host_call(AdapterHostCall::with_signature(
            host_call_id.to_owned(),
            signature,
            [],
        ))
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
        .with_outcome(TaskOutcomeContract::new(RuntimeCheckedType::String))
    }
}
