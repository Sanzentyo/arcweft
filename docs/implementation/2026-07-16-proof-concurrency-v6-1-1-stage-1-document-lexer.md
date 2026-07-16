# Proof-concurrency v6.1.1 Stage 1 document lexer

## Scope

This is the first private Stage 1 slice for
`arcweft-proof-concurrency-v6.1.1-typed-ast-proof-block-hir-runtime-identity-final-contract.zip`
(SHA-256
`1b7de5f2c10a5b29d67c72011e4272df9a76af8907fd21fe162de54809fc69ef`).
It starts from Git `addc49f890967f6ee2820d014ec09976612b4aab` / Jujutsu
change `tpprxroomulqnmrnsrmwuqxptrtytltl` and does not claim the complete
Stage 1 gate or package completion.

## Implemented safe state

- A single private `DocumentLexer` walks the exact source bytes once and emits
  checked `SourceRange` token boundaries for whitespace, physical newlines,
  ordinary/documentation comments, nested-state block-comment fragments,
  identifiers and keywords, lifetimes, numbers, strings, raw strings,
  character literals, entity references, punctuation, and recovery text.
- Numeric tokenization keeps range punctuation separate (`1..2`) while
  retaining decimal/exponent/radix/suffix spellings. Raw-string termination is
  scanned without allocating a synthetic suffix. Character literals recognize
  direct Unicode, byte escapes, and Unicode escapes without consuming ordinary
  lifetime tokens.
- The private document event stream owns one `SourceFile`, one structural
  `ItemList`, depth-zero `LogicalLine` wrappers, current/final top-level item
  family nodes, exact real tokens, and one zero-width EOF token.
- Delimiter depth makes a newline inside a delimited header/body part of the
  same logical line. Unknown top-level input becomes the ordinary current
  `ErrorItem`; there is no removed-spelling recognizer or diagnostic.
- The shadow output remains crate-private and test-only. It allocates no
  production `SyntaxNodeId`, enters no cache, and is not consumed by HIR.

Direct tests prove byte-for-byte reconstruction across UTF-8, CRLF, comments,
multiline raw strings, entity references, lifetime/character ambiguity, numeric
ranges, and representative top-level item classification. The resulting green
tree text equals the original source for every fixture.

## Structural evidence

The canonical post-change audit is stored under
`structure-audits/proof-concurrency-v6-1-1-stage-1-lexer-2026-07-16/`.
It scanned 2,883 files, 1,421 Rust files, 667,495 physical Rust LOC, and 90
package manifests with zero errors and 128 pre-existing repository warnings.

`crates/arcweft-lang-syntax/src/parser/document.rs` is a 23,647-byte,
760-physical-LOC production responsibility module with an embedded test module
starting at physical line 628. Its responsibilities are limited to the shared
document lexer, root event assembly, top-level shadow dispatch, and direct
private tests. It is below the 1,200-LOC production warning threshold and the
file remains within the ordinary responsibility-module review band.

## Validation

The following focused commands pass with `CARGO_INCREMENTAL=0`:

```bash
cargo test -p arcweft-lang-syntax parser::document::tests --lib
cargo clippy -p arcweft-lang-syntax --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
cargo +nightly -Zscript tools/structure-audit.rs --root . --write docs/implementation/structure-audits/proof-concurrency-v6-1-1-stage-1-lexer-2026-07-16
```

Workspace check and clippy are run at the landing cut and recorded in the
commit/push handoff.

## Remaining Stage 1 work

The private cursor still needs grammar-owned item, statement, expression,
pattern, type, recovery, attribute/documentation attachment, delimiter, and
missing-node event production for every existing grammar family. The current
top-level node contains flat real tokens and therefore is not yet suitable for
identity attachment. Stage 2 reconciliation and every public syntax/HIR switch
remain intentionally unstarted until the complete private Stage 1 gate passes.
