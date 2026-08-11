# Capability, privacy, and diagnostics

## 1. Capability names

The capability and host-call IDs are exact lowercase dotted strings:

| ID | Kind | Who may request it | Purpose |
|---|---|---|---|
| `tts.synthesize` | source-visible effect and host call | typed TTS standard functions | Authorizes one selected synthesis task. |
| `tts.provider.execute` | host deployment capability | `TtsHostAdapter` only | Authorizes execution of an accepted provider export. Not source-callable. |
| `secret.resolve` | host deployment capability | `TtsHostAdapter` through `SecretResolver` | Resolves a `SecretRef` to a nonserializable lease. |
| `net.client` | provider transport capability | concrete provider executor | Authorizes outbound provider network I/O. |
| `process.spawn` | provider transport capability | process provider executor | Authorizes the accepted adapter artifact process. |
| `wasm.execute` | provider transport capability | Wasm provider executor | Authorizes the accepted Wasm artifact. |

A Rust in-process provider requires `tts.provider.execute`, `net.client` when it
performs network I/O, and `secret.resolve` when metadata requires credentials.
A Wasm provider additionally requires `wasm.execute`; a process provider
additionally requires `process.spawn`. These deployment capabilities are not
implied merely because source uses `tts.synthesize`.

## 2. Policy check order

Before provider I/O, the implementation MUST validate in this order:

1. the source call was type checked and has effect `tts.synthesize`;
2. the active launch profile authorizes host call `tts.synthesize`;
3. the profile/Character/speaker mapping is accepted for the pinned generation;
4. provider binding, locale, style, rate, pitch, format, and extension options
   are supported;
5. provider/export artifact, protocol, metadata, ABI, and capability digests
   match the accepted catalog;
6. one concrete executor uniquely owns the provider/export;
7. `tts.provider.execute` and transport-specific deployment capabilities are
   authorized;
8. the provider is available in the selected availability snapshot;
9. global/per-provider active and queue limits allow submission;
10. when required, `secret.resolve` is authorized and the `SecretRef` resolves
    to a lease;
11. timeout/cancellation state still permits dispatch;
12. only then is provider I/O started.

Failure at any step produces a typed TTS error and no partial network/process/
SDK operation.

## 3. Credential and cleanup contract

```rust
pub trait SecretResolver: Send + Sync + core::fmt::Debug {
    fn acquire(&self, reference: &SecretRef) -> Result<CredentialLease, SecretResolveError>;
}
```

`CredentialLease`:

- is host-only, non-`Clone`, non-`Serialize`, and non-`Deserialize`;
- has redacted `Debug` and no `Display`;
- exposes bytes only to the concrete executor through a scoped callback;
- is released on completion, provider failure, protocol failure, cancellation,
  timeout, retry transition, adapter loss, catalog retirement, and shutdown;
- may not be placed in `RuntimeValue`, `TaskSpec`, bundle, save, replay, source
  map, diagnostic field, metric label, trace span, or Agent/MCP payload.

Rust/Wasm executors receive a lease handle. A process executor receives a
one-shot inherited secret channel owned by the host; AWTP carries only the
credential slot ID. The child cannot reopen the reference. On cleanup the host
closes the channel, zeroizes owned buffers where supported by the host secret
backend, deletes temporary audio spools, and terminates an unresponsive child
after the fixed cleanup deadline.

## 4. Concurrency and rate limits

`TtsHostAdapter` owns one deterministic queue ordered by:

```text
TaskPriority descending, scheduler logical epoch ascending,
TaskSequence ascending, TaskId ascending
```

Limits are:

- 32 active requests globally;
- per-provider active limit `min(metadata.max_concurrent_requests,
  host_configured_limit)`, with host default 8;
- 256 queued requests;
- provider rate-limit state is host-only and keyed by accepted provider ID and
  artifact identity;
- no lower catalog or runtime model reads a clock or maintains token buckets.

Rate-limit backoff follows the fixed retry schedule. Queue overflow fails the
new request; it never evicts an earlier task.

## 5. Privacy classification

