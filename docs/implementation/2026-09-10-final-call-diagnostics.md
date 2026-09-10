# Final call diagnostics from the sealed outcome

Date: 2026-09-10. Base: clean `main` at
`d7b8d5f2c1fabb92ab3ac2544eed8f8a53ae15ab`, equal to `origin/main`.
Implementation and validation below refer to the working copy at that base.
This continues the [application-group cut](2026-09-10-function-application-arity.md),
the [callable model](2026-09-10-callable-convergence-model.md), and the active
[convergence goal](2026-09-08-convergence-goal-plan.md).

## Selected authority and implementation

Final call facts already owned the semantic outcome, but every prepared
producer supplied an empty diagnostic vector. Rejected calls could therefore
reach the compiler's later missing-selected-call guard without a source-backed
call diagnostic. Non-callable facts were staged and then abandoned through an
`ExpressionTypeUnavailable` error; final validation also rejected their tooling
outcome unconditionally.

`CallTargetFacts::try_new` now derives the mandatory diagnostic from its final
`CallAnalysisOutcome`. Selected outcomes have no rejection diagnostic;
ambiguous, rejected, non-callable and missing outcomes each have one primary
error. Its source is the exact owning module's final-HIR Whole expression
anchor. Source-query failures retain their typed cause, and a diagnostic limit
of zero rejects the generation rather than dropping its error. Candidate,
detached and intermediate seal records no longer carry caller-supplied
diagnostic vectors. The final fact remains the single owner; no peer call or
diagnostic catalog is added.

`CallableDiagnostic` owns the shared source projection and its code enum owns
the stable labels and messages. The compiler consumes final call errors at
TypeCheck before verification or runtime lowering, with the accepted source
document retained. CLI and LSP use that same source diagnostic. The maintained
[LSP chapter](../04-tooling/lsp.md) records this ownership and phase order.

The query result now explicitly distinguishes callable candidates from a
non-callable target. Non-callable results return the existing unavailable
expression state, as rejected/ambiguous calls do. Their authored arguments,
and arguments of associated-receiver recovery, are checked once in the same
fact transaction using its expression authority. This creates no candidate
probe or fallback callable schema; an argument's source failure still aborts
the transaction. Associated-receiver recovery also stops supplying a fabricated
value result. Final validation admits sealed unselected tooling outcomes while
requiring their unavailable result, and preserves source/type validation. The
existing execution-plan sealer still gives them no plan.

The old project-binding test expected `ExpressionTypeUnavailable`. It is now
with the call diagnostic tests and requires the retained non-callable error,
zero candidate probes and an empty physical candidate trace, one retained
authored argument, and absent result/execution authority. Positive generic or
callback acceptance tests were not weakened.

## Performed validation

Logs and command timings are in
`.arcweft-local/validation/2026-09-10-final-call-diagnostics/`.
All Cargo commands ran sequentially with normal Cargo concurrency. Focused
crate commands enabled all features; Justfile gates use their declared feature
sets. No Cargo job override, clean, branch or worktree was used in this cut.

The eight final call-diagnostic tests cover final outcome/source equality,
signature projection, discarded-candidate isolation, inclusive diagnostic
limits, foreign HIR source rejection, argument fact retention/source failure,
poisoned-type precedence and non-callable project-binding accounting. The
compiler regression requires exactly one TypeCheck diagnostic with the original
`factory(1i64, 40i64)` span. The LSP regression independently checks a rejected
call's span in UTF-8/UTF-16 modes and that verification did not run. CLI checks require a single
diagnostic for excess arguments and a non-callable value.

