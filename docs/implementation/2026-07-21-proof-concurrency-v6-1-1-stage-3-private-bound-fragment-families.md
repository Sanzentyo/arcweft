# Proof concurrency v6.1.1 — Stage 3 private bound fragment families

Date: 2026-07-21

Status: `HISTORICAL_PREDECESSOR_SUPERSEDED`

The document-range lexer and `BoundFragment<K>` model described here were
deleted on 2026-07-27. The replacement retains one source-free grammar event
tree and attaches it to exact matching bytes without reparsing; see
[`2026-07-27-proof-unbound-fragment-exact-attachment.md`](2026-07-27-proof-unbound-fragment-exact-attachment.md).

## Outcome

This cut completes the private fragment-family prerequisite for the accepted
Proof-concurrency v6.1.1 contract. The source package is
`arcweft-proof-concurrency-v6.1.1-typed-ast-proof-block-hir-runtime-identity-final-contract.zip`,
with SHA-256
`1b7de5f2c10a5b29d67c72011e4272df9a76af8907fd21fe162de54809fc69ef`.
Implementation started from Git
`110253ccbce8d6a9dd8975175a6881173502d41c` and was sealed on current `main`
`91e6687c604528a9fd9348e2c3fd99a4dae45dbb`.

The syntax crate now has crate-private, database-bound fragment ownership for
expressions, types, patterns, and ordinary statements. Each family:

- requires the exact immutable `SourceDocument` and an explicit `SourceSpan`;
- lexes only that span with the shared document lexer while preserving
  document-absolute token ranges;
- emits the family through the existing grammar-event implementation into the
  same lossless `SourceFile` transaction used by complete documents;
- retains source bytes outside the semantic fragment as root-level lossless
  text rather than reparsing or discarding them;
- attaches the accepted family root to a fresh database-owned syntax lineage;
- returns only a typed private `BoundFragment<K>` with an attached
  `SyntaxNodeHandle`, never a detached expression, type, pattern, statement, or
  item; and
- commits the next lineage only after grammar construction, source-span
  validation, identity allocation, attachment, diagnostic binding, and bound
  product construction all succeed.

The generic marker type owns both its grammar-family selector and its accepted
root-kind predicate. Internal callers therefore cannot pair a marker with the
wrong parser family. Empty and recovered fragments keep their structured
missing/error nodes and revision-qualified diagnostics at the explicit span.

## Attachment identity correction

The new recovered-statement evidence exposed an existing attachment flaw:
nested missing delimiters of the same kind at the same zero-width offset are
equal as Rowan red nodes. A `HashMap<GrammarSyntaxNode, _>` therefore collapsed
two distinct grammar-event paths and rejected the otherwise valid tree as a
duplicate attachment.

Attachment lookup now derives the exact child-index path from the Rowan node
and resolves that path through the authoritative `GrammarEventPath` map. This
preserves distinct stable identities for same-kind, same-offset recovery nodes
without adding a range heuristic, spelling exception, source scan, or fallback
identity. The behavior is covered directly by a recovered nested-call
statement fixture whose two missing closing delimiters bind and round-trip as
distinct nodes.

## Direct behavioral evidence

The new tests prove that:

- embedded type, pattern, and statement spans retain the exact complete source
  while their semantic roots own only the selected range;
- every family receives a distinct database lineage and attached node identity;
- empty non-expression fragments attach the correct zero-width
  `MissingType`, `MissingPattern`, or `ErrorStatement` node;
- recovered type, pattern, and statement diagnostics bind to the exact source
  revision and requested span;
- source mismatch and injected attachment failure consume neither lineage nor
  node identity for every family; and
- two nested same-kind, same-offset missing delimiters retain distinct event
  paths, stable identities, and exact Rowan bindings.

These are parser, typed-family, source-revision, attachment, identity, and
rollback tests. No source gate or repository-text assertion was added.

## Deliberate boundary

Standalone statement fragments use the ordinary function-statement context.
Proof and predicate restrictions remain owned by their enclosing document
items; this cut does not invent a context-free owner policy for them.

