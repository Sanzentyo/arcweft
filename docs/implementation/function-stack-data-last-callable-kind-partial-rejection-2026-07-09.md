# Function Stack Data-Last Callable-Kind Partial Rejection - 2026-07-09

This cut adds focused evidence for data-last partial rejection across the
top-level callable kinds that are outside the accepted runtime function-value
subset.

## What Changed

- Added compiler/runtime-plan regressions for data-last partials through:
  - `task fn`
  - `dialogue fn`
  - `stream fn`
- Each fixture uses a pipe form such as `"Ada" |> load_label`, where the pipe
  supplies the final parameter and the result would otherwise be a function
  value for the missing leading parameter.
- Checked runtime-plan lowering rejects each callable kind with unsupported
  callable family `signature_partial_without_helper`.

## Contract

Task, dialogue, and stream functions are not currently materialized as runtime
function values. Their data-last partial forms must therefore fail through the
same structured non-helper allocation boundary as unsupported ordinary source
functions instead of lowering to incomplete direct runtime calls.

This does not implement accepted task/dialogue/stream callable values. That
remains part of the broader 07.7 identity/effect/suspension/AWBC/persistence
contract.

## Evidence

Focused regressions:

- `checked_runtime_plan_rejects_data_last_task_function_partial`
- `checked_runtime_plan_rejects_data_last_dialogue_function_partial`
- `checked_runtime_plan_rejects_data_last_stream_function_partial`

Validation:

```bash
cargo test -p arcweft-compiler --all-features checked_runtime_plan_rejects_data_last_ -- --nocapture
```
