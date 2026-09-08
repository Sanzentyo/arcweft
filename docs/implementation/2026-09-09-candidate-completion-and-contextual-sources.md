# Candidate completion and contextual source constraints — 2026-09-09

Status: IN_PROGRESS. Supersedes the current-state validation in
[contextual effect hints](2026-09-09-contextual-effect-hints.md), which remains
historical evidence. The [full convergence goal](2026-09-08-convergence-goal-plan.md)
and its acceptance criteria are unchanged.

Inspected accepted Git SHA:
`4da7bfd7a9f8324f3763d9e579fce1f0ab38965d`, existing `main` checkout.
Inherited dirty changes are preserved: 8 deleted / 646 modified / 96 untracked
entries after adding this record and its audit directory; the index is empty.
This follow-up does not commit an incomplete callable implementation.

## Established changes

Required type/constant binding validation now belongs to the constraint context
and is shared by candidate completion and completed-solution construction.
Candidate completion invokes it after source materialization and candidate
uniqueness, before final keyed projections. An absent required parameter is an
ordinary `IncompleteInstantiation` rejection, not a fabricated projection
invariant. The earlier source-fatal ordering, callback cleanup, work accounting
and ambiguity behavior remain necessary and are covered by existing tests.

Final projection invariants now retain their exact `TypeConstraintRejection`
cause. A deliberately closed projection of a future-eligible reference remains
an invariant, while its missing parameter is preserved. Actual/evidence
projection disagreements retain a typed mismatch. No cause-free replacement,
source string matching or error suppression was introduced.

The new lower regression covers both an absent type key and an absent constant
key, including the declaration identity in the ordinary rejection. Three sema
regressions exercise contextual constructor calls; two reproduce already-failing
native/AWBC cases, and one adds complementary evidence from two arguments.
The compiler matrix gains the corresponding complementary-evidence pair.

## Source boundary now identified

The current call constraint producer classifies result-only generic parameters
as bindable at the terminal group, correctly. However, a nested constructor is
required to close that group before its parent has solved all argument
constraints. These ordinary programs demonstrate the missing boundary:

```arcw
enum Slot<T> { Empty, Full T }
fn fallback<T>(input: Slot<T>, value: T) -> T { value }
flow main() -> i64 { return fallback(.Empty(), 42i64) }
```

```arcw
enum Either<A, B> { Left A, Right B }
fn combine<A, B>(left: Either<A, B>, right: Either<A, B>) -> i64 { 42i64 }
flow main() -> i64 { return combine(.Left(1i64), .Right("two")) }
```

The first constructor lacks `T`; a `Left` constructor lacks the parameter used
only by `Right`. Before the completion-order fix, these failures were collapsed
into `Projection(Mismatch)`. The retained cause identifies the exact unbound
child inference slot. After the fix, the child is rejected ordinarily, exposing
the next defect: `publish_recovery_call` provides its primary schema's result
type as a value. That type contains declaration-Free nominal parameters, which
are not authorized in the parent's inference scope. Passing it as accepted
source evidence produces `TypeParameterOutOfScope`.

The complementing pair prevents narrowing the repair to argument reordering:
each constructor determines one variable and needs the other. Retrying whole
arguments only after another whole argument succeeds cannot exchange that
partial evidence. The sema regression checks both argument orders and, once
accepted, requires both constructor owners to have `[i64, String]` arguments.
The compiler pair requires actual native and AWBC returns of `42`.

The final source contract must connect partial source constraints, nested
application scopes and parent completion without admitting a recovery result as
runtime value evidence. It must preserve correlation, callback cleanup,
source-fatal precedence, cancellation and work limits, replay, exactly-once
materialization of each closed source trace, complete nominal owner identities, and runtime evaluation
order. A deferred whole-source queue alone, allowing every future projection,
inventing Free aliases for Inference references, or special-casing a unit case
does not satisfy this boundary.

No new scheduling, cross-scope alias or effect-row algebra is declared accepted
by this investigation. The coupled design remains in
[AW-AH-009.4.2.1.1.1](../reviews/requests/2026-09-08-aw-ah-009.4.2.1.1.1-function-scheme-specialization-and-callable-value-execution.md).
The four inferred-callback effect failures and compiler source-effect projection
gap from the preceding record also remain. There is no external blocker and no
change to the full goal, contract version `1`, or layer/Sans-I/O rules.

## Validation

