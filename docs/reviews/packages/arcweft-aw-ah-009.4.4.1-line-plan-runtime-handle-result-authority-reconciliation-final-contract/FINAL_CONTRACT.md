# Final normative contract

## 1. Scope and precedence

This correction is the executable runtime authority beneath AW-AH-009.4
CharacterDialogue Cut 4 and maintained post-Try convergence item 6.  It
preserves the accepted CharacterDialogue domain, typed dialogue-content
application, non-escaping `DialogueLine<R>` operation rule,
`DialogueContentSpec`, existing `LineTaskGroup` reducer topology, final
RuntimePlan type graph, generic opaque-producer authority, and structured/AWBC
parity.

The following are normative:

- current repository policy and `AGENTS.md` at the fixed baseline;
- every Arcweft-owned version marker involved in this cut remains exactly `1`;
- unreleased wire shapes are replaced in place with one writer and one reader;
- lower crates remain Sans I/O;
- all work and validation are deterministic and bounded;
- missing proof is a typed error, never a fallback.

## 2. Closed invariants

| ID | Invariant |
|---|---|
| INV-01 | There is exactly one executable runtime value algebra: `RuntimeValue`. |
| INV-02 | A line handle is always `RuntimeValue::Opaque` with an admitted existing opaque owner and an affine handle class. |
| INV-03 | `StageApi` and line context cannot be materialized, saved, captured, compared, or returned. |
| INV-04 | Character-look values use the existing exact entity-reference family and retain exact `CharacterId` ownership. |
| INV-05 | The sole live-resource owner is the active dialogue handle ledger; an opaque token is not a host resource by itself. |
| INV-06 | No destructor, producer, type, Character, activation, or resource identity is reconstructed from a display label. |
| INV-07 | The sole RuntimePlan owner of line setup/result/schedule declarations is `LineTaskGroup`; no `RuntimeLinePlan` or parsed statement tree exists. |
| INV-08 | `FlowOp::Dialogue` owns one exact result target.  A completing activation commits exactly once and a successful dialogue publishes exactly once. |
| INV-09 | A line-task child cannot write the result cell.  Only the activation owner or an explicitly admitted cancellation-result function for the same activation may commit. |
| INV-10 | A scheduled callback is a real reducer child with evaluated captures and a deadline; it is never a synthetic wait or unexecuted intrinsic. |
| INV-11 | Every stage command is typed Sans-I/O data and carries proof coordinates; every host response echoes those coordinates. |
| INV-12 | Result pattern matching and affine transfers are transactional: mismatch produces no partial binding, move, or drop. |
| INV-13 | Structured and AWBC execution share the reducer order and emit the same normalized observations. |
| INV-14 | Snapshot restore validates the full candidate before mutating engine, host, queue, ledger, or result state. |
| INV-15 | Active dialogue and handle identities are generation-pinned; hot replacement never rewrites them in place. |

## 3. Required decision closure

### D1 — value owner

- `StageApi(CharacterId)`: checked non-value capability.
- `LineContext`: checked non-value activation capability.
- Character look: unrestricted exact entity-reference value
  `CharacterLookId { character, look }`.
- `StageActorHandle`: affine exact opaque value, exact Character precision where
  known.
- `CueHandle`: affine exact opaque value; schedule and stage-transition cue
  kinds are payload state under one exact source type.
- `VoiceHandle`: affine exact opaque value.

The existing `RuntimeOpaqueTypeOwner` and `RuntimeOpaqueValue` are extended
in place with value-class and persistence fields.  `try_wrap`, `accepts`,
`ownership`, canonical save encoding, AWBC type lowering, constant admission,
and recursive traversal are extended on their existing implementations.

### D2 — activation-scoped identity

`DialogueActivationId` is deterministic and persisted.  It combines the
accepted artifact fingerprint, persistent owner-fiber id, dialogue content
site, and the owner's dialogue occurrence ordinal.  `RuntimeLineHandleToken`
adds a dense handle site and per-site issuance ordinal.  Same-site looped or
repeated operations therefore produce distinct, replay-stable tokens.

### D3 — lifetime

The dialogue ledger owns every handle lease.  A RuntimeValue carries only the
validated token.  Bind, move, result commit, result publish, explicit `_`,
explicit drop, scope exit, cancellation, failure, return/goto, and child exit
change the ledger's typed owner slot.  Destruction dispatches from admitted
handle kind plus ledger state, never a string.

### D4 — RuntimePlan owner

`LineTaskGroup` gains:

- `activation_ops: Box<[FlowOp]>` in source order;
- `result_type: RuntimePlanTypeId`;
- `handle_sites: Box<[RuntimeLineHandleSite]>`;
- scheduled child capture declarations.

