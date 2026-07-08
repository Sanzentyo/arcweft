# Function Stack Spread Contract Closure - 2026-07-09

## Scope

This note closes the current 07.2.1 spread partial/fallback contract for the
active function-stack goal.

The implementation already supports the executable spread forms that have
deterministic source-level arity:

- runtime `Apply` expands `RuntimeExpr::SpreadArg`;
- function-value calls accept inline fixed-length literal spread;
- direct fixed-parameter signature calls accept inline fixed-length literal
  spread for exact and missing-input partial calls;
- data-last method fallback accepts inline fixed-length literal spread and
  records one evidence entry for the source spread argument.

The remaining question in 07.2.1 was whether variable-length spread should be
accepted in partial-call construction or data-last fallback. The current
language contract answers no: variable-length spread remains a structured
rejection in those shapes. This is no longer a design TODO for the active
goal.

## Contract

- `expr...` is still valid call-argument syntax.
- Ordinary exact calls may use spread only where the callee signature gives a
  deterministic target, such as an existing rest parameter after required
  fixed arguments have been supplied.
- Partial-call construction and data-last fallback accept spread only when the
  spread source is an inline fixed-length bracket sequence or compact numeric
  bracket sequence literal.
- Variable-length spread is rejected in partial-call construction and
  data-last fallback, including spread before later fixed arguments, spread
  with missing named inputs, multiple spreads, placeholder partials mixed with
  spread, and ambiguous spread fallback candidates.
- Runtime `Apply` may still expand spread values internally, including across
  curried runtime-function boundaries after expansion.

The stable language summary is recorded in
`docs/01-language/functions-and-pipeline.md`. The request status in
`docs/reviews/requests/2026-07-07-seq-07.2.1-function-stack-spread-partial-and-fallback-contract.md`
now marks the current-milestone contract closed.

## Validation

```bash
cargo test -p arcweft-lang-sema --all-features rejects_fixed_signature_partial_call_with_spread -- --nocapture
cargo test -p arcweft-lang-sema --all-features fixed_literal_spread_signature_call_typechecks_as_exact_and_partial_application -- --nocapture
cargo test -p arcweft-lang-sema --all-features rejects_partial_call_placeholder_mixed_with_spread -- --nocapture
cargo test -p arcweft-lang-sema --all-features rejects_named_missing_input_partial_call_mixed_with_spread -- --nocapture
cargo test -p arcweft-lang-sema --all-features rejects_partial_call_spread_before_positional_fixed_arg -- --nocapture
cargo test -p arcweft-lang-sema --all-features rejects_partial_call_multiple_spreads_with_structured_diagnostic -- --nocapture
cargo test -p arcweft-lang-sema --all-features method_chain_accepts_fixed_literal_spread_data_last_fallback -- --nocapture
cargo test -p arcweft-lang-sema --all-features method_chain_reports_spread_data_last_fallback_as_unsupported -- --nocapture
cargo test -p arcweft-lang-sema --all-features method_chain_reports_ambiguous_spread_data_last_fallback_candidates -- --nocapture
cargo test -p arcweft-compiler --all-features checked_runtime_plan_lowers_function_value_fixed_literal_spread_apply -- --nocapture
cargo test -p arcweft-compiler --all-features checked_runtime_plan_lowers_signature_fixed_literal_spread_apply -- --nocapture
cargo test -p arcweft-compiler --all-features runtime_plan_lowers_fixed_literal_spread_data_last_method_fallback -- --nocapture
cargo test -p arcweft-core --all-features vm_pure_backend_applies_runtime_function_with_spread_args -- --nocapture
cargo test -p arcweft-core --all-features vm_pure_backend_partially_applies_runtime_function_with_spread_prefix -- --nocapture
cargo test -p arcweft-core --all-features vm_pure_backend_applies_curried_runtime_function_with_spread_args -- --nocapture
cargo fmt --all --check
git diff --check
cargo +nightly -Zscript tools/structure-audit.rs --root . --write docs\implementation\structure-audits\function-stack-spread-contract-closure-2026-07-09
```

All listed checks passed on 2026-07-09. The structural audit scanned 2490
files, 1179 Rust files, and 584131 Rust physical LOC, and reported 0 errors
and 151 existing warnings. The generated evidence is under
`docs/implementation/structure-audits/function-stack-spread-contract-closure-2026-07-09/`.

## Remaining Boundaries

This closure does not implement AWBC suspension-aware dynamic apply, persisted
function snapshots, broad effectful/suspending callable allocation, or the
final closure effect-row model.
