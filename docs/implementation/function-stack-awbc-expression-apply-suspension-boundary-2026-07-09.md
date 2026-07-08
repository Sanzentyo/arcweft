# Function Stack AWBC Expression Apply Suspension Boundary - 2026-07-09

## Scope

This note hardens the current 07.5 AWBC dynamic-apply boundary for the active
function-stack goal.

AWBC `ApplyFunction` is still a synchronous expression instruction. It may
execute AWBC-backed runtime function values that complete without suspension.
It must not claim resumable dynamic-call behavior when the applied function
body suspends or exhausts the expression-apply budget.

## Contract

- Non-suspending `RuntimeValue::Function` bodies may be applied by
  `ApplyFunction`.
- If the applied function reaches an AWBC safe point and returns
  `VmExit::Suspended`, expression apply reports a runtime trap instead of
  producing a resumable fiber state.
- If the applied function exhausts the synchronous expression-apply budget and
  returns `VmExit::BudgetYield`, expression apply reports a runtime trap
  instead of silently yielding and resuming.
- Designing a resumable dynamic apply form remains in
  `docs/reviews/requests/2026-07-07-seq-07.5-function-stack-awbc-closure-apply.md`.

## Implementation

`crates/arcweft-core/src/awbc/tests.rs` now has direct VM regressions for both
rejection paths:

- `expression_apply_reports_suspending_function_body_as_runtime_error` builds a
  verifier-valid AWBC program where `ApplyFunction` invokes a synthetic function
  whose body reaches a `BudgetYield` safe point. The outer expression apply
  returns an `InternalInvariant` trap carrying the runtime suspension message.
- `expression_apply_reports_inner_budget_yield_as_runtime_error` builds a
  verifier-valid straight-line synthetic function whose instruction count
  exhausts the internal synchronous expression-apply budget. The outer
  expression apply returns an `InternalInvariant` trap carrying the explicit
  budget message.

No runtime behavior was widened in this cut.

## Validation

```bash
cargo test -p arcweft-core --all-features expression_apply_reports_suspending_function_body_as_runtime_error -- --nocapture
cargo test -p arcweft-core --all-features expression_apply_reports_inner_budget_yield_as_runtime_error -- --nocapture
cargo test -p arcweft-core --all-features awbc::tests -- --nocapture
cargo clippy -p arcweft-core --all-targets --all-features
cargo fmt --all --check
git diff --check
cargo +nightly -Zscript tools/structure-audit.rs --root . --write docs\implementation\structure-audits\function-stack-awbc-expression-apply-suspension-boundary-2026-07-09
```

All listed checks passed on 2026-07-09. The structural audit scanned 2492
files, 1179 Rust files, and 584358 Rust physical LOC, and reported 0 errors
and 151 existing warnings. The generated evidence is under
`docs/implementation/structure-audits/function-stack-awbc-expression-apply-suspension-boundary-2026-07-09/`.

## Remaining Boundaries

This does not implement resumable dynamic apply. Applying a suspending function
value still needs an explicit AWBC resume-point contract before it can become
an accepted executable language behavior.
