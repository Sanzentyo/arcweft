# Typed layout and codec

## 1. Layout identity algorithm

All TTS runtime payloads use the current core nominal layout algorithm without
a TTS-specific envelope:

```text
layout = BLAKE3(
    "arcweft.nominal-schema\0" ||
    u32le(1) ||
    canonical RuntimeTypeSchema bytes
)
```

Records have `deny_unknown_fields = true`; `rust_name == wire_name`; every field
has `has_default = false`, `skip = false`, and no field-level byte format unless
its schema is `Bytes(Binary)`. Record fields are encoded in the ordinal order
below. The type ID is semantic, not a Rust type name or operation string.

## 2. Top-level nominal contracts

| Nominal type ID | Exact layout hash | Fields | Max canonical bytes |
|---|---|---:|---:|
| `std.audio.TtsSynthesisIntent` | `79a77138c3c4b8b400357865ebc33393f4e277aa0daf07facf5de523d996a0c2` | 4 | 131,072 |
| `std.audio.TtsSynthesisRequest` | `83d1252da0097f35f90bdf01477186e2d9572d52108b38008df1ab164177b391` | 11 | 131,072 |
| `std.audio.TtsProgress` | `07d12053d7baee66fa0cb2479ce235bdedd65b0fea8fa41c098649318e18cfc9` | 3 | 1,024 |
| `std.audio.TtsAudioAsset` | `1e7489f55cf229961aa20e5fce12e3fa37f8e6654f6459c0ce3ea6fa29ec18f8` | 7 | 33,685,504 |
| `std.audio.TtsError` | `92a78ac9b20463b3ad920ab634961ef57ee0c2049ad74d0ca0e37c3c8ce2035a` | 1 | 16,384 |

The audio-asset cap is exactly `32 MiB + 128 KiB` so the complete 32 MiB encoded
asset plus bounded typed metadata has a defined enclosing budget.

## 3. Exact field ordinals

### `std.audio.TtsSynthesisIntent`

| Ordinal | Field | Runtime shape |
|---:|---|---|
| 0 | `selector` | closed `TtsProfileSelector` variant with exact path |
| 1 | `text` | UTF-8 String, nonempty, max 65,536 bytes / 16,384 scalars |
| 2 | `locale` | canonical `Option<TtsLocaleId>` |
| 3 | `options` | nominal `std.audio.TtsSynthesisOptions` |

### `std.audio.TtsSynthesisOptions`

| Ordinal | Field | Runtime shape |
|---:|---|---|
| 0 | `style` | `Option<TtsStyleId>` |
| 1 | `rate_milli` | `Option<TtsRateMilli>` |
| 2 | `pitch_cents` | `Option<TtsPitchCents>` |
| 3 | `format` | `Option<AudioFormat>` |
| 4 | `extensions` | ordered dense sequence of nominal `TtsExtensionOption` |
| 5 | `timeout` | nominal `TtsTimeoutMillis` |
| 6 | `retry` | nominal `TtsRetryPolicy` |

`TtsProfileSelector` uses closed external variants and U8 discriminants:
`Profile=0`, `Character=1`, `Speaker=2`. Payload ordinals are respectively
`[profile]`, `[character, profile]`, and `[tts_speaker]`.

### `std.audio.TtsSynthesisRequest`

| Ordinal | Field | Runtime shape |
|---:|---|---|
| 0 | `fingerprint` | nominal 32-byte `TtsRequestFingerprint` |
| 1 | `selection` | nominal `TtsSelectionEvidence` |
| 2 | `text` | exact UTF-8 String |
| 3 | `locale` | nominal `TtsLocaleId` |
| 4 | `style` | `Option<TtsStyleId>` |
| 5 | `rate_milli` | nominal U16 `TtsRateMilli` |
| 6 | `pitch_cents` | nominal I16 `TtsPitchCents` |
| 7 | `output_format` | closed `AudioFormat` |
| 8 | `extensions` | sorted unique sequence of nominal extension options |
| 9 | `timeout` | nominal U32 milliseconds |
| 10 | `retry` | nominal U8 maximum attempts |

### `std.audio.TtsSelectionEvidence`

