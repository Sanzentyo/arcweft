# Lang-01.3.1.2.3.2 — generic ownership identity and slot reconciliation correction

## Sequence position and precedence

This is Lang-01.3.1.2.3.2. It is a narrow mandatory correction to the returned
Lang-01.3.1.2.3 affine runtime-value contract and Lang-01.3.1.2.3.1
affine/View/ABI1 correction. It must return before G1.2 proceeds.

The parent packages remain authoritative for the two-point ownership lattice,
value-graph traversal, checked unrestricted duplication, single opaque affine
token, staged Copy/Move/Drop, closed payload, capture/pattern/constant plans,
whole-execution snapshot/activation, retained/render View unrestricted-only
admission, handler Move, ABI 1/codec 8, and their ordered compile-clean
interleave. This request may supersede only the identity/slot/transaction rows
that are incomplete below.

The implemented classifier at production Git commit
`b76465c128322be2d5e66398bc6c30794ca0276f` is accepted and must not be
redesigned. No Stream handle or affine token is constructible.

Parent archive identities are:

- Lang-01.3.1.2.3:
  `d053fae201afa104f7db9914aebbc08f2456875d1229f5325f86235d4bc0ea94`;
- Lang-01.5.1.1.2:
  `87b7f7bea85bc54254e3a979f0d668026ab75cb1c71955fd7a0f740e4f30c1c6`;
  and
- Lang-01.3.1.2.3.1:
  `a52453fd07fdacf10205cbf621077f923ded714b83e4c64b9b69c52a7350ff7f`.

## Inspected production and package evidence

Current production has:

- `RuntimeRecord` values represented by ordered vectors whose fields carry
  names, but no typed record-field identity;
- `RuntimeEnv` represented by nested name-indexed scopes, but no stable runtime
  local-slot identity, slot revision, or HIR-to-runtime slot projection;
- AWBC typed register IDs, but no diagnostic union covering every runtime slot
  domain; and
- no execution-instance identity shared by core owner evidence and the runtime
  driver.

Exhaustive searches of current production, maintained docs, and all three
returned packages find references but no definitions for
`ExecutionInstanceId`, `RuntimeRecordFieldId`, and `RuntimeLocalSlotId`.
Lang-01.3.1.2.3 G1.2 also references `RuntimeOwnedSlotId`,
`RuntimeOwnershipTransactionId`, `RuntimeMovedValueEvidence`,
`RuntimeDroppedValueEvidence`, prepared Copy/Move records, slot revisions,
transfer commit errors, and transaction limits without exact Rust-shaped
owners. Lang-01.3.1.2.3.1 activation accepts an undefined
`RuntimeFreshExecution`.

## Why local implementation judgment is unsafe

1. `ExecutionInstanceId` appears inside Serde owner/snapshot evidence and the
   domain-wide activation map. Selecting an integer, random UUID, content ID,
   host ID, or composite identity changes canonical save bytes/digests,
   collision and allocation semantics, replay, dormant restore, and atomic
   replacement.
2. A record path keyed by name, layout ordinal, or authored ordinal changes
   deterministic first-error selection and duplicate-name behavior.
3. A local slot keyed globally, per scope, by name, or by projected HIR
   `LocalId` changes shadowing, closure capture, pattern destinations, stale
   revision checks, and plan identity. Core cannot depend on
   `arcweft-lang-hir`.
4. Transaction and slot evidence shapes decide which owners are returned on
   failure and which checks can become infallible after the first committed
   move. Placeholder IDs would therefore change failure atomicity.

These outcomes are language/runtime and persistent-format decisions, not
unobservable Rust details.

## Required decisions

1. Define the exact lower-layer owner, representation, visibility, traits,
   constructors, accessors, ordering, and canonical codec of
   `ExecutionInstanceId`.
2. Define fresh execution creation end to end:
   - the exact `RuntimeFreshExecution` owner and fields;
   - who allocates or derives an execution ID;
   - collision/exhaustion behavior;
   - restart/replay/save/restore semantics;
   - empty versus replacement activation; and
   - how the driver proves one active execution per domain without moving the
     identity authority upward into core.
3. Define `RuntimeRecordFieldId` exactly, including its relationship to
   authored order, accepted nominal layout order, anonymous records, duplicate
   field-name rejection, canonical path comparison, and codec representation.
4. Define `RuntimeLocalSlotId` exactly, including allocation scope/lifetime,
   nested-scope shadowing, reuse policy, slot revision, mutable bindings,
   suspension, restore, and the layer-correct HIR/sema/runtime-plan projection.
   State how HIR `LocalId` maps without a core-to-HIR dependency.
