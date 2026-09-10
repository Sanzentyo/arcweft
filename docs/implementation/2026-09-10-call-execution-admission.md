# Unselected calls have no execution plan

Date: 2026-09-10. Inspected existing `main`, initially clean, at
`95c06a819a2ed7b3399a8c24e397aaf9f3908390`; `origin/main` was the same revision.
Implementation and validation below are from the dirty checkout at that base.
The [callable convergence investigation](2026-09-10-callable-convergence-model.md)
exposed an unselected ordinary call being projected as a structural runtime
expression. This cut repairs that expression-plan authority. It does not
complete the correlated-call inference model or the [convergence goal](2026-09-08-convergence-goal-plan.md).

## Selected boundary and implementation

An analysis report can retain a rejected or ambiguous call for tooling without
having an execution plan for it. `CheckedExpressionResult::Unavailable` already
expressed that lack of a value. The former `RejectedCall` structural-execution
reason incorrectly supplied an executable retention decision (`Omit`) anyway.

`CheckedExpression` now retains an optional execution plan. Its unavailable
call constructor owns no plan. The owning C2 sealer verifies the exact ordinary
call site, unselected outcome and unavailable result before preserving that
absence; a selected application paired with an unavailable result still fails.
An unavailable call cannot acquire evaluated-effect roles or a dialogue consumer.
The old structural `RejectedCall` reason and both producer paths were removed.

The complete execution-plan sealer and its role-collection helpers now live in
the private `final_analysis/execution_plan.rs` owner. Nominal C2 orchestration
calls this phase once. This separates plan/effect/dialogue-consumer state from
nominal schema construction without copying an algorithm or widening a facade.
The consistency test moved with that owner into `execution_plan/tests.rs`; it
still rejects both a selected call with an unavailable result and an unselected
call with a forged available result. Its private production helper stayed private.

The existing fallible `FinalAnalysisExecutionProjection` now owns `plan(owner)`.
It distinguishes an absent expression from an unselected call and rejects the
latter with `FinalAnalysisExecutionProjectionError::UnselectedCall`. Expression
projection and complete executable fact partitioning use that same admission.
The report's raw optional getter remains useful to tooling; absence cannot be
interpreted as a successful instruction to omit a value.

Compiler reachability now consumes the sema expression projection, and runtime
call lowering borrows the admitted plan. Neither reads an unavailable raw plan
and constructs a structural HIR runtime projection. The compiler retains the
typed projection error and uses its existing missing-selected-call diagnostic
code for the unselected case. Prepared source expressions likewise contribute
no runtime type when no execution plan exists.

This is an availability boundary over the existing expression and call owners,
not a second semantic catalog, another call resolver or a new runtime fallback.
It remains necessary when the correlated solver later selects the positive
regressions successfully. No language restriction, compatibility reader,
contract-version change, dependency or I/O boundary was introduced.

## Validation

The tests inspect real rejected overload and ambiguous-call reports. They
retain the original candidate/evidence assertions and additionally require:

- no result value or raw execution plan for the unselected call;
- typed rejection from both the plan and expression projection APIs; and
- an ordinary selected call to retain its application-bound execution plan.

The initial new fixture incorrectly expected missing-name and non-callable
programs to return tooling reports. Those sources already fail earlier with
`UnknownCallTarget` and `ExpressionTypeUnavailable`. The fixture was replaced
with real registered overload candidates whose rejection remains in a report;
production handling of the earlier failures was preserved.

The first check after extracting the sealer failed because its internal test
still referenced the old module. Moving that test to the execution-plan owner
and removing the unused nominal import resolved the check failure; the earlier
failed log remains available.

Logs are under `.arcweft-local/validation/2026-09-10-call-execution-admission/`.

