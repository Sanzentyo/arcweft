# Request, result, and adapter protocol

## 1. Three-stage Sans-I/O model

The final model distinguishes authored intent, a selected runtime task, and the
provider wire request. These are separate nominal records; no stage is encoded
as an optional-field state machine.

```text
TtsSynthesisIntent
    -- profile/Character/catalog/provider selection -->
TtsSynthesisRequest
    -- host policy + credential lease + adapter call allocation -->
TtsProviderSynthesisRequest
```

The compiler lowers the three ordinary functions to
`HostTaskRequestTemplate::TtsSynthesis(TtsSynthesisIntentTemplate)`. Core
evaluates that typed template to
`HostTaskRequest::TtsSynthesisIntent(TtsSynthesisIntent)`; it never enters the
current string dispatcher in `arcweft-core::engine::suspend::
lower_evaluated_host_request`.

Core may construct the existing `TaskSpec` with its current task-local key, but
that spec is an internal unprepared value and is not schedulable or host-visible.
At `BundleSession::dispatch_requested_tasks`, before generation-pin/task-registry
publication, runtime-driver calls
`TtsAcceptedCatalog::prepare_request(intent, availability, generation)` and then
the inherent method:

```rust
impl TaskSpec {
    pub fn prepare_tts(
        self,
        request: TtsSynthesisRequest,
    ) -> Result<Self, TaskPreparationError>;
}
```

The method requires the intent variant, preserves task ID/priority/cancel scope,
sets class `TtsSynthesis`, policy `JoinSameKey`, final `TaskKey::for_tts`, final
redacted debug label, and replaces the request with
`HostTaskRequest::TtsSynthesis(request)`. The host-adapter API accepts only the
final variant. Preparation failure is registered as a terminal typed task error
for the same logical epoch/sequence and produces no `HostTaskDispatch`. This is
a two-stage typed pipeline, not a compatibility dual reader or optional-field
state machine.

## 2. Authored/runtime intent

```rust
pub struct TtsSynthesisIntent {
    pub selector: TtsProfileSelector,
    pub text: Arc<str>,
    pub locale: Option<TtsLocaleId>,
    pub options: TtsSynthesisOptions,
}

pub struct TtsSynthesisOptions {
    pub style: Option<TtsStyleId>,
    pub rate_milli: Option<TtsRateMilli>,
    pub pitch_cents: Option<TtsPitchCents>,
    pub format: Option<AudioFormat>,
    pub extensions: Vec<TtsExtensionOption>,
    pub timeout: TtsTimeoutMillis,
    pub retry: TtsRetryPolicy,
}
```

`TtsSynthesisIntent`, `TtsSynthesisRequest`, and
`TtsProviderSynthesisRequest` use custom `Debug`; they print selector/public
option metadata and text byte/scalar counts, never text, provider key, SecretRef,
credential slot, or restricted digests. They do not derive field-wise `Debug`.

Validation is exact and occurs before selection:

- text is nonempty and at most 65,536 bytes / 16,384 scalars;
- text bytes are retained exactly; no trim, newline rewrite, Unicode
  normalization, SSML interpretation, or display-name substitution occurs;
- extension option IDs are sorted unique after source-map-aware duplicate
  detection, at most 16, and at most 4,096 encoded bytes total;
- timeout is 1,000–120,000 ms, default 30,000 ms;
- rate and pitch use their nominal constructors; values are never clamped;
- the requested format must be one of the existing `AudioFormat` variants.

## 3. Fully selected scheduler request

