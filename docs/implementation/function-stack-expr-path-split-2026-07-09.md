# Function Stack Expr Path Split - 2026-07-09

This note records the structure-only follow-up after the function-type
effect-row slice.

## Change

- Moved literal, path, and short-variant expression type checking from
  `crates/arcweft-lang-sema/src/checker/expr.rs` into
  `crates/arcweft-lang-sema/src/checker/expr/path.rs`.
- Kept the existing `TypeChecker` behavior unchanged; the split only changes
  ownership boundaries inside the expression checker.

## Why

The previous structure audit for
`function-stack-function-type-effect-rows-2026-07-09` reported one error:
`checker/expr.rs` had 2510 physical LOC. This split removes that error before
more function-stack expression work is added.

## Validation

- `cargo check -p arcweft-lang-sema --all-targets --all-features`
- `cargo clippy -p arcweft-lang-sema --all-targets --all-features`
- `cargo +nightly -Zscript tools/structure-audit.rs --root . --write docs\implementation\structure-audits\function-stack-expr-path-split-2026-07-09`

The new structure audit reports 0 errors / 153 warnings.
