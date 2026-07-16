# Proof-concurrency v6.1.1 Stage 1 outer prefixes

## Scope

This private grammar cut is Jujutsu change
`zuzyvtsznsztputtnuwlklwtymlsvorp`, based on Git `af36c9900409`.
It adds the final package's documentation and outer-attribute ownership to the
one-pass predicate/proof event path.

- consecutive documentation lines form one identity-bearing `DocBlock`;
- each `#[...]` form is an independently identity-bearing `OuterAttribute` in
  authored order under an ID-less `AttributeList`;
- directly adjacent prefixes and the following predicate/proof declaration are
  grouped before event emission, so they have one semantic item owner;
- an inner `#![...]` form is not treated as an outer prefix; and
- multiline attributes keep bracket-nested physical newlines inside one
  logical line, while consecutive documentation lines retain one logical line
  per depth-zero newline.

The event tree is still crate-private shadow output. Prefix tokens are consumed
by the same full-source cursor as the declaration and are never retained as a
string for a second parse. No historical spelling recognizer, source gate,
compatibility shim, CSS route, or Takumi route was added.

## Direct evidence

The focused test combines two documentation lines, a single-line attribute, a
multiline attribute, visibility, generics, a fixed parameter, a multiline
`where` clause, and an expression body. It proves one `ProofItem`, one
`DocBlock`, two `OuterAttribute` nodes, the expected typed declaration
descendants, seven exact logical lines, no diagnostics, and byte-for-byte green
tree losslessness.

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

The focused suite has 15 passing tests and the syntax library suite has 160
passing tests. The first focused Clippy run identified one identical match arm;
that arm was removed and the complete command sequence above then passed.

## Structure

The canonical report is stored under
`structure-audits/proof-concurrency-v6-1-1-stage-1-outer-prefixes-2026-07-16/`.
It scanned 2,941 files, 1,455 Rust files, 680,242 physical Rust LOC, and 90
manifests with zero errors and 129 pre-existing repository-wide warnings. No
warning names an in-scope file.

- `parser/document.rs`: 18,356 bytes / 593 physical LOC / 540 code LOC,
  production, hand-maintained, no embedded tests;
- `parser/predicate_proof.rs`: 18,108 bytes / 522 physical LOC / 494 code LOC,
  production, hand-maintained, no embedded tests;
- `parser/predicate_proof_tests.rs`: 17,728 bytes / 517 physical LOC / 489 code
  LOC, test, hand-maintained.

All three files remain inside their applicable structural targets. Full
existing-item shadow grammar coverage, exact fatal grammar limits, public
attachment, HIR, sema, and runtime identity stages remain open under the
mandatory package sequence.
