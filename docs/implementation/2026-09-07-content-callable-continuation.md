# Content/callable continuation — 2026-09-07

This is working-copy evidence. The Content/Fx/ProjectCall vertical is **not
complete or accepted on main**. Compiler regressions were repaired; one
recursive-generic constraint conflict remains underdesigned. Its correction
request is [AW-AH-009.4.2.1.1](../reviews/requests/2026-09-07-aw-ah-009.4.2.1.1-recursive-generic-call-constraint-scope-reconciliation.md).

This supplements the [2026-09-01 critical-path note](2026-09-01-convergence-goal-critical-path.md)
with observable local evidence. It does not supersede that note's mainline
completion ledger or change the predecessor order.

## Git boundary

- Initial checkout: `fed4102e50c90b06e2dbabf1e7259ee09352008d` on `main`,
  empty index, 652 reported dirty/untracked status entries. The large migration
  was already present.
- Fetched and fast-forwarded to `9c2e606e25f6e6e4fe8ff7808e8ce9ecdd9204b0`,
  preserving the working copy.
- Removed only the obsolete fixed agent/model assignment section from root
  `AGENTS.md`, as requested. Explicit staging and cached-diff review preceded
  the pushed commit `3ca699521d49fc54c6e4e0163f53c1097955bb10`.
- Rust evidence below refers to that accepted SHA plus dirty changes. There is
  no resulting accepted Rust implementation SHA. No branch, worktree, reset,
  stash, or speculative WIP push was used.

Instruction cleanup and this implementation/request record are separate
documentation cuts. The incomplete Rust authority switch remains together
until its required design and validation boundaries close.

## Working-copy changes

### Call order and function identity

The checked call seal derives physical operand ABI destinations from logical
parameter coordinates while retaining the source-ordered row. Equal rest
destinations retain source order; receiver and attached slots participate in
the checked permutation. Named arguments and specialized Agent operands no
longer exchange values when evaluation and ABI order differ.

Agent entries/manifests consume the same checked runtime callable identity as
ordinary function sites. AWBC expression-bodied FunctionSites emit ordinary
functions, as ProjectCall verification requires. Controller tests follow the
typed entry adapter into the actual function and inspect its materialized
Agent operands.

Owners: sema `callable/checked_application.rs` and
`final_analysis/analyzer/call_seal.rs`; compiler `agent.rs` and
`project/entry_tests.rs`; runtime-plan `awbc_lower/expr.rs`.

### Parser and type ownership

Flow expression statements retain a trailing semicolon outside the expression
span. Pratt parsing stops after a missing/error prefix instead of attaching
postfix operators to an absent expression head. A Content interpolation can
therefore no longer manufacture an index candidate without a target. The
accepted rule retaining two genuinely viable postfix candidates is unchanged.

HIR type-root projection distinguishes runtime-bearing roots, semantic-only
roots, and callee-resolution inputs. An open associated receiver constructor
can participate in resolution without becoming a standalone runtime value
type. A runtime use takes precedence when a type has multiple roles. Exact HIR
executable expression partitions decide which closed expressions require
runtime types; non-escaping dialogue application results remain semantic
evidence.

Owners: syntax `parser/{statement,expression}.rs` and tests; HIR `expr.rs`,
`final_project/type_roots.rs`, `runtime_semantic_owners.rs`; sema
`final_analysis/{report,nominal_schema}.rs`; compiler `lower.rs`; runtime-plan
`semantic_facts.rs` and `semantic_facts/project_function.rs`.

### Closed executable frames

`ControlLocals` admits Await, Try, pipe, and call-result temporaries from one
executable semantic view. Global, closed-function, and closed-closure frames
receive their own temporaries. Flow, expression, dialogue capture, effect
callback, and line-plan lowering select the same scope for HIR locals,
specialized operands, and control temporaries. The global-only temporary
paths were removed.

Nominal record/variant domain inventories and normalized type roots include
closed function catalogs and their owned closure catalogs. Function bodies
are defined after dialogue handles are admitted. Dialogue applications and
ordinary project calls participate in checked execution classification.
Executable function tails use the shared Flow continuation rather than
assuming expression-only evaluation.

The suspension regression registers a real source-backed adapter contract and
checks closed generic functions containing Try/Await and tail Await. It
replaces the obsolete expectation that every reached suspending function must
be rejected. The standard `load_bg` semantic fixture alone is not a runtime
host contract.

Owners: runtime-plan `final_flow.rs`, `final_flow/control_locals.rs`,
`final_flow/line_plan.rs`, `final_expr.rs`, and semantic catalogs; compiler
`lower.rs`, `project/tests.rs`; sema
`final_analysis/analyzer/callable_effect_graph.rs`.

### Bundle inventory

`RuntimePlan::visit_flow_ops` owns deterministic traversal of flows,
executable function sites, line actions, cancellation rules, and cleanup
bodies. It visits bodies from their owning tables rather than following
ProjectCall references, so recursive call edges do not recurse through the
inventory.

