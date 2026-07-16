# Proof-concurrency v6.1.1 Stage 1 control expressions

## Scope

This private grammar cut is based on Git `fb5b1299cb8a`. It replaces the
remaining flat event shells for block, `if`, `if let`, and `match` expressions
with one-pass structured descendants over the shared full-source cursor.

`IfExpression` and `IfLetExpression` now own explicit condition or
pattern/scrutinee/guard descendants and typed then/else branches. Nested
`else if` expressions retain the same structure. `MatchExpression` now owns a
scrutinee and a `MatchArmList`; each `MatchArm` owns its pattern, optional
guard, and expression or block body. Block expressions reuse the common block
and statement grammar rather than reparsing a text slice.

The statement/block boundary was corrected at the same ownership point. A
final, unterminated `if`, `loop`, `match`, or `thread` form is a block tail;
the same form remains a statement when it has a semicolon or a later sibling.
This prevents a tail `match` from being consumed as a statement before the
expression grammar can emit its arms.

Expression-boundary scanning deliberately treats only parentheses, brackets,
and braces as delimiter nesting. Comparison operators such as `<` and `>`
therefore cannot hide the opening branch brace or an arm boundary. The cut
does not introduce string reparsing, a removed-form recognizer, a source gate,
a compatibility shim, a CSS route, or a Takumi route.

## Direct evidence

Focused tests prove that:

- `if let` emits a variant pattern, scrutinee, guard, and two block branches;
- a nested `match` emits two independently owned arms and their bodies;
- comparison `a < b` remains a binary condition and does not consume the
  branch delimiter;
- the resulting tree contains no `ErrorExpression` for the covered forms; and
- every fixture round trips byte-for-byte through the green tree.

## Validation

The following commands pass with `CARGO_INCREMENTAL=0`:

```bash
cargo test -p arcweft-lang-syntax parser::predicate_proof_tests --lib
cargo test -p arcweft-lang-syntax --lib
cargo clippy -p arcweft-lang-syntax --all-targets --all-features -- -D warnings
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
git diff --check
```

The focused predicate/proof suite has 21 passing tests and the syntax library
suite has 166 passing tests. Workspace check and Clippy complete without
errors or warnings.

## Structure

The canonical report is stored under
`structure-audits/proof-concurrency-v6-1-1-stage-1-control-expressions-2026-07-16/`.
It scanned 2,946 files, 1,456 Rust files, 680,809 physical Rust LOC, and 90
manifests with zero errors and 129 pre-existing repository-wide warnings. No
warning names an in-scope file.

- `parser/expression.rs`: 15,415 bytes / 490 physical LOC, production,
  hand-maintained, no embedded tests;
- `parser/expression/control.rs`: 7,294 bytes / 227 physical LOC, production,
  hand-maintained, no embedded tests;
- `parser/statement.rs`: 24,525 bytes / 723 physical LOC, production,
  hand-maintained, no embedded tests;
- `parser/predicate_proof_tests.rs`: 24,644 bytes / 747 physical LOC, test,
  hand-maintained.

All in-scope files remain below structural warning thresholds. Stage 1 still
requires structured closure and composite expression descendants plus the
remaining documented grammar-family coverage before typed attachment and the
later HIR/runtime stages can begin.
