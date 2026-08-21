# Lang-01.5.1.1.2.1.1.1.1 — checked Match coverage, runtime Need identity, and AWBC allocation correction

## Sequence, inputs, and precedence

This is a narrow mandatory redelivery correction to the returned
Lang-01.5.1.1.2.1.1.1 generic-Match and typed-Need producer ABI package. It
does not replace the primary unary-Need request or reopen the accepted selector
result, typed carrier, resource-registry, journal, observer, start,
cancellation, persistence, or strict version-1 decisions that remain
constructible.

Required retained inputs are:

- the primary
  [Lang-01.5.1.1.2.1 request](2026-08-21-lang-01.5.1.1.2.1-reactive-unary-need-match-reconciliation.md);
- the
  [Lang-01.5.1.1.2.1.1 design-validation correction](2026-08-21-lang-01.5.1.1.2.1.1-reactive-unary-need-match-design-validation-correction.md);
- the
  [Lang-01.5.1.1.2.1.1.1 ABI correction](2026-08-21-lang-01.5.1.1.2.1.1.1-generic-match-and-typed-need-producer-abi-correction.md);
- the retained returned archive
  [`arcweft-lang-01.5.1.1.2.1.1.1-generic-match-and-typed-need-producer-abi-correction-final-contract.zip`](../packages/zips/arcweft-lang-01.5.1.1.2.1.1.1-generic-match-and-typed-need-producer-abi-correction-final-contract.zip),
  SHA-256
  `96F4F84BE1B7B2BBEC9D2BA564418F00F453870F4EA331566A1F51258CC1EF8D`;
- its searchable
  [frozen mirror](../packages/arcweft-lang-01.5.1.1.2.1.1.1-generic-match-and-typed-need-producer-abi-correction-final-contract/README.md);
- the accepted
  [line-plan runtime-handle contract](../packages/arcweft-aw-ah-009.4.4.1-line-plan-runtime-handle-result-authority-reconciliation-final-contract/README.md);
- maintained
  [AWBC runtime](../../02-runtime/executable-runtime-core.md),
  [Need timeout](../../02-runtime/need-timeout.md), and
  [pattern runtime](../../02-runtime/control-flow-runtime.md) contracts; and
- current production plus every task/Need/Match consumer named below.

Inspected production baseline:
`4bda1cdcea63fdf7aac32691d756c1c0e1fc693e`, initially clean and equal to
`origin/main` before package intake.

Current production, maintained stable documentation, and later accepted
contracts take precedence over the returned package's stale or incomplete
claims. Every Arcweft-owned version marker remains exactly `1`.

## Package disposition and split reason

The archive is byte-valid and internally validates, but it is not ready for
implementation after repository reconciliation. Four result-changing gaps
must be closed together because they affect the same checked-Match and runtime
Need authority switch.

### 1. Opcode `0x1e` has three incompatible owners

The return calls `0x1e` unused and assigns it to `MakeNeedHandle`. Maintained
`docs/02-runtime/executable-runtime-core.md` already assigns `0x1e` to
`NeedTimeout`. The accepted line-plan package assigns `0x1e` to
`ExecuteLineOperation` and `0x20` to `CommitDialogueResult`.

All three features are pending convergence work under the same unreleased
version-1 AWBC grammar. Production currently leaves the bytes open, but an
empty implementation slot does not erase accepted allocations. Implementers
cannot select a winner or silently move an operation.

The returned `NEED_PRODUCER = 1 << 4` also conflicts with the accepted stream
producer allocation `OWNS_STREAM_PRODUCER = 1 << 4`. Its wire grammar further
states that all `u32` IDs and lengths are fixed little-endian even though the
current canonical AWBC wire owner encodes ordinary `u32` values and vector
lengths as canonical unsigned base-128 varints. Flags and integer grammar need
the same global reconciliation as opcodes.

### 2. Deleting `AwbcTaskPlan.need_id` removes non-View task identity