| Data | Class | Source value | Bundle | Save/replay | Debug | Agent/MCP/accessibility/capture |
|---|---|---|---|---|---|---|
| `CharacterId` | public semantic | yes when caller supplied it | profile catalog binding | public receipt when retained | privileged or domain-appropriate | existing Character projection only; never inferred from TTS |
| `TtsVoiceProfileId` | public semantic | yes | profile catalog | result receipt | privileged audio debug | omitted by default; may be exposed only by explicit audio-inspection API |
| `TtsSpeakerId` | public semantic | yes | profile and restricted provider catalogs | result receipt | privileged audio debug | omitted by dialogue/accessibility/capture; no inference |
| `TtsProviderId` | restricted artifact | no source constructor | restricted provider catalog | request replay metadata only when privileged | privileged only | never default Agent/MCP text |
| `TtsProviderBindingId` | restricted artifact | no | restricted provider catalog | request trace only | privileged only | never |
| provider speaker key | restricted opaque | never | restricted provider catalog | never in result/save/public replay projection | always redacted | never |
| provider-key digest | restricted equality signal | never | restricted provider catalog | privileged replay binding | first 12 hex only under privileged mode | never |
| `SecretRef` | secret locator | manifest only | restricted provider catalog | never save/replay | redacted | never |
| credential value/lease | secret | never | never | never | never | never |
| request text | content-sensitive | function argument | never profile/provider catalog | recorded only when external outcome/result contract needs it; default trace stores count+request digest | counts only by default | never unless a separate explicit content-debug policy grants exact content |
| encoded audio bytes | content-sensitive | result value | never static catalog | complete result only, under limits | counts/format/duration only | no bytes; no automatic transcript |
| result content digest | restricted equality signal | inside nominal asset, not printed | no catalog | yes for validation | privileged only | omitted by default |
| provider error code/message | restricted sanitized | typed error | no | typed outcome | bounded/sanitized | class and safe summary only |
| raw provider response/trace | restricted/possibly secret | never | never | never | host-private bounded ring only | never |

Provider keys are not credentials, but they are still restricted and may reveal
provider catalog structure. Artifact access control, not a new encryption
scheme, protects the restricted catalog. This contract does not introduce a
second encrypted manifest format.

## 6. Projection rules

- Dialogue character display name resolution never queries a TTS profile or
  provider.
- View evaluation never performs TTS discovery or synthesis.
- Dialogue save/restore never serializes provider ID, binding ID, provider key,
  credential reference, availability, progress, or adapter call ID.
- Agent/MCP/accessibility/capture never infers voice identity from Character,
  display name, provider result, or audio playback.
- A TTS Need may be inspected through an explicit audio task/debug API, subject
  to the table above; it is not merged into ordinary dialogue observation.
- Missing TTS capability cannot invalidate accepted CharacterDialogue or View
  state.

## 7. Stable diagnostics

All diagnostic codes below are stable. `owner` is the crate/module that creates
the structured diagnostic. `location` is the primary source/runtime location.
Fields marked restricted are retained structurally but are redacted by default
formatters.

