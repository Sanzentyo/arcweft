# Proof concurrency v6.1.1 Stage 1 — native Style shadow grammar

## Outcome

The private Stage 1 document grammar now gives every accepted top-level
`style` declaration a typed, lossless descendant tree. This is strictly a
shadow-grammar cut: it allocates no public syntax identity and does not change
the public `ParsedSource`, public Style AST, HIR, semantic analysis, runtime,
or tooling paths.

The private `StyleItem` subtree owns:

- the outer documentation, attributes, visibility, declaration name, and
  braced body;
- sheet-level token declarations, including a shared type descendant and a
  shared expression initializer;
- selector rules, selector sequences, and property declarations;
- `when environment(...)` blocks, their condition clauses, and nested style
  members; and
- missing member assignments and missing close delimiters as typed recovery
  nodes with ordinary current-grammar diagnostics.

The implementation reuses the shared document lexer, expression grammar, and
type-reference grammar. It does not special-case selector names, property
names, or individual environment fields and values. The old public native
Style parser remains untouched until the package's atomic public syntax switch.

## Completion boundary

This completes the sufficiently designed Style row of Proof concurrency
v6.1.1 Stage 1. It does not complete Stage 1 or begin Stage 2 reconciliation.

Remaining Stage 1 blockers are the separately designed retained declaration
families whose final shadow-grammar contracts are not yet attached:

- external-module declarations, pending the single-manifest/build-profile
  design;
- dialogue-default profiles, pending their finalized CharacterDialogue and
  build-profile ownership;
- live-source declarations, pending the corrected ordinary-function-to-Stream
  surface and typed runtime-wire contracts; and
- public entity/resource families, pending the typed `res` migration contract.

No compatibility parser branch, removed-syntax recognizer, source gate, or
public API shim was added by this cut.

## Acceptance evidence

`parser/style_grammar_tests.rs` has three direct private-grammar tests:

1. a complete documented/attributed/visible Style sheet verifies the token,
   selector, property, environment, shared `Type`/`Expr`, authored-order,
   identity-path, and lossless-source families;
2. a malformed property preserves the following valid rule and following proof
   declaration, and owns the exact zero-width recovery range; and
3. a missing Style close does not consume the following declaration.

The existing top-level item inventory test also verifies that `StyleItem` is
dispatched as a structured declaration with typed brace/body descendants.

## Structural measurement

Measured from Jujutsu change `oyxzzqtv` based on main `2e7c90ec4fad`:

| Path | Kind | Bytes | Physical LOC | Responsibility |
|---|---:|---:|---:|---|
| `crates/arcweft-lang-syntax/src/parser/style_grammar.rs` | production | 22,105 | 667 | private native Style shadow grammar and recovery |
| `crates/arcweft-lang-syntax/src/parser/style_grammar_tests.rs` | unit test | 6,376 | 194 | Style grammar acceptance/recovery evidence |
| `crates/arcweft-lang-syntax/src/parser/document.rs` | production | 24,286 | 776 | shared document grouping and private grammar dispatch |
| `crates/arcweft-lang-syntax/src/grammar/kinds.rs` | production | 8,601 | 365 | private grammar kind/identity vocabulary |
| `crates/arcweft-lang-syntax/src/parser.rs` | facade/module wiring | 23,740 | 703 | private module registration |

All changed Rust files are below the 1,200-line production and 2,500-line
integration-test structural warning thresholds. The canonical audit reports no
error-level violations.

## Verification

Validation was run after the PrefixDepth transaction fix at main
`2e7c90ec4fad`:

- `CARGO_INCREMENTAL=0 cargo test -p arcweft-lang-syntax --all-targets`:
  passed; the library unit-test binary reported 300 passed, and every
  integration and trybuild target passed;
- `CARGO_INCREMENTAL=0 cargo clippy -p arcweft-lang-syntax --all-targets
  --all-features -- -D warnings`: passed;
- `cargo fmt --all -- --check`: passed;
- `git diff --check`: passed; and
- `cargo +nightly -Zscript tools/structure-audit.rs --root .`: scanned 3,305
  files, 1,696 Rust files, 782,280 Rust physical lines, and 92 package
  manifests; reported zero errors and 128 pre-existing warnings.

Tier 2 is not required: this is a private syntax-crate shadow-grammar cut. It
does not alter a public contract or a runtime, rendering, Agent, MCP, or
capture path.
