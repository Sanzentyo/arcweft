# Function Stack Effect Row Returned Closure No-Effect Timing - 2026-07-09

This slice adds focused current-graph evidence for returned closure callback
timing under a `no_effect` contract.

## What Changed

- Added `no_effect_rejects_returned_closure_callback_when_called`.
- The fixture creates a returned closure from a higher-order callback, then
  calls the returned function inside a flow that allows `fs.read` but also
  declares `ensures no_effect fs.read`.
- The test asserts that the `no_effect` diagnostic is reported against the
  caller flow only when the returned closure is invoked.

This does not introduce final source-level effect-row syntax or open-row
inference. It strengthens the current delayed-timing evidence that the final
07.8 effect-row contract must preserve.

## Evidence

- `crates/arcweft-lang-sema/src/tests/function_stack.rs`
  - `no_effect_rejects_returned_closure_callback_when_called`

## Validation

```bash
cargo test -p arcweft-lang-sema --all-features no_effect_rejects_returned_closure_callback_when_called -- --nocapture
cargo test -p arcweft-lang-sema --all-features returned_closure_callback_ -- --nocapture
```

Both commands passed.