The return defines a producer-contract/argument-derived `NeedId` only for a
flagged View unary-Need producer with `many == None`, then globally deletes
`AwbcTaskPlan.need_id` and changes `NeedId` from String to fixed bytes.

Current production also consumes the task-plan Need identity in:

- ordinary task start and lifecycle publication;
- direct unary Await and task suspension;
- AwaitMany child launch, source-order indexed child Need identities, in-flight
  state, snapshot, restore, and completion correlation;
- structured runtime suspension and indexed Need derivation;
- runtime-plan task interning and task-plan canonical identity;
- AWBC codec, verifier, bundle mapping, and product-step tests.

The returned package neither preserves that field nor defines a replacement
identity transcript for these non-View paths. Deleting it would leave current
task events and AwaitMany snapshots without their correlation authority.

### 3. `CheckedMatchCoverage` has no generating authority

The return requires one authoritative coverage row, but its normative
`CheckedMatch::try_from_hir` accepts `coverage` from its caller. No current
checked coverage owner exists, and the package defines no exhaustive algorithm
for the full pattern language. Its test matrix contains Match fact
completeness tests but no exhaustive/non-exhaustive/unreachable coverage
matrix.

This permits a caller-fabricated `exhaustive` bit and unreachable-arm list.
The implementation cannot infer the final behavior for guards, infinite
primitive domains, nested products, sequence rests, records, Or/whole/typed
patterns, anonymous choices, or entity patterns.

### 4. Ownership and checked-Match digest inputs remain incomplete

The package names `RegisteredSemanticWorld::checked_ownership(&TypeKind)` but
does not provide a total mapping for every current `TypeKind`, registered
opaque value class/persistence row, borrow kind, callable/frame-local value,
or producer-dependent Need handle. A `Need<T>` type alone does not contain the
producer argument types whose snapshot admission the returned contract also
requires.

The checked-Match digest transcript likewise says HIR IDs have “canonical
generation-bound bytes” without selecting the exact encoding or its rebuild
and replacement stability. More importantly, the accepted final-HIR View
parent makes all HIR IDs session-only and explicitly excludes them from
persistent identity seeds and product bytes. No current general
`ExprId`/`ScopeId`/`PatternId`/`LocalId` digest encoder owns the returned
representation.

These are semantic admission and persisted content-root choices, not private
implementation details.

## Accepted decisions that remain fixed

Do not redesign these returned decisions without a concrete current-source
defect:

- a View selector returns one owning synthetic nominal Variant whose case is
  the source arm and whose payload is a source-ordered binding Tuple;
- zero bindings use an empty Tuple payload;
- no callee register escapes and `arcweft-view` remains core-independent;
- View owns lightweight site/arm/output/local/body coordinates; bundle owns
  the static View/AWBC join; runtime-driver owns private decode/install;
- selector guards use explicit pattern/bind/ordinary-guard/Branch control flow,
  not the currently ignored `AwbcMatchArm.guard`;
- semantic types remain in existing `CheckedExpression`, `CheckedPattern`, and
  `CheckedBinding` `TypeKind` owners; `CheckedMatch` does not copy them;
- Match arm identity is owner expression plus source ordinal and exact current
  HIR scope/pattern/guard/value/locals;
- `CheckedViewCatalog` retains only a checked-Match reference;
- typed Need uses a dedicated payload-typed AWBC type and dedicated
  `RuntimeValue::NeedHandle`, never String or Dynamic;
- generation binding occurs in runtime-driver, not the core value;
- `ResourceTypeRegistry` and its existing digest remain the sole resource
  authority; and
- the final switch is strict version 1 with no compatibility reader, alias,
  dual carrier, fallback resolver, or source reconstruction.

The opcode, function-kind, function-flag, and integer-wire decisions below are
preselected by user direction after an independent Sol-max current-repository
audit. The returned correction MUST adopt them exactly. Global Need/task
identity, coverage construction, ownership classification, and checked-Match
product identity remain the decisions this correction must close.

## Required exact decisions

### A. One collision-free AWBC version-1 allocation

