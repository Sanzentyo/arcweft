# Source, resource, and manifest model

## 1. Typed resource surface

The only authored declaration is a typed `res` using the Lang-01.4 final
resource machinery. The descriptor identity is exactly:

```text
ResourceTypeId = std.audio.TtsVoiceProfile
Public ID family = tts_voice_profile.*
```

Example:

```arcw
pub res @tts_voice_profile.game.akane_ja akane_ja: std.audio.TtsVoiceProfile {
    character = @character.game.akane
    tts_speaker = @tts_speaker.game.akane
    locales = ["ja-JP"]
    default_for_character = true
    selection_priority = 0
    style = .neutral
    rate_milli = 1000
    pitch_cents = 0
}
```

### Exact descriptor fields

| Ordinal | Field | Type | Presence/default | Validation |
|---:|---|---|---|---|
| 0 | `character` | `Option<CharacterRef>` | optional, `none` | Exact retained Character identity; never `ResourceRef<Character>`. |
| 1 | `tts_speaker` | `TtsSpeakerId` | required | Exact `tts_speaker.*` family. |
| 2 | `locales` | `Seq<TtsLocaleId>` | required | 1–16, authored order, unique; first is default. |
| 3 | `default_for_character` | `Bool` | default `false`; forbidden `true` when `character = none` | Exactly one default per Character after project-wide validation. |
| 4 | `selection_priority` | `U16` | default `0` | Unique per Character; retained for unbound profiles but ignored in direct profile selection. |
| 5 | `style` | `Option<TtsStyleId>` | default `none` | Exact style, no fallback. |
| 6 | `rate_milli` | `U16` | default `1000` | 500–2000. |
| 7 | `pitch_cents` | `I16` | default `0` | -1200–1200. |

The resource value lowers to one `TtsVoiceProfile` plus an optional
`CharacterTtsProfileBinding`. The profile semantic digest includes resolved
`CharacterId`, not the source alias spelling. Every accepted profile also
registers its exact logical `TtsSpeakerId` in the project TTS speaker index. The
`@tts_speaker.*` syntax is therefore a typed reference to an ID introduced by
profile data, not a dedicated source declaration. A direct speaker call must
resolve in that index.

No `voice`, `voice profile`, `speaker`, or TTS top-level declaration is created.
Historical forms are rejected by ordinary current grammar and leave no CST,
AST, HIR, descriptor, or executable node.

## 2. Ordinary function surface

The standard callable publication is exact:

```text
tts.synthesize_profile(
    profile: ResourceRef<std.audio.TtsVoiceProfile>,
    text: String,
    locale: Option<TtsLocaleId> = none,
    options: TtsSynthesisOptions = .default,
) -> Need<TtsAudioAsset, TtsError>
!effect(tts.synthesize)

tts.synthesize_character(
    character: CharacterRef,
    text: String,
    profile: Option<ResourceRef<std.audio.TtsVoiceProfile>> = none,
    locale: Option<TtsLocaleId> = none,
    options: TtsSynthesisOptions = .default,
) -> Need<TtsAudioAsset, TtsError>
!effect(tts.synthesize)

tts.synthesize_speaker(
    tts_speaker: TtsSpeakerId,
    text: String,
    locale: TtsLocaleId,
    options: TtsSynthesisOptions = .default,
) -> Need<TtsAudioAsset, TtsError>
!effect(tts.synthesize)
```

`TtsSynthesisOptions` is a nominal record:

```text
style: Option<TtsStyleId> = none
rate_milli: Option<U16> = none
pitch_cents: Option<I16> = none
format: Option<AudioFormat> = none
extensions: Map<TtsOptionId, TtsOptionValue> = {{}}
timeout_millis: U32 = 30_000
retry: TtsRetryPolicy = .default
```

`TtsOptionId` uses `LOWER_LOCAL`, max 64 bytes. `TtsOptionValue` is a closed
scalar enum: `Bool`, `I64`, `U64`, `Milli(i32)`, or `String`; a String value is
at most 512 UTF-8 bytes and 512 scalars. There is no raw JSON/TOML value.

The three calls lower through the ordinary callable registry, type checker,
ordinary suspension, Need, Task, and cancellation contracts. They do not use a
TTS-specific source statement or stream syntax.

### Tooling

LSP hover and signature help show the exact names and types above. Completion
never proposes `speaker`, `voice`, provider IDs, provider keys, or credential
fields. Go-to-definition for `profile` resolves the typed `res`; for
`character`, the retained Character declaration; for `tts_speaker`, the
profile field/source literal when present. Provider metadata is not a source
definition target.

## 3. Sole project manifest path

All handwritten TTS deployment data is under the schema-1 project manifest:

```toml
[tts.providers."tts_provider.acme"]
module = "acme-tts"
export = "acme_tts"
credential-ref = "secret.tts.acme"

[tts.providers."tts_provider.acme".public-config]
region = "ap-northeast-1"
endpoint-class = "standard"

[tts.bindings."tts_binding.game.akane_acme"]
tts-speaker = "@tts_speaker.game.akane"
provider = "tts_provider.acme"
provider-key = "ja-JP-Wavenet-A"
locales = ["ja-JP"]
styles = ["neutral"]
priority = 10
```

### Exact Rust shapes

