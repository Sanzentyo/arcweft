# Final contract

## 1. Status and scope

```text
STATUS=READY_FOR_IMPLEMENTATION
OPEN_RESULT_CHANGING_DECISIONS=0
OPEN_QUESTIONS=0
IMPLEMENTATION_PERFORMED=NO
BASE_COMMIT=e6e8cce33d4c09a9f9efa9ba2169fc5c6b0b7139
```

This contract introduces a real TTS subsystem. It is not a field rename and is
not a prerequisite for dialogue View character projection. It moves directly
from the current unreleased provisional shape to one final model.

The following identities are always distinct:

```text
CharacterId != TtsVoiceProfileId != TtsSpeakerId
TtsSpeakerId != TtsProviderId != TtsProviderSpeakerKey
TtsSpeakerId != AudioVoiceId != AudioBusId != physical output device identity
```

No string fallback, basename conversion, display-name lookup, declaration-name
lookup, historical `speaker` lookup, or implicit identity conversion exists.

## 2. Closed decisions

### D1 — subsystem owner

A new Sans-I/O crate named `arcweft-audio-tts` is selected. The crate is
justified by the complete identity/catalog/protocol surface, not by the name in
the earlier package. It owns no provider SDK and performs no I/O.

### D2 — public and restricted identities

The final types are `TtsVoiceProfileId`, `TtsSpeakerId`, `TtsProviderId`,
`TtsProviderBindingId`, `TtsProviderSpeakerKey`, `TtsLocaleId`, `TtsStyleId`,
`TtsRequestFingerprint`, and `TtsAdapterCallId`. Their exact grammars, limits,
serde forms, classification, and redaction rules are normative in
`IDENTITY_AND_MAPPING_MODEL.md`.

There is no public `TtsRequestId`. `TaskKey::for_tts` is derived from the
request fingerprint after deterministic provider selection.

### D3 — profile and Character relationship

An authored `std.audio.TtsVoiceProfile` typed resource names one logical
`TtsSpeakerId` and synthesis defaults. It may explicitly reference at most one
`CharacterId`. One Character may have many profiles; exactly one is marked the
Character default when any Character-bound profiles exist. Selection priorities
are unique per Character.

A non-dialogue caller may synthesize by explicit profile or explicit logical
speaker. Source code cannot select `TtsProviderId` or
`TtsProviderSpeakerKey`.

### D4 — source surface

`res` is the only declaration keyword. No `voice`, `voice profile`, `speaker`,
or TTS top-level keyword is added or retained. The three ordinary functions are:

```text
tts.synthesize_profile(profile, text, locale?, options?)
tts.synthesize_character(character, text, profile?, locale?, options?)
tts.synthesize_speaker(tts_speaker, text, locale, options?)
```

All return `Need<TtsAudioAsset, TtsError>` and carry the single effect
`tts.synthesize`. Character-valued parameters are named `character`; logical
TTS speaker parameters are named `tts_speaker`.

### D5 — manifest and generated metadata

The sole schema-1 `SourceBackedManifest` decoder owns handwritten provider
instances, provider-speaker bindings, public configuration, and credential
references. Existing `ExternalModuleImportSpec` owns adapter artifact and ABI
pins. The existing schema-1 generated `arcweft-adapter-metadata` document is
extended directly with `tts_providers`; it owns advertised locales, styles,
formats, limits, cancellation, progress, idempotency, and public-config schema.

There is one atomic join and publication transaction. There is no second TOML
or JSON parser and no provider-specific manifest reader.

### D6 — request and result

Ordinary source calls produce a typed `TtsSynthesisIntent`. The compiler uses
the typed TTS request-template variant; Core emits only an internal
`HostTaskRequest::TtsSynthesisIntent`. At the existing
`BundleSession::dispatch_requested_tasks` boundary, runtime-driver invokes the
inherent `TtsAcceptedCatalog::prepare_request` method against one accepted
generation and availability snapshot, then calls the inherent
`TaskSpec::prepare_tts` method. Only the resulting
`HostTaskRequest::TtsSynthesis(TtsSynthesisRequest)` with
`TaskKey::for_tts` may enter the task registry, scheduler, replay owner, or host
adapter. A preparation failure queues a typed `TaskEventKind::Err` for the same
task and emits no `HostTaskDispatch`.

The function resolves only when a complete, bounded, validated encoded audio
asset is available. Provider streaming is internal to the adapter protocol and
does not create a TTS-only source Stream model. Audio playback remains a
separate explicit operation through the existing audio resource/mixer path.

### D7 — wire ownership

The final initial versions are:

