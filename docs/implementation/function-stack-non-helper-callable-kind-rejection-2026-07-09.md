# Function Stack Non-Helper Callable Kind Rejection - 2026-07-09

## Scope

This note hardens the 07.7 non-helper callable allocation boundary for
top-level callable kinds other than ordinary accepted source-local `fn`
candidates.

The active goal still does not accept first-class runtime values for
`task fn`, `dialogue fn`, or `stream fn`. Those callable kinds need stable
contracts for identity, effects, suspension, AWBC resume behavior, and
save/load before they can be materialized as runtime function values.

## Contract

- Type checking may still expose task/dialogue/stream function signatures as
  function-typed values so authoring diagnostics and staged call types remain
  coherent.
- Checked runtime-plan lowering must not lower bare task/dialogue/stream
  function values as ordinary locals.
- When a task/dialogue/stream function has no executable helper or accepted
  source-function candidate, checked runtime-plan lowering rejects the value
  reference with unsupported callable family
  `source_function_value_without_runtime_candidate`.
- This is the same rejection family used for unsupported ordinary source `fn`
  values outside the accepted non-helper subset.

## Implementation

`crates/arcweft-compiler/src/tests.rs` now covers the rejected callable kinds:

- `checked_runtime_plan_rejects_bare_task_function_value`
- `checked_runtime_plan_rejects_bare_dialogue_function_value`
- `checked_runtime_plan_rejects_bare_stream_function_value`

No accepted callable allocation behavior was widened in this cut.

## Validation

```bash
cargo test -p arcweft-compiler --all-features checked_runtime_plan_rejects_bare_task_function_value -- --nocapture
cargo test -p arcweft-compiler --all-features checked_runtime_plan_rejects_bare_dialogue_function_value -- --nocapture
cargo test -p arcweft-compiler --all-features checked_runtime_plan_rejects_bare_stream_function_value -- --nocapture
cargo test -p arcweft-compiler --all-features checked_runtime_plan_rejects_bare -- --nocapture
cargo clippy -p arcweft-compiler --all-targets --all-features
cargo fmt --all --check
git diff --check
cargo +nightly -Zscript tools/structure-audit.rs --root . --write docs\implementation\structure-audits\function-stack-non-helper-callable-kind-rejection-2026-07-09
```

All listed checks passed on 2026-07-09. The clippy run reported existing
warnings in `arcweft-lang-syntax` and `arcweft-lang-sema`, with no new compiler
test failure. The structural audit scanned 2494 files, 1179 Rust files, and
584483 Rust physical LOC, and reported 0 errors and 151 existing warnings. The
generated evidence is under
`docs/implementation/structure-audits/function-stack-non-helper-callable-kind-rejection-2026-07-09/`.

## Remaining Boundaries

This does not implement task/dialogue/stream function values. Accepting them
remains part of the broader 07.7 callable allocation contract and must be
coordinated with 07.5 resumable dynamic apply and save/load policy.
