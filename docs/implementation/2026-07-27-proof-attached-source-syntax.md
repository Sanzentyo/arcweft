# Proof convergence: attached Source syntax

Date: 2026-07-27

Status: implementation complete; reviewable-cut validation complete

## Contract and precedence

The accepted base Proof-concurrency package lists `SourceItem` in the lossless
typed syntax inventory and `HirItem::Source(HirSourceItem)` in the HIR item
inventory. The package re-audit recorded in
[`2026-07-25-proof-stage-3-deletion-driven-authority-switch.md`](2026-07-25-proof-stage-3-deletion-driven-authority-switch.md)
therefore supersedes the older Stage 1 audit that proposed omitting a private
Source node.

Proof keeps the current Source declaration long enough to replace its detached
syntax carrier during the atomic ParsedSource/HIR switch. Lang-01.3 remains the
owner of the later all-layer Source-to-Stream replacement and deletion. This
ordering does not authorize repairs or new features in the old detached Source
AST/HIR/runtime path.

The returned Proof `v6.1.1.4` attachment is still byte-identical to the rejected
SHA-256
`414f95f8ef4c5f3abcce163f0c9b01f124098f0bac856f174af09b5c1e7d564b`.
Final semantic leaf records and the public HIR switch remain gated on the
corrected
[`01.1.1.4.1` redelivery](../reviews/requests/2026-07-26-seq-proof-01.1.1.4.1-final-hir-leaf-expression-redelivery-correction.md).
The attached Source grammar is independent of that leaf schema.

## Implemented attached owner

- added `SourceItem` to the private lossless grammar's identity, item-family,
  classifier, grouped-document dispatcher, exact marker, and typed accessors;
- retained canonical local-name, absolute-ID, relative-ID, family-relative-ID,
  ID-only, and fully elaborated ID-plus-name headers;
- validates attached Source declaration IDs in the grammar transaction: empty
  `@.` / `@source:.` markers require a following name, parent-relative forms
  remain valid, and malformed or wrong-family IDs retain typed recovery nodes;
- partitioned the lexer's combined `@source.events:` token inside the same
  grammar transaction so `DeclarationPublicId` excludes the type colon while
  the lossless tree retains every byte;
- retained the source type as the shared typed type family and the body as the
  shared Block/MissingBody union;
- represented `from` through an `ExpressionStatement` with a typed
  `Initializer`, policy rows through ordinary assignment statements, and
  `on` handlers through shared pattern, condition, statement, and block owners;
- represents `requires` and `ensures` with the shared typed contract-clause
  owner rather than lowering their text through ordinary statement recovery;
- added exact missing-name, missing-type, missing-body, missing-handler-arrow,
  missing-handler-body, and missing-close recovery while preserving a following
  declaration as a root sibling;
- synchronizes an unclosed generic Source type at the actual body brace and an
  unclosed handler block at the next Source-body entry, retaining the body and
  every later declaration without reparsing source text;
- made function-shaped `source name() -> ...` recovered by the ordinary current
  grammar's missing-colon evidence, without a historical AST kind or a
  spelling-specific removed-syntax diagnostic; and
- added no public reader, HIR payload, compatibility alias, wrapper, source
  gate, range search, or source substring reparse.

The protected WIP Source grammar was not copied verbatim. Its absolute-only ID
validation conflicts with the stable grammar, and its exact body accessor did
not admit the package's required MissingBody recovery. This cut uses the stable
Source ID family and a typed body union while keeping the later lexical-metadata
publication out of the private syntax slice.

## Deletion-driven boundary

The public `parser/source.rs`, `ast/source.rs`, `Item::Source`, `HirSource`, and
their production consumers are unchanged and frozen. They remain explicit
deletion inventory for the same public-authority cut that lowers the attached
Source node into the accepted HIR database. No defect in those old carriers was
fixed, and the new attached reader remains crate-private, so this slice does not
create a second public semantic authority.

After that Proof switch, Lang-01.3 replaces Source with ordinary functions that
produce Stream and deletes both the attached Source syntax/HIR and the remaining
runtime/wire Source ownership in one compiling migration.

## Validation

- focused Source grammar matrix: 15 passed, covering canonical/relative ID
  forms, ID partitioning, malformed and wrong-family identity, typed Source
  headers/contracts/handlers, generic and handler recovery, and lossless
  following-item preservation;