```rust
pub struct TtsSynthesisRequest {
    pub fingerprint: TtsRequestFingerprint,
    pub selection: TtsSelectionEvidence,
    pub text: Arc<str>,
    pub locale: TtsLocaleId,
    pub style: Option<TtsStyleId>,
    pub rate_milli: TtsRateMilli,
    pub pitch_cents: TtsPitchCents,
    pub output_format: AudioFormat,
    pub extensions: Vec<TtsExtensionOption>,
    pub timeout: TtsTimeoutMillis,
    pub retry: TtsRetryPolicy,
}

pub struct TtsSelectionEvidence {
    pub profile: Option<TtsVoiceProfileId>,
    pub character: Option<CharacterId>,
    pub tts_speaker: TtsSpeakerId,
    pub provider: TtsProviderId,
    pub binding: TtsProviderBindingId,
    pub provider_key: TtsProviderSpeakerKey,
    pub provider_key_digest: TtsProviderKeyDigest,
    pub profile_catalog_digest: TtsCatalogDigest,
    pub provider_catalog_digest: TtsCatalogDigest,
    pub availability_digest: TtsAvailabilityDigest,
    pub adapter_artifact: TtsArtifactIdentity,
    pub adapter_abi: TtsAbiDigest,
    pub capability_digest: TtsCapabilityDigest,
    pub public_config_digest: TtsConfigDigest,
    pub accepted_generation: u64,
}
```

`provider_key` is present because a provider call requires it. It is a
restricted field with redacted formatting and is removed from every source,
result receipt, ordinary debug, save summary, and Agent projection.

### Task identity

`TtsSynthesisRequest::fingerprint()` computes BLAKE3 with context
`arcweft-tts-request-v1` over the canonical request encoding excluding the
`fingerprint` field itself. The encoding includes:

- selected profile/Character presence and IDs;
- logical speaker, provider, binding, and provider-key digest;
- exact text bytes;
- locale, style, rate, pitch, format, and extension options;
- profile/provider/availability/capability/config/artifact/ABI digests;
- timeout and retry policy;
- accepted generation only through its catalog/artifact digests, not as an
  independent entropy source.

It excludes TaskId, NeedId, cancel scope, priority, queue position, attempt
number, wall-clock time, progress, provider trace ID, credential lease ID,
output bytes, and output digest.

The existing owner receives:

```rust
impl TaskKey {
    pub fn for_tts(request: &TtsSynthesisRequest) -> Self {
        // exact text: tts.v1.<64 lowercase hex fingerprint>
    }
}
```

The source functions use `TaskPolicy::JoinSameKey`. Two requests join only when
all deterministic semantic and selected-artifact inputs above are equal.

## 4. Provider request

```rust
pub struct TtsProviderSynthesisRequest {
    pub protocol: TtsProviderProtocolId,
    pub call: TtsAdapterCallId,
    pub request_fingerprint: TtsRequestFingerprint,
    pub provider: TtsProviderId,
    pub provider_key: TtsProviderSpeakerKey,
    pub text: Arc<str>,
    pub locale: TtsLocaleId,
    pub style: Option<TtsStyleId>,
    pub rate_milli: TtsRateMilli,
    pub pitch_cents: TtsPitchCents,
    pub output_format: AudioFormat,
    pub extensions: Vec<TtsExtensionOption>,
    pub timeout_millis: u32,
    pub attempt: u8,
    pub credential_slot: Option<TtsCredentialSlotId>,
}
```

`attempt` is one-based and is not part of request identity. A credential slot
identifies a host-owned lease; the credential value is never encoded in AWTP.

## 5. Progress

```rust
pub enum TtsProgressPhase {
    Queued = 0,
    Connecting = 1,
    Synthesizing = 2,
    Receiving = 3,
    Validating = 4,
}

pub struct TtsProgress {
    pub phase: TtsProgressPhase,
    pub completed_basis_points: Option<u16>, // 0..=10_000
    pub received_bytes: Option<u32>,
}
```

Progress is monotonic by phase and, when present, by numeric value. At most 128
progress events are published per request; adjacent events in the same phase
are coalesced to the highest values. Progress contains no text, provider key,
credential, provider trace payload, or raw provider message.

## 6. Complete audio result

```rust
pub struct TtsAudioAsset {
    pub format: AudioFormat,
    pub bytes: Arc<[u8]>,
    pub content_digest: TtsAudioDigest,
    pub duration_millis: u32,
    pub sample_rate_hz: u32,
    pub channels: u8,
    pub receipt: TtsPublicReceipt,
}

pub struct TtsPublicReceipt {
    pub profile: Option<TtsVoiceProfileId>,
    pub character: Option<CharacterId>,
    pub tts_speaker: TtsSpeakerId,
    pub locale: TtsLocaleId,
    pub style: Option<TtsStyleId>,
    pub rate_milli: TtsRateMilli,
    pub pitch_cents: TtsPitchCents,
    pub format: AudioFormat,
}
```

