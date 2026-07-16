# Proof-concurrency v6.1.1 Stage 1 bracket and call expressions

## Scope

This private grammar cut is based on Git `59b29ea8c2a6`. It completes the
distinct existing bracket-sequence roots and gives call arguments their own
authored shapes without adding a fragment parser.

An integer-only bracket sequence with one common explicit suffix, or no suffix
on every item, now emits one compact `NumericBracketSequenceExpression` and no
per-item literal expressions. Mixed suffixes, floats, and general expressions
remain `BracketSequenceExpression` children. A top-level semicolon selects
`ArrayRepeatExpression` with independently typed value and length descendants.
The parser classifies numeric tokens from the single lexer pass; it neither
copies nor reparses the complete sequence.

Every call argument remains an identity-bearing `CallArgument`. Positional,
named `name = value`, and postfix spread `value...` spellings now retain a
typed operand; named arguments additionally own a `NameReference`. Nested
bracket delimiters keep their commas inside the argument rather than splitting
the surrounding call.

The integer classifier uses a private enum to distinguish no suffix from an
explicit canonical suffix. Invalid or mixed representations fall back to the
ordinary expression sequence rather than relying on a nested optional or a
stringly public boundary.

This cut remains crate-private shadow output and allocates no production
syntax identity. It introduces no removed-form recognizer, source gate,
compatibility shim, CSS route, or Takumi route.

## Direct evidence

Focused tests prove that:

- `[1, 2, 3]` emits a compact numeric sequence;
- `[value, count]` and `[value; count]` emit the ordinary and repeat families;
- one call owns five distinct positional, named, and spread arguments;
- `[1u8, 2u16]` remains an ordinary sequence with two literal descendants;
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

The focused predicate/proof suite has 25 passing tests and the syntax library
suite has 170 passing tests. Workspace check and Clippy complete without
errors or warnings. The first combined validation wrapper reached its
124-second command timeout after the syntax suite; workspace check and Clippy
were rerun as direct commands and both completed successfully.

## Structure

The canonical report is stored under
`structure-audits/proof-concurrency-v6-1-1-stage-1-bracket-call-expressions-2026-07-16/`.
It scanned 2,953 files, 1,457 Rust files, 681,157 physical Rust LOC, and 90
manifests with zero errors and 129 pre-existing repository-wide warnings. No
warning names an in-scope file.

- `parser/expression.rs`: 13,374 bytes / 418 physical LOC, production,
  hand-maintained, no embedded tests;
- `parser/expression/composite.rs`: 10,202 bytes / 309 physical LOC,
  production, hand-maintained, no embedded tests;
- `parser/predicate_proof_tests.rs`: 28,465 bytes / 858 physical LOC, test,
  hand-maintained.

All in-scope files remain below structural warning thresholds. Stage 1 still
requires record, named/computation/memo block, thread/dialogue, and remaining
item-family structured descendants and direct grammar-family coverage.
