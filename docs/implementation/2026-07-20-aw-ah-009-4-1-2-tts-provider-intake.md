# AW-AH-009.4.1.2 TTS provider/speaker identity intake

## Package and readiness

The source package is
`arcweft-aw-ah-009.4.1.2-tts-provider-speaker-identity-and-adapter-final-contract.zip`
with SHA-256
`cb087cc2e4e137edde1732c11df579a1c71371769633bfdcf807fd367b30fdc1`.
It is a design-only package with `STATUS=READY_FOR_IMPLEMENTATION`,
179 normative test rows, eight ordered implementation cuts, and no
result-changing open questions.

The selected final model is a new lower Sans-I/O `arcweft-audio-tts` crate.
It owns nominal TTS identities, immutable profile/provider catalogs, typed
intent/request/result/error/progress records, canonical codecs, limits, and
the AWTP adapter protocol. Provider SDKs, process/network work, secrets, clocks,
queues, retry, and rate limiting remain host-adapter responsibilities.

The following identities remain nominally distinct:

```text
CharacterId
TtsVoiceProfileId
TtsSpeakerId
TtsProviderId
TtsProviderSpeakerKey
AudioVoiceId
AudioBusId
physical output device identity
```

No display-name, basename, declaration-name, Character, dialogue presentation
voice, provider-key, or raw-string fallback is permitted.

## Entry-gate result

Cut 1 cannot begin on the current checkout because its explicit entry gates are
not both closed:

1. Lang-01.5.1 has a functioning single launch path, but its final
   source-backed manifest and accepted topology migration is still active; and
2. Lang-01.4 has private typed-`res` substrate, but its final public
   `ResourceRef<T>`/retained-identity owner and direct keyword switch have not
   landed.

The package mandates Cut 1 through Cut 8 in order and forbids publishing a
transient legacy/final dual surface. Starting `arcweft-audio-tts` at Cut 2,
adding provisional provider manifest types, or restoring a dedicated
`voice`/`voice profile` declaration would violate that order. This intake
therefore records a real predecessor gate rather than inventing an
implementation subset.

The gate does not block Proof, AW-AH-007/008, AW-AH-009.3, dialogue profile
ownership, or Character projection work. Dialogue/View projection must remain
fully usable without a TTS provider, catalog, capability, adapter, or
credential.

## Final implementation order

After both predecessor gates close:

1. extend the sole manifest decoder and generated adapter-metadata schema,
   reserve AWFB section kinds 22/23, and add typed capabilities;
2. add `arcweft-audio-tts` and directly replace provisional stringly TTS
   request and task-error carriers;
3. add immutable profile/provider catalogs, canonical codecs, and one atomic
   accepted publication transaction;
4. add host-only adapter dispatch, provider executors, secret leases, queues,
   timeout/retry/cancellation, and AWTP simulation;
5. publish `std.audio.TtsVoiceProfile` through final `res` and expose only the
   three ordinary typed `tts.synthesize_*` functions;
6. integrate preparation, `TaskKey::for_tts`, Need completion, save blocking,
   replay, reload, privacy, and non-interference;
7. delete provisional `voice`, `speaker`, `tts.synthesis`, raw error, and
   resource-keyword paths without aliases or removed-spelling diagnostics; and
8. run all 179 rows, workspace quality gates, canonical tamper vectors,
   dependency checks, Tier 2, and native/Web/headless validation.

## Current direct-replacement inventory

The implementation must eventually replace, not wrap:

- `arcweft-core::task::TtsRequest { voice: Option<String>, text: String }`;
- the string-dispatched `tts.synthesize | tts.synthesis` suspension branch;
- `TaskEventKind::Err(String)`;
- `HostTaskOutcome.result: Result<RuntimePayload, String>`;
- replay external failures that flatten the error to strings; and
- the legacy `voice`/`voice profile` declaration family as part of the final
  Lang-01.4 `res` switch.

`CharacterDialogueVoiceId` is explicitly preserved as an independent
presentation reference and receives no implicit TTS conversion.

## Prohibited outcomes

No compatibility alias, dual reader, provisional wire version, extension
trait, ad hoc endpoint conversion helper, provider-specific manifest parser,
source gate, dedicated removed-name diagnostic, CSS path, or Takumi path may
be introduced. Provider keys and credential values may not appear in public
IDs, diagnostics, debug labels, Agent/MCP projection, or public bundle
summaries.

## Completion boundary

This package is accepted and queued behind its explicit Lang-01.4 and
Lang-01.5.1 entry gates. It remains incomplete until all eight cuts land in
order and all 179 matrix rows and broad validation gates pass.