1. Adopt this exact final opcode table. Current implemented rows do not move.
   Pending rows occupy one dense tail while the accepted maintained
   `NeedTimeout`, `CommitDialogueResult`, and `CopyValue` coordinates remain
   fixed.

   | Byte | Final opcode |
   |---:|---|
   | `0x00` | `Nop` |
   | `0x01` | `LoadConst` |
   | `0x02` | `Move` |
   | `0x03` | `Clear` |
   | `0x04` | `EnterScope` |
   | `0x05` | `ExitScope` |
   | `0x06` | `BindPattern` |
   | `0x07` | `TestPattern` |
   | `0x08` | `MakeTuple` |
   | `0x09` | `MakeSequence` |
   | `0x0a` | `RepeatSequence` |
   | `0x0b` | `SequenceLen` |
   | `0x0c` | `SequenceGet` |
   | `0x0d` | `SequenceSlice` |
   | `0x0e` | `SequencePush` |
   | `0x0f` | `MakeRecord` |
   | `0x10` | `MakeVariant` |
   | `0x11` | `ProjectTuple` |
   | `0x12` | `ProjectRecord` |
   | `0x13` | `ProjectField` |
   | `0x14` | `Unary` |
   | `0x15` | `Binary` |
   | `0x16` | `CallPureHelper` |
   | `0x17` | `CallIntrinsic` |
   | `0x18` | `EnsureContent` |
   | `0x19` | `EmitEffect` |
   | `0x1a` | `StartTask` |
   | `0x1b` | `SpawnFiber` |
   | `0x1c` | `StreamYield` |
   | `0x1d` | `StreamClose` |
   | `0x1e` | `NeedTimeout` |
   | `0x1f` | `Drop` |
   | `0x20` | `CommitDialogueResult` |
   | `0x21` | `AssignRecordField` |
   | `0x22` | `CallTraitMethod` |
   | `0x23` | `RegisterCleanup` |
   | `0x24` | `CancelCleanup` |
   | `0x25` | `MakeFunction` |
   | `0x26` | `ApplyFunction` |
   | `0x27` | `MakeAgent` |
   | `0x28` | `MakeReductionUnchanged` |
   | `0x29` | `MakeNeedHandle` |
   | `0x2a` | `CopyValue` |
   | `0x2b` | `ExecuteLineOperation` |
   | `0x2c` | `OpenStream` |
   | `0x2d` | `FinishStream` |
   | `0x2e` | `ApplyExternalStreamGroup` |
   | `0x2f..=0x7f` | unknown/reserved; reject |
   | `0x80` | `Jump` |
   | `0x81` | `Branch` |
   | `0x82` | `Match` |
   | `0x83` | `CallFunction` |
   | `0x84` | `GotoStatic` |
   | `0x85` | `GotoDynamic` |
   | `0x86` | `Dialogue` |
   | `0x87` | `Choice` |
   | `0x88` | `Await` |
   | `0x89` | `AwaitMany` |
   | `0x8a` | `HostCall` |
   | `0x8b` | `Return` |
   | `0x8c` | `Trap` |
   | `0x8d` | `BudgetYield` |
   | `0x8e` | `Unreachable` |
   | `0x8f` | `NextStream` |
   | `0x90` | `YieldStream` |
   | `0x91..=0xff` | unknown/reserved; reject |

   This supersedes `MakeNeedHandle=0x1e`,
   `ExecuteLineOperation=0x1e`, and Stream
   `OpenStream/FinishStream/ApplyExternalStreamGroup=0x27/0x28/0x29`.
   It retains `NeedTimeout=0x1e`, `CommitDialogueResult=0x20`,
   `CopyValue=0x2a`, current `StreamYield/StreamClose=0x1c/0x1d`, and every
   current production row.

