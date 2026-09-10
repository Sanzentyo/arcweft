# Curried application effects and continuation ABI

Date: 2026-09-10. Existing `main` was clean at
`2a466cbcdcea89191b9fe90d9914446322e1ced1`, equal to `origin/main`.
The implementation and validation in this note refer to the dirty checkout
at that base. This completes a concrete boundary exposed by the
[callable investigation](2026-09-10-callable-convergence-model.md); the full
[convergence goal](2026-09-08-convergence-goal-plan.md) remains active.

## Boundary and implementation

A declared call group before the terminal group retains its evaluated
arguments. Its function arrow has an empty invocation row. Only the terminal
declared group receives the declaration's invocation row. Function types
inside parameters and the authored result retain their own rows. Argument
expression effects still belong to their actual evaluation sites. The
[maintained language chapter](../01-language/functions-and-pipeline.md)
now states this timing explicitly.

The schema owns one private `project_function_type_from_group` fold. Prepared
source, prepared projected, checked remaining, fixed-schema and analyzer
source projections all use it; the five separate group folds were deleted.
The function-value schema constructor moved into the same responsibility
module. Projection contexts still own their parameter/result substitutions,
and the shared fold preserves their typed errors. Private projection API names
now identify the supplied row as terminal effects.

An already known declaration row also constrains the candidate's scoped
invocation variable before source probing. The prepared constraint set retains
this evidence and its existing driver submits equality to the lower path's
effect environment. Equality is both subset directions: the known row cannot
collapse to pure or grow to satisfy an incompatible use. Foreign/unknown rows
remain subject to the existing scope validation, and accounting/cancellation
failure cannot publish a partially constrained path. Unknown declaration
summaries supply no ground constraint. Fixed schema rows already own their
row and do not receive a second declaration constraint.

The checked core compares a fixed schema row only at an effective terminal
application; earlier applications require an empty invocation row. Existing
bound extension groups remain handled by the resolved base's `next_group_for`.
Runtime selection projects a direct callable using the checked declaration
row, instead of using the current intermediate call's empty row.

The executable three-group case also exposed a second invalid boundary:
native construction and AWBC verification required a newly produced prefix
to have the input prefix's lineage and function type. The accepted
[ProjectCall contract](../reviews/designs/aw-ah-009.4.2.1-project-callable-attached-content-declaration-runtime-abi-final-resolution/FINAL_CONTRACT.md)
and [Rust shapes](../reviews/designs/aw-ah-009.4.2.1-project-callable-attached-content-declaration-runtime-abi-final-resolution/RUST_SHAPES.md)
instead give every checked nonterminal result its own lineage and the
remaining function type. Both validation contexts now compare the input
function's parameter row with the current logical ABI and its result with the
new remaining function type. Native construction also compares every appended
prefix binding's semantic type, matching the existing AWBC binding check.
Prefix preservation, exact length, one-group advancement, function membership,
operand coverage and the input value's exact expected lineage remain checked.

The AWBC fixture now creates the first prefix through a real direct
ProjectCall before applying the next group. It no longer uses an ordinary
closure with an unchanged function type to stand in for that transition.
Negative cases reject a retained input function type, mismatched parameters,
dropped prefixes and changed binding types.

No dependency, feature, contract version, serialized field or runtime carrier
was added. The separate native construction and AWBC verification contexts
resolve their own table identities; no local ordinal is used as a semantic ID.

## Validation

Local command logs are under
`.arcweft-local/validation/2026-09-10-curried-group-effects/`.
Every Cargo command ran sequentially with Cargo's normal concurrency; no
explicit job count was used. Focused runs used all features consistently. The
mandatory Justfile workspace/doc/Tier 2 recipes retain their own feature sets.

The first semantic regression failed because the retained terminal row was
empty instead of `fs.read`. An initial implementation check also failed on a
nonexistent effect-row accessor; the corrected code uses the actual closed-tail
API. After the semantic fix, both new compiler cases failed at the invalid
continuation-identity check. They now pass after both runtime validation
consumers migrated.

The AWBC test migration first failed to compile because it tried to access a
private type-shape field; it now uses the existing constructor. A first fixture
layout then failed the backedge safe-point rule, and a negative fixture still
referenced the old Dynamic row index. The fixture now follows forward execution
order and derives that index before insertion. These failed logs are retained;
no production check was weakened to accommodate the fixtures.