| Ordinal | Field |
|---:|---|
| 0 | `profile: Option<TtsVoiceProfileId>` |
| 1 | `character: Option<CharacterId>` |
| 2 | `tts_speaker: TtsSpeakerId` |
| 3 | `provider: TtsProviderId` |
| 4 | `binding: TtsProviderBindingId` |
| 5 | `provider_key: TtsProviderSpeakerKey` (restricted) |
| 6 | `provider_key_digest: TtsProviderKeyDigest` |
| 7 | `profile_catalog_digest: TtsCatalogDigest` |
| 8 | `provider_catalog_digest: TtsCatalogDigest` |
| 9 | `availability_digest: TtsAvailabilityDigest` |
| 10 | `adapter_artifact: TtsArtifactIdentity` |
| 11 | `adapter_abi: TtsAbiDigest` |
| 12 | `capability_digest: TtsCapabilityDigest` |
| 13 | `public_config_digest: TtsConfigDigest` |
| 14 | `accepted_generation: U64` |

### Progress, asset, receipt, and error

```text
TtsProgress      = [phase, completed_basis_points, received_bytes]
TtsAudioAsset    = [format, bytes, content_digest, duration_millis,
                    sample_rate_hz, channels, receipt]
TtsPublicReceipt = [profile, character, tts_speaker, locale, style,
                    rate_milli, pitch_cents, format]
TtsError         = [kind]
```

`TtsProgressPhase`: `Queued=0`, `Connecting=1`, `Synthesizing=2`,
`Receiving=3`, `Validating=4`.

`AudioFormat`: `WAV=0`, `FLAC=1`, `OGG_VORBIS=2`, `MP3=3`, `AAC_MP4=4`.

`TtsOptionValue`: `Bool=0`, `I64=1`, `U64=2`, `Milli=3`, `String=4`.

## 4. Nested nominal layout identities

