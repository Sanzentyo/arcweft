# Controlled type projection — 2026-09-09

Supersedes the compiler/sema validation state in
[instance graph and type dependencies](2026-09-09-instance-graph-and-type-dependencies.md)
and [recovery value evidence](2026-09-09-recovery-value-evidence.md). Their other
implementation findings remain applicable. This is progress within the same
[active goal](2026-09-08-convergence-goal-plan.md), not a completed main push cut.

Inspected HEAD is `4da7bfd7a9f8324f3763d9e579fce1f0ab38965d` on the existing
`main` checkout. `git fetch origin` confirmed 0/0 divergence. Before this note,
the index was empty and Git reported 8 deleted tracked paths, 648 modified
tracked paths and 102 untracked status entries. These include inherited work.
No branch, worktree, reset, implementation commit or push was made.

## Implemented boundary

Sema now supplies `TypeProjectionControl` with type, constant and effect
occurrences, insertion depth, binding visits and cancellation checkpoints.
`TypeProjectionError` preserves a consumer's exact abort separately from a
semantic instantiation error. Sema owns no compiler budget, clock or I/O.

The existing scoped term mapper now uses explicit traversal frames. The same
exhaustive `TypeConstraintShape` reconstruction projects scalar children before
copying them. Default and controlled APIs use this one traversal. Replacement
values retain their insertion depth and nested binders; they are not reapplied
through the callee's declaration keys. The binding closure no longer clones an
effect-substitution map and recursively substitutes an entire type before
starting projection. Effect row resolution uses one shared iterative resolver
with borrowed lookup and visitation, retaining cycle/unknown/unbound errors.

Compiler `ProjectInstantiationWork` owns the cumulative counters and first
abort. Discovery transfers this same ledger into the sealed graph.
`ProjectInstanceTypes` pairs a borrowed closed solution with that ledger and its
origin. Body parameters, expressions, patterns, locals, type roots, nested
closures, attached defaults, nominal/variant operations and Content/Fx
consumers now receive this context. No consumer creates a fresh materialization
budget. Controlled call closure also uses the transaction's ledger.

| Inclusive production bound | Value |
| --- | ---: |
| Distinct instances | 4,096 |
| Distinct dependency edges | 65,536 |
| Structural type/constant/effect occurrences | 1,048,576 |
| Structural projection depth | 128 |
| Graph work, binding visits and structural visits | 4,194,304 |

Each admitted structural occurrence consumes one node and work unit; binding
visits consume work. A substitution occurrence and its copied replacement are
both visited at the insertion depth. An effect row and each concrete effect or
open tail are occurrences. Limits are checked before committing a counter
update. A limit or cancellation abort remains latched, including after graph
sealing, and preserves its typed origin through the compiler diagnostic.

## Validation actually run

| Command / scope | Result |
| --- | --- |
| `cargo test -p arcweft-lang-sema --lib -- --nocapture` | Final run: 746 passed / 7 failed / 753 total; zero ignored. All three new projection tests passed. |
| `cargo test -p arcweft-compiler --lib lower::project_instances::tests -- --nocapture` | 19 passed, 62 filtered out; before the later consumer migration. |
| `cargo test -p arcweft-compiler --test project_function_instances --test project_cache_transaction -- --nocapture` | 11 and 19 passed, before the stronger rollback fixture. |
| `cargo test -p arcweft-compiler --lib --test project_function_instances --test project_cache_transaction --test callable_execution --test evaluated_effects --test try_pipe --no-fail-fast -- --nocapture` | Final library 81/81; project instances 11/11; cache transactions 19/19; evaluated effects 9/9; Try/pipe 8/8. Callable execution: 53 passed / 18 failed / 71 total. |
| `cargo test -p arcweft-compiler --lib --test project_function_instances --test project_cache_transaction --no-fail-fast -- --nocapture` | After the final internal error-type cleanup: 81/81, 11/11 and 19/19 passed; zero ignored. |
| `cargo check --workspace --all-targets --all-features` | Passed, exit 0; before the internal error-type cleanup. Final Clippy compiled that cleanup across the same workspace/target/feature scope. |
| `cargo clippy --workspace --all-targets --all-features` | Final run passed with warnings, exit 0. |
| `cargo fmt --all`, `cargo fmt --all -- --check`, `git diff --check` | Passed, exit 0. |
| Documentation link check | Final three documents, 43 relative targets, zero missing files. Anchors not checked. |
| `cargo +nightly -Zscript tools/structure-audit.rs --root . --write docs/implementation/structure-audits/2026-09-09-controlled-type-projection --fail-on-blocking` | Passed: 95 packages, 2,232 Rust files, 310 review triggers, zero blocking violations. |

The final integration total is 100 passed / 18 failed. The 18 existing paired
native/AWBC failures remain: inferred callback effects, three contextual
constructor cases, curried/generic prefixes as callbacks, shared prefixes with
different later types, an uninvoked inferred callback, and CharacterDialogue
factories. The seven sema failures remain the three contextual constructor
cases and four inferred higher-order effect cases. These are required work,
not expected-success exclusions or ignored tests.

New evidence covers exact inclusive node/depth/work limits, constant visits,
sticky aborts, work continuing after discovery, growing specialization, and
deep types inside an otherwise shallow function or nested closure. The final
cache test checks instance admission, substitution-node and body-depth failures
against the same previously accepted generation, with no cache store and with
subsequent normal compilation preserving the accepted HIR and program hash.

