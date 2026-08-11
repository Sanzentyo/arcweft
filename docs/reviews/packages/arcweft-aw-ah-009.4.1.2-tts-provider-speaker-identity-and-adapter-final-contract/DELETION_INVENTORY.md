# Deletion inventory

This inventory separates verified current provisional paths from final-surface
deletions that must be discovered through typed APIs during implementation.
Nothing is deleted by this design package.

## 1. Verified current direct replacements

| Current path / symbol | Current defect | Final action |
|---|---|---|
| `crates/arcweft-core/src/task.rs::TtsRequest { voice: Option<String>, text: String }` | Conflates an untyped `voice` string with profile/speaker/provider choices and lacks locale/options/evidence/limits. | Delete the struct and use `arcweft_audio_tts::TtsSynthesisRequest` in the existing enum variant. No wrapper or conversion alias. |
| `HostTaskRequest::debug_label` TTS branch using `request.voice` / `default` | Exposes ambiguous identity and cannot obey text/key privacy. | Replace directly with the exact sanitized count/locale/format label. |
| `crates/arcweft-core/src/engine/suspend.rs` branch `("tts", "synthesize" | "synthesis")` | String dispatcher accepts an alias and the ambiguous `voice` argument. | Delete the whole TTS branch. Typed standard-call lowering emits the intent variant; runtime-driver prepares
it at `dispatch_requested_tasks`. No legacy branch remains. |
| `crates/arcweft-core/src/task.rs::TaskEventKind::Err(String)` | Cannot carry structured provider errors. | Directly change the owning enum to `Err(RuntimePayload)` and update all exhaustive matches. |
| `crates/arcweft-host-adapter/src/lib.rs::HostTaskOutcome.result: Result<RuntimePayload, String>` | Same structured-error defect. | Directly change to `Result<RuntimePayload, RuntimePayload>`. |
| replay schema-1 `RecordedExternalOutcomeResultV1::Failure { kind, message }` | Cannot preserve exact typed TTS error and retry/provider context. | Directly change schema 1 to carry the typed error payload; no v1/v2 dual reader. |
| `crates/arcweft-lang-syntax/src/parser/headers.rs::EntityDeclKind::Voice`, `"voice profile"`, `"voice"` | Dedicated resource-family keyword conflicts with final Lang-01.4 `res` surface. | Delete in the Lang-01.4 direct switch. No removed-token recognizer. |
| corresponding `EntityDeclKind::Voice` CST/AST/HIR/sema/bundle/tooling branches | Carries the same removed top-level family. | Delete exhaustively in the same compiling cut; use typed resource descriptor instead. |
| `docs/03-presentation/audio.md` TTS sketches and `pub voice profile` examples | Aspirational text assumes a crate/API/provider model absent from production. | Rewrite to the final typed resource, logical speaker, provider manifest, Need result, and explicit playback separation. |
| `docs/examples/audio.md` and `docs/schemas/audio-manifest.md` voice-profile/provider examples | Historical documentation surface. | Rewrite or remove in Cut 7; no legacy examples. |

## 2. Required typed API deletion audit

During Cuts 5 and 7, compile/API evidence must enumerate and replace every:

- provider-valued field or parameter named `speaker`;
- Character-valued field or parameter named `speaker`;
- TTS provider-key field named `voice`, `speaker`, or untyped `id`;
- public callable parameter named `voice` for TTS;
- generated metadata field named `speaker` instead of `tts_speaker` or
  `provider_key`;
- test fixture constructing the old stringly TTS request;
- source signature or signature-help fixture advertising `tts.synthesis`.

The audit is performed through compiler errors, exhaustive enum matches,
trybuild/API fixtures, typed metadata decoders, and runtime calls. A source-text
search is not accepted as proof of absence.

## 3. Explicitly preserved items

The following are not deleted or redesigned by this sequence:

- `CharacterId` and Character registration;
- `CharacterDialogue`, AW-AH-009.4 Cut 1, dialogue View identity/projection, and
  display-name localization;
- `CharacterDialogueVoiceId` as an independent already-selected presentation
  reference; it has no automatic TTS conversion;
- `AudioResourceId`, `AudioVoiceId`, `AudioBusId`, effects, snapshots, and
  physical output-device identities;
- `TaskClass::TtsSynthesis`, `Need`, Task/Need cancellation, scheduler event
  ordering, and `TaskPolicy::JoinSameKey`;
- host-adapter registry ownership, policy, pending completion, and cancellation
  methods;
- existing single-manifest/generated-metadata substrate;
- `AudioFormat`, `DecodedAudio`, codec, mixer, and CPAL output path;
- quiescent save blocker and generation pinning;
- replay external-outcome injection.

## 4. No compatibility interval

The final switch is atomic. There is no point at which both old and final TTS
source arguments, manifest fields, metadata fields, runtime requests, error
wires, or bundle catalogs are accepted. Historical input fails as ordinary
unknown current input. No migration shim, alias, dual reader, source gate, CSS,
or Takumi route is permitted.