| Performed validation | Result |
| --- | --- |
| `cargo test -p arcweft-lang-sema --all-features --lib fixed_effect_evidence` | Passed: 3 tests; fixed row survives sealing, shrinking/expansion reject, and node-limit interruption publishes no partial equality; 18.296 s |
| `cargo test -p arcweft-lang-sema --all-features --lib --no-fail-fast` | 770 passed / 9 known failures; 1.751 s; includes both new curried effect tests |
| `cargo test -p arcweft-core --all-features --lib` | Passed: 361 tests, including all 19 ProjectCall tests and existing snapshot/ABI rejection tests; 8.750 s |
| `cargo test -p arcweft-compiler --all-features --lib --test evaluated_effects --test project_cache_transaction --test callable_execution --no-fail-fast` | Library 82 passed; evaluated effects 17 passed; cache transactions 20 passed; callable execution 56 passed / 24 known failures; 101.033 s |
| `cargo check --workspace --all-targets --all-features` | Passed; 69.537 s |
| `cargo clippy --workspace --all-targets --all-features` | Passed with existing warnings; 75.317 s |
| `just test-workspace` | Failed at callable execution: 56 passed / 24 known failures; 464.846 s; later workspace targets and CLI recipe commands were not run |
| `just test-doc` | Passed: 8 tests, none ignored; 146.010 s |
| `just test-tier2` | MCP stdio 4 passed and native observe/capture 1 passed; failed at the existing `samples/image-animation.arcw:1:1` declaration parse/recovery error; 59.891 s |
| `just structure-audit` / `just structure-audit-gate` | Both passed: 95 packages, 2,261 Rust files, 310 review triggers, zero blocking violations; 5.846 / 2.371 s |
| `cargo fmt --all -- --check` | Passed; 10.276 s |
| `cargo metadata --no-deps --format-version 1 --all-features` | Passed; dependency graph captured; 0.149 s |
| Relative documentation links / `git diff --check` | 12 file targets resolved, none missing; whitespace check passed |

The Tier 2 failure stops the first auxiliary image test. Subsequent auxiliary
capture tests, visual goldens and Select/Flow production-limit recipes were
not run. The current cut does not change that sample or its grammar. Failed
and unexecuted tiers remain explicit; passing focused tests do not imply that
the whole convergence goal or workspace test suite is green.

The semantic failures remain the three contextual-constructor, two correlated
ordinary-call and four inferred-callback-row requirements. The 24 compiler
failures remain the prior callable requirements, including the two executable
nonterminal-prefix callback probes. The new `curried_terminal_effect` pair
returns 42 through native execution and canonical AWBC encode/decode plus VM
execution. Comparing the two compiler runs removes exactly that pair from the
failure set. Positive requirements remain enabled, and no failure was changed
to an expected rejection or ignored test.

## Structural review

Exact physical measurements below compare the complete file at the base SHA
with the current file. The crate owner is the path segment after `crates/`.
Embedded test LOC use the canonical scanner's measurement convention; they
are ownership-review evidence, not source-spelling acceptance checks.

| Path | Class | Base LOC | Current LOC | Bytes | Embedded test LOC |
| --- | --- | ---: | ---: | ---: | ---: |
| crates/arcweft-compiler/tests/callable_execution.rs | Test | 634 | 650 | 16556 | 0 |
| crates/arcweft-core/src/awbc/schema.rs | Production | 3040 | 3048 | 92331 | 0 |
| crates/arcweft-core/src/awbc/tests.rs | Test | 5029 | 5000 | 178543 | 0 |
| crates/arcweft-core/src/awbc/verify/code.rs | Production | 3753 | 3763 | 150041 | 0 |
| crates/arcweft-core/src/plan/construction/lower.rs | Production | 5248 | 5283 | 218770 | 208 |
| crates/arcweft-core/src/plan/project_call.rs | Production | 728 | 735 | 25131 | 0 |
| crates/arcweft-lang-sema/src/callable/checked_application.rs | Production | 4498 | 4503 | 166819 | 0 |
| crates/arcweft-lang-sema/src/callable/constraints.rs | Production | 2185 | 2194 | 87421 | 1317 |
| crates/arcweft-lang-sema/src/callable/continuation.rs | Production | 2388 | 2399 | 93703 | 27 |
| crates/arcweft-lang-sema/src/callable/join.rs | Production | 1906 | 1906 | 72302 | 0 |
| crates/arcweft-lang-sema/src/callable/resolver/outcome.rs | Production | 1514 | 1510 | 55422 | 0 |
| crates/arcweft-lang-sema/src/callable/schema.rs | Production | 4548 | 4487 | 164977 | 1186 |
| crates/arcweft-lang-sema/src/final_analysis/analyzer/call_seal.rs | Production | 2015 | 2015 | 88103 | 0 |
| crates/arcweft-lang-sema/src/final_analysis/analyzer/calls.rs | Production | 4304 | 4312 | 186780 | 185 |
| crates/arcweft-lang-sema/src/final_analysis/analyzer/calls/constraints.rs | Production | 4940 | 4966 | 202870 | 480 |
| crates/arcweft-lang-sema/src/final_analysis/analyzer/calls/semantics.rs | Production | 236 | 230 | 8197 | 0 |
| crates/arcweft-lang-sema/src/final_analysis/tests/higher_order_effects.rs | Test | 317 | 415 | 14067 | 0 |
| crates/arcweft-lang-sema/src/types/constraints/transaction.rs | Production | 1786 | 1842 | 68279 | 0 |
| crates/arcweft-lang-sema/src/types/constraints/transaction/tests.rs | Test | 133 | 259 | 9254 | 0 |
| crates/arcweft-lang-sema/src/callable/schema/function_type.rs | Production | 0 | 136 | 5479 | 0 |