Initial compile attempts found a wrong test constructor arity and a missed
effect-error conversion in the global closure path; both were repaired before
the final runs. Final logs are
`target/type-projection-control-{sema-final,compiler-final}.log`.

The final compiler context returns the precise projection error without
premature conversion into every unrelated lowering error. That eliminated 23
new lint reports. Remaining Clippy library/test-library counts are compiler
223/227 (220 duplicates), sema 1,204/1,396 (1,202 duplicates), and runtime-plan
146/148 (146 duplicates), plus other workspace/integration warnings. The new
`close_instance` lowering entry also encounters the existing common lowering
error-size warning; no blanket suppression was added. Logs additionally use
`target/type-projection-control-{workspace-check,workspace-clippy-final,compiler-post-lint,structure-audit}.log`.

Full workspace tests, doctests, exhaustive codec/golden and Tier 2 have not run
in this follow-up. The callable/effects/Try broad run preceded the internal
error-type cleanup; the final post-lint run repeats the compiler library and
both instance/rollback integration targets. Required final tiers remain
attached to the coherent connected main push cut.

## Structural review

The generated [findings](structure-audits/2026-09-09-controlled-type-projection/findings.md),
[file metrics](structure-audits/2026-09-09-controlled-type-projection/file_metrics.csv)
and [dependency graph](structure-audits/2026-09-09-controlled-type-projection/package_metrics.csv)
describe the final Rust state. HEAD-to-current growth includes inherited work.

| Owner / path | HEAD → current physical LOC | Bytes | Embedded test LOC |
| --- | ---: | ---: | ---: |
| sema `src/types.rs` | 1,330 → 1,718 | 56,476 | 108 |
| sema `src/types/projection_control.rs` | new → 112 | 3,456 | 0 |
| sema `src/types/constraints/shape.rs` | 489 → 558 | 19,788 | 0 |
| sema `src/types/constraints/solution/template.rs` | existing untracked → 515 | 19,409 | 0 |
| sema `src/types/constraints/solution/instantiation.rs` | existing untracked → 332 | 13,113 | 0 |
| sema `src/callable/checked_application.rs` | 3,641 → 4,505 | 167,242 | 0 |
| sema `src/callable/join.rs` | 859 → 1,851 | 70,240 | 0 |
| sema `src/effect_row.rs` | 1,289 → 1,308 | 45,495 | 339 |
| compiler `src/lower.rs` | 3,981 → 7,841 | 331,556 | 0 |
| compiler `src/lower/project_instances.rs` | existing untracked → 517 | 17,626 | 0 |
| compiler `src/lower/project_instances/work.rs` | new → 166 | 5,444 | 0 |
| compiler `src/lower/project_instances/types.rs` | new → 89 | 3,114 | 0 |

The sema type facade retains the exhaustive type algebra and its established
leaf/domain tests; budget policy is in no sema type. Scoped substitution and
type reconstruction remain in their existing constraint owner, with the
control contract separated by consumer responsibility. Binding closure and
the explicit frame traversal are separate cohesive responsibilities sharing
the same structural constructor, not competing type authorities.

The large checked-application owner retains immutable application evidence and
its frozen solution. Its change delegates controlled closure to the constraint
owner. The callable join owner still validates that evidence and constructs
runtime selections; it has no compiler counter state. The effect row owner
retains its row algebra, resolver and algebra tests; borrowed controlled and
default lookup share one resolver. Those state/API/test boundaries justify
retaining these owners during the connected migration.

Compiler accounting and the bound lexical projection context are now separate
children of the existing instance graph owner. The large lowerer delegates
through that context instead of acquiring a second budget or substitution
catalog. Its semantic-to-runtime family projection remains coupled to accepted
generations and exact executable partitions; splitting it by line count would
not change that authority. Tests follow the constraint fold, graph lifecycle,
compiler limit diagnostics and transaction rollback, in their existing test
owners. No facade or mutable-state API was widened merely to move code.

Normal/development workspace fan-in/out remains sema 8/14 and 3/0; compiler
3/23 and 1/5. No Cargo dependency, feature, transport, persistence, or platform
I/O boundary was added. Remaining broad design obligations below are not
resolved by this structural audit's zero-blocking result.

## Remaining work and design limits

This does not establish complete bounded instantiation. Canonical semantic and
callable digest generation still uses its existing encoder and unmetered base
projection. Checked nominal `ty()` construction, copies surrounding runtime
normalization, and some recursive non-projection traversals also remain outside
the controlled fold. Their actual producers and consumers must migrate; a
preflight walk, second encoder or assumption that all inputs are shallow would
not close this boundary. Preserve version-1 identity bytes while completing
controlled encoding, including independently hashed variant owner and payload
field identities and nested lexical scope.

Partial source constraints, declaration-owned effect schemes, callable value
specialization and execution, persistence and the complete bounded transcript
remain coupled in
[AW-AH-009.4.2.1.1.1](../reviews/requests/2026-09-08-aw-ah-009.4.2.1.1.1-function-scheme-specialization-and-callable-value-execution.md).
Match, retained View, RuntimePlan/task-plan, remaining nominal C1-C6 and
scheduler/restore acceptance remain in the active goal. There is no external
blocker. No contract version, compatibility exception or layer direction was
changed, and no frozen review package was edited.

Final Git status is 8 deleted tracked paths, 648 modified tracked paths and
104 untracked status entries, with an empty index. Required implementation and
validation remain, so the active goal is neither completed nor blocked.
