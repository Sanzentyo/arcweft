# Warning Structured Diagnostics Audit

Source package: `arcweft-warning-structured-diagnostics-2026-06-21.zip`

## Accepted Scope

The package audit found that the warning data model had already moved away from
generic warning fallback routing:

- `TypeCheckWarningKind` has no `Message` variant.
- `TypeCheckWarning` has no `new` constructor.
- public ABI anonymous sums are emitted through
  `TypeCheckWarning::public_abi_anonymous_sum`.
- effect warnings are emitted through `TypeCheckWarning::effect`.
- LSP warning codes are derived from structured warning kind data.

The remaining implementation work at that cut was therefore a hardening slice
rather than a new diagnostic model:

- remove the stale LSP warning index plumbing left over from generic fallback
  code generation;
- lock public ABI anonymous sum warning payloads in sema tests;
- lock the public ABI anonymous sum LSP code
  `sema.public_abi.anonymous_sum` in LSP tests;
- keep effect warning routing through `EffectDiagnosticCode::as_str()`.

## Superseded Effect Warning Slice

The later first-order effect lowering slice changes source `effects` clauses
from declared required rows into explicit upper bounds. Under that model an
unused upper-bound member is not a semantic warning; it is simply not lowered
into the Agent artifact's closed effect row. The former `AWF-EFX-008`
overdeclared-effect warning and its LSP regression were removed with that
slice.

## Non-Goals

- No compatibility warning variant, legacy alias, wrapper, or fallback code
  is introduced.
- `TypeCheckErrorKind::Message` remains outside this scope. The package and
  implemented change target non-fatal warning diagnostics only.
- No parser, runtime, renderer, profile, or CLI behavior changes are included.

## Validation Results

Completed on 2026-06-22:

```bash
cargo test -p arcweft-lang-sema declarations --lib -- --format=terse # passed
cargo test -p arcweft-lsp diagnostics_surface_public_abi_anonymous_sum_warning --lib -- --nocapture # passed
cargo check -p arcweft-lang-sema -p arcweft-lsp --all-targets # passed
cargo clippy -p arcweft-lang-sema -p arcweft-lsp --all-targets --all-features -- -D warnings # passed
cargo check --workspace --all-targets # passed
cargo clippy --workspace --all-targets --all-features -- -D warnings # passed
cargo fmt --all --check # passed
cargo +nightly -Zscript tools/arcweft-structure-audit.rs --root . # passed: 0 error(s), 87 warning(s)
rg -n "TypeCheckWarningKind::Message|TypeCheckWarning::new|sema\.typecheck\.warning|typecheck_warning_code\([^)]*," crates/arcweft-lang-sema/src crates/arcweft-lsp/src # no matches
```
