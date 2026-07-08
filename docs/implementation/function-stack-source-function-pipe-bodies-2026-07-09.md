# Function Stack Source Function Pipe Bodies - 2026-07-09

## Status

Implemented as a 07.7 source-function candidate hardening slice.

Accepted source-local `fn` runtime-function candidates can now contain pure
pipeline expressions when the pipe lowers only through already executable
function-value paths:

- pipe-left substitution with `^`, when the substituted expression remains in
  the accepted source-function subset;
- no-`^` data-last pipes to local function-valued bindings;
- no-`^` data-last pipes to already-lowered pure helpers; and
- no-`^` data-last pipes to already-accepted source-local function
  candidates.

Unknown callable labels, host/adapter calls, task/dialogue/stream functions,
method-value extraction, collection `map`/`filter` method lowering, effectful
calls, suspending calls, `await`, `try`, and threads remain outside this
source-function candidate subset.

## Contract

This slice does not introduce a broad callable-allocation contract. It only
allows `Expr::Pipe` when the runtime-plan strict lowering path can prove the
pipe target is already executable as a local function value, pure helper, or
accepted source-function candidate.

Data-last pipe partials through pure helpers and accepted source-function
candidates register a local arity-only function signature when assigned to a
`let` binding, so later calls through that local value lower as local
`RuntimeExpr::Apply`.

Pipe-left substitution reuses the shared runtime-plan desugaring logic. The
substituted expression must still satisfy the same accepted source-function
body rules, which prevents `^` from opening a backdoor to adapter calls or
suspension-capable expressions.

## Evidence

Compiler regressions cover:

- a source function body that creates a partial function through
  `let add_label = value |> add`, invokes it later, and also uses
  `value |> add(^, 5i64)` as an exact pure-helper call; and
- a source function body that calls an already-accepted source-local candidate
  through a named data-last pipe, `tail |> pair(left = "head")`.

## Validation

```bash
cargo test -p arcweft-compiler --all-features checked_runtime_plan_materializes_source_function_pure_helper_pipe_body -- --nocapture
cargo test -p arcweft-compiler --all-features checked_runtime_plan_materializes_source_function_named_source_pipe_body -- --nocapture
cargo test -p arcweft-compiler --all-features checked_runtime_plan_materializes_source_function_pure_helper_call_body -- --nocapture
cargo test -p arcweft-compiler --all-features when_body_calls_unaccepted_source -- --nocapture
cargo check -p arcweft-runtime-plan -p arcweft-compiler --all-targets --all-features
cargo clippy -p arcweft-runtime-plan -p arcweft-compiler --all-targets --all-features
cargo fmt --all --check
git diff --check
cargo +nightly -Zscript tools/structure-audit.rs --root . --write docs\implementation\structure-audits\function-stack-source-function-pipe-bodies-2026-07-09
```

All commands passed for this slice. Clippy still reports pre-existing warnings
in `arcweft-lang-syntax` and `arcweft-lang-sema`; the structure audit still
reports the existing `crates/arcweft-lang-sema/src/checker/expr.rs` size error
plus 150 warnings.
