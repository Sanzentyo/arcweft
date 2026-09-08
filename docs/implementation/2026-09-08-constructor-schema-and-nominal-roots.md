# Constructor schemas and semantic nominal roots — 2026-09-08

Supersedes the immediate constructor status in
[contextual variant owners](2026-09-08-contextual-variant-owner.md).
The [convergence goal](2026-09-08-convergence-goal-plan.md) remains active;
this is implementation evidence, not a completed language/runtime cut.

Work stayed in the existing `D:/git/arcweft` checkout on `main`. A fresh
`git fetch origin` succeeded. HEAD and `origin/main` both remain
`4da7bfd7a9f8324f3763d9e579fce1f0ab38965d`, with 0/0 divergence and an
empty index. This follow-up began with 8 deleted / 642 modified / 73 untracked
status entries and reached 75 untracked entries before this note. Inherited
changes were preserved. No branch, worktree, commit, or push was made.

## Implemented boundary

`fallback(.Full(42i64), 0i64)` previously lost the enclosing enum shape when
the parent candidate still owned inference variables. `CallSource` now retains
the full typed expectation. Constructor lookup reads its contextual shape;
an independent child call receives only a complete result expectation. The
constructor's formal type parameters come from the actual nominal declaration
or existing Option/Result intrinsic issuer. Parent inference holes do not
become invented declarations or stable owner IDs.

The constructor signature owns the declaration-template result type and
payload parameter schema. Its signature ID identifies that template and case
ordinal. `ExpectedEnum { expected }` was replaced by the unit
`EnumConstructor` role in prepared constraint inputs and prepared/checked base
instantiations. Copied expected types and case ordinals were removed from
constructor seeds and prepared language identities. The redundant constraint
against the copied template, its unused result-projection helper, and the
already-checked-expression constructor reader were deleted. The schema factory
checks that the signature owner digest matches the result template.

Final call sealing constructs complete pending callee-fact replacements before
publication. For a constructor head it applies the current application's
`instantiate_template` operation to the prepared owner and every case payload,
then checks the resulting owner against the application's result. It does not
reapply callee bindings to completed caller-owned operands. Non-callable
semantic heads retain their semantic role rather than acquiring a function
value type.

A second failure exposed a phase mismatch in nominal collection: the semantic
inventory reused the runtime-layout collector and therefore discarded
`Slot<T>` in a generic function's `if let`. Semantic and runtime inventories
now share a canonical `NominalProjectionRequestSet` but own distinct admission
rules. Semantic collection retains declaration-owned Free references. Nominals
borrowing an enclosing quantifier remain under that template until application;
they are not hashed as standalone root types. Runtime collection still admits
only concrete roots. The sole semantic catalog seals the exact owner and full
case inventory, including generic pattern owners.

`FinalAnalysisExecutionProjection::variant_constructor` now projects named
Option/Result and contextual enum constructors from the accepted call result.
It validates the template/application relation and joins the final source
variant when required. The compiler's separate constructor matrix and obsolete
case helper were deleted. Project variant lowering closes the owner through
the enclosing instance before consulting the runtime nominal catalog. The
same projection supports construction and pattern payload extraction in both
native and AWBC execution.

No contract version was changed; replaced transcript shapes continue to use
version `1`, with no compatibility reader. No language-level scheme ban or
new inference restriction was adopted. The representation changes implement
the existing typed-authority requirements; they do not settle the remaining
parent/child inference contract.

## Validation actually performed

- New semantic constructor regression: **PASSED**. It now checks selected
  applications, the concrete constructor owner and complete cases, the generic
  pattern owner, and exact catalog projections.
- `cargo test -p arcweft-lang-sema --lib -- --nocapture`: **PASSED**,
  **730 tests**, zero ignored. Log: `target/contextual-constructor-sema-suite.log`.
  This was after deleting the copied enum instantiations and adding a negative
  signature/result-owner test. The subsequent unused-import/helper deletion
  was compiled by the final compiler test and Clippy runs.
  Intermediate runs failed on missing callee/result propagation, the semantic
  inventory's runtime filter, and fixtures comparing a declaration template
  with a concrete result. Three schema-only fixtures also supplied inconsistent
  result-owner coordinates and were corrected. These failures are not counted
  as passing runs.
- `cargo test -p arcweft-compiler --test callable_execution
  contextual_project_variant_payload_closes_its_owner -- --nocapture`:
  **PASSED**, native and AWBC both return `42` from the constructed payload.
  Log: `target/contextual-constructor-execution.log`.
