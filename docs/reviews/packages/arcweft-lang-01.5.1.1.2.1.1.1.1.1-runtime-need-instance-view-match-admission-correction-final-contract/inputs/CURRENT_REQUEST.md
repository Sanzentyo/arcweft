# Lang-01.5.1.1.2.1.1.1.1.1 — runtime Need instance and View Match admission correction

## Sequence, inputs, and precedence

This is a narrow mandatory nonnumeric correction to the returned
Lang-01.5.1.1.2.1.1.1.1 checked-Match/runtime-Need package. It does not reopen
the user-selected AWBC opcode, function-kind, function-flag, varint, encoder,
or implementation-order allocation.

Required retained inputs are:

- the primary
  [Lang-01.5.1.1.2.1 request](2026-08-21-lang-01.5.1.1.2.1-reactive-unary-need-match-reconciliation.md);
- the
  [design-validation correction](2026-08-21-lang-01.5.1.1.2.1.1-reactive-unary-need-match-design-validation-correction.md);
- the
  [generic-Match/typed-Need ABI correction](2026-08-21-lang-01.5.1.1.2.1.1.1-generic-match-and-typed-need-producer-abi-correction.md);
- the
  [checked-Match/runtime-Need correction](2026-08-21-lang-01.5.1.1.2.1.1.1.1-checked-match-coverage-runtime-need-identity-and-awbc-allocation-correction.md),
  including its later user-directed semantic-range allocation;
- the retained returned archive
  [`arcweft-lang-01.5.1.1.2.1.1.1.1-checked-match-coverage-runtime-need-identity-and-awbc-allocation-correction-final-contract.zip`](../packages/zips/arcweft-lang-01.5.1.1.2.1.1.1.1-checked-match-coverage-runtime-need-identity-and-awbc-allocation-correction-final-contract.zip),
  SHA-256
  `DDD097E8057A8D45018528431790C20A2DE665CDE40F0329B82CB0366CF95D32`;
- its searchable
  [frozen mirror](../packages/arcweft-lang-01.5.1.1.2.1.1.1.1-checked-match-coverage-runtime-need-identity-and-awbc-allocation-correction-final-contract/README.md);
- the
  [repository intake and reconciliation](../../implementation/2026-08-22-lang-01-5-1-1-2-1-1-1-1-checked-match-need-identity-return-intake.md);
- maintained
  [AWBC runtime](../../02-runtime/executable-runtime-core.md),
  [Need timeout](../../02-runtime/need-timeout.md),
  [scheduler](../../02-runtime/async-scheduler.md), and
  [pattern runtime](../../02-runtime/control-flow-runtime.md) contracts; and
- current production at
  `cbf0acedb98de260d8ecaab70a39933c39f30708`.

Current production, maintained stable documentation, and later accepted
contracts take precedence over stale package observations. Every Arcweft-owned
version marker remains exactly `1`. No compatibility reader, old numeric
reader, identity translation table, String fallback, or dual carrier is
authorized.

## Frozen numeric authority — out of scope

The returned package predates the user-directed full semantic reorder. Its
numeric tables are preserved only as frozen evidence. The following maintained
choices are already final and MUST NOT be redesigned or reallocated in this
return:

- the semantic opcode families and bytes in
  `docs/02-runtime/executable-runtime-core.md`;
- dense function kinds `Flow..LineTask = 0..8`;
- `Deterministic`, `MayAllocate`, `MaySuspend`, `HasDynamicTarget`,
  `NeedProducer`, `OwnsStreamProducer` at flag bits `0..5`;
- discriminant-only `#[repr(u8)]` numeric enums, allocation-free const decode,
  direct numeric Serde/private Wire, and inherent family behavior;
- canonical shortest base-128 varint for every ordinary `u32`;
- one final encoder buffer and direct borrowed-reader decode; and
- no tombstone or compatibility interpretation for unassigned bytes.

The archive MAY restate these as an external prerequisite but MUST NOT emit a
second allocation table. A numeric mismatch with the frozen returned package
is not an open question.

## Why the returned nonnumeric contract is not implementable

### 1. AlwaysStart publishes distinct launches into one terminal cell

The return simultaneously requires equal-input `AlwaysStart` launches to share
one `NeedId`, distinct `TaskKey`/`TaskId` values, and conflicting terminal
values for one `NeedId` to fail. Separate I/O launches can legitimately finish
differently, so the second launch would be rejected as a false correlation
conflict. The returned `TaskKey` also contains the launch ordinal and `TaskId`
hashes that ordinal again, mixing coalescing and launch identity.