| Wire | Owner | Initial version |
|---|---|---|
| Project manifest | `arcweft-manifest-model` | schema 1, direct final extension |
| Generated adapter metadata | `arcweft-adapter-metadata` | schema 1, direct final extension |
| TTS profile bundle section | `arcweft-bundle` + `arcweft-audio-tts` codec | AWFB section kind 22, schema 1 |
| Restricted provider catalog section | same | AWFB section kind 23, schema 1 |
| Provider adapter frame protocol | `arcweft-audio-tts::protocol` | `arcweft-tts-provider-v1` / AWTP 1 |
| Root replay external error payload | existing replay owner | direct corrected schema 1 |

There is no released provider `speaker` wire, so no legacy reader or version
whose sole purpose is to remember the provisional sketch is permitted.

### D8 — adapter and capability boundary

`arcweft-host-adapter::tts::TtsHostAdapter` is the only owner of
`tts.synthesize`. Concrete provider SDK integrations implement
`TtsProviderExecutor` in provider-specific host crates. Lower crates never read
environment variables, files, credentials, clocks, process state, or network.

The adapter owns queues, per-provider concurrency, retry, timeouts, cancellation
cleanup, secret leases, protocol validation, output buffering/spooling, and
structured provider-error sanitization.

### D9 — diagnostics

Stable TTS diagnostics use the `tts.identity.*`, `tts.catalog.*`,
`tts.manifest.*`, `tts.adapter.*`, and `tts.runtime.*` namespaces listed in
`CAPABILITY_PRIVACY_AND_DIAGNOSTICS.md`. Old `speaker`, `voice`, and
`tts.synthesis` spellings receive ordinary current-grammar unknown-field,
unknown-argument, or unknown-call diagnostics only.

### D10 — limits and privacy

All exact limits are centralized in `arcweft-audio-tts::limits` and repeated in
`WIRE_VERSION_AND_LIMITS.md`. Provider keys are restricted artifact data;
credentials and credential values are secret. Neither is public semantic
identity or Agent/MCP-visible text. Request text and synthesized bytes are
content-sensitive. Default debug output exposes only counts and sanitized
metadata.

## 3. Required preserved substrate

The implementation MUST preserve, except for the one concrete structured-error
defect below:

- existing Character identity and registration;
- `CharacterDialogue` and dialogue View projection work;
- ordinary function, suspension, Need, task, cancellation, and scheduler flow;
- current deterministic task-event ordering;
- existing host-adapter policy, ownership rejection, pending completion, and
  cancellation hooks;
- single-manifest and generated-metadata ownership;
- audio codec, decoded PCM, mixer, buses, logical playback voices, and CPAL
  output device boundary;
- quiescent save blocking and generation pinning;
- external-outcome replay injection;
- Agent/MCP/accessibility/capture projection owners.

### Concrete defect and direct correction

Current host-task failures collapse to `String` in `TaskEventKind::Err`,
`HostTaskOutcome`, and recorded external outcomes. That cannot preserve the
structured provider class, sanitized provider code, retryability, request
fingerprint, and diagnostic fields required here. The final implementation
MUST directly replace those unreleased error carriers with `RuntimePayload` and
update every owner in one compiling cut. `TtsError` is encoded as the nominal
runtime type `std.audio.TtsError`. No parallel string/error form is allowed.

## 4. Projection non-blocking guarantee

Dialogue display name resolution, View evaluation, save/restore of dialogue
presentation, Agent observation, accessibility, capture, and the AW-AH-009.4
runtime sequence MUST NOT consult the TTS catalog. A dialogue value may retain
an optional already-selected presentation voice/profile reference, but no code
may infer a provider speaker from it. Missing `tts.synthesize`, a missing
credential, or an unavailable provider never invalidates an otherwise valid
dialogue View projection.

## 5. Prohibited outcomes

The implementation MUST NOT introduce:

- a compatibility alias, shim, dual reader, legacy wire, migration wrapper, or
  deprecated field;
- a source-text correctness gate or spelling-presence audit;
- a provider-specific TOML/JSON decoder or direct filesystem read below the
  loader;
- a dedicated removed-name diagnostic for `speaker`, `voice`, or
  `tts.synthesis`;
- provider I/O in `arcweft-audio-tts`, `arcweft-audio-core`,
  `arcweft-manifest-model`, `arcweft-adapter-metadata`, or `arcweft-core`;
- provider keys or credentials in public IDs, public bundle summaries, Agent or
  MCP output, source diagnostics, or ordinary debug labels;
- implicit locale fallback, Character/display-name fallback, or cross-provider
  retry after dispatch;
- CSS or Takumi integration.

## 6. Readiness criterion

Implementation is complete only after all eight cuts in
`IMPLEMENTATION_ORDER.md` have landed atomically at their stated boundaries and
all applicable rows in `TEST_MATRIX.md` pass. A partial implementation must not
publish the final manifest decoder, metadata schema, provider catalog, or
source functions independently.
