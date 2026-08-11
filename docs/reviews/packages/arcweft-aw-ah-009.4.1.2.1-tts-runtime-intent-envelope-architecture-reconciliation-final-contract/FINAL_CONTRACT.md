# Final contract

## 1. Status

```text
STATUS=READY_FOR_IMPLEMENTATION
OPEN_RESULT_CHANGING_DECISIONS=0
OPEN_QUESTIONS=0
IMPLEMENTATION_PERFORMED=NO
CURRENT_MAIN=15cf571416245e1530c0d9902ab3ff6befbdb39e
```

The accepted lower model remains normative. This correction replaces only its
forbidden `arcweft-core -> arcweft-audio-tts` edge, core-owned TTS request
variants, preselection task key, and string operation dispatch.

## 2. Closed architecture decisions

### C1 — executable intent owner and representation

`arcweft-audio-tts-runtime` is a new narrow Sans-I/O composition crate. It
depends directly on `arcweft-core` and `arcweft-audio-tts` and owns the sole
conversion between the accepted domain types and nominal `RuntimePayload`.

The preselection executable value is exactly nominal type
`std.audio.TtsSynthesisIntent`, layout
`79a77138c3c4b8b400357865ebc33393f4e277aa0daf07facf5de523d996a0c2`,
with four ordinal fields. It is not a string tag, JSON/TOML document, Rust type
name, `Any`, downcast, task table, compatibility envelope, or TTS-specific core
enum.

### C2 — generic core task stages

`arcweft-core::task` directly replaces the provisional TTS variants with the
following generic model:

```rust
pub enum RuntimeTaskRequestTemplate {
    Host(HostTaskRequestTemplate),
    Intent(RuntimeTaskIntentTemplate),
}

pub struct RuntimeTaskIntentTemplate {
    payload: RuntimeExpr,
}

pub struct RuntimeTaskIntent {
    payload: RuntimePayload,
}

pub struct TaskIntentSpec {
    id: TaskId,
    class: TaskClass,
    priority: TaskPriority,
    cancel_scope: CancelScopeId,
    policy: TaskPolicy,
    intent: RuntimeTaskIntent,
    outcome: TaskOutcomeContract,
}

pub enum RuntimeRequestedTask {
    Prepared(TaskSpec),
    Intent(TaskIntentSpec),
}

pub struct NominalPayloadContract {
    type_id: RuntimeNominalTypeId,
    layout: TypeLayoutHash,
    field_count: u16,
    max_canonical_bytes: u32,
}

pub struct SchemaPayloadContract {
    schema: RuntimeTypeSchema,
    layout: TypeLayoutHash,
    limits: RuntimeSchemaLimits,
}

pub enum RuntimePayloadContract {
    Nominal(NominalPayloadContract),
    Schema(SchemaPayloadContract),
}

pub enum TaskCancellationContract {
    Cancelled,
    Error(RuntimePayload),
}

pub struct TaskOutcomeContract {
    ready: RuntimePayloadContract,
    error: RuntimePayloadContract,
    progress: Option<RuntimePayloadContract>,
    cancellation: TaskCancellationContract,
}
```

All fields above are private. Core exposes read-only accessors and checked
constructors. `RuntimeTaskIntent` and `TaskIntentSpec` have no public constructor
and do not implement `Serialize` or `Deserialize`; only the core evaluator can
materialize them. `TaskIntentSpec` deliberately has no `TaskKey`, host request,
debug label, logical epoch, sequence, generation, registration, or pin.

`HostRequestBatch.tasks` becomes `Vec<RuntimeRequestedTask>`. Existing non-TTS
work uses `Prepared(TaskSpec)`. TTS uses `Intent(TaskIntentSpec)`.

`TaskSpec` retains its existing fields and adds one required
`TaskOutcomeContract`. Its checked constructor takes that contract explicitly;
there is no default, wildcard, `Any`, or implementation-selected outcome shape.
`SchemaPayloadContract` computes and retains an exact schema layout and bounded
validator for non-nominal generic work; TTS uses only `Nominal` contracts.

### C3 — nominal runtime expressions and checked values

Core adds the generic owner behavior:

