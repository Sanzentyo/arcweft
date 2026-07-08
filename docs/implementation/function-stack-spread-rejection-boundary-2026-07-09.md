# Function Stack Spread Rejection Boundary - 2026-07-09

This note records the focused hardening slice for
`docs/reviews/requests/2026-07-07-seq-07.2.1-function-stack-spread-partial-and-fallback-contract.md`.

## Scope

This slice does not accept executable spread partial application or executable
spread data-last fallback. It keeps the current language/runtime boundary:
regular rest-parameter calls may use spread where the existing rest contract
already applies, while partial-call construction and data-last fallback reject
spread until the runtime expansion/evidence contract is designed.

The change makes the rejection boundary more precise:

- partial-call construction with multiple spreads now reports
  `UnsupportedSignaturePartialCall` with a multiple-spread reason;
- partial-call construction where a spread is followed by positional or named
  fixed arguments now reports `UnsupportedSignaturePartialCall` with a runtime
  expansion order reason;
- data-last fallback with multiple spreads now reports
  `UnsupportedDataLastMethodFallback` with a multiple-spread reason;
- data-last fallback where a spread is followed by a named fixed argument now
  reports `UnsupportedDataLastMethodFallback` with a runtime argument order
  reason;
- all rejected cases continue to avoid recording selected partial/fallback
  lowering evidence.

## Evidence

Focused sema coverage:

- `rejects_partial_call_spread_before_positional_fixed_arg`
- `rejects_partial_call_spread_before_named_fixed_arg`
- `rejects_partial_call_multiple_spreads_with_structured_diagnostic`
- `method_chain_reports_spread_then_named_data_last_fallback_as_unsupported`
- `method_chain_reports_multiple_spread_data_last_fallback_as_unsupported`

These extend the existing coverage for plain spread partials, `_` placeholder
spread partials, named missing-input spread partials, curried first/later call
groups, unsupported single-spread fallback, and ambiguous spread fallback.

## Remaining Contract Work

The request remains open for any accepted executable spread semantics:

- how spread maps onto fixed parameters, rest parameters, and missing partial
  inputs;
- whether spread may cross curried call-group boundaries;
- typed lowering evidence for runtime-expanded argument ranges;
- runtime-plan/AWBC lowering for any accepted spread partial/fallback shape.

## Validation

```bash
cargo test -p arcweft-lang-sema --all-features spread -- --nocapture
cargo check -p arcweft-lang-sema --all-targets --all-features
cargo clippy -p arcweft-lang-sema --all-targets --all-features
cargo +nightly -Zscript tools/structure-audit.rs --root . --write docs\implementation\structure-audits\function-stack-spread-rejection-boundary-2026-07-09
rustfmt --edition 2024 --check crates\arcweft-lang-sema\src\checker\expr\signature_call.rs crates\arcweft-lang-sema\src\checker\expr\method_fallback.rs crates\arcweft-lang-sema\src\tests\function_stack.rs
```

The focused spread test filter ran 12 sema tests. `cargo check` passed.
`cargo clippy` passed with pre-existing warnings in `arcweft-lang-syntax` and
`arcweft-lang-sema`. The structure audit reported 0 errors and 151 warnings.