| Code | Owner | Location | Structured fields |
|---|---|---|---|
| `tts.identity.invalid-speaker` | `arcweft-audio-tts::identity` | source profile field / manifest binding | `kind`, `reason`, `range?` |
| `tts.identity.wrong-speaker-family` | same | source/manifest value | `expected_family=tts_speaker`, `actual_family`, `range?` |
| `tts.identity.oversized-speaker` | same | source/manifest value | `maximum_bytes=256`, `actual_bytes`, `maximum_scalars=256`, `actual_scalars`, `range?` |
| `tts.identity.invalid-profile` | same | resource public ID | `reason`, `range?` |
| `tts.identity.invalid-provider` | same | manifest/metadata provider ID | `reason`, `range?`, `metadata_export?` |
| `tts.identity.invalid-provider-key` | same | manifest `provider-key` range | `reason`, `range`; never value |
| `tts.identity.oversized-provider-key` | same | manifest `provider-key` range | `maximum_bytes=256`, `actual_bytes`, `maximum_scalars=128`, `actual_scalars`, `range`; never value |
| `tts.catalog.duplicate-provider` | manifest accepted join | later provider table | `provider`, `first_range`, `later_range` |
| `tts.catalog.duplicate-binding` | same | later binding table | `binding`, `first_range`, `later_range` |
| `tts.catalog.duplicate-provider-key` | accepted provider catalog | later binding key range | `provider`, `first_binding`, `later_binding`, `first_range`, `later_range`; key redacted |
| `tts.catalog.duplicate-speaker-priority` | accepted provider catalog | later priority | `tts_speaker`, `priority`, `first_binding`, `later_binding`, ranges |
| `tts.catalog.unknown-provider` | accepted join / catalog preparation | manifest provider field or runtime task | `provider`, `binding?`, `range?` |
| `tts.catalog.unknown-speaker` | accepted join / catalog preparation | source/profile/binding | `tts_speaker`, `profile?`, `binding?`, `range?` |
| `tts.catalog.character-profile-mismatch` | profile catalog / catalog preparation | profile `character` or runtime call | `character`, `profile`, `bound_character?`, `range?` |
| `tts.catalog.missing-character-default` | profile catalog | Character/profile group | `character`, `profiles`, `ranges` |
| `tts.catalog.duplicate-character-priority` | profile catalog | later resource priority | `character`, `priority`, `first_profile`, `later_profile`, ranges |
| `tts.catalog.mapping-missing` | catalog preparation | ordinary call site | `selector_kind`, `character?`, `profile?`, `tts_speaker?`, `locale?`, `call_range?` |
| `tts.catalog.mapping-ambiguous` | catalog preparation | ordinary call site | `character?`, `locale?`, `candidate_profiles`, `call_range?` |
| `tts.catalog.unsupported-locale` | accepted join / catalog preparation | profile/binding locale or call | `profile?`, `tts_speaker`, `provider?`, `locale`, `range?` |
| `tts.catalog.unsupported-style` | accepted join / catalog preparation | style field or call | `profile?`, `tts_speaker`, `provider?`, `style`, `range?` |
| `tts.catalog.unsupported-option` | catalog preparation | option field | `option`, `provider?`, `range?` |
| `tts.catalog.unsupported-format` | catalog preparation | option field | `format`, `provider_candidates`, `range?` |
| `tts.manifest.secret-value-forbidden` | manifest decoder/model | `credential-ref` value | `range`, `expected=SecretRef`; no value echo |
| `tts.adapter.artifact-mismatch` | accepted join / executor registry | provider table/export | `provider`, `expected_artifact`, `actual_artifact`, `range?` |
| `tts.adapter.abi-mismatch` | same | provider table/export | `provider`, `expected_abi`, `actual_abi`, `range?` |
| `tts.adapter.metadata-mismatch` | same | metadata handle | `provider`, `expected_raw_hash`, `actual_raw_hash`, `expected_payload_hash`, `actual_payload_hash` |
| `tts.adapter.capability-mismatch` | same | provider/binding/config range | `provider`, `capability_kind`, `expected`, `actual`, `range?` |
| `tts.adapter.protocol-mismatch` | protocol negotiation | runtime adapter location | `provider`, `expected_protocol`, `actual_protocol`, `stage` |
| `tts.runtime.capability-unavailable` | runtime driver / host policy | call site or task | `capability=tts.synthesize`, `profile_id?`, `task_id?` |
| `tts.runtime.provider-unavailable` | catalog preparation | task | `tts_speaker`, `locale`, `considered_providers`, `availability_digest` |
| `tts.runtime.request-limit` | intent/request validator | call site | `limit_kind`, `maximum`, `actual`, `range?` |
| `tts.runtime.queue-limit` | TTS host adapter | task | `maximum=256`, `provider?`, `task_id` |
| `tts.runtime.timeout` | TTS host adapter | task | `timeout_millis`, `attempt`, `phase`, `provider`, `request_fingerprint_prefix` |
| `tts.runtime.cancelled` | scheduler/TTS host adapter | task | `phase`, `provider?`, `request_fingerprint_prefix` |
| `tts.runtime.catalog-changed` | runtime driver | queued task | `old_catalog_digest`, `new_catalog_digest`, `changed_coordinate`, `task_id` |
| `tts.runtime.provider-failure` | TTS host adapter | task | sanitized `provider`, `class`, `code`, `message`, `retryable`, `attempt`, `request_fingerprint_prefix` |
| `tts.runtime.protocol-failure` | protocol decoder/state machine | task/adapter | `provider`, `stage`, `protocol_code`, `frame_kind?`, `sequence?` |
| `tts.runtime.result-too-large` | TTS host adapter | task | `maximum_bytes=33554432`, `observed_bytes`, `provider` |
| `tts.runtime.invalid-audio` | audio-codec/TTS adapter mapping | task | `format`, `reason_kind`, `encoded_bytes`, `provider`; no bytes |

## 8. Diagnostic formatting and limits

- One rendered diagnostic is at most 8,192 UTF-8 bytes.
- Candidate lists are capped at 16 entries followed by an omitted count.
- Provider messages are bounded and sanitized before entering a diagnostic.
- Source diagnostics may display public Character/profile/speaker IDs and
  ranges. They never display provider key, credential reference suffix,
  credential value, request text, audio bytes, or raw provider payload.
- Runtime diagnostics may display provider ID only when the runtime's
  restricted diagnostic policy permits it; otherwise they say `provider` and
  retain the ID structurally.
- No diagnostic is dedicated to the removed names `speaker`, `voice`, or
  `tts.synthesis`; current generic syntax/sema diagnostics own those failures.

## 9. Test doubles and simulation

`arcweft-host-adapter::tts::testing` supplies a deterministic scripted executor
that consumes typed requests and emits typed `TtsProviderEvent` sequences. It
has no filesystem, network, clock, or environment access. A manual host clock
and explicit availability snapshots drive timeout/retry tests. Protocol vector
tests feed the same scripts through Rust typed, Wasm codec, and process AWTP
paths.
