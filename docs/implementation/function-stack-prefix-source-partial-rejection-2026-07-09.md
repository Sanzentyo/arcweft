# Function Stack Prefix Source-Partial Rejection - 2026-07-09

This cut adds focused evidence for the 07.7 prefix partial-call rejection
boundary.

## What Changed

- Added a compiler/runtime-plan regression for a positional prefix partial call
  to a source-local `fn` outside the accepted runtime-function subset:
  `let trim_head = trim_right("head")`.
- The fixture uses a source function body containing a method call
  (`right.trim()`), which remains outside the accepted source-local function
  materialization contract.
- Checked runtime-plan lowering rejects the partial with unsupported callable
  family `signature_partial_without_helper`.

## Contract

Positional prefix partials and named missing-input partials share the same
current rejection boundary when the callee is a source-local function that has
no pure-helper lowering and no accepted runtime function-value candidate.

This does not accept effectful, suspending, host/adapter-backed, or otherwise
unaccepted source-function values. It only proves that the existing structured
rejection applies to the prefix partial shape called out by 07.7.

## Evidence

Focused regression:

- `checked_runtime_plan_rejects_prefix_source_function_partial_when_body_calls`

Validation:

```bash
cargo test -p arcweft-compiler --all-features checked_runtime_plan_rejects_prefix_source_function_partial_when_body_calls -- --nocapture
```