| Performed validation | Result |
| --- | --- |
| `cargo check -p arcweft-lang-sema -p arcweft-compiler --all-targets --all-features` | Passed; 33.62 s |
| `cargo test -p arcweft-lang-sema --lib --all-features --no-fail-fast`, `final-sema.log` | 764 passed, 9 existing failures; 21.846 s including rebuild / 1.31 s test execution |
| Exact `unselected_call_outcomes_never_grant_execution`, `unselected-outcomes-3.log` | 1 passed; 13.95 s including rebuild |
| `cargo test -p arcweft-compiler --all-features --lib --test evaluated_effects --test project_cache_transaction --test callable_execution --no-fail-fast`, `final-compiler.log` | Library 82 passed; evaluated effects 17 passed; cache transactions 20 passed; callable execution 54 passed / 22 existing failures; 33.158 s |
| `cargo check --workspace --all-targets --all-features`, `final-check-2.log` | Passed; 18.70 s including command startup |
| `cargo clippy --workspace --all-targets --all-features`, `final-clippy-2.log` | Passed with existing warnings; check and Clippy together 44.03 s |
| `just test-workspace`, `workspace-tests.log` | Failed after 279.961 s at callable execution: 54 passed / 22 existing failures; subsequent workspace targets and CLI recipe commands were not run |
| `just test-doc`, `doctests.log` | Passed: 8 tests, none ignored; 73.432 s |
| `just structure-audit` / `just structure-audit-gate` | Both passed; 95 packages, 2,260 Rust files, 310 review triggers, zero blocking violations; 2.674 / 2.440 s |
| `just test-tier2`, `tier2.log` | Failed after 57.247 s: MCP stdio 4 passed and native observe/capture 1 passed; first auxiliary image test failed at the existing `samples/image-animation.arcw` declaration parse/recovery error |
| `cargo fmt --all -- --check`, `final-fmt.log` | Passed; 10.712 s |
| `cargo metadata --no-deps --format-version 1 --all-features` | Passed; current dependency graph captured locally |
| Documentation links / `git diff --check` | 43 relative targets resolved; whitespace check passed |

The nine sema failures are the original three contextual constructors, four
inferred effect rows and the two newly recorded ordinary correlated-call
requirements. The 22 compiler failures are the preceding 18 cases plus the four
ordinary correlated-call native/AWBC requirements. These remain positive tests;
they were not changed to expect rejection or skipped. The `empty()` compiler
pair now receives the typed missing-selected-call error instead of a structural
Call edge. It still cannot execute until its type constraints are completed.

This cut does not populate the missing callable diagnostic rows or solve
parent/child constraints, effect quantification, function-scheme specialization,
runtime scoped types or callable-value activation. Those obligations remain in
the [existing coupled request](../reviews/requests/2026-09-08-aw-ah-009.4.2.1.1.1-function-scheme-specialization-and-callable-value-execution.md).
No external blocker or completed whole-goal acceptance is claimed.

The final sema and compiler commands were rerun after the sealer and its test
moved. Their failure-name sets exactly match the preceding focused runs (nine
and 22 respectively). The earlier `sema-1.log` iteration had 763 passes and ten
failures, including the invalid new fixture, and `unselected-outcomes-2.log`
had one fixture failure; neither is represented as passing validation. Cargo
commands ran sequentially with normal concurrency and no explicit job count.
The earlier `workspace-check-clippy.log` passed before extraction; the subsequent
`final-check-clippy.log` failed on the moved test reference described above.
Only `final-check-2.log` and `final-clippy-2.log` describe the final check/lint
state. Later auxiliary captures, visual goldens and production Select/Flow
boundaries were not run after the Tier 2 recipe stopped. No required tier is
reported as fully passing when its recipe stopped at an existing failure.

## Structure review

Measurements compare complete files at the inspected base with this dirty
checkout. All paths below are relative to the named crate's `src/` directory.
These are production and test sources, not generated reports. None of the
touched production files contains an embedded test-module body. The raw
measurements are in the local validation directory's `structure-measurements.json`.

