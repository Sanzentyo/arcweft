# Lang-01.4 typed resource shadow grammar Cut 1a

## Scope

This is the first implementation slice of the implementation-ready Lang-01.4
typed resource package. The package inspected `main` at `a8403dcb26d7`; this
slice was implemented on the later accepted base `590b7c5fd90a`, after the
module/import, nominal type, trait/impl, and Flow shadow-grammar cuts.

This slice installs the final `res` declaration vocabulary in the private,
lossless shadow grammar. It does not claim completion of package Cut 1 or of
Lang-01.4 as a whole. The public AST/parser, formatter, semantic resource
identity, descriptor registry, HIR, bundle/runtime directory, family lowering,
and repository source migration remain later cuts.

## Implemented contract

- `res` is the sole shadow-grammar resource declaration keyword. It is kept as
  authored and is not accompanied by `resource`, `def`, or another alias.
- The provisional `EntityDeclarationItem` shadow scaffold and its `entity`
  keyword classification were directly replaced by
  `ResourceDeclarationItem`, `ResourceBody`, and
  `ResourceFieldInitializer`.
- A declaration owns outer documentation/attributes, visibility, an optional
  explicit entity-reference token, a local `NameDefinition`, `:`, a typed
  nominal head, and a braced field body.
- Explicit `@.relative` declaration IDs receive
  `syntax.resource.relative_declaration_id`; ordinary absolute IDs remain
  lossless tokens for the later identity layer.
- Path and generic-application heads are structurally typed. Non-path heads
  are retained with `syntax.resource.invalid_type_head`; semantic rejection of
  generic resource heads remains owned by Lang-01.4 sema Cut 3.
- Every field is a `ResourceFieldInitializer` with its own
  `NameDefinition` and typed expression initializer. No raw field line or
  string field bag was introduced.
- Newline and comma field separators are accepted. Shared record-expression
  parsing now also retains newline-separated nested nominal-record fields, so
  resource values reuse the ordinary expression grammar rather than a resource
  reparser.
- Missing header parts and malformed fields use the package's typed diagnostic
  codes and preserve the following field/declaration as a sibling.

The shadow grammar remains crate-private and allocates no production
`SyntaxNodeId`. The old public parser still owns current executable syntax
until the planned atomic syntax switch; this slice does not add a second public
resource reader.

## Direct evidence

`parser::resource_grammar_tests` covers:

- generated fully-qualified syntax with docs, attributes, visibility, an
  explicit public ID, a module-qualified type, native field expressions, and a
  nested record whose fields use logical newlines;
- one generic head and one invalid reference-type head;
- missing name, colon, type, and body plus a relative declaration ID, followed
  by a clean proof declaration;
- malformed and missing field initializers without hiding later fields or the
  next declaration; and
- absence of resource nodes for the removed `entity` scaffold and an old
  family head.

The focused suite passes 5/5. The complete `arcweft-lang-syntax` all-targets
suite passes 202 unit tests and every integration/UI target.

## Validation

The following commands pass with `CARGO_INCREMENTAL=0` where Cargo is used:

```bash
cargo fmt --all -- --check
cargo test -p arcweft-lang-syntax resource_grammar_tests --lib
cargo test -p arcweft-lang-syntax --all-targets
cargo clippy -p arcweft-lang-syntax --all-targets --all-features -- -D warnings
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root . --write docs/implementation/structure-audits/lang-01-4-typed-resource-shadow-grammar-cut-1a-2026-07-17
git diff --check
```

The first focused Clippy run reported only a missing semicolon and two
redundant test closures; both were corrected before the passing runs. One
subsequent Clippy process reached Cargo's successful `Finished` state but the
shell wrapper timed out at 120 seconds; an immediate identical rerun exited
successfully, and the later workspace-wide Clippy run also passed.

## Structure

The canonical report is stored under
`structure-audits/lang-01-4-typed-resource-shadow-grammar-cut-1a-2026-07-17/`.
It scanned 3,094 files, 1,548 Rust files, 709,061 physical Rust LOC, and 90
manifests with zero errors and 128 pre-existing repository-wide warnings. No
warning names an in-scope file.

- `grammar/kinds.rs`: 8,322 bytes / 353 physical LOC, private final grammar
  vocabulary;
- `parser.rs`: 24,169 bytes / 702 physical LOC, parser facade;
- `parser/document.rs`: 21,205 bytes / 665 physical LOC, declaration grouping
  and event dispatch;
- `parser/item.rs`: 5,194 bytes / 157 physical LOC, top-level classification;
- `parser/lexer.rs`: 12,908 bytes / 445 physical LOC, one-pass lexer;
- `parser/resource_grammar.rs`: 12,810 bytes / 383 physical LOC, resource
  declaration ownership and recovery;
- `parser/expression/composite.rs`: 17,819 bytes / 550 physical LOC, shared
  record-expression field separation; and
- `parser/resource_grammar_tests.rs`: 5,764 bytes / 169 physical LOC, direct
  tests.

All changed production files remain below repository warning thresholds. This
slice changes no Cargo dependency, feature, public Rust API, serialization
format, or crate boundary, so dependency fan-in and fan-out are unchanged.

## Remaining Lang-01.4 work

Package Cut 1 still requires the public syntax switch/AST accessors and
formatter ownership. Cuts 2 through 6 remain open: typed descriptor manifests
and immutable registry; identity/HIR/sema/project-index/tooling; bundle/runtime/
Agent/save/hot-reload mapping; the Image vertical slice; the ordered audio,
motion, and rig migrations; and atomic removal of every old public family
reader/variant/example.

Those later cuts must preserve the final contract's `asset`/`res` separation,
native-only Motion lowering, and verified image/audio/View owners. They must not
add a compatibility parser, dedicated removed-syntax diagnostic, source gate,
CSS route, or Takumi route.
