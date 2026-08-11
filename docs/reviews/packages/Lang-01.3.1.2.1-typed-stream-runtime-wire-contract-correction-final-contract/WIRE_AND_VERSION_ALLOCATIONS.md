# Wire, version, allocation, and canonical-order contract

## 1. Integration base and single cut

This allocation is reconciled against `Sanzentyo/arcweft` `main` at
`23ed5d93824630d8ead9092d32f7fc70f0a8f314`. At that revision:

- `AWBC_ABI_VERSION = 1`;
- `AWBC_CODEC_VERSION = 7`;
- `ARCWEFT_BUNDLE_SCHEMA_VERSION = 5`;
- `BundleAwbcEncoding` contains only `AwbcV1` / `"awbc_v1"`;
- `BUNDLE_SESSION_SAVE_SCHEMA_VERSION = 1`;
- runtime-type tags end at 20, instruction opcodes at `0x26`, terminators at
  `0x8e`, and safe-point tags at 11.

The production cut assigns exactly these replacement versions:

| Owner | Current main | Sole corrected value | Rejected values |
| --- | ---: | ---: | --- |
| `arcweft-core::awbc::AWBC_ABI_VERSION` | 1 | **2** | 1 and every other value |
| `arcweft-core::awbc::AWBC_CODEC_VERSION` | 7 | **8** | 7 and every other value |
| `arcweft-bundle::ARCWEFT_BUNDLE_SCHEMA_VERSION` | 5 | **6** | 5 and every other value |
| `arcweft-bundle::BundleAwbcEncoding` | `awbc_v1` | **`awbc_v2`** | `awbc_v1`, aliases, unknown strings |
| `arcweft-runtime-driver::session_save` | 1 | **2** | 1 and every other value |

There is one codec-8 decoder/writer, one bundle-schema-6 decoder/writer, and one
save-schema-2 decoder/writer. No dispatcher attempts codec 7, ABI 1, bundle schema 5,
`awbc_v1`, or save schema 1.

## 2. Existing AWBC primitive encoding retained

The corrected records use the existing codec primitives without a second codec:

- enum/opcode tags: one `u8`;
- `u16`: two little-endian bytes;
- `u32` and every table ID: canonical unsigned base-128 varint, at most five bytes;
- `u64`: eight little-endian bytes;
- booleans: `0x00` or `0x01` only;
- `Option<T>`: tag `0` for `None`, tag `1` followed by `T` for `Some`;
- vectors/tables/UTF-8 byte strings: canonical `u32` varint length followed by items/bytes;
- digests/layout hashes: 32 raw bytes;
- trailing bytes, noncanonical varints, unknown tags, bounds failures, and decode-budget
  excess are errors.

## 3. Canonical RuntimePlan field order

`RuntimePlan` serializes/fingerprints its owned top-level inventories in this exact order:

1. `entries` (existing owner/order);
2. `callable_executables` (existing owner/order);
3. `flow_executables` (existing owner/order);
4. `flows` (existing owner/order);
5. `pure_helpers` (existing owner/order);
6. `trait_methods` (existing owner/order);
7. `line_task_groups` (existing owner/order);
8. `effect_sets` (new canonical owner);
9. `stream_definitions` (sole corrected table).

The first seven retain their already accepted typed producer order and validation; this
contract does not resort or duplicate them. `effect_sets` is canonicalized first, then
`stream_definitions` is sorted strictly by `RuntimeStreamDefinitionKey`, deduplicated,
and assigned `RuntimeStreamDefinitionId` by zero-based index. Every ID is bounds-checked.

## 4. Canonical AWBC program table order

Codec 8 writes the following tables immediately after the existing header, in order.
The order includes the current callable/flow executable tables and every unrelated
accepted table; only the old Stream/Source pair is replaced.