This remains a private predecessor. It deliberately does not:

- publish `UnboundFragment<K>`, `AttachedFragment<K>`, or
  `SyntaxDatabase::attach_fragment`;
- add an item-fragment family before retained top-level ownership is final;
- return detached AST values or preserve a public dual reader;
- manufacture RichText shadow nodes or reparse dialogue payload ranges; or
- migrate `ParsedSource`, HIR, sema, runtime-plan, verifier, CLI, LSP, Agent,
  MCP, or capture consumers.

The remaining public switch must bind the exact caller-provided fragment bytes,
publish explicit attachment in the accepted API shape, migrate all consumers
to attached syntax ownership, and delete the detached fragment and parse
authorities in one coherent cut. That integration cut must run Tier 2.

## Verification

All commands ran from the repository root after rebasing onto current `main`:

- `cargo test -p arcweft-lang-syntax --all-features`: passed, including 414
  unit tests, all integration and compile-fail suites, and 3 doc tests;
- `cargo test -p arcweft-lang-syntax --all-features private_`: passed, 21
  focused tests;
- `cargo test -p arcweft-lang-syntax --all-features attachment::`: passed, 15
  focused tests;
- `cargo clippy -p arcweft-lang-syntax --all-targets --all-features -- -D warnings`:
  passed;
- `cargo fmt --all -- --check`: passed;
- `git --git-dir=D:\git\arcweft\.git
  --work-tree=D:\git\arcweft-ws-proof-fragments diff --check
  91e6687c604528a9fd9348e2c3fd99a4dae45dbb --`: passed; and
- `cargo +nightly -Zscript tools/structure-audit.rs --root . --write
  docs/implementation/structure-audits/proof-concurrency-v6-1-1-stage-3-private-bound-fragment-families-2026-07-21`:
  scanned 3,447 files, 1,793 Rust files, 826,967 Rust physical lines, and 94
  manifests; it reported 0 errors and 131 existing warnings.

Tier 2 is not required for this private syntax-and-attachment-only cut. It
changes no public contract and reaches no runtime, renderer, Agent, MCP, or
capture path.

## Structural audit

No manifest, dependency, feature, public contract, or crate boundary changed.
The shared root transaction remains in the document parser; fragment-family
selection and atomic commit remain in incremental ownership; stable attachment
lookup remains path-based in the attachment subsystem.

| Changed Rust file | Bytes | Physical LOC | Classification | Responsibility |
| --- | ---: | ---: | --- | --- |
| `src/attachment/snapshot.rs` | 11,230 | 393 | production | immutable path-based Rowan attachment lookup |
| `src/attachment.rs` | 34,768 | 974 | production with embedded unit tests | attachment construction and exact recovery identity fixtures |
| `src/grammar/build.rs` | 22,663 | 629 | production with embedded unit tests | structured invalid-fragment-range failure |
| `src/incremental/bound.rs` | 8,543 | 294 | production | generic private bound fragment and typed family markers |
| `src/incremental/database.rs` | 20,403 | 632 | production | explicit source/span entry points and atomic commit boundary |
| `src/incremental/database_tests.rs` | 66,068 | 1,864 | unit test | family source, recovery, identity, and rollback evidence |
| `src/incremental/transaction.rs` | 9,019 | 282 | production | typed family grammar staging and fresh-lineage attachment |
| `src/parser/document.rs` | 30,913 | 958 | production | shared lossless fragment root and family dispatch |
| `src/parser/lexer.rs` | 13,444 | 464 | production | exact-range shared lexical event production |
| `src/parser/statement.rs` | 26,435 | 785 | production | ordinary statement-fragment grammar context |
| `src/parser.rs` | 25,786 | 774 | production facade | crate-private family entry-point routing |

No changed production file crosses its applicable structural warning
threshold. The changed unit-test file remains below the 2,500-line
integration-test warning threshold. Existing repository warnings remain
outside this cut.
