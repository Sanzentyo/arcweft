# Function Stack Apply Spread Runtime Substrate - 2026-07-09

This note records a narrow substrate validation for
`docs/reviews/requests/2026-07-07-seq-07.2.1-function-stack-spread-partial-and-fallback-contract.md`.

## Scope

This slice does not accept new source-level spread partial application or
source-level spread data-last fallback shapes. It verifies the lower runtime
piece those future shapes would need:

- `RuntimeExpr::Apply` evaluates `RuntimeExpr::SpreadArg` entries through the
  same call-argument expansion path as ordinary runtime calls.
- Spread expansion may satisfy a function exactly.
- Spread expansion may supply a prefix that produces a partial runtime function.
- Spread expansion may cross curried runtime function boundaries when an apply
  receives more values than the current function arity.

The surface language still rejects spread partial construction and spread
data-last fallback unless a later contract explicitly accepts a shape.

## Contract Finding

The runtime substrate is already capable of applying spread-expanded argument
values to function values. The remaining blocker is not low-level evaluation;
it is the source-level mapping contract.

For data-last fallback, Arcweft currently models the receiver as the final
callable parameter. Arcweft source rest parameters are also constrained to be
the final parameter. Therefore a source signature cannot simultaneously place
a rest parameter immediately before the receiver and keep the receiver as the
final data-last parameter. Accepting spread data-last fallback will require one
of these explicit design decisions:

1. keep spread data-last fallback rejected;
2. change the fallback contract so the receiver can be inserted before a final
   rest parameter;
3. introduce a separate source spelling for the data-last receiver slot; or
4. accept only a provably fixed-length spread form with typed length evidence.

Until one of those options is chosen, sema/runtime-plan should continue to
reject source spread data-last fallback with structured diagnostics.

## Evidence

Runtime regression coverage:

- `vm_pure_backend_applies_runtime_function_with_spread_args`
- `vm_pure_backend_partially_applies_runtime_function_with_spread_prefix`
- `vm_pure_backend_applies_curried_runtime_function_with_spread_args`

These tests sit next to the existing runtime function value tests in
`crates/arcweft-core/src/tests/pure.rs`.

## Validation

```bash
cargo test -p arcweft-core --all-features vm_pure_backend_applies_runtime_function_with_spread_args -- --nocapture
cargo test -p arcweft-core --all-features vm_pure_backend_partially_applies_runtime_function_with_spread_prefix -- --nocapture
cargo test -p arcweft-core --all-features vm_pure_backend_applies_curried_runtime_function_with_spread_args -- --nocapture
cargo test -p arcweft-core --all-features vm_pure_backend_ -- --nocapture
rustfmt --edition 2024 --check crates\arcweft-core\src\tests\pure.rs
git diff --check
cargo check -p arcweft-core --all-targets --all-features
cargo clippy -p arcweft-core --all-targets --all-features
cargo +nightly -Zscript tools/structure-audit.rs --root . --write docs\implementation\structure-audits\function-stack-apply-spread-runtime-substrate-2026-07-09
```

All commands passed. The focused VM pure backend filter covered 21 tests. The
structure audit scanned 2478 files / 1179 Rust files / 583089 Rust physical LOC
and reported 0 errors / 151 warnings.