### 2. View identity is nonexistent and cyclic

Current View authority is `ViewProgramId` plus
`AcceptedViewProgramRevision([u8; 32])` in
`crates/arcweft-view/src/view/identity.rs:23-223`. The return instead invents a
`ViewProgramSemanticDigest` and canonical-u32 revision. The accepted revision
is a digest of the typed View-program semantic transcript. Including it in a
Match digest which is retained by the View program is cyclic, and including it
in `NeedId` makes a compatible old/new-revision rebind change the claimed
logical identity.

### 3. Generic Match is incorrectly gated by View persistence

The return runs ownership/persistence admission inside generic
`CheckedMatch::try_from_hir`. This rejects legal ordinary Match expressions
which move or destructure affine, Stream, callable, or non-snapshot values even
when no View retains them. Coverage validity and retained-View admission are
different semantic products.

### 4. Ownership evidence cannot be constructed

`AcceptedNominalSemantics::Opaque` gains value-class/persistence fields, but
current `AcceptedNominalInventoryInput` at
`crates/arcweft-lang-sema/src/registration/environment_input.rs:190-409`
carries only the runtime producer. `AgentResource` and `AgentResourceBody`
contain no exact resource-registry identity. The proposed Need handle owns a
boxed argument vector and can contain snapshot-clone values, while `Ref` is a
String-backed runtime identity. `ViewValue` has no unconditional snapshot
layout. The returned unconditional Copy/SnapshotClone rows are therefore not
owned by current evidence.

### 5. Digest, event, and compile-clean boundaries are incomplete

The return redeclares `RuntimeValueDigest` despite the existing
`arcweft_core::entry::RuntimeValueDigest` and
`RuntimeValue::try_canonical_bytes`/`try_digest`. It gives no exact producer
contract transcript, ordered-argument/source transcript, final Task event and
Need-state schemas, or event-to-journal correlation API. It stores a task-plan
digest inside the same plan from which that digest is derived.

It also deletes String task identity and direct Await before the typed carrier
and delays journal/save/replay migration until several cuts later. With no
fallback reader, those intermediate states cannot compile and execute.

## Required exact decisions

Every decision below is mandatory. Do not return alternatives.

### A. Separate producer, terminal-cell, coalescing, and launch identity

1. Add these typed owners:

   ```rust
   pub struct NeedProducerInstanceKey([u8; 32]);
   pub struct TaskLaunchOrdinal(u64);
   ```

   `NeedProducerInstanceKey` uses exactly this transcript:

   ```text
   domain = "arcweft.need.producer-instance.v1\0"
   family_tag:u8
   NeedProducerContractDigest:[u8;32]
   TaskPlanSemanticDigest:[u8;32]
   producer_site:u32-le
   payload_type_digest:[u8;32]
   arguments_digest:arcweft_core::entry::RuntimeValueDigest
   ```

   A View task-plan semantic digest includes `ViewProgramId`, stable site, and
   `CheckedViewMatchAdmissionDigest`, but excludes
   `AcceptedViewProgramRevision`.
2. Keep fixed-byte `NeedId`, `TaskKey`, and `TaskId` with these exact roles:

   - `NeedId`: one concrete terminal cell;
   - `TaskKey`: one generation-bound coalescing key;
   - `TaskId`: one actual launched task.

3. Use these exact transcripts:

   ```text
   NeedId = BLAKE3("arcweft.need.id.v1\0"
                   || NeedProducerInstanceKey
                   || policy:u8
                   || launch_ordinal:u64-le)

   TaskKey = BLAKE3("arcweft.task.key.v1\0"
                    || GenerationId:u64-le
                    || NeedProducerInstanceKey
                    || policy:u8)

   TaskId = BLAKE3("arcweft.task.id.v1\0"
                   || TaskKey
                   || launch_ordinal:u64-le)
   ```

   Policy tag `0` is `JoinSameKey`; tag `1` is `AlwaysStart`.
   `JoinSameKey` always uses ordinal `0`. Equal generation and instance key
   therefore produce the same NeedId, TaskKey, and TaskId. `AlwaysStart` uses a
   journal-owned counter scoped to `(GenerationId, NeedProducerInstanceKey)`;
   ordinal starts at `1` and increases monotonically. It gives every launch a
   distinct NeedId and TaskId while TaskKey retains the producer group.
   Allocation, journal insertion, and launch acceptance are one transaction;
   failure consumes no ordinal and exposes no task.