- Full existing 36-case native/AWBC matrix after the representation cleanup:
  **FAILED**, **26 passed / 10 failed**. All original passing cases remained
  passing; the earlier project-payload failures were fixed.
- Expanded `cargo test -p arcweft-compiler --test callable_execution --
  --nocapture`: **FAILED**, **28 passed / 14 failed out of 42**, zero ignored.
  Log: `target/contextual-constructor-callable-matrix.log`. Six additional tests
  exercise the three cases below in both engines. The generic-body case passes;
  the two parent-only inference cases fail during semantic checking.
- `cargo test -p arcweft-compiler --test evaluated_effects
  --test project_function_instances -- --nocapture`: **PASSED**,
  **9 evaluated-effect tests and 6 project-instance tests**, zero ignored.
  These cover enum defaults, typed text proxies, Content/Fx effects through
  AWBC, source-order materialization, rest operands, curried lineage and
  distinct generic instance keys. Log: `target/contextual-constructor-consumers.log`.
- `cargo clippy -p arcweft-lang-sema -p arcweft-compiler --all-targets
  --message-format=short`: **completed with exit 0 and warnings**, not a clean
  lint gate. Sema reports 1,206 library warnings and 1,386 library-test warnings
  (1,205 duplicates); compiler reports 220 library and 225 library-test warnings
  (218 duplicates), plus integration-target/dependency warnings. Log:
  `target/contextual-constructor-clippy.log`. No suppressions or broad automatic
  fixes were applied. The new 110-line constructor projection is one exhaustive
  typed family operation; large error-enum warnings remain visible.
- `cargo fmt --all -- --check` and `git diff --check`: **PASSED**.
  Markdown target inspection across this record, the goal plan and the active
  request found 22 relative targets and no missing files. Anchors were not
  checked.
- Canonical audit: **PASSED**, 95 packages, 2,220 Rust files, 310 review
  triggers, zero blocking violations. Command: `cargo +nightly -Zscript
  tools/structure-audit.rs --root . --write
  docs/implementation/structure-audits/2026-09-08-constructor-schema-and-nominal-roots
  --fail-on-blocking`. Log: `target/contextual-constructor-audit.log`.
- Workspace all-target/all-feature check, workspace Clippy/test, doctests,
  codec/golden and Tier 2: **NOT RUN** here and still required for the connected
  cut. No independent test commands ran concurrently; no Cargo job count was
  set. An early formatter invocation was inadvertently still completing when
  the first focused test command started; both were subsequently completed and
  formatting, full sema tests, compiler tests and lint were rerun sequentially.

## Remaining coupled work

These additional native/AWBC fixtures distinguish complete child inference
from inference that requires the parent's later arguments:

```arcw
enum Slot<T> { Empty, Full T }
fn fallback<T>(input: Slot<T>, value: T) -> T { value }
flow main() -> i64 { return fallback(.Empty(), 42i64) }
```

```arcw
enum Either<A, B> { Left A, Right B }
fn fallback<A, B>(input: Either<A, B>, value: A, other: B) -> A {
    if let .Left(item) = input { item } else { value }
}
flow main() -> i64 { return fallback(.Left(42i64), 0i64, "other") }
```

Both fail with a typed call-constraint error during semantic checking.
Their constructor arguments cannot independently determine every result
parameter. In contrast, `.Full(value)` inside `pack<T>(value: T) -> Slot<T>`
retains the enclosing declaration's identity and executes successfully after
`pack(42i64)` is instantiated.

The four new failures require the same parent/child constraint ownership and
completion design as the existing
[function-scheme and callable-execution request](../reviews/requests/2026-09-08-aw-ah-009.4.2.1.1.1-function-scheme-specialization-and-callable-value-execution.md).
Do not fix them by aliasing unrelated inference issuers, fabricating Free IDs,
discarding an unselected case's parameters, or publishing an unresolved value.
The exact child-to-parent obligation and final sealing order remain open.
This is an internal design/implementation obligation, not an external blocker.

The five earlier failure families also remain, each in both engines:
callback effect-row completion; callbacks with ProjectCall bodies; curried
prefix values passed as callbacks; monomorphic uses of generic prefixes; and
one prefix reused at distinct later types. Closure inspection confirms that
ordinary function-site reservation still uses Expression/empty-effects and
expression-only body lowering, while project closure instances retain their
checked execution row. That discrepancy was inspected but not changed here.

