# Contextual effect hints and source contract evidence — 2026-09-09

Supersedes the current-state conclusions in
[higher-order effect ownership](2026-09-08-higher-order-effect-ownership.md).
That note remains historical validation evidence. This follow-up continues the
[active convergence goal](2026-09-08-convergence-goal-plan.md); it does not
complete the callable cut or any downstream goal obligation.

Inspected accepted Git commit:
`4da7bfd7a9f8324f3763d9e579fce1f0ab38965d`, existing `main` checkout.
The working tree contains inherited and continuing implementation changes:
8 deleted / 646 modified / 94 untracked entries after adding this record and
its audit directory; the index is empty. A fresh fetch found HEAD/origin/main
divergence 0/0. No implementation commit or push occurred in this follow-up.

## Established behavior

The pending checked-callable catalog now exposes an authored effect bound
before body inference completes. Unbounded bodies expose their inferred row
only when it exists. Source call preparation uses this catalog authority;
the old helpers that substituted a closed empty row for unavailable project
effects were deleted. Creating a curried prefix still has an empty invocation
row because that application does not enter the function body.

Constraint projection already validated function effect rows against the
candidate scope, but omitted their variables from its remaining-parameter
inventory. Consequently, a callback expectation containing an open effect row
was classified as complete. Ordinary Boolean compatibility then rejected an
actual callback with a nonempty row before the parent solver could relate it.

The same projection now retains effect-variable identities alongside type
and constant references. The structural generic-use visitor inventories these
variables for exact child-expectation projection. A contextual hint preserves
its parametric status through explicit/implicit closures, blocks, conditionals,
loops, match/choice outputs, tuple/sequence elements, repeat/range elements,
pipe, try checks and postfix probes. Full compatibility and independent child
call result equations consume only a complete expectation. Authored complete
annotations continue to constrain values; global type compatibility was not
relaxed.

The existing nonempty callback tests now verify both current-group and retained
prefix ABI rows. An unused `fs.write` callback retains that row in its value
type while the enclosing `ignore` invocation remains effect-free. A new source
test checks contextual unannotated closures across blocks, conditionals, tuples
and sequences, including the selected runtime parameter ABI.

Two diagnostics followed from making source effects available earlier:

- Effectful compile-time defaults now fail the existing expression-effect
  check before scalar reduction. The test expects that existing typed cause.
- Rejected method calls no longer lose their call owner during method-selection
  enrichment. An authored missing selection reports `CallResolutionFailed`
  at the call, while malformed join evidence retains its structured error.
  Both invalid return type and nonempty handler effects remain rejected.

## Remaining authority gaps

Four sema regressions still fail with `CallSeal` / `Instantiation` /
`Effect(UnknownRow)`: invoking a callback with omitted effects, joining two
independent invoked callbacks, leaving a second callback latent, and giving
one declaration separate pure/nonempty applications.

The constraint hint inventory uses the effect identities issued by the existing
effect constraint owner. This is not completion of semantic Free/Bound/Inference
effect references or of function-scheme effect algebra. Unknown body rows still
need dependency evidence; taking their concrete component cannot establish a
final closed row. No claim is made that the source/catalog/solution mismatch is
fully repaired for inferred bodies.

Compiler instance materialization still reads source parameter, binding, local,
pattern and type facts through `CheckedProjectFunctionInstanceSolution`.
Source annotations can retain `Unknown` effect rows even when selection has
closed the parameter ABI. The missing source identity must be fixed at its
semantic owner and carried through all these consumers. Replacing only the
compiler parameter loop, defaulting unknown rows to purity, or matching source
shapes to copied ABI tables would leave competing type authorities.

The complete scheme, pending-body effect, instantiation and runtime-value
decisions remain in
[AW-AH-009.4.2.1.1.1](../reviews/requests/2026-09-08-aw-ah-009.4.2.1.1.1-function-scheme-specialization-and-callable-value-execution.md).
There is no new external blocker and no new compatibility/version exception.
This follow-up changes no maintained language rule or frozen review archive.

## Validation