4. A reusable immutable `RuntimeNeedHandle` and `MakeNeedHandle` producer are
   `JoinSameKey` only. The verifier rejects an AlwaysStart task plan for
   `MakeNeedHandle`. An AlwaysStart producer descriptor obtains a concrete
   `RuntimeNeedHandle` only as the output of the accepted launch boundary; no
   reusable pre-launch handle exists for that policy.
5. `AwaitMany` first derives the source-order base/child producer-instance key,
   then applies the same policy relation. Duplicate item values at distinct
   source indexes remain distinct. Direct Await of a `RuntimeNeedHandle` reads
   its concrete JoinSameKey `NeedId`; it does not rederive or parse it.
6. Timeout derives a new producer-instance key from the exact source `NeedId`,
   timeout contract/site, and limit value digest, then publishes its own
   concrete JoinSameKey Need cell. It never mutates or parses the source ID.
7. Terminal idempotence/conflict is scoped to exact
   `(GenerationId, NeedId, NeedProducerContractDigest, cursor)`. Different
   AlwaysStart NeedIds may publish different values. JoinSameKey observers
   share the one publication and keep separate observer state outside task
   identity.

### B. One total fixed-byte and runtime-value digest policy

1. Reuse `arcweft_core::entry::RuntimeValueDigest`; do not define another type
   with that name. Reuse the existing canonical runtime-value byte/digest owner
   for every admitted argument and AwaitMany item.
2. Empty arguments are the existing canonical digest of
   `RuntimeValue::Tuple([])`. A source-order argument or AwaitMany item list is
   represented as the corresponding canonical runtime Tuple value and uses the
   same owner. Do not add `ordered_source_digest` grammar, use map iteration,
   generic Serde, or treat `RuntimeValueDigest::ZERO` as empty.
3. Make the existing canonical runtime-value encoder sink-parametric so the
   same visitor writes either final bytes or a BLAKE3 sink. The digest route
   creates no intermediate byte buffer and does not duplicate the grammar.
   Add `RuntimeValue::NeedHandle` as one canonical variant in that owner.
4. `NeedProducerInstanceKey`, `NeedId`, `TaskKey`, and `TaskId` reserve all zero
   as invalid. A zero hash result returns a typed error; there is no rehash
   fallback. `GenerationId(0)` and Join ordinal `0` remain valid. Semantic
   digest types accept the hash's full output; absence always uses `Option`.
5. Give the exact ordered inputs for `NeedProducerContractDigest` and
   `TaskPlanSemanticDigest`, and prove they do not duplicate value/digest
   authority.

### C. Make generation, events, and terminal correlation constructible

1. Move the existing runtime `GenerationId(u64)` to
   `arcweft_core::task::GenerationId` as the one shared typed owner. Update
   runtime-driver to consume that owner; do not retain a driver-local duplicate
   or conversion DTO.
2. Select these exact final core schemas:

   ```rust
   pub struct NeedProducerInstance {
       key: NeedProducerInstanceKey,
       contract: NeedProducerContractDigest,
       plan: TaskPlanSemanticDigest,
       payload_type: RuntimeTypeSemanticDigest,
       arguments: RuntimeValueDigest,
   }

   pub struct TaskSpec {
       pub generation: GenerationId,
       pub producer: NeedProducerInstance,
       pub class: TaskClass,
       pub priority: TaskPriority,
       pub cancel_scope: CancelScopeId,
       pub policy: TaskPolicy,
       pub outcome: TaskOutcomeContract,
       pub request: HostTaskRequest,
       pub debug_label: String,
   }

   pub struct TaskCorrelation {
       pub generation: GenerationId,
       pub producer: NeedProducerInstanceKey,
       pub producer_contract: NeedProducerContractDigest,
       pub need: NeedId,
       pub task_key: TaskKey,
       pub task_id: TaskId,
       pub launch_ordinal: TaskLaunchOrdinal,
   }

   pub struct TaskHandle {
       pub correlation: TaskCorrelation,
   }

   pub struct TaskEvent {
       pub correlation: TaskCorrelation,
       pub cursor: TaskPublicationCursor,
       pub kind: TaskEventKind,
   }

   pub struct RuntimeNeedState {
       pub correlation: TaskCorrelation,
       pub cursor: Option<TaskPublicationCursor>,
       pub state: Need<RuntimeNeedOutcome>,
   }

   pub enum RuntimeNeedOutcome {
       Value(RuntimePayload),
       InfrastructureFailure(RuntimeTaskFailure),
   }
   ```

   Give exact final correlated shapes for `AwaitTarget`, `AwaitManyTarget`,
   `FiberAwaitManyInFlight`, `RuntimeNeedHandle`, runtime journal,
   save/replay, and host/adaptor envelopes around these owners.
