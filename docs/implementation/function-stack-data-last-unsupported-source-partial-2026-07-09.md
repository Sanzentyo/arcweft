# Function Stack Data-Last Unsupported Source Partial - 2026-07-09

## Scope

This hardening slice closes the data-last pipe version of the unsupported
non-helper source-function partial boundary.

Sema checks `lhs |> callee` by desugaring it to a data-last call. When the
desugared call is a partial signature call, the authored pipe expression now
also records `SignaturePartialCall` lowering evidence. Runtime-plan strict
pipe lowering consumes that evidence before falling back to a direct named
runtime call.

## Implemented Behavior

The following source shape now fails during checked runtime-plan lowering when
`trim_right` is neither a pure helper nor an accepted source-function
candidate:

```arcw
fn trim_right(left: String, right: String) -> String {
    return right.trim()
}

flow @flow.main main {
    let trim_tail: String -> String = "head" |> trim_right
    let value: String = trim_tail(" tail ")
    return "done"
}
```

The diagnostic uses the existing unsupported callable family
`signature_partial_without_helper`, matching direct partial-call rejection.

Accepted data-last paths are unchanged:

- local function-valued aliases still lower to runtime `Apply`;
- pure helpers still lower through helper function values;
- accepted source-function candidates still lower through materialized runtime
  function values.

## Validation

```bash
cargo fmt --all
cargo test -p arcweft-compiler --all-features checked_runtime_plan_rejects_data_last_source_function_partial_when_body_calls -- --nocapture
cargo test -p arcweft-compiler --all-features checked_runtime_plan_rejects_source_function_partial_when_body_calls -- --nocapture
cargo test -p arcweft-compiler --all-features checked_runtime_plan_rejects_bare_source_function_value_when_body_calls -- --nocapture
cargo test -p arcweft-compiler --all-features runtime_plan_lowers_local_function_data_last_pipe_to_apply -- --nocapture
cargo test -p arcweft-lang-sema --all-features data_last_pipe_through_local_function_value_records_call_evidence -- --nocapture
cargo check -p arcweft-lang-sema -p arcweft-runtime-plan -p arcweft-compiler --all-targets --all-features
cargo clippy -p arcweft-lang-sema -p arcweft-runtime-plan -p arcweft-compiler --all-targets --all-features
cargo +nightly -Zscript tools/structure-audit.rs --root . --write docs\implementation\structure-audits\function-stack-data-last-unsupported-source-partial-2026-07-09
```

All commands passed. Clippy still reports pre-existing warnings from
`arcweft-lang-syntax` large enum variants and existing `too_many_lines`
warnings in sema tests/analysis code; no warning is attributed to this slice.
The structure audit scanned 2488 files / 1179 Rust files / 584131 Rust
physical LOC and reported 0 errors / 151 warnings. Reports were written under
`docs/implementation/structure-audits/function-stack-data-last-unsupported-source-partial-2026-07-09/`.

## Remaining Boundaries

This slice does not implement a general callable representation for
host/adapter call-bearing, effectful, suspending, task/dialogue/stream,
trait/impl, or persisted callable values. It only prevents data-last pipe
partials from bypassing the existing unsupported non-helper diagnostic.