| Position | Codec-8 field | Relation to codec 7 |
| ---: | --- | --- |
| 0 | `strings` | unchanged |
| 1 | `runtime_types` | unchanged; adds tag 21 |
| 2 | `constants` | unchanged |
| 3 | `effect_sets` | same field; canonical owner strengthened |
| 4 | `signatures` | same position; parameter records corrected |
| 5 | `frame_layouts` | unchanged |
| 6 | `functions` | unchanged table; function-kind tags corrected |
| 7 | `blocks` | unchanged |
| 8 | `instructions` | unchanged table; removed/new opcodes below |
| 9 | `resume_points` | unchanged |
| 10 | `patterns` | unchanged |
| 11 | `match_arms` | unchanged |
| 12 | `intrinsics` | unchanged |
| 13 | `host_calls` | unchanged |
| 14 | `task_plans` | unchanged |
| 15 | `audio_commands` | unchanged |
| 16 | `effect_plans` | unchanged |
| 17 | `choices` | unchanged |
| 18 | `choice_options` | unchanged |
| 19 | `content_units` | unchanged |
| 20 | `line_task_groups` | unchanged |
| 21 | `line_task_nodes` | unchanged |
| 22 | `stream_definitions` | replaces codec-7 `stream_plans` |
| — | — | codec-7 `source_plans` position 23 is deleted |
| 23 | `pure_helpers` | shifts from 24 |
| 24 | `trait_methods` | shifts from 25 |
| 25 | `display_map` | shifts from 26; debug only |
| 26 | `source_map` | shifts from 27; debug only |
| 27 | `resources` | shifts from 28 |
| 28 | `callable_executables` | shifts from 29; otherwise unchanged |
| 29 | `flow_executables` | shifts from 30; otherwise unchanged |
| 30 | `entries` | shifts from 31; otherwise unchanged |

The canonical string table remains strictly increasing by UTF-8 bytes and deduplicated.
Effect set ID 0 is the empty set. Every nonempty effect row is sorted/deduplicated by the
resolved effect ID's canonical UTF-8 bytes; rows are then sorted lexicographically by
that member sequence and deduplicated. RuntimePlan row *n* projects to AWBC row *n* after
string-ID remapping. Tampered duplicate/noncanonical rows or out-of-bounds IDs fail
verification.

All unrelated indexed tables retain current accepted lowerer order and current structural
verification. The existing executable-identity transcript includes their canonical bytes,
including `callable_executables`, `flow_executables`, and `entries`; only existing debug
`display_map` and `source_map` evidence remains excluded. Stream fingerprints therefore
cannot accidentally omit unrelated executable fields.

## 5. Top-level numeric allocations

| Family | Existing maximum/current relevant tags | Codec-8 allocation | Rule |
| --- | --- | --- | --- |
| Runtime type | 20 | `StreamHandle = 21` (`0x15`) | fields `item`, `error` |
| Instruction | `ApplyFunction = 0x26` | `OpenStream = 0x27`; `FinishStream = 0x28` | first next unused opcodes |
| Terminator | `Unreachable = 0x8e` | `NextStream = 0x8f`; `YieldStream = 0x90` | first next unused terminators |
| Safe point | `None = 11` | `StreamNext = 12`; `StreamYield = 13` | first next unused tags |
| Function kind | existing 0..7 | `Ordinary = 8`; `GeneratorProducer = 9` | tags 3,4,5 removed/unknown |
| Function flag | bits 0..3 used | `OWNS_STREAM_PRODUCER = 1 << 4` | required on generator producer |

Codec 8 treats old instruction tags `0x1c` (`StreamYield`), `0x1d` (`StreamClose`),
`0x1e` (`SourceClose`), and `0x20` (`SourceYield`) as unknown. Existing `Drop = 0x1f`
and unrelated tags remain. Old function-kind tags 3 (`StreamTransform`), 4
(`SourceOpen`), and 5 (`SourceHandler`) are unknown. There are no deprecated variants.

## 6. New nested binary enum tags

These tags are local to their exact owning enum and are not interchangeable.
Semantic privacy/replay ordering uses explicit rank functions from `POLICY_PROFILE.md`,
not declaration or wire-tag order.

