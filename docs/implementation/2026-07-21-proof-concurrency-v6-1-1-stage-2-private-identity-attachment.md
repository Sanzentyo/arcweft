# Proof concurrency v6.1.1 — Stage 2 private identity and attachment

Date: 2026-07-21

## Outcome

Proof-concurrency v6.1.1 Stage 2 is implemented behind the existing public
syntax reader. The accepted source package is
`arcweft-proof-concurrency-v6.1.1-typed-ast-proof-block-hir-runtime-identity-final-contract.zip`,
with SHA-256
`1b7de5f2c10a5b29d67c72011e4272df9a76af8907fd21fe162de54809fc69ef`.
The implementation cut started from Git
`84f9574cfd4a8e0325895156f58f50e5cf4e4c57`.

The private one-pass grammar now participates in every successful initial parse
and reparse transaction:

- `SyntaxDatabaseId`, `SyntaxLineageId`, `SyntaxSnapshotId`, and the new
  qualified private `SyntaxNodeId` prevent equal raw slots from crossing
  database or lineage boundaries;
- immutable `SyntaxSnapshotData`, exact event-path attachment, Rowan
  round-trip lookup, and sealed `AstNode<K>` handles preserve one snapshot and
  one typed kind without range or source-text lookup;
- grammar reconciliation uses identity-parent-local role buckets, ordinal-free
  role classes, owned non-trivia token digests, ordered child-role digests,
  explicit missing/omitted/error recovery classes, unique full-shape matching,
  stable linear-space LCS matching, and deterministic slot tie-breaking;
- initial parse and reparse stage the private grammar, shapes, identities,
  attachment, and allocator in shadow state and publish them only after every
  fallible operation succeeds; and
- attachment failure, syntax-budget failure, and reconciliation failure consume
  neither lineage nor node identities and do not replace the current snapshot.

The existing public incremental `SyntaxNodeId`, `ParsedSource` reader, detached
surface AST, and HIR lowering input are unchanged. The new attachment is
crate-private transaction state. This avoids a dual public reader and reserves
the public AST/HIR switch for Stage 3.

The private grammar inventory no longer contains the unproducible historical
`ExternModuleItem`, `DialogueDefaultsItem`, or `SourceItem` kinds. Their text is
handled only by ordinary current-grammar `ErrorItem` recovery. No compatibility
AST/CST kind, removed-spelling diagnostic, or source gate was added. This
cleanup does not change retained native Style.

## Direct behavioral evidence

The added tests prove:

- repeated nested descendants on one physical line receive distinct qualified
  identities;
- independent databases may allocate the same raw slot but cannot resolve one
  another's IDs;
- exact typed and Rowan handles round-trip without range lookup, while a
  structurally equal foreign Rowan root is rejected;
- trivia-only reparses preserve descendant IDs while old and new handles retain
  their own immutable ranges and reject cross-snapshot resolution;
- unique same-parent reorder preserves identities;
- copying a node preserves one original identity and allocates a fresh copy;
- moving an expression across block parents allocates a fresh identity;
- a changed node is fresh while an unchanged sibling survives;
- missing and error nodes reconcile through their recovery roles; and
- injected attachment failure rolls back both initial and reparse transactions,
  including the private allocator sequence.

Lookup failures remain exact and typed:
`WrongDatabase`, `WrongLineage`, `WrongSnapshot`, `ForeignRowanRoot`,
`MissingNode`, and `KindMismatch`. Private construction failures map to the
existing fatal transaction boundary rather than expanding the public parser
API during Stage 2.

## Completion boundary

This cut completes only private Stage 2. It intentionally does not:

- switch `ParsedSource`, surface AST, HIR, sema, runtime-plan, verifier, CLI, or
  LSP readers to the attached grammar;
- publish any new syntax identity or typed attachment API;
- delete the still-public detached AST before all Stage 3 consumers can move
  atomically;
- implement `HirDatabase`, HIR arenas, proof runtime identity, assertion
  persistence, or codec changes from later package stages; or
- add compatibility aliases, dual readers, migration shims, historical syntax
  diagnostics, or source-text gates.

