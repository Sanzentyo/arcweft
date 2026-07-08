# Function Stack Non-Helper Callable Inventory - 2026-07-08

## Purpose

This note closes the inventory step from
`docs/reviews/requests/2026-07-08-seq-07.7-function-stack-non-helper-callable-allocation.md`.
It classifies callable families by their current function-value behavior and
records which families remain blocked on larger contracts.

Status: inventory complete; general non-helper callable allocation is still not
complete.

## Current Accepted Function-Value Families

| Family | Current behavior | Evidence |
| --- | --- | --- |
| Expression closures | `RuntimeExpr::Function` and `RuntimeExpr::Apply` execute in the core evaluator; non-suspending AWBC closure/apply is implemented. | `docs/implementation/2026-07-07-functions-closures-pipeline-language-stack.md`; AWBC 07.5 request implemented-boundary section. |
| Local aliases to function values | Calls through local function-valued bindings lower to `RuntimeExpr::Apply` from typed evidence. | Compiler/runtime-plan function-stack regressions. |
| Helper-backed top-level `fn` values | Bare helper names and prefix partial calls materialize executable runtime functions when pure-helper lowering succeeds. | `runtime_plan_lowers_non_annotated_function_prefix_partial_with_typecheck`; `runtime_plan_lowers_named_missing_inferred_helper_input`. |
| Data-last pipes through helper/local function values | Fixed data-last order lowers through helper or local function-value apply. | `runtime_plan_lowers_local_function_data_last_pipe_to_apply`; typed data-last method fallback regressions. |
| Non-suspending AWBC-backed generated closures | AWBC lowers generated functions with `MakeFunction` and executes `ApplyFunction` for exact, partial, and chained apply when no suspension occurs. | `docs/reviews/requests/2026-07-07-seq-07.5-function-stack-awbc-closure-apply.md`. |

## Current Rejected Or Deferred Families

| Family | Current behavior | Reason / next contract |
| --- | --- | --- |
| Signature partial without helper lowering | Checked runtime-plan lowering rejects this as `unsupported callable family signature_partial_without_helper`. | Needs 07.7 accepted callable representation before lowering can produce a function value. |
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

The next design decision is to choose the first accepted expansion beyond
helper-backed top-level function values. That decision must specify:

- runtime representation;
- effect timing;
- suspension and AWBC behavior;
- curried group handling;
- save/load behavior;
- diagnostics for families that remain rejected.

Until that contract exists, existing helper-backed/local closure behavior
should not be redesigned, and helper-less signature partials must keep failing
with the explicit unsupported-family diagnostic.

## Validation

```bash
cargo test -p arcweft-compiler --all-features checked_runtime_plan_rejects_non_helper_signature_partial_call -- --nocapture
cargo check -p arcweft-runtime-plan -p arcweft-compiler --all-targets --all-features
cargo clippy -p arcweft-runtime-plan -p arcweft-compiler --all-targets --all-features
cargo fmt -p arcweft-runtime-plan -p arcweft-compiler -- --check
git diff --check -- crates/arcweft-runtime-plan/src/expr.rs crates/arcweft-compiler/src/tests.rs docs/implementation/function-stack-non-helper-callable-inventory-2026-07-08.md docs/implementation/function-stack-current-status-2026-07-08.md docs/implementation/function-stack-goal-completion-audit-2026-07-08.md docs/implementation/2026-07-07-functions-closures-pipeline-language-stack.md docs/implementation/current-work-status-2026-07-08.md docs/implementation/function-stack-request-split-audit-2026-07-08.md
cargo +nightly -Zscript tools/structure-audit.rs --root .
```
