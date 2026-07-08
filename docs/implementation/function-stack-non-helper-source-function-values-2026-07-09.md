# Function Stack Non-Helper Source Function Values - 2026-07-09

## Status

First accepted 07.7 expansion implemented.

Runtime-plan lowering can now materialize a narrow subset of ordinary
source-local top-level `fn` declarations as runtime function values without
using the pure-helper table. The first follow-up expands that subset from one
parameter group to multiple curried `ParamGroup`s by lowering each group to a
nested `RuntimeExpr::Function`. The second follow-up accepts source function
bodies that return simple closure literals, so a source-local function can now
return another runtime function without being forced through pure-helper
lowering. The third follow-up accepts direct calls to function-typed
parameters, lowering them to `RuntimeExpr::Apply` against the local function
value instead of adapter-style `RuntimeExpr::Call`. The fourth follow-up tracks
function-valued `let` bindings derived from aliases, closure literals, and
partial calls to function-typed parameters, so a source function can bind a
partially applied callback and invoke it later in the same accepted body.

## Accepted Contract

The accepted family is intentionally small:

- `FunctionKind::Function` only.
- One or more parameter groups.
- Fixed parameters only; no rest/default/receiver parameters.
- Parameter patterns must be simple identifier bindings.
- Body must be a final expression or final `return` expression, optionally
  preceded by simple `let` statements.
- Body expressions must lower through strict runtime expression lowering and
  must not contain host/top-level calls, pipes, `await`, `try`, threads,
  dialogue calls, placeholders, raw syntax, or lifetime paths.
- Closure literal expressions are accepted when their parameters are simple
  identifiers and their body recursively satisfies this accepted contract.
- Direct calls to parameters whose declared type is a function type are
  accepted when all provided arguments are positional accepted expressions and
  the call does not exceed the declared function arity.
- Simple `let` bindings become local function values inside the accepted body
  when their expression is a function-typed parameter alias, a simple closure
  literal, or a partial call to an existing local function value whose result
  type is still a function.
- `let`, `if let`, `match`, and closure parameter patterns that would shadow a
  function-typed parameter name keep the source function outside this accepted
  subset.

Accepted functions lower to `RuntimeExpr::Function` values. Curried groups
lower to nested functions so evaluating an inner group captures earlier group
arguments through the existing runtime closure environment. Direct calls lower
to `RuntimeExpr::Apply`. Named missing-input partial calls in the current group
synthesize a wrapper function whose parameters are the missing inputs and whose
body applies the materialized source function with arguments in declaration
order.
Direct calls to function-typed parameters lower as `RuntimeExpr::Apply` with a
local callee, preserving higher-order source functions without pretending the
call is an adapter or top-level runtime call. Function-valued `let` bindings
lower as ordinary `RuntimeExpr::Let` values, and later calls to those locals
also lower as local `RuntimeExpr::Apply`.

Pure helpers keep priority when a function is also accepted by the pure-helper
candidate pass. Local function-valued bindings keep priority over both
top-level families.

## Behavior

Function-value creation is effect-free. The accepted subset does not contain
host/effect/suspension syntax outside recursively accepted closure literals,
direct calls to supplied function values, and local aliases/partials of those
function values, so creating the value cannot perform hidden host, adapter, or
suspension work in this cut. Calling a supplied function value composes that
value's behavior at invocation time through the existing runtime `Apply` path.
Returning a closure only allocates another `RuntimeExpr::Function`; its body is
still constrained by the same accepted subset.

Product AWBC save/load behavior is unchanged: any escaped runtime
`RuntimeValue::Function` is still rejected by the existing structured
unsupported-runtime-value path.

## Remaining 07.7 Boundaries

These are still not accepted:

- source function values whose bodies contain host/top-level calls, effects,
  pipes, closure bodies with host/effect calls, `await`, `try`, or other
  suspension-capable constructs;
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
cargo test -p arcweft-compiler --all-features checked_runtime_plan_materializes_curried_source_function_value -- --nocapture
cargo test -p arcweft-compiler --all-features checked_runtime_plan_materializes_source_function_returned_closure -- --nocapture
cargo test -p arcweft-compiler --all-features checked_runtime_plan_materializes_source_function_callback_param_call -- --nocapture
cargo test -p arcweft-compiler --all-features checked_runtime_plan_materializes_source_function_callback_partial_let -- --nocapture
cargo test -p arcweft-compiler --all-features checked_runtime_plan_rejects_source_function_partial_when_body_calls -- --nocapture
cargo test -p arcweft-compiler --all-features runtime_plan_lowers_non_annotated_function_prefix_partial_with_typecheck -- --nocapture
```