| Nominal type ID | Exact layout hash | Fields |
|---|---|---:|
| `std.audio.TtsSynthesisOptions` | `1db6ff709145255271cb9a5cc6fa9034346360d274ea2fcd330137157ed123f9` | 7 |
| `std.audio.TtsProfileSelector.Profile` | `9fba169bfa1fac3559f5e2e84d10ca589e7d71e32b95b14552746c02781ae62d` | 1 |
| `std.audio.TtsProfileSelector.Character` | `283fd0789b520215ee9fcb664da0d561fe22ad6949a7d6ea8309e3f7f94eb639` | 2 |
| `std.audio.TtsProfileSelector.Speaker` | `c10ba6db0433b02de117423cff3337e63d13f92dabe5d3ff0686bb17cd31397c` | 1 |
| `std.audio.TtsSelectionEvidence` | `4e142b12b22ac40dd44b1c16b86300a54d004bbc410b01dd0ce539098f38b6b4` | 15 |
| `std.audio.TtsExtensionOption` | `45f40ee0dbe97a0d6f198d8815646241edc8a8623e2ab0a3df618f30cfc1f90d` | 2 |
| `std.audio.TtsRetryPolicy` | `59c17ff51bca2632664522285eb4c7792a323c877a4cc2602d7a2757994e9cd5` | 1 |
| `std.audio.TtsTimeoutMillis` | `7d897eb7c04d3ffbd0578b5d2598469aec08ff5fed1c9f9113ca65068e9ba9a8` | 1 |
| `std.audio.TtsRateMilli` | `d2004030574058d320ca57f90b4e8c099152ad032edc47f7cf4e8dbd10aa0165` | 1 |
| `std.audio.TtsPitchCents` | `148c424b126193c5414e421a21e875e679ec2036f1e2a1880221b92bd0b40b44` | 1 |
| `std.audio.TtsPublicReceipt` | `a6e8b0573326dd2a208ffeebd1a1fd3fb3e715abf5e559cdbe61c0e76542f298` | 8 |
| `std.audio.TtsProviderFailure` | `8cc026b2d2da66e83261a688790c7155c0ed1281964c2691f95e866bdabc4e3e` | 7 |
| `std.audio.TtsRequestFingerprint` | `3d009d631165b1b1a58a033b5d761ddcb615c2b99665bae9b5f11ab231f4cf6b` | 1 |
| `std.audio.TtsProviderKeyDigest` | `fac2b9e7393f1f8fc333d81a7ad7e89c20355716d32d69cc4658393ea276e472` | 1 |
| `std.audio.TtsCatalogDigest` | `57aa9c1f5da04e4f81fa77ad410ba9b499e321aa5b96742d01fa5b19458f21dd` | 1 |
| `std.audio.TtsAvailabilityDigest` | `e77ed98b0f33c3f9e3d527aa0910cf4a8d2bebe569015d2a658a8d3f68231f75` | 1 |
| `std.audio.TtsArtifactIdentity` | `5db0454fe2fab8d5060c1a487f067b7305cd86d6aa3a46163b7fa09937fee4f3` | 1 |
| `std.audio.TtsAbiDigest` | `28e677367313e06a2e31d881200fb438aa7e50310c3eacd0637376feaf33626b` | 1 |
| `std.audio.TtsCapabilityDigest` | `831f5e5110bf372dc40777b997030cda21b2c0dcb2edbf31f8343b2278b495b4` | 1 |
| `std.audio.TtsConfigDigest` | `c23f2a5d09df6595ab166177da4f0a3bbb5270b7e6a4f93da4989a0e47690d3f` | 1 |
| `std.audio.TtsAudioDigest` | `1924418d1912dbbf1d6d339699b93bfe84c1b4509c38bb854fee21e16f5f2d0e` | 1 |
| `std.audio.TtsProviderPayloadDigest` | `cc41bb89bef645bffaf1245aee10ba45f7b678323468e4db905ef9f13df4b878` | 1 |
| `std.audio.TtsVoiceProfileId` | `f304a049308c73dd8ba238427f27773c08321f470ae48f3aad6b1293f1d80812` | 1 |
| `std.character.CharacterId` | `ea856bb2525b997dc24e946f2ba7c3e3d3fdb0f0389daf13c40cf5c7cf330ba1` | 1 |
| `std.audio.TtsSpeakerId` | `7056559fde00504af128b1fc5b1bc8e62ad119fbf9274486b2eff297679417d4` | 1 |
| `std.audio.TtsProviderId` | `bb571b529093f25a82e5bd93851ddb6e1480de92fc47d94bbd49274affd7a502` | 1 |
| `std.audio.TtsProviderBindingId` | `c2c9e8e9561ca30bb9ef06c9f3fdbe030077ff04b7a949819022575bf26aa905` | 1 |
| `std.audio.TtsProviderSpeakerKey` | `c5a1485d5b1022577d706c348f8a8a1b57ae699c676892bb62c25c035973eb39` | 1 |
| `std.audio.TtsLocaleId` | `61342b75198ac8bd1c21198aa91d44d4d0a8cfdc61d12ac9c7be54bc6bf42b6f` | 1 |
| `std.audio.TtsStyleId` | `a7e2519464bb51cb6ff99fa058a37640c3bde030efe31ab941d7561b6617e74b` | 1 |
| `std.audio.TtsOptionId` | `c032495244c4ee0bcca3af52319cfc211f8928d33b6ede745a4384fc492c3c0f` | 1 |

Every digest record contains ordinal 0 `bytes: Bytes(Binary)` and requires
exactly 32 bytes. Every identifier record contains ordinal 0 `value: String`
and applies the accepted identity grammar/limit. `TtsArtifactIdentity` contains
ordinal 0 `canonical_bytes: Bytes(Binary)` and is bounded to 256 bytes.

## 5. Exact TtsError discriminants and payload layouts

