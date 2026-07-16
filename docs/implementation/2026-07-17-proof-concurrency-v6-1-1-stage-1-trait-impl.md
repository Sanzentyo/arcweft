# Proof-concurrency v6.1.1 Stage 1 trait and impl grammar

## Scope

This cut extends the private, lossless shadow grammar with the retained
`trait` and `impl` declaration families. It is based on main `7abb217093e0`
and Jujutsu change `qrrvowns`. It does not switch the public parser, syntax
identity, HIR, project, or runtime contracts.

Both families consume the document parser's existing lexer tokens and emit
into its single cursor/event stream. The implementation introduces no source
substring reparse, compatibility alias, removed-syntax recognizer, source
gate, CSS route, or Takumi route.

## Typed ownership

- trait and impl headers own visibility, ordinary names or target types,
  generic parameters, supertraits, and `where` predicates;
- associated type requirements and assignments are nested identity-bearing
  `TypeAliasItem` nodes;
- method signatures are nested `FunctionItem` nodes and retain every curried
  fixed-parameter group independently;
- `self`, `mut self`, `&self`, and `&mut self` use the shared binding-pattern
  vocabulary without inventing missing parameter types;
- method bodies reuse the shared block/statement/expression grammar; and
- declaration and method delimiters use the shared delimiter-node authority.

Trait default methods, trait associated-type defaults, and bodyless impl
method signatures remain syntax-preserving forms. Whether a retained form is
accepted semantically stays with the existing sema/coherence layer rather than
being reclassified as a syntax error by the shadow grammar.

Member documentation and outer attributes remain attached typed prefixes.
Optional semicolons terminate associated types and bodyless signatures without
becoming part of the nested type node. Several semicolon-delimited members on
one physical line still receive distinct identity-bearing item nodes.

## Recovery

- an impl associated type without `= Type` retains its name and a zero-width
  `MissingType`;
- invalid member lines become typed `ErrorItem` children;
- missing declaration braces emit zero-width close nodes; and
- an unclosed declaration synchronizes before the following unindented
  declaration, which remains a sibling item.

Every recovery case remains byte-lossless and queryable. This private shadow
output allocates no production `SyntaxNodeId` and is not executable.

## Direct evidence

`parser::trait_impl_grammar_tests` covers production trait/impl fixtures with
associated types, generic impl targets, supertraits and bounds, curried method
groups, method bodies, all four receiver forms, member documentation and
attributes, semicolon separators, missing associated assignments, invalid
member recovery, and unclosed-body synchronization. Every case asserts typed
descendants and byte-for-byte green-tree text.

The focused suite passes 6/6. The complete `arcweft-lang-syntax` all-targets
suite passes 193 unit tests together with every integration and UI test. Its
documentation-test target also passes with zero documentation tests present.

## Validation

The following commands pass with `CARGO_INCREMENTAL=0` where Cargo is used:

```bash
cargo fmt --all -- --check
cargo test -p arcweft-lang-syntax trait_impl_grammar_tests --lib -- --nocapture
cargo test -p arcweft-lang-syntax --all-targets
cargo test -p arcweft-lang-syntax --doc
cargo clippy -p arcweft-lang-syntax --all-targets --all-features -- -D warnings
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root . --write docs/implementation/structure-audits/proof-concurrency-v6-1-1-stage-1-trait-impl-2026-07-17
```

The first focused test exposed that the shared fixed-parameter parser treated
`self` as an untyped ordinary parameter. Receiver recognition was moved into
that shared parameter authority and the four accepted receiver forms now have
direct coverage. The first Clippy run then rejected one match arm without a
trailing semicolon; that formatting issue was corrected and the exact command
passed on rerun.

## Structure

The canonical report is stored under
`structure-audits/proof-concurrency-v6-1-1-stage-1-trait-impl-2026-07-17/`.
It scanned 3,089 files, 1,545 Rust files, 708,209 physical Rust LOC, and 90
manifests with zero errors and 128 pre-existing repository-wide warnings. No
warning names an in-scope file.

- `parser.rs`: 24,070 bytes / 697 physical LOC, production parser facade;
- `parser/declaration.rs`: 16,518 bytes / 467 physical LOC, shared declaration
  grammar including receiver patterns;
- `parser/document.rs`: 19,843 bytes / 623 physical LOC, private
  document/event orchestration;
- `parser/trait_impl_grammar.rs`: 15,892 bytes / 498 physical LOC, production
  trait/impl grammar; and
- `parser/trait_impl_grammar_tests.rs`: 5,637 bytes / 169 physical LOC, direct
  test module.

All changed production files remain below repository warning thresholds. This
cut changes no Cargo dependency, feature, public API, serialization contract,
or crate boundary, so dependency fan-in and fan-out are unchanged.

## Remaining Stage 1 work

Stage 1 remains open. The remaining retained declaration families and their
malformed/recovery cross-products still require direct typed descendants. The
Lang-01 contracts continue to own migration-gated top-level reductions; this
cut does not preserve a declaration merely because the old parser once
accepted it. Proof-concurrency Stages 2 through 8 remain open.
