# Final architecture and production invariants

## 1. Decision summary

Cut 01.1.1 replaces three transitional representations together rather than layering another adapter over them:

1. `Root -> Line -> token` becomes a grammar-level, byte-lossless Rowan tree whose semantic nodes carry stable, reconciled identities.
2. detached cloned Rust syntax values become snapshot-owned typed handles attached to the exact Rowan nodes.
3. `Vec`-owned syntax-as-HIR and linked modules become immutable, module-qualified HIR snapshots owned by `HirDatabase`.

The runtime boundary is closed in the same cut: runtime-capable assertion conditions receive a persisted guard key and a separate session-only inventory that maps the guard to the exact `StmtId`, condition index, mode, and revision-bound `SourceSpan`. `arcweft-core` remains runtime/data-only and never depends on syntax or HIR.

## 2. End-to-end ownership

```text
SourceDocument (arcweft-source; exact bytes and provenance)
    |
    v
SyntaxDatabase transaction (arcweft-lang-syntax)
    lex -> grammar events -> lossless Rowan green tree
    -> grammar-node reconciliation -> SyntaxNodeId allocation
    -> typed attachment index -> immutable ParsedSource
    |
    v
HirDatabase transaction (arcweft-lang-hir)
    attached typed handles -> direct lowering
    -> source/synthetic allocation keys -> typed arenas
    -> scopes/locals/captures -> immutable HirModule snapshot
    |
    v
HirProjectView + ProjectSymbolTable
    module-preserving registration, resolution, sema, verification
    |
    v
runtime-plan
    executable guards + persisted core payloads
    + session-only RuntimeAssertionInventory
    |
    +--> arcweft-core / AWBC / bundle / save / checkpoint
    |      persisted runtime data; no SyntaxNodeId, StmtId, HirSnapshotId
    |
    +--> CLI / LSP / Agent / debug projection
           runtime.assertion_failed + exact fresh-session source evidence
```

Only the syntax and HIR database transactions allocate session identities. Only their successful commits publish a new snapshot.

## 3. Fixed authority boundaries

### 3.1 Source

`SourceDocumentIdentity` remains exact content provenance. `SourceSnapshotId` remains source-name plus incremental generation. They are not merged. Syntax adds a session-only database/lineage wrapper solely to prevent cross-database resolution; it does not replace either source authority.

Every `SourceSpan` is created by the exact `SourceDocument` revision. Syntax and HIR never synthesize a span by combining a source label and integers.

### 3.2 Syntax

`arcweft-lang-syntax` owns lexing, grammar events, lossless tree construction, parser recovery, syntax limits, identity reconciliation, and typed attachment. It does not own name resolution, proof discharge, HIR arenas, semantic purity, or runtime behavior.

There is one source parse. Nested expression/type/pattern/statement parsers consume the same full-source token cursor and event sink. No source-backed body, signature, clause, expression, pattern, or type is reparsed from a string.

### 3.3 HIR

`arcweft-lang-hir` owns HIR database identity, typed IDs, slot liveness, immutable arenas, lexical scope/local/capture records, direct lowering, lowering transactions, module snapshots, project views, and source/synthetic allocation keys.

HIR stores semantic names, literal values, operators, `BorrowKind`, `AssertionMode`, and typed child IDs. It stores neither Rowan nodes nor typed syntax handles nor authoritative source strings.

### 3.4 Symbols and semantics

The existing `ProjectSymbolTable` remains the only project declaration authority. Functions, predicates, proofs, and Character declarations are registered in one revision-bound table. No callable-only or proof-only symbol table is introduced.

Name resolution, context restrictions, type checking, purity, recursion rejection, and executable readiness remain semantic responsibilities. Proof discharge and solver behavior remain later cuts.

### 3.5 Runtime

`arcweft-core` remains Sans I/O runtime/data core. It may serialize a typed assertion guard key and existing materialized condition/message/profile data. It may not import `arcweft-lang-hir`, `arcweft-lang-syntax`, compiler, LSP, filesystem, network, or process APIs.

Session identities and revision-bound spans remain in `arcweft-runtime-plan` and presentation/session layers. Persisted data can be associated with a fresh session only by an exact runtime artifact fingerprint match.

## 4. Non-negotiable invariants

1. No source-backed HIR ID is keyed by a line node after the public switch.
2. Tokens never receive `SyntaxNodeId`; semantic grammar nodes do.
3. Repeated identical grammar descendants on one physical line receive distinct IDs.
4. A `SyntaxNodeId`, typed AST handle, or HIR ID cannot be constructed from a public integer.
5. Session IDs implement no `Serialize` or `Deserialize`.
6. Every typed handle owns its immutable syntax snapshot through `Arc`; clone is cheap and cannot detach identity.
7. Trivia-only and accepted reconciled edits may retain IDs while ranges change in the new immutable snapshot.
8. Cross-parent moves receive fresh syntax and source-backed HIR identities. Same-parent unique reorder retains identity. Copies allocate fresh identity for every additional copy.
9. Fatal syntax or HIR transaction failure consumes no generation, slot, tombstone, diagnostic state, local generation, cache epoch, or module identity.
10. Recovered snapshots are queryable but not executable and are ineligible for sema/codegen/runtime caches.
11. HIR slots are never reused. Old immutable snapshots keep resolving IDs that were live in their revision.
12. Project iteration never rebases, clones, or flattens module arenas.
13. `ProofArtifactId` is derived, session-only, and has no authored spelling or codec.
14. `AssertionMode::Prove` cannot construct a runtime guard or runtime fault.
15. Release runtime-plan output contains no Debug evaluation or Debug assertion inventory entry.
16. Removed syntax receives ordinary current-grammar recovery only; no permanent historical node, recognizer, diagnostic code, source scan, or spelling-specific implementation test survives.

## 5. Scope

This contract includes the grammar tree, typed attachment, final predicate/proof surface, proof body model, immutable HIR, scope/local/capture lowering, project/symbol migration, runtime assertion-fault identity, serialization boundary, caller deletion, direct tests, and implementation verification.

It does not define proof discharge, solver algorithms, Copy/Move analysis, borrow dataflow, runtime reference storage, runtime scheduling, checkpoint semantics, persistent HIR, or proof-concurrency cuts 2 through 11.

## 6. Rejected implementation shapes

The following shapes are incompatible with the contract and must not appear even temporarily as a supported public API:

- attaching several typed descendants to one `Line` ID;
- range or source-text search to attach typed syntax;
- traversal-order IDs reconstructed on demand;
- two source parses, substring fragment parsing for document children, or raw proof-body parsing;
- a typed-only tree that discards trivia or recovery tokens;
- cloned syntax enums inside HIR arenas;
- local extension traits or free conversion helpers that compensate for missing behavior on repository-owned enums;
- linked/flattened project HIR;
- a proof-only symbol table;
- `StmtId` or `HirSnapshotId` in core/AWBC/bundle/save/checkpoint codecs;
- message-text parsing to recover assertion identity; or
- compatibility aliases, deprecated fields, dual readers, and historical removed-syntax diagnostics.