2. Make one closed enum the numeric authority. The final shape is a
   `#[repr(u8)] AwbcOpcode` with each numeric literal appearing only as its
   discriminant. `encoded()` is `self as u8`. `from_encoded()` indexes an
   allocation-free `[Option<AwbcOpcode>; 256]` built at compile time from
   `AwbcOpcode::ALL`; it MUST NOT use a second feature-local numeric table.
   `AwbcInstruction::opcode()` and `AwbcTerminator::opcode()` map semantic
   variants to enum variants only and contain no numeric literals.

   Implement manual numeric `Serialize`/`Deserialize` directly on the enum:
   `serialize_u8(encoded())`, then `u8` to `from_encoded()` with unknown bytes
   rejected. Private AWBC `Wire` uses those same inherent methods directly.
   No raw opcode DTO, String tag, copied match table, `unsafe`, transmute,
   compatibility reader, or allocation is permitted for opcode encode/decode.
   Serde is a structured-data boundary; canonical executable identity remains
   the private AWBC wire codec.

3. Adopt these exact function-kind tags and typed flags.

   | Tag | `AwbcFunctionKind` |
   |---:|---|
   | `0` | `Flow` |
   | `1` | `PureHelper` |
   | `2` | `TraitMethod` |
   | `3` | `StreamTransform` |
   | `4`, `5` | removed tombstones; reject |
   | `6` | `LineTask` |
   | `7` | `Synthetic` |
   | `8` | `Ordinary` |
   | `9` | `GeneratorProducer` |
   | `10` | `LineActivation` |
   | `11..=255` | unknown; reject |

   `LineActivation=10` supersedes the conflicting returned tag 6. Use the
   same `#[repr(u8)]` discriminant-only numeric authority, compile-time decode
   table, direct numeric Serde, and private Wire behavior as `AwbcOpcode`.
   Removed tags 4/5 are never reused.

   | Bit | `AwbcFunctionFlag` |
   |---:|---|
   | `0` | `MaySuspend` |
   | `1` | `MayAllocate` |
   | `2` | `Deterministic` |
   | `3` | `HasDynamicTarget` |
   | `4` | `OwnsStreamProducer` |
   | `5` | `NeedProducer` |
   | `6..=31` | unknown; reject |

   Add `#[repr(u8)] AwbcFunctionFlag` as the one bit-position owner and a
   `#[repr(transparent)] AwbcFunctionFlags(u32)` with a private field.
   Expose only `empty`, `with`, `contains(AwbcFunctionFlag)`, `bits`, and
   `try_from_bits`. Derive each mask as `1_u32 << flag as u8`; do not copy mask
   literals into callers. `KNOWN_MASK` is exactly `0x3f`. Deserialize and
   private Wire decode through `try_from_bits` and reject unknown bits. The
   flag enum uses direct numeric-u8 Serde/Wire; the set serializes directly as
   one numeric `u32` and private Wire emits that `u32` as the canonical varint,
   never as a temporary Vec/list of flag names.

   `GeneratorProducer` requires bit 4 and forbids bit 5. A Need producer is
   `Synthetic`, requires bit 5 plus `Deterministic|MayAllocate`, and forbids
   bit 4 and `MaySuspend`. `LineActivation`, `LineTask`, `Ordinary`, and
   `StreamTransform` forbid both producer bits. Bit 4+5 together always fails
   verification. A non-producer selector may remain `Synthetic` without bit 5.

