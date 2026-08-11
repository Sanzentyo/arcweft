# Normative delta from Lang-01.3.1.2 and provisional Lang-01.1.1 Stream shapes

This table is normative. A row marked **Replace/Delete** means there is no compatibility
alias, deprecated field, translation layer, or second product representation. Wire tags
refer to codec 8 unless the row explicitly says host/save JSON.

| Stable delta ID | Defective/provisional shape | Corrected owner/shape | Normative invariant | Wire/version allocation | Required action |
| --- | --- | --- | --- | --- | --- |
| DELTA-CALL-001 | Lang-01.3.1.2 `RuntimeParameterPassing::{Positional,Named,RestPositional,RestNamed}` | Existing `arcweft-core::entry` callable boundary projection | Five variants exactly: PositionalOnly, PositionalOrNamed, NamedOnly, RestPositional, RestNamed | AWBC tags 0..4 | Replace; no second resolver |
| DELTA-CALL-002 | Presence omitted Optional | Existing callable boundary projection | Required, Optional, Defaulted; only Optional may be Omitted | AWBC tags 0,1,2 | Replace |
| DELTA-CALL-003 | Stream-local resolved arguments | `RuntimeResolvedArguments<T>` in `arcweft-core::entry` | One declaration-order entry per accepted parameter; Value/Omitted | Argument tags 0,1 | Generalize owner in place |
| DELTA-CALL-004 | Optional rejection left open | Shared resolver/checker + runtime-plan projection | Optional is accepted and lossless; no rejection solely for Optional | Host JSON `type=value\|omitted` | Close decision |
| DELTA-CALL-005 | Source/default recovery possible | `RuntimeCallableProjectionError` in runtime-plan | Defaults already materialized; all failures carry existing call/parameter `SourceSpan` | No source bytes | Prohibit recovery |
| DELTA-ID-001 | One `StreamRuntimeId` conflating definition/state | `RuntimeStreamDefinitionId` + `RuntimeStreamDefinitionKey` + `StreamInstanceKey` | Static table identity separated from generation/ordinal instance identity | Definition transcript v1; IDs u32/u64 | Replace |
| DELTA-ID-002 | String/debug protocol identity | Typed key/newtypes | Diagnostic display is nonparseable evidence only | Definition key 32 bytes / 64-hex JSON | Delete parse path |
| DELTA-OWN-001 | Global map and `FiberState.stream_instances` both own state | One session/executor `StreamInstanceTable` | Every key has one Live or Tombstone entry | Snapshot one sorted vector | Replace both authorities |
| DELTA-OWN-002 | Fiber owns instance vector | `FiberState.producer_stream: Option<ProducerStreamRef>` plus affine handle values | Fibers hold references/values only | Save schema 2 | Remove vectors |
| DELTA-OWN-003 | Host lookup unspecified | `BTreeMap<StreamInstanceKey, StreamInstanceEntry>` | Structural validate, then exact `get_mut`; no scan/sidecar | No tag | Define |
| DELTA-OWN-004 | Affine movement unspecified across fibers | Sole table consumer lease + runtime value handle | Remove source occurrence, rotate lease, install destination atomically | Existing Move opcode | Define |
| DELTA-OWN-005 | Producer-child ownership ambiguous | Reciprocal `ProducerStreamRef` / `StreamProducerOwner::Fiber` | Key and producer lease must match both directions | Save schema 2 | Define |
| DELTA-OWN-006 | External producer identity implicit | `StreamProducerOwner::External` + `StreamExternalLifecycle` | Open and reserved close request IDs are authoritative | Host request JSON | Define |
| DELTA-OWN-007 | Duplicated compact/facade state rebuild | One `StreamInstanceTableSnapshot` | No `rebuild_facade_*`; candidate validates and swaps once | Save schema 2 | Delete |
| DELTA-OWN-008 | Terminal state plus separate tombstone/sidecar possible | `StreamInstanceEntry::{Live,Tombstone}` | Tombstone replaces live entry at same map key | Snapshot adjacent `kind`/`entry` tags | Define |
| DELTA-REPLAY-001 | No replay owner | `StreamReplayStore` embedded in live entry and moved to tombstone | One owner, stable FIFO record order | Save schema 2 | Add |
| DELTA-REPLAY-002 | Full unspecified | `StreamReplayRecordBody::Payload` | Only Recordable Full stores payload | body tag 0 | Define |
| DELTA-REPLAY-003 | HashOnly unspecified | `StreamReplayRecordBody::Digest` | BLAKE3-256 domain transcript; no retained payload | body tag 1 | Define |
| DELTA-REPLAY-004 | Summary unspecified | `StreamReplayRecordBody::Summary` | Type layout, canonical byte count, closed shape only | body tag 2 | Define |
| DELTA-REPLAY-005 | EventOnly unspecified | `StreamReplayRecordBody::EventOnly` | Envelope/event identity only | body tag 3 | Define |
| DELTA-REPLAY-006 | None represented ambiguously | `StreamReplayDecision::NoRecord` | No ID/record allocated; typed reason | No-record tags 0..5 when diagnostic | Define |
| DELTA-REPLAY-007 | Redaction left procedural | Privacy/replay projection matrix | Redacted never stores Payload; Full projects to Summary | Policy tags fixed | Define |
| DELTA-REPLAY-008 | Transient/Private retention unclear | NoRecord(Transient/Private) | No replay body; nonempty payload queue blocks save | No-record tags 1,2 | Define |
| DELTA-REPLAY-009 | Record identity/order absent | `StreamReplayRecordId`, commit, ingress, event | Strictly increasing IDs; append order is canonical | u64 string JSON | Define |
| DELTA-REPLAY-010 | Limit behavior absent | Per-instance/global record and byte limits | Append then deterministic oldest eviction; no side index | Profile fields | Define |
| DELTA-REPLAY-011 | Terminal retention unconstrained | Profile terminal replay cap | Payload→Digest→Summary→EventOnly downgrade only; never upgrade | Data-class tags 0..3 | Define |
| DELTA-REPLAY-012 | Payload erasure overclaimed/unspecified | Logical owner release rule | Drop all owners and verify no serialized/replay access; no physical zeroization claim | Behavioral tests | Define |
| DELTA-EFFECT-001 | `RuntimeEffectSetId` without table | `RuntimePlan.effect_sets: RuntimeEffectSetTable` | ID 0 empty; rows canonical, deduped, bounds checked | RuntimePlan position 8 | Add owner |
| DELTA-EFFECT-002 | Effect strings/ad hoc inventory possible | Existing accepted semantic `EffectId` inventory | Canonical effect IDs only; dependency direction preserved | UTF-8 member bytes | Reuse |
| DELTA-EFFECT-003 | AWBC mapping unspecified | `AwbcProgram.effect_sets` one-to-one | Runtime row n maps to AWBC row n after string remap | AWBC table position 3 | Define |
| DELTA-EFFECT-004 | Fingerprint omission possible | Executable identity + Stream definition transcript | Effect members participate; ID number alone never hashes | BLAKE3 transcript | Define |
| DELTA-PLAN-001 | `RuntimeSourceMapRef` proposed | Existing `SourceSpan` and `AwbcSourceMapId` | Debug evidence only; excluded from semantic identity | AWBC source_map position 26 | Delete proposed type |
| DELTA-PLAN-002 | `RuntimeStreamFrameLayout` proposed | Existing `AwbcFrameLayout`/ID | One frame owner shared with direct suspension | AWBC table position 5 | Delete proposed type |
| DELTA-PLAN-003 | Stream-local binding/expression/pattern/arm | Existing RuntimeBinding/RuntimeExpr/RuntimePattern/CFG/match owners | No duplicate AST/CFG or source recovery | Existing tables/opcodes | Reuse |
| DELTA-PLAN-004 | RuntimePlan old `streams` + `sources` | `stream_definitions` sole table | Sorted by semantic key; references checked | RuntimePlan position 9; AWBC position 22 | Replace |
| DELTA-PLAN-005 | Callable/flow tables could be omitted by new canonicalization | Existing top-level order retained and full AWBC executable identity used | Unrelated executable bytes remain fingerprint participants | AWBC positions 28,29,30 | Preserve |
| DELTA-PROFILE-001 | Stringly profile lookup/missing owner | manifest-model authored spec + launch accepted spans + compiler projection + runtime-plan accepted profile + core evidence | Native/Web/Agent built-ins plus monotonic project tightening; no sibling dependency inversion | RuntimePlan/bundle profile digest | Add exact pipeline owners |
| DELTA-PROFILE-002 | Blocking support unspecified | All three baseline targets `supports_provider_blocking=false` | ProviderBlocking rejected before RuntimePlan; no fallback | Backpressure tag 2 | Close decision |
| DELTA-PROFILE-003 | Privacy/permission minimum implicit | Explicit rank functions and floors | Project may only tighten | Policy profile schema | Define |
| DELTA-PROFILE-004 | Restart/replacement maxima absent | Typed flags and maxima | Native bounded support; Web/Agent reject | Policy fields | Define |
| DELTA-PROFILE-005 | Validation error order unspecified | 24 stable error codes | First error exactly POLICY_PROFILE section 8 order | Typed source/manifest range | Define |
| DELTA-EXH-001 | Exhaustion both rejection and mutation | Structural rejection vs accepted RuntimeLimit terminalization | Rejected has byte-identical state; terminalization consumes valid sequence only | Outcome `rejected\|terminalized` | Separate |
| DELTA-EXH-002 | Unchecked `emitted_count += 1` | Checked cursors/counters | No wrap or partial commit | u64 domain newtypes | Replace |
| DELTA-EXH-003 | Sequence max unspecified | `StreamSequenceCursor::{Next,Exhausted}` | At Next(u64::MAX), valid envelope terminalizes without body | RuntimeLimit tag 0 | Define |
| DELTA-EXH-004 | Close/result/observation repetition possible | Reserved close ID + flags in terminal state | Each emitted at most once; repeated input nonmutating | Host outcome/observation | Define |
| DELTA-DROP-001 | Sole consumer drop preserves unreachable queue | Registry synchronous cleanup | No vanished consumer drain requirement | Consumer state/observation | Correct |
| DELTA-DROP-002 | Cleanup mode absent | `DiscardQueued` | Drop queued payloads immediately; retention rule controls replay | drop tag 0 | Define |
| DELTA-DROP-003 | `DrainAndRetainReplay` lacked owner | Registry FIFO projection then payload drop | Requires replay!=None, Recordable/Redacted, ThroughTombstone, nonzero limits | drop tag 1 | Define |
| DELTA-DROP-004 | Terminal may overtake live queue | Live consumer queue-before-terminal invariant | NextStream drains FIFO then reports terminal | NextStream 0x8f | Preserve |
| DELTA-DROP-005 | Tombstone/release unbounded | Profile-bounded tombstones/replay and deterministic oldest releasable eviction | No forced state violation; creation blocks if none releasable | Save/profile fields | Define |
| DELTA-HOST-001 | Source ingress + separate Stream egress | `RuntimeStepInput.stream_events` | One normalized typed ingress | Canonical JSON | Replace |
| DELTA-HOST-002 | No per-event disposition owner | `RuntimeStepOutput.stream_event_outcomes` | Exactly one outcome per ingress event | accepted/terminalized/rejected tags | Add |
| DELTA-HOST-003 | Progress/control mixed with items/strings | `RuntimeEffectBatch.stream_observations` | Control/telemetry only; never T or payload duplicate | Observation JSON tags | Replace |
| DELTA-HOST-004 | Separate Source close request | `HostRequestBatch.stream_requests` | One vector for typed Open/Close | request `type=open\|close` | Replace |
| DELTA-HOST-005 | Endpoint DTOs possible | Shared core serde/codec | Native/Web/Agent exact byte parity | Same canonical JSON | Prohibit |
| DELTA-JSON-001 | Only selected newtype IDs stringified | Every Stream-owned primitive integer and nested integer | Canonical decimal string; no usize | JSON strings | Broaden exact rule |
| DELTA-JSON-002 | Unknown/duplicate behavior unspecified | Strict shared decoder | Unknown/duplicate/BOM/invalid UTF-8/numeric token rejected | Canonical JSON | Define |
| DELTA-AWBC-001 | ABI1/codec7 claimed by two packages | One `arcweft-core::awbc` cut | ABI2/codec8 includes direct suspension generator + sole Stream model | 2 / 8 | Reconcile |
| DELTA-AWBC-002 | Runtime type table no typed handle | `AwbcRuntimeType::StreamHandle{item,error}` | Typed affine layout | tag 21 | Add |
| DELTA-AWBC-003 | `AwbcSignature.params: Vec<TypeId>` loses parameter mode | `Vec<AwbcParameter>` | Name/type/passing/presence lossless | field order fixed | Replace |
| DELTA-AWBC-004 | Separate stream/source tables | `stream_definitions` sole table | No Source table | position 22 | Replace/delete |
| DELTA-AWBC-005 | Old StreamYield/Close/Source opcodes | OpenStream/FinishStream + NextStream/YieldStream | Old tags unknown | 0x27,0x28,0x8f,0x90 | Replace |
| DELTA-AWBC-006 | Provisional/old function kinds | Ordinary and GeneratorProducer | Old 3/4/5 unknown; producer flag required | 8,9; flag bit4 | Replace |
| DELTA-AWBC-007 | Generator safe points unspecified | StreamNext and StreamYield | Resume ownership/frame validation | 12,13 | Add |
| DELTA-BUNDLE-001 | Bundle schema5 + awbc_v1 | Bundle schema6 + awbc_v2 only | Old outer/discriminator direct reject | 6 / `awbc_v2` | Atomic replacement |
| DELTA-BUNDLE-002 | Summary has stream_plans/source_plans | `stream_definitions` count | No old counts | Schema6 JSON/AWFB | Replace |
| DELTA-SAVE-001 | Save schema1 compact/facade/source state | Save schema2 sole Stream table snapshot | One sorted vector, global affine uniqueness | schema 2 | Replace |
| DELTA-SAVE-002 | External/live save ambiguity | Typed `StreamSaveBlocker` variants | Only closed external and safe-point local states save | Save error enum | Define |
| DELTA-SAVE-003 | Restore may mutate while validating | 12-stage candidate validation + one swap | Any error leaves active executor byte-identical | Atomic save2 restore | Define |
| DELTA-SAVE-004 | Generation pins/replay/tombstones omitted | Schema2 exact fields | All included and validated | Save2 JSON | Add |
| DELTA-LANG-001 | Lang-01.1.1 provisional StreamPlan/handle/state/event/wire | Corrected Lang-01.3.1.2.1 model | Provisional shapes never land | Codec8 only | Supersede design-time only |
| DELTA-LANG-002 | Possible `stream fn`/source workaround | One ordinary `fn`, typed generator classification, external ordinary callable | No spelling/role attribute introduced | No syntax tag | Reconfirm |
| DELTA-DEL-001 | Source product modules/types/tables/events | Corrected Stream owners | All Source runtime product paths deleted after invariants ported | Old formats rejected | Delete |
| DELTA-DEL-002 | arcweft-source/debug source maps could be mistaken for Source runtime | Existing source-document/range/debug owners | Preserved; not semantic identity | Debug table only | Keep |

## Terminal payload ownership correction

| Stable delta ID | Defective/provisional shape | Corrected owner/shape | Normative invariant | Wire/version allocation | Required action |
| --- | --- | --- | --- | --- | --- |
| DELTA-TERM-001 | Tombstone reused live `StreamTerminalState` and could retain/repeat an error payload | Live `StreamTerminalReason::Error { payload: Option<_>, marker }`; payload-free `StreamTombstoneTerminal` | First live observation moves E once; drop erases it; tombstone only after payload is absent; later calls return `Closed(reason_code)` | Save2 reason code / existing NextStream | Replace; never serialize terminal payload in tombstone |

## Completeness rule

Any implementation field, variant, table, tag, or owner that differs from this ledger is a
contract change and requires a new reviewed correction. Implementation judgment may choose
private algorithms only where they do not alter observable ordering, identity, ownership,
error order, canonical bytes, limits, or dependency direction.