`TtsAudioAsset` also uses custom `Debug`: format, byte count, duration, sample
rate, channels, and public receipt are visible; encoded bytes and content digest
are not. It does not derive field-wise `Debug`.

The result is published only after:

1. exact provider-protocol completion;
2. encoded-byte count and content digest validation;
3. format match;
4. complete decode/probe using `arcweft-audio-codec` under the TTS limits;
5. nonempty duration, sample-rate, and mono/stereo validation;
6. spool finalization and cleanup.

The provider's duration/sample metadata is advisory; accepted result metadata is
computed from the validated bytes. Provider bytes are inherently
nondeterministic unless a provider separately guarantees otherwise. The task
fingerprint is reproducible; the result content digest records what was
observed and does not claim that a repeated external call will reproduce bytes.

`TtsAudioAsset` is not automatically installed into an `AudioGraph` and does
not select an `AudioVoiceId`, bus, device, gain, pan, or playback policy.
`TtsAudioDigest` is BLAKE3 context `arcweft-tts-audio-v1` over the complete
accepted encoded bytes.

## 7. Structured error model

The source/runtime nominal type is `std.audio.TtsError` and maps exactly to:

```rust
pub enum TtsError {
    InvalidIdentity { kind: TtsIdentityKind },
    UnknownProfile { profile: TtsVoiceProfileId },
    MissingCharacterProfile { character: CharacterId },
    CharacterProfileMismatch { character: CharacterId, profile: TtsVoiceProfileId },
    UnknownSpeaker { tts_speaker: TtsSpeakerId },
    UnknownProvider { provider: TtsProviderId },
    UnsupportedLocale { tts_speaker: TtsSpeakerId, locale: TtsLocaleId },
    UnsupportedStyle { tts_speaker: TtsSpeakerId, style: TtsStyleId },
    UnsupportedOption { option: TtsOptionId },
    UnsupportedFormat { format: AudioFormat },
    CapabilityUnavailable,
    ProviderUnavailable { provider: TtsProviderId },
    QueueLimit { maximum: u16 },
    RequestLimit { maximum_bytes: u32, actual_bytes: u32 },
    Timeout { timeout_millis: u32 },
    Cancelled,
    CatalogChanged,
    ProviderFailure(TtsProviderFailure),
    ProtocolFailure { stage: TtsProtocolStage, code: TtsProtocolFailureCode },
    ResultTooLarge { maximum_bytes: u32, observed_bytes: u64 },
    InvalidAudio { kind: TtsInvalidAudioKind },
}

pub struct TtsProviderFailure {
    pub provider: TtsProviderId,
    pub class: TtsProviderFailureClass,
    pub code: TtsProviderErrorCode,
    pub message: TtsSanitizedProviderMessage,
    pub retryable: bool,
    pub request: TtsRequestFingerprint,
    pub restricted_payload_digest: Option<TtsProviderPayloadDigest>,
}
```

Provider failure classes are `Authentication`, `Authorization`, `RateLimited`,
`InvalidRequest`, `Unavailable`, `Transport`, `Internal`, and `Unknown` with
fixed discriminants 0–7. Provider codes are at most 64 printable ASCII bytes.
Messages are sanitized UTF-8 at most 1,024 bytes, with control characters,
credential values, provider keys, and request-text substrings removed. A
restricted provider payload may be retained only up to 4,096 bytes in a
host-private ring and is represented elsewhere solely by a digest.

