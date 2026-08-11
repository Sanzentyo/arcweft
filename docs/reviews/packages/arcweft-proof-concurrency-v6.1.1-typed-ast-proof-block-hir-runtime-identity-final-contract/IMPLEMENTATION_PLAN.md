# Required implementation order and safe compiling states

## 1. Global rule

Implementation follows the dependency order in this document. Each numbered stage ends in a compiling state. A stage may use crate-private temporary scaffolding, but no reviewable/push cut may expose mixed line/grammar HIR identity, two symbol authorities, linked plus arena HIR public contracts, or serialized session IDs.

The implementation branch begins from latest `main`, reads current `AGENTS.md` and the full Rust skill again, records exact Git/Jujutsu identity, and performs a fresh structural audit before editing.

## 2. Stage 0 — baseline and freeze

### Work

- run the focused current tests and canonical structure audit before changes;
- record exact file metrics, embedded-test LOC, crate dependency fan-in/fan-out, and current callers;
- add internal design constants/enums for final `SyntaxKind`, `SyntaxRole`, syntax/HIR limits, diagnostics, `SyntheticRole`, and runtime fault boundary;
- extend repository-owned enums through inherent implementations; do not add helper traits;
- add compile-fail test fixtures for the final raw-ID/Serde/fragment/project/runtime boundaries, initially marked to land with their owning API stage rather than ignored.

### Safe state

No public production behavior changes. Old parser/HIR remains the sole public source-backed path. Final enums/types may exist crate-private but have no duplicate public authority.

### Gate

Focused current suites pass; no user-visible temporary syntax recognizer exists.

## 3. Stage 1 — private grammar events and lossless tree

### Work

- split `cst.rs` into grammar kind/event/build modules;
- make lexer tokens cover exact bytes once;
- implement one full-source parser cursor/event sink for existing grammar families;
- emit grammar nodes, ID-less layout wrappers, missing tokens, and recovery nodes;
- validate event balance and token-byte losslessness;
- add grammar tree direct tests for every existing item/statement/expression/type/pattern family.

### Safe state

The new grammar tree is crate-private shadow output used only in tests. Public `ParsedSource` and HIR still use the old path. The shadow tree must not allocate production `SyntaxNodeId` or enter caches.

### Gate

For every accepted/recovered fixture, concatenated real token text equals source bytes and event validation passes. No second parse is added.

## 4. Stage 2 — private reconciliation and typed attachment

### Work

- add database/lineage/snapshot-scoped syntax IDs;
- port accepted reconciliation policy to identity-bearing grammar nodes and roles;
- implement attachment table, sealed typed handles, bidirectional node lookup, exact snapshot errors;
- integrate staging with `SyntaxDatabase` transaction without publishing it yet;
- migrate test builders to attached snapshots;
- add fatal rollback and two-database collision tests.

### Safe state

New grammar identities and typed handles remain crate-private. Old source-backed HIR remains the only public lowering path. No HIR may consume shadow identities.

### Gate

All lossless/attachment/reconciliation/atomic tests pass privately, including same-line repeated descendants and missing/error nodes.

## 5. Stage 3 — atomic syntax public switch

### Work

In one workspace-compiling change:

- change canonical `SyntaxDatabase::{parse_initial,reparse}` to publish grammar tree plus attached typed handles;
- change compiler/LSP/document facades to own databases and exact documents;
- replace detached `TypedSyntaxTree` values with `AstNode` wrappers;
- migrate expression/type/pattern/statement/item accessors;
- replace source-backed fragment APIs with `UnboundFragment`/explicit attachment;
- migrate formatter/tooling/tests;
- change every source-backed HIR key input to grammar `SyntaxNodeId` but do not yet publish final arena HIR;
- delete line event bridge, detached source-backed parse, and hidden line identity authority.

### Safe state

Only grammar-node syntax identity is public. The old HIR representation may still exist internally for a short compiling stage, but every source-backed HIR value is keyed only from grammar node identity. No line ID remains in HIR keys.

### Gate

Workspace compiles and all syntax consumers use bound handles. Compile-fail proves unbound fragments cannot enter source-backed HIR lowering.

## 6. Stage 4 — final predicate/proof surface and `ProofBlock`

### Work

- implement ordinary-name predicate/proof grammar, exact limits, recovery, clauses, expression/block bodies;
- add exact `ProofBlock`/predicate block typed wrappers and shared typed descendants;
- add semantic context hooks for tail, assertions, pure lets, proof calls, mutable bindings, result name, and recursion;
- migrate formatter/LSP syntax views;
- delete provisional proof/trusted parser/types/body strings/authored IDs/old clauses and historical docs/fixtures.

### Safe state

The public language has only the final predicate/proof surface. Removed forms receive ordinary recovery. HIR may lower final typed nodes through an internal bridge until Stage 5, but no raw body/signature/clause string is authoritative.

### Gate

Complete grammar/recovery/limit matrix passes. Direct API tests prove no executable removed node.

