# Callable execution and instance discovery continuation — 2026-09-08

Status: IN_PROGRESS. This is working-copy evidence for the active
[convergence goal](2026-09-08-convergence-goal-plan.md), not a completed
Content/Fx/callable cut or an accepted implementation commit.

The subsequent [generic scope migration](2026-09-08-generic-scope-migration.md)
changes the semantic APIs and currently does not compile. The validation
results below describe the earlier working-copy state, not the current tree.

Inspected Git HEAD and accepted `origin/main`:
`4da7bfd7a9f8324f3763d9e579fce1f0ab38965d`.

The continuation began on the existing `main` checkout with 658 modified,
deleted, or untracked paths and an empty index. Those inherited changes remain.
The implementation changes described here have not been staged, committed, or
pushed. They remain connected to the unfinished callable migration.

Supersedes the runtime execution assessment in
[the September 7 continuation](2026-09-07-content-callable-continuation.md)
only for the newly executed tests below. Earlier validation counts remain
historical; this note does not replace the complete goal or its acceptance
criteria.

## Performed implementation

### Instance discovery and publication

`crates/arcweft-compiler/src/lower/project_instances.rs` owns one discovery
session, a canonical pending-key queue, affine work items, and the immutable
discovered graph. The existing dependency discovery enters this owner through
`ProjectInstanceProjection`.

- A new closed instance is queued once. Repeated calls and recursive edges
  reuse its existing key.
- Both inter-function discovery and the nested executable/closure partition
  walk use explicit ordered queues instead of host recursion.
- Removing a pending key does not prove that its dependencies were inspected.
  The session retains an active item until its exact work permission is
  consumed. Dropped, stale, or foreign work cannot seal the graph.
- Sealing consumes discovery. Dialogue projection reads the complete graph;
  materialization may reference only already discovered targets.
- Runtime function facts are returned only after every materialization
  succeeds. The old mixed `Discovering`/`Discovered`/`Complete` map and its
  mutation during materialization were removed.

This establishes discovery/publication phase separation. It does **not** yet
establish the production instance/edge/type-node/depth/work limits, cancellation
accounting, the full dependency transcript, or a flat scoped generic
substitution. Those remain mandatory work in this goal.

### Conditional flow-value evaluation

`crates/arcweft-runtime-plan/src/final_flow/value_branches.rs` now owns the
flow-value continuation for `if`, `if let`, `match`, and boolean short-circuit
operators. The selector is evaluated first; each branch retains its own body
operations and outer continuation. An already evaluated expression override
is consumed as a value before lowering its source again.

The previous generic child walk evaluated calls in unselected branches. The
new non-generic mutual-recursion test exposed this concretely: `is_even(4)`
kept invoking the recursive branch after reaching zero. Both native and AWBC
execution exceeded the test's deterministic step limit. After this correction,
both return `true`.

Additional behavioral tests cover skipped recursive calls under `&&`, `||`,
`if let`, and `match`, along with calls in selected branches and nested generic
calls. These tests compare returned values through the actual Engine and
verified AWBC paths; they do not evaluate removed PureHelper bodies.

Flow execution inside pattern guards is still not covered by this correction:
the existing guard expression boundary remains and must be reconciled with
the callable/Match execution model. This is unfinished implementation, not a
new language restriction.

## Reproduced remaining failures

`crates/arcweft-compiler/tests/callable_execution.rs` retains both native and
AWBC tests for each source program. The final run contains 28 tests:
**16 passed, 12 failed, 0 ignored**.

| Source behavior | Current failure in both test routes |
| --- | --- |
| A generic function calls itself with its caller-owned `T` | Semantic call-constraint failure; the declaration-ID rigid/bindable collision remains |
| One `choose<A, B>(A)(B)` prefix is reused with String and i64 later arguments | Runtime projection has no representation for the remaining generic `B` |
| That generic prefix is passed to a monomorphic callback | Semantic call-constraint failure |
| A fully monomorphic curried prefix is passed to an explicitly pure callback | Native expects Function and receives ProjectContinuation; AWBC traps with the same mismatch |
| An explicitly pure closure callback has a body that calls a project function | Global closure lowering still attempts expression-only projection and lacks the flow-owned call projection |
| A callback function type omits its effect annotation | Final semantic sealing panics on `EffectRowError::UnknownRow` |

