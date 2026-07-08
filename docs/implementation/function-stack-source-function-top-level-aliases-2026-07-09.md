# Function Stack Source Function Top-Level Aliases - 2026-07-09

## Status

Implemented as a 07.7 source-function candidate hardening slice.

The accepted source-local `fn` runtime-function subset now tracks simple
`let` aliases to already executable top-level function values inside accepted
source-function bodies. This covers:

- aliases to already-lowered pure helpers; and
- aliases to already-accepted source-local runtime-function candidates.

The alias is registered in the source-function candidate context with the
callee's input arity, so later calls through that local name lower as local
`RuntimeExpr::Apply` instead of causing the containing source function to fall
out of the accepted candidate family.

## Contract

This does not widen the broad callable-allocation contract. The alias target
must already be executable as a runtime function value through the existing
pure-helper table or accepted source-function candidate fixed point.

The local alias receives an arity-only signature unless the expression already
has a source `TypeRef` function signature. That is sufficient for exact calls
and prefix partials through the local alias, while avoiding guessed return-row,
suspension, or persistence semantics for callable families that are still
covered by request 07.7.

Unsupported source functions, task/dialogue/stream functions, methods,
effectful bodies, suspending bodies, adapter thunks, and persisted callable
values remain outside the accepted contract.

## Evidence

Runtime-plan candidate discovery now resolves function-local signatures for
`Expr::Path` in this order:

1. existing local function-valued bindings;
2. already-lowered pure helpers;
3. already-accepted source-local runtime-function candidates.

Compiler regressions cover:

- a source function body that binds `let op = add`, where `add` is a pure
  helper, partially applies it with `let add_label = op(value)`, and then
  calls `add_label(5i64)`; and
- a source function body that binds `let make_pair = pair`, where `pair` is an
  accepted source-local candidate, and then calls `make_pair("head", tail)`.

## Validation

```bash
cargo test -p arcweft-compiler --all-features checked_runtime_plan_materializes_source_function_pure_helper_call_body -- --nocapture
cargo test -p arcweft-compiler --all-features checked_runtime_plan_materializes_source_function_exact_source_alias_body -- --nocapture
cargo test -p arcweft-compiler --all-features checked_runtime_plan_materializes_source_function_exact_source_call_body -- --nocapture
cargo test -p arcweft-compiler --all-features when_body_calls_unaccepted_source -- --nocapture
cargo check -p arcweft-runtime-plan -p arcweft-compiler --all-targets --all-features
cargo clippy -p arcweft-runtime-plan -p arcweft-compiler --all-targets --all-features
cargo fmt --all --check
git diff --check
cargo +nightly -Zscript tools/structure-audit.rs --root . --write docs\implementation\structure-audits\function-stack-source-function-top-level-aliases-2026-07-09
```

All commands passed for this slice. Clippy still reports pre-existing warnings
in `arcweft-lang-syntax` and `arcweft-lang-sema`; the structure audit still
reports the existing `crates/arcweft-lang-sema/src/checker/expr.rs` size error
plus 150 warnings.
