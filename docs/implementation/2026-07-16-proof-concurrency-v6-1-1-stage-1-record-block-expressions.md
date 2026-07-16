# Proof-concurrency v6.1.1 Stage 1 record and block expressions

## Scope

This private grammar cut is based on Git `328e362f8118`. It replaces the
remaining flat or misclassified record, scoped-block, computation, memo, and
braced-thread roots with structured descendants over the shared cursor.

Nominal records whose final path segment is a nominal name emit
`RecordExpression`; field-shaped bare braces emit
`RecordLiteralExpression`. Each authored field is an identity-bearing
`RecordField` with its name and optional initializer. Statement-shaped bare
braces remain `BlockExpression`, so a `let` plus tail cannot be mistaken for a
record. This is a grammar classification over already lexed tokens, not a
second expression parse.

Canonical `result`/`task`/`seq`/`stream` roots emit
`ComputationBlockExpression`; `memo(...)` emits typed option
`CallArgument` children and a `MemoBlockExpression`; `scope name { ... }`
emits a `NamedBlockExpression`; and braced `thread` roots emit
`ThreadExpression`. All four reuse one `Block` and statement grammar directly.
They do not create a second identity-bearing `BlockExpression` merely to reach
the shared block implementation.

This cut remains crate-private shadow output and allocates no production
syntax identity. It introduces no removed-form recognizer, source gate,
compatibility shim, CSS route, or Takumi route.

## Direct evidence

Focused tests prove that one lossless tuple can independently contain:

- a two-field nominal record and a two-field anonymous record;
- a computation block;
- a memo block with named options;
- a named scope block; and
- a detached, named thread block.

The fixture owns exactly four record fields and four shared `Block` nodes. A
separate statement-shaped brace fixture emits `BlockExpression` and a typed
`LetStatement`, never `RecordLiteralExpression`. Covered fixtures contain no
`ErrorExpression` and round trip byte-for-byte through the green tree.

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

The focused predicate/proof suite has 27 passing tests and the syntax library
suite has 172 passing tests. Workspace check and Clippy complete without
errors or warnings.

## Structure

The canonical report is stored under
`structure-audits/proof-concurrency-v6-1-1-stage-1-record-block-expressions-2026-07-16/`.
It scanned 2,955 files, 1,457 Rust files, 681,517 physical Rust LOC, and 90
manifests with zero errors and 129 pre-existing repository-wide warnings. No
warning names an in-scope file.

- `parser/expression.rs`: 14,239 bytes / 441 physical LOC, production,
  hand-maintained, no embedded tests;
- `parser/expression/composite.rs`: 18,741 bytes / 578 physical LOC,
  production, hand-maintained, no embedded tests;
- `parser/expression/control.rs`: 7,528 bytes / 233 physical LOC, production,
  hand-maintained, no embedded tests;
- `parser/predicate_proof_tests.rs`: 30,747 bytes / 920 physical LOC, test,
  hand-maintained.

All in-scope files remain below structural warning thresholds. Stage 1 still
requires dialogue-context expression ownership and complete direct coverage
for the remaining existing item families before its gate can close.
