# Wire, version, and limits

## 1. Canonical primitive encoding

TTS binary codecs use the following primitives. Decoders reject any value that
cannot be re-encoded byte-for-byte to the input.

| Primitive | Encoding |
|---|---|
| `u8`, `i8` | one byte |
| `u16`, `i16`, `u32`, `i32`, `u64` | fixed-width little endian |
| `bool` | `0x00` or `0x01`; other values invalid |
| enum | fixed-width unsigned discriminant specified below |
| string | `u32` byte length followed by exact UTF-8 bytes; then nominal validation |
| bytes | `u32` length followed by bytes |
| option | one-byte `0`/`1`, then payload when `1` |
| vector | `u32` count followed by elements |
| digest | exactly 32 bytes |
| artifact ID | owner-defined exact canonical bytes preceded by `u32` length |

Counts and lengths are checked before allocation. Integer overflow, invalid
UTF-8, unknown discriminant, noncanonical bool/option, duplicate element,
wrong order, trailing byte, truncation, and one-over-budget values are hard
errors.

## 2. Existing project manifest wire

Owner: `arcweft-manifest-model` plus the final sole span-preserving schema-1
TOML decoder.

- schema: exactly 1;
- TTS root field: `tts`;
- tables: `tts.providers` and `tts.bindings`;
- unknown root/table/field: reject;
- duplicate key/table/quoted or unquoted map ID: reject with first/later ranges;
- semantic digest: existing manifest canonical semantic-digest transaction,
  extended with accepted TTS records in sorted ID order;
- no second parser and no accepted-text reparse.

The provider key is restricted manifest data. Credential values are forbidden;
only `SecretRef` is encoded.

## 3. Generated adapter metadata wire

Owner: `arcweft-adapter-metadata`.

The existing exact schema and format stay:

```text
schema = 1
format = arcweft.adapter-metadata
```

The direct final field is `exports.tts_providers`. Canonical order is
`(provider, export)`. Locale/style/config-field vectors are sorted by their
canonical text, and output formats by discriminant. Duplicate or unsorted data
is rejected rather than sorted by the decoder.

### Fixed discriminants

| Type | Values |
|---|---|
| `AudioFormat` | WAV=0, FLAC=1, OGG_VORBIS=2, MP3=3, AAC_MP4=4 |
| `TtsCancellationCapability` | NONE=0, COOPERATIVE=1, ABORTABLE=2 |
| `TtsIdempotencyCapability` | NONE=0, REQUEST_FINGERPRINT=1 |
| derived `TtsAdapterTargetFamily` in the accepted provider catalog | RUST=0, WASM=1, PROCESS=2 |
| public config type | BOOL=0, I64=1, U64=2, STRING=3, STRING_LIST=4 |

Unknown JSON fields and unknown enum strings are rejected. Canonical JSON,
payload hash, ABI hash, raw metadata hash, package/module/export identity, and
artifact binding remain the existing owners. Every TTS field participates in
payload and ABI digests. There is no schema-2 reader.

`TtsPublicConfigValue` uses discriminants BOOL=0, I64=1, U64=2, STRING=3,
STRING_LIST=4 in the restricted provider catalog. A config field ID uses
`LOWER_LOCAL`, max 64 bytes. One String is at most 1,024 bytes/scalars; one list
has at most 32 strings, each at most 256 bytes/scalars. Generated schema ranges
and finite enums further restrict accepted values.

`TtsOptionValue` uses BOOL=0, I64=1, U64=2, MILLI=3, STRING=4. Option String is
at most 512 bytes/scalars; the complete extension vector remains capped at 16
entries and 4,096 encoded bytes.

## 4. AWFB TTS profile catalog

Owner: `arcweft-bundle` section registry and
`arcweft-audio-tts::codec::profile_catalog`.

```text
BundleSectionKind::TtsProfileCatalog
SectionKindCode = 22
schema_version = 1
semantic digest context = arcweft-tts-profile-catalog-v1
allowed bundle kinds = Program, AgentController, ContentPack
residency = Startup
placement = Embedded or External according to existing content policy
required = true iff executable code references a TTS profile/Character call
```

