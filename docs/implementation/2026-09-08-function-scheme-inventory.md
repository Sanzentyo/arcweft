# Function-scheme inventory integration — 2026-09-08

Supersedes the compilation and immediate scope-inventory status in
[flat instance/template projection](2026-09-08-flat-instance-and-template-projection.md).
Earlier results remain historical. The full
[convergence goal](2026-09-08-convergence-goal-plan.md) is still active.

Inspected the existing `D:/git/arcweft` checkout on `main`. HEAD and
`origin/main` remain
`4da7bfd7a9f8324f3763d9e579fce1f0ab38965d`; the index is empty. This
continuation began with 8 deleted / 641 modified / 66 untracked status entries
and reached 8 deleted / 642 modified / 67 untracked entries before this record
was added. Untracked entries may represent whole directories. No existing
changes were discarded, and no branch, worktree, commit, or push was made.
After adding the record, status was 8 deleted / 642 modified / 68 untracked
entries, with the index still empty.

## Implemented and inspected boundary

Callable inventories now use the existing kind-separated type/const
references as formal keys: a declaration Free reference or a root-bound
function-scheme slot. The inventory retains the incoming template scope.
It does not manufacture declaration IDs for anonymous parameters.
`CallableGenericParameterIssuer` distinguishes declaration, function-scheme,
and empty authority; the scheme retains its actual binder.
`for_function_value` now seals type/const slot inventories and scoped schema
identity instead of ignoring its binder.

`TypeConstraintParameterScope` retains that template scope separately from
its fresh application opening. It validates the complete root-slot inventory,
rejects inference references as persistent keys, and keeps caller-rigid Free
references distinct from candidate parameters. Opening a template replaces
only its formal references; a nested function keeps its local binders.
The active lexical scope is restored on return from template opening.
Continuation restoration compares the template scope as part of its exact
completed contract.

Completed binding keys and values both expose contextual views. The common
`ScopedView<T>` carrier supplies the corresponding type, length, and
reference views; these are context, not unchecked scope certificates.
Residual origins retain declaration or scheme references and reopen into a
fresh application. Template substitution and flat instance specialization
now accept both key forms. A caller's scheme key cannot capture a binder
inside an already completed operand; template and operand scopes are explicit
at that boundary.

The generic-use visitor normalizes an outer reference encountered beneath
nested binders back to its incoming coordinate. Repeated appearances of one
outer type/const variable therefore share one inventory key and first-use
position. Scope conversion belongs to the reference type, and schema
encoding uses the retained template scope. Schema encoding includes the
binder arities, with the existing version-1 domain. No old encoder or version
reader was retained.

Consumers migrated include callable constraint initialization, deferred
continuation keys, frozen solution and instance transcripts, analyzer future
projections, and declaration-owned enclosing scopes. The compiler propagates
fallible type identity into runtime and View projection. View handler
admission compares the complete existing zero-argument, pure, fixed-result
signature, including its binder, instead of matching only selected function
fields. This does not add a language-wide function-scheme prohibition.

The now-executable lower tests exposed an omitted `VariantPayload`
compatibility arm. Payload compatibility now walks its owner and fields under
structural compatibility, so identical physical fields cannot widen the
dependent owner to a different semantic case. Early keyed-projection opening
also uses the same invariant/error classification as final projection.
Declaration-only fixtures now explicitly open active equation/source
references instead of treating declaration Free references as inference
variables. Completed-claim fixtures preserve owner classification when
opening detects an invalid term.

Obsolete schema-content encoder accessors, the unused nominal reconstruction
helper, and the unused active-scope binder getter were removed. Declaration
collection conveniences needed only by tests are confined to test builds.
Generic effect-reference APIs are still pending integration and emit three
dead-code warnings; those warnings were not suppressed or counted as a clean
lint result.

## Validation actually performed

- `cargo check -p arcweft-lang-sema --tests --message-format=short`:
  **PASSED** at the completed inventory checkpoint. The following focused
  test builds also compiled the later sema changes. Intermediate migration
  failures included 42 and 7 library diagnostics and 66 library-test
  diagnostics. They are not test results.
- `cargo test -p arcweft-lang-sema --lib types::constraints::solution:: --
  --nocapture`: **PASSED**, 13 tests. Log:
  `target/generic-scheme-solution-tests.log`. An earlier attempt failed to
  compile three test-only collector imports, which were corrected.
