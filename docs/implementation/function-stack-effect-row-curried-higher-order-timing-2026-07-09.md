# Function Stack Effect-Row Curried Higher-Order Timing - 2026-07-09

This cut strengthens the current 07.8 evidence for curried higher-order
callback timing.

## What Changed

- Added
  `no_effect_rejects_partial_curried_higher_order_callback_on_final_call`.
- The fixture creates a curried higher-order stage, supplies the callback in a
  later group, and leaves one argument missing.
- Creating the partial remains effect-free under the existing tests.
- Calling the final partial value now has explicit `no_effect` coverage: the
  callback body effect is rejected only when the final call reaches the group
  that invokes the callback.

## Contract

Curried higher-order calls compose callback effects only when execution reaches
the call group that invokes the callback. Partially supplying earlier groups,
or creating a partial value from a later group, does not itself perform the
callback effects.

This is still current graph evidence. It does not finalize source-level
effect-row syntax, open-row inference, row variables, or row-bearing callable
values.

## Evidence

Existing timing regressions:

- `curried_higher_order_function_argument_composes_when_later_group_param_is_called`
- `curried_higher_order_function_alias_composes_when_later_group_param_is_called`
- `partial_curried_higher_order_callback_does_not_compose_until_final_call`
- `partial_curried_higher_order_callback_composes_on_final_call`
- `partial_curried_higher_order_callback_composes_on_immediate_final_call`

New forbidden-row regression:

- `no_effect_rejects_partial_curried_higher_order_callback_on_final_call`

Validation:

```bash
cargo test -p arcweft-lang-sema --all-features no_effect_rejects_partial_curried_higher_order_callback_on_final_call -- --nocapture
```
