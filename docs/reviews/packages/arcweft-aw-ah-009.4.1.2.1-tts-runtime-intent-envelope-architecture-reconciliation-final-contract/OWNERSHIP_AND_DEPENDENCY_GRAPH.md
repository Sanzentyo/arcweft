# Ownership and dependency graph

## 1. Final direct Cargo edges

Every arrow is `dependent -> direct dependency`. Existing unrelated direct
edges remain unchanged; the rows below are the complete normative TTS/runtime
edge set.

```text
arcweft-lang-syntax -> (no TTS crate)
arcweft-lang-hir -> arcweft-lang-syntax
arcweft-lang-sema -> arcweft-lang-hir, arcweft-lang-syntax

arcweft-character -> arcweft-id, arcweft-source
arcweft-audio-core -> arcweft-interaction-model
arcweft-audio-tts -> arcweft-id, arcweft-character, arcweft-audio-core
arcweft-audio-tts-runtime -> arcweft-core, arcweft-audio-tts

arcweft-core -> (no audio, TTS, host-adapter, runtime-driver, or runtime-plan crate)
arcweft-runtime-plan -> arcweft-core, arcweft-lang-sema, arcweft-audio-tts-runtime
arcweft-compiler -> arcweft-runtime-plan, arcweft-bundle, arcweft-audio-tts

arcweft-runtime-scheduler -> arcweft-core
arcweft-host-adapter -> arcweft-core, arcweft-adapter-context, arcweft-audio-tts
arcweft-runtime-driver -> arcweft-core, arcweft-bundle, arcweft-audio-tts,
                          arcweft-audio-tts-runtime, arcweft-runtime-scheduler,
                          arcweft-save
arcweft-runtime-host -> arcweft-core, arcweft-runtime-driver,
                        arcweft-runtime-scheduler, arcweft-host-adapter,
                        arcweft-audio-tts, arcweft-audio-tts-runtime

arcweft-bundle -> arcweft-manifest-model, arcweft-audio-tts
arcweft-save -> arcweft-core, arcweft-bundle
arcweft-manifest-model -> arcweft-audio-tts
arcweft-adapter-metadata -> arcweft-manifest-model, arcweft-audio-tts
arcweft-project-loader -> arcweft-manifest-model, arcweft-adapter-metadata,
                          arcweft-bundle, arcweft-audio-tts
provider-specific-adapter -> arcweft-host-adapter, arcweft-audio-tts, provider SDK
```

`arcweft-replay` is not introduced. The existing replay module remains inside
`arcweft-runtime-driver` and therefore inherits the driver's lower edges.

## 2. Required forbidden edges

| Dependent | Forbidden direct/transitive intent | Reason |
|---|---|---|
| `arcweft-core` | `arcweft-audio-tts`, `arcweft-audio-*`, bridge | Core remains runtime/data-only Sans-I/O. |
| syntax/HIR/sema | audio TTS or bridge | Source typing uses shared nominal names/callable IDs, not runtime/domain implementation types. |
| scheduler | audio TTS, bridge, host adapter | One domain-neutral scheduler only. |
| audio TTS | core, runtime-plan, driver, host adapter | Lower domain model cannot reverse-depend on runtime integration. |
| bridge | runtime-plan, compiler, driver, scheduler, host adapter | Narrow two-lower-crate composition, not a facade. |
| host adapter | bridge, runtime-driver, runtime-plan | Host APIs accept domain selected types, not runtime payload constructors. |
| bundle/save | runtime-driver or host adapter | Persistent owners cannot depend upward on execution/privilege owners. |
| provider adapters | bridge or core intent types | Providers receive credential-slot-only domain requests. |

## 3. Type and method owners

