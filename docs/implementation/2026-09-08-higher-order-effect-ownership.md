# Higher-order effect ownership and parameter ABI projection

- Date: 2026-09-08 (checkpoint 23:14 JST).
- Inspected HEAD and freshly fetched `origin/main`:
  `4da7bfd7a9f8324f3763d9e579fce1f0ab38965d`; divergence `0 0`.
- Existing `main` checkout; inherited changes preserved. Start: 8 deleted,
  646 modified, 89 untracked status entries; empty index.
  Final checkpoint: 8 deleted, 646 modified, 92 untracked entries; empty index.
- Supersedes the effect/callable validation checkpoint in
  [block binding context](2026-09-08-block-binding-context.md).
  Prior passes remain historical evidence, not validation of the strengthened
  tests added here.
- The [convergence goal](2026-09-08-convergence-goal-plan.md) remains active.
  Inferred callback effects are not implemented by this follow-up.

## Findings that change the next implementation step

There are three distinct incomplete joins, established with executable Rust
tests over real Arcweft source:

1. Invoking an unannotated callback parameter fails final call sealing with
   `Instantiation(Effect(UnknownRow))`. Nonempty callback contracts, two
   independently invoked callbacks, an unused second callback, and separate
   pure/nonempty applications of one declaration all reproduce that failure.
2. An uninvoked unannotated callback is accepted by sema and contributes no
   invocation effect. Runtime parameter and retained-prefix ABI construction
   nevertheless read the source parameter's unresolved row. The projection
   changes below remove those raw-schema reads.
3. Projecting through the selected callable exposes an earlier stale result:
   the callback's closed inferred ABI row is empty even though the callback
   invokes a function exposing `fs.write`. A strengthened existing test also
   shows that a closure value's function type has an empty row while its final
   checked closure catalog exposes `fs.read`.

The third finding prevents treating the second change as completed higher-order
effect support. The new native/AWBC uninvoked-callback pair still fails during
runtime semantic projection, before either engine executes.

The source producers explain why the issue cannot be solved only in runtime
argument materialization. In `analyzer/calls/semantics.rs`, provisional project
calls whose schemas have no fixed row receive an empty closed row. Explicit and
implicit closure construction in `analyzer/expressions.rs` then embeds the
body's current concrete effects in a closed function type. Callable effect
closure in `analyzer/items.rs` and `analyzer/callable_effect_graph.rs` later
computes the exposed declaration and closure rows. The early function types
and candidate effect solutions do not retain those pending dependencies.

Current type/const references have Free/Bound/Inference scope identities.
`GenericEffectReference` has corresponding shapes, but effect rows and the
callable effect overlay still use `EffectVar` directly. The generic effect
scope methods remain unused. This is an incomplete effect producer/consumer
boundary, not evidence that the existing type/const scope model should be
replaced.

## Changes actually implemented

`ResolvedCallableBase::project_parameter_type` is the shared checked-owner
projection for formal parameter templates. Ordinary project-call argument ABI
rows and retained curried-prefix binding rows use it before applying the
frozen solution. Parameter source-proof issuance, source-actual sealing and
checked argument validation consume the same operation. The old runtime paths
which read unresolved parameter types directly from the source signature were
removed. Rest binding and source-operand coordinates keep their existing
semantics.

Callable-type construction now preserves parameter projection errors instead
of discarding them through `ok().flatten()`. Runtime selection preserves the
lower typed cause through an opaque `CheckedProjectFunctionProjectionFailure`;
the private constraint vocabulary is not made public. The former cause-free
`CallableInvariant` variant was removed. An intermediate direct exposure of the
private invariant produced a `private_interfaces` warning; that exposure was
replaced before final validation.

These changes preserve the authoritative projection path and make its failures
reviewable. They do not establish that the earlier inferred effect bindings
are correct. In particular, the new ABI tests reject an empty row where the
captured callback's exposed row must be `fs.write`.

These edits introduce no pure default, later source re-evaluation or fallback
resolver, and loosen no validation rule. No
dependency, unsafe code, I/O boundary or contract version changed. All versions
remain `1`.

## Acceptance probes

