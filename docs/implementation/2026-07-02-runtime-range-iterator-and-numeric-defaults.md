# Runtime Range Iterator and Numeric Defaults

Date: 2026-07-02

## Summary

Arcweft now treats range expressions as a typed runtime contract instead of an
unsupported runtime-plan expression. `TypeKind::Range` carries its item type as
`Range<T>`, `RuntimeValue::Range` preserves signed/unsigned integer width, and
`RuntimeIterator` owns the sequential runtime contract used by `for`, `map`,
and `sum`.

Unsuffixed numeric literals now follow stable defaults when no expected type is
available:

- integer literals default to `i32`;
- float literals default to `f64`;
- expected types and explicit suffixes still override the default.

## Implementation Notes

The runtime loop state now stores a `RuntimeIterator` and consumes it through
the standard `Iterator::next()` contract. This intentionally avoids a
`ForNext`-local materialized value array for ranges. `Seq`, `Tuple`, and
`Range` all enter the same iterator contract through
`RuntimeIterator::from_value`.

`arcweft-core::value` now keeps width-preserving integer scalar behavior in
`value/integer.rs` and range/iterator behavior in `value/range.rs`. The
root `value` module remains the intentional facade for `RuntimeValue` and its
public scalar/range boundary types.

AWBC supports the new public contract through `AwbcConstant::Range` and the
typed `core.range(start, end, inclusive)` intrinsic used by expression
lowering. Direct VM evaluation uses `RuntimeExpr::Range`; compact AWBC lowering
uses the intrinsic so both paths produce `RuntimeValue::Range`.

The previous `runtime.plan.lower` error for `Expr::Range` was not a desirable
seq07-style diagnostic. This cut removes that error for supported range
expressions. Other unsupported runtime-plan expressions may still use the
existing string-backed lower diagnostic path and should be moved to structured
diagnostics in a separate diagnostics-focused cut.

## Validation

- `cargo check -p arcweft-core -p arcweft-lang-sema -p arcweft-runtime-plan --all-targets`
- `cargo clippy -p arcweft-core -p arcweft-lang-sema -p arcweft-runtime-plan -p arcweft-render-text -p arcweft-host-adapter -p arcweft-runtime-accelerator -p arcweft-agent-runner -p arcweft-verify -p arcweft-cli --all-targets -- -D warnings`
- `cargo test -p arcweft-core runtime_range_iterates_one_value_at_a_time --all-targets`
- `cargo test -p arcweft-lang-sema unsuffixed_numeric_literals_default_to_stable_widths --all-targets`
- `cargo test -p arcweft-lang-sema range_expression_infers_item_type_from_bound --all-targets`
- `cargo test -p arcweft-runtime-plan runtime_plan_lowers_range_for_source_as_runtime_range_expr --test runtime_plan`
- `cargo run -p arcweft-cli -- check --manifest-path samples/native-text-input/arcw.toml`
- `cargo run -p arcweft-cli -- run samples/native-text-input/src/main.arcw --runner headless --steps 1 --mode drain --max-ops 16`
- `cargo +nightly -Zscript tools/structure-audit.rs --root . --write target/structure-audit-runtime-range`
- `git diff --check`

The structural audit reports `0 error(s), 124 warning(s)` after the value and
range checker splits. The remaining warnings are existing workspace ownership
review items outside this cut.