| Discriminant | Variant | Payload nominal type | Payload layout | Ordinal fields |
|---:|---|---|---|---|
| 0 | `InvalidIdentity` | `std.audio.TtsError.InvalidIdentity` | `dbdb3a8afecb3c7d1167acd9937046dd183adea9d978f70d94f8c60e9d3a7936` | kind |
| 1 | `UnknownProfile` | `std.audio.TtsError.UnknownProfile` | `d9e92f93f7bccc1c6e81edbfd48000c6cffb48d2c364c532e6baeeb6652625a2` | profile |
| 2 | `MissingCharacterProfile` | `std.audio.TtsError.MissingCharacterProfile` | `577c10a00313878c0cfc4eaf0de8aa1722b4de8740396e48b192e9e9168a82b1` | character |
| 3 | `CharacterProfileMismatch` | `std.audio.TtsError.CharacterProfileMismatch` | `cf369e1a396718e73b9028d974f6135d374482f4ec30f0b17ca037861563baf1` | character, profile |
| 4 | `UnknownSpeaker` | `std.audio.TtsError.UnknownSpeaker` | `8d23b85302fde413ef191cc42e7324240b9ddced58db859c04db49bd281c275a` | tts_speaker |
| 5 | `UnknownProvider` | `std.audio.TtsError.UnknownProvider` | `77f31edebe4501eeb086251d0cbb344301c6d49369f93f642c2761e898115d00` | provider |
| 6 | `UnsupportedLocale` | `std.audio.TtsError.UnsupportedLocale` | `7a13871e732a5fe58335c310ed64e926870bf8b67b329d8b4f2e637ae04f6c7c` | tts_speaker, locale |
| 7 | `UnsupportedStyle` | `std.audio.TtsError.UnsupportedStyle` | `52ee7eefd3b37cb926022f58ef92a56866d15b16093d7c5a44dce4c128211c70` | tts_speaker, style |
| 8 | `UnsupportedOption` | `std.audio.TtsError.UnsupportedOption` | `5dd49994dfc8c18751a45e676c867a2bf5068cfa60319335675d560871325791` | option |
| 9 | `UnsupportedFormat` | `std.audio.TtsError.UnsupportedFormat` | `63a83cc7d9db42c0b8fbd239ca791f3480bac27d853eb8d18008607f0656b268` | format |
| 10 | `CapabilityUnavailable` | `—` | `—` | none |
| 11 | `ProviderUnavailable` | `std.audio.TtsError.ProviderUnavailable` | `d1396ae7577df4d0d6cd431047aa16da50721984f0974ec560724ba632e3b05b` | provider |
| 12 | `QueueLimit` | `std.audio.TtsError.QueueLimit` | `02cd19b1b4b901580123c48038296fb11d1f4db49ddc9471c63a94dec381d948` | maximum |
| 13 | `RequestLimit` | `std.audio.TtsError.RequestLimit` | `db5cc6bdd73c3a087e5fbcefe7cab83355ac19b97f8b1dd6f8042ef785f8c770` | maximum_bytes, actual_bytes |
| 14 | `Timeout` | `std.audio.TtsError.Timeout` | `f316fd1ee91a012a7653d36df6031ce44a9c16a79a8f667f5a037748e2bb864f` | timeout_millis |
| 15 | `Cancelled` | `—` | `—` | none |
| 16 | `CatalogChanged` | `—` | `—` | none |
| 17 | `ProviderFailure` | `std.audio.TtsProviderFailure` | `8cc026b2d2da66e83261a688790c7155c0ed1281964c2691f95e866bdabc4e3e` | provider, class, code, message, retryable, request, restricted_payload_digest |
| 18 | `ProtocolFailure` | `std.audio.TtsError.ProtocolFailure` | `20c86d6e02c2aae05ca07bbeed192e0f5894133b136747ce416148cefa8302bf` | stage, code |
| 19 | `ResultTooLarge` | `std.audio.TtsError.ResultTooLarge` | `848ffcb14ad45738061dd62a8149428340c263c23849dde789cd543255924675` | maximum_bytes, observed_bytes |
| 20 | `InvalidAudio` | `std.audio.TtsError.InvalidAudio` | `160da99d03657e305c17c72074d469afb555fa4735547234c8779d81afa8d9e4` | kind |

Nested fixed discriminants remain the accepted values:

```text
TtsIdentityKind: Speaker=0, Profile=1, Provider=2, Binding=3,
                 Locale=4, Style=5, Option=6
TtsProviderFailureClass: Authentication=0, Authorization=1, RateLimited=2,
                         InvalidRequest=3, Unavailable=4, Transport=5,
                         Internal=6, Unknown=7
TtsProtocolStage: Negotiation=0, Request=1, Accepted=2, Progress=3,
                  Audio=4, Completion=5, Cancellation=6, Cleanup=7
TtsProtocolFailureCode: BadMagic=0, UnsupportedVersion=1, NonzeroFlags=2,
 UnknownKind=3, Truncated=4, Trailing=5, Oversized=6, WrongCall=7,
 Sequence=8, InvalidState=9, DigestMismatch=10, SizeMismatch=11,
 NegotiationMismatch=12, InvalidPayload=13
TtsInvalidAudioKind: Empty=0, FormatMismatch=1, Probe=2, Decode=3,
 UnsupportedCodec=4, UnsupportedChannels=5, SampleRate=6, Duration=7,
 StreamChanged=8, DigestMismatch=9, SizeMismatch=10
```