| Enum | Tag assignments |
| --- | --- |
| `AwbcParameterPassing` | 0 PositionalOnly; 1 PositionalOrNamed; 2 NamedOnly; 3 RestPositional; 4 RestNamed |
| `AwbcParameterPresence` | 0 Required; 1 Optional; 2 Defaulted |
| `AwbcResolvedArgument` | 0 Value(register); 1 Omitted |
| `AwbcStreamOrigin` | 0 External; 1 AuthoredGenerator; 2 Derived |
| `AwbcStreamBackpressure` | 0 LatestOnly; 1 Bounded; 2 ProviderBlocking |
| `AwbcStreamOverflow` | 0 DropOldest; 1 DropNewest; 2 TerminalError; 3 Coalesce |
| `AwbcStreamReplayMode` | 0 Full; 1 HashOnly; 2 Summary; 3 EventOnly; 4 None |
| `AwbcStreamPrivacy` | 0 Transient; 1 Redacted; 2 Recordable; 3 Private |
| `AwbcStreamPermissionRule` | 0 AtOpen; 1 OnRestart; 2 EachEvent |
| `AwbcStreamConsumerDropPolicy` | 0 DiscardQueued; 1 DrainAndRetainReplay |
| `AwbcStreamReplayRetention` | 0 UntilConsumerDrop; 1 ThroughTombstone |
| `AwbcStreamReplayDataClass` | 0 EventOnly; 1 Summary; 2 Digest; 3 Payload |
| `AwbcStreamRestartRule` | 0 Deny; 1 SameProvider |
| `AwbcStreamProviderReplacementRule` | 0 Deny; 1 SameOrigin |
| `AwbcStreamProducerOutcome` | 0 Complete; 1 Fail(register); 2 Cancelled |
| `StreamReplayRecordBody` | 0 Payload; 1 Digest; 2 Summary; 3 EventOnly |
| `StreamReplayHashDomain` | 0 Item; 1 RecoverableError; 2 Progress; 3 TerminalError |
| `StreamReplayNoRecordReason` | 0 PolicyNone; 1 Transient; 2 Private; 3 LimitZero; 4 RecordTooLarge; 5 TerminalCap |
| `StreamReplayEventKind` | 0 Opened; 1 Progress; 2 Item; 3 RecoverableError; 4 End; 5 TerminalError; 6 Disconnected; 7 PermissionRevoked; 8 CloseAcknowledged; 9 Restarted; 10 ProviderReplaced; 11 RuntimeLimit; 12 Cancelled |
| `StreamRuntimeLimit` | 0 EventSequence; 1 CommitSequence; 2 LifetimeEvents; 3 LifetimeItems; 4 RecoverableErrors; 5 ProgressEvents; 6 ReplayRecordId; 7 ReplayStatistics; 8 QueueBytes; 9 DeliveryInvariant |
| `StreamTerminalReasonCode` | 0 End; 1 Error; 2 RuntimeLimit(+limit tag); 3 Cancelled; 4 Disconnected; 5 PermissionRevoked; 6 ProviderReplaced |

Unknown nested tags are errors. No `Other`, integer passthrough, or catch-all variant is
present.

## 7. Exact new AWBC field encoding

All fields are written in declaration order with the primitives in section 2.

- `AwbcParameter`: `name: Option<AwbcStringId>`, `ty: AwbcTypeId`, passing tag,
  presence tag.
- `AwbcSignature`: vector of `AwbcParameter`, optional result type, effect-set ID.
- `AwbcRuntimeType::StreamHandle`: tag 21, item type ID, error type ID.
- `AwbcStreamDefinition`: public ID, 32-byte semantic key, item type, error type,
  signature ID, effect-set ID, origin, policy, handle-layout digest, state-layout digest,
  optional producer-code digest, optional generator-frame digest.
- `External` origin: tag 0, module string ID, module-ABI digest, capability string ID,
  operation string ID.
- `AuthoredGenerator`/`Derived`: tag 1/2, producer function ID.
- `OpenStream`: opcode `0x27`, destination register, definition ID, vector of
  `AwbcResolvedArgument`.
- `FinishStream`: opcode `0x28`, stream register, producer outcome.
- `NextStream`: opcode `0x8f`, stream register, destination register, resume-point ID,
  ready block ID.
- `YieldStream`: opcode `0x90`, stream register, value register, resume-point ID,
  continuation block ID.

`Move` and `Drop` retain their existing opcodes and implement affine handle transfer and
drop. `NextStream` and `YieldStream` require matching safe-point tags 12 and 13 and a
resume-point whose function/frame/block ownership verifies. `OpenStream` argument count
and each Value/Omitted shape must match the referenced corrected signature.

Worked record bytes are in `WORKED_EXAMPLES.md`.

## 8. Exact Stream definition-key transcript

`RuntimeStreamDefinitionKey` is BLAKE3-256 of this byte transcript:

1. raw domain bytes `arcweft.stream.definition.v1` followed by NUL;
2. origin tag;
3. origin semantic fields:
   - External: length-prefixed module ID, raw 32-byte module ABI hash,
     length-prefixed capability ID, length-prefixed operation ID;
   - AuthoredGenerator: length-prefixed callable ID and callable contract hash;
   - Derived: length-prefixed callable ID, callable contract hash, vector of ordered input
     definition keys;
4. length-prefixed accepted callable ID;
5. raw 32-byte callable contract hash;
6. raw 32-byte item layout hash;
7. raw 32-byte error layout hash;
8. parameter count and each parameter in declaration order: canonical `u32` index,
   optional name, raw 32-byte type-layout hash, passing tag, presence tag;
9. effect member count and each canonical effect ID as a length-prefixed UTF-8 string.

Strings and counts use the AWBC primitives in section 2. External origin fields are not
repeated in step 3 when the accepted callable identity already names them; the explicit
fields still remain in the transcript to bind provider ABI independent of display naming.
The transcript excludes source files/spans/maps, display labels, table indices,
generation, resolved policy capacities, and code bytes.