```rust
pub struct TtsSpec {
    pub providers: BTreeMap<TtsProviderId, TtsProviderInstanceSpec>,
    pub bindings: BTreeMap<TtsProviderBindingId, TtsProviderSpeakerBindingSpec>,
}

pub struct TtsProviderInstanceSpec {
    pub module: ExternalModuleImportId,
    pub export: AdapterExportId,
    pub credential_ref: Option<SecretRef>,
    pub public_config: BTreeMap<TtsConfigFieldId, TtsPublicConfigValue>,
}

pub struct TtsProviderSpeakerBindingSpec {
    pub tts_speaker: TtsSpeakerIdRef,
    pub provider: TtsProviderId,
    pub provider_key: TtsProviderSpeakerKey,
    pub locales: NonEmptyVec<TtsLocaleId>,
    pub styles: Vec<TtsStyleId>,
    pub priority: u16,
}
```

`TtsSpeakerIdRef` accepts exactly an `@` reference whose target family is
`tts_speaker.*`; the accepted join stores `TtsSpeakerId`, not the `@` marker.

### Manifest limits

- providers: 0–64;
- bindings: 0–8192;
- public config fields per provider: 0–32;
- public config encoded bytes per provider: at most 16 KiB;
- binding locales: 1–32;
- binding styles: 0–32.

### Source-map ownership

The sole decoder records ranges for:

```text
tts
  providers
    <provider-id> table key and full table
      module
      export
      credential-ref
      public-config table and every key/value
  bindings
    <binding-id> table key and full table
      tts-speaker
      provider
      provider-key
      locales and each array element
      styles and each array element
      priority
```

The source map is bound to the exact `SourceBackedManifest` document revision.
Duplicate diagnostics retain first and later ranges. Accepted TTS records never
reparse or slice source text.

## 4. Generated adapter metadata extension

The existing generated JSON document with:

```text
schema = 1
format = arcweft.adapter-metadata
```

is extended directly, without a version bump or parallel decoder, by adding the
canonical field `exports.tts_providers`. Empty is allowed. Each record is:

```rust
pub struct AdapterTtsProviderExport {
    pub export: AdapterExportId,
    pub provider: TtsProviderId,
    pub protocol: TtsProviderProtocolId, // exactly arcweft-tts-provider-v1
    pub capabilities: TtsProviderCapabilities,
    pub public_config_schema: Vec<TtsPublicConfigFieldSchema>,
    pub credential_required: bool,
}
```

The enclosing existing `AdapterTarget::{Rust,Wasm,Process}` is the sole target
and transport-family owner. The TTS export does not duplicate a transport
field. The accepted provider catalog derives one closed
`TtsAdapterTargetFamily::{Rust,Wasm,Process}` coordinate from that target for
dispatch and canonical binary encoding. Capability locale/style/format and extension-option schema vectors are
canonical sorted unique data. A provider advertises at most 32 extension-option
schemas, each using the closed option value kinds/ranges/enums. Public-config
schema fields are sorted by ID,
unique, and use the closed types `Bool`, `I64`, `U64`, `String`, or
`StringList`; each may carry a bounded numeric range or finite string enum.

The existing generated-metadata raw hash, payload hash, ABI hash, package,
module, export, and artifact identities bind the TTS record. Both the metadata
payload digest and ABI digest include every TTS export field.

## 5. Accepted join and publication

The single transaction is:

```text
SourceBackedManifest revision
+ immutable external module metadata/artifact handles
+ generated adapter metadata schema-1 decode
+ accepted typed-resource/profile catalog revision
+ active capability policy
-> validate all provider/profile/binding joins
-> construct immutable TtsAcceptedCatalog
-> compute profile/provider semantic digests
-> construct complete candidate launch topology
-> publish once
```

Any error rejects the complete candidate. No partial provider, profile,
callable, capability, bundle, or runtime publication is visible. The profile
catalog is a valid independently published component when the launch topology
does not authorize executable TTS; in that case an empty provider catalog is
valid and dialogue/profile metadata remains available. Once `tts.synthesize` is
authorized or statically reachable, every reachable logical speaker/profile
locale must have at least one accepted provider binding or the TTS launch
portion is rejected before runtime dispatch.

Validation order is deterministic:

1. local field/type/limit validation;
2. duplicate ID and source-range diagnostics;
3. module/import/export existence;
4. metadata raw/payload/ABI/artifact checks;
5. provider identity/protocol/transport/config schema checks;
6. speaker/profile/Character catalog checks;
7. binding capability/locale/style checks;
8. capability policy and concrete host implementation checks;
9. canonical digest construction;
10. atomic publication.

## 6. Secret and restricted-field policy

`credential-ref` contains only a `SecretRef`. The manifest decoder rejects a
value that looks like a credential payload rather than a reference, including
inline tables/arrays, multiline strings, or values over the SecretRef grammar.
Unknown fields such as `api-key`, `token`, `secret`, or `password` receive the
ordinary strict-manifest unknown-field diagnostic; the accepted typed model has
no slot for them.

`provider-key` is permitted because it is the provider's opaque voice key, not
a credential. It is immediately wrapped in `TtsProviderSpeakerKey`; diagnostics
may point to its source range but must not echo its value.

## 7. Historical spelling behavior

The final accepted names are `tts-speaker`, `tts_speaker`, and
`tts.synthesize_*` as specified here. The following are not aliases:

```text
speaker
voice
voice-profile
tts.synthesis
```

They fail through ordinary current unknown-field, unknown-argument,
unknown-call, or grammar diagnostics. No removed-name code or migration path is
added.
