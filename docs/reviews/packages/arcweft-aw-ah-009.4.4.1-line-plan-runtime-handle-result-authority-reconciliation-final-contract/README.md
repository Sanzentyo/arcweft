# AW-AH-009.4.4.1 final contract

**Archive role:** design-only, independently usable implementation contract.  It contains no production overlay, patch, generated Rust source, compatibility shim, or old-reader proposal.

**Requested artifact:** `arcweft-aw-ah-009.4.4.1-line-plan-runtime-handle-result-authority-reconciliation-final-contract.zip`

**Repository authority used:** `Sanzentyo/arcweft` `origin/main` at
`9138efeeabdfca56809e8ad9c16fc85380ae18c5`.

**Request-preserved authored baseline string:**
`15ad861a954249a9430b32d53ae0fc79c019a4f0`.
The repository-resolved production predecessor inspected through GitHub is
`15ad861a9b89e8b4b69f40381d00e74ab7392961`; the discrepancy is recorded rather
than silently normalized.

## Final decisions in one page

1. `StageApi` and line context are checked non-values.  A Character look is an
   existing exact entity value.  `StageActorHandle`, `CueHandle`, and
   `VoiceHandle` are affine exact opaque values owned by the existing
   `RuntimeValue::Opaque` algebra.
2. The existing opaque owner/value implementation gains value class and
   persistence authority.  No handle label, source spelling, or copied producer
   table participates in validation or destruction.
3. Every live handle carries one deterministic token
   `(artifact generation, persistent owner fiber, dialogue site, occurrence,
   handle site, issuance ordinal)`.  The active dialogue owns the sole ledger.
4. The sole RuntimePlan owner of executable line-plan work is the existing
   `LineTaskGroup`: it gains source-ordered activation `FlowOp`s, exact result
   type, and handle-site declarations.  There is no runtime `LinePlan` tree.
5. `FlowOp::Dialogue` gains the sole result target `(R, RuntimePattern)`.  A
   line result is committed once into a hidden typed dialogue cell; that value
   is published through the sole pattern once, after successful joined close.
6. `at` lowers to `RuntimeLineOperation::Schedule`, not to wait or a pure
   intrinsic.  It evaluates delay, then captures, arms a real child, and
   returns a `CueHandle`.
7. Stage acquire/look/release/cancel are typed Sans-I/O presentation commands.
   Native, Web, and headless hosts consume the same data and never parse a
   callable or opaque display label.
8. `line.voice_handle()` is a dedicated line-context operation.  It resolves
   the exact active voice, can suspend for lazy start, and fails activation on
   absence or host rejection because its accepted return type is `VoiceHandle`,
   not `Option<VoiceHandle>`.
9. `LineEffectRequest::{RegisterHandle,DropHandle,Out}` and `LineOutRequest`
   are deleted, together with AWBC effect kinds/readers and all match arms.
10. RuntimePlan and AWBC admission are fail-closed and prove exact types,
    producers, activation ownership, schedule topology, one result on every
    completing path, and cleanup coverage.
11. AWBC remains ABI version `1` and codec version `1`.  Reserved opcodes
    `0x1e` and `0x20` become `ExecuteLineOperation` and
    `CommitDialogueResult`; `Dialogue` remains `0x86` with a replaced typed
    payload.  There is one reader only.
12. Structured and AWBC runtimes share one reducer and one normalized trace
    grammar.  Their request, result, cleanup, binding, status, and diagnostic
    sequences must compare byte-for-byte.
13. Save/replay persist activation identity, issuance counters, handle ledger,
    scheduled child captures, result state, and suspended frames.  Active
    dialogues remain pinned to their original artifact generation across hot
    replacement.
14. Temporary `Named` handle spellings, capacity-family `voice_handle`, runtime
    semantic exclusions, string carriers, fixture exceptions, and obsolete
    tests are removed in one compile-clean interleave.
15. Plan construction, execution, host queues, result/capture values, and
    restore validation have explicit hard limits and deterministic yield
    budgets.

## Reading order

1. `FINAL_CONTRACT.md`
2. `RUST_OWNERS_AND_APIS.md`
3. `RUNTIME_PLAN_AND_ADMISSION.md`
4. `IDENTITY_LIFETIME_AND_FAILURE.md`
5. `COMMAND_AND_RESULT_TIMELINES.md`
6. `AWBC_SCHEMA_CODEC_VM.md`
7. `SAVE_REPLAY_HOT_REPLACEMENT.md`
8. `STRUCTURED_AWBC_PARITY.md`
9. `BOUNDED_WORK.md`
10. `IMPLEMENTATION_INTERLEAVE.md` and `DELETION_MATRIX.md`
11. `TEST_MATRIX.md`
12. `REQUIREMENTS_TRACEABILITY.md`
13. `VERIFICATION.md`
14. `OPEN_QUESTIONS.md`

## Acceptance statement

The contract is closed.  `OPEN_QUESTIONS.md` contains exactly `none`.  A
production implementation is acceptable only when the positive, negative,
tamper, persistence, differential, API-deletion, and CLI rows in
`TEST_MATRIX.md` are green without a fixture allowlist or source-text fallback.
