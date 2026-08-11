# Migration and deletion inventory

## 1. Completion rule

The migration is complete only when the final public workspace has one grammar-level syntax identity path, one attached typed syntax model, one arena HIR model, one module-preserving project model, one symbol table, and one runtime assertion identity boundary. A compatibility wrapper that delegates to an old model still violates completion.

The inventory below is based on latest inspected `main` Git `76d39983ad8770a87d6e81745785b6b362a381b4`. Implementation must re-run symbol/reference discovery on its checkout and add any newly introduced caller to the same migration; it must not preserve an obsolete caller because it was absent from this snapshot.

## 2. Syntax tree and parser inventory

### 2.1 Files to split or migrate

- `crates/arcweft-lang-syntax/src/cst.rs`
- `crates/arcweft-lang-syntax/src/cst/lexer.rs`
- `crates/arcweft-lang-syntax/src/cst/line.rs`
- `crates/arcweft-lang-syntax/src/cst/classify.rs`
- `crates/arcweft-lang-syntax/src/cst/punctuation.rs`
- `crates/arcweft-lang-syntax/src/parser.rs`
- `crates/arcweft-lang-syntax/src/parser/source.rs`
- `crates/arcweft-lang-syntax/src/parser/fragment.rs`
- `crates/arcweft-lang-syntax/src/parser/items.rs`
- `crates/arcweft-lang-syntax/src/parser/headers.rs`
- `crates/arcweft-lang-syntax/src/parser/statements.rs`
- `crates/arcweft-lang-syntax/src/parser/control_flow.rs`
- `crates/arcweft-lang-syntax/src/parser/helpers.rs`
- `crates/arcweft-lang-syntax/src/parser/proof.rs`
- `crates/arcweft-lang-syntax/src/ast/items.rs`
- `crates/arcweft-lang-syntax/src/ast/flow.rs`
- `crates/arcweft-lang-syntax/src/ast/proof.rs`
- `crates/arcweft-lang-syntax/src/ast/common.rs`
- `crates/arcweft-lang-syntax/src/ast/pattern.rs` and pattern submodules
- `crates/arcweft-lang-syntax/src/expr.rs`
- `crates/arcweft-lang-syntax/src/expr/source_ranges.rs`
- `crates/arcweft-lang-syntax/src/types.rs`
- `crates/arcweft-lang-syntax/src/source.rs`
- `crates/arcweft-lang-syntax/src/incremental.rs`
- `crates/arcweft-lang-syntax/src/incremental/reconcile.rs`
- `crates/arcweft-lang-syntax/src/incremental/shape.rs`
- `crates/arcweft-lang-syntax/src/incremental/limits.rs`
- `crates/arcweft-lang-syntax/src/lib.rs`

### 2.2 Symbols to replace

- coarse `cst::SyntaxKind::{Root, Line, Error, ...}` with final grammar/token inventory;
- `parse_cst` line builder with event-driven grammar tree builder;
- `CstLine`, `CstLineEvents`, `cst_lines`, and `cst_lines_for_source` as typed parser input;
- line-only semantic-parent derivation in incremental shape/reconciliation;
- detached `TypedSyntaxTree` fields containing owned `String`/`Vec` syntax values;
- detached source `ParsedSource` that separately owns root and typed tree;
- `parse_source(source: &str)`/`parse_document` source-backed facades that invent no caller-owned lineage;
- substring/body callback parsers that accept authoritative `&str` for document descendants;
- `ParsedFragmentKind` as a source-backed lowering input;
- range/source-search helpers in `expr/source_ranges.rs` that exist only because nested typed nodes lack grammar identity.

### 2.3 Required additions

- `grammar.rs` and `grammar/{event,build,kinds}.rs`;
- `attachment.rs` and `attachment/{snapshot,error}.rs`;
- split incremental database/transaction modules;
- final predicate/proof parser and typed AST module;
- bound document parse and explicit unbound/attached fragment types;
- direct round-trip APIs between snapshot-owned Rowan and typed handles.

