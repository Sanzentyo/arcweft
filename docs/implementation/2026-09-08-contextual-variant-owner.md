# Contextual variant owner preparation — 2026-09-08

Supersedes the immediate contextual-unit-variant status in
[saved-prefix reapplication](2026-09-08-prefix-reapplication.md). The
[convergence goal](2026-09-08-convergence-goal-plan.md) remains active.

Worked in the existing `D:/git/arcweft` checkout on `main`. A fresh
`git fetch origin` succeeded; HEAD and `origin/main` both remain
`4da7bfd7a9f8324f3763d9e579fce1f0ab38965d`, with 0/0 divergence and an
empty index. This follow-up began with 8 deleted / 642 modified / 70
untracked status entries and reached 72 untracked entries before this record.
Inherited changes were preserved. No branch, worktree, commit, or push was
made.
After adding this record, status is 8 deleted / 642 modified / 73 untracked
entries; the index remains empty.

## Implemented boundary

Contextual short-variant resolution previously issued a checked owner while
the enclosing call still owned active inference variables. For example, the
first operand in `choose(None, 42i64)` could not yet issue an `Option<i64>`
identity. The project-only preparation model also required an already checked
nominal and constructed payload identities during pattern preparation.

`PreparedVariantOwnerSeed` now retains the complete logical owner and case
inventory for project, character, environment, Option, and Result variants.
The shared `VariantOwnerKind<N>` algebra distinguishes a project nominal's
logical and checked phases without duplicating the owner families. Raw cases
retain ordinals, names, and optional logical tuple/record payload shapes;
they issue no stable case or field IDs. Project raw arguments may contain
active types. The final seal joins the exact project nominal catalog and case
inventory, checks the owner type is unchanged, and issues all case/field
identities only after type closure.

`PreparedVariantExpression` and `PreparedVariantPattern` replace the old
project-specific carriers. Source paths, short variants, qualified environment
variants, and variant patterns use that common preparation. Case lookup and
the unit/payload admission rule read the owner's inventory, removing the
separate source-spelling switches for Option/Result cases. Logical payload
construction and stable payload sealing share the existing type-shape owner.
Character/environment/Option/Result checked factories use the same prepared
case inventory before sealing; the unused checked project factory was deleted.

Call-source observations compare the logical owner type. Compile-time enum
reduction joins the exact project declaration and closed type. Explicit drop
policy preparation consumes the prepared case inventory rather than assuming
the source expression already has final case IDs. No version marker changed.

## Validation actually performed

- `cargo check -p arcweft-lang-sema --tests --message-format=short`:
  **PASSED** during migration. The initial attempt reported six errors in the
  enum-constructor seed and one inferred error type; all were corrected.
  Final test/lint builds below compile the subsequent edits.
  Intermediate format/test-build attempts also exposed a child-module path,
  delimiter, and misplaced fixture insertion; those were fixed before the
  successful runs and are not counted as executed tests.
- `cargo test -p arcweft-lang-sema --lib
  contextual_unit_variant_closes_with_the_later_argument_type -- --nocapture`:
  **PASSED**, one test. It verifies the final `Option<i64>` owner, semantic
  identity, case ordinal, and unit shape. Log:
  `target/contextual-variant-semantic-test.log`.
- `cargo test -p arcweft-lang-sema --lib -- --nocapture`: **PASSED**, all
  **728 tests**, zero ignored. Log: `target/contextual-variant-sema-suite.log`.
  The preceding run was **FAILED**, 726 passed / 2 failed. One failure exposed
  the drop-policy consumer of the replaced prepared source model. The other
  was an operational trace fixture still expecting declaration Free references;
  it now verifies distinct candidate-local inference references at probe time.
  That private operational trace is not the stable semantic type/codec
  transcript. Its later concrete hints remain checked as i64.
- `cargo test -p arcweft-compiler --test callable_execution
  later_argument_closes_an_earlier_contextual_variant -- --nocapture`:
  **PASSED**, native and AWBC both return 42. Log:
  `target/contextual-variant-execution-test.log`.
- `cargo test -p arcweft-compiler --test callable_execution
  later_argument_closes_a_project_enum_owner -- --nocapture`: **PASSED**,
  both engines. The new generic `Slot<T>` / `.Empty` fixture also returns 42.
  Log: `target/contextual-project-variant-execution-test.log`.
- `cargo test -p arcweft-compiler --test callable_execution -- --nocapture`:
  **FAILED**, **24 passed / 12 failed** out of 36. Log:
  `target/contextual-variant-callable-matrix.log`. Four cases were added to
  the previous 32-test matrix: the project-unit case passes in both engines;
  the contextual project payload case fails in both. The previous `None`
  failures are fixed. The five other previously failing examples remain.