Unknown variant names/discriminants or payload presence are rejected. Unit
variants must have no payload. Payload variants must carry exactly the listed
nominal payload.

## 6. Runtime value representation

The runtime representation is structural and canonical:

- domain records: `RuntimeValue::NominalRecord` with exact type ID, layout, and
  ordinal fields;
- domain enums: `RuntimeValue::Variant { path: Some(exact_type_id), name,
  payload }`; bridge validation checks the fixed name/discriminant table;
- `Option`: exact path `Some("Option")`, name `Some` with one payload or `None`
  with no payload;
- bytes: dense U8 sequence; no base64, JSON array, or string;
- extension sequences: canonical order by `TtsOptionId`, no duplicates;
- no anonymous named-field map is accepted for any TTS nominal record.

Relevant canonical runtime-value tags remain:

```text
String=7, Tuple=11, Seq=12, Record=13, Variant=14, NominalRecord=15
NominalRecord bytes = 0x0f || u32le(type_id_len) || type_id_utf8 ||
                      layout[32] || u32le(field_count) || field_values...
Variant bytes       = 0x0e || option(path) || name || option(payload)
```

Core adds the inverse decoder for this existing canonical format. The decoder
must consume exactly one value and reject trailing bytes.

```rust
pub fn decode_canonical_runtime_value(
    bytes: &[u8],
    limits: RuntimeSchemaLimits,
) -> Result<RuntimeValue, RuntimeValueCodecError>;
```

`RuntimeValueCodecError` is closed and typed:

```rust
pub enum RuntimeValueCodecError {
    Empty,
    UnknownTag { offset: usize, tag: u8 },
    Truncated { offset: usize, needed: usize, remaining: usize },
    InvalidUtf8 { offset: usize },
    InvalidScalar { offset: usize, kind: RuntimeScalarKind },
    LengthOverflow { offset: usize },
    DepthLimit { maximum: usize },
    NodeLimit { maximum: usize },
    SequenceLimit { maximum: usize, actual: usize },
    StringLimit { maximum: usize, actual: usize },
    BinaryLimit { maximum: usize, actual: usize },
    EncodedLimit { maximum: usize, actual: usize },
    Trailing { offset: usize, remaining: usize },
}
```

`RuntimeSchemaLimits` gains generic `max_binary_bytes`. `Bytes(Binary)` validates
a dense byte sequence without expanding one node per byte; its byte length is
checked against `max_binary_bytes`. This is generic core owner behavior and has
no TTS variant.

## 7. Per-payload validation limits

| Contract | max depth | max nodes | max sequence items | max string bytes | max binary bytes | max encoded bytes |
|---|---:|---:|---:|---:|---:|---:|
| intent | 16 | 1,024 | 64 | 65,536 | 4,096 | 131,072 |
| selected request | 16 | 2,048 | 64 | 65,536 | 4,096 | 131,072 |
| progress | 8 | 64 | 8 | 0 | 0 | 1,024 |
| error | 16 | 256 | 8 | 1,024 | 4,096 | 16,384 |
| audio asset | 16 | 512 | 64 | 256 | 33,554,432 | 33,685,504 |

Additional exact accepted limits are enforced before encoding and after decode:

```text
text: nonempty; <=65,536 UTF-8 bytes; <=16,384 Unicode scalars
extensions: <=16; sorted unique; <=4,096 canonical bytes
timeout: 1,000..=120,000 ms; default 30,000
retry maximum_attempts: 1..=3; default 2
progress: <=128 published events; basis points 0..=10,000
result bytes: <=32 MiB
selected request canonical bytes: <=128 KiB
queued TTS tasks: <=256; global active: <=32
```

## 8. AWBC codec 8

The canonical AWBC codec version is directly replaced from 7 to 8. No dual
reader is retained.

