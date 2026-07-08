# Function Stack Closure Effect-Row Audit - 2026-07-09

This note closes the first implementation-order item from
`docs/reviews/requests/2026-07-08-seq-07.8-function-stack-closure-effect-row-final-contract.md`:
audit the implemented closure-effect composition paths and classify them as
stable behavior, temporary evidence-based composition, or diagnostics-only
coverage.

Status: audit complete; the final closure/function effect-row contract is
still open.

Follow-up implementation also connects the existing
`effect_row` data model to the current analyzer through
`EffectAnalysisReport::closed_effect_rows()`. That report projection exposes
closed inferred, upper-bound, and forbidden rows for every callable without
making downstream consumers inspect the temporary effect graph.
The Agent verified-effects builder now consumes this projection when lowering
artifact boundary effect proofs.

## Current Implementation Shape

The current checker does not yet have a source-level effect-row type model.
Instead, it collects an effect graph in `arcweft-lang-sema`:

- source callables such as flows/functions are registered as effect graph
  callables;
- closure literals are registered as private synthetic callables named like
  `closure.expr.N`;
- returned function values are represented by private return proxy callables
  named like `fn.name.return`;
- direct effects and call edges are propagated by the first-order effect
  analyzer;
- checker-specific evidence connects closure values, local aliases, higher-order
  parameters, returned closures, curried groups, and data-last fallback calls to
  that graph.

This is intentionally an evidence graph, not the final row representation.
The row report projection is a stable boundary over that graph: it gives
tooling, artifact proof, and future runtime-plan/verifier code closed rows
today, while keeping the path-specific graph wiring private to sema.

## Stable Behavior To Preserve

These are language/runtime timing rules that should remain true when the final
effect-row model is introduced:

| Behavior | Current evidence |
| --- | --- |
| Creating a closure value is effect-free. | `closure_body_effects_do_not_leak_on_function_value_creation` |
| Partially applying a closure value is effect-free until the resulting value is called. | `partial_local_closure_application_does_not_compose_until_called`; `partial_immediate_closure_application_does_not_compose_until_called` |
| Exact invocation of a local or immediate closure composes the closure body effects into the caller. | `local_closure_call_composes_body_effects_into_caller`; `immediate_closure_call_composes_body_effects_into_caller` |
| Calling a partial closure alias composes the original closure body effects. | `partial_local_closure_alias_composes_body_effects_when_called`; `partial_immediate_closure_alias_composes_body_effects_when_called` |
| `map` and `filter` compose callback effects only because they invoke the callback. | `map_closure_argument_composes_body_effects_into_caller`; `map_local_closure_alias_composes_body_effects_into_caller`; `map_partial_closure_alias_composes_body_effects_into_caller`; `filter_closure_argument_composes_body_effects_into_caller` |
| A higher-order function parameter composes the supplied callback effects only if the parameter is actually invoked. | `user_higher_order_function_argument_composes_when_param_is_called`; `user_higher_order_function_argument_does_not_compose_when_param_is_not_called` |
| Returned closures delay captured callback effects until the returned closure is called. | `returned_closure_callback_does_not_compose_until_closure_is_called`; `returned_closure_callback_composes_when_returned_closure_is_called`; `stored_returned_closure_callback_composes_when_returned_closure_is_called` |
| Later curried callback groups compose only when the reached call group invokes the callback. | `curried_higher_order_function_argument_composes_when_later_group_param_is_called`; `partial_curried_higher_order_callback_does_not_compose_until_final_call`; `partial_curried_higher_order_callback_composes_on_final_call`; `partial_curried_higher_order_callback_composes_on_immediate_final_call` |
| `no_effect` rejects closure body effects when a closure value is actually called, not when it is merely created. | `no_effect_rejects_local_closure_effect_when_called` |
| The current analyzer can project closed row evidence without exposing graph internals. | `closure_effect_rows_project_closed_report_evidence` |
| Agent verified-effects manifests are built from the closed row projection rather than from graph summaries directly. | `compile_agent_bundle_with_project_builds_agent_controller_bundle`; `compile_agent_bundle_lowers_inferred_effects_not_unused_source_upper_bound` |

The new `no_effect_rejects_local_closure_effect_when_called` regression fixes
the 07.8 test requirement that forbidden-effect bounds apply to closure values
whose inferred effects exceed the bound after invocation.

## Temporary Evidence-Based Composition

These paths are implemented and covered, but they should be rewritten to emit
final effect-row evidence rather than retaining their current path-specific
graph wiring:

- synthetic closure callable IDs keyed by expression IDs;
- `last_checked_closure_effect_callable` as an implicit side channel between
  expression checking and later alias/call recording;
- local function-effect alias tables used to reconnect closure values after
  `let` binding;
- function-return proxy callables for returned function values;
- pending higher-order effect calls keyed by callee function name and parameter
  name;
- curried group pending callback evidence;
- tuple/record destructured higher-order callback binding evidence;
- `Option` / `Result`, module-local enum, and environment enum payload metadata
  that lets variant payloads expose callback positions;
- data-last method fallback callback-effect composition.

These are useful implementation evidence, but they are not yet a stable row
data model. The final contract should replace them with explicit sema row
values and typed boundary data.

## Diagnostics-Only Coverage

These paths are user-facing diagnostics that should survive, but they should
not be treated as the final effect-row representation:

- effect traces rendered as diagnostic notes and LSP related information;
- borrowed closure captures crossing suspension boundaries;
- numeric fallback lints inside inferred closure bodies;
- dynamic call diagnostics that report missing function effect signatures;
- unsupported non-helper/effectful/suspending callable allocation diagnostics
  from the 07.7 boundary.

The LSP-facing trace currently verifies that a returned-closure path can surface
steps such as `flow -> returned closure -> higher-order argument -> fs.read`.
That display is valuable, but the underlying trace should eventually be derived
from the final row evidence rather than the temporary graph wiring.

## Remaining Final-Contract Work

The following 07.8 decisions remain open:

1. Source-level effect-row syntax for function values, including explicit rows,
   inferred rows, row bounds, and no-effect constraints.
2. Open-row inference and substitution for function values. The closed-row
   report projection is implemented, but source-level row variables are not
   yet inferred from function signatures.
3. Sema representation for synthetic closures, returned function values,
   curried groups, and higher-order parameters as first-class row-bearing
   callable values rather than temporary graph side channels.
4. Runtime-plan/verifier/LSP consumers for the closed-row report projection.
   Agent artifact verified-effects lowering is the first consumer.
5. Replacement of path-specific closure/higher-order graph edges with final row
   evidence.
6. LSP rendering policy for inferred rows, row origins, callback edges, and
   performed effects.
7. Interaction with 07.7 non-helper callable values and 07.5 suspending dynamic
   apply.

## Validation

```bash
cargo test -p arcweft-lang-sema --all-features no_effect_rejects_local_closure_effect_when_called -- --nocapture
cargo test -p arcweft-lang-sema --all-features closure_effect_rows_project_closed_report_evidence -- --nocapture
cargo test -p arcweft-compiler --all-features compile_agent_bundle_with_project_builds_agent_controller_bundle -- --nocapture
cargo test -p arcweft-compiler --all-features compile_agent_bundle_lowers_inferred_effects_not_unused_source_upper_bound -- --nocapture
```