The new semantic test module is
`crates/arcweft-lang-sema/src/final_analysis/tests/higher_order_effects.rs`.
Its fixtures use ordinary project functions which advertise `fs.read` or
`fs.write` in their exposed contracts but return their input. They validate
effect typing and ABI evidence without performing filesystem I/O.

| Probe | Current result |
| --- | --- |
| Explicit callback rows, both invoked versus only the first invoked | Passed. The two call rows are `{fs.read, fs.write}` and `{fs.read}` respectively. |
| One inferred nonempty callback row | Failed at final call seal: `UnknownRow`. |
| Two independent inferred rows, both invoked | Failed at final call seal: `UnknownRow`. |
| Inferred rows with an unused second callback | Failed at final call seal: `UnknownRow` for the invoked callback. |
| One declaration called with pure and nonempty callbacks | Failed at final call seal: `UnknownRow`. Required rows remain distinct per application. |
| Uninvoked inferred callback | Sema accepts the empty invocation row. Stronger selected-ABI assertions fail: parameter effect row is empty instead of `fs.write`. |
| Curried uninvoked callback | Selected prefix producer/consumer ABI projection succeeds; retained callback row assertion fails: empty instead of `fs.write`. |
| Existing closure catalog test, strengthened to inspect the closure value type | Failed: function type row is empty instead of the catalog's `fs.read`. |
| New native/AWBC uninvoked inferred-callback pair | Both fail runtime semantic projection with `UnknownRow`; execution has not started. |

The first six-test run had 2 passes / 4 failures. Adding ABI evidence to the
uninvoked test and adding the curried-prefix test produced 1 pass / 6 failures.
The existing catalog-only regression was then strengthened to inspect the
actual function value type. No test was ignored, deleted or weakened to hide
these inconsistencies.

## Required next boundary

Close effect-source ownership together with the
[coupled function-scheme request](../reviews/requests/2026-09-08-aw-ah-009.4.2.1.1.1-function-scheme-specialization-and-callable-value-execution.md):

- An omitted callback row must acquire a real declaration-owned parameter
  identity where the declaration admits effect polymorphism. Body/closure
  inference and use-site inference are distinct scopes.
- Pending callable/closure effects must remain typed dependencies until their
  authority closes. An empty provisional concrete set is not a complete
  function type or source-argument effect observation.
- The body, function value type, local/capture types, source observations,
  candidate constraints, frozen solutions, continuation ABI and closed runtime
  instances must consume the same completed effect authority. Updating only
  the final catalog or only one ABI consumer is insufficient.
- Two invoked callbacks require effect combination; uninvoked callbacks remain
  latent. Distinct uses of the same declaration must not be replaced by one
  program-wide union of their actual rows. Defaults, curried groups, returned
  closures, recursion and suspension must keep their owned execution timing.
- Candidate selection and publication must remain transactional when effect
  constraints affect applicability. Preserve cancellation, limits and the
  actual physical source-evaluation transcript; do not rebuild a selected
  outcome from source strings after effect closure.

The effect algebra itself must be adjudicated with the scheme/constraint
representation. Adding unions of effect variables is not inference-neutral:
Leijen's section 2.3 describes ambiguous union equations and contrasts retained
constraints with row-polymorphic inference. This supports requiring an explicit
decision on residual constraints and completion; it does not make Koka's
language rules Arcweft's contract. [Primary paper](https://www.microsoft.com/en-us/research/wp-content/uploads/2016/02/paper-20.pdf).

No new effect algebra or function-scheme contract is declared implementation-ready
here. The request remains active. The required boundary is internal design and
implementation work; there is no external blocker or request for user approval.

## Validation actually run

Cargo used its normal concurrency. No explicit job count or independent test
commands in parallel were used. The structural audit was an independent
metadata command during the workspace validation sequence. Rust was not edited
while builds/tests/formatting were live.