Stage 3 must first inventory every current `typed_tree()` and detached
`Item`/expression reader, then replace them in one coherent public switch. The
private attachment added here is the accepted source for that later migration,
not a second public contract.

## Verification

All commands ran from the repository root:

- `cargo check -p arcweft-lang-syntax --all-features`: passed;
- `cargo test -p arcweft-lang-syntax --all-features`: passed, including 386
  unit tests, every integration and compile-fail suite, and 3 doc tests;
- `cargo clippy -p arcweft-lang-syntax --all-targets --all-features -- -D warnings`:
  passed;
- `cargo fmt --all -- --check`: passed after formatting;
- `git diff --check`: passed; and
- `cargo +nightly -Zscript tools/structure-audit.rs --root .`: scanned 3,421
  files, 1,780 Rust files, 818,166 Rust physical lines, and 93 manifests;
  reported 0 errors and 131 existing warnings.

Tier 2 is not required for this private syntax-only cut. It changes no public
contract and reaches no runtime, render, Agent, MCP, or capture path. The
subsequent public syntax/HIR switch will qualify as a broad integration cut and
must run `just test-tier2`.

## Structural audit

The current `arcweft-lang-syntax` dependency graph has 14 incoming and 8
outgoing normal/development edges. No manifest, dependency, feature, or crate
boundary changed in this cut.

| Changed Rust file | Bytes | Physical LOC | Classification | Embedded test LOC | Responsibility |
| --- | ---: | ---: | --- | ---: | --- |
| `src/attachment.rs` | 11,723 | 376 | production | 93 | attachment construction, typed handle boundary, invariant tests |
| `src/attachment/error.rs` | 2,256 | 60 | production | 0 | exact lookup and attachment failures |
| `src/attachment/snapshot.rs` | 8,974 | 329 | production | 0 | qualified IDs, immutable snapshots, handles |
| `src/grammar/budget.rs` | 19,392 | 559 | production | 71 | grammar budget classification |
| `src/grammar/build.rs` | 20,200 | 563 | production | 129 | event paths and unattached grammar build |
| `src/grammar/kinds.rs` | 15,345 | 574 | production | 35 | current kinds, semantic roles, role classes |
| `src/incremental.rs` | 321 | 13 | facade | 0 | incremental responsibility modules |
| `src/incremental/database.rs` | 16,226 | 500 | production | 0 | public transaction integration and committed lineage |
| `src/incremental/database_tests.rs` | 43,469 | 1,316 | unit test | 0 | identity, reconciliation, and rollback evidence |
| `src/incremental/reconcile.rs` | 28,448 | 829 | production | 141 | old CST and private grammar reconciliation |
| `src/incremental/shape.rs` | 18,901 | 597 | production | 33 | semantic CST and role-aware grammar shapes |
| `src/incremental/transaction.rs` | 6,656 | 215 | production | 0 | shadow staging and allocator commit |
| `src/lib.rs` | 515 | 24 | facade | 0 | crate-private attachment module registration |
| `src/parser.rs` | 25,741 | 774 | production | 0 | private shadow parser exposure |
| `src/parser/document.rs` | 27,568 | 852 | production | 0 | one-pass document grammar entry point |
| `src/parser/retained_grammar_tests.rs` | 10,692 | 300 | unit test | 0 | reduced private inventory recovery evidence |

No changed production file crosses the 1,200-line warning threshold. The
largest changed file is the 1,316-line dedicated unit-test module, below the
2,500-line integration-test warning threshold. The five largest non-generated
workspace production Rust files at this audit were:

| File | Bytes | Physical LOC |
| --- | ---: | ---: |
| `crates/arcweft-lang-sema/src/checker/module.rs` | 93,423 | 2,482 |
| `crates/arcweft-core/src/engine/eval/calls.rs` | 89,488 | 2,481 |
| `crates/arcweft-core/src/value.rs` | 83,366 | 2,465 |
| `crates/arcweft-cli/src/toolchain_profile.rs` | 75,712 | 2,463 |
| `crates/arcweft-bundle/src/container.rs` | 78,366 | 2,393 |

None is changed by this cut. The new attachment and transaction responsibilities
are separate modules rather than additions to the incremental facade or parser
root.
