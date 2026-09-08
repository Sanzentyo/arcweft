# Rejected call value evidence and branch context — 2026-09-09

Supersedes the current-state portion of
[candidate completion and contextual sources](2026-09-09-candidate-completion-and-contextual-sources.md).
The earlier validation record remains historical evidence.

Inspected HEAD and fetched `origin/main` are
`4da7bfd7a9f8324f3763d9e579fce1f0ab38965d`, with divergence 0/0. Work continues
in the existing `main` checkout. Before this follow-up's documentation was
added, the index was empty and the checkout contained 8 deleted tracked paths,
646 modified tracked paths and 97 untracked status entries, including the new
audit directory. These totals include inherited connected work.

## Established behavior

`publish_recovery_call` previously returned the primary candidate's result
schema as a normal value even when no candidate was selected. It also returned
a successful content-emission result for rejected emission calls. Both paths
are removed. Rejected and ambiguous calls now have
`CheckedExpressionResult::Unavailable`: no value type, no type-selection
evidence, and an omitted runtime result with the existing `RejectedCall`
execution reason. Candidate signatures, considered/tied candidates and
accounting remain on the diagnostic call facts.

The expression validator admits this result only for an unselected call. The
execution-plan owner checks the relation in both directions: a rejected call
cannot carry successful value evidence, and a selected call cannot carry an
unavailable result. The semantic transcript encodes the new result variant
within its existing version-1 contract. A source callback that receives no
value reports a semantic mismatch; it no longer fabricates an invalid-variant
invariant. Actual malformed variant evidence still has its own checks.

Removing recovery values exposed a separate branch-context defect. With no
outer expectation, `if` and `if let` had used the first branch's inferred type
as a complete expectation for the second. Thus `alice()` followed by `bob()`
rejected the second factory against Alice's exact dialogue type, but recovery
then supplied a value that made the join appear successful. Both branches now
receive the parent's expectation, and the existing common-type operation joins
their inferred types. No Character-specific inference exception was added.

The existing branch/reconfiguration/collection/capture semantic test now
requires every call to be selected and passes. Rejected and ambiguous call
tests require unavailable results, and the new execution-plan test forges both
opposite result/selection combinations and verifies `CallFactMismatch`.

## Remaining implementation and design work

The three contextual constructor tests still fail. Removing fabricated
recovery values prevents declaration-Free nominal keys from being admitted as
the child's value; it does not provide the missing partial constraints between
parent and child. The failures now report `ExpressionTypeUnavailable` rather
than accepting recovery typing. Complementary `.Left(1i64)` and `.Right("two")`
sources still require shared inference evidence in both argument orders.

The four invoked/inferred callback semantic cases and the compiler's source
effect projection gap also remain. Source-local existential completion,
universal function schemes, correlated nested candidates, effect constraints,
materialization and callable-value execution remain coupled in
[AW-AH-009.4.2.1.1.1](../reviews/requests/2026-09-08-aw-ah-009.4.2.1.1.1-function-scheme-specialization-and-callable-value-execution.md).
No new source-scope registry, existential carrier, effect-row algebra, runtime
function adapter or alternative solver was implemented or accepted here.

The new `character_factory_branches_keep_both_selected_calls` native/AWBC pair
adds executable coverage of both branch outcomes inside a project function.
It compiles through semantic analysis, then fails runtime semantic projection:
the Dialogue callable family has no typed runtime intrinsic. The compiler
currently ignores factory/reconfiguration expression facts when building
value facts, routes those call applications through ordinary target dispatch,
and represents the dialogue type as an opaque runtime type without providing
the value construction/reconfiguration execution. Dynamic dialogue application
also explicitly requires missing typed runtime-plan lowering. These are
implementation gaps; the pair is retained without changing its expected
success or adding an expression-evaluation fallback.

General bidirectional branch inference, partial source completion, all
callable-value cases and downstream Match, View, RuntimePlan, nominal and
scheduler/restore acceptance remain required by the
[active goal](2026-09-08-convergence-goal-plan.md). There is no external blocker.
No language restriction, compatibility path, version increment or layer/Sans-I/O
deviation was adopted. No implementation commit or push occurred: the connected
cut is not complete.

## Validation actually run