| Command | Result |
| --- | --- |
| `cargo test -p arcweft-lang-sema --lib final_analysis::tests::higher_order_effects -- --nocapture` | Final focused run: 1 passed / 6 failed / 737 filtered / zero ignored. Log: `target/higher-order-effect-boundaries-abi.log`. |
| `cargo test -p arcweft-lang-sema --lib -- --nocapture` | Failed: 737 passed / 7 failed / zero ignored. Log: `target/higher-order-effect-sema-tests.log`. |
| `cargo test -p arcweft-compiler --lib --test callable_execution --test evaluated_effects --test project_function_instances --test try_pipe --no-fail-fast -- --nocapture` | Failed overall. Library 67 passed; callable execution 43 passed / 14 failed; evaluated effects 9 passed; project instances 6 passed; Try/pipe 8 passed. Zero ignored. Log: `target/higher-order-effect-compiler-tests.log`. |
| `cargo check --workspace --all-targets --all-features --message-format=short` | Passed, with existing sema dead-code and macro linker-output warnings. Log: `target/higher-order-effect-workspace-check.log`. |
| `cargo clippy --workspace --all-targets --all-features --message-format=short` | Passed with warnings, exit 0. Log: `target/higher-order-effect-workspace-clippy.log`. |
| `cargo fmt --all` | Passed after final Rust edits. |
| `git diff --check` | Passed. |
| Maintained documentation link check | Passed: 3 documents, 31 relative targets, zero missing files. Anchors were not checked. |
| `cargo +nightly -Zscript tools/structure-audit.rs --root . --write docs/implementation/structure-audits/2026-09-08-higher-order-effect-ownership --fail-on-blocking` | Passed: 95 packages, 2,228 Rust files, 310 review triggers, zero blocking violations. |

The main callable file now contains 28 native/AWBC pairs and one negative
codec/verifier test. Related integrations total 66 passed / 14 failed. The
earlier six paired families remain incomplete, joined by the new uninvoked
callback pair; the named block-argument and pipe-order fixes still pass.

Clippy's sema library/test-library summaries are 1,203/1,395 warnings (1,202
duplicates), and compiler summaries are 222/226 (219 duplicates), plus other
workspace and integration warnings. No lint suppression was added and no
warning-free result is claimed.

Full workspace tests, doctests, exhaustive codec/golden and Tier 2 were not run
in this follow-up. They remain required for the connected main push cut.
Generic Match, retained View, RuntimePlan/task-plan, nominal C1–C6,
scheduler/restore and the other callable requirements remain goal obligations.
The index is empty; no implementation commit/push, branch/worktree creation,
checkout switch, reset or unrelated cleanup occurred.

The retained scope archive remains 45,039 bytes with SHA-256
`6c6d2f0885a7f9e76e815e09be397556bac99b3097d290255eee66f75ad32185`.
Frozen review archives and extracted mirrors were not edited.

## Structural review

The [generated findings](structure-audits/2026-09-08-higher-order-effect-ownership/findings.md),
[file measurements](structure-audits/2026-09-08-higher-order-effect-ownership/file_metrics.csv)
and [dependency graph](structure-audits/2026-09-08-higher-order-effect-ownership/package_metrics.csv)
describe the current checkout. HEAD-to-current measurements include inherited
changes, not only this follow-up.

| Owner | HEAD → current LOC | Bytes |
| --- | ---: | ---: |
| sema `callable.rs` | 211 → 233 | 13,752 |
| sema `callable/join.rs` | 859 → 1,776 | 67,154 |
| sema `callable/checked_application.rs` | 3,641 → 4,503 | 167,122 |
| sema `final_analysis/tests.rs` | 8,292 → 9,019 | 311,304 |
| sema `final_analysis/tests/higher_order_effects.rs` | new → 247 | 8,482 |
| compiler `tests/callable_execution.rs` | existing untracked → 476 | 12,026 |

The checked callable base owns formal parameter projection, and the join owner
owns runtime selection, completed-group materialization and retained-prefix ABI.
The same selected authority now reaches all these consumers. The opaque failure
carrier exposes reporting without exporting private constraint construction.
No additional semantic state, catalog, transport or persistence responsibility
was introduced. These cohesive owners remain above size review thresholds;
their publication/validation invariants must not be split into independent
readers merely to reduce file size.

The large final-analysis test file received one value-type assertion beside
its existing catalog assertion. New related cases reside in their dedicated
test module. No embedded production test body was added. Dependency fan-in/out
remains sema 8/14 (development 3/0) and compiler 3/23 (development 1/5).