```rust
pub enum RuntimeExpr {
    // existing variants
    NominalRecord {
        type_id: RuntimeNominalTypeId,
        layout: TypeLayoutHash,
        fields: Vec<RuntimeExpr>,
    },
}

impl RuntimeNominalRecordValue {
    pub(crate) fn new_unchecked(
        type_id: RuntimeNominalTypeId,
        layout: TypeLayoutHash,
        fields: Vec<RuntimeValue>,
    ) -> Self;

    pub fn try_new(
        contract: &NominalPayloadContract,
        fields: Vec<RuntimeValue>,
    ) -> Result<Self, RuntimePayloadContractError>;
}

impl RuntimeValue {
    pub fn result_ok(value: RuntimeValue) -> Self;
    pub fn result_err(error: RuntimeValue) -> Self;
}
```

`result_ok` and `result_err` produce the existing canonical variant shape with
`path = Some("Result")`, names `Ok`/`Err`, and exactly one payload. No local
extension trait or helper duplicates this behavior.

### C4 — bridge-owned payload wrappers

`arcweft-audio-tts-runtime` exposes exactly these wrappers with private fields:

```rust
pub struct TtsIntentPayload(RuntimePayload);
pub struct TtsSelectedRequestPayload(RuntimePayload);
pub struct TtsProgressPayload(RuntimePayload);
pub struct TtsAudioAssetPayload(RuntimePayload);
pub struct TtsErrorPayload(RuntimePayload);

pub struct TtsRuntimeLayouts {
    pub intent: NominalPayloadContract,
    pub selected_request: NominalPayloadContract,
    pub progress: NominalPayloadContract,
    pub audio_asset: NominalPayloadContract,
    pub error: NominalPayloadContract,
}
```

Each wrapper has inherent `encode`, `decode`, `as_runtime`, `into_runtime`, and
`contract` methods. `TtsIntentPayload` additionally has an inherent
`template(selector, text, locale, options) -> RuntimeTaskIntentTemplate` method
that constructs the exact four-ordinal `RuntimeExpr::NominalRecord` used by
runtime-plan. Every decode verifies top-level nominal identity, layout, exact
field count, nested nominal identities/layouts, enum path/name/discriminant,
option path/payload, scalar width, canonical order, and limits before producing
a domain value.

### C5 — source and shared callable registry

`arcweft-lang-sema::callable::identity` adds:

```rust
#[repr(u8)]
pub enum TtsCallableId {
    SynthesizeProfile = 0,
    SynthesizeCharacter = 1,
    SynthesizeSpeaker = 2,
}

pub enum BuiltinCallableId {
    // existing variants
    Tts(TtsCallableId),
}
```

The existing shared callable registry owns the exact accepted signatures:

```text
tts.synthesize_profile(profile, text, locale?, options?)
tts.synthesize_character(character, text, profile?, locale?, options?)
tts.synthesize_speaker(tts_speaker, text, locale, options?)
```

Each returns `Need<TtsAudioAsset, TtsError>` and has the one effect
`tts.synthesize`. Runtime-plan matches `BuiltinCallableId::Tts`, never callable
name text, and lowers exactly one `RuntimeTaskIntentTemplate`. There is no TTS
keyword, statement, source-specific task path, provider callable,
`tts.synthesis`, `voice`, or `speaker` compatibility spelling.

### C6 — preparation owner and API

The accepted lower owner remains unchanged:

```rust
impl TtsAcceptedCatalog {
    pub fn prepare_request(
        &self,
        intent: &TtsSynthesisIntent,
        availability: &TtsProviderAvailabilitySnapshot,
        accepted_generation: u64,
    ) -> Result<TtsSynthesisRequest, TtsError>;

    pub fn rebind_queued_request(
        &self,
        previous: &TtsSynthesisRequest,
        availability: &TtsProviderAvailabilitySnapshot,
        accepted_generation: u64,
    ) -> Result<TtsSynthesisRequest, TtsQueuedReloadError>;
}

#[repr(u8)]
pub enum TtsQueuedCompatibilityCoordinate {
    ProfileSemanticDigest = 0,
    SelectedBindingId = 1,
    ProviderId = 2,
    ProviderKeyDigest = 3,
    CapabilityDigest = 4,
    PublicConfigDigest = 5,
    ArtifactIdentity = 6,
    AbiHash = 7,
    CredentialRef = 8,
    ProtocolId = 9,
}

pub enum TtsQueuedReloadError {
    CompatibilityChanged { coordinate: TtsQueuedCompatibilityCoordinate },
    SelectedProviderUnavailable,
    AvailabilityChanged,
    FingerprintChanged,
}
```