3. `TaskSpec` contains no caller-supplied NeedId, TaskKey, TaskId, or ordinal.
   `TaskHost::ensure_task` derives/allocates correlation by policy and returns
   `TaskHandle`.
4. One launch produces one task event stream. JoinSameKey observer fanout is an
   observer-table operation, not multiple terminal publications. Define exact
   validation, cursor precedence, duplicate/stale/conflict behavior,
   cancellation, journal insertion, and rollback.
5. Domain errors remain typed `Ready(Result::Err(...))` values.
   `InfrastructureFailure` is only a host/runtime failure; cancellation remains
   the existing `Need::Cancelled` state.
6. Replace `AwbcTaskPlan.need_id` with a mandatory typed producer row, but do
   not store `plan_digest` inside the plan itself. The final plan owns
   `semantic_digest(&AwbcProgram)`. Runtime-plan/bundle/snapshot bindings may
   retain an expected digest and verifier/restore recompute it.

### D. Use the current non-cyclic View identity split

1. `CheckedMatchSemanticDigest` is owned by lang-sema and commits exactly:

   - checked scrutinee expression digest and type digest;
   - source-order arms;
   - checked pattern digest per arm;
   - each binding's stable pattern coordinate and type;
   - checked guard expression digest and direct-Boolean-literal guard class;
   - checked arm body expression digest; and
   - coverage exhaustive value and sorted unreachable arm ordinal/reason.

   It excludes:

   - `ViewProgramId`;
   - `AcceptedViewProgramRevision`;
   - View site/arm/output coordinates;
   - View ownership/persistence admission and resource catalog;
   - coverage accounting counters;
   - HIR/session/arena IDs; and
   - source/debug spelling.

2. Compiler projection constructs a separate
   `CheckedViewMatchAdmissionDigest` from:

   - `CheckedMatchSemanticDigest`;
   - source-order retained output bindings and captures;
   - each binding/capture type digest and `Copy`/`SnapshotClone` disposition;
   - the exact `OwnershipEvidenceDigest` actually consulted; and
   - `CheckedNeedProducerAdmissionDigest`.

   It excludes `ViewProgramId`, revision, and site. It commits referenced
   nominal/opaque evidence, not an unrelated whole-catalog digest.
3. Define the persistent coordinate exactly as:

   ```rust
   pub struct CheckedViewMatchCoordinate {
       pub program: ViewProgramId,
       pub site: ViewMatchSiteId,
       pub admission: CheckedViewMatchAdmissionDigest,
   }
   ```

   `ViewMatchSiteId` derives from `ViewProgramId`, enclosing stable declaration
   identity, and checked-expression child-role path. It excludes HIR IDs,
   SourceSpan, and revision.
4. Use current `ViewProgramId` as stable program owner and current
   `AcceptedViewProgramRevision([u8; 32])` only for accepted-catalog revision,
   bundle validation, registry, and replacement transaction. Do not introduce
   `ViewProgramSemanticDigest` or a u32 revision.
5. The View task-plan semantic digest, and therefore the producer-instance key,
   commits the stable View program/site/admission evidence but excludes
   `AcceptedViewProgramRevision`.
6. Bundle admission and replacement mapping carry the accepted revision.
   Old/new revisions may differ. Live state rebind is allowed only when the
   explicit site mapping and generic Match, View admission, producer, payload,
   plan, resource-dependency, and argument identities agree. Generation changes
   the active runtime key; revision is not translated into another NeedId.

### E. Separate generic Match coverage from retained View admission

1. Generic `CheckedMatch::try_from_hir` validates exact HIR children, types,
   patterns, guards, coverage, reachability, and generic Match digest only. It
   does not run retained-value ownership or producer admission.
2. Keep the returned private bounded Maranget usefulness owner and full pattern
   matrix, subject to this guard correction:

   - only exact
     `CheckedExpressionResolution::Literal(HirLiteral::Boolean(true/false))`
     is ConstantTrue/ConstantFalse;
   - every other checked guard is Dynamic until a separate checked constant
     authority exists;
   - source evaluation and source-string folding are forbidden; and
   - ConstantFalse owns `FalseGuard` reason precedence independently of whether
     earlier rows cover its pattern.

