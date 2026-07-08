# Function Stack Non-Helper Callable Inventory - 2026-07-08

## Purpose

This note closes the inventory step from
`docs/reviews/requests/2026-07-08-seq-07.7-function-stack-non-helper-callable-allocation.md`.
It classifies callable families by their current function-value behavior and
records which families remain blocked on larger contracts.

Status: inventory complete; the first narrow non-helper source function value
cut is implemented; general non-helper callable allocation is still not
complete.

## Current Accepted Function-Value Families

| Family | Current behavior | Evidence |
| --- | --- | --- |
| Expression closures | `RuntimeExpr::Function` and `RuntimeExpr::Apply` execute in the core evaluator; non-suspending AWBC closure/apply is implemented. | `docs/implementation/2026-07-07-functions-closures-pipeline-language-stack.md`; AWBC 07.5 request implemented-boundary section. |
| Local aliases to function values | Calls through local function-valued bindings lower to `RuntimeExpr::Apply` from typed evidence. | Compiler/runtime-plan function-stack regressions. |
| Helper-backed top-level `fn` values | Bare helper names and prefix partial calls materialize executable runtime functions when pure-helper lowering succeeds. | `runtime_plan_lowers_non_annotated_function_prefix_partial_with_typecheck`; `runtime_plan_lowers_named_missing_inferred_helper_input`. |
| Simple non-helper source-local `fn` values | Ordinary `fn` declarations with simple identifier parameters and expression bodies that contain no call/effect/suspension-capable syntax materialize as `RuntimeExpr::Function` without using the pure-helper table. Multiple curried `ParamGroup`s lower to nested functions. Returned simple closure literals also lower to nested runtime functions when the closure body stays inside the same accepted expression subset. Named missing-input partial calls synthesize wrapper functions that preserve declaration argument order. | `docs/implementation/function-stack-non-helper-source-function-values-2026-07-09.md`; `checked_runtime_plan_materializes_named_missing_source_function_partial_call`; `checked_runtime_plan_materializes_curried_source_function_value`; `checked_runtime_plan_materializes_source_function_returned_closure`. |
| Data-last pipes through helper/local function values | Fixed data-last order lowers through helper or local function-value apply. | `runtime_plan_lowers_local_function_data_last_pipe_to_apply`; typed data-last method fallback regressions. |
| Non-suspending AWBC-backed generated closures | AWBC lowers generated functions with `MakeFunction` and executes `ApplyFunction` for exact, partial, and chained apply when no suspension occurs. | `docs/reviews/requests/2026-07-07-seq-07.5-function-stack-awbc-closure-apply.md`. |

## Current Rejected Or Deferred Families

| Family | Current behavior | Reason / next contract |
| --- | --- | --- |
| Signature partial without helper lowering | Checked runtime-plan lowering rejects this as `unsupported callable family signature_partial_without_helper` when no pure helper and no accepted source function-value candidate exists. | Needs additional 07.7 callable representations for call-bearing/effectful/suspending/adapter-backed bodies. |
| Effectful top-level callable values | Creation timing, call timing, and row composition remain under the effect-row final contract. | Blocked on 07.8 plus 07.7 representation. |
| Suspending callable values | Dynamic apply that may suspend has no resumable AWBC safe point contract. | Blocked on 07.5 dynamic apply/resume design. |
| `task fn`, `dialogue fn`, and `stream fn` values | Their curried signatures type-check, but materializing them as runtime function values is not accepted. | Needs callable identity, suspension/effects, and runtime lowering contract. |
| Trait/impl method values | Curried method calls are checked, but extracting method values with receiver binding is not accepted. | Needs explicit receiver binding representation and trait dispatch lowering. |
| Adapter/host-call-backed callable values | Adapter calls remain call requests, not first-class function values. | Needs thunk representation, effects, suspension, and save/load policy. |
| Persisted function values | Product AWBC save/load rejects `RuntimeValue::Function`. | Serializable closure/callable snapshots are blocked on 07.5. |

## Diagnostic Boundary

Runtime-plan lowering now marks unsupported helper-less signature partials as:

```text
unsupported callable family `signature_partial_without_helper`
```

The marker is intentionally narrow: it means sema proved a signature partial
call, but compiler/runtime-plan did not have an executable helper or accepted
callable representation for that callee. It does not claim to distinguish every
future family yet.

## Remaining 07.7 Work

The first accepted expansion beyond helper-backed top-level function values is
the simple source-local `fn` family documented in
`docs/implementation/function-stack-non-helper-source-function-values-2026-07-09.md`.

The remaining design decisions must specify:

- runtime representation for call-bearing/effectful/suspending callable
  values;
- effect timing once body calls or adapter effects are accepted;
- suspension and AWBC behavior for dynamic apply that may yield;
- curried group handling for non-helper source functions;
- save/load behavior for escaped callable values;
- diagnostics for families that remain rejected.

Until that contract exists, existing helper-backed/local closure behavior
should not be redesigned, and helper-less signature partials must keep failing
with the explicit unsupported-family diagnostic.

## Validation

```bash
cargo test -p arcweft-compiler --all-features checked_runtime_plan_materializes_named_missing_source_function_partial_call -- --nocapture
cargo test -p arcweft-compiler --all-features checked_runtime_plan_materializes_curried_source_function_value -- --nocapture
cargo test -p arcweft-compiler --all-features checked_runtime_plan_materializes_source_function_returned_closure -- --nocapture
cargo test -p arcweft-compiler --all-features checked_runtime_plan_rejects_source_function_partial_when_body_calls -- --nocapture
cargo check -p arcweft-runtime-plan -p arcweft-compiler --all-targets --all-features
cargo clippy -p arcweft-runtime-plan -p arcweft-compiler --all-targets --all-features
cargo fmt -p arcweft-runtime-plan -p arcweft-compiler -- --check
git diff --check -- crates/arcweft-runtime-plan/src/expr.rs crates/arcweft-compiler/src/tests.rs docs/implementation/function-stack-non-helper-callable-inventory-2026-07-08.md docs/implementation/function-stack-current-status-2026-07-08.md docs/implementation/function-stack-goal-completion-audit-2026-07-08.md docs/implementation/2026-07-07-functions-closures-pipeline-language-stack.md docs/implementation/current-work-status-2026-07-08.md docs/implementation/function-stack-request-split-audit-2026-07-08.md
cargo +nightly -Zscript tools/structure-audit.rs --root .
```