`rebind_queued_request` accepts only a fully validated selected request. It
looks up the same selected profile, binding, provider, and provider speaker in
the candidate accepted catalog, compares the accepted ten-coordinate tuple
above without exposing restricted values, preserves text/options, reconstructs
selection evidence with the candidate generation, then separately requires the
availability digest, accepted fingerprint, and TaskKey to remain byte-identical.
It never accepts or reconstructs a
`TtsSynthesisIntent`. Runtime-driver maps every variant to exact
`TtsError::CatalogChanged` for queued observers.

Runtime integration is owned by `arcweft-runtime-driver::tts`:

```rust
pub struct TtsPreparationSnapshot {
    program_generation: GenerationId,
    accepted_catalog_generation: u64,
    catalog: Arc<TtsAcceptedCatalog>,
    availability: Arc<TtsProviderAvailabilitySnapshot>,
    registration: HostAdapterRegistrationId,
}

pub struct TtsRuntimeTaskPreparer {
    snapshot: Arc<TtsPreparationSnapshot>,
}

pub struct TtsPreparationFailure {
    task_id: TaskId,
    cancel_scope: CancelScopeId,
    outcome: TaskOutcomeContract,
    error: TtsError,
    diagnostic: Option<TtsRuntimeDiagnostic>,
}

impl TtsRuntimeTaskPreparer {
    pub fn prepare_task(
        &self,
        intent: TaskIntentSpec,
    ) -> Result<TaskSpec, TtsPreparationFailure>;
}
```

Runtime-host retains the typed `TtsHostAdapterRegistration` returned by the
registry and passes only its generic `HostAdapterRegistrationId` into
`TtsPreparationSnapshot`. Runtime-driver therefore cannot call host-adapter APIs
or construct a typed token; runtime-host must match both forms before I/O.

`prepare_task` performs, in this order, with no mutation outside local values:

1. validate `TaskClass::TtsSynthesis`, `JoinSameKey`, priority, cancellation
   scope, and exact TTS outcome contracts;
2. decode `TtsIntentPayload`;
3. call `TtsAcceptedCatalog::prepare_request` against the one pinned snapshot;
4. compute the accepted request fingerprint;
5. obtain exact key text from inherent
   `TtsSynthesisRequest::task_key_text()` and pass it to generic
   `TaskKey::try_new`;
6. encode `TtsSelectedRequestPayload`;
7. construct `HostTaskRequest::Registered(RegisteredHostTaskRequest)` with the
   typed registration ID and selected payload;
8. construct the final redacted debug label and final `TaskSpec`.

`TtsSynthesisRequest::task_key_text()` is owned by `arcweft-audio-tts` and
returns exactly `tts.v1.<64 lowercase hex>`. This replaces the prerequisite's
impossible core-owned `TaskKey::for_tts` without changing fingerprint content.

Only after all eight steps succeed may runtime-driver assign logical
sequence, admit the task to the scheduler, publish registry state or a
generation pin, create `HostTaskDispatch`, consult/capture replay, or call host
code.

A failure is atomically published through:

```rust
impl RuntimeTaskRegistry {
    pub fn publish_terminal_error(
        &mut self,
        generation: GenerationId,
        logical_epoch: LogicalEpoch,
        sequence: TaskSequence,
        task: TaskId,
        cancel_scope: CancelScopeId,
        error_contract: &RuntimePayloadContract,
        error: RuntimePayload,
    ) -> Result<TaskEvent, RuntimeTaskRegistryError>;
}
```

The method validates `error` through the supplied generic error contract first,
then inserts one terminal
failed record and queues one `TaskEventKind::Err` for the original `TaskId` in
one mutation. It creates no active record, scheduler admission, host dispatch,
or generation pin. A failure before this commit point leaves no registry
mutation.

Malformed intent payloads map to exact typed
`TtsError::ProtocolFailure { stage: Request, code: InvalidPayload }` plus a
structured diagnostic. Catalog and availability failures retain their exact
accepted `TtsError` variant.

### C7 — final task identity and scheduler policy

Runtime-plan attaches `TaskClass::TtsSynthesis`, `TaskPolicy::JoinSameKey`,
`TaskPriority(0)`, the ordinary source cancellation scope, and exact outcome
contracts before preparation. Runtime-driver attaches the final key and host
registration only after provider/catalog selection. Logical epoch and sequence
are assigned at the admission/failure publication commit point. The execution
owner generation pin is published only for a scheduled execution.

The intent cannot be a final join key because it contains no selected provider,
binding, provider-key digest, profile/provider/availability/capability/config
catalog digests, adapter artifact, ABI, accepted defaults, or selected format.
Two equal intents may select different providers under different accepted
snapshots; therefore any preselection key is invalid by construction.

