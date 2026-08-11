# Ownership and dependency graph

## 1. Selected layering

```text
# Every edge is written dependent -> direct lower dependency.

arcweft-character -> arcweft-id, arcweft-source
arcweft-audio-core -> arcweft-interaction-model
arcweft-audio-tts -> arcweft-id, arcweft-character, arcweft-audio-core

arcweft-core -> arcweft-audio-tts
arcweft-manifest-model -> arcweft-audio-tts
arcweft-adapter-metadata -> arcweft-manifest-model, arcweft-audio-tts
arcweft-bundle -> arcweft-manifest-model, arcweft-audio-tts

arcweft-project-loader -> core, bundle, manifest-model, adapter-metadata, audio-tts
arcweft-compiler -> core, bundle, runtime-plan, audio-tts
arcweft-runtime-plan -> core, audio-tts
arcweft-runtime-driver -> core, bundle, audio-tts
arcweft-host-adapter -> core, adapter-context, audio-tts
arcweft-runtime-host/players -> runtime-driver, host-adapter

provider Rust SDK adapter -> host-adapter, audio-tts, provider SDK
provider Wasm adapter -> host-adapter, audio-tts, Wasm host services
provider process adapter -> host-adapter, audio-tts, process/network/secret services
```

`arcweft-project-loader::topology::LoadedProfileTopology` is the immutable
all-or-nothing accepted-topology owner. It gains the accepted TTS catalogs and
the exact metadata/artifact handles used to build them; no second topology or
publication registry is introduced. The accepted data flow is
`LoadedProfileTopology -> compiler/bundle sections 22/23 -> runtime-driver
accepted generation -> host-adapter`; after success, `TtsAudioAsset` flows
through the existing audio codec, mixer, and device path. Those arrows are data
flow, not reverse Cargo dependencies. No edge from a lower model/data crate to
runtime-driver, host adapter, source syntax, LSP, network, filesystem, provider
SDK, platform audio, dialogue, View, Agent, or MCP is permitted.

## 2. New crate: `arcweft-audio-tts`

### Direct dependencies

```toml
[dependencies]
arcweft-audio-core = { path = "../arcweft-audio-core" }
arcweft-character = { path = "../arcweft-character" }
arcweft-id = { path = "../arcweft-id" }
blake3.workspace = true
serde = { workspace = true, features = ["derive", "rc"] }
thiserror.workspace = true
```

The Character dependency is intentional and one-way. It permits the explicit
`CharacterTtsProfileBinding` record while preserving nominal inequality. The
crate does not depend on `arcweft-dialogue` and does not inspect display names,
View data, localization catalogs, or dialogue runtime state.

### Public responsibility modules

```text
arcweft_audio_tts::identity
arcweft_audio_tts::locale
arcweft_audio_tts::profile
arcweft_audio_tts::catalog
arcweft_audio_tts::request
arcweft_audio_tts::result
arcweft_audio_tts::error
arcweft_audio_tts::progress
arcweft_audio_tts::protocol
arcweft_audio_tts::codec
arcweft_audio_tts::limits
```

`lib.rs` is a small facade that re-exports only the source/runtime-facing
identity, profile, request/result/error, and limits surface. Restricted provider
keys, accepted provider catalog internals, raw protocol frames, and codec helper
types remain under their responsibility modules rather than being flattened.

### Owned data

The crate owns:

- all nominal TTS IDs and validation errors;
- `TtsVoiceProfile`, `CharacterTtsProfileBinding`, and immutable profile catalog;
- provider declarations, capability descriptors, provider-speaker bindings,
  accepted provider catalog, and deterministic selection evidence;
- `TtsSynthesisIntent`, `TtsSynthesisRequest`, options, progress, result, and
  structured errors;
- request/catalog/output digests;
- canonical binary profile/provider catalogs;
- AWTP adapter envelope and payload records;
- centralized limits and validation methods.

It does not own Task/Need execution, source syntax, host dispatch, provider
SDKs, credential resolution, networking, clocks, audio output, save files, or
Agent projection.

## 3. Existing owners and exact additions

