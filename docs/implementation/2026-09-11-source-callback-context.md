# Source callbacks borrow their driver context

The source-probe API previously accepted a source coordinate, a borrowed hint
and a work session separately. `CandidateConstraintSourceContext` now keeps
the exact lower probe ticket and the driver's current type-constraint context
together. Only the callable driver constructs that borrowed capability.
Clients obtain the source and hints from its ticket and charge work through
its existing accountant.

The inspected base is `main` at
`b1d89f05e4d4574102299e5b76ecd87eb33f4ca2`. This cut changes only the
crate-private source callback contract and all of its production/test clients.
It removes the former tuple of callback arguments. It does not change language
syntax, published crate APIs, runtime formats or any contract version.

## Ownership and scope

- `callable/constraints.rs` owns source callback entry, ticket/checkpoint
  lifecycle and work borrowing. A client cannot construct a callback context
  from independently chosen hints or an unrelated accountant.
- `final_analysis/analyzer/calls/constraints.rs` owns the expression/fact
  adapter. It reads the source and projected expectations from the driver
  capability, performs its existing semantic checks and returns the same
  typed outcome to the affine close operation.
- `callable.rs` exposes the new type only inside the sema crate. No lower layer
  depends on analyzer types, and no I/O or new dependency is introduced.

The callback context borrows its owner; it creates no fresh constraint context
or work reservation. Higher-ranked hint access retains the existing borrowed
hint lifetime. Checkpoint opening/closing, materialization and failure
precedence keep their existing owners.

This ownership cut is independent of the unfinished path-scope migration in
the same checkout. To validate the exact cut, the other agent-owned changes
were preserved as Git blobs, a binary patch and a raw SHA-256 manifest, then
temporarily reversed in the existing `main` checkout. The tracked working tree
was checked against the staged index before validation. No branch, worktree,
additional checkout, reset or deletion was used. After validation, all 30
preserved files were restored and verified byte-for-byte against that manifest;
the staged source-context code was unchanged.

## Validation

Command logs and the index/backup manifest are in
`.arcweft-local/validation/2026-09-11-source-callback-context/`.

| Evidence | Result |
| --- | --- |
| `cargo test -p arcweft-lang-sema --all-features --lib` | **Failed:** 778 passed, 9 existing positive failures |
| Source driver tests within that run | 16 passed; success/rejection/fatal cleanup, wrong-ticket rejection, cancellation, work limits and error precedence |
| `cargo check --workspace --all-targets --all-features` | Passed with warnings |
| `cargo clippy --workspace --all-targets --all-features` | Passed with warnings |
| `just test-workspace` | **Failed:** compiler `callable_execution` has 57 passed, 24 existing failures; the recipe stops at its first Cargo command |
| `just test-doc` | 8 passed |
| `cargo fmt --all --check` | Passed |
| Canonical structure audit with retained local reports; `just structure-audit-gate` | Passed; 95 packages, 2,273 Rust files, 310 review triggers, 0 blocking violations |
| Working/index diff checks and restoration hashes | Passed |

The nine existing failures are the three contextual-constructor cases, two
correlated ordinary-call cases and four inferred callback-effect cases. Their
positive expectations remain unchanged. The absence of the six uncommitted
scope-admission tests from this isolated run is intentional: that unfinished
implementation is not part of this cut.

The 24 compiler failure names match the prior semantic-project-lease
`verify.log` exactly: no additions or removals. Later targets in the failed
workspace command and subsequent CLI recipe commands were not run. Focused
sema/check/Clippy use all features; the additional default-feature combination
comes from the canonical main-push and doctest recipes. Tier 2 is not applicable
to this crate-private callback ownership change: it changes no Agent/MCP
protocol, I/O, capture, native/render behavior or runtime contract. `just verify`
and generated JLREQ checks were not selected for this narrow cut.

## Structural review

The three changed files keep their existing responsibilities: the callable
facade, affine callback driver and analyzer expression/fact adapter. The new
borrowed context is the source entry authority, not a second solver, source
catalog or fact store. The adapter retains source-specific semantic checking;
it does not take ownership of lower constraint state. The dependency direction
remains `analyzer -> callable -> types`.

The driver module's embedded tests exercise its private ticket/checkpoint and
failure lifecycle. They stay with that owner without widening production APIs
for test access. The analyzer module remains the boundary for candidate
preparation, source observation and materialization facts. This cut changes
that boundary's input contract and does not mix in transport, storage or
runtime execution.

Generated [changed-file measurements](structure-audits/2026-09-11-source-callback-context/changed-files.csv)
retain exact bytes, complete-file LOC, embedded test LOC and growth from the
inspected base:

| Owner | Bytes | LOC / growth | Embedded test LOC | Disposition |
| --- | ---: | ---: | ---: | --- |
| Callable facade | 13,833 | 233 / 0 | 0 | Retain the crate-private export boundary |
| Affine source driver | 88,391 | 2,218 / +24 | 1,316 | Retain one ticket/checkpoint lifecycle and its private protocol tests |
| Analyzer constraint adapter | 202,309 | 4,965 / +9 | 480 | Retain the candidate/source/fact adapter and its exact evidence, failure-owner and rollback tests |

The analyzer module's upper-size trigger receives an explicit cohesion
disposition: preparation, observation and materialization share the candidate
schema, source coordinates and affine fact scope. This change adds no second
state store or traversal and requires no public visibility widening for file
splitting. The driver and adapter tests follow their respective production
boundaries. The sema package's normal workspace fan-in/fan-out is 8/14; its
development-only counts are 3/0, unchanged by this cut. The scanner also sees
the three preserved, unreferenced Rust WIP files physically present in the
checkout; they are excluded from these changed-file measurements and from the
validated Cargo module graph.

## Remaining goal work

This is not completion of
[correlated callable inference](2026-09-10-callable-convergence-model.md) or the
[convergence goal](2026-09-08-convergence-goal-plan.md). Child calls still finish
independently. The pending child contribution, candidate correlations,
component closure, function-scheme/effect integration and runtime/restore
requirements remain open. The preserved path-scope work must be connected and
validated before it can be published as its own complete implementation cut.