- `cargo test -p arcweft-lang-sema --lib types::constraints:: -- --nocapture`:
  **PASSED**, 101 tests. Log: `target/generic-scheme-constraint-tests.log`.
  The preceding run was **FAILED**, 87 passed / 13 failed; it exposed the
  payload compatibility omission, early projection classification, and stale
  declaration/inference fixtures described above.
- `cargo test -p arcweft-lang-sema --lib generic_inventory_tests --
  --nocapture`: **PASSED**, 16 tests. Log:
  `target/generic-scheme-schema-tests.log`.
- `cargo test -p arcweft-lang-sema --lib types::generic_use:: -- --nocapture`:
  **PASSED**, 9 tests. Log: `target/generic-scheme-use-tests.log`.
  The 13 solution tests are included in the 101 constraint tests; the last
  three selections cover **126 distinct tests**, not 139.
- `cargo check -p arcweft-compiler --tests --message-format=short`:
  **PASSED**, with the three sema effect-reference warnings. Log:
  `target/generic-scheme-compiler-check.log`. Earlier attempts failed at 10
  compiler library/test diagnostics, then four integration-test digest calls.
- `cargo test -p arcweft-compiler --test callable_execution -- --nocapture`:
  **FAILED**, 20 passed / 12 failed, exit 101. Log:
  `target/generic-scheme-callable-execution.log`.
- Changed-crate formatting: **PASSED**. Canonical structural audit:
  **PASSED**, 95 packages, 2,218 Rust files, 311 review triggers, zero blocking
  violations. Command: `cargo +nightly -Zscript tools/structure-audit.rs
  --root . --write
  docs/implementation/structure-audits/2026-09-08-function-scheme-inventory
  --fail-on-blocking`. Log: `target/function-scheme-inventory-audit.log`.
  The audit ran while the single compiler execution-test build was pending;
  no parallel test commands or explicit Cargo job counts were used.
- `cargo fmt --all -- --check` and `git diff --check`: **PASSED**.
  Local Markdown target inspection of this record, the preceding flat
  projection record, and the correction request found 22 targets and zero
  missing files. Anchor fragments were not checked.
- Workspace all-target/all-feature checks, Clippy, workspace tests, doctests,
  codec/golden, and Tier 2: **NOT RUN**. They remain required for the connected
  cut. The complete sema unit suite and other compiler execution suites were
  not run in this continuation.

The callable execution successes include native and AWBC recursive generics,
nested generic calls, instantiated Option payload owners, direct calls, mutual
recursion, and the exercised lazy branch cases. They do not prove all callable
execution or the later goal stages.

| Remaining reproducer, both engines | Observed failure |
| --- | --- |
| `later_argument_closes_an_earlier_contextual_variant` | Active type inference escapes during source/semantic completion of the earlier contextual variant |
| `callback_with_inferred_effects` | Final call sealing reports an unknown dynamic-call effect row |
| `callback_with_project_call_body` | Runtime-plan lowering lacks a flow-owned project-call projection and a checked expression row |
| `curried_prefix_as_callback` | Native/AWBC ordinary function application does not accept the project-continuation value |
| `shared_prefix_with_distinct_later_types` | Runtime reachability sees a call expression with a structural projection |
| `generic_prefix_as_monomorphic_callback` | Reaches runtime-plan lowering, which reports a structural projection for a call expression |

## Structure review

Retained generated [file metrics](structure-audits/2026-09-08-function-scheme-inventory/file_metrics.csv),
[package metrics](structure-audits/2026-09-08-function-scheme-inventory/package_metrics.csv),
[dependency edges](structure-audits/2026-09-08-function-scheme-inventory/dependency_edges.csv),
and [findings](structure-audits/2026-09-08-function-scheme-inventory/findings.md)
record the full measurement. The base is the complete accepted HEAD file;
growth includes inherited dirty work. Paths below are relative to `crates/`.