The sole scheduler adds an inherent admission result:

```rust
pub enum TaskAdmission {
    Scheduled { execution: TaskId },
    Joined { execution: TaskId, observer: TaskId },
}

pub enum TaskAdmissionError {
    ContractMismatch { execution: TaskId, observer: TaskId },
}

impl RuntimeScheduler {
    pub fn submit_one(
        &mut self,
        task: TaskSpec,
    ) -> Result<TaskAdmission, TaskAdmissionError>;
}
```

It stores each observer's `TaskId`, cancel scope, outcome contract, and original
sequence. A same-key admission joins only when class, `JoinSameKey` policy, and
all four outcome/cancellation contracts are exactly equal to the execution
owner; a mismatch is `TaskAdmissionError::ContractMismatch` and publishes
nothing. Priority is intentionally observer-external: when only priority differs,
the request joins and the execution retains the first admitted owner's priority.
Identical selected keys therefore produce one execution, one host dispatch, and
one execution-generation pin. Progress and terminal events are validated once
and cloned to each live observer with that observer's identity/sequence. Joined
observers have no independent host dispatch or generation pin.

### C8 — cancellation without a parallel scheduler

`SchedulerDispatchBatch` directly replaces scope-only host cancellation with:

```rust
pub struct SchedulerDispatchBatch {
    pub tasks: Vec<TaskSpec>,
    pub cancel_tasks: Vec<TaskId>,
}
```

Cancellation detaches only observers whose own scope matches. For TTS, each
detached observer receives the exact payload
`TtsError::Cancelled` through `TaskEventKind::Err`; the bare `Cancelled` event
never reaches a TTS Need. The provider execution is sent one targeted
`cancel_tasks` request only when its final observer is detached. Completion or
progress after the final detach is discarded after host cleanup. Other generic
tasks may retain `TaskCancellationContract::Cancelled`.

### C9 — typed host visibility and one registration path

Core owns only the generic numeric handle:

```rust
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct HostAdapterRegistrationId(NonZeroU32);

pub struct RegisteredHostTaskRequest {
    registration: HostAdapterRegistrationId,
    payload: RuntimePayload,
}
```

`arcweft-host-adapter::tts` owns the only TTS registration and submit API:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TtsHostAdapterRegistration {
    id: HostAdapterRegistrationId,
}

pub struct TtsHostTaskContext {
    task_id: TaskId,
    key: TaskKey,
    logical_epoch: LogicalEpoch,
    sequence: TaskSequence,
    cancel_scope: CancelScopeId,
    call: TtsAdapterCallId,
}

pub enum TtsHostTaskSubmission {
    Completed {
        result: Result<TtsAudioAsset, TtsError>,
        metrics: HostTaskMetrics,
    },
    Pending,
}

pub enum TtsHostTaskUpdate {
    Progress(TtsProgress),
    Completed {
        result: Result<TtsAudioAsset, TtsError>,
        metrics: HostTaskMetrics,
    },
}