| Owner/module | Normative owned surface | Visibility/constructor rule |
|---|---|---|
| `arcweft-lang-sema::callable::identity` | `TtsCallableId`, `BuiltinCallableId::Tts` | Public IDs; constructed only by shared resolver. |
| `arcweft-core::value` | `RuntimeExpr::NominalRecord`, canonical Result constructors, canonical value decoder | Generic only; no TTS constants or variants. |
| `arcweft-core::value::nominal_record` | checked nominal record construction/shape | `new_unchecked` crate-private; `try_new` public and checked. |
| `arcweft-core::task` | generic intent/template/spec/outcome/registration carriers; typed `TaskEventKind::Err` | Fields private; checked constructors; no TTS name. |
| `arcweft-core::awbc` | codec-8 generic typed task request/outcome wire | Verifier constructs runtime forms; no string TTS branch. |
| `arcweft-audio-tts` | accepted intents, selected requests, fingerprints, errors, progress, assets, provider protocol | Accepted lower constructors remain normative. `task_key_text` is inherent on selected request. |
| `arcweft-audio-tts-runtime` | five nominal payload wrappers, layouts, recursive ordinal codecs | Tuple fields private. No serde-derived JSON envelope. |
| `arcweft-runtime-plan` | selected callable-to-intent lowering | Consumes `TtsCallableId`; cannot match source name text. |
| `arcweft-runtime-driver::tts` | snapshot, atomic preparation, typed host dispatch conversion, reload comparison | Combines catalog, availability, generation, generic registration ID, and core task; has no host-adapter dependency. |
| `arcweft-runtime-scheduler` | admission/join/observer cancellation/event cloning | Generic contracts only; inherent methods on scheduler. |
| `arcweft-host-adapter::tts` | typed registration, selected-request submit, domain progress/result/error updates | No bridge dependency, intent constructor, TTS RuntimePayload API, or driver type. |
| `arcweft-runtime-host` | typed dispatch/token match, call-ID/context construction, provider-I/O pump, domain outcome-to-payload encoding | Validates registration and selected stage before privilege/I/O. |
| `arcweft-runtime-driver::session::replay` | schema-1 task external outcome capture/injection | Existing vector only; typed payloads. |
| `arcweft-runtime-driver::session_save` | existing blockers | No TTS-specific active-state variant. |
| `arcweft-bundle` | accepted catalog/profile sections 22/23 | Lower accepted package unchanged. |

## 4. Exact constructor and API signatures

```rust
// arcweft-core
impl RuntimeTaskIntentTemplate {
    pub fn new(payload: RuntimeExpr) -> Self;
}

impl NominalPayloadContract {
    pub fn new(
        type_id: RuntimeNominalTypeId,
        layout: TypeLayoutHash,
        field_count: u16,
        max_canonical_bytes: u32,
    ) -> Self;
}

impl SchemaPayloadContract {
    pub fn try_new(
        schema: RuntimeTypeSchema,
        limits: RuntimeSchemaLimits,
    ) -> Result<Self, RuntimePayloadContractError>;
}

impl RuntimePayloadContract {
    pub fn validate(
        &self,
        payload: &RuntimePayload,
    ) -> Result<(), RuntimePayloadContractError>;
}

impl TaskOutcomeContract {
    pub fn try_new(
        ready: RuntimePayloadContract,
        error: RuntimePayloadContract,
        progress: Option<RuntimePayloadContract>,
        cancellation: TaskCancellationContract,
    ) -> Result<Self, TaskOutcomeContractError>;
}

impl RegisteredHostTaskRequest {
    pub fn new(
        registration: HostAdapterRegistrationId,
        payload: RuntimePayload,
    ) -> Self;
}

impl TaskSpec {
    pub fn new(
        id: TaskId,
        key: TaskKey,
        class: TaskClass,
        priority: TaskPriority,
        cancel_scope: CancelScopeId,
        policy: TaskPolicy,
        request: HostTaskRequest,
        outcome: TaskOutcomeContract,
        debug_label: String,
    ) -> Self;
}

// arcweft-audio-tts-runtime
impl TtsIntentPayload {
    pub fn template(
        selector: RuntimeExpr,
        text: RuntimeExpr,
        locale: RuntimeExpr,
        options: RuntimeExpr,
    ) -> RuntimeTaskIntentTemplate;
    pub fn encode(value: &TtsSynthesisIntent) -> Result<Self, TtsPayloadEncodeError>;
    pub fn decode(payload: RuntimePayload) -> Result<TtsSynthesisIntent, TtsPayloadDecodeError>;
    pub fn contract() -> NominalPayloadContract;
    pub fn into_runtime(self) -> RuntimePayload;
}

// selected/progress/asset/error wrappers expose the same encode/decode/contract/into_runtime set.

// arcweft-audio-tts
impl TtsAdapterCallId {
    pub fn from_generation_sequence(generation: u64, sequence: u64) -> Self;
}

`from_generation_sequence` is the accepted fixed-width constructor: its exact
16 bytes are `generation.to_le_bytes()` followed by `sequence.to_le_bytes()`.
It performs no hashing, textual formatting, truncation, or host allocation.

// arcweft-runtime-driver
impl TtsRuntimeTaskPreparer {
    pub fn prepare_task(&self, intent: TaskIntentSpec)
        -> Result<TaskSpec, TtsPreparationFailure>;
}

impl BundleSession {
    pub fn dispatch_requested_tasks(
        &mut self,
        clock: RuntimeClock,
        requested: Vec<RuntimeRequestedTask>,
    ) -> Result<Vec<HostTaskDispatch>, BundleSessionTaskError>;
}

// arcweft-runtime-scheduler
impl RuntimeScheduler {
    pub fn submit_one(
        &mut self,
        task: TaskSpec,
    ) -> Result<TaskAdmission, TaskAdmissionError>;

    pub fn replace_pending_in_place(
        &mut self,
        execution: &TaskId,
        expected_key: &TaskKey,
        replacement: TaskSpec,
    ) -> Result<(), PendingTaskReplacementError>;
}

// arcweft-host-adapter
impl TtsHostAdapterRegistration {
    pub fn id(&self) -> HostAdapterRegistrationId;
}

impl TtsHostTaskContext {
    pub fn new(
        task_id: TaskId,
        key: TaskKey,
        logical_epoch: LogicalEpoch,
        sequence: TaskSequence,
        cancel_scope: CancelScopeId,
        call: TtsAdapterCallId,
    ) -> Self;
}

impl HostAdapterRegistryBuilder {
    pub fn register_tts_synthesize(
        &mut self,
        adapter: Arc<TtsHostAdapter>,
        accepted: &TtsAcceptedCatalog,
    ) -> Result<TtsHostAdapterRegistration, HostAdapterError>;
}

impl HostAdapterRegistry {
    pub fn submit_tts(
        &self,
        registration: &TtsHostAdapterRegistration,
        context: TtsHostTaskContext,
        request: TtsSynthesisRequest,
    ) -> Result<TtsHostTaskSubmission, HostAdapterError>;

    pub fn drain_tts_updates(
        &self,
        registration: &TtsHostAdapterRegistration,
    ) -> Result<Vec<TtsHostAdapterCompletion>, HostAdapterError>;

    pub fn cancel_tts(
        &self,
        registration: &TtsHostAdapterRegistration,
        task_id: &TaskId,
    ) -> Result<bool, HostAdapterError>;

    pub fn pump_tts_main_thread(
        &self,
        registration: &TtsHostAdapterRegistration,
    ) -> Result<(), HostAdapterError>;
}
```