The direct generic boundary correction is:

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
```

There is no string error fallback. Existing adapters publish their own nominal
host errors or the shared generic host-error nominal record.

## 8. Host adapter trait

The existing `HostAdapter` trait remains the scheduler-facing interface.
`TtsHostAdapter` claims exactly `tts.synthesize` and delegates provider work to:

```rust
pub trait TtsProviderExecutor: Send + Sync + core::fmt::Debug {
    fn accepted_provider(&self) -> &AcceptedTtsProviderInstance;
    fn submit(
        &self,
        request: TtsProviderSynthesisRequest,
        credential: Option<CredentialLease>,
    ) -> Result<TtsProviderSubmission, TtsAdapterError>;
    fn drain_events(&self) -> Vec<TtsProviderEvent>;
    fn cancel(&self, call: TtsAdapterCallId) -> bool;
    fn abort(&self, call: TtsAdapterCallId);
}

pub enum TtsProviderSubmission {
    Completed(Vec<TtsProviderEvent>),
    Pending,
}

pub enum TtsProviderEvent {
    Accepted { call: TtsAdapterCallId },
    Progress { call: TtsAdapterCallId, progress: TtsProgress },
    AudioChunk { call: TtsAdapterCallId, sequence: u32, bytes: Arc<[u8]> },
    Completed { call: TtsAdapterCallId, chunk_count: u32, total_bytes: u64,
                content_digest: TtsAudioDigest },
    Failed { call: TtsAdapterCallId, failure: TtsProviderFailureWire },
    Cancelled { call: TtsAdapterCallId },
}
```

`accepted_provider()` must match the catalog's artifact, ABI, provider,
capability, config, and protocol digests before registry publication. One
provider/export pair has one executor owner. Duplicate ownership is rejected by
the existing registry builder semantics.

## 9. Dispatch state machine

```text
Prepared
  -> Queued
  -> Starting
  -> Accepted
  -> Receiving*
  -> Validating
  -> Ready
```

Terminal alternatives are `Err`, `Cancelled`, or `Timeout`. Events after a
terminal state are discarded and counted. `Completed` before `Accepted`, gaps
or repeats in chunk sequence, progress regression, a second terminal event,
digest/size mismatch, or trailing frames are protocol failures.

### Buffering

- up to 4 MiB: host-owned memory buffer;
- above 4 MiB: host-owned private temporary spool;
- absolute encoded result maximum: 32 MiB;
- spool files are opened through the host temp owner, are not addressable from
  source, and are deleted on success after materialization, error, cancellation,
  timeout, adapter loss, or host shutdown.

## 10. Timeout, retry, and cancellation

### Timeout

The timeout includes queue wait, connection, synthesis, receive, and
validation. The host clock is the only clock. A timeout emits exactly one typed
`TtsError::Timeout`, cancels the executor, waits at most 5,000 ms for cleanup,
then calls `abort` and discards later events.

### Retry

`TtsRetryPolicy` is:

```rust
pub struct TtsRetryPolicy {
    pub maximum_attempts: u8, // 1..=3, default 2
}
```

Retry is permitted only when generated metadata advertises
`RequestFingerprint` idempotency and the sanitized failure class is
`RateLimited`, `Unavailable`, or `Transport` with `retryable = true`. Attempt 2
starts after 250 ms and attempt 3 after an additional 1,000 ms, both inside the
original timeout. Retry uses the same provider, binding, provider key, options,
credential reference, request fingerprint, and adapter artifact. Partial bytes
from a failed attempt are destroyed before retry.

### Cancellation

- queued: remove immediately, release generation pin, emit `Cancelled`;
- active cooperative/abortable provider: send one cancel, begin cleanup;
- provider advertising no cancellation: stop accepting events, close/kill the
  transport at cleanup deadline;
- credential leases and buffers are released in every path;
- cancellation is never converted to provider failure and is never retried.

## 11. Replay and debug

Recording captures the selected request fingerprint and the actual terminal
external outcome. Success records the complete nominal `TtsAudioAsset`,
including bytes, subject to replay budgets. Replay injects those exact bytes
through the existing external-outcome path and suppresses provider dispatch.
It never contacts a provider to fill missing replay bytes.

Ordinary debug label is exactly:

```text
tts.synthesize text_bytes=<n> locale=<locale> format=<format>
```

It contains no text, Character, profile, speaker, provider, provider key,
credential reference, or result digest. Privileged audio debugging may add
profile/speaker/provider and the first 12 fingerprint hex characters, but never
raw provider key, credential, text, or audio bytes.
