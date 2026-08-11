# Repository evidence

## 1. Inspection boundary

The repository was inspected read-only through the configured GitHub connector at latest `main` Git `76d39983ad8770a87d6e81745785b6b362a381b4`. The supplied request and Rust skill were also read in full. No checkout was materialized in the archive, no production file was edited, and no Cargo/Just command was executed while preparing this design package.

The repository records the most recent validated production substrate for this cut as Git `5a36cd0af83085179c299ef50ec8aa786ed731aa` / Jujutsu `nowqxzku`. The latest `main` adds design/dispatch material after that production substrate. The GitHub connector does not expose the local `.jj` operation store, so this package records the exact repository-published Jujutsu value rather than inventing a longer spelling.

## 2. Confirmed current syntax representation

At the inspected head:

- `crates/arcweft-lang-syntax/src/cst.rs` exposes only `Root`, `Line`, `Error`, and coarse token kinds.
- `parse_cst` opens a `Line` node for non-newline tokens and closes it on a newline; the tree is `Root -> Line -> token`.
- `parser::parse_source_with_options` first builds the CST and then separately builds `TypedSyntaxTree` from `CstLineEvents` and nested parsers.
- `parser/fragment.rs` exposes detached expression/statement/item fragments.
- `source.rs::ParsedSource` owns a source string, Rowan root, detached typed tree, errors, and statistics.
- incremental `ParsedSource` separately owns `SourceSnapshotId`, the exact `SourceDocument`, identity map, status, and the detached parse.
- `SyntaxIdentityMap` maps Rowan `SyntaxNode` values to a private-raw `SyntaxNodeId`; the current grammar only gives useful distinct identities to roots and lines.
- `incremental/shape.rs` derives brace/indent semantic parents over the flat line tree solely for reconciliation.

These facts require a grammar-level tree. An attachment table layered over unchanged line nodes cannot meet same-line descendant identity.

## 3. Confirmed source and incremental substrate

At the inspected head:

- `arcweft-source` owns `SourceDocument`, `SourceDocumentIdentity`, `SourceSpan`, `SourceGeneration`, and `SourceSnapshotId`.
- `SourceDocumentIdentity` includes exact document identity and content digest; `SourceSpan` is validated by the exact document.
- `SourceSnapshotId` contains a source name and nonzero generation. It does not contain database identity.
- `SyntaxDatabase` commits initial parses and reparses atomically, returns the same `Arc` on no-op edits, increments generation only for successful byte changes, rejects stale and cross-database snapshots, and never reuses syntax slots.
- accepted reconciliation behavior is implemented by unique full-shape matching, same-parent stable sequence matching, and deterministic distance/old-ID ties.
- current syntax transaction limits include prefix depth 64, assertion conditions 64, top-level items 16,384, statements 65,536, expressions 262,144, type nodes 131,072, pattern nodes 131,072, and diagnostics 1,024.

The final design preserves those observable transaction and reconciliation properties while moving them from line nodes to identity-bearing grammar nodes.

## 4. Confirmed predicate/proof and assertion starting point

At the inspected head:

- `ast/proof.rs::ProofItem` owns an authored entity-style ID, a raw body string, old `ProofClause` variants, and a range.
- `parser/proof.rs` recognizes `proof @...` and `trusted axiom`, then parses body lines into provisional clauses.
- `TypedSyntaxTree::Item` contains provisional `Proof` and `TrustedAxiom` variants.
- stable language documentation still describes those provisional forms.
- `AssertionMode::{Prove, Check, Debug}` and typed `AssertionStmt` with structured expression conditions are implemented in their owning syntax module.
- the removed ownership/borrow block is absent; reference types, prefix borrow/dereference, and `BorrowKind` are current authorities.

The provisional proof path is deletion inventory. The assertion and reference authorities are retained and carried by ID into HIR.

## 5. Confirmed HIR and project starting point

At the inspected head:

- `arcweft-lang-hir::identity` already defines private-raw module-qualified typed IDs, `HirSnapshotId`, `HirIdKind`, `LocalGeneration`, `SyntheticKey`, initial `SyntheticRole` values, and typed stale-ID errors.
- `model.rs` remains a `Vec`/clone model and stores syntax values.
- `lower.rs` accepts a detached `TypedSyntaxTree` and clones syntax into HIR.
- `project.rs` clones modules, mutates package/module ownership, exposes `linked_module`, and calls `append_module_body`.
- compiler `CompiledProject` retains both `hir_project` and `linked_hir`; resolution, readiness, type checking, style, line-task, and runtime-plan lowering still consume the flattened module.
- `ProjectSymbolTable` exists as the generalized project symbol authority.
- `CallableDeclarationOwner` already includes Function, Predicate, and Proof, but current registration primarily inserts functions.