### Payload field order

```text
magic[8] = "AWTTSPRF"
version u32 = 1
profile_count u32
for each profile sorted by profile_id:
    profile_id string
    tts_speaker_id string
    locale_count u16
    locales strings in authored order, unique
    default_style option<string>
    rate_milli u16
    pitch_cents i16
character_binding_count u32
for each binding sorted by (character_id, selection_priority, profile_id):
    character_id string
    profile_id string
    default_for_character bool
    selection_priority u16
semantic_digest[32]
```

The trailing semantic digest is computed over all preceding bytes under the
context above. The AWFB section stored/content digests and the outer artifact
identity additionally bind the payload. The section contains no provider ID,
provider key, credential reference, request text, audio result, display name,
or dialogue projection data.

Limits: profiles 4,096; Character bindings 4,096; decoded payload 8 MiB.

## 5. AWFB restricted provider catalog

Owner: `arcweft-bundle` section registry and
`arcweft-audio-tts::codec::provider_catalog`.

```text
BundleSectionKind::TtsProviderCatalog
SectionKindCode = 23
schema_version = 1
semantic digest context = arcweft-tts-provider-catalog-v1
allowed bundle kinds = Program, AgentController
residency = Startup
placement = Embedded or External according to existing content policy
required = true iff executable code can issue tts.synthesize
privacy = RestrictedArtifact
```

### Payload field order

```text
magic[8] = "AWTTSPRV"
version u32 = 1
provider_count u16
for each provider sorted by provider_id:
    provider_id string
    module_import_id string
    export_id string
    protocol string                         # exact arcweft-tts-provider-v1
    target_family u8                        # derived from existing AdapterTarget
    artifact_identity bytes
    raw_metadata_hash[32]
    metadata_payload_hash[32]
    abi_hash[32]
    capability_digest[32]
    public_config_digest[32]
    credential_ref option<string>          # locator only
    capabilities record in fixed field order
    public_config_count u16
    public config entries sorted by field ID
binding_count u32
for each binding sorted by (tts_speaker, priority, provider_id, binding_id):
    binding_id string
    tts_speaker_id string
    provider_id string
    provider_key string                    # restricted; redacted diagnostics
    provider_key_digest[32]
    locale_count u16 + sorted locale strings
    style_count u16 + sorted style strings
    priority u16
semantic_digest[32]
```

Capabilities encode locales, styles, formats, extension-option schemas,
rate/pitch ranges, provider text limits, max concurrency, progress, cancellation,
and idempotency in the field order of `TtsProviderCapabilities`.

The catalog is available only to runtime/host policy owners. Bundle summaries,
debug symbols, Agent manifests, MCP resources, and source maps do not copy its
provider key or credential reference values. There is no alternate public
provider catalog.

Limits: 64 providers; 8,192 bindings; decoded payload 16 MiB.

## 6. Provider adapter framing: AWTP 1

Owner: `arcweft-audio-tts::protocol`.

AWTP is the TTS subprotocol carried inside the existing process target
`arcweft-process-v1` / `stdio-framed-v1`; it does not replace or version that
outer adapter transport. Every process TTS frame payload has exactly this
32-byte header:

```text
offset  size  field
0       4     magic = "AWTP"
4       2     version = 1, little endian
6       1     kind
7       1     flags = 0
8       16    TtsAdapterCallId
24      4     sequence, little endian
28      4     payload length, little endian
```

The payload follows immediately. `flags != 0`, unknown kind, payload over its
kind budget, truncated payload, or trailing bytes after the final frame are
rejected.

### Frame kinds