```rust
pub struct AwbcTaskPlan {
    pub public_id: AwbcStringId,                 // field 0
    pub need_id: AwbcStringId,                   // field 1
    pub signature: AwbcTypeId,                   // field 2
    pub class: AwbcTaskClass,                    // field 3
    pub priority: i32,                           // field 4
    pub cancel_scope: AwbcStringId,              // field 5
    pub policy: AwbcTaskPolicy,                  // field 6
    pub request: AwbcTaskRequest,                // field 7
    pub outcome: AwbcTaskOutcomeContract,        // field 8
    pub many: Option<AwbcAwaitManyPlan>,         // field 9
}

pub enum AwbcTaskRequest {
    Host {
        capability: AwbcStringId,
        operation: AwbcStringId,
        arguments: Vec<AwbcTaskArgument>,
    },
    Intent {
        payload_type: AwbcTypeId,
    },
}

pub struct AwbcRuntimeSchemaLimits {
    pub max_depth: u32,
    pub max_nodes: u32,
    pub max_sequence_items: u32,
    pub max_string_bytes: u32,
    pub max_binary_bytes: u32,
    pub max_encoded_bytes: u32,
}

pub enum AwbcRuntimePayloadContract {
    Nominal {
        type_id: AwbcStringId,
        layout: [u8; 32],
        field_count: u16,
        max_canonical_bytes: u32,
    },
    Schema {
        payload_type: AwbcTypeId,
        layout: [u8; 32],
        limits: AwbcRuntimeSchemaLimits,
    },
}

pub struct AwbcTaskOutcomeContract {
    pub ready: AwbcRuntimePayloadContract,
    pub error: AwbcRuntimePayloadContract,
    pub progress: Option<AwbcRuntimePayloadContract>,
    pub cancellation: AwbcTaskCancellationContract,
}

pub enum AwbcTaskCancellationContract {
    Cancelled,
    Error { payload: AwbcConstantId },
}
```

The codec-8 wire tags are exact: task request `Host=0`, `Intent=1`; payload
contract `Nominal=0`, `Schema=1`; cancellation `Cancelled=0`, `Error=1`.
`AwbcTaskOutcomeContract` field order is ready, error, progress, cancellation.
For TTS all three payload contracts must be `Nominal`; `Schema` is rejected for
those three positions. The cancellation constant is the exact nominal
`TtsError::Cancelled` payload.

A TTS `StartTask` supplies exactly one operand: the nominal intent value.
`AwbcTaskRequest::Intent.payload_type` must index a record type whose
`public_id` is `std.audio.TtsSynthesisIntent`, whose derived layout is the exact
hash above, and whose field count is four. `MakeRecord` must consume its `ty`
operand: `public_id = Some` constructs a nominal ordinal record; `None` retains
the existing anonymous record behavior. Missing/extra operands, wrong type
index, wrong layout, or unknown tags are verification errors before execution.

No AWBC field contains `tts.synthesize`, `tts.synthesis`, a Rust type-name
string, JSON/TOML, selected request, provider, or registration.

## 9. Generic and bridge codec errors

