# Function Stack Effect-Row Curried Higher-Order Timing - 2026-07-09

This note records the current 07.8 implementation evidence for open inferred
rows and curried callable timing.

## What Changed

- Added
  `no_effect_rejects_partial_curried_higher_order_callback_on_final_call`.
- The fixture creates a curried higher-order stage, supplies the callback in a
  later group, and leaves one argument missing.
- Creating the partial remains effect-free under the existing tests.
- Calling the final partial value now has explicit `no_effect` coverage: the
  callback body effect is rejected only when the final call reaches the group
  that invokes the callback.
- Every inferred source `fn` and analyzable closure now receives a fresh
  `EffectVar` from the collector-owned `EffectVarSupply`. Raw row reports keep
  that tail visible, while the report-owned substitution produces the closed
  boundary consumed by artifact code.
- Omitted source-function rows and inferred closure rows therefore no longer
  use `Unknown` when their bodies are analyzable. Explicit source rows and
  expected closure rows remain closed contracts.
- A curried source function places its body row only on the function type for
  the final call group. Direct calls, aliases, partial first-group calls, and
  data-last method fallback retain the source callable and pending callback
  evidence until that group is reached.
- Higher-order arguments supplied to an earlier or partially supplied group
  are retained as typed pending callback evidence. Supplying the value does not
  perform its body effects.
- Curried ordinary functions that return a function now register the returned
  callable proxy from the terminal body return type, not from an intermediate
  curry wrapper. Completing the final source group remains effect-free when it
  only creates the returned closure; invoking that closure composes its body
  effects.
- Data-last fallback now treats a function-typed receiver as a real
  higher-order argument. Local callable aliases retain the canonical source
  function identity alongside signature metadata, so source-body and callback
  edges are not keyed to a lexical alias that the effect graph cannot resolve.
- Open inferred rows and final-group invocation timing are limited to ordinary
  `fn` and closures. `task fn`, `dialogue fn`, and `stream fn` retain unknown
  function-value rows and the pre-existing eager graph approximation until
  their execution ABI is specified.

## Contract

Curried higher-order calls compose callback effects only when execution reaches
the call group that invokes the callback. Partially supplying earlier groups,
or creating a partial value from a later group, does not itself perform the
callback effects.

Inferred source functions and closures expose open raw rows such as
`{ fs.read | e3 }`; `EffectAnalysisReport::closed_effect_rows()` resolves those
fresh variables for consumers that require a closed boundary. This does not
define the execution ABI for `task fn`, `dialogue fn`, or `stream fn`; that
larger contract is split into
`docs/reviews/requests/2026-07-08-seq-07.8.1-task-dialogue-stream-callable-effect-abi.md`.

## Evidence

Existing timing regressions:

- `curried_higher_order_function_argument_composes_when_later_group_param_is_called`
- `curried_higher_order_function_alias_composes_when_later_group_param_is_called`
- `partial_curried_higher_order_callback_does_not_compose_until_final_call`
- `partial_curried_higher_order_callback_composes_on_final_call`
- `partial_curried_higher_order_callback_composes_on_immediate_final_call`

New forbidden-row regression:

- `no_effect_rejects_partial_curried_higher_order_callback_on_final_call`

Additional final-row and source-call timing regressions:

- `inferred_source_function_value_uses_an_open_row_that_closes_from_its_body`
- `analyzable_closure_type_and_report_use_a_resolved_open_effect_row`
- `curried_source_body_effects_begin_only_at_the_final_call_group`
- `curried_source_function_alias_preserves_final_group_effect_timing`
- `partial_first_curried_group_preserves_final_group_effect_timing`
- `first_group_callback_effect_is_deferred_until_curried_body_invocation`
- `partial_uncurried_callback_effect_is_deferred_until_exact_application`
- `curried_method_fallback_defers_first_group_callback_until_final_call`
- `curried_source_returned_closure_keeps_delayed_effect_proxy`
- `data_last_function_receiver_composes_when_curried_body_invokes_it`
- `data_last_callable_alias_retains_source_body_and_callback_identity`
- `suspending_callable_kinds_do_not_claim_ordinary_open_rows`

Test ownership was also split by responsibility:

- `function_effect_rows.rs`: row inference, contracts, and report boundaries;
- `function_effect_timing.rs`: closure, higher-order, and curried call timing;
- `function_method_fallback.rs`: data-last and method-resolution behavior;
- `function_stack.rs`: the remaining function/closure language surface.

Current physical LOC after the split is 681, 1,689, 919, and 2,113
respectively; every test module is below the 2,500-LOC warning threshold.

Validation:

```bash
cargo check -p arcweft-lang-sema --all-targets --all-features
cargo test -p arcweft-lang-sema --lib function_effect_rows --all-features
cargo test -p arcweft-lang-sema --lib function_effect_timing --all-features
cargo test -p arcweft-lang-sema --lib function_method_fallback --all-features
cargo test -p arcweft-lang-sema --lib function_stack --all-features
cargo clippy -p arcweft-lang-sema --all-targets --all-features -- -D warnings
```

The four focused suites passed with 12, 44, 17, and 50 tests respectively.
Strict clippy passed after call dispatch, curried completion, closure parameter
resolution, and argument collection were separated into their owning contexts.
LSP hover expectations were updated for open inferred source-function and
closure rows. Their focused validation is intentionally left to the parent cut
because concurrent runtime-plan edits prevented compiling the LSP dependency
graph during this slice.
