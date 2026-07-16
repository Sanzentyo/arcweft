# Proof-concurrency v6.1.1 Stage 1 closure expressions

## Scope

This private grammar cut is based on Git `a347bffe80c2`. It replaces the flat
closure shell with typed, lossless descendants over the shared document cursor
and corrects parenthesized expression ownership.

Each `ClosureExpression` now owns an ID-less `ParameterList` whose
identity-bearing `ClosureParameter` children own full pattern and optional type
nodes. Both `|...|` and zero-parameter `||` spellings use that one authority.
An authored `-> Type` is retained as a typed `ReturnType`; its required braced
body reuses the common structured block grammar. An unannotated closure body is
parsed directly by the shared expression grammar. No parameter, type, or body
text is sliced and parsed again.

A parenthesized single expression now uses the ID-less `DelimitedGroup`
wrapper, while `()` and comma-bearing forms remain identity-bearing
`TupleExpression` nodes. Postfix calls can wrap the completed group without
changing its nested expression identity or token order.

This cut remains crate-private shadow output. It allocates no production
`SyntaxNodeId` and introduces no removed-form recognizer, source gate,
compatibility shim, CSS route, or Takumi route.

## Direct evidence

Focused tests prove that:

- a typed closure owns its binding pattern, parameter type, return type,
  structured block, local statement, binary tail, and outer call;
- `(closure)(value)` uses one `DelimitedGroup` and no false tuple node;
- `|| true` is a zero-parameter closure rather than a binary-or expression;
- covered fixtures contain no `ErrorExpression`; and
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

The focused predicate/proof suite has 23 passing tests and the syntax library
suite has 168 passing tests. Workspace check and Clippy complete without
errors or warnings.

## Structure

The canonical report is stored under
`structure-audits/proof-concurrency-v6-1-1-stage-1-closure-expressions-2026-07-16/`.
It scanned 2,949 files, 1,457 Rust files, 680,975 physical Rust LOC, and 90
manifests with zero errors and 129 pre-existing repository-wide warnings. No
warning names an in-scope file.

- `parser/expression.rs`: 14,025 bytes / 442 physical LOC, production,
  hand-maintained, no embedded tests;
- `parser/expression/composite.rs`: 5,318 bytes / 160 physical LOC,
  production, hand-maintained, no embedded tests;
- `parser/predicate_proof_tests.rs`: 26,542 bytes / 801 physical LOC, test,
  hand-maintained.

All in-scope files remain below structural warning thresholds. Stage 1 still
requires the remaining bracket, record, named/computation/memo, dialogue, and
item-family structured descendants and direct grammar-family coverage.