5. Define the complete `RuntimeOwnedSlotId` enum and canonical ordering for
   environment locals, closure captures, AWBC registers/frame locals, mailbox
   lanes, child/transfer packets, and cleanup slots. It is diagnostic evidence,
   not storage.
6. Define exact Rust-shaped owners/APIs/traits for
   `RuntimeOwnershipTransactionId`, slot revisions,
   `RuntimeMovedValueEvidence`, `RuntimeDroppedValueEvidence`,
   `RuntimePreparedCopy`, `RuntimePreparedMove`,
   `RuntimeTransferCommitError`, transaction limits, and the transaction owner
   itself. Close every symbol used by the final G1.2 snippets rather than
   leaving prose-only placeholders.
7. Specify preflight/stage/commit behavior and owner return for every error:
   stale revision, source not live, destination not empty, type mismatch,
   duplicate owner, affine copy, budget, allocation, and commit mismatch.
   Identify the exact point after which commit is infallible.
8. Reconcile canonical `RuntimeValuePath` construction with these IDs and all
   current graph shapes, including iterator remainder and nominal/anonymous
   record order. Fix path comparison and first-error precedence.
9. Reconcile snapshot/restore codecs and digests with execution, slot, and
   owner identity. State which IDs persist, which are rebuilt, and how tampered
   or duplicate evidence fails before activation.
10. Provide the corrected G1.1/G1.2/G1.3/G1.4 compile-clean order, explicitly
    preserving `b76465c12` and naming the first cut where each identity becomes
    constructible or serialized.

## Required producer and consumer inventory

Inspect and cover at least:

- `arcweft-core::value::ownership`, `RuntimeValue`, record/nominal record,
  sequence/iterator, `RuntimeBinding`, and `RuntimeEnv`;
- HIR local/capture identities and sema/runtime-plan capture and pattern
  projection, with layer direction preserved;
- structured engine scopes, pattern binding, closure capture, suspension,
  mailboxes, child transfer, and cleanup;
- AWBC registers, frames, verifier facts, fibers, and snapshots;
- runtime-driver execution creation, activation map, save/restore, replay, hot
  replacement, and the persisted affine allocator cursor; and
- bundle/save codecs and canonical digest owners affected by any serialized
  identity.

## Tests to specify

- execution-ID creation, collision/exhaustion, deterministic codec/golden
  bytes, restart/replay behavior, and domain-wide activation exclusivity;
- record-field path order for anonymous and nominal records, authored versus
  layout order, duplicate names, exact-limit/one-over, and deterministic first
  error;
- local-slot allocation, nested shadowing, scope exit, reuse/non-reuse,
  mutation revision, suspension/restore, and HIR-to-plan mapping;
- canonical ordering and codec/diagnostic rendering for every
  `RuntimeOwnedSlotId` variant;
- Copy/Move preparation and commit, stale revision, duplicate owner,
  destination race, budget/allocation failure, exact source preservation, and
  no fallible branch after first take;
- moved/dropped evidence and use-after-move/use-after-drop diagnostics;
- canonical `RuntimeValuePath` nesting and first-error precedence across all
  aggregate, capture, environment, register, mailbox, transfer, and cleanup
  domains;
- snapshot tamper cases for missing/extra/duplicate execution, owner, slot,
  revision, and allocator-cursor evidence; and
- compile-fail/API tests proving raw ID constructors, identity rebinding,
  upward core dependencies, fake tokens, and reduced placeholder variants are
  unreachable.

## Constraints and non-goals

- Do not redesign the shipped ownership lattice/classifier or add a parallel
  value/path/slot/environment model.
- Do not use names, source spans, debug strings, process-local pointers, or
  iteration accidents as identity.
- Do not let `arcweft-core` depend on HIR, sema, runtime-plan, or driver.
- Do not add public fake token/handle/execution constructors, source gates,
  compatibility aliases, dual readers, migration shims, or side tables.
- Do not start G1.3/G1.4, View, AWBC wire, or Stream publication in this
  correction.
- Preserve ABI 1/codec 8 and the correction package's View/save/activation
  results.

## Expected output

Return one independently usable design-only archive named
`arcweft-lang-01.3.1.2.3.2-generic-ownership-identity-and-slot-reconciliation-correction-final-contract.zip`.
It must contain `OPEN_QUESTIONS=0`, exact Rust-shaped definitions for every
G1.2 symbol closure, execution/local/record/transaction identity rules,
canonical codec and ordering decisions, a narrow supersession delta against
the two affine parent packages, a complete producer/consumer/deletion
inventory, corrected compile-clean order, and positive/negative/tamper/full
test matrices. Do not include a production code overlay.
