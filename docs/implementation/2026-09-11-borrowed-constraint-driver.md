# Constraint driver and accounting context lifetime

Date: 2026-09-11. Inspected `main` at
`9bf29c04153ca6fe574378688e60e5d5e3a8ad1f` with unfinished correlated-call
changes present. This record covers the independent context-lifetime
correction. The [convergence goal](2026-09-08-convergence-goal-plan.md) and
[coupled callable model](2026-09-10-callable-convergence-model.md) remain open.

## Ownership change

`CandidateConstraintDriver` previously owned `TypeConstraintContext` and
transferred it into `TypeConstraintRun` when finishing. That tied the context's
accumulated bounds and accounting reservation to one driver. A nested driver
could not borrow that same context through its full lifetime.

The driver now borrows the context. `CandidateConstraintWorkSession::with_driver`
owns the context while lending the driver to its operation; the higher-ranked
loan cannot escape in the operation's result. The context retains its checked
node, branch and work counters and commits the existing accounting reservation
when released. Accounting remains idempotent, including a session abandoned
before it enters a context.

Finishing a lower transaction consumes its candidate state and returns the
owned solved result while borrowing the context. It does not consume or reset
the surrounding context. The old `TypeConstraintRun`, its separate `complete`
step and `commit_accounting` forwarding method were removed. The analyzer's
production driver entry and all lower/callable test consumers use the new
boundary. Callback checkpoint closure, rejection/fatal precedence and selected
result publication retain their existing owners.

The new behavioral test finishes or abandons one transaction, then attempts
another against the same one-branch context. The second admission must still
fail at branch 2; neither transaction may commit the surrounding reservation.
Releasing the context commits the accepted one-branch report exactly once.

This is the final context/accounting lifetime boundary. It does not yet add the
authenticated child contribution, per-application source/projection ownership,
correlated ranking or residual closure required by the complete callable model.

## Exact validation scope

Seven Rust files were explicitly staged and their cached diff reviewed.
The complete unfinished state of 29 dirty/untracked files was preserved as raw
Git blobs with SHA-256/size records. A checked inverse patch temporarily removed
the remaining tracked work in the existing `main` checkout so validation
compiled the staged Rust. The three untracked scope Rust files stayed present
but were not referenced by that module graph. No branch, worktree, extra
checkout, stash, reset or deletion was used.

Local receipts are under
`.arcweft-local/validation/2026-09-11-borrowed-constraint-driver/`.

| Performed validation | Result |
| --- | --- |
| Working-copy changed-crate check with all features and tests | Passed |
| Working-copy sema tests before the new lifetime case | **Failed:** 791 passed, 9 existing failures |
| Working-copy lower tests including the new lifetime case | 130 passed |
| Exact staged Rust: `cargo fmt --all --check` | Passed |
| Exact staged Rust: `cargo test -p arcweft-lang-sema --all-features --lib` | **Failed:** 781 passed, 9 existing failures |
| Lower cases and callable driver cases in that exact run | 119 and 16 passed, respectively |
| `cargo check --workspace --all-targets --all-features` | Passed with warnings |
| `cargo clippy --workspace --all-targets --all-features` | Passed with warnings |
| `just test-workspace` | **Failed:** compiler `callable_execution`, 57 passed and 24 existing failures |
| Sema/compiler failure-name comparison with the preceding materialization cut | Exact matches; no additions or removals |
| `just test-doc` | 8 passed |
| Canonical structure audit and `just structure-audit-gate` | Passed; 95 packages, 2,273 Rust files, 310 review triggers, 0 blocking violations |
| Cached/working diff checks, validated Rust identity and WIP restoration hashes | Passed |

The first new lifetime test build failed because it used a nonexistent report
getter. It was corrected to read the existing counter before the passing run.
The nine positive sema failures remain the five contextual/correlated call
cases and four inferred callback-effect cases. Their expectations were not
weakened. Working-copy scope/frontier evidence is separate from exact-cut
evidence and does not establish child-call integration.

All 29 preserved files were restored and matched their original raw hashes and
sizes. All seven staged Rust blobs remained identical to the validated index
tree. The physical structure scanner includes the three unreferenced WIP Rust
files; the changed-file record below covers only this cut.

The failed workspace recipe did not run later targets or its subsequent CLI
steps. Focused sema and workspace check/Clippy use all features; the canonical
workspace/doctest recipes also use their default-feature combination. Tier 2
is not applicable to this private sema lifetime boundary: it changes no
Agent/MCP, I/O, capture, native/render behavior or runtime contract. `just verify`
and generated JLREQ checks were not selected for this correction. No clean or
disk failure occurred during validation.

## Structural disposition and remaining work

The owning boundaries remain `analyzer -> callable -> types`: the analyzer
drives prepared semantic operations, callable owns affine callback checkpoints,
and types owns lower solving and accumulated accounting. The context's release
is its accounting boundary. There is no second driver algorithm, replacement
accountant, report merge path or owned/borrowed context enum.

The driver and analyzer modules retain their cohesive orchestration roles.
The lower context and transaction keep their existing separate responsibilities
of work/lexical control and candidate state. Their tests stay with those owners;
moving private lifecycle checks to another crate would require widening APIs.
No crate dependency, public API, language syntax, runtime contract or version
marker changes.

Generated [changed-file measurements](structure-audits/2026-09-11-borrowed-constraint-driver/changed-files.csv)
retain the exact validation tree's bytes, complete physical LOC and growth:

| Owner / responsibility | Bytes | LOC / growth | Embedded test LOC |
| --- | ---: | ---: | ---: |
| Callable driver and affine callbacks | 92,898 | 2,281 / +63 | 1,367 |
| Callable accounting reservation and limits | 48,632 | 1,412 / +7 | 156 |
| Analyzer source preparation/materialization adapter | 202,685 | 4,966 / +1 | 480 |
| Lower context, work and lexical control | 62,748 | 1,700 / +5 | 0 |
| Lower transaction, sources and candidate completion | 67,289 | 1,810 / -34 | 0 |
| Lower constraint behavior test module | 144,032 | 4,104 / -16 | 0 |
| Transaction lifetime/materialization test module | 14,960 | 429 / +84 | 0 |

The existing production size triggers retain the cohesion dispositions above.
Callback tests remain with the callable owner, while lower equation/projection
tests and transaction lifecycle tests keep their existing child modules. The
change separates reservation lifetime from driver lifetime without combining
unrelated state, transport, persistence or runtime responsibilities. The owning
sema crate's workspace fan-in/out is 8/14, and development fan-in/out is 3/0.

The uncommitted path-scope and source-frontier migration remains governed by the
[coupled request](../reviews/requests/2026-09-08-aw-ah-009.4.2.1.1.1-function-scheme-specialization-and-callable-value-execution.md).
Actual child application admission, whole-component materialization and the
function-scheme/native/AWBC consumers remain required work. No external blocker
or design deviation was introduced.
