# Proof concurrency v6.1.1 — Stage 3 private bound parse product

Date: 2026-07-21

## Outcome

This cut adds the last safe private ownership step before the Proof-concurrency
v6.1.1 public syntax/HIR migration. The accepted source package is
`arcweft-proof-concurrency-v6.1.1-typed-ast-proof-block-hir-runtime-identity-final-contract.zip`,
with SHA-256
`1b7de5f2c10a5b29d67c72011e4272df9a76af8907fd21fe162de54809fc69ef`.
The cut started from Git
`b2c2f423ba41f719c8ee86c2009d4333a9a4b865`.

The private grammar transaction now commits one `BoundParsedSource` containing:

- the exact immutable `SourceDocument` already owned by its attached syntax
  snapshot;
- the qualified syntax snapshot and its stable grammar identities;
- recoverable grammar diagnostics in parser order, with primary and optional
  related ranges converted to `SourceSpan` values bound to that exact source
  revision; and
- a clean/recovered status derived from both structured diagnostics and
  missing/error grammar nodes.

The product remains crate-private. It is shared inside the session-thread-affine
incremental database with `Rc`, while its immutable source and attached syntax
data retain their existing `Arc` ownership. This avoids claiming cross-thread
semantics for Rowan red nodes and avoids adding another public reader.

Initial parse and reparse stage the complete product before lineage mutation.
Grammar construction validates primary and related diagnostic ranges before
attachment, and binding either span can still fail the same transaction as
attachment or identity failure. A successful reparse publishes a fresh bound
product; previous immutable products and their revision-bound diagnostics
remain unchanged.

## Direct behavioral evidence

The new tests prove that:

- an accepted recovered proof retains its attached snapshot, exact source
  revision, grammar diagnostic, zero-width insertion span, message, and
  recovered status in one product;
- a duplicate Character member binds both its primary span and the related
  first-declaration span to the exact committed source revision; and
- an invalid related diagnostic range is rejected during grammar construction,
  while a missing-token event without a diagnostic still marks the complete
  transaction recovered; and
- repairing recovered input commits a fresh clean product with no diagnostics
  while the old recovered product and source remain immutable.

No test searches repository source text. The spelling searches in the fixtures
locate expected ranges inside the source value under test, not implementation
files.

## Why the public switch does not land in this cut

A current-checkout consumer audit found two parse-result authorities:

- `source::ParsedSource`, which owns the existing line CST and detached
  `TypedSyntaxTree`; and
- `incremental::ParsedSource`, which exposes that result while privately
  retaining the accepted attached grammar snapshot.

The incremental transaction still invokes both parsers. HIR then clones
detached expressions, statements, patterns, types, and declaration payloads.
Switching only `ParsedSource` would therefore require either an
attached-to-detached compatibility adapter or a second public authority, both
of which are prohibited.

At this base revision, a mechanical dependency survey found 315
`.typed_tree()` references across 80 Rust files, 49 `TypedSyntaxTree`
references, 638 `lower_to_hir` references across 65 Rust files, and 418
`parse_source` references across 99 Rust files. Fourteen workspace packages
depend directly on `arcweft-lang-syntax`. These counts are migration-size
evidence, not source gates or frozen acceptance criteria.

The survey used `rg` only over `crates/**/*.rs`, with exact regexes
`\.typed_tree\(\)`, `\bTypedSyntaxTree\b`, `\blower_to_hir\b`, and
`\bparse_source\b`; names such as `parse_source_fragment` were intentionally
excluded. This records the filtering method without turning the count into an
automated gate.

The accepted private grammar also must converge with the final top-level
declaration reduction before it can replace the old parser. Its RichText
boundary currently retains dialogue bracket payloads losslessly but does not
own identity-bearing tag or tag-argument nodes.

## RichText boundary

This cut deliberately does not manufacture private RichText tag nodes, reparse
tag payload ranges, wrap detached `DialogueTagArg` values, or publish a dual
reader. Ordered and ranged RichText tag arguments must be produced by the same
shared grammar transaction that becomes the sole `ParsedSource` authority.
Only then may the attached accessor and HIR ownership switch land together.

## Required atomic migration order

The remaining public Stage 3 work is one dependency-ordered migration:

1. finish the canonical top-level declaration reduction and accepted item
   inventory;
2. extend the shared grammar transaction to every retained surface, including
   ordered/ranged RichText tags and arguments;
3. add bound fragment entry points needed by REPL/tooling without returning a
   detached AST;
4. switch `ParsedSource`, HIR lowering, project/LSP/compiler consumers, and
   diagnostic ownership to attached `SyntaxNodeId`-backed structures in one
   coherent cut; and
5. delete the detached `TypedSyntaxTree`, old line-identity bridge, duplicate
   parse, and detached fragment entry points rather than preserving aliases or
   adapters.

The later public cut spans syntax, HIR, tooling, and runtime consumers. It must
run workspace validation and Tier 2 after reconciling current production
identities and authored View geometry.

## Verification

All commands ran from the repository root:

- `cargo test -p arcweft-lang-syntax --all-features private_bound`: passed,
  3 focused tests;
- `cargo test -p arcweft-lang-syntax --all-features grammar::build::tests`:
  passed, 5 focused grammar-build tests;
- `cargo test -p arcweft-lang-syntax --all-features`: passed, including 406
  unit tests, all integration and compile-fail suites, and 3 doc tests;
- `cargo clippy -p arcweft-lang-syntax --all-targets --all-features -- -D warnings`:
  passed after replacing the private product's inappropriate outer `Arc` with
  session-thread-affine `Rc` ownership;
- `cargo fmt --all -- --check`: passed;
- `git diff --check`: passed; and
- `cargo +nightly -Zscript tools/structure-audit.rs --root . --write
  docs/implementation/structure-audits/proof-concurrency-v6-1-1-stage-3-private-bound-product-2026-07-21`:
  scanned 3,437 files, 1,789 Rust files, 823,224 Rust physical lines, and 93
  manifests; it reported 0 errors and 131 existing warnings.

Tier 2 is not required for this private syntax-only cut. It changes no public
contract and reaches no runtime, renderer, Agent, MCP, or capture path.

## Structural audit

No manifest, dependency, feature, public contract, or crate boundary changed.
The new responsibility module owns only the exact source-bound parse product;
grammar event construction, attachment, identity reconciliation, and public
legacy projection remain separate during this private stage.

| Changed Rust file | Bytes | Physical LOC | Classification | Responsibility |
| --- | ---: | ---: | --- | --- |
| `src/grammar/build.rs` | 22,473 | 623 | production | complete-transaction recovery classification and diagnostic-range validation |
| `src/incremental/bound.rs` | 3,733 | 124 | production | private exact source/snapshot/diagnostic product |
| `src/incremental/database.rs` | 16,392 | 508 | production | public legacy result plus private bound-product commit |
| `src/incremental/database_tests.rs` | 47,874 | 1,439 | unit test | transaction, revision, and diagnostic ownership evidence |
| `src/incremental/transaction.rs` | 7,002 | 225 | production | atomic grammar staging and lineage reconciliation |
| `src/incremental.rs` | 332 | 14 | production facade | private module ownership |

No changed file crosses its applicable structural warning threshold. The
`arcweft-lang-syntax` dependency graph remains 14 incoming and 8 outgoing
normal/development workspace edges.