Touched review triggers have these dispositions:

- `callable/schema.rs` is decomposed at the complete schema/function-type
  projection boundary, including function-value schema construction. The
  existing schema validation, coordinate and family rules stay in their
  owning module; the moved responsibility remains private and has no
  independent catalog or effect solver. Existing schema tests still exercise
  the same owner. There is no facade or public-API widening for the split.
- `checked_application.rs`, `resolver/outcome.rs` and `join.rs` retain their
  respective checked authority, prepared definition and exact runtime-selection
  join. All group folds delegate to the schema owner. These contexts keep
  ownership of substitutions and catalog evidence; moving that state into the
  fold would combine unrelated lifecycle authorities.
- `callable/constraints.rs` remains the affine client/solver driver; the new
  effect operation forwards into its existing lower transaction. Its embedded
  tests cover that source/materialization protocol. `types/constraints/transaction.rs`
  owns path-local equations and effect environments, including pruning and
  abort behavior. Tests for the new evidence live in its existing child test
  module. No parallel solution or materialization path was added.
- `continuation.rs` retains its typed callable invariant conversion with the
  existing error owner; no new stored state or public error branch was added.
  The small embedded tests remain tied to that continuation responsibility.
- Analyzer `calls.rs` and `calls/constraints.rs` retain candidate orchestration
  and authenticated prepared constraints. Known row evidence moves through
  the same prepared set as base/receiver/source constraints. It does not
  acquire a separate inference map. `call_seal.rs` has only the private API
  rename and retains its final sealing context. Existing embedded source and
  solver tests remain with their protocol owners.
- Core `plan/construction/lower.rs` resolves semantic identities to the one
  plan type table while admitting complete call plans. The appended binding
  and applied function checks require that context and remain there. Its
  existing embedded construction tests are unchanged. No persistence or
  engine state is added to construction.
- AWBC `schema.rs` owns a narrow accessor on its existing logical argument
  enum. `verify/code.rs` continues to own instruction/terminator validation
  against the bytecode type table and verifier accounting. This adds no schema
  fields, second runtime type table, runtime resolver or cross-layer import.
- AWBC `tests.rs` keeps the real ProjectCall fixture and negative mutations
  together with the existing codec/verifier/VM harness. Replacing the fake
  closure producer deletes the unrelated function-body fixture. Tests use
  existing public constructors rather than widening private type fields.

All other changed owners stay below their applicable review thresholds. No
file grew by 300 LOC. Native construction, sema inference and bytecode
verification retain separate legitimate contexts; no source/HIR dependency
was added to core, and no I/O entered a Sans-I/O owner. Canonical screening,
the blocking gate and fresh dependency measurements are recorded with the
final validation results.

Fresh Cargo metadata gives normal workspace dependency fan-in/out of core
29/6, sema 8/14 and compiler 3/23; development fan-in/out are 3/6, 3/0 and 1/5
respectively. There are no manifest, dependency, feature or layer-direction
changes. The canonical tool ran without writing generated reports; the exact
touched-file measurements and owner dispositions above are retained instead.
All 71 review ZIPs (4,802,433 bytes) were re-enumerated and compared with the
preceding cut: no path, length or SHA-256 difference. Frozen packages are
unchanged.

## Remaining work

This cut does not close declaration summaries with inferred callback rows,
correlated parent/child source constraints, function-scheme specialization,
ordinary application of runtime project-continuation values, scoped runtime
nominal types or program-bound restore. Their joint design and implementation
remain in the existing [coupled callable request](../reviews/requests/2026-09-08-aw-ah-009.4.2.1.1.1-function-scheme-specialization-and-callable-value-execution.md).
The investigation remains proposed; Generic Match C3/C5, retained View,
RuntimePlan/task-plan, nominal C1-C6 and scheduler/restore acceptance are not
awarded completion credit. There is no external blocker.
