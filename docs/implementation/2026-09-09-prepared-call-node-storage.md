# Prepared call node storage — 2026-09-09

Inspected base: `393308a6f903d7f947c06fe0a9da8961aa067331`, existing `main`,
pushed and clean before this cut. Supersedes the unresolved capture stack
overflow in the [Agent enum record](2026-09-09-agent-enum-case-authority.md).

## Reproduction and change

After the enum correction, capture source checking succeeded, but
`agent_script_run_persists_attach_capture_debug_record` still failed because
its source subprocess exhausted the Windows main-thread stack. The earlier
LLDB trace retained in the enum record located the exhaustion in BTreeMap
insertion of PreparedCallNode during selected-call publication. That trace
measured the pre-enum executable; the rebuilt enum-corrected executable also
failed the same source regression before this storage change.

PreparedCallGraph and PreparedCallGraphDelta now own boxed nodes. Selected
values, selected continuations and unselected results all use the same storage
rule. Insertion and transfer between maps move the box instead of copying the
large candidate payload through BTree insertion frames. Issuer/node identities,
dependency ordering, preflight, replay comparison, commit, rollback and final
seal semantics are unchanged. A node remains singly owned and moves with its
transaction; no shared mutable alias or parallel node catalog was added.

This repairs the graph's storage boundary without increasing a stack limit,
changing Cargo concurrency or debug profiles, creating a worker thread, or
special-casing capture source. It does not establish a new global source-depth
or memory-bound guarantee; the goal's remaining traversal and budget work
retains its own acceptance criteria.

## Validation

Logs are local and ignored under
`.arcweft-local/validation/2026-09-09-prepared-call-node-storage/`.

| Command | Result |
| --- | --- |
| Existing CLI capture regression before this cut | 0 passed / 1 failed, main-thread stack overflow |
| `cargo test -p arcweft-cli --test check agent_script_run_persists_attach_capture_debug_record -- --exact` | 1 passed; source compilation, capture, attach and persisted debug record; 48.82 s including rebuild |
| `cargo test -p arcweft-lang-sema --lib --quiet` | 760 passed / the same 7 generic inference and higher-order effect failures; 25.06 s. Existing graph publication, candidate rollback, stale-reference and continuation tests ran in this suite |
| `cargo fmt --all` | Passed; 10.40 s |
| `cargo check --workspace --all-targets --all-features` | Passed with existing warnings; 25.81 s |
| `cargo clippy --workspace --all-targets --all-features` | Passed with existing warnings; 28.46 s |
| `just test-workspace` | 856 passed / the same 18 callable failures, 84 reports; 254.22 s. The recipe stopped in compiler `callable_execution`, so later workspace/CLI commands were not run |
| `just test-slow-mcp` | 4 passed; 3.39 s |
| Structural audit with `--fail-on-blocking` | 95 packages, 2,243 Rust files, 309 review triggers, 0 blocking violations; 2.60 s |
| Documentation links and `git diff --check` | Passed; local link targets checked, anchors not checked |

The production change is private storage in one sema module. The matching MCP
Tier 2 target and direct CLI capture regression are selected; exhaustive
native/auxiliary/visual/proof Tier 2 is not repeated for this representation
change. The previous cut's native observe HIR publication failure remains
unresolved. Doctests are not repeated: no public API, Rust documentation
example or public documentation contract changed in this cut.

## Ownership and remaining work

`crates/arcweft-lang-sema/src/callable/continuation.rs` remains the prepared
call graph and transaction owner. Its source changes from 2,385 to 2,388
physical LOC, 93,235 bytes, production classification and 27 embedded test
LOC. Normal dependency fan-in/out is 8/14 and development fan-in/out is 3/0.
There is no new crate, dependency, public export, type
version or Cargo feature. The graph and extracted delta keep the same node
representation; no map-specific fallback is retained. The added allocation
belongs to the graph node's lifetime, and all moves and drops remain governed
by the existing transaction paths. This is a cohesive storage correction in
the existing owner; no unrelated state or transport concern was introduced.
The audit ran on the above base with this cut dirty; generated metrics remain
in the local validation directory.

Capture's enum and stack failures are now resolved by consecutive cuts.
The known sema 7 and callable 18 failures, native HIR publication, and the
remaining Match/View/task-plan/nominal/scheduler acceptance remain part of the
active convergence goal. No design deviation is introduced.
