# 2026-07-07 Call/Select Unification Refactor

## Source package

- `D:/sanze/Downloads/arcweft-call-select-unification-refactor-2026-07-07.zip`

## Implemented scope

- Removed parser-level `Expr::MethodCall` and `Expr::Field` from the active expression AST.
- Added neutral parser-level `Expr::Select(SelectExpr)` for dotted selection.
- Lowered dotted calls as `Expr::Call { callee: Expr::Select(...), args }`, preserving the distinction between `f(a)(b)` and `f(a, b)`.
- Updated syntax, HIR-facing sema, verifier, tooling, CLI, agent REPL, and runtime-plan code that previously matched parser-level method/field variants.
- Kept `RuntimeExpr::MethodCall` and `RuntimeExpr::Field` as runtime IR semantics. Parser `Select` lowers to runtime field projection or selected-call semantics where appropriate.
- Preserved selected-call handling for trait/inherent methods, data-last fallback evidence, presentation handle lifecycle calls, host request lowering, inline failure constructors, agent predicates, and runtime await-many traversal.

## Validation

- `cargo test -p arcweft-lang-syntax --test parser_p0 --all-features --quiet`
- `cargo test -p arcweft-lang-sema --all-features`
- `cargo test -p arcweft-runtime-plan --all-features`
- `cargo check -p arcweft-agent-repl -p arcweft-verify -p arcweft-tooling -p arcweft-cli --all-features`
- `cargo check --workspace --all-targets --all-features`
- `cargo clippy --workspace --all-targets --all-features`
- `cargo +nightly -Zscript tools/structure-audit.rs --root . --write docs/implementation/structure-audits/call-select-unification-2026-07-08`

## Notes

- `RuntimeExpr::MethodCall` remains intentionally present because runtime backends and AWBC lowering still use it as lowered executable semantics.
- No compatibility parser branch for the removed variants was added; removed syntax/model usage now fails at compile sites until migrated.
- Clippy completed with existing warnings outside the Call/Select refactor surface, including large enum warnings in syntax AST items, float comparisons in scroll region tests, and clipboard/web text-input warnings from earlier slices.
- The structural audit completed and wrote report files under `docs/implementation/structure-audits/call-select-unification-2026-07-08`; it reported the repository's current 1 error and 148 warnings for structural follow-up tracking.