## 3. Predicate/proof deletion inventory

Delete, not alias:

- provisional `ast::proof::ProofItem` fields `id: IdRef`, `body: String`, and old clauses;
- old `ProofClause` enum and every variant;
- `TrustedAxiomItem` and `Item::TrustedAxiom`;
- parser branches for `proof @...` and `trusted axiom`;
- authored proof artifact/entity IDs and textual codecs;
- line/body string parsing in `parser/proof.rs`;
- old proof clause recognition, `calc` node/recognizer if present, and any removed-syntax diagnostic branch;
- raw proof body, signature, clause, type, pattern, or expression payload used as lowering authority;
- fixtures and examples that present provisional syntax as current language.

Retain and reuse:

- shared outer attributes/docs;
- current `Visibility`;
- generic, parameter, pattern, type, where, expression, and statement parsers after migration to one event sink;
- existing `AssertionMode` and typed `AssertionStmt`;
- current `BorrowKind`, reference types, borrow/dereference expressions, and semantics.

## 4. HIR inventory

### 4.1 Files to replace/split

- `crates/arcweft-lang-hir/src/identity.rs`
- `crates/arcweft-lang-hir/src/id_context.rs`
- `crates/arcweft-lang-hir/src/model.rs`
- `crates/arcweft-lang-hir/src/lower.rs`
- `crates/arcweft-lang-hir/src/project.rs`
- `crates/arcweft-lang-hir/src/lib.rs`
- `crates/arcweft-lang-hir/src/symbol/identity.rs`
- `crates/arcweft-lang-hir/src/symbol/table.rs`
- HIR tests and trybuild/public API fixtures.

### 4.2 Symbols to delete

- `HirModule` fields that own `Vec` collections of cloned syntax values;
- HIR payload fields that store syntax AST enums or authoritative source strings;
- detached `lower_source_document`/lowering entrypoints that accept `TypedSyntaxTree` without bound source snapshot;
- `HirProject::linked_module`;
- `HirModule::append_module_body`;
- ID rebasing/appending helpers;
- panic-based project package/path mutation;
- any raw ID constructor/Serde implementation introduced during migration;
- old assertion Rust shapes that clone line-plan/source strings rather than carrying the existing typed mode and `ExprId`s.

### 4.3 Required additions

- `HirDatabase`, checked module key, lowering request, module state, private transaction;
- database-qualified module identity and immutable snapshot retention;
- private paged typed arenas and slot ledgers;
- source/synthetic allocation indices and liveness;
- direct typed item/expr/stmt/type/pattern lowering;
- scope/local/capture arenas and resolvers;
- module-preserving project/view types;
- final source provenance and cache invalidation set.

## 5. Compiler and project caller inventory

At the inspected head, `crates/arcweft-compiler/src/project.rs` constructs `HirProjectModule`, calls `linked_module`, and retains `CompiledProject::linked_hir`. Migrate:

- project source loading and `SourceDocument` creation;
- compiler-owned `SyntaxDatabase` and `HirDatabase` session state;
- `compile_module` parse/lower facades;
- `compile_project_with_cache` module assembly;
- readiness/resolution/type-check invocation;
- runtime-plan and line-task lowering;
- exported-part/style aggregation;
- source maps and diagnostics;
- project cache keys;
- compiler tests/fixtures/trybuild suites.

Related compiler files include:

- `crates/arcweft-compiler/src/project.rs`;
- `crates/arcweft-compiler/src/hir.rs` and lowering helpers;
- `crates/arcweft-compiler/src/style.rs`;
- compiler cache modules;
- `crates/arcweft-compiler/src/tests.rs` and integration tests.

Final `CompiledProject` stores `HirProject`, `ProjectSymbolTable`, module-qualified semantic results, runtime plan, and assertion inventory. It has no linked module field.

## 6. Sema and symbol caller inventory

Migrate project/module iteration and syntax-clone assumptions in:

- `crates/arcweft-lang-sema/src/checker/module.rs`;
- `crates/arcweft-lang-sema/src/checker/expr.rs`;
- `crates/arcweft-lang-sema/src/checker/stmt.rs`;
- `crates/arcweft-lang-sema/src/checker/helpers.rs`;
- declaration/source registration modules;
- callable lookup/import/alias logic;
- semantic cache keys and readiness gates;
- sema tests and public API compile-fail tests.

Extend `CallableDeclarationOwner` and `ProjectSymbolTable` inherent implementations directly. Do not introduce `CallableSymbolTable`, `ProofSymbolTable`, or local extension traits.

Add predicate/proof context checks, no-recursive-contract SCC validation, clause/tail typing, proof-call kind validation, and recovered-module cache exclusion.

## 7. Verifier inventory

Migrate:

- `arcweft-verify` project/item traversal from linked HIR to `HirProjectView`;
- proof/predicate declaration discovery to `HirItemKind::{Predicate, Proof}`;
- source labels to HIR slot metadata spans;
- proof artifact references to session-only derived `ProofArtifactId`;
- verifier cache keys to module/project snapshot identities;
- verify-LSP adapters and tests.

Do not implement proof discharge or solver behavior in this cut. Existing verifier entrypoints may report unsupported/not-yet-discharged after consuming the final identity model.

## 8. Runtime-plan inventory

Migrate/split:

- `crates/arcweft-runtime-plan/src/assertion.rs`;
- `crates/arcweft-runtime-plan/src/expr.rs`;
- `crates/arcweft-runtime-plan/src/expr/effect.rs`;
- `crates/arcweft-runtime-plan/src/flow.rs`;
- `crates/arcweft-runtime-plan/src/host_request.rs`;
- AWBC/runtime-plan product lowering modules;
- line-task/runtime-plan project entrypoints;
- runtime-plan tests.

Required changes:

- consume module-qualified `HirProjectView` and typed HIR resolvers;
- carry existing assertion modes/condition `ExprId`s rather than line/source strings;
- add `assertion_identity.rs` and session inventory;
- derive guard keys from typed canonical seed;
- omit Debug guards/sites in release;
- make Prove conversion impossible;
- return assertion inventory beside the runtime plan.

Do not add HIR dependencies to core or runtime host production code.

## 9. Core, AWBC, bundle, runtime, save/checkpoint/replay inventory

### 9.1 Core/AWBC

Migrate:

- `crates/arcweft-core/src/effect.rs` `RuntimeAssertion` to carry checked guard bytes while retaining condition/message/profile;
- AWBC product/effect mapping that constructs/encodes runtime assertions;
- core and AWBC codec tests;
- artifact fingerprint exposure/reuse.

Do not add syntax/HIR imports or session IDs.

### 9.2 Bundle and persisted formats

Update the single current format version for:

- bundle runtime-plan/AWBC assertion payloads;
- cache serialization;
- debug trace assertion failure records when present;
- save/checkpoint/replay records that transit runtime assertion data.

Delete prior reader/writer shapes in the same format cut; no dual reader remains. Assert structurally that decoded values contain guard/fingerprint/core data but no HIR/session IDs.

### 9.3 Runtime host/driver

Migrate runtime failure surfacing to return `RuntimeAssertionFailure` core data. Keep runtime host normal dependencies free of runtime-plan/HIR/syntax. Fresh-session projection happens in a higher session/presentation owner holding `ExecutionDiagnosticContext`.

## 10. CLI, LSP, Agent, tooling, formatter, and caches

### 10.1 CLI

Migrate check/run/profile/debug/project commands to:

- bound syntax/HIR/compiler session APIs;
- `HirProjectView` iteration;
- module-qualified IDs in reports;
- runtime assertion projection through `ExecutionDiagnosticContext`;
- stable `runtime.assertion_failed` diagnostics;
- no linked HIR accessor.