| Command or validation | Observed result |
| --- | --- |
| Focused `cargo test -p arcweft-lang-sema --all-features --lib final_call_diagnostics` | Seven new tests passed in the focused run; the moved eighth test passed in the subsequent full library run |
| `cargo test -p arcweft-lang-sema --all-features --lib --no-fail-fast` | Final run: 778 passed / 9 known failures; 15.357 s including rebuild |
| Exact compiler application-group test | Passed: 1 test; 44.904 s |
| Compiler all-feature library, `callable_execution`, `evaluated_effects`, `project_cache_transaction` with `--no-fail-fast` | Library 82, effects 17 and cache 20 passed; callable 57 passed / 24 known failures; 34.761 s |
| Exact LSP call-range test | Passed: 1 test, including both encodings |
| `cargo test -p arcweft-lsp --all-features --lib --no-fail-fast` | Passed: 218 tests; 28.719 s |
| `cargo test -p arcweft-cli --all-features --test check_core_cli --no-fail-fast` | Passed: 5 tests; 175.860 s including all-feature compilation |
| `cargo check --workspace --all-targets --all-features` | Passed; 42.398 s |
| `cargo clippy --workspace --all-targets --all-features` | Final run passed with existing warnings; the three new warnings were removed; 24.343 s |
| `just test-workspace` | Failed at callable execution: 57 passed / 24 known failures; 279.530 s; later workspace targets and the recipe's CLI commands did not run |
| `just test-doc` | Passed: 8 tests; 86.597 s |
| `just structure-audit` / `just structure-audit-gate` | Passed: 95 packages, 2,267 Rust files, 310 review triggers, zero blocking violations; 3.530 / 2.413 s |
| `cargo fmt --all -- --check` | Passed; 10.159 s |
| Whitespace / relative document links | Working-copy and staged whitespace passed; 14 relative targets resolve, none missing |
| `cargo metadata --no-deps --format-version 1 --all-features` | Passed; structured dependency counts below |

Initial compilation used the wrong source severity variant and compared a
typed diagnostic code directly with a string. Both were corrected to the
existing APIs. One accidentally shortened `--exact` filter selected zero tests;
it is not acceptance evidence. The first outcome matrix exposed the abandoned
non-callable publication and unconditional final rejection described above.
The associated-receiver fixture has a poisoned nominal type, so its correct
assertion preserves `PoisonedType`; it does not pretend that an unknown-call
report should replace the earlier failure. The full sema run then exposed the
obsolete project-binding expectation, which was migrated with its physical
trace checks. No failure was ignored.

The first workspace Clippy run identified a 432-byte query enum variant and
two redundant test closures. The query payload is now boxed, and the tests
use the existing `DiagnosticCode::as_str` method directly. These changes do
not add another semantic owner. Validation after these corrections is recorded
in the final rows above. The full sema run, workspace Clippy and workspace tests
ran after the boxing/closure corrections. The separate LSP/CLI runs preceded
those representation-only corrections and were not repeated.

The nine sema failures remain contextual/correlated inference (five) and
implicit callback effect rows (four). The 24 compiler callable failures remain
the required positive matrix from the preceding cut. This frontend diagnostic
cut does not alter runtime execution, rendering, Agent/MCP, capture, codecs or
saved state. Tier 2 and visual goldens are not selected for this cut; the
previous cut's Tier 2 result is not counted as a fresh pass.

## Structural review

Measurements compare complete files with the base SHA, not diff additions.
Paths identify the owning crate. Embedded test LOC use the canonical scanner's
convention as review evidence, not an acceptance gate.

| Path | Class | Base LOC | Current LOC | Bytes | Embedded test LOC |
| --- | --- | ---: | ---: | ---: | ---: |
| crates/arcweft-cli/tests/check_core_cli.rs | Test | 62 | 90 | 3007 | 0 |
| crates/arcweft-compiler/src/project.rs | Production | 1708 | 1723 | 61332 | 0 |
| crates/arcweft-compiler/tests/callable_execution.rs | Test | 678 | 687 | 17914 | 0 |
| crates/arcweft-lang-sema/src/callable/error.rs | Production | 689 | 691 | 25964 | 0 |
| crates/arcweft-lang-sema/src/callable/facts.rs | Production | 1265 | 1267 | 41423 | 0 |
| crates/arcweft-lang-sema/src/callable/facts/diagnostics.rs | Production | 0 | 147 | 7801 | 0 |
| crates/arcweft-lang-sema/src/final_analysis/analyzer/call_seal.rs | Production | 2015 | 2025 | 88391 | 0 |
| crates/arcweft-lang-sema/src/final_analysis/analyzer/calls.rs | Production | 4312 | 4290 | 186084 | 156 |
| crates/arcweft-lang-sema/src/final_analysis/analyzer/calls/constraints.rs | Production | 4966 | 4956 | 202310 | 480 |
| crates/arcweft-lang-sema/src/final_analysis/error.rs | Production | 1005 | 1011 | 37439 | 64 |
| crates/arcweft-lang-sema/src/final_analysis/tests.rs | Test | 9044 | 9046 | 312043 | 0 |
| crates/arcweft-lang-sema/src/final_analysis/tests/call_diagnostics.rs | Test | 0 | 262 | 9377 | 0 |
| crates/arcweft-lang-sema/src/final_analysis/validation.rs | Production | 2779 | 2786 | 115173 | 0 |
| crates/arcweft-lsp/src/diagnostics.rs | Production | 1321 | 1351 | 50332 | 933 |

