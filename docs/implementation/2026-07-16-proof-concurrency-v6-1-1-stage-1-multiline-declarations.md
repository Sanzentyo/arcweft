# Proof-concurrency v6.1.1 Stage 1 multiline declarations

## Scope

This cut advances the private one-pass grammar on Jujutsu change
`vlytkrovkqykxrnormysxszwktzunkrl`, based on Git `c7e804fa2086`.

Depth-zero physical lines beginning a current `predicate` or `proof`
declaration are now grouped through their expression or block body before the
declaration grammar emits events. Header continuations beginning with `where`,
`requires`, `ensures`, `->`, `=`, or `{` remain under the same item, as do blank
continuation lines and a split generic header. The resulting declaration owns
lossless `LogicalLine` wrappers without reparsing a retained string.

Recovery stops before a following clean top-level declaration. In particular,
a predicate with a missing body retains `MissingBody` and its current
diagnostic while the following proof is emitted as an independent `ProofItem`.
Canonical multiline clauses and a multiline predicate block round trip every
source byte and retain typed clause, statement, expression, and body children.

This is a private Stage 1 slice. It does not claim the public syntax database
switch, attached typed AST, HIR lowering, or the later runtime identity stages.
Those remain part of the active proof-concurrency goal.

No removed-syntax recognizer, spelling-specific diagnostic, source gate,
compatibility shim, CSS route, or Takumi route was introduced.

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

The focused predicate/proof suite has 14 passing tests and the syntax library
suite has 159 passing tests. Workspace check and Clippy also complete without
warnings or errors.

## Structure

The canonical report is stored under
`structure-audits/proof-concurrency-v6-1-1-stage-1-multiline-declarations-2026-07-16/`.
It scanned 2,939 files, 1,455 Rust files, 680,034 physical Rust LOC, and 90
manifests with zero errors and 129 pre-existing repository-wide warnings. No
warning names an in-scope file.

- `parser/document.rs`: 16,295 bytes / 530 physical LOC / 481 code LOC,
  production, hand-maintained, no embedded tests;
- `parser/predicate_proof_tests.rs`: 15,803 bytes / 459 physical LOC / 434 code
  LOC, test, hand-maintained.

Both remain below their applicable structural warning thresholds and retain
one responsibility: root lossless document event emission and focused private
predicate/proof grammar evidence, respectively.