| Hex | Kind | Sequence rule | Maximum payload |
|---:|---|---|---:|
| `01` | `NegotiationRequest` | 0 | 64 KiB |
| `02` | `NegotiationResponse` | 0 | 256 KiB |
| `10` | `SynthesizeRequest` | 0 | 128 KiB |
| `11` | `Accepted` | 0 | 4 KiB |
| `12` | `Progress` | increasing event sequence | 4 KiB |
| `13` | `AudioChunk` | contiguous from 0 | 256 KiB |
| `14` | `Completed` | exact chunk count | 4 KiB |
| `15` | `Failed` | terminal | 8 KiB |
| `16` | `Cancel` | 0 | 0 |
| `17` | `Cancelled` | terminal | 0 |

### Negotiation payload

`NegotiationRequest` field order:

```text
protocol string
expected_provider_id string
expected_export_id string
expected_artifact_identity bytes
expected_abi_hash[32]
expected_capability_digest[32]
```

`NegotiationResponse` repeats those exact values and then encodes one
`TtsProviderCapabilities` record. Any mismatch prevents executor publication.

### Synthesis payload

`SynthesizeRequest` uses the field order of `TtsProviderSynthesisRequest` after
`call` (already in the header). `credential_slot` is a lease slot, never secret
bytes. Extension options are sorted by ID. The request payload limit is 128 KiB.

`AudioChunk` payload is raw encoded audio bytes. `Completed` contains
`chunk_count u32`, `total_bytes u64`, and `content_digest[32]`.
`Failed` contains class discriminant, code string, sanitized message string,
retryable bool, and optional restricted-payload digest. Provider trace IDs and
raw response bodies are not in the public protocol result.

Rust in-process executors use the same typed messages without serializing them.
Wasm components expose the equivalent typed component interface under the
existing `arcweft-wasm-component-v1` target. Process adapters use AWTP bytes
inside `stdio-framed-v1`. Behavioral parity tests run all three target families
against the same semantic vectors; AWTP byte-golden tests apply to Process.

## 7. Request/result runtime wire

`TtsSynthesisRequest`, `TtsProgress`, `TtsAudioAsset`, and `TtsError` map to
nominal runtime records with exact schema-ordinal fields. Runtime values carry
no field-name strings after lowering. Layout hashes bind ordinal, field type,
and nested discriminants.

`TtsError` variant discriminants are, in declaration order:

```text
0 InvalidIdentity
1 UnknownProfile
2 MissingCharacterProfile
3 CharacterProfileMismatch
4 UnknownSpeaker
5 UnknownProvider
6 UnsupportedLocale
7 UnsupportedStyle
8 UnsupportedOption
9 UnsupportedFormat
10 CapabilityUnavailable
11 ProviderUnavailable
12 QueueLimit
13 RequestLimit
14 Timeout
15 Cancelled
16 CatalogChanged
17 ProviderFailure
18 ProtocolFailure
19 ResultTooLarge
20 InvalidAudio
```

Nested closed-enum discriminants are also fixed:

```text
TtsIdentityKind: SPEAKER=0, PROFILE=1, PROVIDER=2, BINDING=3, LOCALE=4, STYLE=5, OPTION=6
TtsAvailabilityState: AVAILABLE=0, UNAVAILABLE=1
TtsProtocolStage: NEGOTIATION=0, REQUEST=1, ACCEPTED=2, PROGRESS=3, AUDIO=4, COMPLETION=5, CANCELLATION=6, CLEANUP=7
TtsProtocolFailureCode: BAD_MAGIC=0, UNSUPPORTED_VERSION=1, NONZERO_FLAGS=2, UNKNOWN_KIND=3, TRUNCATED=4, TRAILING=5, OVERSIZED=6, WRONG_CALL=7, SEQUENCE=8, INVALID_STATE=9, DIGEST_MISMATCH=10, SIZE_MISMATCH=11, NEGOTIATION_MISMATCH=12, INVALID_PAYLOAD=13
TtsInvalidAudioKind: EMPTY=0, FORMAT_MISMATCH=1, PROBE=2, DECODE=3, UNSUPPORTED_CODEC=4, UNSUPPORTED_CHANNELS=5, SAMPLE_RATE=6, DURATION=7, STREAM_CHANGED=8, DIGEST_MISMATCH=9, SIZE_MISMATCH=10
```