## 7. Stage 5 — private `HirDatabase`, arenas, scopes, locals, captures

### Work

- add database-qualified HIR identity, module key, immutable module snapshot, paged arenas, slot metadata, liveness, source/synthetic indices;
- add direct item/expr/stmt/type/pattern lowering from typed handles;
- add lexical scopes, pre-binding locals, shadow generations, captures, control-flow scope rules, postcondition result;
- add private staging transaction, diagnostics ordering, invalidations, all limits/exhaustion hooks;
- port existing reference/assertion data through owned enums/IDs;
- add old/current snapshot resolver and atomic tests.

### Safe state

Final arena HIR is crate-private and exercised by tests. Old public HIR remains the only public contract until all direct callers are prepared. No public dual model.

### Gate

Arena/liveness/scope/local/capture/direct-lowering test matrix passes privately. The behavior-string disagreement test proves typed-child authority.

## 8. Stage 6 — atomic HIR and project public switch

### Work

In one workspace-compiling change:

- make `HirDatabase::lower` and immutable `HirModule` the public HIR API;
- migrate `ProjectSymbolTable`, sema, verifier, compiler, runtime-plan preparation, tooling, CLI, and LSP to typed resolvers;
- introduce checked `HirProjectModule`, `HirProject`, and borrowed `HirProjectView`;
- migrate exported-part/style aggregation and all project iteration;
- delete clone model, detached lowering, syntax-as-HIR fields, `linked_module`, `append_module_body`, compiler `linked_hir`, rebasing helpers, and all callers/fixtures;
- remove wholesale syntax re-exports from HIR.

### Safe state

Only arena HIR and module-preserving project APIs are public. There is one symbol table. Module IDs remain unchanged through every consumer.

### Gate

Workspace compiles with no linked/append APIs. Project/symbol/sema/verifier/compiler direct and compile-fail tests pass. Recovered modules are tooling-only.

## 9. Stage 7 — runtime assertion identity and persisted boundary

### Work

- add checked core guard/fingerprint data newtypes without compiler dependencies;
- version/update assertion payload codecs once;
- add runtime-plan assertion identity inventory and exact condition sites;
- derive guard keys from canonical typed seed;
- return inventory beside plan;
- carry failure guard through existing runtime failure surfacing;
- add fresh-session `ExecutionDiagnosticContext` in the layer that sees both compile metadata and runtime failures;
- migrate CLI/LSP/Agent/debug presentation;
- omit Debug sites/evaluation in release and make Prove conversion impossible;
- update bundle/AWBC/cache/save/checkpoint/replay codecs that transit assertion data.

### Safe state

Core/persisted data contains guard/fingerprint/core assertion data only. Session inventory is non-Serde and lives above core/runtime host. No dual assertion reader remains.

### Gate

Runtime assertion test/codec/dependency matrix passes, including release omission, invalid condition index, reloaded association, and no core-HIR edge.

## 10. Stage 8 — final migration, deletion, docs, and structural closure

### Work

- migrate remaining CLI/LSP/tooling/formatter/cache/style/export/test callers;
- update stable language/runtime/architecture docs and examples;
- delete every temporary recognizer, shim, duplicate builder, old fixture, compatibility export, and migration-only feature gate;
- run all direct compile-fail suites;
- run focused tests, workspace check, Clippy with `-D warnings`, format, `just verify`, dependency evidence, diff checks, and fresh structural audit;
- check in the implementation note and audit report with exact commands/results;
- inspect final diff for dependency direction and generated output exclusions.

### Safe state

This is the only push-ready completion state for cut 01.1.1. It contains no provisional public path and no historical-syntax production memory.

### Gate

Every command in `VERIFICATION_PLAN.md` required for the implementation risk area passes. `OPEN_RESULT_CHANGING_DECISIONS` remains zero. Only then may cut 2 be dispatched.

## 11. Forbidden intermediate states

The implementation must stop and restructure rather than review/push any state with:

- some source-backed HIR IDs keyed by line and others by grammar node;
- a public detached typed tree beside attached handles;
- source-backed lowering from unbound fragments;
- both provisional and final proof item types;
- both clone/Vec HIR and arena HIR as public contracts;
- both linked HIR and module-preserving project as supported APIs;
- a proof-only or callable-only second symbol table;
- core/runtime host normal dependencies on HIR/syntax/runtime-plan;
- serialized `SyntaxNodeId`, `StmtId`, `HirSnapshotId`, `ProofArtifactId`, inventory, or fault identity;
- temporary removed-syntax recognizers in the final diff;
- ignored compile-fail tests or source-scanning implementation assertions.

## 12. Review decomposition

Implementation commits may be internally decomposed along stages, but the public switch stages (3, 6, and 7) must each be workspace-compiling and self-contained. A review stack can keep private preparatory commits, then expose the public cut only when all direct callers in that boundary migrate.

No implementation commit modifies unrelated runtime scheduling, proof solving, Copy/Move analysis, borrow dataflow, resource/effect kernels, or checkpoint semantics.
