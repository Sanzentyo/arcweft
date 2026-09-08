# Saved-prefix reapplication — 2026-09-08

Supersedes the immediate shared-prefix failure status in
[function-scheme inventory](2026-09-08-function-scheme-inventory.md).
The earlier validation remains evidence of its own checkpoint. The
[convergence goal](2026-09-08-convergence-goal-plan.md) remains active.

Inspected `D:/git/arcweft` on `main`; HEAD and `origin/main` both remain
`4da7bfd7a9f8324f3763d9e579fce1f0ab38965d`. The index is empty. This
follow-up began with 8 deleted / 642 modified / 68 untracked status entries;
the retained audit adds one untracked directory entry and this record adds
one file. Inherited changes were preserved. No branch, worktree, commit, or
push was made.
Final status after the record was added: 8 deleted / 642 modified / 70
untracked entries, still on `main` with an empty index.

## Implemented boundary

The shared-prefix execution failure first originated in semantic rejection.
The callee constraint compared the saved quantified continuation type with
the declaration's remaining uninstantiated function template. That compared
different binding scopes and rejected a valid later application before
runtime reachability.

`PreparedResolvedCallable::prepare_function_value_constraint` now owns the
distinction already present in its Base/PreparedContinuation state. It checks
the exact group and, for a continuation, the retained function type. The
existing graph checks ancestry, and the lower driver reopens the inherited
solution in the new application scope. This path no longer adds a second
structural constraint between the saved scheme and its declaration template.
Independent function values retain their source-effect constraint. The old
remaining-function effect-projection token variant and issuer were deleted.

Two regression tests exercise independent uses of a saved two-group prefix
and reuse at multiple positions in a three-group application. They assert
that every application is selected, its expression retains the exact
application digest, and the terminal results remain String and i64.
No new declaration IDs, inference issuer, substitution history, runtime
fallback, or language restriction was introduced.

## Validation actually performed

- `cargo test -p arcweft-lang-sema --lib
  a_shared_generic_prefix_retains_each_accepted_call_execution -- --nocapture`:
  initially **FAILED** on an unselected later call, then **PASSED**, one test.
  Log: `target/shared-prefix-sema-probe.log` retains the successful rerun.
- `cargo test -p arcweft-lang-sema --lib
  final_analysis::tests::generic_calls:: -- --nocapture`: **PASSED**, six tests.
  Log: `target/shared-prefix-generic-tests.log`.
- `cargo test -p arcweft-lang-sema --lib
  final_analysis::analyzer::semantic_fact_transaction_tests:: -- --nocapture`:
  **PASSED**, 20 tests, including rollback, stale continuation rejection,
  alias/capture origins, and multi-group dependencies. Log:
  `target/shared-prefix-transaction-tests.log`. The last two selections cover
  **26 distinct tests**; the single-test rerun is included in those six.
- `cargo test -p arcweft-compiler --test callable_execution
  shared_prefix_with_distinct_later_types -- --nocapture`: **FAILED**, both
  engines now reach runtime semantic projection and report a binder outside
  its lexical scope. Log: `target/shared-prefix-execution-probe.log`.
- `cargo test -p arcweft-compiler --test callable_execution -- --nocapture`:
  **FAILED**, 20 passed / 12 failed. Log:
  `target/saved-prefix-callable-execution.log`. The six failing cases still
  fail in both engines. Shared-prefix reapplication has moved past semantic
  rejection; the monomorphic-callback use still has a structural call
  projection. The other failures remain contextual variant completion,
  unknown callback effects, closure ProjectCall lowering, and ordinary apply
  of a project-continuation value.
- `cargo clippy -p arcweft-lang-sema --all-targets --message-format=short`:
  **completed with exit 0 and warnings**, not a clean lint gate. Cargo reports
  1,206 sema library warnings and 1,387 library-test warnings, of which 1,205
  are duplicates; dependencies also report warnings. Log:
  `target/saved-prefix-sema-clippy.log`. No automatic fixes or suppressions
  were applied.
- `cargo fmt -p arcweft-lang-sema`, `cargo fmt --all -- --check`, and
  `git diff --check`: **PASSED**. Local Markdown target inspection across this
  record, the preceding inventory record, and the correction request found
  21 targets and zero missing files. Anchor fragments were not checked.
- `cargo +nightly -Zscript tools/structure-audit.rs --root . --write
  docs/implementation/structure-audits/2026-09-08-prefix-reapplication
  --fail-on-blocking`: **PASSED**, 95 packages, 2,218 Rust files, 311 review
  triggers, zero blocking violations. Log:
  `target/prefix-reapplication-audit.log`. No test commands ran concurrently
  and no Cargo job count was set.
- Full sema/workspace tests, workspace all-target/all-feature check,
  workspace Clippy, doctests, codec/golden, and Tier 2: **NOT RUN** in this
  follow-up. They remain required for the connected cut.

## Structure and remaining work

Current generated [file metrics](structure-audits/2026-09-08-prefix-reapplication/file_metrics.csv)
and [findings](structure-audits/2026-09-08-prefix-reapplication/findings.md)
cover the full checkout. The touched production owners are:

| Owner under `crates/arcweft-lang-sema/src/` | HEAD LOC | Current LOC | Bytes | Embedded test LOC | Disposition |
| --- | ---: | ---: | ---: | ---: | --- |
| `callable/resolver/outcome.rs` | 1,490 | 1,534 | 56,414 | 0 | Existing prepared callable state owns callee validation and source-effect preparation |
| `final_analysis/analyzer/calls/constraints.rs` | 4,241 | 4,940 | 201,892 | 476 | Existing candidate planner consumes the owner's constraint decision; no second resolver or scope model |

The generic-call regression module is 140 LOC / 3,940 bytes. Growth relative
to HEAD includes inherited work. Dependencies, features, public exports,
transport, persistence, and I/O ownership did not change. Tests follow the
prepared-application and frozen-scope behavior; physical splitting would not
resolve a separate state owner in this change.

Runtime function-scheme representation, monomorphic specialization evidence,
ordinary callable execution, effects, contextual constructor ownership, and
suspension/restore remain governed by the
[active correction request](../reviews/requests/2026-09-08-aw-ah-009.4.2.1.1.1-function-scheme-specialization-and-callable-value-execution.md).
Inspection also located immediate checked-variant construction in contextual
short-variant resolution, which must be reconciled with candidate-owned active
types. This follow-up does not claim that boundary is fixed.
Independent function-value preparation still needs examination of nonempty
scheme binders throughout its assembled source and projected function types;
the saved-continuation fix does not establish that separate base-value path.

No design archive was accepted and no stable language rule changed. Later
Match/View/RuntimePlan/nominal/scheduler stages, required broad validation,
and main commit/push remain open. There is no external blocker.

[Contextual variant owner preparation](2026-09-08-contextual-variant-owner.md)
records the subsequent source-phase migration, all 728 sema unit tests, and
the expanded native/AWBC matrix. The payload-constructor and callable
obligations remain open.
