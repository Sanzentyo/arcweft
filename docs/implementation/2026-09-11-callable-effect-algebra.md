# Callable effect algebra: implementation in progress

Date: 2026-09-11. Inspected `main` at
`55633925b72706132ae3cf4c4287d8645be31a68`; `origin/main` matched after fetch.
The index was empty at the start, with 28 Rust working-copy paths from the
ongoing callable migration. The changes below remain working-copy work within
that same obligation. This note does not award implementation readiness or
complete the [convergence goal](2026-09-08-convergence-goal-plan.md).

Design decisions are recorded in
[callable component design](2026-09-11-callable-component-design.md). The
[coupled request](../reviews/requests/2026-09-08-aw-ah-009.4.2.1.1.1-function-scheme-specialization-and-callable-value-execution.md)
remains active. The preceding
[effect-completion investigation](2026-09-11-callable-effect-completion.md)
remains the source of the declaration-body UnknownRow diagnosis.

## Implemented working-copy boundary

`effect_row/decision.rs` owns a private reduced ordered membership DAG.
Construction, existential projection, simultaneous substitution, least-witness
completion and canonical reachable postorder are iterative. Conditional
construction is the single Boolean operation primitive. A local node index
cannot be supplied by a consumer or mixed with another graph.

`effect_row/membership.rs` applies that decision owner to a default label class
and sorted concrete-effect overrides. Finite set formulas and universally
interpreted predicates have distinct private constructors. Union, intersection,
difference and subset retain independent row variables. Substitution includes
labels introduced only by replacement rows and does not substitute a second
time inside a replacement.

Completion projects the admissible domain and computes the intersection of
all witnesses. It accepts those witnesses only if their simultaneous
substitution satisfies the relation throughout that domain. A relation such as
`read subset a union b` cannot choose `a` or `b` by iteration order. A predicate
that fails at the all-empty default valuation is impossible for finite rows;
it cannot acquire an infinite witness set. Contradictory concrete label classes
retain their exact labels for rejection diagnostics.

The existing `EffectConstraintEnvironment` now consumes this predicate owner.
Its old lower/upper sets, propagation edges and `solve_bounds` loop were
deleted. Parameter eligibility, touched state and inherited values remain in
the environment. Subset admission retains the relation and checks existence;
it does not prematurely choose a witness. Exact inherited rows and each new
constraint commit only after successful decision work and validation.

Every production caller supplies the same `TypeConstraintContext` to effect
admission, completion, equality, restoration and substitution. Decision visits
and emitted nodes consume its existing node/work counters, cancellation and
external accounting. No fresh context, independent limit or unmetered production
control was added. The standalone error mapping helper was replaced by `From`
on the owning constraint error.

This connects the new algebra to the existing lower engine, but the input row
and final callable type still use the old scalar effect-variable interface.
The declaration/scheme generic-reference migration and symbolic callable-body
publication have not been completed. Consequently the six higher-order
UnknownRow acceptance cases still fail; low-level algebra tests do not claim
otherwise.

## Validation actually performed

All Cargo commands used normal concurrency and the same all-features sema
configuration. No additional clean was required.

| Command / scope | Result |
| --- | --- |
| `cargo fmt -p arcweft-lang-sema` | Passed |
| `cargo test -p arcweft-lang-sema --lib --all-features effect_row::decision::tests::` | 8 passed before production integration |
| `cargo test -p arcweft-lang-sema --lib --all-features effect_row::` | 29 passed before lower-engine integration, including 15 new algebra tests |
| `cargo check -p arcweft-lang-sema --all-targets --all-features` during integration | Failed on five test call sites missing the new control argument; corrected |
| First full sema run after integration | 811 passed / 16 failed; five additional budget failures |
| Full sema run after avoiding terminal graph work | 814 passed / 13 failed; inclusive 256-candidate and comparison-limit regressions resolved |
| Focused `fixed_effect_evidence` after further Boolean reduction | 1 passed / 2 failed at the test-only 128-work bound |
| Full sema run with semantic fixture budgets corrected | 816 passed / 11 failed out of 827 |
| `cargo clippy -p arcweft-lang-sema --all-targets --all-features` | Passed with warnings; includes the complete sema test target |
| Final full sema run after Boolean style cleanup and all quantifier positions | 816 passed / 11 failed out of 827; same remaining failure set |
| `cargo +nightly -Zscript tools/structure-audit.rs --root . --fail-on-blocking` | Passed: 95 packages, 2,279 Rust files, 310 review triggers, zero blocking violations |
| Documentation links and `git diff --check` | Passed |