The final contract extends these repository-owned enum/table implementations directly. It does not add helper traits or parallel tables.

## 6. Confirmed runtime boundary

At the inspected head:

- `arcweft-core::effect::RuntimeAssertion` is serialized runtime data with materialized condition, message, and profile.
- AWBC product mapping constructs that core payload.
- runtime-plan maps `Prove` to compile-time only, `Check` to an always-present runtime assertion, and `Debug` to a debug-only assertion omitted from release plans.
- `arcweft-runtime-plan` normally depends on HIR, source, and core.
- `arcweft-runtime-host` normally depends on core and runtime/data crates; its runtime-plan dependency is development-only.

Therefore session-only HIR identity belongs in runtime-plan/compiler presentation context, not core or persisted payloads.

## 7. Confirmed structural audit

The checked-in canonical audit `docs/implementation/structure-audits/proof-concurrency-surface-identity-2026-07-15` reports:

- 2,853 files;
- 1,408 Rust files;
- 661,972 physical Rust lines;
- 90 Cargo manifests;
- zero structural errors; and
- 128 structural warnings.

Important current hotspots include:

| File | Bytes | Physical LOC | Code LOC | Embedded tests |
|---|---:|---:|---:|---|
| `arcweft-lang-syntax/src/ast/items.rs` | 45,104 | 1,910 | 1,563 | audit flag: no |
| `arcweft-lang-syntax/src/expr/source_ranges.rs` | 51,746 | 1,488 | 1,388 | audit flag: yes |
| `arcweft-lang-syntax/src/expr.rs` | 32,507 | 1,230 | 1,043 | audit flag: yes |
| `arcweft-lang-syntax/src/incremental.rs` | 44,290 | 1,270 | 1,144 | 837 physical LOC in its contiguous test module |
| `arcweft-lang-syntax/src/parser/items.rs` | 54,632 | 1,560 | 1,498 | audit flag: no |
| `arcweft-lang-hir/src/model.rs` | 27,226 | 1,071 | 852 | audit flag: no |
| `arcweft-lang-hir/src/symbol/table.rs` | 39,565 | 1,099 | 1,013 | audit flag: yes |
| `arcweft-compiler/src/project.rs` | 33,246 | 1,024 | 918 | audit flag: yes |
| `arcweft-lang-sema/src/checker/module.rs` | 89,011 | 2,335 | 2,211 | audit flag: yes |
| `arcweft-lang-sema/src/checker/expr.rs` | 95,241 | 2,495 | 2,406 | audit flag: yes |
| `arcweft-runtime-plan/src/expr.rs` | 83,555 | 2,365 | 2,229 | audit flag: yes |
| `arcweft-runtime-plan/src/flow.rs` | 72,711 | 1,976 | 1,850 | audit flag: yes |

The checked-in audit emits only a boolean `has_embedded_tests`; it does not emit per-file embedded-test line counts. `STRUCTURE_PLAN.md` therefore records exact available metrics, the one directly countable contiguous test module, and requires the implementation audit to add an exact embedded-test-LOC column. No unavailable count is fabricated.

## 8. Dependency evidence

The canonical dependency edge CSV confirms the intended direction:

- syntax normal dependencies: dialogue, source, BLAKE3, Rowan, and error support;
- HIR normal dependencies: syntax, source, and error support;
- sema normal dependencies include HIR and syntax;
- runtime-plan normal dependencies include HIR, source, and core;
- compiler normal dependencies include syntax, HIR, sema, runtime-plan, source, project, core, and bundle layers;
- core has no syntax or HIR dependency;
- runtime-host has no normal syntax, HIR, compiler, or runtime-plan dependency.

`STRUCTURE_PLAN.md` freezes numeric fan-in/fan-out reporting for the implementation audit and names the affected direct consumers.

## 9. Validation honesty

No command listed in `VERIFICATION_PLAN.md` was run for this design-only package. The checked-in prior audit and implementation notes were inspected as evidence only. The implementation agent must produce fresh command output and a fresh checked-in structural audit for the implementation commit.
