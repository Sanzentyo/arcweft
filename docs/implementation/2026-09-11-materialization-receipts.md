# Constraint materialization receipt completion

Date: 2026-09-11. Inspected `main` at
`fdad7ab45fdb3098b7049ba719e8334d70b45772`, matching fetched `origin/main`.
This record covers the independent materialization protocol correction in
`types/constraints/transaction.rs` and its existing child test module. The
broader [convergence goal](2026-09-08-convergence-goal-plan.md) and
[correlated-call integration](2026-09-10-callable-convergence-model.md) remain
in progress.

## Defect and final behavior

The transaction previously removed the next materialization ticket from its
queue before checking whether the preceding ticket had been submitted. An
invalid advance therefore lost a candidate. If that queue was already empty,
the same operation reported that materialization was finished even while the
last ticket remained outstanding.

There was a second publication defect: after one alternative had completed,
`finish` could select it while another issued ticket had not been submitted.
That unresolved alternative could still have changed rejection, ambiguity or
the selected result.

The existing transaction now checks its active receipt before dequeuing,
including when the queue is empty. Completion uses the same close-and-select
route and rejects an outstanding receipt before examining completed
alternatives. Unissued queued work continues to prevent completion. The
transaction retains its existing issuer-qualified affine ticket authority;
the fix adds no parallel state machine, public API or compatibility path.

Two behavioral regressions cover advancement with both a queued successor and
the final outstanding ticket, successful submission of every retained path,
and refusal to publish an earlier completed alternative prematurely. They use
real lower transactions and callback-bound submissions.

## Exact validation scope

The unrelated in-progress scope and source-frontier changes were preserved as
raw Git blobs with a SHA-256/size manifest for all 29 dirty or untracked files.
Only this correction and its tests were staged, and the cached diff was
reviewed. A checked inverse patch temporarily removed the remaining tracked
work in the existing `main` checkout so the compiled Rust matched the index.
Untracked scope files remained present but were not referenced by its module
graph. No branch, worktree, additional checkout, stash, reset or deletion was
used.

Local receipts and restoration evidence are in
`.arcweft-local/validation/2026-09-11-materialization-receipts/`.

| Performed validation | Result |
| --- | --- |
| `cargo fmt --all --check` | Passed |
| `cargo test -p arcweft-lang-sema --all-features --lib` | **Failed:** 780 passed, 9 existing failures |
| Lower constraint cases in that run | 118 passed, including both new regressions |
| Callable driver cases in that run | 16 passed |
| Sema failure-name comparison with the inspected base's prior run | Exact match; no additions or removals |
| `cargo check --workspace --all-targets --all-features` | Passed with warnings |
| `cargo clippy --workspace --all-targets --all-features` | Passed with warnings |
| `just test-workspace` | **Failed:** compiler `callable_execution`, 57 passed and 24 existing failures |
| Compiler failure-name comparison with the inspected base's prior run | Exact match; no additions or removals |
| `just test-doc` | 8 passed |
| Canonical structure audit and `just structure-audit-gate` | Passed; 95 packages, 2,273 Rust files, 310 review triggers, 0 blocking violations |
| Cached/working diff checks, validated Rust blob identity and WIP restoration hashes | Passed |

All 29 saved files were restored and matched their original raw hashes and
sizes. The staged Rust blobs still matched the exact validated index tree.
The physical structure scanner includes the three preserved, unreferenced
scope Rust files; the retained changed-file measurements cover only this cut.

The nine sema failures remain the five contextual/correlated generic-call cases
and four inferred callback-effect cases. Their required positive expectations
are unchanged. The focused working-copy run before isolation passed 128 lower
cases; it also included ten unfinished scope/source-frontier cases and is
separate from the 118-case exact-cut evidence above. Its first materialization
test attempt failed because a test import was missing; that was corrected
before the passing run.

The workspace recipe stopped at the compiler failures; later targets and
subsequent CLI recipe commands were not run. Focused sema and workspace
check/Clippy use all features; the canonical workspace/doctest recipes also
exercise their default-feature combination. Tier 2 is not applicable to this
private sema protocol correction: no Agent/MCP, I/O, capture, native/render
behavior or runtime contract changed. `just verify` and generated JLREQ checks
were not selected for this narrow correction. No clean or disk failure occurred
during this validation.

## Ownership and remaining scope

`TypeConstraintTransaction` owns candidate frontiers, source obligations,
materialization receipts and completion. The changed transitions belong to
that same lifetime; moving them into a separate owner would split the receipt
from the queue and publication decision it protects. The tests remain in the
existing child module, exercising private transitions without widening APIs.
No dependency, wire format, runtime type, language syntax or version marker
changes.

Generated [changed-file measurements](structure-audits/2026-09-11-materialization-receipts/changed-files.csv)
record the production transaction at 1,844 physical LOC, 68,293 bytes, growth
of 2 lines and no embedded test LOC. Its child test module is 345 LOC and
12,352 bytes, growth of 86 lines. The production size trigger is retained with
the cohesion justification above: queue issuance and candidate publication
must consult the same transaction receipt. State, dependency, API and test
boundaries remain unchanged; this cut adds no unrelated responsibility or
duplicate solver. The owning sema crate has workspace fan-in/out of 8/14 and
development fan-in/out of 3/0.

This correction does not provide a child-application producer, whole-component
ranking or closure, function-scheme execution, inferred callback effects, or
native/AWBC runtime integration. Those obligations remain in the
[coupled callable request](../reviews/requests/2026-09-08-aw-ah-009.4.2.1.1.1-function-scheme-specialization-and-callable-value-execution.md).
No design deviation or external blocker was introduced.