| Crate | Path | Class | Base LOC | Current LOC | Bytes |
| --- | --- | --- | ---: | ---: | ---: |
| arcweft-compiler | `lower.rs` | Production | 7861 | 7854 | 332152 |
| arcweft-compiler | `lower/reachability.rs` | Production | 741 | 744 | 28633 |
| arcweft-lang-sema | `final_analysis.rs` | Production module interface | 216 | 217 | 11868 |
| arcweft-lang-sema | `final_analysis/model.rs` | Production | 2807 | 2807 | 91561 |
| arcweft-lang-sema | `final_analysis/nominal_schema.rs` | Production | 2946 | 2625 | 107086 |
| arcweft-lang-sema | `final_analysis/nominal_schema/tests.rs` | Test | 704 | 668 | 22700 |
| arcweft-lang-sema | `final_analysis/prepared.rs` | Production | 1566 | 1564 | 51283 |
| arcweft-lang-sema | `final_analysis/report.rs` | Production | 2264 | 2275 | 91334 |
| arcweft-lang-sema | `final_analysis/tests.rs` | Test | 9046 | 9044 | 311983 |
| arcweft-lang-sema | `final_analysis/tests/callable_values.rs` | Test | 154 | 207 | 6431 |
| arcweft-lang-sema | `final_analysis/tests/generic_calls.rs` | Test | 373 | 376 | 12481 |
| arcweft-lang-sema | `final_analysis/execution_plan.rs` | Production | 0 | 341 | 14184 |
| arcweft-lang-sema | `final_analysis/execution_plan/tests.rs` | Test | 0 | 44 | 1437 |

The touched review triggers have these dispositions:

- `nominal_schema.rs` / `execution_plan.rs`: decompose at the complete plan
  publication phase, including its role accumulation, dialogue joins and
  consistency test. The remaining `RuntimeNominalProjectionContext`, schema
  expansion and C2 orchestration retain their shared nominal validation and
  projection state. No nominal schema logic was copied into the new module.
  Its only production entry is `pub(super) seal`, called once by C2; the
  module interface does not re-export it.
- `model.rs`: retain the checked-expression payload and its result/plan
  invariants together. This cut removes the invalid structural reason and
  narrows the plan setter. Splitting the enum, data and methods by file size
  would not establish a different owner or state boundary.
- `prepared.rs`: retain the typed prepared-to-complete expression state
  transition. Its runtime-type query now handles unavailable execution
  explicitly; it owns no alternative call catalog or inference state.
- `report.rs`: retain the final generation's borrowed execution projection.
  Plan admission and executable partitioning read the same checked-expression
  map. The new method adds no stored state, traversal or secondary authority.
- Compiler `lower.rs`: retain the existing cross-layer inventory join at
  `project_runtime_semantic_fact_inventories`; this change removes its raw
  expression lookup and consumes sema's fallible admission. The companion
  reachability adapter continues to own the sema-to-HIR conversion required by
  dependency direction. Neither owner acquires semantic inference, transport,
  persistence or additional schema state in this cut.
- `final_analysis/tests.rs`: retain the existing overload ambiguity and
  registered-call rejection fixtures with their candidate/accounting evidence.
  Their additional assertions share the callable-value test helper; the
  internal execution-sealer test moved to its production owner's child
  module. Test access did not widen a production API.

There are no dependency, Cargo feature, contract-version or I/O changes.
Fresh Cargo metadata gives compiler workspace fan-in/out of 3/23 (development
1/5) and sema 8/14 (development 3/0), unchanged from the preceding retained
audit. Sema continues to own execution admission and compiler owns its HIR
adapter; no reverse dependency was introduced. The canonical audit ran without
writing generated reports; exact touched-file measurements and ownership
decisions above are retained instead.
All 71 review ZIPs were re-enumerated (4,802,433 bytes); path, byte size and
SHA-256 comparison with the preceding cut found zero differences. Frozen
packages were not modified.