| Command / scope | Result |
| --- | --- |
| `cargo test -p arcweft-lang-sema --lib -- --nocapture` | Failed: 743 passed / 7 failed / 750 total / 0 ignored. The new result/selection invariant test passes; the three constructor and four effect failures remain. |
| `cargo test -p arcweft-compiler --lib --test callable_execution --test evaluated_effects --test project_function_instances --test try_pipe --no-fail-fast -- --nocapture` | Library passed 67/67. `callable_execution`: 43 passed / 18 failed / 61 total. Related integrations passed 9 + 6 + 8, giving 66 passed / 18 failed across integration targets. |
| `cargo check --workspace --all-targets --all-features` | Passed, exit 0. |
| `cargo clippy --workspace --all-targets --all-features` | Passed with warnings, exit 0. |
| `cargo fmt --all` | Passed before the final focused runs. |
| `cargo fmt --all -- --check`, `git diff --check` | Passed, exit 0. |
| Maintained documentation link check | Passed: 3 documents, 38 relative targets, zero missing files. Anchors were not checked. |
| `cargo +nightly -Zscript tools/structure-audit.rs --root . --write docs/implementation/structure-audits/2026-09-09-recovery-value-evidence --fail-on-blocking` | Passed: 95 packages, 2,228 Rust files, 310 review triggers, zero blocking violations. |

An intermediate sema run after the producer change failed 737 passed / 12
failed. The validators and branch-context repair restored all previously
passing tests: the next run had 742 passed / 7 failed before the new invariant
test was added. No existing regression expectation was weakened.

Logs are `target/rejected-source-values-{sema,compiler,workspace-check,workspace-clippy,structure-audit}.log`.
Clippy summaries report sema library/test-library 1,204/1,396 warnings (1,202
duplicates), compiler 222/226 (219 duplicates), plus other workspace warnings.
These results are not warning-free acceptance.

Full workspace tests, doctests, exhaustive codec/golden and Tier 2 were not run.
They remain required at the connected main push cut. The targeted native/AWBC
matrix does not stand in for those tiers.

The final `git fetch origin` confirmed the same HEAD and remote SHA with 0/0
divergence. The retained scope archive is unchanged at 45,039 bytes, SHA-256
`6c6d2f0885a7f9e76e815e09be397556bac99b3097d290255eee66f75ad32185`.
Frozen archive contents and extracted mirrors were not edited.

## Structural review

The generated [findings](structure-audits/2026-09-09-recovery-value-evidence/findings.md),
[file measurements](structure-audits/2026-09-09-recovery-value-evidence/file_metrics.csv)
and [dependency graph](structure-audits/2026-09-09-recovery-value-evidence/package_metrics.csv)
describe the current checkout. HEAD-to-current growth includes earlier work.

| Owner / file within `src/final_analysis/` unless noted | HEAD → current physical LOC | Current bytes | Embedded test LOC |
| --- | ---: | ---: | ---: |
| sema `model.rs` | 2,481 → 2,792 | 90,802 | 0 |
| sema `analyzer/calls.rs` | 3,386 → 4,304 | 186,490 | 185 |
| sema `analyzer/calls/constraints.rs` | 4,241 → 4,921 | 201,051 | 480 |
| sema `analyzer/expressions.rs` | 3,357 → 4,037 | 175,733 | 88 |
| sema `semantic_transcript.rs` | 1,193 → 4,101 | 164,212 | 98 |
| sema `validation.rs` | 2,242 → 2,755 | 113,702 | 0 |
| sema `nominal_schema.rs` | 2,357 → 2,946 | 120,514 | 0 |
| sema `tests.rs` (test) | 8,292 → 9,046 | 312,121 | 0 |
| sema `nominal_schema/tests.rs` (test) | 662 → 704 | 23,976 | 0 |
| compiler `tests/callable_execution.rs` (test) | existing untracked → 503 | 12,739 | 0 |

The result algebra belongs to the checked expression model. Its producer is
the existing call transaction, its publication admission belongs to expression
validation, and its execution classification belongs to the existing final
projection owner. These consumers now agree on the same result; no copied
success type or independent diagnostic value authority remains. The transcript
owns deterministic encoding of that model, without a parallel schema or I/O.

Branch expectations remain in the expression analyzer, and source rejection
remains in its affine call-source adapter. The adapter's callback state,
cleanup and work ownership are unchanged. Tests follow semantic analysis,
projection admission and compiler execution responsibilities. The large
semantic test owner gains assertions on existing domain cases, while the new
projection test stays in its existing dedicated child module. This follow-up
does not justify splitting phase authority or widening API visibility to meet
a LOC threshold. The touched owners remain above review thresholds and retain
these cohesive responsibilities; no transport, persistence or rendering state
was added. Sema fan-in/out remains 8/14 (development 3/0), compiler 3/23
(development 1/5).