3. Add a separate `CheckedViewMatchAdmission` which receives a
   `CheckedMatchRef`, exact retained outputs, and the legitimate ownership
   context. Its failure blocks only the View catalog/product row, not the
   generic Match fact.
4. Producer argument/capture admission is another certificate and produces
   `CheckedNeedProducerAdmissionDigest`. It may consume
   ownership evidence but it does not construct producer contract identity.

### F. Correct the ownership evidence chain and rows

1. Extend `AcceptedNominalInventoryInput` with mandatory typed
   `RuntimeOpaqueValueClass` and `RuntimeOpaquePersistence`. Carry both through
   registrar validation to the original `AcceptedNominalSemantics::Opaque`
   variant. Missing evidence rejects; no default or name-based inference is
   allowed.
   The exact publication chain is:

   ```text
   AcceptedNominalInventoryInput
     { runtime_producer, value_class, persistence }
       -> registrar (no defaults)
       -> AcceptedNominalSemantics::Opaque
          { producer, value_class, persistence }
       -> AcceptedNominalCatalogDigest
       -> CheckedOwnershipCertificate / runtime-plan projection
   ```

   `AcceptedNominalInventoryInput::new` and
   `AcceptedNominalRecord::try_new_opaque` require both fields.
2. `AgentResource` and `AgentResourceBody` use their current core Agent DTO
   owner and classify `SnapshotClone`; do not query `ResourceTypeRegistry`
   without a `TypeKind` that carries an exact resource-type identity.
3. Define the current ownership context as exactly
   `{ ProjectSymbolTable, RegisteredSemanticWorld }`. Remove
   `ResourceTypeRegistry` from it until such
   an exact typed resource key is present. A later exact resource-bearing type
   may add a legitimate typed registry input without changing current Agent DTO
   rows.
4. Correct at least these rows:

   - `Need<T>`: `SnapshotClone` because the handle retains verified arguments;
   - `Ref`: `SnapshotClone` because the carrier is `EntityRef(String)`;
   - `ViewValue`: closed rejection until an exact runtime projection/snapshot
     owner is published, using `MissingViewPersistenceEvidence`;
   - `Function`: type-level rejection; capture-free/closure decisions require
     a value-level certificate;
   - `Shared<T>`: `SnapshotClone` after child admission;
   - opaque `Plain`: `SnapshotClone` under either admitted persistence mode;
   - opaque `AffineHandle`: closed rejection;
   - affine/Stream/Function/borrow/frame-local values: closed rejection at
     retained View admission, not generic Match construction.

5. Apply this classifier only to retained View outputs/captures and Need
   producer arguments. Do not apply it to generic Match construction.
6. Reconcile every remaining current `TypeKind` with its exact current owner,
   recursion, cycle, limit, and first-error behavior. `Copy` and
   `SnapshotClone` are semantic dispositions, but must still agree with the
   concrete runtime carrier and snapshot path.

## Required compile-clean sequence

Return one deletion-driven sequence with these dependency cuts:

1. **Generic Match cut** — add the checked expression/pattern semantic encoder,
   coverage analyzer, generic `CheckedMatch`, and
   `CheckedMatchSemanticDigest`. Do not touch ownership, View, or runtime.
2. **Ownership evidence cut** — change the opaque publication chain from its
   input through catalog digest in one cut, add the total classifier and
   value-level certificates, and update every constructor/fixture. Do not add a
   ResourceTypeRegistry route.
3. **View admission cut** — add `CheckedViewMatchAdmission`, its digest, stable
   site, checked View catalog, compiler projection, bundle codec, and
   runtime-driver evaluator/replacement validation together. Do not copy the
   generic Match authority.
4. **Private identity preparation cut** — add fixed identity types, transcript
   owners, sink-parametric canonical value encoding, and the core
   `GenerationId`. Do not switch an existing public task path in this cut.
5. **Atomic task/Need carrier cut** — switch TaskSpec, TaskHandle, TaskEvent,
   RuntimeNeedState, RuntimeNeedHandle, TaskHost, engine, runtime-plan, AWBC
   verifier/VM, host adapters, Await/AwaitMany, timeout, View/line/host
   producers, journal/save/snapshot/restore/replay/replacement, codecs, bundle
   bindings, and all fixtures in one protected cut. In the same commit delete
   String NeedId/TaskKey/TaskId, caller-supplied identity, direct-Await
   surrogate, indexed suffixes, old snapshot fields, and every fallback.

