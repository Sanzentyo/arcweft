# Function Stack Unsupported Bare Source Function Values - 2026-07-09

## Scope

This hardening slice closes a checked runtime-plan gap in the 07.7
non-helper callable boundary.

Sema already type-checks a top-level source `fn` path in value position as a
function value. Before this slice, checked runtime-plan lowering could still
fall through to `RuntimeExpr::Local("function_name")` when that function was
not a pure helper and was not in the accepted source-function candidate set.
That was too weak: it made an unsupported callable family look like an ordinary
local variable and deferred the real error.

The new behavior records function-value reference evidence during type
checking, converts that evidence into runtime-plan evidence, and rejects a bare
source function value during strict runtime-plan path lowering when no
executable helper or accepted source-function candidate exists.

## Implemented Behavior

- `TypedLoweringEvidenceKind::FunctionValueReference` records top-level
  function path references keyed to the source expression judgment.
- `RuntimeTypedLoweringEvidenceKind::FunctionValueReference` carries the
  evidence into checked runtime-plan lowering.
- Strict runtime expression lowering now handles `Expr::Path` through an
  explicit path-lowering function:
  - enum constructor paths still lower to `RuntimeExpr::Variant`;
  - local function values, pure helpers, and accepted source-function
    candidates still lower through the existing helper lookup;
  - unresolved ordinary locals still lower as `RuntimeExpr::Local`;
  - a path proven by sema to be a top-level source function value, but missing
    any executable runtime candidate, fails with unsupported callable family
    `source_function_value_without_runtime_candidate`.

This keeps the current accepted source-local `fn` family unchanged. It does
not broaden support for host/adapter call-bearing, effectful, or suspending
source function values.

## Regression

The new compiler regression covers a source function whose body calls an
ordinary method:

```arcw
fn trim_right(left: String, right: String) -> String {
    return right.trim()
}

flow @flow.main main {
    let trim = trim_right
    let value: String = trim("head", " tail ")
    return "done"
}
```

Type checking succeeds because `trim_right` has a valid function type, but
checked runtime-plan lowering rejects it because `trim_right` is neither a
pure helper nor an accepted source-function candidate.

## Validation

```bash
cargo fmt --all
cargo test -p arcweft-compiler --all-features checked_runtime_plan_rejects_bare_source_function_value_when_body_calls -- --nocapture
cargo test -p arcweft-compiler --all-features checked_runtime_plan_materializes_named_missing_source_function_partial_call -- --nocapture
cargo test -p arcweft-compiler --all-features runtime_plan_lowers_local_function_data_last_pipe_to_apply -- --nocapture
cargo test -p arcweft-lang-sema --all-features top_level_function_path_typechecks_as_function_value -- --nocapture
cargo check -p arcweft-lang-sema -p arcweft-runtime-plan -p arcweft-compiler --all-targets --all-features
cargo clippy -p arcweft-lang-sema -p arcweft-runtime-plan -p arcweft-compiler --all-targets --all-features
cargo +nightly -Zscript tools/structure-audit.rs --root . --write docs\implementation\structure-audits\function-stack-unsupported-bare-source-function-values-2026-07-09
```

All commands passed. Clippy still reports pre-existing warnings from
`arcweft-lang-syntax` large enum variants and existing `too_many_lines`
warnings in sema tests/analysis code; no warning is attributed to this slice.
The structure audit scanned 2486 files / 1179 Rust files / 584012 Rust
physical LOC and reported 0 errors / 151 warnings. Reports were written under
`docs/implementation/structure-audits/function-stack-unsupported-bare-source-function-values-2026-07-09/`.

## Remaining Boundaries

This slice only improves rejection for unsupported bare source function value
references. The 07.7 request remains open for broader callable allocation:
effectful and suspending source function values, host/adapter call-bearing
function values, task/dialogue/stream functions, trait/impl method values,
adapter thunks, AWBC resumability, and persistence.
