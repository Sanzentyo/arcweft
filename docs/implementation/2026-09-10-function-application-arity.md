# One function application consumes one argument group

Date: 2026-09-10. Inspected clean `main` at
`73f8283ceb6dc5bb1b92f557de21e6ccfbe81a08`, equal to `origin/main`.
Implementation and validation below refer to the dirty checkout at that base.
The preceding [curried effects and ABI cut](2026-09-10-curried-application-effects-and-abi.md)
is committed and pushed. This continues the
[callable model](2026-09-10-callable-convergence-model.md) and the active
[convergence goal](2026-09-08-convergence-goal-plan.md).

## Adjudicated application rule

The current typed runtime-plan constructor validates an Apply against exactly
one function arrow's parameter/result row. The maintained language preserves
call groups, and the AWBC VM rejects excess arguments before entering a
callee. Native expression, pure and flow application nevertheless split an
oversupplied argument list at the current arity, ran the first body, and
recursively applied its returned function to the remaining values. For a
non-function result, the error happened only after that first execution.

The selected rule is that an application never transfers excess values into
a returned function. A later group is a separate explicit application.
This preserves the declared group distinction and agrees with the admitted
typed plan; it does not change the rule because one backend is easier to
implement. Existing supported prefix binding for fewer values remains a
non-invoking operation. Completing a group can return a function, which is
applied separately. The maintained language chapter now states the excess
argument rule explicitly.

## Implementation

The native expression and pure evaluators pass the full current argument
slice to their existing exact-arity invocation validation. Neither executes
the first body and recursively applies its result. The flow application
entry rejects excess arguments before installing parameter values or creating
an executable frame. Its result continuation now owns only the result pattern;
the extra-argument payload, both producers and the post-return replay branch
were deleted. ProjectCall target/default return behavior retains the same
frame and result-binding authority. AWBC already has the required rejection
and needs no production change for this rule.

The three native owner tests use one typed, fully built plan whose first
function returns a second function. They call each actual evaluator entry,
requiring excess arguments to reject instead of applying both functions.
The expression and pure tests also execute the two separate applications.
The flow test compares the complete fiber and output before and after the
failure; the pure test requires zero evaluated body expressions on failure.
The shared fixture exposes only test APIs, and obtains final site/pattern
coordinates from the admitted plan instead of manufacturing raw IDs.

A semantic test retains the exact rejected one-parameter function candidate
and proves that it has no selected application or execution plan. A compiler
test uses the existing nested executable-closure example: separate applications
compile, while the merged groups are rejected before execution. The current
compiler reports the latter through its missing-selected-call projection
guard, because final call diagnostics are still empty. An initial test
incorrectly assumed a TypeCheck-stage diagnostic; the test now checks the
actual obligation (semantic rejection of valid syntax) alongside the direct
sema evidence. This does not close the separate diagnostic-publication gap in
the coupled request.

## Validation

Logs are under `.arcweft-local/validation/2026-09-10-function-application-arity/`.
All Cargo commands ran sequentially using normal Cargo concurrency. Focused
runs consistently enabled all features; the required Justfile recipes retained
their own feature sets.

Initial native fixture compilation failed because it used an empty backend
and a nonexistent flow-field accessor. It now uses the actual VM call backend
and admitted executable-body API. The next fixture run failed on an invalid
canonical flow name containing the reserved family prefix; the valid canonical
name fixes it. All three tests then pass. The source test's initial diagnostic-
stage assumption and its correction are explained above; the direct semantic
rejection assertion is independent evidence, not an inferred stage label.

| Performed validation | Result |
| --- | --- |
| `cargo test -p arcweft-core --all-features --lib surplus_arguments` | Passed: 3 tests; 7.934 s |
| Exact sema `surplus_function_arguments_leave_no_selected_application` | Passed: 1 test, 779 filtered; 64.421 s including rebuild |
| Exact compiler `function_value_calls_do_not_merge_argument_groups` | Passed: 1 test, 80 filtered; 8.132 s |
| `cargo test -p arcweft-core --all-features --lib` | Final run passed: 364 tests, including the existing AWBC partial/excess-application tests and native await/return test; 9.209 s |
| `cargo test -p arcweft-lang-sema --all-features --lib --no-fail-fast` | 771 passed / 9 known failures; 1.846 s |
| `cargo test -p arcweft-compiler --all-features --lib --test evaluated_effects --test project_cache_transaction --test callable_execution --no-fail-fast` | Library 82 passed; effects 17 passed; cache 20 passed; callable 57 passed / 24 known failures; 15.274 s |
| `cargo check --workspace --all-targets --all-features` | Passed; 41.934 s |
| `cargo clippy --workspace --all-targets --all-features` | Final run passed with existing warnings; 15.725 s |
| `just test-workspace` | Failed at callable execution: 57 passed / 24 known failures; 368.973 s; subsequent workspace targets and CLI recipe commands not run |
| `just test-doc` | Passed: 8 tests; 94.935 s |
| `just test-tier2` | MCP stdio 4 passed, native observe/capture 1 passed, then the known `samples/image-animation.arcw:1:1` parse/recovery failure; 50.279 s |
| `cargo fmt --all` followed by `cargo fmt --all -- --check` | Passed; 20.241 s combined in final review |
| `just structure-audit` / `just structure-audit-gate` | Final runs passed: 95 packages, 2,265 Rust files, 310 review triggers, zero blocking violations; 2.445 / 2.337 s |
| `cargo metadata --no-deps --format-version 1 --all-features` | Passed; 0.136 s |
| Relative documentation links / `git diff --check` | 12 file targets resolved with none missing; whitespace check passed |
| Final `git diff --cached --check` / `cargo fmt --all -- --check` | Passed after normalizing two test-file line endings; formatting 10.357 s |