CLI host-call and static image-reference collection consume this traversal;
their duplicate recursive Flow/line-task walkers were deleted. Host calls
inside ordinary functions are visible to the bundle manifest. JIT test
construction supplies ABI positions, and CLI value summaries handle typed
ProjectContinuations.

Owners: core `plan.rs`, `plan/flow_ops.rs`; CLI `app/{bundle,jit}.rs` and
`app/bundle/tests.rs`; JIT `src/tests.rs`.

### Generic scope: ingress repaired, recursion unresolved

The analyzer formerly passed an empty enclosing generic scope to every
candidate. It now gets the exact declaration from HIR semantic topology and
the inventory from its accepted callable signature. Equal rigid references
shared with a closure schema do not create two owners. A constructor can bind
its own parameter to the caller's rigid type, while the caller's type cannot
be bound to an unrelated concrete type.

Self-recursion still requires the same declaration-owned generic ID to denote
both a caller-rigid type and a callee inference parameter. The graph's one
eligibility map keyed by that ID rejects the collision with
`MalformedSchemaInventory`. No wildcard, insertion-order override, empty-scope
fallback, or recursive-call exception was adopted. The positive regression
remains enabled and failing in sema
`final_analysis/tests/generic_calls.rs`.

The [correction request](../reviews/requests/2026-09-07-aw-ah-009.4.2.1.1-recursive-generic-call-constraint-scope-reconciliation.md)
requires a complete invocation-scope, solution, continuation, and recursive
closed-instance contract before this Rust cut can close.

## Validation ledger

Cargo selected its normal concurrency; no explicit job count was supplied.
Focused suites used package defaults. Workspace compile/lint gates used the
all-feature combination required by policy. No test was disabled to obtain a
passing result.

| Command | Observed result |
|---|---|
| `git diff --check` | Passed at the recorded review checks |
| `cargo fmt --all` | Passed, preserving inherited Rust edits |
| `cargo fmt --all --check` | Passed on the final Rust edits |
| `cargo test -p arcweft-lang-syntax --lib` | Passed: 670 tests |
| `cargo check -p arcweft-runtime-plan --all-targets` | Passed during scoped frame/type projection work |
| `cargo test -p arcweft-compiler --lib project::tests::generic_project_` | Passed: 3 tests after dialogue/closure scope repairs |
| `cargo test -p arcweft-compiler --lib` | Passed: 62 tests, including two generic instantiations each of Try/Await and tail-Await functions |
| `cargo test -p arcweft-compiler --test project_function_instances` | Passed: 6 tests |
| `cargo test -p arcweft-core --lib project_call` | Passed: 27 native/AWBC execution, ordering, verifier, unwind, and snapshot tests |
| `cargo test -p arcweft-core --lib project_continuation` | Passed: 3 serialization/snapshot ABI tests |
| `cargo test -p arcweft-lang-sema --lib final_analysis::tests::generic_calls::generic_result_constructor_retains_enclosing_parameter_identity -- --exact` | Passed after scope admission; previously failed with `TypeParameterOutOfScope` |
| `cargo test -p arcweft-lang-hir -p arcweft-lang-sema --lib` | HIR: 881 passed, 8 ignored. Sema then: 683 passed, 2 failed, including an invalid test identifier subsequently corrected |
| `cargo test -p arcweft-lang-sema --lib` | Latest: 684 passed, 1 failed; recursive generic scope collision |
| `cargo test -p arcweft-lang-sema --lib final_analysis::tests::generic_calls::` | Final-edit recheck: 3 passed, 1 failed with the same recursive scope invariant; 681 filtered |
| `cargo check --workspace --all-targets --all-features` | Passed after JIT/CLI migration; unused CLI imports reported then were subsequently removed |
| `cargo test -p arcweft-cli --lib app::bundle::tests::bundle_host_inventory_includes_nested_ordinary_function_body -- --exact` | Passed: 1 test after disk cleanup; the earlier attempt failed before test execution |
| `cargo clippy --workspace --all-targets --all-features` | Passed with warnings on the final edits; earlier disk failure was retried. This is not a warning-free workspace claim |
| `cargo +nightly -Zscript tools/structure-audit.rs --root . --fail-on-blocking --write target/continuation-structure` | Passed on final Rust edits: 95 packages, 2,329 files, 2,201 Rust files, 1,243,336 physical Rust LOC, 309 review triggers, 0 blocking violations |

Intermediate compiler failures exposed source-order, parser, Agent identity,
type/domain, dialogue control, and frame-local defects. The initial full suite
had 45 passes and 17 failures; a run explicitly skipping the generic dialogue
regression had 45 passes and 16 failures. Later runs repaired those failures
and reached the recorded full-suite pass. Workspace checks first found a JIT
ABI constructor omission and then five CLI consumer errors, repaired before
the passing check.

