# Proof-concurrency v6.1.1 Stage 1 statement context correction

## Correction

After landing the shared statement event inventory at Git `23bf20bac14d`, the
final package's `PREDICATE_PROOF_GRAMMAR.md` and `PROOF_BLOCK.md` were checked
again against the private wiring. The common parser was intentionally broad,
but predicate/proof blocks were also exposing those broad statement kinds.
That was not the final contract.

The corrected boundary is now explicit:

- predicate blocks attach pure-let-shaped `LetStatement`, recovery-only typed
  assertions, and `ErrorStatement`;
- proof blocks additionally attach call-shaped `ProofCallStatement`;
- terminated control, transfer, derived-let, assignment, and ordinary
  expression-statement families recover as `ErrorStatement` in these two
  declaration contexts; and
- the complete shared statement inventory is tested through a generic private
  block built from the same one-pass lexer and event cursor.

An unterminated final expression remains the block tail. A final call without a
terminator is therefore a tail, while the same call with a terminator is a
proof-call statement only in proof context. No spelling-specific branch,
removed-syntax diagnostic, source gate, or compatibility layer was introduced.

## Validation

The following commands pass with `CARGO_INCREMENTAL=0`:

```bash
cargo test -p arcweft-lang-syntax --lib
cargo test -p arcweft-lang-syntax parser::predicate_proof_tests --lib
cargo clippy -p arcweft-lang-syntax --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
git diff --check
```

The library suite contains 157 passing tests. The combined workspace landing
validation completed successfully in 100.7 seconds.

## Structure

The canonical correction report is stored under
`structure-audits/proof-concurrency-v6-1-1-stage-1-statement-context-correction-2026-07-16/`.
It scanned 2,937 files, 1,455 Rust files, 679,794 physical Rust LOC, and 90
manifests with zero errors and 129 pre-existing repository-wide warnings. No
warning names an in-scope file.

- `parser/statement.rs`: 24,314 bytes / 720 physical LOC / 687 code LOC,
  production, hand-maintained, no embedded tests;
- `parser/predicate_proof_tests.rs`: 13,529 bytes / 384 physical LOC / 363 code
  LOC, test, hand-maintained.

The statement module remains within the package's 450-800 LOC target.