The first Clippy run passed with one new warning: the shared typed-plan fixture
has 109 counted lines. Its one complete plan construction remains together
with an explicit, fulfilled lint expectation explaining that ownership.
Only that test lint attribute changed after the broader gates. Formatting,
workspace Clippy, the full core library tests and both structural commands
were repeated afterward; production Rust was unchanged.

The first staged whitespace check found one stray carriage return in each of
the two new engine test files. Both were normalized to their existing LF line
endings, and the staged check and formatting check passed afterward. This
changed no Rust tokens or behavior; the full test matrix was not repeated for
that correction.

The nine sema failures remain the three contextual-constructor, two correlated
ordinary-call and four inferred-callback-row requirements. The 24 callable
failures remain the preceding matrix's required positive cases. They were
not ignored or changed to expected rejection. Tier 2 stops at its first
auxiliary image test; later auxiliary capture, visual goldens and Select/Flow
production-limit recipes were not run. The sample grammar and these remaining
requirements were not changed in this cut.

## Structural review

The complete current file is compared with the complete file at the base SHA.
The path identifies its crate owner. Embedded test LOC follow the canonical
scanner's convention and are review evidence, not source-spelling gates.

| Path | Class | Base LOC | Current LOC | Bytes | Embedded test LOC |
| --- | --- | ---: | ---: | ---: | ---: |
| crates/arcweft-compiler/tests/callable_execution.rs | Test | 650 | 678 | 17567 | 0 |
| crates/arcweft-core/src/engine.rs | Production | 2056 | 2055 | 77792 | 0 |
| crates/arcweft-core/src/engine/eval/function.rs | Production | 227 | 217 | 8321 | 0 |
| crates/arcweft-core/src/engine/flow.rs | Production | 1566 | 1565 | 61388 | 0 |
| crates/arcweft-core/src/engine/flow/function_call.rs | Production | 320 | 311 | 11946 | 0 |
| crates/arcweft-core/src/pure.rs | Production | 2992 | 2984 | 110706 | 132 |
| crates/arcweft-core/src/tests.rs | Test | 16 | 17 | 420 | 0 |
| crates/arcweft-lang-sema/src/final_analysis/tests/callable_values.rs | Test | 241 | 271 | 8559 | 0 |
| crates/arcweft-core/src/engine/eval/function/tests.rs | Test | 0 | 41 | 1371 | 0 |
| crates/arcweft-core/src/engine/flow/function_call/tests.rs | Test | 0 | 42 | 1377 | 0 |
| crates/arcweft-core/src/pure/function_application_tests.rs | Test | 0 | 35 | 1238 | 0 |
| crates/arcweft-core/src/tests/function_application.rs | Test | 0 | 148 | 5782 | 0 |

The touched size triggers have these ownership dispositions:

- `engine.rs` retains the function return continuation beside the same-fiber
  state that owns it. Removing the extra-argument field removes state; no
  parallel invocation, return or snapshot representation is introduced.
- `engine/flow.rs` retains ProjectCall orchestration. Its only change uses the
  same result-only return continuation. The complete function-value entry and
  return handling remains in `engine/flow/function_call.rs`, whose owner test
  checks rejection before frame/environment mutation.
- `pure.rs` retains the plan/environment/statistics-owned pure evaluator. This
  change deletes result reapplication and delegates the full argument row to
  its existing exact invocation guard. It adds no alternate body owner, I/O or
  transport responsibility. Its existing 132 embedded test lines belong to
  opaque-record projection; they are unchanged. The new application test is a
  child module with access to the evaluator's private entry and statistics.
  Splitting unrelated pure arithmetic or record state would not strengthen
  this application boundary and is not part of this cut.

Other changed files stay below their applicable thresholds, and no file grew
by 300 LOC. The shared function-plan fixture is test-only and supports the
three legitimate evaluator contexts without widening a production API.
Compiler and sema tests use their existing integration and final-analysis
harnesses. No dependency, feature, facade, serialized contract field or
version changes. All Arcweft-owned contract markers remain at 1.

Fresh Cargo metadata retains the same normal workspace dependency fan-in/out:
core 29/6, sema 8/14 and compiler 3/23; development fan-in/out remain 3/6, 3/0
and 1/5. The canonical scanner ran without writing generated reports. All
71 review ZIPs (4,802,433 bytes) were re-enumerated with zero path, length or
SHA-256 differences. The local `source-inputs.json` snapshots 22 relevant
Rust input files (1,366,246 bytes) with their exact paths, lengths and hashes.
Frozen packages were not changed.

## Remaining coupled work

This fixes the application-group rule and removes its contradictory native
success path. It does not finish the
[coupled callable request](../reviews/requests/2026-09-08-aw-ah-009.4.2.1.1.1-function-scheme-specialization-and-callable-value-execution.md).
Correlated source constraints, effect quantification/union, complete value
origins, function-scheme specialization, shared callable-value activation,
scoped nominal runtime types and program-bound restore still need their
complete joint model and implementation. The proposed model is not declared
ready. Later Match, View, task-plan, nominal and scheduler acceptance remains
required. There is no external blocker.