| Command / scope | Actual result |
| --- | --- |
| `cargo test -p arcweft-lang-sema --lib higher_order_effects -- --nocapture` | Failed: 3 passed / 4 failed / 737 filtered out at the first hint-repair checkpoint, before adding the propagation test. |
| `cargo test -p arcweft-lang-sema --lib -- --nocapture` | Failed: latest run 741 passed / 4 failed / 745 total / 0 ignored. All remaining failures are the inferred invocation rows listed above. |
| `cargo test -p arcweft-compiler --lib --test callable_execution --test evaluated_effects --test project_function_instances --test try_pipe -- --nocapture` | Compiler library passed: 67/67. `callable_execution` failed: 43 passed / 14 failed / 57 total. Cargo stopped before the three subsequent integration targets. |
| `cargo test -p arcweft-compiler --test evaluated_effects --test project_function_instances --test try_pipe --no-fail-fast -- --nocapture` | Passed: 9 + 6 + 8 tests, 23 total. |
| `cargo check --workspace --all-targets --all-features` | Passed, exit 0. |
| `cargo clippy --workspace --all-targets --all-features` | Passed with warnings, exit 0. |
| `cargo fmt --all`, followed by `cargo fmt --all -- --check` | Passed. |
| `git diff --check` | Passed, including the documentation changes. |
| Maintained documentation link check | Passed: 3 documents, 34 relative targets, zero missing files. Anchors were not checked. |
| `cargo +nightly -Zscript tools/structure-audit.rs --root . --write docs/implementation/structure-audits/2026-09-09-contextual-effect-hints --fail-on-blocking` | Passed: 95 packages, 2,228 Rust files, 310 review triggers, zero blocking violations. |

The integration matrix remains 66 passed / 14 failed in total. It reproduces
the same seven paired failure families as the preceding record. In particular,
the unused-callback pair still fails runtime type projection with an unknown
row despite its now-correct selected sema ABI. The sema improvement is not an
execution success. No test was ignored or deleted to remove these failures.

Clippy reports sema library/test-library summaries of 1,205/1,397 warnings
(1,203 duplicates), and compiler summaries of 222/226 (219 duplicates), plus
other workspace and integration warnings. No warning-free result is claimed.

Logs are under `target/contextual-effect-hints-{focused,sema,compiler,related-tests,workspace-check,workspace-clippy,structure-audit,fmt}.log`.
The initial source-contract change was also tested before hint repair:
`target/source-callable-effect-authority-sema.log` records 736 passed / 8 failed.
Intermediate failures are historical checkpoints; only the latest results in
the table describe the final source state in this follow-up.

Full workspace tests, doctests, exhaustive codec/golden and Tier 2 were not run.
They remain required for the connected main push cut. Generic Match, retained
View, RuntimePlan/task-plan, nominal C1–C6, scheduler/restore and the other
callable obligations remain in the goal. No branch/worktree, checkout switch,
reset, unrelated cleanup or partial implementation commit was used.

The retained scope archive is still 45,039 bytes with SHA-256
`6c6d2f0885a7f9e76e815e09be397556bac99b3097d290255eee66f75ad32185`.
The active request was updated to distinguish repaired source/ABI and closure
substrate from remaining effect and callable-value obligations.

## Structural review

The [findings](structure-audits/2026-09-09-contextual-effect-hints/findings.md),
[file measurements](structure-audits/2026-09-09-contextual-effect-hints/file_metrics.csv)
and [dependency graph](structure-audits/2026-09-09-contextual-effect-hints/package_metrics.csv)
are generated from the current checkout. HEAD-to-current values include
inherited work; they are not additions made only in this follow-up.

| Sema owner / file | HEAD → current physical LOC | Current bytes |
| --- | ---: | ---: |
| `callable/checked_catalog.rs` | 1,898 → 2,657 | 97,639 |
| `final_analysis/analyzer/calls.rs` | 3,386 → 4,333 | 187,558 |
| `final_analysis/analyzer/expressions.rs` | 3,357 → 4,049 | 176,311 |
| `final_analysis/match_edges.rs` | 1,259 → 1,467 | 60,214 |
| `types/generic_use.rs` | 517 → 820 | 31,474 |
| `final_analysis/tests.rs` | 8,292 → 9,018 | 311,233 |
| `final_analysis/tests/higher_order_effects.rs` | new → 317 | 10,850 |

The callable catalog owns staged contract evidence; its new accessor reads
that existing state. Call preparation owns source use of the evidence.
Expression analysis owns contextual syntax and its candidate fact transaction;
this change threads the same expectation carrier through those producers.
The constraint projector validates effect scope and the existing structural
generic-use visitor supplies child-reference membership. These changes add no
second schema/catalog, HIR traversal, transport, persistence or I/O state.

Method-selection enrichment owns the association between a callee method fact
and its call join, so it is also the location that retains the rejected call
owner. Final-analysis error conversion preserves the underlying typed cause.
No new crate or facade export was required. These cohesive owners remain above
review thresholds; splitting their staged/checked views into independent
readers merely to reduce file size would break their shared invariants.

The generic visitor's HEAD-to-current growth exceeds 300 LOC, largely from
the existing scoped reference migration and tests. Its effect inventory uses
the same exhaustive type walk. Embedded test measurements are 185 LOC for
`calls.rs`, 88 for `expressions.rs`, and 293 for `generic_use.rs`; this follow-up
adds source behavioral coverage to the dedicated higher-order-effect test
module rather than adding another embedded production test body. The large
test root only adjusts an existing rejection expectation. Sema dependency
fan-in/out remains 8/14 (development 3/0), compiler 3/23 (development 1/5).
