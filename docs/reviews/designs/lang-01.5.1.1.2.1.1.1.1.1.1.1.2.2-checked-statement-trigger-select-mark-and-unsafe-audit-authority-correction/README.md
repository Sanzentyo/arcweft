# Checked statement, Trigger, Select, mark, and unsafe-audit authority correction

Status: `READY_FOR_IMPLEMENTATION`.

This checked-in directory is the maintained final contract for request
`2026-08-29-lang-01.5.1.1.2.1.1.1.1.1.1.1.2.2`. It is a design package, not
an implementation patch and not a returned ZIP. Repository policy makes Git
the authority for checked-in review designs, so no duplicate archive was
created.

## Reading order

1. `FINAL_DESIGN.md` — sole normative answer and precedence.
2. `HIR_AND_SEMA_SCHEMAS.md` — exact Rust-shaped schemas, visibility, tags,
   and invariants.
3. `SCRUTINEE_TYPE_SOURCES.md` — constructible type sources and the required
   Entry-root preparation order.
4. `MARK_COORDINATE_AND_TRANSCRIPT.md` — coordinate bytes and transcript
   grammar.
5. `OWNER_CONSUMER_MATRIX.md` — layer ownership, consumers, and deletions.
6. `IMPLEMENTATION_AND_DELETION_ORDER.md` — atomic implementation order.
7. `TEST_MATRIX.md` — positive, negative, perturbation, and all-35 gates.
8. `DECISION_REGISTER.md` and `SOURCE_EVIDENCE.md` — rationale and inspected
   repository facts.
9. `machine/final_contract.json` — closed machine mirror checked by
   `tools/validate_design.rs` and `tools/negative_self_tests.rs`.

`REQUEST.md` is a byte-identical mirror of the maintained request.
`FINAL_STATUS.md`, `OPEN_QUESTIONS.md`, `MANIFEST.txt`, and
`VALIDATION_REPORT.md` record package terminal state and validation.

## Inspected Git evidence

- checkout: `D:\git\arcweft`
- branch: `main`
- accepted `HEAD` and `origin/main` at inspection:
  `163a3b0da9fdcd5524ffeca8b055d774d53008e2`
- relevant evidence included the existing dirty evaluated-effect worktree cut;
  it was inspected but not modified by this design task
- no worktree, branch, workspace checkout, commit, push, Rust edit, Cargo edit,
  or unrelated-WIP edit was performed

The validator has two modes. Full mode checks the package and this inspected
repository baseline. `--design-only` checks package semantics, request bytes,
and the manifest without requiring the baseline source checkout.

```text
cargo +nightly -Zscript tools/validate_design.rs .
cargo +nightly -Zscript tools/negative_self_tests.rs .
```

All Arcweft-owned contract and transcript domains in this design remain
version `1`.