The first integrated test wrapper accidentally returned the subsequent `rg`
status instead of the Cargo status. Its log explicitly says FAILED and it is
recorded above as failed. Later wrappers preserve Cargo's exit code before
displaying summaries.

Two fixed-effect semantic tests used a 128-work/128-node fixture limit. They
now use 1,024 for each to include the newly metered predicate construction and
completion. Their assertions about exact rows and rejection are unchanged.
Production limits were not raised. The dedicated one-node abort test, exact
comparison-limit test and inclusive 256-candidate acceptance remain unchanged
and pass. The new decision test also aborts at every work boundary below the
measured successful operation and verifies that both inputs remain unchanged.

The final exhaustive oracle checks all 256 Boolean relations over three row
references and all eight choices of existential references: 2,048 relation/
quantifier combinations. It enumerates satisfying assignments independently,
checks the projected domain and least witness, and verifies that quantified
references do not survive either result. It covers empty binders as well as
rigid references before and after the quantified positions.

The remaining full-library failures are the five established contextual or
ordinary correlated-call cases and the six higher-order effect cases, including
the two acceptance cases added in the preceding investigation. These are
required work. The earlier 801/9 result predates the two new acceptance tests
and the 15 algebra tests, so it is not the current denominator.

Clippy reported existing warnings and unused portions of the new algebra that
will be consumed by the pending scheme/type migration. No warning suppression
was added. A new Boolean-condition style warning was corrected after that run.

The structural review retains the existing state owners. At this checkpoint,
`effect_row.rs` is 1,310 lines: public row/report APIs, constraint-environment
admission and their existing tests remain one effect-domain entry point; the
new decision algorithm (511 lines) and label membership (383 lines) are split
along their distinct algebra responsibilities, each with a separate test file.
`types/constraints/context.rs` (1,732 lines) remains the one accounting/scope
owner; this change adds the effect-control implementation there instead of
forwarding accounting through a new context. `transaction.rs` (1,911 lines)
retains the affine source/solve/materialize lifecycle, with its existing child
test modules. Its new calls borrow that same context. No new crate dependency,
public facade, I/O owner or independent mutable catalog was introduced. These
are cohesion dispositions for the touched owners, not a claim that a lower LOC
count completes their architecture or the broader migration.

All 71 retained review ZIPs were re-enumerated and SHA-256 inventoried. There
were no modified tracked ZIPs or untracked ZIPs; no package readiness was changed.
The local `current-rust-inputs.json` records paths, byte lengths, physical lines
and SHA-256 for all 32 preserved Rust working-copy inputs.

Logs are retained locally under
`.arcweft-local/validation/2026-09-11-callable-effect-completion/`. Workspace
check/Clippy/tests, compiler native/AWBC, codecs, doctests and Tier 2 were not
rerun for this unfinished Rust stage. No prior workspace result is promoted
to current acceptance.

## Remaining work

1. Replace scalar row payloads with scoped generic effect references and the
   formula/predicate authority across type folds, schemas, residual binders,
   declaration/closure body equations, final facts and stable encoding.
2. Complete correlated source contributions and component-wide closure/ranking;
   preserve the existing borrowed driver and all application scopes.
3. Connect concrete-use specialization, complete origin relations, common
   callable states, finite instance discovery, native/AWBC invocation and
   program-bound save/restore. Delete the replaced representations and success
   paths only as their final consumers migrate.
4. Run the coupled request's complete acceptance and repository validation
   before committing the Rust implementation as a completed cut.

The new design record is development evidence, not the independently packaged
returned contract requested by the active design request. No archive or
`READY_FOR_IMPLEMENTATION` status is claimed here.