Likely owners include project command, runtime/profile output, diagnostics, and application/project modules under `crates/arcweft-cli/src`.

### 10.2 LSP

Migrate `crates/arcweft-lsp` and `arcweft-verify-lsp`:

- document state owns/uses a `SyntaxDatabase` lineage;
- incremental edits consume bound `ParsedSource`;
- hover/definition/references/rename/semantic tokens/document symbols use typed handles/HIR resolvers;
- diagnostics use exact spans from immutable snapshots;
- project-wide lookup uses `HirProjectView` and one symbol table;
- runtime fault presentation uses fresh session inventory when available;
- stale/wrong snapshot errors are handled explicitly.

### 10.3 Agent/tooling

Migrate `arcweft-agent-repl`, `arcweft-tooling`, and debug-model adapters that inspect compiler/HIR data. Preserve module qualification in summaries and proof identities. Do not serialize session IDs in agent/debug payloads.

### 10.4 Formatter

The formatter consumes the grammar-level lossless tree and typed handles. It must preserve recovery/missing boundaries and exact comments/trivia. It cannot use the removed line hierarchy as semantic identity. Formatting tests compare public parse/format/reparse outcomes, not implementation source text.

### 10.5 Caches

Migrate all cache keys:

- syntax cache: `SyntaxSnapshotId` plus exact source identity in-memory only;
- HIR module cache: `HirSnapshotId` in-memory only;
- project semantic cache: ordered module snapshot set plus `ProjectSymbolRevision`;
- persisted compiler/runtime caches: stable source/artifact digests and canonical declaration data only, never syntax/HIR session IDs;
- recovered snapshots never populate executable caches.

## 11. Documentation and examples

Update stable language/runtime documentation and examples, including:

- `docs/01-language/grammar.md`;
- `docs/01-language/proofs-and-unsafe-audits.md`;
- assertion/reference sections affected by identity presentation;
- compiler/HIR architecture documentation;
- LSP/tooling examples;
- runtime failure/debug documentation;
- source examples and test fixtures.

Current docs must show ordinary-name `predicate`/`proof`, one fixed parameter group, final clauses/bodies, and no trusted axiom/authored proof ID/calc/borrow-block path.

Documentation migration is content work, not a source-scanning enforcement mechanism. Tests must exercise APIs/behavior.

## 12. Ordered deletion gates

### Gate A: grammar switch

After all syntax consumers compile on bound typed handles, delete:

- flat line tree as parser authority;
- line event bridge;
- detached source-backed typed tree;
- source-backed substring parsing;
- hidden line semantic-parent identity.

### Gate B: proof surface

After final predicate/proof typed/HIR tests compile, delete provisional proof/trusted types, parser branches, raw body strings, old clauses, authored IDs, docs, and fixtures.

### Gate C: HIR switch

After every HIR/sema/compiler/tooling caller uses arenas, delete clone model, detached lowering, syntax fields in HIR, old source-range helpers, and compatibility exports.

### Gate D: project switch

After every project consumer accepts `HirProjectView`, delete `linked_module`, `append_module_body`, `linked_hir`, ID rebasing, and all flattened fixtures.

### Gate E: runtime boundary

After all codecs/runtime/projectors consume guard/fingerprint data, delete old assertion payload constructors and presentation paths that identify failures by message/source string.

### Gate F: final cleanup

Delete temporary implementation-only recognizers, shims, feature gates, duplicate tests, migration scripts, and aliases. Re-run direct compile-fail/dependency tests and structural audit.

## 13. Required absence evidence

Use compile-fail tests and direct public/crate-owned API tests to prove absence of:

- old proof/trusted types;
- authored proof artifact constructors/codecs;
- raw syntax/HIR/session ID constructors and Serde;
- detached fragments accepted by HIR lowering;
- linked/append HIR APIs;
- runtime `Prove` fault constructors;
- core-to-HIR/syntax dependencies.

Do not search checked-in source, paths, snippets, symbols, or documentation to prove implementation shape.