- focused Source incremental reconciliation: passed; trivia edits preserve the
  SourceItem, header, type, and handler `SyntaxNodeId` values while updating
  their revision-local spans;
- full `cargo test -p arcweft-lang-syntax`: passed 488 library tests and every
  syntax integration, compile-fail, and documentation test;
- `cargo check -p arcweft-lang-syntax --all-targets --all-features`: passed;
- `cargo clippy -p arcweft-lang-syntax --all-targets --all-features -- -D
  warnings`: passed;
- `cargo check --workspace --all-targets --all-features`: passed;
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`:
  passed;
- independent review found three initial blockers: unclosed generic grouping,
  empty/wrong-family Source IDs, and untyped Source contract clauses. All three
  were corrected, the requested handler/reconciliation matrix was added, and a
  later stable delimited `@<...>` EntityRef gap was corrected without retaining
  the provisional `@{...}` spelling. Final re-review found no implementation
  blocker;
- the canonical structural audit scanned 3,718 files, 1,941 Rust files,
  903,839 Rust physical lines, and 95 manifests with 0 errors and 144 existing
  warnings. Its exact reports are in
  [`structure-audits/proof-attached-source-syntax-2026-07-27/`](structure-audits/proof-attached-source-syntax-2026-07-27/);
- the first `just test-workspace` attempt was interrupted by Windows paging-file
  exhaustion while rustc tried to map an `arcweft-lsp` rlib (`os error 1455`),
  not by a Rust diagnostic or test failure;
- a retry with `CARGO_BUILD_JOBS=2` reached the end of the workspace suite. It
  retained only the established `arcw_fixtures_check_run` baseline failures for
  `spec_should_pass/check/010_capability_fs_read.arcw` and
  `spec_should_pass/run/002_file_read_task.arcw`; all preceding suites passed;
  and
- exact `cargo test -p arcweft-cli --test arcw_fixtures_check_run --
  --nocapture` reproduction reported 3 passed and the same 2 failed. These
  `FsError` capability fixtures predate and do not exercise the private Source
  grammar.

This private syntax-only change does not affect runtime, rendering, Agent, MCP,
or capture behavior, so Tier 2 is not required by the test-execution policy.

## Structural measurement

Revision under review: Jujutsu change `uusyxvks` over `main@843a68d2`. No
workspace dependency, Cargo feature, public crate contract, or crate boundary
changed.

| Path | Bytes | Physical LOC | Classification |
| --- | ---: | ---: | --- |
| `attachment/access.rs` | 29,810 | 859 | production with embedded unit tests |
| `attachment/node.rs` | 16,717 | 462 | production with embedded unit tests |
| `attachment.rs` | 52,629 | 1,497 | attachment facade plus embedded unit tests |
| `grammar/kinds.rs` | 38,441 | 1,178 | production identity vocabulary with embedded unit tests |
| `incremental/database_tests.rs` | 63,518 | 1,761 | test |
| `parser/declaration.rs` | 25,977 | 729 | production shared declaration grammar |
| `parser/document.rs` | 33,208 | 1,027 | production document orchestration |
| `parser/item.rs` | 5,456 | 150 | production item classification |
| `parser/item_tests.rs` | 4,829 | 150 | test |
| `parser/lexer.rs` | 16,397 | 546 | production lossless tokenization |
| `parser/retained_grammar_tests.rs` | 10,689 | 299 | test |
| `parser/source_grammar.rs` | 18,920 | 552 | production Source grammar |
| `parser/source_grammar_tests.rs` | 17,233 | 493 | test |
| `parser/statement.rs` | 26,877 | 800 | production shared statement/block grammar |
| `parser/type_ref.rs` | 9,532 | 293 | production shared type grammar |
| `parser.rs` | 25,786 | 778 | production parser facade/module owner |

`attachment.rs` remains above the 1,200-LOC warning threshold and contains its
pre-existing embedded attachment tests. This cut adds only the crate-private
Source marker export there; the new Source responsibility is isolated in the
552-LOC grammar module and its separate 493-LOC test module. The audit reports
no error-level ownership violation, so moving the established attachment test
suite is not part of this syntax slice.