The existing node graph, cancel rules, and cleanup policy remain in that same
owner.  `RuntimeDialogueContentPlan` retains only the group id.

### D5 — result and pattern

`FlowOp::Dialogue` gains `RuntimeDialogueResultTarget { ty, pattern }`.
`FlowOp::CommitDialogueResult` is the only result producer.  It writes one
hidden cell in `DialogueState`.  On normal joined close, the engine validates
the result and pattern in a temporary transaction, transfers kept handles,
drops explicit discards, commits parent locals, and resumes.  Cancellation,
failure, return, or goto that bypass successful completion drops the hidden
result and never binds the target.

### D6 — `at`

`at` resolves to a dedicated checked scheduling identity and lowers to
`RuntimeLineOperation::Schedule`.  Delay is evaluated and validated first;
callback captures are evaluated left-to-right next; only then are token,
deadline, child live state, and `CueHandle` created.  Zero delay runs in the
post-result-commit activation microstep.  Negative, conversion overflow,
deadline overflow, capture failure, and issuance overflow fail before a
partially armed cue exists.

### D7 — `actor.look`

The operation validates exact actor producer, generation, activation,
Character, and look ownership.  It emits
`RuntimeStageCommand::SetCharacterLook` through the sole typed presentation
command owner.  Native, Web, and headless implementations return the same
`RuntimeStageCommandOutcome` shape.

### D8 — `voice_handle()`

The callable moves from `CapacityMethodId` to `LineContextMethodId`.  Runtime
lowering reads only the current dialogue activation.  `Ready` returns a lease;
`Lazy` issues a typed start request and suspends activation; `Absent` and host
failure are structured activation failures.  Multiple calls issue distinct
affine leases to the same voice session and use per-site issuance ordinals.

### D9 — string routes

Delete:

- `LineEffectRequest::RegisterHandle`;
- `LineEffectRequest::DropHandle`;
- `LineEffectRequest::Out`;
- `LineOutRequest`;
- `AwbcEffectKind::{RegisterHandle,DropHandle,Out}`;
- all codec, verifier, VM, product-step, observation, host, CLI, and test match
  arms for those variants.

There is no released external consumer.  The present consumers are internal
observation/CLI/bundle plumbing and cannot justify retention.  Typed handles
are values; typed line results are the dialogue result cell.

### D10 — construction and admission

The builder proves exact result type, local and capture types, handle producer
owners, unique sites, activation-only capabilities, schedule-child ownership,
callback signatures, exactly-one dynamic commit on each completing path,
zero commits on non-completing nonlocal paths, and cleanup coverage.  Every
missing fact yields a structured lowering/admission error.

### D11 — AWBC

AWBC ABI and codec versions remain `1`.  The final schema adds one line
operation table, one line-activation function kind, one typed result target,
handle-site declarations, result/ledger snapshot state, and two instructions:

- `0x1e ExecuteLineOperation`;
- `0x20 CommitDialogueResult`.

`0x86 Dialogue` retains its opcode and replaces its payload.  Replaced effect
kinds and old payload decoders are deleted in the same atomic cut.

### D12 — parity

A normalized trace observes activation allocation, host request/output,
operation completion, cue arming/firing, advance, close, cleanup, result
publish, binding, status, and diagnostics.  Structured and AWBC runs must
produce identical trace bytes after executor-only ids are excluded.

### D13 — persistence and replacement

Snapshots persist activation id, pinned generation, phase, activation frame,
per-site issuance counters, handle ledger, schedule deadlines/captures/states,
line reducer state, host command sequence, and result state.  Replay consumes
recorded logical ticks and typed outcomes.  Active dialogues and continuations
remain on their old generation; a hot replacement affects new activations
only.

### D14 — compile-clean interleave

The interleave removes temporary `Named` handle spellings and the
capacity-family voice method while adding direct semantic types; then lands
opaque authority, RuntimePlan operations, structured execution, AWBC, hosts,
persistence, and final string-route deletion in dependency order.  No
compatibility alias, source-name recognizer, fixture branch, or dual reader is
allowed at an intermediate compile gate.

### D15 — bounded work

All counts and value traversals have hard limits.  Runtime reducer work uses a
deterministic transition budget and resumable cursor.  Oversize is a typed
admission, execution, or restore error as specified in `BOUNDED_WORK.md`.

## 4. Non-goals retained

This cut does not redesign RichText, CharacterDialogue patch/default
semantics, View, Await/Need, Const, timeout, CSS, or Takumi.  It does not revive
source-parsed line plans, raw statement readers, callee-name recognizers,
source-site exceptions, fixture allowlists, fake task specs, no-op stage calls,
or pure `at` intrinsics.
