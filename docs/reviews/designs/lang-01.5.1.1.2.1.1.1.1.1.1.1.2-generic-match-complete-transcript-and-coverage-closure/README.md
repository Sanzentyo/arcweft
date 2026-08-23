# Generic Match complete transcript and coverage closure

Status: `READY_FOR_IMPLEMENTATION`

This repository-local accepted design closes maintained request
Lang-01.5.1.1.2.1.1.1.1.1.1.1.2 at Git
`9a5d30d25620541c3f2975d31e04e04e3bc9514c`. It replaces the rejected
return's unsupported-family sketches with one constructible sema-owned checked
fact graph, a complete version-1 semantic transcript, and a private bounded
Maranget coverage engine.

The selected design is:

- extend HIR's existing declaration-rooted path authority for View values and
  expression-owned statement/pattern bodies;
- construct exact project-item, entry, case, field, look, modifier, rich-text,
  statement, and body semantic atoms in the checker that already resolves
  them;
- transcribe every current expression, value, select, pattern, statement, and
  executable-body family without a spelling/raw-ID fallback;
- replace finite atom coverage with one private checked-`u64` pattern matrix
  over closed, product, symbolic sequence, Choice, Never, and open/Other
  domains; and
- delete old unsupported and basic-coverage paths as their replacements become
  constructible in the compile-clean order.

This does not change production code. It does not define an external return
wire, persisted Match DTO, runtime carrier, task-plan seal, View admission,
scheduler/snapshot contract, whole-catalog seal, legacy reader, or version
other than `1`.

## Reading order

1. `SOURCE_EVIDENCE.md` — pinned source and complete live inventories.
2. `FINAL_DESIGN.md` — final ownership and publication flow.
3. `SCHEMAS.md` and `schemas/final_contract.rs` — constructible checked rows.
4. `TRANSCRIPT_GRAMMAR.md` — exact semantic bytes for all families.
5. `COVERAGE_ALGORITHM.md` — private matrix/usefulness/witness algorithm.
6. `DEPENDENCIES.md` — owner/consumer/dependency matrices.
7. `CUTS_TESTS_AND_DELETION.md` — compile-clean implementation and tests.
8. `DECISION_REGISTER.md` — unique decisions 1–7 traceability.
9. `machine/final_contract.json` — validator authority.

`REQUEST.md` is the byte-identical maintained request mirror.
`FINAL_STATUS.md` and `OPEN_QUESTIONS.md` are machine-gated terminal files.

## Acceptance rule

Prose is not a gate by itself. The repository-aware Rust validator checks the
pinned Git and request bytes, structured inventory/decisions/non-goals,
required anchors, manifest, status, and every negative mutation. Production
implementation acceptance additionally requires all compile, behavior,
differential, perturbation, and deletion gates in
`CUTS_TESTS_AND_DELETION.md`.