4. Keep every schema/codec/ABI marker at `1` and adopt one canonical integer
   grammar. Every ordinary `u32`—ID, register, table index, site, ordinal,
   revision, source offset, count, length, tensor dimension, and Char scalar—is
   the current shortest unsigned base-128 varint, at most five bytes.
   Reject overflow, a sixth byte, unterminated input, and redundant encodings
   such as `80 00`. There is no fixed-little-endian `u32` integer exception.

   Fixed-width wire values are limited to: `u8` tags; envelope version and
   reserved `u16-le`; existing stream group/parameter and audio-channel
   `u16-le` coordinates; envelope length and existing duration/frame/feature/
   budget `u64-le`; priority `i32-le`; 16/32-byte digests or integer bit
   patterns; 4-byte LE F32 raw bits/TensorF32 elements; and 8-byte LE F64 raw
   bits/TensorF64 elements. `usize` never enters wire; collection lengths are
   checked `u32` varints.

   Repair the current tensor-shape asymmetry in the same primitive cut:
   the writer currently emits shape items through fixed-LE
   `write_u32_slice`, while the reader consumes `Vec<u32>` varints. Both sides
   SHALL use the one `u32::Wire` varint authority.

   “No copy” means canonical encoding writes envelope and payload directly
   into one final `Vec<u8>`, patches the payload length in place, and removes
   the current payload-buffer-to-final-buffer copy. Decoding reads the input
   `&[u8]` through `Reader<'_>` directly into final `AwbcProgram` values.
   Opcode, kind, flag, ID, and scalar decoding is allocation-free; Vec/String
   allocate only their final owned storage, with no raw DTO or intermediate
   opcode records. Serde implementation buffering is outside this canonical
   zero-intermediate-copy guarantee.

   The correction MUST give exact instruction grammars using these primitives,
   including:

   ```text
   29 dst:varu32 plan:varu32 site:varu32 argc:varu32 args[argc]:varu32
   1e dst:varu32 source:varu32 limit:varu32 producer_site:varu32
   2b dst:varu32 operation:varu32 argc:varu32 args[argc]:varu32
   ```

   Required tests include all 256 opcode bytes; enum/Serde/Wire round trips;
   wrong-class and `2f/7f/91/ff` rejection; exact golden bytes for each new
   opcode; varint `0/1/127/128/u32::MAX` plus noncanonical/overflow/truncation;
   tensor shape `[1, 128]`; flag uniqueness/unknown bits/kind constraints;
   single-buffer envelope length and failure rollback; canonical
   decode/re-encode property/fuzz tests; VM/structured/AOT parity; version 1
   fixation; and structural absence of duplicate numeric maps/readers.

### B. One runtime Need/task identity model for every consumer

5. Select the final `NeedId` representation and canonical derivation domains
   for all producer families, not only View unary-Need producers. Cover:

   - verified View producer function + arguments;
   - ordinary `StartTask`/HostTask/task plan publication;
   - direct Await over an already-produced Need handle;
   - AwaitMany base and source-index child identities;
   - structured runtime Await/AwaitMany parity;
   - runtime Timeout derived Need output/source identities; and
   - any line-task producer that shares the task/publication substrate.

6. Decide whether `AwbcTaskPlan.need_id` remains, is narrowed to a typed
   producer identity row, or is deleted only after a complete replacement.
   Give the exact final `AwbcTaskPlan`, runtime-plan task seed/spec, verifier,
   codec, bundle, product-step, and snapshot schemas. No optional string
   fallback or parallel legacy field is allowed.
7. If `NeedId` becomes fixed bytes, define domain-separated transcripts for
   ordinary plans and AwaitMany indexed children. Replace string concatenation
   and parsing in both structured and AWBC paths, including exact index width,
   order, collision tests, display-only formatting, malformed restore, and
   task-event correlation.
8. Define how `TaskId`, `TaskKey`, `NeedId`, producer contract, task plan,
   arguments, `JoinSameKey`, and active generation relate for every family.
   Equal/different inputs, duplicate observers, fanout children, replay, and
   hot replacement must have one deterministic result.
9. Give a deletion matrix for every current `need_id`, `task_need_id`, String
   conversion, indexed suffix, in-flight String snapshot, codec, bundle,
   fixture, and generated consumer. A row may be deleted only in the same cut
   that its final typed replacement is executable.

### C. One inherent checked Match coverage authority

10. Define the sole owner and exact constructor for `CheckedMatchCoverage`.
    `CheckedMatch::try_from_hir` must compute or borrow a non-forgeable result
    from that owner; it must not accept caller-supplied `exhaustive` or
    unreachable-arm data.
11. Give the exact coverage/reachability algorithm and bounded-work model for
    every current pattern family:

    - discard, binding, mutable binding, whole binding, and typed binding;
    - literals and entity references;
    - closed project/builtin variants, Result, Option, and anonymous Choice;
    - tuples and nested product patterns;
    - records with exact, ignore-rest, and whole-record binding;
    - Vec/Array/Slice sequence patterns with exact and tail rest;
    - Or patterns and nested combinations; and
    - poisoned, open, opaque, unsupported, or future-non-exhaustive domains.