Unknown discriminants, wrong nominal identity/layout, extra/missing fields, or
oversized nested payloads are rejected. Struct field ordinals are exactly the
declaration order shown in `REQUEST_RESULT_AND_ADAPTER_PROTOCOL.md`; no named
field map is accepted at the lowered runtime/protocol boundary.

## 8. Save and replay

### Save

Active or queued TTS work is not resumable. Existing
`BundleSessionPendingBlocker::HostTasks { active, queued_events }` and
`TaskGenerationPins` MUST block save. No TTS-specific save variant or active
request serialization is added.

A completed `TtsAudioAsset` retained in ordinary durable runtime state is saved
as its nominal value, subject to 32 MiB per asset and 256 MiB aggregate TTS
asset bytes per save. Provider keys, provider catalogs, credentials, adapter
calls, progress, and partial spools are never save state.

### Replay

The current unreleased root replay schema 1 is corrected directly so external
failure stores a typed `RuntimePayload`. A TTS success records the complete
`TtsAudioAsset`; replay validates its nominal layout and content digest and
injects it without dispatch. If bytes were omitted by recording policy, replay
fails explicitly; it never calls the provider as fallback.

## 9. Hot-reload compatibility wire

A queued request is compatible only when this tuple is byte-equal between old
and new accepted catalogs:

```text
profile semantic digest
selected binding ID
provider ID
provider-key digest
capability digest
public-config digest
artifact identity
ABI hash
credential-ref canonical text
protocol ID
```

Active requests use the old pinned generation. There is no wire translation
between generations.

## 10. Central limits

| Item | Exact limit |
|---|---:|
| `TtsSpeakerId` / `TtsVoiceProfileId` | 256 bytes and 256 scalars |
| `TtsProviderId` | 96 bytes/scalars |
| `TtsProviderBindingId` | 192 bytes/scalars |
| provider speaker key | 256 bytes, 128 scalars |
| locale | 64 bytes/scalars |
| style / option ID | 64 bytes/scalars |
| SecretRef | 128 bytes/scalars |
| providers | 64 |
| profiles | 4,096 |
| Character-profile bindings | 4,096 |
| provider-speaker bindings | 8,192 |
| profile locales | 16 |
| binding locales/styles | 32 / 32 |
| provider advertised locales/styles | 128 / 64 |
| provider public-config fields | 32 |
| provider extension-option schemas | 32 |
| public config bytes/provider | 16 KiB |
| request text | 65,536 bytes, 16,384 scalars, nonempty |
| extension options / encoded bytes | 16 / 4,096 bytes |
| selected task canonical wire | 128 KiB |
| encoded result | 32 MiB |
| in-memory result threshold | 4 MiB |
| AWTP audio chunk | 256 KiB |
| decoded duration | 30 minutes |
| sample rate | 8,000–192,000 Hz |
| channels | 1–2 |
| progress events | 128 per request after coalescing |
| global active TTS tasks | 32 |
| default per-provider active tasks | min(metadata, host config), host default 8 |
| queued TTS tasks | 256 |
| retry attempts | 1–3, default 2 |
| timeout | 1,000–120,000 ms, default 30,000 |
| cancellation cleanup deadline | 5,000 ms |
| provider error code | 64 printable ASCII bytes |
| sanitized provider message | 1,024 UTF-8 bytes |
| restricted provider payload | 4,096 bytes host-private |
| one rendered diagnostic | 8,192 bytes |
| saved/replayed TTS bytes | 32 MiB/asset; 256 MiB aggregate |

## 11. Universal rejection rules

Every TTS decoder rejects malformed, duplicate, unknown, trailing, truncated,
noncanonical, and oversized input. Optional unknown AWFB sections follow the
existing container policy only at the outer container; once section kind 22 or
23 is recognized, its inner schema is closed. No decoder silently ignores an
unknown inner field, sorts input into canonical order, truncates a value,
normalizes text, or accepts a legacy spelling.