```rust
pub enum RuntimePayloadContractError {
    ContractLayout { expected: TypeLayoutHash, actual: TypeLayoutHash },
    NominalType { expected: RuntimeNominalTypeId, actual: RuntimeNominalTypeId },
    NominalLayout { expected: TypeLayoutHash, actual: TypeLayoutHash },
    NominalFieldCount { expected: u16, actual: usize },
    Schema { source: RuntimeSchemaError },
    Canonical { source: RuntimeValueCodecError },
    CanonicalBytes { maximum: u32, actual: usize },
}

#[repr(u8)]
pub enum TtsPayloadLimit {
    Depth = 0,
    Nodes = 1,
    SequenceItems = 2,
    StringBytes = 3,
    BinaryBytes = 4,
    CanonicalBytes = 5,
    TextBytes = 6,
    TextScalars = 7,
    Extensions = 8,
    ExtensionBytes = 9,
    ProgressEvents = 10,
    ResultBytes = 11,
}

pub enum TtsPayloadFieldError {
    ValueKind,
    NestedNominal { source: Box<TtsPayloadDecodeError> },
    VariantPath,
    VariantDiscriminant,
    PayloadPresence,
    OptionShape,
    Identity,
    ScalarRange,
    DigestLength { expected: u16, actual: usize },
    NonCanonicalOrder,
    DuplicateExtension,
}

pub enum TtsPayloadEncodeError {
    Field { ordinal: u16, source: TtsPayloadFieldError },
    Canonical { source: RuntimeValueCodecError },
    Limit { name: TtsPayloadLimit, maximum: u64, actual: u64 },
}

pub enum TtsPayloadDecodeError {
    NominalType { expected: RuntimeNominalTypeId, actual: RuntimeNominalTypeId },
    Layout { expected: TypeLayoutHash, actual: TypeLayoutHash },
    FieldCount { expected: u16, actual: usize },
    Field { ordinal: u16, source: TtsPayloadFieldError },
    Canonical { source: RuntimeValueCodecError },
    Limit { name: TtsPayloadLimit, maximum: u64, actual: u64 },
}

#[repr(u8)]
pub enum TtsOutcomePayloadRole {
    Progress = 0,
    Ready = 1,
    Error = 2,
}

pub struct TtsCodecFault {
    pub field_ordinal: Option<u16>,
    pub codec_offset: Option<u32>,
    pub limit: Option<TtsPayloadLimit>,
    pub maximum: Option<u64>,
    pub actual: Option<u64>,
}

pub enum TtsRuntimeDiagnostic {
    IntentNominalMismatch { expected: RuntimeNominalTypeId, actual: RuntimeNominalTypeId },
    IntentLayoutMismatch { expected: TypeLayoutHash, actual: TypeLayoutHash },
    IntentCodecInvalid { fault: TtsCodecFault },
    SelectedNominalMismatch { expected: RuntimeNominalTypeId, actual: RuntimeNominalTypeId },
    SelectedLayoutMismatch { expected: TypeLayoutHash, actual: TypeLayoutHash },
    OutcomeNominalMismatch {
        role: TtsOutcomePayloadRole,
        expected: RuntimeNominalTypeId,
        actual: RuntimeNominalTypeId,
    },
    OutcomeLayoutMismatch {
        role: TtsOutcomePayloadRole,
        expected: TypeLayoutHash,
        actual: TypeLayoutHash,
    },
    OutcomeCodecInvalid { role: TtsOutcomePayloadRole, fault: TtsCodecFault },
    RegistrationMissing { registration: HostAdapterRegistrationId },
    ReplayOutcomePayloadInvalid {
        role: TtsOutcomePayloadRole,
        source: RuntimePayloadContractError,
    },
    ReplayResultBytesOmitted {
        type_id: RuntimeNominalTypeId,
        layout: TypeLayoutHash,
        omitted_bytes: u64,
    },
}

impl TtsRuntimeDiagnostic {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::IntentNominalMismatch { .. } =>
                "tts.runtime.intent-nominal-mismatch",
            Self::IntentLayoutMismatch { .. } =>
                "tts.runtime.intent-layout-mismatch",
            Self::IntentCodecInvalid { .. } =>
                "tts.runtime.intent-codec-invalid",
            Self::SelectedNominalMismatch { .. } =>
                "tts.runtime.selected-nominal-mismatch",
            Self::SelectedLayoutMismatch { .. } =>
                "tts.runtime.selected-layout-mismatch",
            Self::OutcomeNominalMismatch { .. } =>
                "tts.runtime.outcome-nominal-mismatch",
            Self::OutcomeLayoutMismatch { .. } =>
                "tts.runtime.outcome-layout-mismatch",
            Self::OutcomeCodecInvalid { .. } =>
                "tts.runtime.outcome-codec-invalid",
            Self::RegistrationMissing { .. } =>
                "tts.runtime.registration-missing",
            Self::ReplayOutcomePayloadInvalid { .. } =>
                "tts.replay.outcome-payload-invalid",
            Self::ReplayResultBytesOmitted { .. } =>
                "tts.replay.result-bytes-omitted",
        }
    }
}
```

The diagnostic enum carries no request text, provider
key, credential, restricted digest, provider payload, audio bytes, or content
digest. `RuntimePayloadContractError` is the sole generic boundary failure; it never
contains a display-only payload string. Intent decode errors become typed
request-stage `ProtocolFailure/InvalidPayload`
and one stable diagnostic. Selected payload decode errors prevent host dispatch.
Outcome decode errors become typed completion-stage
`ProtocolFailure/InvalidPayload`. Replay decode errors stop replay. No boundary
uses `Display` text as a task error.