| Command / scope | Actual result |
| --- | --- |
| `cargo test -p arcweft-lang-sema --lib final_analysis::tests::generic_calls -- --nocapture` | Failed: 8 passed / 2 failed at the initial constructor investigation checkpoint, before adding the complementary-source test. The typed errors identify missing child inference slots. |
| `cargo test -p arcweft-lang-sema --lib -- --nocapture` | Latest run failed: 742 passed / 7 failed / 749 total / 0 ignored. The previous 741 passing tests remain passing, and the new required-key regression passes. Three new constructor regressions and the four previous effect regressions fail. |
| `cargo test -p arcweft-compiler --lib --test callable_execution --test evaluated_effects --test project_function_instances --test try_pipe --no-fail-fast -- --nocapture` | Compiler library passed 67/67; `callable_execution` failed 43 passed / 16 failed / 59 total; related integrations passed 9 + 6 + 8 tests. `--no-fail-fast` kept all requested targets running. |
| `cargo check --workspace --all-targets --all-features` | Passed, exit 0. |
| `cargo clippy --workspace --all-targets --all-features` | Passed with warnings, exit 0. |
| `cargo fmt --all`, then `cargo fmt --all -- --check` | Passed. |
| `git diff --check` | Passed, including the documentation changes. |
| Maintained documentation link check | Passed: 3 documents, 36 relative targets, zero missing files. Anchors were not checked. |
| `cargo +nightly -Zscript tools/structure-audit.rs --root . --write docs/implementation/structure-audits/2026-09-09-candidate-source-completion --fail-on-blocking` | Passed: 95 packages, 2,228 Rust files, 310 review triggers, zero blocking violations. |

The compiler integration total is 66 passed / 16 failed. The new complementary
constructor pair accounts for the two additional failures; the preceding seven
paired failure families remain. The two earlier constructor pairs now expose
the invalid recovery source scope rather than hiding the missing child key
behind an unexplained projection mismatch. This is diagnostic progress, not
acceptance of those programs.

An intermediate placement of completion validation before materialization
failed 735 passed / 12 failed. It incorrectly skipped six existing checks of
materialization, fatal ordering, limits and ambiguity. The guard was moved to
the final projection boundary; those six tests pass in the final run. Their
fixtures and expectations were not weakened. Intermediate evidence remains in
`target/candidate-completion-before-materialization.log`.

Final logs: `target/candidate-completion-before-projection.log` and
`target/candidate-source-completion-{compiler,workspace-check,workspace-clippy,structure-audit,fmt}.log`.
The initial constructor probe log is `target/contextual-constructor-parent-scope.log`.
Clippy reports sema library/test-library summaries of 1,205/1,397 warnings
(1,203 duplicates), compiler 222/226 (219 duplicates), plus other workspace
and integration warnings. No warning-free result is claimed.

Full workspace tests, doctests, exhaustive codec/golden and Tier 2 were not run.
They remain required for the connected main push cut. No branch/worktree,
checkout switch, reset, speculative push or unrelated cleanup occurred.
The scope archive remains 45,039 bytes with SHA-256
`6c6d2f0885a7f9e76e815e09be397556bac99b3097d290255eee66f75ad32185`.
Frozen archives and extracted mirrors were not edited. Remaining Match, View,
RuntimePlan/task-plan, nominal and scheduler/restore work is not waived.

## Structural review

The [findings](structure-audits/2026-09-09-candidate-source-completion/findings.md),
[file measurements](structure-audits/2026-09-09-candidate-source-completion/file_metrics.csv)
and [dependency graph](structure-audits/2026-09-09-candidate-source-completion/package_metrics.csv)
are generated from the current checkout. HEAD-to-current growth includes
inherited changes, not only this follow-up.

| Owner / file | HEAD → current physical LOC | Current bytes |
| --- | ---: | ---: |
| sema `types/constraints/context.rs` | 1,282 → 1,684 | 61,978 |
| sema `types/constraints/solution.rs` | 959 → 1,037 | 39,856 |
| sema `types/constraints/transaction.rs` | 1,957 → 1,780 | 66,056 |
| sema `types/constraints/tests.rs` | 3,868 → 4,120 | 144,525 |
| sema `final_analysis/analyzer/calls/constraints.rs` | 4,241 → 4,919 | 200,990 |
| sema `final_analysis/tests/generic_calls.rs` | existing untracked → 331 | 10,647 |
| compiler `tests/callable_execution.rs` | existing untracked → 487 | 12,345 |

The lower context owns generic eligibility and the mapping between declaration
keys and active inference references. Required-key validation therefore reads
that context, replacing the two copied loops in solution construction. The
transaction owns phase order; it invokes the same validation without copying
binding maps or constructing another solution. Projection errors own their
typed lower cause. No new catalog, schema projection, HIR walk, transport,
persistence, I/O, public facade or dependency edge was introduced.

The large analyzer constraint module remains the adapter between typed source
evidence, affine callback cleanup and the lower driver. This follow-up changes
its mismatch payload only. Its 480 embedded test LOC and the solution module's
67 embedded test LOC remain attached to their existing responsibilities; new
behavioral coverage is in the dedicated constraint/generic-call test modules.
These cohesive owners remain above review thresholds. Splitting their phase
authority into independent readers to meet a LOC target would lose the
invariants being repaired. Sema fan-in/out remains 8/14 (development 3/0),
compiler 3/23 (development 1/5).
