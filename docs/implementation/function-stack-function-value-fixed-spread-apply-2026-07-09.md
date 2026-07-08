# Function Stack Function-Value Fixed Spread Apply - 2026-07-09

This note records the narrow accepted source-level spread slice for
`docs/reviews/requests/2026-07-07-seq-07.2.1-function-stack-spread-partial-and-fallback-contract.md`.

## Scope

Function-value calls now accept spread arguments only when the spread source is
an inline fixed-length bracket sequence literal:

```arcw
let add_one = add(1i64)
let ok: i64 = add_one([2i64]...)
```

Both ordinary `BracketSeq` literals and compact numeric `NumericBracketSeq`
literals are accepted. Sema expands the fixed literal length for arity,
remaining-function result type, curried group progress, and argument type
checking. Runtime-plan lowering keeps the source call as
`RuntimeExpr::Apply` with a `RuntimeExpr::SpreadArg`; compact numeric literals
lower to dense runtime sequence values and use the already-verified runtime
apply spread expansion substrate.

This slice did not accept source spread partial construction or data-last
method fallback. A later signature-call slice accepts inline fixed-length
literal spread for direct fixed-parameter signature exact and partial calls;
non-literal spread sources such as `values...` remain rejected because their
runtime length is not known to sema and can change whether the result is an
exact value or another partial function.

## Evidence

Sema coverage:

- `curried_function_value_accepts_fixed_literal_spread_group`
- `curried_function_value_rejects_later_spread_group_with_structured_diagnostic`

Compiler/runtime-plan coverage:

- `checked_runtime_plan_lowers_function_value_fixed_literal_spread_apply`

Related lower-runtime substrate coverage remains recorded in
`docs/implementation/function-stack-apply-spread-runtime-substrate-2026-07-09.md`.

## Remaining Contract Work

The spread request remains open for:

- variable-length source spread partial-call construction;
- source spread data-last method fallback;
- rest-parameter plus data-last receiver interaction;
- typed lowering evidence for runtime-expanded argument ranges;
- variable-length spread sources in function-value calls.

## Validation

```bash
cargo test -p arcweft-lang-sema --all-features curried_function_value_accepts_fixed_literal_spread_group -- --nocapture
cargo test -p arcweft-lang-sema --all-features curried_function_value_rejects_later_spread_group_with_structured_diagnostic -- --nocapture
cargo test -p arcweft-compiler --all-features checked_runtime_plan_lowers_function_value_fixed_literal_spread_apply -- --nocapture
cargo check -p arcweft-lang-sema -p arcweft-compiler --all-targets --all-features
cargo clippy -p arcweft-lang-sema -p arcweft-compiler --all-targets --all-features
cargo fmt --all --check
git diff --check
cargo +nightly -Zscript tools/structure-audit.rs --root . --write docs\implementation\structure-audits\function-stack-function-value-fixed-spread-apply-2026-07-09
```

All commands passed. Clippy reported only existing unrelated warnings in
`arcweft-lang-syntax` large enum variants and sema line-count lint sites. The
structure audit reported 0 errors and 151 warnings.