| Owner | Final responsibility | Required addition |
|---|---|---|
| `arcweft-id` | Generic `PublicId` | None. TTS family validation stays on TTS newtypes. |
| `arcweft-character` | `CharacterId` | None. No TTS behavior is added to Character identity. |
| `arcweft-audio-core` | `AudioFormat`, decoded PCM, graph, buses, playback voices | None except normal dependency exposure already present. No TTS model or I/O. |
| `arcweft-audio-codec` | Complete encoded-audio validation/decoding | Add a bounded validation entry point that returns existing decoded metadata; no provider logic. |
| `arcweft-core::task` | Task templates/specs and `HostTaskRequest` | Replace the local stringly request with distinct typed `TtsSynthesisIntent` and final `TtsSynthesis` variants; add inherent `TaskSpec::prepare_tts` and `TaskKey::for_tts`; directly type task failures as `RuntimePayload`. |
| `arcweft-manifest-model` | Schema-1 typed manifest records | Add `TtsSpec`, provider instance, binding, public config, and `SecretRef`; depend on TTS nominal types. |
| `arcweft-adapter-metadata` | Generated metadata schema, strict decoder, ABI/payload digests | Extend schema 1 with `tts_providers`; include it in canonical validation and both semantic digests. |
| `arcweft-project-loader::topology::LoadedProfileTopology` | Immutable all-or-nothing accepted topology, filesystem containment, and retained artifact handles | Add accepted TTS profile/provider catalogs, their semantic digests, and exact metadata/artifact handles to the existing product; never resolve credential values while decoding manifest data. |
| `arcweft-bundle` | AWFB section registry and artifact binding | Add section kinds 22 and 23 and call TTS canonical codecs. |
| `arcweft-compiler` / `arcweft-runtime-plan` | Accepted typed resource lowering and ordinary function calls | Lower exact profile/Character references and TTS intents; carry accepted catalog revision/digest. |
| `arcweft-runtime-driver::tts` | Integration of catalog-owned request preparation at the existing dispatch boundary | Hold `TtsPreparationContext`, invoke `TtsAcceptedCatalog::prepare_request`, call `TaskSpec::prepare_tts` before registry publication, queue typed preparation failures, and pin the generation. |
| `arcweft-host-adapter::tts` | Host task implementation and provider executor registry | Add `TtsHostAdapter`, queues, limits, cancellation, retries, secret leases, AWTP dispatch. |
| provider-specific adapter crates | Provider SDK/process/Wasm integration | Implement `TtsProviderExecutor`; contain all provider dependencies and I/O. |
| `arcweft-runtime-host` and embedding players | Worker/main-thread pumping and host policy | Register TTS adapter only when the accepted launch topology authorizes it. |
| `arcweft-runtime-driver::session_save` | Quiescent save blocker | Reuse `HostTasks` and `TaskGenerationPins`; add no TTS save variant. |
| replay owner | External outcome capture/injection | Directly replace string failure with typed payload; record actual result bytes/digest. |
| Agent/MCP/debug owners | Sanitized observation | Project only public receipt/count metadata under the privacy policy. |
| `arcweft` facade | Deliberate application-facing API | Re-export source-facing TTS profile/options/result/error only. No provider key or secret type. |

## 4. Host adapter ownership

`arcweft-host-adapter::tts` owns the following host-only types:

```rust
pub struct TtsHostAdapter;
pub struct TtsProviderExecutorRegistry;
pub trait TtsProviderExecutor;
pub trait SecretResolver;
pub struct CredentialLease;
pub struct ProviderAvailabilitySnapshot;
pub struct TtsHostLimits;
```

`CredentialLease` is neither `Clone` nor `Serialize`; its `Debug` is always
redacted. Provider-specific crates receive a lease and the restricted
`TtsProviderSpeakerKey` only after policy, catalog, capability, queue, and
request-limit checks succeed.

## 5. Exact dependency prohibitions

The following Cargo edges are forbidden:

```text
arcweft-audio-tts -> arcweft-core
arcweft-audio-tts -> arcweft-dialogue
arcweft-audio-tts -> arcweft-runtime-driver
arcweft-audio-tts -> arcweft-host-adapter
arcweft-audio-tts -> arcweft-lang-syntax / HIR / sema / LSP
arcweft-audio-tts -> filesystem/network/process/provider SDK/platform audio
arcweft-manifest-model -> project-loader or filesystem
arcweft-adapter-metadata -> project-loader or provider SDK
arcweft-audio-core -> arcweft-audio-tts
provider-specific adapter -> dialogue/View/Agent source models
```

Cargo-metadata tests must assert the actual graph, not source-file spellings.

## 6. Why the existing audio crates are not the TTS owner

`arcweft-audio-core` owns playback assets, decoded PCM, graph preparation, buses,
effects, and logical playback voices. A TTS provider catalog introduces public
semantic speaker identity, provider artifact identity, restricted provider
keys, locale/style capabilities, external request semantics, errors, retries,
and an adapter wire. Placing those responsibilities in the graph or mixer would
couple deterministic playback to external synthesis and provider policy.

`arcweft-audio-codec` remains the encoded-audio validator/decoder. It must not
know why bytes were produced. `arcweft-audio-mixer` and
`arcweft-audio-device-cpal` remain unchanged because synthesis completion is not
a playback command.

## 7. Structured-error correction ownership

The current string task error is a concrete cross-cutting defect for this
contract. The correction belongs on the existing Arcweft-owned boundary types:

```text
HostTaskOutcome.result: Result<RuntimePayload, RuntimePayload>
TaskEventKind::Err(RuntimePayload)
RecordedExternalOutcomeResultV1::Failure { error: RuntimePayload }
```

Formatting a payload into a diagnostic is an inherent method on the owning
runtime diagnostic/error type. No extension trait, endpoint-conversion helper,
wrapper outcome, or dual error carrier is allowed.