pub struct TtsHostAdapterCompletion {
    task_id: TaskId,
    update: TtsHostTaskUpdate,
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

The builder verifies unique ownership and exact accepted artifact, ABI,
provider, capability, public-config, and protocol digests through additions to
the existing `HostAdapterError` owner. No string operation is registered. The
adapter's submit, drain, cancel, and main-thread pump methods
are `pub(crate)`; privileged callers can use only the four typed
`HostAdapterRegistry` methods with the retained token.

Before returning a dispatch to privileged runtime-host code, runtime-driver
validates the selected payload and replaces it with its own selected-only form:

```rust
pub struct TtsHostTaskDispatch {
    pub registration: HostAdapterRegistrationId,
    pub generation: GenerationId,
    pub task_id: TaskId,
    pub key: TaskKey,
    pub logical_epoch: LogicalEpoch,
    pub sequence: TaskSequence,
    pub cancel_scope: CancelScopeId,
    pub request: TtsSynthesisRequest,
}
```

Runtime-host verifies that the numeric ID equals its retained typed registration,
constructs `TtsAdapterCallId` by the accepted generation/sequence inherent API,
constructs `TtsHostTaskContext`, and calls `submit_tts`. The concrete provider
executor receives only the accepted credential-slot-only
`TtsProviderSynthesisRequest`. No host API accepts `TtsSynthesisIntent`,
`TtsIntentPayload`, a generic TTS operation string, an unvalidated selected
payload, or a TTS `RuntimePayload`. `HostTaskRequest::TtsSynthesisIntent`,
`HostTaskRequest::TtsSynthesis`, `TtsRequest`, and the old string branch are
deleted.

### C10 — typed progress, result, error, and Need

The generic carrier correction is exact:

```rust
pub enum TaskEventKind {
    Ready(RuntimePayload),
    Err(RuntimePayload),
    Cancelled,
    Progress(RuntimePayload),
}

pub struct HostTaskOutcome {
    pub result: Result<RuntimePayload, RuntimePayload>,
    pub metrics: HostTaskMetrics,
}

pub enum HostTaskUpdate {
    Progress(RuntimePayload),
    Completed(HostTaskOutcome),
}

pub struct HostAdapterCompletion {
    pub task_id: TaskId,
    pub update: HostTaskUpdate,
}

pub enum FlowEvent {
    // existing variants
    AwaitErr { need: NeedId, error: RuntimePayload },
}
```

Runtime-host encodes domain `TtsProgress`, `TtsAudioAsset`, and `TtsError`
through the bridge. Runtime-driver validates the exact nominal contract before
scheduler completion. A wrong progress/result/error nominal or layout is
rejected, diagnosed, and converted to typed
`TtsError::ProtocolFailure { stage: Completion, code: InvalidPayload }`; no
string projection is produced.

Core `AwaitState` retains its `TaskOutcomeContract`. Progress emits
`AwaitProgress` and remains pending. Ready emits `AwaitReady`, resumes the
ordinary await expression with `Result::Ok(TtsAudioAsset)`, and preserves the
payload. Err emits `AwaitErr`, resumes with `Result::Err(TtsError)`, and
preserves the payload. `try await`/`await?` remain ordinary `Try` lowering over
that Result. Cancellation uses the selected cancellation contract.

### C11 — AWBC, bundle, save, replay, and reload

AWBC is directly replaced at codec version 8. It encodes one typed intent
request and its nominal outcome contracts; it contains no selected provider,
registration, fingerprint, generation pin, credential, or host request. Codec
7 is rejected rather than dual-read. No TTS V2 type exists.

Preselection selector/text/locale/options expressions, task class/policy/
priority/cancel scope, and nominal outcome contracts are executable program
data. Selected provider/binding/key/digests/format/defaults/fingerprint,
registration, attempts, credentials, sequence, generation, and pins are
runtime-only.

Save keeps the existing `HostTasks` and `TaskGenerationPins` blockers. Replay
keeps schema 1 and the one existing `external_outcomes` vector, directly typed
for task outcomes. There is no TTS task log. Replay always prepares and admits
the selected task first, then substitutes the recorded typed host outcome at
the ordinary dispatch seam. Omitted audio bytes are a terminal replay error;
provider fallback is forbidden.

Queued selected requests migrate only when the accepted AW-AH-009.4.1.2
compatibility tuple is byte-equal and
`TtsAcceptedCatalog::rebind_queued_request` reconstructs an identical
fingerprint and TaskKey from the already selected request. No source intent is
retained or re-prepared. Otherwise every observer receives
`TtsError::CatalogChanged` with no host dispatch. Active executions finish
under the old generation pin.

### C12 — stable diagnostics

The correction adds exactly:

```text
tts.runtime.intent-nominal-mismatch
tts.runtime.intent-layout-mismatch
tts.runtime.intent-codec-invalid
tts.runtime.selected-nominal-mismatch
tts.runtime.selected-layout-mismatch
tts.runtime.outcome-nominal-mismatch
tts.runtime.outcome-layout-mismatch
tts.runtime.outcome-codec-invalid
tts.runtime.registration-missing
tts.replay.outcome-payload-invalid
tts.replay.result-bytes-omitted
```

Diagnostics include expected/actual nominal ID, layout, field ordinal, codec
offset, and named limit where applicable. They never include request text,
provider key, credentials, restricted digests, provider payload, audio bytes,
or content digest. Existing accepted lower diagnostics remain unchanged.

## 3. Prohibited implementation outcomes

The implementation must not introduce a core audio dependency, TTS-specific
core task variant, extension trait, `Any`, downcast, string operation dispatch,
JSON/TOML TTS envelope, provider-specific parser, broad facade dependency,
parallel scheduler, parallel TTS replay log, unprepared host/replay state,
compatibility alias, V2 solely for the provisional layout, source-text gate, or
old `tts.synthesis`/`voice`/`speaker` surface.