Generic Match, retained View, RuntimePlan, nominal reachability/restore,
scheduler, full validation, and the coherent main commit/push remain goal
requirements. This note does not turn them into non-goals or claim that all
constructor inference is complete.

## Structural ownership review

The [generated audit](structure-audits/2026-09-08-constructor-schema-and-nominal-roots/findings.md)
and [file metrics](structure-audits/2026-09-08-constructor-schema-and-nominal-roots/file_metrics.csv)
are generated evidence. The following touched production owners exceed the
review threshold. LOC comparisons use the full accepted HEAD above and the
current whole files, including inherited work; they are not this follow-up's
incremental additions.

| Owner (relative to crate `src/`) | HEAD → current LOC | Bytes | Embedded test LOC |
| --- | ---: | ---: | ---: |
| sema `callable/schema.rs` | 2693 → 4548 | 167374 | 1186 |
| sema `callable/checked_application.rs` | 3641 → 4512 | 167492 | 0 |
| sema `callable/identity.rs` | 1799 → 1858 | 57268 | 0 |
| sema `callable/join.rs` | 859 → 1757 | 66618 | 0 |
| sema `callable/resolver.rs` | 1412 → 1579 | 58567 | 0 |
| sema `callable/resolver/outcome.rs` | 1490 → 1514 | 55734 | 0 |
| sema `final_analysis/analyzer/call_seal.rs` | 1542 → 2015 | 88137 | 0 |
| sema `final_analysis/analyzer/calls.rs` | 3386 → 4333 | 187563 | 185 |
| sema `final_analysis/analyzer/calls/constraints.rs` | 4241 → 4913 | 200695 | 476 |
| sema `final_analysis/analyzer/expressions.rs` | 3357 → 3909 | 170832 | 88 |
| sema `final_analysis/analyzer/dialogue_line_plan.rs` | 482 → 1668 | 72667 | 66 |
| sema `final_analysis/nominal_schema.rs` | 2357 → 2934 | 120043 | 0 |
| sema `final_analysis/prepared.rs` | 957 → 1511 | 49428 | 0 |
| sema `final_analysis/report.rs` | 1165 → 2234 | 89991 | 0 |
| compiler `lower.rs` | 3981 → 7750 | 328376 | 0 |

- Callable schema/identity/application/join owners retain the existing schema,
  fresh opening, frozen solution, and stable transcript responsibilities. This
  migration removes copied enum type state and a redundant constraint rather
  than introducing another instantiation model. Schema fixture tests exercise
  the same owner boundary, including the new negative join. Their embedded
  tests do not gain access to unrelated production state.
- Analyzer calls/constraints/call-seal remain one candidate transaction and
  dependency-first publication path. Full expectation transport and pending
  typed callee replacements belong to that path. No I/O, global registry, or
  alternate publication state was added. The expression and dialogue owners
  only forward the appropriate expectation; their existing syntax-dispatch
  responsibility remains cohesive.
- Prepared variant mapping lives with its logical owner/case algebra. Nominal
  collection shares the canonical request set while phase-specific inventories
  control admission. The semantic catalog remains the only accepted project
  owner authority. A runtime filter no longer governs semantic roots.
- Constructor execution projection was decomposed into the 139-line,
  5,979-byte sema `final_analysis/report/variants.rs` owner. It borrows final
  analysis and introduces no independent state. Compiler `lower.rs` consumes
  it; `lower/variants.rs` is now 251 LOC / 9,977 bytes and only performs runtime
  projection. No compiler-side HIR constructor reconstruction remains.
- The prepared variant owner is 313 LOC / 11,566 bytes; payload projection is
  392 LOC / 13,440 bytes. The generic semantic regressions are 258 LOC /
  8,059 bytes and the native/AWBC fixture file is 343 LOC / 8,994 bytes.
  Full classifications and smaller producer/consumer metrics are in the CSV.
- Cargo dependency direction is unchanged: sema workspace fan-in/out is 8/14
  (development 3/0), compiler 3/23 (development 1/5). The new cross-layer API
  follows sema → compiler consumption and replaces repeated interpretation.
  No feature, manifest, transport, persistence, or Sans-I/O boundary changed.

The frozen scope archive remains 45,039 bytes with SHA-256
`6c6d2f0885a7f9e76e815e09be397556bac99b3097d290255eee66f75ad32185`.
Neither the archive nor its extracted mirror was edited.