Cut 5 is intentionally indivisible. Publishing only the Task schema, typed
handle, or identity codec before its persistence/replay/host consumers would be
contract-incomplete even if it happened to compile. The already-selected AWBC
numeric primitive migration and later CopyValue/Need/timeout/line/Stream
feature cuts remain external prerequisites/consumers; this return does not
redesign or reorder them.

Every public cut must compile and execute using only final owners. No empty
catalog, dummy enum row, old/new dual schema, compatibility branch, or delayed
snapshot migration is allowed.

## Required validation and artifacts

Return one independently usable design archive containing at minimum:

- `README.md`, exact reading order, full inspected Git SHA, and final status;
- `OPEN_QUESTIONS.md` containing exactly `none`;
- complete final contract, decision register, Rust schemas, owner/API map, and
  dependency graph;
- exact producer-instance/Need/Task domains and task-policy truth table;
- the reused sink-parametric canonical runtime-value owner plus exact
  producer-contract, plan, View Match, and View admission digest grammars;
- final TaskSpec/event/journal/snapshot/replay/replacement schemas;
- generic coverage and separate View ownership/admission matrices;
- corrected opaque evidence publication chain;
- deletion matrix and compile-clean sequence;
- source evidence with exact current line ranges;
- positive/negative/tamper/property/differential/exact-limit/one-over/rollback/
  structural/Tier-2 tests; and
- machine/human validation plus an internal SHA-256 manifest covering every
  payload and exact request-copy hashes.

Required tests include at least:

- equal/different producer-instance transcripts for every family;
- JoinSameKey duplicate observers sharing one launch and publication;
- two equal-input AlwaysStart launches with different terminal values and no
  conflict;
- failed launch transaction consuming no ordinal;
- Join ordinal exactly zero; AlwaysStart ordinal replay from one; TaskKey
  launch-ordinal absence; and TaskId single inclusion;
- direct Await, AwaitMany reorder/duplicate/index-boundary, timeout source/output;
- event field tamper, journal mismatch, stale/duplicate/conflicting cursor;
- zero Need/Task/producer-instance ID rejection without rehash, valid
  `GenerationId(0)`/Join ordinal zero, and missing `Option` rejection;
- empty arguments equal the canonical `RuntimeValue::Tuple([])` digest and not
  `RuntimeValueDigest::ZERO`;
- same Match semantics across different HIR allocation;
- revision-only View replacement retention with stable producer-instance
  identity;
- revision/Match/admission/producer/ownership-evidence mismatch cancellation;
- legal ordinary affine Match accepted while retained View admission rejects;
- opaque input evidence missing/tampered/default absence;
- Boolean literal true/false and nonliteral dynamic guard coverage;
- exact-limit/one-over coverage, ownership, observer, journal, restore, and
  digest work; and
- structural absence of the invented View digest/u32 revision, duplicate
  RuntimeValueDigest, resource lookup without typed key, plan self-digest field,
  String/suffix identity, copied coverage, and numeric reallocation.

The validator must fail for an unresolved alternative, policy/identity
conflation, current-View owner mismatch, generic/View admission conflation,
unconstructible evidence, incomplete event schema, delayed persistence switch,
version marker other than `1`, stale request copy, vague evidence, or manifest
mismatch.

## Constraints and non-goals

- This is design-only. Do not edit production code, tests, fixtures, manifests,
  branches, patches, PRs, or implementation overlays.
- Do not return another AWBC numeric table or reopen the selected enum/varint/
  encoder authority.
- Do not add a second public producer identity, String fallback, source-derived
  key, HIR/product identity leak, extension trait, copied registry, default
  opaque evidence, compatibility reader, or dual carrier.
- Do not redesign the accepted selector Variant/Tuple ABI, explicit guard
  Branch lowering, View/core independence, parent journal/observer/start/
  cancellation lifecycle, timeout race ordering, line-plan result authority,
  or protected Stream shapes except where this request names a concrete
  identity/admission contradiction.
- Do not implement unrelated Dialogue/RichText, Stream, CSS, Takumi, producer
  outcome, standard-map, or extension-receiver work in this design return.

## Expected output

Return one archive named
`arcweft-lang-01.5.1.1.2.1.1.1.1.1-runtime-need-instance-view-match-admission-correction-final-contract.zip`.
It must be a complete corrected design answer, not a delta, pointer, patch,
code overlay, compatibility package, or validation-only response.