| Owner | Base LOC | Current LOC | Bytes | Embedded tests | Cohesion disposition |
| --- | ---: | ---: | ---: | ---: | --- |
| arcweft-lang-sema/src/types.rs | 1,330 | 1,715 | 56,294 | 108 | Existing semantic facade exports contextual reference views and confines test utilities |
| arcweft-lang-sema/src/types/generics.rs | 0 | 573 | 17,950 | 0 | Owns shared scoped views and conversion from nested occurrences to template keys |
| arcweft-lang-sema/src/types/generic_use.rs | 517 | 807 | 31,007 | 293 | One visitor retains domain-specific key admission and first-use collection |
| arcweft-lang-sema/src/types/compatibility.rs | 1,702 | 1,749 | 64,218 | 711 | Completes the existing type relation's payload branch; no alternate relation engine |
| arcweft-lang-sema/src/types/constraints/context.rs | 1,282 | 1,650 | 60,444 | 0 | Owns formal scope, fresh opening, admission, and scope restoration |
| arcweft-lang-sema/src/types/constraints/transaction.rs | 1,957 | 1,779 | 65,946 | 0 | Projection admission uses its existing typed failure classification |
| arcweft-lang-sema/src/types/constraints/solution/tests.rs | 0 | 574 | 19,536 | 0 | Tests follow the completed-solution and caller/template ownership boundary |
| arcweft-lang-sema/src/types/constraints/tests.rs | 3,868 | 4,062 | 142,321 | 0 | Existing transaction fixtures now distinguish active variables and declaration references |
| arcweft-lang-sema/src/callable/schema.rs | 2,693 | 4,460 | 163,475 | 1,168 | Owns declaration/scheme inventory derivation and schema admission |
| arcweft-lang-sema/src/callable/checked_application.rs | 3,641 | 4,516 | 167,640 | 0 | Owns scoped frozen/deferred transcript projection; no copied key table |
| arcweft-lang-sema/src/callable/continuation.rs | 2,307 | 2,385 | 92,956 | 27 | Builds the exact parameter scope from schema and enclosing declaration evidence |
| arcweft-lang-sema/src/callable/constraints.rs | 2,174 | 2,185 | 87,160 | 1,317 | Existing driver fixtures use explicit declaration origins |
| arcweft-lang-sema/src/callable/join.rs | 859 | 1,758 | 66,696 | 0 | Instance identity consumes scoped keys from the single lower environment |
| arcweft-lang-sema/src/final_analysis/analyzer/calls.rs | 3,386 | 4,328 | 186,876 | 185 | Checks declaration origins for the enclosing lexical callable |
| arcweft-lang-sema/src/final_analysis/analyzer/calls/constraints.rs | 4,241 | 4,944 | 202,168 | 476 | Future projection keys retain scheme/declaration references |
| arcweft-lang-sema/src/final_analysis/nominal_schema.rs | 2,357 | 2,900 | 119,083 | 0 | Deletes obsolete nominal reconstruction; retained catalog ownership is unchanged |
| arcweft-lang-sema/src/final_analysis/tests.rs | 8,292 | 8,993 | 310,253 | 0 | Existing declaration inventory assertions adopt explicit Free references |
| arcweft-compiler/src/lower.rs | 3,981 | 7,755 | 328,452 | 0 | Existing runtime projection propagates scope errors and reuses its checked identity |
| arcweft-compiler/src/view.rs | 1,031 | 1,421 | 57,566 | 0 | Existing View parameter/capture/handler authority checks full identity and ABI |

The smaller touched owners include reference opening, residual reification,
template/flat instantiation, schema encoding/errors, and compiler variant
projection. Related type/const fixtures and four compiler cache assertions
were migrated; the latter file is 1,482 LOC, below the integration-test review
threshold. No dependency, Cargo feature, I/O owner, transport, or persistence
surface changed. Sema workspace fan-in/out remains 8/14 (development 3/0),
compiler 3/23 (development 1/5). The large owners retain their existing state
and API responsibilities; the new key/scope behavior stays with the type and
schema owners instead of adding a parallel resolver. Their full connected
cut still requires the wider structural and runtime review.

## Remaining work and design status

The [active correction request](../reviews/requests/2026-09-08-aw-ah-009.4.2.1.1.1-function-scheme-specialization-and-callable-value-execution.md)
still governs effect-reference integration, contextual/generic guard evidence,
per-use monomorphic specialization, one callable execution route, finite
discovery with limits/cancellation, and suspension/restore. The old instance
layers remain deleted. The callback reproducer reaching runtime-plan lowering
does not prove a sealed general specialization contract or justify reviving a
blanket scheme ban.

Match C3/C5, retained View, RuntimePlan, nominal, scheduler/restore, required
whole-workspace validation, and final commit/push are not waived. No returned
design was accepted in this continuation; no contract version was bumped.
The implementation is materially further along, but the twelve execution
failures and the other goal requirements prevent completion. There is no
external blocker.

The subsequent [saved-prefix reapplication record](2026-09-08-prefix-reapplication.md)
updates the shared-prefix failure boundary and adds focused transaction tests
and changed-crate Clippy evidence. It does not supersede the historical
validation above with an overall engine pass.
