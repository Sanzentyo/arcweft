# Proof concurrency v6.1.1 — Stage 3 private bound expression fragment

Date: 2026-07-21

## Outcome

This cut adds the smallest private fragment prerequisite for the accepted
Proof-concurrency v6.1.1 contract. The source package is
`arcweft-proof-concurrency-v6.1.1-typed-ast-proof-block-hir-runtime-identity-final-contract.zip`,
with SHA-256
`1b7de5f2c10a5b29d67c72011e4272df9a76af8907fd21fe162de54809fc69ef`.
The cut started from Git
`aedb457be465ebc21c4e674be98f6c6fccc2f086`.

The syntax crate now has one crate-private standalone-expression entry point
that:

- uses the same `DocumentLexer`, grammar budget, event builder, lossless
  `SourceFile` root, Pratt expression grammar, recovery nodes, and diagnostic
  transaction as complete documents;
- requires an explicit `SourceSnapshotId` and exact `SourceDocument` instead
  of inventing source identity;
- attaches the expression to a fresh database-owned syntax lineage and returns
  a distinct `BoundExpressionFragment` containing an attached
  `SyntaxNodeHandle`;
- binds all recovery diagnostics to the exact immutable source revision; and
- commits the next lineage only after parsing, identity allocation, attachment,
  and bound-product construction all succeed.

The entry point returns no detached `Expr`, `Stmt`, or `Item`. It is not
publicly exported, cannot be passed as a complete `ParsedSource`, and creates
no compatibility reader or alternative public authority.

## Direct behavioral evidence

The new tests prove that:

- a clean expression with leading and trailing trivia retains exact source
  text while its attached expression node owns only the semantic expression
  range;
- two explicit attachments allocate distinct database lineages and node
  identities;
- empty input produces an attached zero-width `MissingExpression`, not a
  detached placeholder value;
- a recovered call binds its missing-close diagnostic and insertion range to
  the exact fragment source revision; and
- source-name mismatch and injected attachment failure consume neither syntax
  lineage nor node identity, so the next valid transaction matches a clean
  control database.

These are direct parser, identity, attachment, and rollback checks. No source
gate or repository-text assertion was added.

## Deliberate boundary

This predecessor is expression-only. Type, pattern, and statement fragments
remain out of this cut because their final ordinary owner/context contract and
the package's public `UnboundFragment<K>` plus explicit source-span attachment
API must land together. Publishing a partial fragment family now would create
the prohibited dual reader.

The cut also does not add RichText tag nodes, reparse dialogue payload ranges,
or migrate REPL, LSP, HIR, compiler, or runtime consumers. Those consumers must
switch atomically after the retained top-level grammar and complete bound
fragment contract converge.

## Verification

All commands ran from the repository root:

- `cargo test -p arcweft-lang-syntax --all-features`: passed, including 410
  unit tests, all integration and compile-fail suites, and 3 doc tests;
- `cargo clippy -p arcweft-lang-syntax --all-targets --all-features -- -D warnings`:
  passed;
- `cargo fmt --all -- --check`: passed;
- `git diff --check`: passed; and
- `cargo +nightly -Zscript tools/structure-audit.rs --root . --write
  docs/implementation/structure-audits/proof-concurrency-v6-1-1-stage-3-private-bound-expression-fragment-2026-07-21`:
  scanned 3,437 files, 1,789 Rust files, 823,577 Rust physical lines, and 93
  manifests; it reported 0 errors and 131 existing warnings.

Tier 2 is not required for this private syntax-only cut. It changes no public
contract and reaches no runtime, renderer, Agent, MCP, or capture path. The
later public syntax/HIR/tooling migration remains a Tier 2 cut.

## Structural audit

No manifest, dependency, feature, public contract, or crate boundary changed.
The shared root transaction remains in the document parser, fresh identity and
attachment staging remain in the incremental transaction, and the bound
fragment product remains in the incremental ownership module.

| Changed Rust file | Bytes | Physical LOC | Classification | Responsibility |
| --- | ---: | ---: | --- | --- |
| `src/incremental/bound.rs` | 5,931 | 208 | production | private shared bound product and expression-fragment owner |
| `src/incremental/database.rs` | 18,245 | 560 | production | explicit private fragment entry point and atomic commit boundary |
| `src/incremental/database_tests.rs` | 53,394 | 1,574 | unit test | fragment source, recovery, identity, and rollback evidence |
| `src/incremental/transaction.rs` | 8,933 | 283 | production | shared fresh-lineage grammar attachment transaction |
| `src/parser/document.rs` | 28,523 | 876 | production | shared lossless root and standalone expression grammar emission |
| `src/parser.rs` | 25,777 | 774 | production facade | crate-private grammar entry-point routing |

No changed production file crosses its applicable structural warning
threshold. The changed test file remains below the 2,500-line integration-test
warning threshold.
