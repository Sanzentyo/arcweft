# Proof-concurrency v6.1.1 Stage 1 module/use grammar

## Scope

This cut extends the private, lossless shadow grammar with the stable source
header declarations `mod` and `use`. It is based on main `f420ee8fbf24` and the
Jujutsu change `omzvmlsx`. It does not switch the public parser, syntax identity,
HIR, or project resolver.

Both declarations consume the document parser's existing lexer tokens and emit
into its single cursor/event stream. The module path is a typed `Path`; imports
retain visibility, module paths, grouped names, aliases, globs, delimiters, and
all trivia without reparsing source text. Group members use `NameReference` and
aliases use `NameDefinition`; layout-only grouping remains a non-identity
`DelimitedGroup`.

## Recovery

- a missing module path emits a typed `Path`/`MissingName` and leaves the next
  declaration as a sibling;
- a missing import tree or alias emits the corresponding typed missing node;
- a missing grouped-use close delimiter emits the zero-width close wrapper and
  synchronizes at the following declaration boundary;
- invalid trailing tokens remain in an `ErrorNode`, preserving exact source
  bytes and the valid prefix.

No public compatibility surface, second lexer, source gate, spelling-specific
removed-syntax recognizer, CSS route, or Takumi route is introduced.

## Direct evidence

`parser::module_use_grammar_tests` covers a source module declaration, public
grouped imports, aliases, glob imports, exact typed descendant counts, and a
byte-for-byte green-tree round trip. Negative cases cover a missing module path,
missing alias, and an unterminated group followed by a valid proof declaration.

The focused suite passes 4/4. The complete `arcweft-lang-syntax` suite passes
184 unit tests plus all integration, UI, and documentation tests. The public
parser's existing malformed/grouped import tests also continue to pass.

## Validation

The following commands pass with `CARGO_INCREMENTAL=0`:

```bash
cargo fmt --all
cargo test -p arcweft-lang-syntax module_use_grammar_tests -- --nocapture
cargo test -p arcweft-lang-syntax
cargo clippy -p arcweft-lang-syntax --all-targets --all-features -- -D warnings
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root . --write docs/implementation/structure-audits/proof-concurrency-v6-1-1-stage-1-module-use-grammar-2026-07-17
```

## Structure

The canonical report is stored under
`structure-audits/proof-concurrency-v6-1-1-stage-1-module-use-grammar-2026-07-17/`.
It scanned 3,065 files, 1,533 Rust files, 705,500 physical Rust LOC, and 90
manifests with zero errors and 130 pre-existing repository-wide warnings. No
warning names an in-scope file.

- `parser.rs`: 23,924 bytes / 691 physical LOC, production parser facade;
- `parser/document.rs`: 18,639 bytes / 589 physical LOC, private document/event
  orchestration;
- `parser/module_use_grammar.rs`: 11,364 bytes / 365 physical LOC, module and
  import declaration grammar;
- `parser/module_use_grammar_tests.rs`: 5,069 bytes / 178 physical LOC, direct
  test module.

All changed production files remain below repository warning thresholds. This
cut changes no Cargo dependency, feature, public API, serialization contract,
or crate boundary, so dependency fan-in and fan-out are unchanged.

## Remaining Stage 1 work

Stage 1 remains open. Stable retained declaration families such as `struct`,
`enum`, `type`, `trait`, and `impl` still need direct typed descendants and the
full recovery matrix. Declarations whose final top-level role is under the
Lang-01 design requests are not frozen by this cut. Proof-concurrency Stages 2
through 8 remain open.