Windows linking also failed with a PDB limit; a subsequent compiler build and
later CLI/Clippy builds exhausted disk space. The exact generated compiler PDB
was removed. Scoped `cargo clean -p arcweft-compiler` reclaimed compiler
artifacts; `cargo clean -p arcweft-cli` removed 766 files (4.2 GiB). After the
second exhaustion, `cargo clean -p arcweft-lang-sema -p arcweft-bundle -p
arcweft-core` removed 15,346 generated files (102.5 GiB), restoring about 97 GB
free space. An earlier whole incremental-cache deletion was rejected by
automatic approval review with a policy-block reason and did not execute.
Scoped Cargo cleanup succeeded. No user source was removed; an empty new
implementation note left by disk exhaustion was rewritten after cleanup.

Not run: `just test-workspace`, exhaustive `just test-tier2`, workspace
doctests, complete codec/golden acceptance, and the remainder of the parent
Content/Fx acceptance matrix. These remain required before Rust mainline
closeout. The enabled sema regression already prevents that claim.

## Structural ownership review

These are complete current-file measurements from the canonical audit, not
diff additions. Base LOC is at
`3ca699521d49fc54c6e4e0163f53c1097955bb10`; growth includes inherited WIP.
Paths are relative to `crates/`; entries are production modules.

| Owner/path | Bytes | Base → current LOC | Disposition |
|---|---:|---:|---|
| compiler `src/lower.rs` | 334515 | 3981 → 7941 | One checked-sema/runtime projection transaction; closed catalog admission remains atomic and has no copied fallback |
| sema `src/callable/checked_application.rs` | 165928 | 3641 → 4470 | One checked application/solution/ABI seal; source permutation is construction-only |
| sema `src/callable/continuation.rs` | 92578 | 2307 → 2369 | Affine graph initialization; 27 embedded test LOC. Recursive scope is explicitly blocked, not accepted as complete |
| sema `src/final_analysis/analyzer/calls.rs` | 186244 | 3386 → 4314 | Candidate transactions consume existing topology/signatures; 185 embedded test LOC, no second scope index |
| sema `src/final_analysis/analyzer/call_seal.rs` | 86628 | 1542 → 1957 | One final call seal; ABI behavior belongs to its typed application owner |
| sema `src/final_analysis/report.rs` | 89668 | 1165 → 2225 | Checked-generation query/projection; HIR partitions own runtime type membership |
| sema `src/final_analysis/nominal_schema.rs` | 120105 | 2357 → 2929 | Accepted nominal/type-root admission; runtime layout does not move into syntax/HIR |
| syntax `src/parser/expression.rs` | 87079 | 2513 → 2519 | Cohesive Pratt prefix/postfix parsing boundary |
| syntax `src/parser/statement.rs` | 73619 | 2183 → 2190 | Statement/CST delimiter ownership, shared expression-end rule |
| runtime-plan `src/final_expr.rs` | 102280 | 1967 → 2542 | Pure expression projection consumes exact scope locals |
| runtime-plan `src/final_flow.rs` | 289235 | 4343 → 7038 | Executable orchestration; 361 embedded test LOC. Temporary admission decomposed at its state boundary into `final_flow/control_locals.rs` (136 LOC) |
| runtime-plan `src/semantic_facts.rs` | 392101 | 7328 → 10319 | Whole-generation catalog validation; instance catalogs remain in `semantic_facts/project_function.rs` (2358 LOC), borrowed for domain admission |
| runtime-plan `src/awbc_lower/expr.rs` | 74431 | 1865 → 2016 | Function/expression bytecode lowering; ordinary/synthetic kind follows existing ownership |

The bundle change adds core `src/plan/flow_ops.rs` (4,035 bytes, 108 LOC)
behind RuntimePlan. Core `src/plan.rs` is 48,971 bytes/1,387 LOC and retains the
admitted operation algebra. CLI `src/app/bundle.rs` is 43,057 bytes/1,191 LOC
after deleting duplicated walkers. CLI `src/app/jit.rs` is 58,154 bytes/1,628
LOC, retaining JIT command construction/reporting. Regression tests remain in
their corresponding compiler/CLI test modules, below the integration-test
review trigger.

Production workspace fan-in/fan-out is compiler 3/23, HIR 10/3, sema 8/14,
syntax 12/2, runtime-plan 5/9, and JIT 2/1. No dependency or feature was added.
Core traversal stays Sans I/O; host formatting and file access remain in CLI.
These owner/cohesion dispositions concern the changes above. Formatting-only
changes elsewhere do not establish a new architecture or acceptance result.

## Archive scope and remaining work

Enumerated 70 retained ZIPs, totaling 4,757,394 bytes, with no ZIP at the inbox
root. SHA-256 comparison found 31 hashes in implementation notes and 39 without
an exact match there. This is an audit-coverage gap, not a finding of corrupt
archives or new readiness. No archive was moved, rewritten, or re-adjudicated
as ready. The in-repository AW-AH-009.4.2.1 design remains inherited uncommitted
state.

Next: close the recursive-generic request, migrate its complete authority,
finish the parent acceptance matrix, and run broad gates. Generic Match C3/C5,
retained View request refresh, task-plan sealing, structural nominal closeout,
and scheduler/restore are outside this continuation. First-class runtime Need
values were not implemented or claimed by the registered-host Await test.

No stable language/wire design changed. The recursive conflict is a required
contract correction; no guessed design deviation was adopted.
