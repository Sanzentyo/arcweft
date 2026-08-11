# Test matrix

```text
NORMATIVE_ROWS=316
PRODUCTION_RESULTS_RECORDED=NO
```

Every row is required for the completed broad runtime cut. Unit/property tests
must use production owners and exact constants. Compile-fail, Cargo metadata,
structured AWBC inspection, and behavioral execution are normative; source-text
presence/absence alone is not acceptance evidence.

| ID | Tier | Owner/target | Case | Required result |
|---|---|---|---|---|
| SRC-001 | T1 | sema/runtime-plan | profile ordinary call with all arguments | Resolved callable is Tts::SynthesizeProfile; one intent template; Need<TtsAudioAsset,TtsError>. |
| SRC-002 | T1 | sema/runtime-plan | profile ordinary call with defaults | Defaults are typed in shared registry; same four-field intent shape. |
| SRC-003 | T1 | sema/runtime-plan | character call with explicit profile | Selector Character has exact character/profile ordinals. |
| SRC-004 | T1 | sema/runtime-plan | character call without profile | Canonical Option::None; no display-name/default inference in lowering. |
| SRC-005 | T1 | sema/runtime-plan | speaker call with mandatory locale | Selector Speaker and locale lower exactly. |
| SRC-006 | T1 | sema/runtime-plan | callable effect facts | Only tts.synthesize effect is present. |
| SRC-007 | T1 | sema/runtime-plan | direct-style await inside ordinary fn | Suspends through ordinary Need path; no TTS syntax node. |
| SRC-008 | T1 | sema/runtime-plan | try await / await? | Ordinary Try over awaited Result; no TTS-specific propagation. |
| SRC-009 | T1 | sema/runtime-plan | tts.synthesis spelling | Unknown callable; no compatibility diagnostic/alias. |
| SRC-010 | T1 | sema/runtime-plan | voice named argument | Unknown argument under current grammar. |
| SRC-011 | T1 | sema/runtime-plan | speaker compatibility callable/keyword | Unknown current grammar surface. |
| SRC-012 | T1 | sema/runtime-plan | raw provider ID argument attempt | Type-check rejection; no provider callable. |
| SRC-013 | T1 | sema/runtime-plan | shared callable identity serialization | U8 discriminants 0/1/2 stable; no name matching. |
| SRC-014 | T1 | sema/runtime-plan | shadowed local tts symbol | Shared resolver follows ordinary shadowing; no source-text gate. |
| SRC-015 | T1 | sema/runtime-plan | source-map duplicate extension option | Source-aware duplicate diagnostic before runtime encoding. |
| SRC-016 | T1 | sema/runtime-plan | text retained exactly | No trim, normalization, SSML parsing, or newline rewrite. |
| SRC-017 | T1 | sema/runtime-plan | empty text | Typed validation failure; no task publication. |
| SRC-018 | T1 | sema/runtime-plan | unrelated ordinary callable | Existing host task lowering unchanged. |
| AWBC-001 | T1 | core AWBC | codec-8 structured round trip | Byte-identical re-encode and one nominal intent operand. |
| AWBC-002 | T1 | core AWBC | codec version 7 input | Rejected; no dual reader. |
| AWBC-003 | T1 | core AWBC | unknown task-request tag | Verifier rejects before VM. |
| AWBC-004 | T1 | core AWBC | Host request tag 0 | Existing host shape round trips. |
| AWBC-005 | T1 | core AWBC | Intent request tag 1 | Payload type index resolves to exact intent nominal. |
| AWBC-006 | T1 | core AWBC | Intent with zero StartTask operands | Verifier rejects exact arity. |
| AWBC-007 | T1 | core AWBC | Intent with two operands | Verifier rejects exact arity. |
| AWBC-008 | T1 | core AWBC | wrong payload type index | Verifier rejects. |
| AWBC-009 | T1 | core AWBC | anonymous record as intent | Verifier rejects nominal requirement. |
| AWBC-010 | T1 | core AWBC | intent type ID mismatch | Verifier rejects. |
| AWBC-011 | T1 | core AWBC | intent layout mismatch | Verifier rejects. |
| AWBC-012 | T1 | core AWBC | intent field count 3 | Verifier rejects missing field. |
| AWBC-013 | T1 | core AWBC | intent field count 5 | Verifier rejects extra field. |
| AWBC-014 | T1 | core AWBC | MakeRecord public nominal | VM creates RuntimeValue::NominalRecord using ty and ordinals. |
| AWBC-015 | T1 | core AWBC | MakeRecord anonymous type | Existing anonymous RuntimeValue::Record behavior retained. |
| AWBC-016 | T1 | core AWBC | TTS ready contract uses Schema instead of Nominal | Verifier rejects. |
| AWBC-017 | T1 | core AWBC | TTS error contract uses Schema instead of Nominal | Verifier rejects. |
| AWBC-018 | T1 | core AWBC | TTS outcome contract missing progress | Verifier rejects. |
| AWBC-019 | T1 | core AWBC | TTS cancellation bare Cancelled | Verifier rejects; exact typed error required. |
| AWBC-020 | T1 | core AWBC | TTS cancellation constant wrong nominal | Verifier rejects. |
| AWBC-021 | T1 | core AWBC | AWBC truncation at every byte boundary | Always typed truncation; no panic/partial plan. |
| AWBC-022 | T1 | core AWBC | AWBC trailing byte | Rejected. |
| AWBC-023 | T1 | core AWBC | AWBC exact maximum accepted build | Accepted. |
| AWBC-024 | T1 | core AWBC | AWBC one byte over accepted maximum | Rejected with named budget. |
| AWBC-025 | T1 | core AWBC | structured inspection of TTS task | No operation string, JSON, selected provider, or registration. |
| LAY-001 | T1 | audio-tts-runtime | TtsSynthesisIntent canonical encode/decode | Domain value exactly equal; re-encode byte-identical. |
| LAY-002 | T1 | audio-tts-runtime | TtsSynthesisIntent wrong top nominal ID | NominalType error and stable diagnostic; no publication. |
| LAY-003 | T1 | audio-tts-runtime | TtsSynthesisIntent wrong top layout hash | Layout error and stable diagnostic; no publication. |
| LAY-004 | T1 | audio-tts-runtime | TtsSynthesisIntent one field missing | Exact FieldCount error. |
| LAY-005 | T1 | audio-tts-runtime | TtsSynthesisIntent one field extra | Exact FieldCount error. |
| LAY-006 | T1 | audio-tts-runtime | TtsSynthesisIntent malformed nested nominal identity | Field ordinal identifies nested failure. |
| LAY-007 | T1 | audio-tts-runtime | TtsSynthesisIntent malformed nested nominal layout | Field ordinal identifies nested failure. |
| LAY-008 | T1 | audio-tts-runtime | TtsSynthesisIntent wrong enum path/name/payload presence | Closed-enum rejection. |
| LAY-009 | T1 | audio-tts-runtime | TtsSynthesisIntent truncate canonical bytes at every offset | Typed Truncated; no panic or partial value. |
| LAY-010 | T1 | audio-tts-runtime | TtsSynthesisIntent append one trailing byte | Typed Trailing. |
| LAY-011 | T1 | audio-tts-runtime | TtsSynthesisIntent exact 131072 canonical-byte cap | Accepted. |
| LAY-012 | T1 | audio-tts-runtime | TtsSynthesisIntent 131073 canonical bytes | EncodedLimit one-over rejection. |
| LAY-013 | T1 | audio-tts-runtime | TtsSynthesisRequest canonical encode/decode | Domain value exactly equal; re-encode byte-identical. |
| LAY-014 | T1 | audio-tts-runtime | TtsSynthesisRequest wrong top nominal ID | NominalType error and stable diagnostic; no publication. |
| LAY-015 | T1 | audio-tts-runtime | TtsSynthesisRequest wrong top layout hash | Layout error and stable diagnostic; no publication. |
| LAY-016 | T1 | audio-tts-runtime | TtsSynthesisRequest one field missing | Exact FieldCount error. |
| LAY-017 | T1 | audio-tts-runtime | TtsSynthesisRequest one field extra | Exact FieldCount error. |
| LAY-018 | T1 | audio-tts-runtime | TtsSynthesisRequest malformed nested nominal identity | Field ordinal identifies nested failure. |
| LAY-019 | T1 | audio-tts-runtime | TtsSynthesisRequest malformed nested nominal layout | Field ordinal identifies nested failure. |
| LAY-020 | T1 | audio-tts-runtime | TtsSynthesisRequest wrong enum path/name/payload presence | Closed-enum rejection. |
| LAY-021 | T1 | audio-tts-runtime | TtsSynthesisRequest truncate canonical bytes at every offset | Typed Truncated; no panic or partial value. |
| LAY-022 | T1 | audio-tts-runtime | TtsSynthesisRequest append one trailing byte | Typed Trailing. |
| LAY-023 | T1 | audio-tts-runtime | TtsSynthesisRequest exact 131072 canonical-byte cap | Accepted. |
| LAY-024 | T1 | audio-tts-runtime | TtsSynthesisRequest 131073 canonical bytes | EncodedLimit one-over rejection. |
| LAY-025 | T1 | audio-tts-runtime | TtsProgress canonical encode/decode | Domain value exactly equal; re-encode byte-identical. |
| LAY-026 | T1 | audio-tts-runtime | TtsProgress wrong top nominal ID | NominalType error and stable diagnostic; no publication. |
| LAY-027 | T1 | audio-tts-runtime | TtsProgress wrong top layout hash | Layout error and stable diagnostic; no publication. |
| LAY-028 | T1 | audio-tts-runtime | TtsProgress one field missing | Exact FieldCount error. |
| LAY-029 | T1 | audio-tts-runtime | TtsProgress one field extra | Exact FieldCount error. |
| LAY-030 | T1 | audio-tts-runtime | TtsProgress malformed nested nominal identity | Field ordinal identifies nested failure. |
| LAY-031 | T1 | audio-tts-runtime | TtsProgress malformed nested nominal layout | Field ordinal identifies nested failure. |
| LAY-032 | T1 | audio-tts-runtime | TtsProgress wrong enum path/name/payload presence | Closed-enum rejection. |
| LAY-033 | T1 | audio-tts-runtime | TtsProgress truncate canonical bytes at every offset | Typed Truncated; no panic or partial value. |
| LAY-034 | T1 | audio-tts-runtime | TtsProgress append one trailing byte | Typed Trailing. |
| LAY-035 | T1 | audio-tts-runtime | TtsProgress exact 1024 canonical-byte cap | Accepted. |
| LAY-036 | T1 | audio-tts-runtime | TtsProgress 1025 canonical bytes | EncodedLimit one-over rejection. |
| LAY-037 | T1 | audio-tts-runtime | TtsAudioAsset canonical encode/decode | Domain value exactly equal; re-encode byte-identical. |
| LAY-038 | T1 | audio-tts-runtime | TtsAudioAsset wrong top nominal ID | NominalType error and stable diagnostic; no publication. |
| LAY-039 | T1 | audio-tts-runtime | TtsAudioAsset wrong top layout hash | Layout error and stable diagnostic; no publication. |
| LAY-040 | T1 | audio-tts-runtime | TtsAudioAsset one field missing | Exact FieldCount error. |
| LAY-041 | T1 | audio-tts-runtime | TtsAudioAsset one field extra | Exact FieldCount error. |
| LAY-042 | T1 | audio-tts-runtime | TtsAudioAsset malformed nested nominal identity | Field ordinal identifies nested failure. |
| LAY-043 | T1 | audio-tts-runtime | TtsAudioAsset malformed nested nominal layout | Field ordinal identifies nested failure. |
| LAY-044 | T1 | audio-tts-runtime | TtsAudioAsset wrong enum path/name/payload presence | Closed-enum rejection. |
| LAY-045 | T1 | audio-tts-runtime | TtsAudioAsset truncate canonical bytes at every offset | Typed Truncated; no panic or partial value. |
| LAY-046 | T1 | audio-tts-runtime | TtsAudioAsset append one trailing byte | Typed Trailing. |
| LAY-047 | T1 | audio-tts-runtime | TtsAudioAsset exact 33685504 canonical-byte cap | Accepted. |
| LAY-048 | T1 | audio-tts-runtime | TtsAudioAsset 33685505 canonical bytes | EncodedLimit one-over rejection. |
| LAY-049 | T1 | audio-tts-runtime | TtsError canonical encode/decode | Domain value exactly equal; re-encode byte-identical. |
| LAY-050 | T1 | audio-tts-runtime | TtsError wrong top nominal ID | NominalType error and stable diagnostic; no publication. |
| LAY-051 | T1 | audio-tts-runtime | TtsError wrong top layout hash | Layout error and stable diagnostic; no publication. |
| LAY-052 | T1 | audio-tts-runtime | TtsError one field missing | Exact FieldCount error. |
| LAY-053 | T1 | audio-tts-runtime | TtsError one field extra | Exact FieldCount error. |
| LAY-054 | T1 | audio-tts-runtime | TtsError malformed nested nominal identity | Field ordinal identifies nested failure. |
| LAY-055 | T1 | audio-tts-runtime | TtsError malformed nested nominal layout | Field ordinal identifies nested failure. |
| LAY-056 | T1 | audio-tts-runtime | TtsError wrong enum path/name/payload presence | Closed-enum rejection. |
| LAY-057 | T1 | audio-tts-runtime | TtsError truncate canonical bytes at every offset | Typed Truncated; no panic or partial value. |
| LAY-058 | T1 | audio-tts-runtime | TtsError append one trailing byte | Typed Trailing. |
| LAY-059 | T1 | audio-tts-runtime | TtsError exact 16384 canonical-byte cap | Accepted. |
| LAY-060 | T1 | audio-tts-runtime | TtsError 16385 canonical bytes | EncodedLimit one-over rejection. |
| LAY-061 | T1 | core/bridge codec | intent text 65,536 bytes and 16,384 scalars | Accepted when both exact limits hold. |
| LAY-062 | T1 | core/bridge codec | intent text 65,537 bytes | RequestLimit; no preparation. |
| LAY-063 | T1 | core/bridge codec | intent text 16,385 scalars under byte cap | RequestLimit; no preparation. |
| LAY-064 | T1 | core/bridge codec | intent empty text | RequestLimit/validation error; no publication. |
| LAY-065 | T1 | core/bridge codec | 16 extension options / 4,096 encoded bytes | Accepted. |
| LAY-066 | T1 | core/bridge codec | 17 extension options | Rejected one-over. |
| LAY-067 | T1 | core/bridge codec | 4,097 extension bytes | Rejected one-over. |
| LAY-068 | T1 | core/bridge codec | duplicate extension ID | Rejected; canonical sequence not produced. |
| LAY-069 | T1 | core/bridge codec | unsorted extension IDs in payload | Decode rejects noncanonical order. |
| LAY-070 | T1 | core/bridge codec | timeout 1,000 / 120,000 | Both accepted. |
| LAY-071 | T1 | core/bridge codec | timeout 999 / 120,001 | Both rejected exact one-over/under. |
| LAY-072 | T1 | core/bridge codec | retry 1 / 3 | Both accepted. |
| LAY-073 | T1 | core/bridge codec | retry 0 / 4 | Both rejected. |
| LAY-074 | T1 | core/bridge codec | digest nested bytes 32 | Accepted. |
| LAY-075 | T1 | core/bridge codec | digest nested bytes 31 / 33 | Rejected. |
| LAY-076 | T1 | core/bridge codec | asset bytes 33,554,432 | Accepted after digest/decode validation. |
| LAY-077 | T1 | core/bridge codec | asset bytes 33,554,433 | ResultTooLarge. |
| LAY-078 | T1 | core/bridge codec | binary validation dense U8 | No per-byte node expansion; binary budget applied. |
| LAY-079 | T1 | core/bridge codec | canonical decoder unknown tag | UnknownTag with exact offset. |
| LAY-080 | T1 | core/bridge codec | canonical decoder invalid UTF-8 | InvalidUtf8 with exact offset. |
| LAY-081 | T1 | core/bridge codec | canonical Option path absent or non-Option | Bridge rejects noncanonical path. |
| LAY-082 | T1 | core/bridge codec | anonymous named-field TTS record | Rejected; ordinal nominal only. |
| PREP-001 | T1 | runtime-driver preparation | valid profile intent and available catalog | Selected request/key/spec created; no publication before return. |
| PREP-002 | T1 | runtime-driver preparation | valid character explicit profile | Exact binding/evidence selected. |
| PREP-003 | T1 | runtime-driver preparation | valid character default profile | Accepted deterministic default rule. |
| PREP-004 | T1 | runtime-driver preparation | valid logical speaker | Exact speaker/provider selection. |
| PREP-005 | T1 | runtime-driver preparation | InvalidIdentity | Same observer receives exact variant; atomic no-dispatch. |
| PREP-006 | T1 | runtime-driver preparation | UnknownProfile | Exact variant; atomic no-dispatch. |
| PREP-007 | T1 | runtime-driver preparation | MissingCharacterProfile | Exact variant; atomic no-dispatch. |
| PREP-008 | T1 | runtime-driver preparation | CharacterProfileMismatch | Exact variant; atomic no-dispatch. |
| PREP-009 | T1 | runtime-driver preparation | UnknownSpeaker | Exact variant; atomic no-dispatch. |
| PREP-010 | T1 | runtime-driver preparation | UnknownProvider | Exact variant; atomic no-dispatch. |
| PREP-011 | T1 | runtime-driver preparation | UnsupportedLocale | Exact variant; atomic no-dispatch. |
| PREP-012 | T1 | runtime-driver preparation | UnsupportedStyle | Exact variant; atomic no-dispatch. |
| PREP-013 | T1 | runtime-driver preparation | UnsupportedOption | Exact variant; atomic no-dispatch. |
| PREP-014 | T1 | runtime-driver preparation | UnsupportedFormat | Exact variant; atomic no-dispatch. |
| PREP-015 | T1 | runtime-driver preparation | CapabilityUnavailable | Exact variant; atomic no-dispatch. |
| PREP-016 | T1 | runtime-driver preparation | ProviderUnavailable snapshot | Exact variant; atomic no-dispatch. |
| PREP-017 | T1 | runtime-driver preparation | QueueLimit 256 exact | Accepted if all other budgets permit. |
| PREP-018 | T1 | runtime-driver preparation | QueueLimit 257 | QueueLimit maximum=256; no host dispatch. |
| PREP-019 | T1 | runtime-driver preparation | RequestLimit selected canonical 128 KiB exact | Accepted. |
| PREP-020 | T1 | runtime-driver preparation | RequestLimit selected canonical one-over | RequestLimit; no publication. |
| PREP-021 | T1 | runtime-driver preparation | malformed intent nominal | ProtocolFailure Request/InvalidPayload; diagnostic; no dispatch. |
| PREP-022 | T1 | runtime-driver preparation | missing availability entry | Typed preparation error; no implicit unavailable fallback. |
| PREP-023 | T1 | runtime-driver preparation | catalog generation changes during local preparation | Arc snapshot remains immutable and selected evidence self-consistent. |
| PREP-024 | T1 | runtime-driver preparation | failure before terminal error encoding | No sequence or registry mutation. |
| PREP-025 | T1 | runtime-driver preparation | terminal error registry commit | One terminal record and one event atomically; never Pending. |
| PREP-026 | T1 | runtime-driver preparation | registration absent/mismatched | Typed protocol error and diagnostic; no host dispatch. |
| PREP-027 | T1 | runtime-driver preparation | fingerprint/key mismatch after selected encode | Rejected before scheduler. |
| PREP-028 | T1 | runtime-driver preparation | non-TTS intent reaches TTS preparer | Typed unsupported nominal failure; no downcast/Any. |
| JOIN-001 | T1 | audio fingerprint/scheduler | identical selected requests | one Scheduled + one Joined; one host dispatch/pin |
| JOIN-002 | T1 | audio fingerprint/scheduler | profile presence | different keys |
| JOIN-003 | T1 | audio fingerprint/scheduler | profile ID | different keys |
| JOIN-004 | T1 | audio fingerprint/scheduler | character presence | different keys |
| JOIN-005 | T1 | audio fingerprint/scheduler | character ID | different keys |
| JOIN-006 | T1 | audio fingerprint/scheduler | logical speaker | different keys |
| JOIN-007 | T1 | audio fingerprint/scheduler | provider ID | different keys |
| JOIN-008 | T1 | audio fingerprint/scheduler | binding ID | different keys |
| JOIN-009 | T1 | audio fingerprint/scheduler | provider-key digest | different keys |
| JOIN-010 | T1 | audio fingerprint/scheduler | exact text bytes | different keys |
| JOIN-011 | T1 | audio fingerprint/scheduler | locale | different keys |
| JOIN-012 | T1 | audio fingerprint/scheduler | style presence/value | different keys |
| JOIN-013 | T1 | audio fingerprint/scheduler | rate | different keys |
| JOIN-014 | T1 | audio fingerprint/scheduler | pitch | different keys |
| JOIN-015 | T1 | audio fingerprint/scheduler | format | different keys |
| JOIN-016 | T1 | audio fingerprint/scheduler | extension option ID | different keys |
| JOIN-017 | T1 | audio fingerprint/scheduler | extension option value | different keys |
| JOIN-018 | T1 | audio fingerprint/scheduler | profile catalog digest | different keys |
| JOIN-019 | T1 | audio fingerprint/scheduler | provider catalog digest | different keys |
| JOIN-020 | T1 | audio fingerprint/scheduler | availability digest | different keys |
| JOIN-021 | T1 | audio fingerprint/scheduler | capability digest | different keys |
| JOIN-022 | T1 | audio fingerprint/scheduler | public-config digest | different keys |
| JOIN-023 | T1 | audio fingerprint/scheduler | artifact identity | different keys |
| JOIN-024 | T1 | audio fingerprint/scheduler | ABI digest | different keys |
| JOIN-025 | T1 | audio fingerprint/scheduler | timeout | different keys |
| JOIN-026 | T1 | audio fingerprint/scheduler | retry maximum attempts | different keys |
| JOIN-027 | T1 | audio fingerprint/scheduler | TaskId only | same key and joins |
| JOIN-028 | T1 | audio fingerprint/scheduler | NeedId only | same key and joins |
| JOIN-029 | T1 | audio fingerprint/scheduler | cancel scope only | same key and joins with separate observers |
| JOIN-030 | T1 | audio fingerprint/scheduler | priority only | same key; joins and owner scheduling priority remains first admitted task |
| JOIN-031 | T1 | audio fingerprint/scheduler | same key but different ready/error/progress/cancellation contract | ContractMismatch; no join or publication |
| JOIN-032 | T1 | audio fingerprint/scheduler | same key but different task class or policy | ContractMismatch; no join or publication |
| JOIN-033 | T1 | audio fingerprint/scheduler | accepted_generation only with identical sealed digests | same fingerprint/key |
| JOIN-034 | T1 | audio fingerprint/scheduler | attempt/credential slot only | not part of selected key |
| HOST-001 | T1 | host/Need/cancellation | one typed TTS registration | Builder returns one token and rejects duplicate ownership. |
| HOST-002 | T1 | host/Need/cancellation | artifact/ABI/provider/capability/config/protocol mismatch at registration | Builder rejects before publication. |
| HOST-003 | T1 | host/Need/cancellation | TTS HostTaskDispatch shape | Contains selected request, never intent/generic payload. |
| HOST-004 | T1 | host/Need/cancellation | generic string operation tts.synthesize | No branch/registration accepts it. |
| HOST-005 | T1 | host/Need/cancellation | old tts.synthesis operation | No branch/registration accepts it. |
| HOST-006 | T1 | host/Need/cancellation | provider request credential handling | Contains credential slot only; never value/ref text. |
| HOST-007 | T1 | host/Need/cancellation | host cannot reselect provider | Request provider/binding/key remain selected. |
| HOST-008 | T1 | host/Need/cancellation | wrong selected nominal at dispatch | No privileged submit; typed diagnostic/error. |
| HOST-009 | T1 | host/Need/cancellation | selected fingerprint differs from key | No privileged submit. |
| HOST-010 | T1 | host/Need/cancellation | debug label | Exact accepted redacted label. |
| HOST-011 | T1 | host/Need/cancellation | typed TTS token submit/drain/cancel/pump | All four operations require the retained token and preserve selected/domain types. |
| HOST-012 | T1 | host/Need/cancellation | wrong typed token or numeric registration | HostAdapterError mismatch/missing; no provider I/O. |
| HOST-013 | T1 | host/Need/cancellation | TtsAdapterCallId byte layout | Exact generation u64 LE followed by sequence u64 LE; 16 bytes. |
| NEED-001 | T1 | host/Need/cancellation | valid progress | AwaitProgress emitted; Need stays Pending. |
| NEED-002 | T1 | host/Need/cancellation | progress phase regression | Typed ProtocolFailure; no regressed progress. |
| NEED-003 | T1 | host/Need/cancellation | progress value regression | Typed ProtocolFailure. |
| NEED-004 | T1 | host/Need/cancellation | 128 progress events | Accepted/coalesced. |
| NEED-005 | T1 | host/Need/cancellation | 129th non-coalescible progress event | Typed ProtocolFailure. |
| NEED-006 | T1 | host/Need/cancellation | valid audio asset | AwaitReady and Result::Ok exact payload. |
| NEED-007 | T1 | host/Need/cancellation | valid TtsError | AwaitErr and Result::Err exact payload. |
| NEED-008 | T1 | host/Need/cancellation | try await error | Ordinary Try propagates typed TtsError. |
| NEED-009 | T1 | host/Need/cancellation | wrong progress nominal/layout | Completion InvalidPayload error; no string. |
| NEED-010 | T1 | host/Need/cancellation | wrong result nominal/layout | Completion InvalidPayload error; no asset publication. |
| NEED-011 | T1 | host/Need/cancellation | wrong error nominal/layout | Completion InvalidPayload replacement error. |
| NEED-012 | T1 | host/Need/cancellation | HostTaskOutcome typed failure | RuntimePayload retained end-to-end. |
| NEED-013 | T1 | host/Need/cancellation | non-TTS typed task error | Existing generic nominal error remains typed. |
| NEED-014 | T1 | host/Need/cancellation | provider Timeout | Exact TtsError::Timeout. |
| NEED-015 | T1 | host/Need/cancellation | ProviderFailure all fields | Exact structured payload and redaction. |
| NEED-016 | T1 | host/Need/cancellation | ProtocolFailure all stage/code values | Exact discriminants round trip. |
| NEED-017 | T1 | host/Need/cancellation | ResultTooLarge | Exact maximum/observed values. |
| NEED-018 | T1 | host/Need/cancellation | InvalidAudio all kinds | Exact typed variants. |
| CANCEL-001 | T1 | host/Need/cancellation | single queued observer cancel | Typed Cancelled Err; no host dispatch; pin released. |
| CANCEL-002 | T1 | host/Need/cancellation | one of two joined scopes cancels | Only matching observer Err(Cancelled); execution continues. |
| CANCEL-003 | T1 | host/Need/cancellation | all joined observers cancel | One targeted execution cancel; all observers typed Cancelled. |
| CANCEL-004 | T1 | host/Need/cancellation | duplicate cancel request | Idempotent; one terminal event per observer. |
| CANCEL-005 | T1 | host/Need/cancellation | provider event after final cancellation | Discarded and counted; no Need mutation. |
| CANCEL-006 | T1 | host/Need/cancellation | active cooperative cancel | One provider cancel then cleanup. |
| CANCEL-007 | T1 | host/Need/cancellation | active noncooperative cancel | Abort at cleanup deadline; typed Cancelled. |
| CANCEL-008 | T1 | host/Need/cancellation | cancellation during retry delay | No further attempt; typed Cancelled. |
| CANCEL-009 | T1 | host/Need/cancellation | owner scope differs from joined observer | Targeted cancel never kills remaining observer. |
| CANCEL-010 | T1 | host/Need/cancellation | non-TTS cancellation contract | Bare Cancelled behavior remains available. |
| SAVE-001 | T1/T2 | save/replay/reload | active TTS execution | HostTasks and TaskGenerationPins block save. |
| SAVE-002 | T1/T2 | save/replay/reload | queued selected TTS execution | Existing blockers block save. |
| SAVE-003 | T1/T2 | save/replay/reload | joined observer | HostTasks blocker counts observer; only one pin. |
| SAVE-004 | T1/T2 | save/replay/reload | preparation failure event consumed | No TTS blocker remains. |
| SAVE-005 | T1/T2 | save/replay/reload | completed asset 32 MiB | Ordinary nominal save succeeds under aggregate cap. |
| SAVE-006 | T1/T2 | save/replay/reload | completed asset one byte over | Save rejects exact per-asset cap. |
| SAVE-007 | T1/T2 | save/replay/reload | aggregate TTS asset bytes 256 MiB exact | Accepted. |
| SAVE-008 | T1/T2 | save/replay/reload | aggregate one byte over | Rejected. |
| SAVE-009 | T1/T2 | save/replay/reload | save payload privacy | No provider key/credential/catalog/call/progress/spool. |
| SAVE-010 | T1/T2 | save/replay/reload | restore wrong asset nominal/layout/digest | Rejected before state publication. |
| REPLAY-001 | T1/T2 | save/replay/reload | recorded success with complete asset | Same typed bytes injected; provider suppressed. |
| REPLAY-002 | T1/T2 | save/replay/reload | recorded typed failure | Same TtsError injected; provider suppressed. |
| REPLAY-003 | T1/T2 | save/replay/reload | joined success | One recorded owner outcome; deterministic scheduler fan-out. |
| REPLAY-004 | T1/T2 | save/replay/reload | joined failure | One recorded owner error; deterministic fan-out. |
| REPLAY-005 | T1/T2 | save/replay/reload | SuccessOmitted marker | Explicit omitted-bytes replay failure; no provider fallback. |
| REPLAY-006 | T1/T2 | save/replay/reload | success missing bytes but no marker | Invalid payload replay failure. |
| REPLAY-007 | T1/T2 | save/replay/reload | wrong task key | ExternalTaskMismatch. |
| REPLAY-008 | T1/T2 | save/replay/reload | wrong task class | ExternalTaskMismatch. |
| REPLAY-009 | T1/T2 | save/replay/reload | wrong logical epoch/sequence | ExternalTaskMismatch. |
| REPLAY-010 | T1/T2 | save/replay/reload | duplicate task outcome | Duplicate/mismatch failure. |
| REPLAY-011 | T1/T2 | save/replay/reload | out-of-order task outcome | Mismatch failure. |
| REPLAY-012 | T1/T2 | save/replay/reload | wrong success nominal/layout | InvalidExternalTaskPayload. |
| REPLAY-013 | T1/T2 | save/replay/reload | wrong failure nominal/layout | InvalidExternalTaskPayload. |
| REPLAY-014 | T1/T2 | save/replay/reload | truncated canonical outcome | InvalidExternalTaskPayload. |
| REPLAY-015 | T1/T2 | save/replay/reload | trailing canonical outcome | InvalidExternalTaskPayload. |
| REPLAY-016 | T1/T2 | save/replay/reload | asset digest mismatch | Replay failure; no Need result. |
| REPLAY-017 | T1/T2 | save/replay/reload | preparation failure during recording/replay | No external outcome recorded; deterministic typed failure recomputed. |
| REPLAY-018 | T1/T2 | save/replay/reload | schema_version != 1 | Rejected; no TTS schema 2. |
| RELOAD-001 | T1/T2 | save/replay/reload | queued exact compatibility tuple and fingerprint | Migrates registration/pin atomically; same key/observer order. |
| RELOAD-002 | T1/T2 | save/replay/reload | profile semantic digest change | All observers CatalogChanged; no host dispatch. |
| RELOAD-003 | T1/T2 | save/replay/reload | binding ID change | CatalogChanged. |
| RELOAD-004 | T1/T2 | save/replay/reload | provider ID change | CatalogChanged. |
| RELOAD-005 | T1/T2 | save/replay/reload | provider-key digest change | CatalogChanged. |
| RELOAD-006 | T1/T2 | save/replay/reload | capability digest change | CatalogChanged. |
| RELOAD-007 | T1/T2 | save/replay/reload | public-config digest change | CatalogChanged. |
| RELOAD-008 | T1/T2 | save/replay/reload | artifact identity change | CatalogChanged. |
| RELOAD-009 | T1/T2 | save/replay/reload | ABI hash change | CatalogChanged. |
| RELOAD-010 | T1/T2 | save/replay/reload | credential-ref canonical text change | CatalogChanged without exposing ref. |
| RELOAD-011 | T1/T2 | save/replay/reload | protocol ID change | CatalogChanged. |
| RELOAD-012 | T1/T2 | save/replay/reload | tuple equal but fingerprint differs | CatalogChanged; no migration. |
| RELOAD-013 | T1/T2 | save/replay/reload | candidate selected provider unavailable or availability digest changes | CatalogChanged; no dispatch. |
| RELOAD-014 | T1/T2 | save/replay/reload | active execution during reload | Completes under old pin/request; no duplicate dispatch. |
| RELOAD-015 | T1/T2 | save/replay/reload | pin swap failure | Old queued state remains intact; no partial candidate publication. |
| DEP-001 | T1/T2 | dependency/visibility/diagnostics | cargo metadata core dependency closure | Contains no audio/TTS/bridge crate. |
| DEP-002 | T1/T2 | dependency/visibility/diagnostics | cargo metadata sema closure | Contains no audio/TTS/bridge crate. |
| DEP-003 | T1/T2 | dependency/visibility/diagnostics | cargo metadata scheduler closure | Only generic core among task-domain crates. |
| DEP-004 | T1/T2 | dependency/visibility/diagnostics | cargo metadata bridge direct deps | Exactly core + audio TTS (+ thiserror utility). |
| DEP-005 | T1/T2 | dependency/visibility/diagnostics | cargo metadata host-adapter closure | No bridge or runtime-driver dependency. |
| DEP-006 | T1/T2 | dependency/visibility/diagnostics | cargo metadata audio TTS closure | No core/runtime upper dependency. |
| DEP-007 | T1/T2 | dependency/visibility/diagnostics | cargo metadata runtime-plan | Has bridge and no host/runtime-driver dependency. |
| DEP-008 | T1/T2 | dependency/visibility/diagnostics | cargo metadata provider adapter | No core intent/bridge dependency. |
| VIS-001 | T1/T2 | dependency/visibility/diagnostics | trybuild core imports TtsSynthesisIntent | Compile fail. |
| VIS-002 | T1/T2 | dependency/visibility/diagnostics | trybuild external crate calls RuntimeTaskIntent constructor | Compile fail/private. |
| VIS-003 | T1/T2 | dependency/visibility/diagnostics | trybuild host constructs TtsIntentPayload | Compile fail/no dependency/API. |
| VIS-004 | T1/T2 | dependency/visibility/diagnostics | trybuild host submits TtsSynthesisIntent | Compile fail/type mismatch. |
| VIS-005 | T1/T2 | dependency/visibility/diagnostics | trybuild provider executor accepts RuntimePayload | Compile fail/trait mismatch. |
| VIS-006 | T1/T2 | dependency/visibility/diagnostics | trybuild external unchecked nominal constructor | Compile fail/private. |
| VIS-007 | T1/T2 | dependency/visibility/diagnostics | public API inventory | No TTS V2/Compat/Legacy/alias/extension trait. |
| STR-001 | T1/T2 | dependency/visibility/diagnostics | scheduler type inventory | Exactly one RuntimeScheduler owner; no TTS scheduler. |
| STR-002 | T1/T2 | dependency/visibility/diagnostics | replay model inventory | Exactly one external_outcomes vector; no TTS log. |
| STR-003 | T1/T2 | dependency/visibility/diagnostics | save blocker inventory | Only HostTasks and TaskGenerationPins for active TTS. |
| STR-004 | T1/T2 | dependency/visibility/diagnostics | host registration inventory | Exactly one typed tts.synthesize registration; no string claim. |
| STR-005 | T1/T2 | dependency/visibility/diagnostics | AWBC structural decode | No JSON/TOML/string operation envelope. |
| STR-006 | T1/T2 | dependency/visibility/diagnostics | core API inventory | No TTS-specific task/request variant. |
| STR-007 | T1/T2 | dependency/visibility/diagnostics | owner behavior audit | New enum behavior is inherent; no local extension trait. |
| DIAG-001 | T1/T2 | dependency/visibility/diagnostics | intent nominal mismatch diagnostic | Exact code/structured fields; redacted. |
| DIAG-002 | T1/T2 | dependency/visibility/diagnostics | intent layout mismatch diagnostic | Exact code/structured fields; redacted. |
| DIAG-003 | T1/T2 | dependency/visibility/diagnostics | intent codec invalid diagnostic | Exact code/offset/limit; redacted. |
| DIAG-004 | T1/T2 | dependency/visibility/diagnostics | selected nominal/layout mismatch | Exact selected codes; no host dispatch. |
| DIAG-005 | T1/T2 | dependency/visibility/diagnostics | outcome nominal/layout/codec mismatch | Exact outcome codes; typed replacement error. |
| DIAG-006 | T1/T2 | dependency/visibility/diagnostics | registration missing | Exact code; no operation-string fallback. |
| DIAG-007 | T1/T2 | dependency/visibility/diagnostics | replay invalid payload | Exact code; replay stops. |
| DIAG-008 | T1/T2 | dependency/visibility/diagnostics | replay omitted bytes | Exact code; no provider fallback. |
| DIAG-009 | T1/T2 | dependency/visibility/diagnostics | debug formatting request/result/error | No text/key/credential/digest/bytes. |
| DIAG-010 | T1/T2 | dependency/visibility/diagnostics | provider sanitized message | <=1,024 bytes and removes controls/secrets/text substrings. |
| DIAG-011 | T1/T2 | dependency/visibility/diagnostics | diagnostic code exhaustiveness | All eleven enum variants map to the exact stable code by exhaustive match. |
| T2-001 | T2 | broad runtime cut | native desktop simulated TTS provider | Typed request/progress/result/error/cancel end-to-end. |
| T2-002 | T2 | broad runtime cut | native provider protocol chunk/spool path | 4 MiB memory threshold, 32 MiB cap, cleanup on all terminals. |
| T2-003 | T2 | broad runtime cut | Web host simulated provider | Same typed dispatch/Need/replay semantics; no native-only shortcut. |
| T2-004 | T2 | broad runtime cut | headless deterministic run | Same preparation/key/join/replay ordering. |
| T2-005 | T2 | broad runtime cut | Agent observation | No request text, provider key, credential, audio bytes, or restricted digest. |
| T2-006 | T2 | broad runtime cut | native/Web/headless recorded success replay | Identical typed asset injection without provider. |
| T2-007 | T2 | broad runtime cut | native/Web/headless typed failure replay | Identical TtsError injection. |
| T2-008 | T2 | broad runtime cut | reload under native/Web/headless adapters | Queued/active generation semantics equal. |
| T2-009 | T2 | broad runtime cut | workspace fmt/check/clippy | All pass with no warning allowance added for cut. |
| T2-010 | T2 | broad runtime cut | workspace all-target/all-feature tests | All pass. |
| T2-011 | T2 | broad runtime cut | cargo metadata structural assertions | All allowed/forbidden edge rows pass. |
| T2-012 | T2 | broad runtime cut | final deletion/audit | No provisional bridge, alias, old branch, second scheduler/log, or source-text gate. |

## Matrix execution rules

- Exact-limit rows must exercise production constructors/codecs, not reduced
  local meters.
- Truncation rows iterate every possible cut point of a valid canonical value.
- Tamper rows mutate one coordinate at a time and assert no partial
  scheduler/registry/pin/replay/host publication.
- Join separation rows use the same logical task timing and vary only the named
  coordinate.
- All typed-error rows decode the observed payload back to the exact `TtsError`
  variant; comparing display strings is insufficient.
- Tier 2 rows record exact commands, target/platform, commit, Jujutsu change ID,
  and result in implementation evidence.
