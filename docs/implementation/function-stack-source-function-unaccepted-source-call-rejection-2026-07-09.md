# Function Stack Source Function Unaccepted Source-Call Rejection - 2026-07-09

This slice strengthens the 07.7 non-helper callable allocation boundary for
source-local function candidates.

## What Changed

- Added
  `checked_runtime_plan_rejects_source_function_partial_when_body_calls_unaccepted_source`.
- Added
  `checked_runtime_plan_rejects_bare_source_function_value_when_body_calls_unaccepted_source`.
- Added
  `checked_runtime_plan_rejects_data_last_source_function_partial_when_body_calls_unaccepted_source`.

Both fixtures define an unsupported source-local function whose body uses an
unaccepted call shape, then define another source-local function that exact
calls that unsupported function. The wrapper function must not become a
runtime function-value candidate merely because its own call is exact.

The checked runtime-plan path now has focused evidence for these surfaces:

- missing-input partial construction rejects as
  `signature_partial_without_helper`;
- data-last partial construction rejects as
  `signature_partial_without_helper`;
- bare function value reference rejects as
  `source_function_value_without_runtime_candidate`.

This does not widen the accepted callable family. It preserves the existing
contract that exact calls to source-local functions inside accepted source
function bodies are admitted only when the callee is already an accepted
source-function candidate.

## Evidence

- `crates/arcweft-compiler/src/tests.rs`
  - `checked_runtime_plan_rejects_source_function_partial_when_body_calls_unaccepted_source`
  - `checked_runtime_plan_rejects_bare_source_function_value_when_body_calls_unaccepted_source`
  - `checked_runtime_plan_rejects_data_last_source_function_partial_when_body_calls_unaccepted_source`

## Validation

```bash
cargo test -p arcweft-compiler --all-features when_body_calls_unaccepted_source -- --nocapture
cargo test -p arcweft-compiler --all-features checked_runtime_plan_rejects_data_last_source_function_partial_when_body_calls -- --nocapture
```

Both commands passed.