Worked transcript SHA-equivalent identity:

- transcript byte count: `394`;
- BLAKE3-256: `06c9bbdb24b2d49e046351ed1abc232b56a36f2fcbb7001cbe5c88342f7725dd`.

## 9. Replay digest, summary, and byte accounting

HashOnly body digest uses BLAKE3-256 over:

1. domain `arcweft.stream.replay.body.v1` plus NUL;
2. definition key;
3. generation, ordinal, record ID, and commit sequence as little-endian `u64`;
4. ingress option tag plus sequence when present;
5. replay-event tag;
6. replay-hash-domain tag;
7. type-layout digest;
8. canonical payload byte length as little-endian `u64`;
9. existing canonical `RuntimePayload` bytes.

The payload bytes are never retained by a Digest record. A Summary record stores only
`type_layout`, canonical byte count, and the closed `StreamReplayShape`; it stores neither
payload nor digest. `accounted_bytes` is the exact encoded record-body byte count plus the
fixed record envelope and collection-length contribution; implementation tests use the
actual codec's length function rather than `size_of`.

Worked transcript:

- byte count: `155`;
- BLAKE3-256: `947ea4e1562aaadb16a5541cb6f9d229a9ebbe451e9aa40dda0564e30f9a14eb`.

## 10. Bundle Stream fingerprint transcript

The Stream compatibility fingerprint is BLAKE3-256 over:

1. domain `arcweft.bundle.stream-fingerprint.v1` plus NUL;
2. existing complete AWBC executable-identity digest (which includes unrelated executable
   tables and excludes only accepted debug maps);
3. Stream-definition count;
4. for each definition in key order: definition key, handle-layout digest, state-layout
   digest, optional producer-code digest, optional generator-frame digest;
5. resolved target/project profile digest;
6. adapter-requirements digest.

Provider ABI, callable signature, effects, and item/error layout are already bound by the
definition key and definition record. Source/debug maps never enter the transcript.
Worked byte count is `232`; fingerprint is `7ee9a40fbd4025897267158027764cca41dba79dce5b56d14fec7283764600b3`.

## 11. Shared host/save JSON representation

The shared core codec, used byte-for-byte by native, web, and Agent adapters, applies:

- every Stream-owned integer: JSON string matching exactly `0|[1-9][0-9]*`, with the
  declared `u32`/`u64` upper bound;
- every Stream digest field: exactly 64 lowercase hex characters;
- data-bearing enums: internal object tag field `"type"` first, lower snake case;
- unit enums: lower-snake-case JSON string;
- `StreamInstanceSnapshotEntry`: adjacent tag fields `"kind"` then `"entry"`;
- optional fields: explicit `null` in canonical output;
- declaration-order object fields and already-normative array ordering;
- compact UTF-8, no BOM, no duplicate or unknown field, no trailing bytes.

`StreamBackpressure` JSON is `{"type":"latest_only"}` or an internally tagged
`bounded`/`provider_blocking` object. `StreamOverflow::Coalesce` is
`{"type":"coalesce","reducer":...}`; other overflow values are internally tagged unit
objects. All other simple policy enums are lower-snake-case strings. Existing
`RuntimePayload` JSON remains its existing serde shape; the Stream codec does not invent a
payload DTO.

## 12. Tamper and old-format rejection order

AWBC decoding checks magic, codec version, trailing bytes/canonical primitive form, then
constructs a candidate and verifies ABI, budgets, canonical tables, all bounds/types,
Stream signature/origin/policy invariants, CFG/safe points, affine ownership constraints,
and entry contracts. Any failure discards the candidate.

Bundle decoding rejects outer schema/version and executable discriminator before invoking
AWBC. Save decoding rejects schema 1 before payload deserialization/migration dispatch.
The following never enter fallback logic: codec 7, ABI 1, bundle schema 5, `awbc_v1`,
save schema 1, Source table position/tag, old Stream plan position/tag, old function-kind
3/4/5, old Stream/Source opcodes, and provisional Lang-01.1.1 shapes.

## 13. Immediate pre-implementation rebase rule

Immediately before the version-changing merge group, the implementing owner must inspect
current `main` constants, opcode/tag functions, table write/read order, and verifier
budgets. If and only if one of the *new* values 21, `0x27`, `0x28`, `0x8f`, `0x90`, 12,
13, 8, or 9 has been consumed, move that colliding new value to the next unused value in
that family and update the contract package/test vectors in the same review. Existing
accepted tags are never renumbered. No alias is retained for the superseded proposal.
