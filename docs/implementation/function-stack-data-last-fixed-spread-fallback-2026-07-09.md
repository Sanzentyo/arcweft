# Function Stack Data-Last Fixed Spread Fallback - 2026-07-09

This note records the data-last method fallback slice for
`docs/reviews/requests/2026-07-07-seq-07.2.1-function-stack-spread-partial-and-fallback-contract.md`.

## Scope

Data-last method fallback now accepts spread arguments when every spread source
is an inline fixed-length bracket sequence literal:

```arcw
let direct = score.between([60i64, 90i64]...)
let mixed = score.between([60i64]..., max = 90i64)
```

Sema expands the fixed literal spread for parameter-slot checking, but records
the source spread argument only once in `DataLastMethodFallbackArg` evidence.
This prevents a multi-slot spread from being lowered repeatedly. Runtime-plan
lowering now accepts the evidence-backed spread argument and preserves it as a
single `RuntimeExpr::SpreadArg` in the proven data-last order.

The checker also reserves the fixed spread container expression in its
expression-id traversal before checking bracket-sequence item expressions. That
keeps sema evidence ids aligned with runtime-plan lowering when a later
expression in the same flow also needs typed lowering evidence. Compact
numeric bracket sequences reserve only the container, matching their runtime
AST shape.

## Remaining Contract Work

The spread request remains open for variable-length spread sources and for any
runtime-expanded argument range that cannot be proven from an inline fixed
literal. In particular, `score.above(thresholds...)` remains a structured
unsupported data-last fallback diagnostic.

## Evidence

Sema coverage:

- `method_chain_accepts_fixed_literal_spread_data_last_fallback`
- `method_chain_reports_spread_data_last_fallback_as_unsupported`

Compiler/runtime-plan coverage:

- `runtime_plan_lowers_fixed_literal_spread_data_last_method_fallback`
- `checked_runtime_plan_lowers_signature_fixed_literal_spread_apply`

Related spread evidence:

- `docs/implementation/function-stack-apply-spread-runtime-substrate-2026-07-09.md`
- `docs/implementation/function-stack-function-value-fixed-spread-apply-2026-07-09.md`
- `docs/implementation/function-stack-signature-fixed-spread-apply-2026-07-09.md`

## Validation

```bash
cargo test -p arcweft-lang-sema --all-features method_chain_accepts_fixed_literal_spread_data_last_fallback -- --nocapture
cargo test -p arcweft-lang-sema --all-features method_chain_reports_spread_data_last_fallback_as_unsupported -- --nocapture
cargo test -p arcweft-lang-sema --all-features method_chain_reports_spread_then_named_data_last_fallback_as_unsupported -- --nocapture
cargo test -p arcweft-lang-sema --all-features method_chain_reports_multiple_spread_data_last_fallback_as_unsupported -- --nocapture
cargo test -p arcweft-lang-sema --all-features method_chain_reports_ambiguous_spread_data_last_fallback_candidates -- --nocapture
cargo test -p arcweft-compiler --all-features runtime_plan_lowers_fixed_literal_spread_data_last_method_fallback -- --nocapture
cargo test -p arcweft-compiler --all-features checked_runtime_plan_lowers_signature_fixed_literal_spread_apply -- --nocapture
cargo check -p arcweft-lang-sema -p arcweft-runtime-plan -p arcweft-compiler --all-targets --all-features
cargo clippy -p arcweft-lang-sema -p arcweft-runtime-plan -p arcweft-compiler --all-targets --all-features
cargo fmt --all --check
cargo +nightly -Zscript tools/structure-audit.rs --root . --write docs\implementation\structure-audits\function-stack-data-last-fixed-spread-fallback-2026-07-09
git diff --check
```

All listed commands passed. Clippy reported only existing unrelated warnings in
`arcweft-lang-syntax` large enum variants and sema line-count lint sites. The
structure audit reported 0 errors and 151 warnings, recorded under
`docs/implementation/structure-audits/function-stack-data-last-fixed-spread-fallback-2026-07-09/`.