- `cargo clippy -p arcweft-lang-sema --all-targets --message-format=short`:
  **completed with exit 0 and warnings**, not a clean lint gate. The final
  run reports 1,205 sema library warnings and 1,386 library-test warnings
  (1,204 duplicates); dependencies also have warnings. Log:
  `target/contextual-variant-clippy.log`. The final lint build includes a
  semantics-neutral Vec cloning cleanup after the test runs. No suppressions
  or broad automatic fixes were applied.
- Changed-crate formatting, `cargo fmt --all -- --check`, and
  `git diff --check`: **PASSED**. Markdown target inspection across this
  record, the preceding prefix record, goal plan, and active request found
  28 targets and zero missing files. Anchor fragments were not checked.
- Canonical audit: **PASSED**, 95 packages, 2,219 Rust files, 310 review
  triggers, zero blocking violations. Command: `cargo +nightly -Zscript
  tools/structure-audit.rs --root . --write
  docs/implementation/structure-audits/2026-09-08-contextual-variant-owner
  --fail-on-blocking`. Log: `target/contextual-variant-audit.log`.
  No independent test commands ran concurrently and no Cargo job count was set.
- Workspace all-target/all-feature check, workspace Clippy/test, doctests,
  codec/golden, and Tier 2: **NOT RUN** in this follow-up. They remain required
  for the connected cut.

## Structure review

Retained [file metrics](structure-audits/2026-09-08-contextual-variant-owner/file_metrics.csv),
[package metrics](structure-audits/2026-09-08-contextual-variant-owner/package_metrics.csv),
and [findings](structure-audits/2026-09-08-contextual-variant-owner/findings.md)
describe the full checkout. Paths below are under `crates/arcweft-lang-sema/src/`.

| Owner | HEAD LOC | Current LOC | Bytes | Embedded tests | Cohesion disposition |
| --- | ---: | ---: | ---: | ---: | --- |
| `callable/resolver.rs` | 1,412 | 1,634 | 60,763 | 0 | Existing constructor-seed admission consumes the common logical case shape; its remaining closed-type requirement is identified below |
| `final_analysis/analyzer/expressions.rs` | 3,357 | 3,908 | 170,809 | 88 | Existing expression dispatch owns source resolution and defers variant identity sealing |
| `final_analysis/analyzer/patterns.rs` | 1,235 | 1,183 | 47,399 | 0 | Variant patterns now use one prepared owner path; old eager payload identity construction was removed |
| `final_analysis/analyzer/calls/constraints.rs` | 4,241 | 4,940 | 201,875 | 476 | Existing source transaction observes the logical owner |
| `final_analysis/analyzer/evaluated_effects.rs` | 850 | 1,890 | 86,065 | 0 | Existing drop-policy preparation reads its source-phase case evidence |
| `final_analysis/prepared.rs` | 957 | 1,492 | 48,808 | 0 | Expression/pattern carriers retain lifecycle responsibility; variant schema construction moved to its domain owner |
| `final_analysis/nominal_schema.rs` | 2,357 | 2,895 | 119,007 | 0 | The final catalog join seals project and intrinsic cases through the shared owner |
| `final_analysis/tests.rs` | 8,292 | 9,008 | 310,886 | 0 | Existing operational-probe fixture checks the fresh inference ownership model |

The variant model is 434 LOC; its new preparation child is 261 LOC. The
logical payload type projection is 392 LOC. This separation follows raw schema
preparation versus stable owner/case sealing, not a numeric file-size target.
The generic regression module is 172 LOC and the compiler execution matrix is
305 LOC. Growth against HEAD includes inherited work. Dependencies, Cargo
features, transport, persistence, and I/O ownership did not change. Sema
workspace fan-in/out remains 8/14 (development 3/0); compiler remains 3/23
(development 1/5).

## Remaining constructor and callable work

The new `contextual_project_variant_payload_closes_its_owner` fixture calls
`fallback(.Full(42i64), 0i64)` for `Slot<T>` and reads the selected payload in
an if-let body. Both engines fail in semantic analysis with "no admissible
final type". Source inspection locates a remaining constructor boundary:
`AcceptedEnumVariantCase::try_from_checked` still requires a stable owner
digest and sealed payload parameter IDs during callee preparation. The exact
nested-call inference/admission path still needs reconciliation; this record
does not claim the new failure has been repaired or fully localized.

The [active correction request](../reviews/requests/2026-09-08-aw-ah-009.4.2.1.1.1-function-scheme-specialization-and-callable-value-execution.md)
retains that obligation together with scheme specialization, callback effects,
global closure executable lowering, one callable-value execution route,
instance discovery limits, and suspension/restore. This result does not
complete generic constructors, Generic Match C3/C5, View, RuntimePlan, nominal,
or scheduler work. No returned design was accepted, no stable language rule
changed, and no external blocker prevents continuing.