12. Select guard semantics for coverage and reachability. State whether a
    guarded otherwise-covering arm contributes to exhaustiveness, when a later
    arm is unreachable, and how false/unknown guards affect diagnostics.
13. Select publication behavior for non-exhaustive and unreachable arms:
    hard error, diagnostic, retained evidence, or a precise combination. Give
    exact source roles, diagnostic precedence, sorted/unique unreachable rows,
    work limits, and no-partial-publication behavior.
14. Provide positive, negative, nested, exact-limit/one-over, and property or
    differential tests for coverage. The matrix must include at least guarded
    wildcard, duplicate variant/literal, missing Result/Option case, Or overlap,
    nested tuple/variant, record rest, sequence rest, infinite primitive with
    and without wildcard, and reordered/forged coverage rejection.

### D. Total ownership and persistence admission

15. Define one exhaustive table from every current `TypeKind` family and its
    legitimate registered/project context to `Copy`, `SnapshotClone`, or one
    closed rejection reason. Name the existing opaque value-class/persistence,
    runtime-handle, nominal field, borrow, callable, agent, dialogue, resource,
    and composite owners used by each row.
16. Give the exact API inputs. If `RegisteredSemanticWorld` alone cannot see
    project nominal fields, resource registry facts, or checked producer
    arguments, move the inherent behavior to a legitimate context that can;
    do not add copied side tables or source/string lookup.
17. Reconcile type-level `Need<T>` disposition with value/producer-dependent
    constructor arguments. Select whether generic Match records a type-class
    disposition while checked View producer admission separately certifies
    captures/arguments, or another one-authority model. No runtime guess may
    fill a missing semantic certification.
18. Define recursive limits, cycle behavior, missing opaque/nominal evidence,
    affine line handles, non-snapshot opaque values, closures/functions,
    references, nested Result/Option/tuple/sequence/record/variant, and exact
    error/source precedence.

### E. Canonical checked-Match identity and validator closure

19. Preserve the accepted parent split: `ExprId`, `ScopeId`, `PatternId`,
    `LocalId`, snapshots, and other HIR arena coordinates are session-only
    lookup facts and never enter product wire, content-root, save, replay, or
    stable replacement identity. Define the exact one-way projection from the
    checked session fact to product-stable program/site/arm/output coordinates.
20. Define the exact product checked-Match semantic digest encoder for every
    stable value it commits to. Specify byte order/width, canonical type/effect
    digests, arm/output order, ownership, coverage, resource digest, and the
    accepted View program/revision scope. State deterministic recompilation,
    bundle content-root, save/replay, and hot-replacement behavior. Do not use
    HIR IDs, arena ordinals, debug formatting, source spelling,
    platform-sized integers, or an unspecified “canonical bytes” helper.
21. Replace the package's stale input copy with the exact repository request
    chain or explicitly include both predecessor bytes and this correction.
    The validator must compare their expected SHA-256 values, not merely test
    that an `INPUT_REQUEST.md` filename exists.
22. Refresh source evidence with exact current line ranges for every affected
    production and maintained-document consumer. “Located by search” and
    line labels such as `1-end` do not satisfy a result-changing claim.
23. Use constructible current owner names and APIs. Reconcile proposed rows
    with the existing `RuntimePlanSemanticFactInput` and functional AWBC VM
    entry points; do not normatively require nonexistent
    `RuntimeSemanticFactInput` or `AwbcVm` owners without defining their one
    final construction, visibility, migration, and deletion path.

## Required implementation sequence

Return a deletion-driven compile-clean sequence that preserves the accepted
five-cut intent but places newly required owners before their consumers. At a
minimum it must show:

1. checked coverage/ownership/digest authority and generic `CheckedMatch`;
2. resource input, complete checked View catalog, and lightweight View rows;
3. global opcode allocation plus complete runtime Need/task identity substrate
   across ordinary task, Await, AwaitMany, timeout, line-plan, and View paths;