`HostAdapterError` is extended in its existing owner. New core, scheduler, and
driver errors are defined directly in their owning modules; no local wrapper error
is introduced:

```rust
#[repr(u8)]
pub enum TtsRegistrationCoordinate {
    ProviderId = 0,
    ArtifactIdentity = 1,
    AbiHash = 2,
    CapabilityDigest = 3,
    PublicConfigDigest = 4,
    ProtocolId = 5,
}

pub enum HostAdapterError {
    // pre-existing variants remain unchanged
    DuplicateTtsSynthesize,
    TtsAcceptedMismatch { coordinate: TtsRegistrationCoordinate },
    TtsRegistrationMissing { registration: HostAdapterRegistrationId },
    TtsRegistrationMismatch {
        expected: HostAdapterRegistrationId,
        actual: HostAdapterRegistrationId,
    },
}

pub enum TaskAdmissionError {
    ContractMismatch { execution: TaskId, observer: TaskId },
}

pub enum PendingTaskImmutableField {
    TaskId,
    TaskKey,
    Class,
    Priority,
    CancelScope,
    Policy,
    Outcome,
}

pub enum PendingTaskReplacementError {
    NotPending { execution: TaskId },
    KeyMismatch { expected: TaskKey, actual: TaskKey },
    ImmutableFieldChanged { field: PendingTaskImmutableField },
}

pub enum TaskOutcomeContractError {
    CancellationPayload { source: RuntimePayloadContractError },
}

pub enum RuntimeTaskRegistryError {
    InvalidTerminalError { source: RuntimePayloadContractError },
    DuplicateTerminalTask { task: TaskId },
}
```

## 5. Stage visibility proof obligations

| Crate | May name intent domain type | May construct intent payload | May prepare selected request | May receive selected request | May construct provider request |
|---|---:|---:|---:|---:|---:|
| core | no | no | no | no | no |
| audio TTS | yes | no | yes, catalog inherent | yes | yes, host-policy API only |
| bridge | yes | yes | no | codec only | no |
| runtime-plan | no domain value; expression only | template only | no | no | no |
| runtime-driver | yes via bridge decode | no authored construction | yes, orchestration | yes | no credential value |
| scheduler | no | no | no | opaque payload only | no |
| host adapter | no intent API | no | no | yes, typed | yes, credential slot only |
| provider adapter | no | no | no | no | yes, credential slot only |

Compile-fail tests must prove that core cannot import audio TTS, host-adapter
cannot import the bridge, call any intent constructor, accept a driver dispatch,
or accept a TTS `RuntimePayload`, and provider adapters cannot accept
`RuntimePayload` in the TTS executor trait.
