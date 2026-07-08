# Function Stack Non-Helper Source Function Values - 2026-07-09

## Status

First accepted 07.7 expansion implemented.

Runtime-plan lowering can now materialize a narrow subset of ordinary
source-local top-level `fn` declarations as runtime function values without
using the pure-helper table.

## Accepted Contract

The accepted family is intentionally small:

- `FunctionKind::Function` only.
- Exactly one parameter group.
- Fixed parameters only; no rest/default/receiver parameters.
- Parameter patterns must be simple identifier bindings.
- Body must be a final expression or final `return` expression, optionally
  preceded by simple `let` statements.
- Body expressions must lower through strict runtime expression lowering and
  must not contain calls, pipes, closures, `await`, `try`, threads, dialogue
  calls, placeholders, raw syntax, or lifetime paths.

Accepted functions lower to `RuntimeExpr::Function` values. Direct calls lower
to `RuntimeExpr::Apply`. Named missing-input partial calls synthesize a wrapper
function whose parameters are the missing inputs and whose body applies the
materialized source function with arguments in declaration order.

Pure helpers keep priority when a function is also accepted by the pure-helper
candidate pass. Local function-valued bindings keep priority over both
top-level families.

## Behavior

Function-value creation is effect-free. The accepted subset does not contain
call/effect/suspension syntax, so invoking the value cannot perform hidden
host, adapter, or suspension work in this cut.

Product AWBC save/load behavior is unchanged: any escaped runtime
`RuntimeValue::Function` is still rejected by the existing structured
unsupported-runtime-value path.

## Remaining 07.7 Boundaries

These are still not accepted:

- source function values whose bodies contain calls, effects, pipes, closure
  values, `await`, `try`, or other suspension-capable constructs;
- curried non-helper source functions;
- `task fn`, `dialogue fn`, and `stream fn` values;
- trait/impl method values and receiver binding extraction;
- adapter/host-call-backed callable thunks;
- persisted source function values.

Unsupported signature partial calls still fail as
`signature_partial_without_helper` when no pure helper or accepted source
function candidate exists.

## Validation

```bash
cargo test -p arcweft-compiler --all-features checked_runtime_plan_materializes_named_missing_source_function_partial_call -- --nocapture
cargo test -p arcweft-compiler --all-features checked_runtime_plan_rejects_source_function_partial_when_body_calls -- --nocapture
cargo test -p arcweft-compiler --all-features runtime_plan_lowers_non_annotated_function_prefix_partial_with_typecheck -- --nocapture
```