4. typed selector/producer core and runtime-plan projection;
5. staged bundle/runtime journal/save/replay/replacement consumers; and
6. one final atomic publication switch and deletion of old View Await,
   payloadless/string NeedHandle, and every superseded task-identity route.

If these need separate reviewable commits, each commit must compile with only
final owners and no empty catalog, dummy row, compatibility branch, duplicate
identity, or conflicting opcode meaning.

The selected allocation itself lands in dependency order rather than as dummy
enum variants:

1. first migrate the currently executable enum variants to repr/direct
   Serde/Wire, typed flag-set APIs, the single-buffer encoder, the varint
   authority, and the tensor-shape repair while preserving every current
   meaning; pending enum variants are still absent at this stage;
2. publish `CopyValue=0x2a` only with its ownership verifier/VM/AOT cut;
3. publish `MakeNeedHandle=0x29` and Need flag bit 5 only with the typed Need
   carrier/producer cut;
4. publish `NeedTimeout=0x1e` only after the canonical Need identity substrate;
5. publish `CommitDialogueResult=0x20`, `ExecuteLineOperation=0x2b`, and
   `LineActivation=10` only with the complete line-plan table/verifier/VM/AOT
   cut; and
6. publish Stream `0x2c..=0x2e`, terminators `0x8f..=0x90`, kinds 8/9, and
   flag bit 4 only in its complete protected Stream cut.

Each step deletes the superseded feature-local numeric table, reader, golden,
and generated row in the same cut. An enum variant MUST NOT land earlier with
unsupported/dummy execution.

## Required artifacts and validation

Return one independently usable archive containing at minimum:

- `README.md`, exact reading order, full inspected Git SHA, and final status;
- `OPEN_QUESTIONS.md` containing exactly `none`;
- final contract, decision register, exact Rust schemas, owner/API map, and
  dependency graph;
- a single global AWBC opcode/wire allocation table;
- a complete producer/task/Need identity table and consumer/deletion matrix;
- the checked coverage algorithm, ownership table, and digest grammar;
- persistence/replay/replacement and failure-precedence contracts;
- compile-clean implementation sequence;
- current source/maintained-document evidence with exact line ranges;
- requirement traceability and full positive/negative/tamper/property/
  differential/exact-limit/one-over/rollback/structural/Tier-2 test matrix;
- structural absence rules; and
- machine/human validation plus an internal SHA-256 manifest covering every
  payload and exact request-copy hashes.

Validation must fail for any opcode/flag collision, wrong fixed-width/varint
grammar, uncovered current task identity consumer, caller-supplied coverage,
missing pattern family, partial ownership table, persisted HIR identity,
nonconstructible owner/API, stale request copy, vague evidence range,
unresolved alternative, version marker other than `1`, or manifest mismatch.

## Constraints and non-goals

- This is design-only. Do not edit production code, tests, fixtures,
  manifests, branches, patches, PRs, or implementation overlays.
- Do not redesign accepted selector Variant/Tuple results, View/core
  independence, explicit guard Branch lowering, dedicated typed Need value,
  resource registry ownership, parent unary-Need lifecycle, or strict
  version-1 migration without a concrete current-source flaw.
- Do not add a View VM, multi-result AWBC, retained register/frame export,
  String NeedHandle, copied type/endpoint table, source identity, extension
  trait, fallback resolver, compatibility reader, or dual carrier.
- Do not implement production Need timeout, line-plan, Dialogue/RichText,
  Stream/Watch, CSS, Takumi, or unrelated producer outcomes in this design
  return. Their accepted allocations/consumers are inputs only where necessary
  to prevent collision or identity breakage.

## Expected output

Return one archive named
`arcweft-lang-01.5.1.1.2.1.1.1.1-checked-match-coverage-runtime-need-identity-and-awbc-allocation-correction-final-contract.zip`.
It must be a complete corrected design answer, not a delta, pointer, patch,
code overlay, compatibility package, or validation-only response.