Touched review-trigger dispositions:

- Compiler `project.rs` owns the ordered compilation transaction and accepted
  source leases. Its added gate uses the existing diagnostic projection and
  error/source retention path; it adds no transport or parallel analyzer.
- Callable `facts.rs` owns immutable final facts and constructor validation.
  The new child module owns final-outcome diagnostic issuance and source
  rendering on the same types; it owns no separate state or facade.
- `call_seal.rs` retains the consuming C sealer and atomic pending-fact
  publication. Removing intermediate diagnostic fields and using the exact HIR
  module completes that boundary without a second publication pass.
- `calls.rs` retains resolver/candidate/fact-transaction orchestration. The
  zero-candidate source staging uses the existing transaction and expression
  authority. Its diagnostic behavior test moved to the final-analysis harness;
  the remaining 156 embedded test lines cover private call-frame/type helpers.
- `calls/constraints.rs` retains affine prepared/detached constraint evidence.
  Its only change deletes diagnostic fields and projections. The existing 480
  test lines exercise private constraint admission; no public API was widened
  to move them.
- Final `validation.rs` owns generation-wide fact consistency. The changed
  checks distinguish tooling outcomes from execution and require unavailable
  results; source, type and candidate validation remains with this owner.
- Final-analysis `tests.rs` retains its shared fixture and registration
  harness. Only a child-module declaration was added; the diagnostic tests use
  that harness without duplicating production registration.
- LSP `diagnostics.rs` owns exact-revision source projection and its accepted
  project fixture. Its only change is a test of the shared compiler output.
  The existing private fixture and 933 embedded test lines remain with that
  projection boundary; no LSP implementation or public API was widened.

No file grew by 300 LOC, no dependency or feature changed, and all contract
versions remain 1. The source measurement snapshot covers 14 changed Rust
files / 1,158,590 bytes with exact lengths and SHA-256 values. All 71 review
ZIPs (4,802,433 bytes) were re-enumerated without path, length or hash changes.
Fresh Cargo metadata gives normal workspace dependency fan-in/out of sema 8/14,
compiler 3/23, LSP 0/23 and CLI 0/53. Development fan-in/out are 3/0, 1/5, 1/3
and 0/3 respectively. The canonical screen and gate ran without writing
generated reports, and both reported zero blocking violations.

## Remaining scope

This closes final call diagnostic issuance and source presentation through
compiler/CLI/LSP. It does not close the complete
[coupled callable request](../reviews/requests/2026-09-08-aw-ah-009.4.2.1.1.1-function-scheme-specialization-and-callable-value-execution.md).
In particular, interactive LSP signature acquisition still requires an
executable product, so it cannot acquire the retained semantic report after a
compilation failure. The semantic signature API's passing projection tests are
not claimed as that missing interactive lifetime implementation.

The semantic/tooling lease, correlated source constraints, effect algebra,
schemes/specialization, reusable activation, scoped nominal runtime types and
program-bound restore remain coupled work. Later Match, View, task-plan,
nominal and scheduler acceptance also remains required. The model stays
proposed and the goal stays active. No external blocker or released-format
compatibility exception was introduced.
