# Function Stack Signature Fixed Spread Apply - 2026-07-09

This note records the fixed-signature spread slice for
`docs/reviews/requests/2026-07-07-seq-07.2.1-function-stack-spread-partial-and-fallback-contract.md`.

## Scope

Direct signature calls now accept spread arguments when the spread source is an
inline fixed-length bracket sequence literal and the callable has only fixed
parameters:

```arcw
let exact: i64 = add([1i64, 2i64]...)
let also_exact: i64 = add([1i64]..., 2i64)
let add_one = add([1i64]...)
```

Sema expands `BracketSeq` and compact `NumericBracketSeq` spread sources into
fixed positional parameter slots for exact calls, missing-input partial calls,
and curried current-group completion checks. Runtime-plan lowering preserves
the source spread as `RuntimeExpr::SpreadArg`; runtime `Apply` and pure-call
evaluation expand that argument at execution time.

This slice intentionally keeps variable-length spread sources rejected in
partial-call construction. It also does not change rest-parameter semantics:
when a signature has a rest parameter, spread remains the existing rest
sequence operation rather than fixed-slot expansion.

## Evidence

Sema coverage:

- `fixed_literal_spread_signature_call_typechecks_as_exact_and_partial_application`
- `rejects_fixed_signature_partial_call_with_spread`
- `rejects_partial_call_spread_before_positional_fixed_arg`
- `rejects_partial_call_multiple_spreads_with_structured_diagnostic`

Compiler/runtime-plan coverage:

- `checked_runtime_plan_lowers_signature_fixed_literal_spread_apply`

Related function-value and runtime substrate evidence:

- `docs/implementation/function-stack-function-value-fixed-spread-apply-2026-07-09.md`
- `docs/implementation/function-stack-apply-spread-runtime-substrate-2026-07-09.md`

## Remaining Contract Work

The spread request remains open for:

- data-last method fallback with spread;
- variable-length spread in partial-call construction;
- rest-parameter plus fixed-slot spread interactions beyond the existing rest
  spread rule;
- typed lowering evidence for runtime-expanded argument ranges where source
  argument indices are no longer enough.

## Validation

```bash
cargo test -p arcweft-lang-sema --all-features fixed_literal_spread_signature_call_typechecks_as_exact_and_partial_application -- --nocapture
cargo test -p arcweft-lang-sema --all-features rejects_fixed_signature_partial_call_with_spread -- --nocapture
cargo test -p arcweft-lang-sema --all-features rejects_partial_call_spread_before_positional_fixed_arg -- --nocapture
cargo test -p arcweft-lang-sema --all-features rejects_partial_call_multiple_spreads_with_structured_diagnostic -- --nocapture
cargo test -p arcweft-compiler --all-features checked_runtime_plan_lowers_signature_fixed_literal_spread_apply -- --nocapture
cargo check -p arcweft-lang-sema -p arcweft-compiler --all-targets --all-features
cargo clippy -p arcweft-lang-sema -p arcweft-compiler --all-targets --all-features
cargo fmt --all --check
cargo +nightly -Zscript tools/structure-audit.rs --root . --write docs\implementation\structure-audits\function-stack-signature-fixed-spread-apply-2026-07-09
git diff --check
```

All listed commands passed. Clippy reported only existing unrelated warnings in
`arcweft-lang-syntax` large enum variants and sema line-count lint sites. The
structure audit reported 0 errors and 151 warnings, recorded under
`docs/implementation/structure-audits/function-stack-signature-fixed-spread-apply-2026-07-09/`.