The effect-omission case also needs its intended acceptance rule reconciled;
the panic is an observed defect, not evidence that effect inference has
already been designed or implemented. The source matrix keeps this case
visible while that decision is closed.

These failures substantiate the goal's requirement to reassess callable values
and their execution consumers together. In particular, restricting a
polymorphic prefix does not fix the already failing monomorphic prefix
callback. No frozen review package was edited and no compatibility reader,
runtime type fallback, or synthetic wrapper function was introduced.

## Validation actually run

| Command / check | Result |
| --- | --- |
| `cargo test -p arcweft-compiler --test callable_execution` | FAILED: 16 passed, 12 failed; full output in `target/callable-execution-2026-09-08.log` |
| `cargo test -p arcweft-compiler --test project_function_instances` | PASSED: 6 tests after instance discovery migration |
| `cargo test -p arcweft-compiler --lib lower::project_instances::tests` | PASSED: 5 tests for pending/in-flight work, foreign work, repeated/recursive requests, and sealed membership |
| `cargo test -p arcweft-compiler --lib` | PASSED: 67 tests after the branch correction, including the 5 discovery tests |
| `cargo test -p arcweft-runtime-plan --lib` | PASSED: 58 tests after the branch correction |
| `cargo check -p arcweft-compiler --tests` | PASSED during discovery migration; later compilation was performed by the test and Clippy commands |
| `cargo clippy -p arcweft-compiler -p arcweft-runtime-plan --all-targets --all-features` | PASSED with warnings; output in `target/callable-clippy-2026-09-08.log` |
| `cargo fmt --all -- --check` | PASSED after implementation formatting |
| Structure audit with `--fail-on-blocking` | PASSED: 95 packages, 2333 files, 2205 Rust files, 309 review triggers, 0 blocking violations |
| `git diff --check` for the modified tracked Rust owners | PASSED |

The initial mutual-recursion run failed in both engines before the branch
correction; its final two tests passed. Initial test-harness Flow ID selection
mistakes were corrected by selecting the fixture's sole admitted flow. A
missing module-path attribute briefly failed compilation and was fixed before
the successful validation above.

No Cargo job count was overridden. Full workspace checks, `test-workspace`,
doctests, Tier 2, the complete callable/Content/Fx codec and restore matrix,
and final commit/push are **not run in this continuation**. They remain due at
the complete implementation cut. The Clippy run includes existing warnings;
it is not a warning-free claim.

## Structural review

The large-owner triggers for compiler `lower.rs` and runtime-plan
`final_flow.rs` were reviewed. Their shared responsibility is still the
accepted-generation projection and flow-value lowering respectively.

- Discovery state, its publication transition, and negative protocol tests
  now reside under `lower/project_instances/`. The compiler facade consumes
  a sealed graph and retains the accepted HIR/sema dependency-inversion work.
- Conditional value evaluation now resides under
  `final_flow/value_branches.rs`; it reuses the existing flow operation and
  continuation vocabulary. It does not introduce a second runtime evaluator.
- The work permission is deliberately passed by value and cannot be cloned.
  Its Clippy expectation documents affine consumption. The conditional
  selector/body match remains cohesive in one function with a documented
  line-count expectation.

The generated structure report is in
`target/callable-execution-2026-09-08/`. Its counts precede the final
comment-only lint explanations and this evidence note. No LOC change is used
as acceptance evidence.

## Next mandatory work

Continue the active goal from the failures above: finish the generic
reference/scope model and flat closed substitution; reconcile ordinary
function values, partial values, callback execution and effect rows across
sema/compiler/native/AWBC; migrate the remaining expression-only closure and
guard consumers; complete bounded discovery and cancellation; migrate stale
PureHelper integration tests to actual execution; then complete the connected
Match, View, task-plan, nominal and scheduler/restore work in the goal plan.

There is no external blocker recorded in this continuation. The remaining
items are unfinished required work, and the goal remains active.
