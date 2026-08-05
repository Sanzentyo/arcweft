# Proof public-switch session ownership and publication decision

- Decision date: 2026-08-06
- Inspected Git revision: `77b28b493ce70b8bd0c4637a2c375c6f71553574`
- Working tree: dirty protected ordinary-Flow final-HIR WIP; none of the
  project-loader, compiler, LSP, or documentation files inspected for this
  decision were modified at inspection time
- Status: `DECISION_COMPLETE_IMPLEMENTATION_PENDING`

This note closes the remaining orchestration seam identified by the
[Proof final-HIR database-owned entry](2026-08-03-proof-final-hir-database-owned-entry.md)
and the
[Stage 3 deletion-driven authority switch](2026-07-25-proof-stage-3-deletion-driven-authority-switch.md).
It does not introduce or name a new public façade. Implementation must adapt
the existing owners and entry points directly.

## Decision

One project build uses one syntax session and one HIR session. The selected
syntax-session owner is created before source discovery and remains alive
through project publication:

- a non-LSP build keeps the `SyntaxDatabase` in the compiler-facing project
  compilation transaction;
- an LSP build uses the `SyntaxDatabase` owned by `DocumentStore`; the profile
  rebuild borrows that same database instead of creating a compiler-local one;
- `project-loader` never creates a second syntax database or invokes a detached
  complete-document parser. It borrows the selected database and retains the
  returned cheap-clone `incremental::ParsedSource` for each canonical module;
- the compiler consumes those exact `ParsedSource` values and owns the one
  `HirDatabase` used for every module in the candidate project; and
- `CompiledProject`, `AcceptedProjectSnapshot`, and
  `AcceptedProfileEnvironment` retain the same accepted `Arc<HirProject>`
  allocation. Cloning the `Arc` is permitted; cloning or rebuilding the
  project value is not.

The syntax database has one serialized writer. Loader discovery, compiler
lowering, and LSP edits may take successive mutable borrows, but they do not
parse concurrently through independent databases. Immutable `ParsedSource`
and accepted project leases may be shared after their owning transaction has
committed.

For LSP, that database owns both open-overlay lineages and unopened workspace
module lineages discovered for the profile. `DocumentStore`'s URI map may stay
limited to open documents; unopened module snapshots remain in the loaded
project inventory, not in a second syntax database.

`LoadedProject` remains the loader-owned source/topology result. Its module
inventory must retain the exact `ParsedSource` associated with each existing
`Arc<SourceDocument>` and `CanonicalModulePath`; it must not project that
snapshot back into a detached tree. This is an ownership change to the
existing result, not a parallel parsed-project model.

## Transaction and publication order

1. **Acquire exact documents.** The filesystem/profile adapter creates each
   existing `Arc<SourceDocument>` once. An editor overlay is rebound to the
   accepted logical `SourceDocumentIdentity` before it enters the syntax
   transaction.
2. **Commit syntax once.** New documents use `SyntaxDatabase::parse_initial`.
   An LSP FULL change is represented as one full-document `SourceEdit` passed
   to `SyntaxDatabase::reparse` with the current `ParsedSource`. A no-op keeps
   the exact snapshot; a failed or stale edit publishes no syntax generation.
3. **Read topology from the committed snapshot.** The module declaration and
   `use` inventory are read from `ParsedSource::tree()`. Both
   `project.rs::scan_source` and
   `topology/loader.rs::load_module_dependencies` consume this retained
   snapshot. Neither calls `parse_document_with_source`.
4. **Lower the same snapshot.** The compiler constructs the existing
   `HirModuleKey` and `LoweringRequest::try_new` from the canonical package,
   module, exact document identity, and retained `ParsedSource`, then uses the
   existing database-owned lowering transaction. There is no
   `lower_document_to_hir` fallback.
5. **Freeze one module-preserving project.** Each returned exact
   `Arc<HirModule>` is checked by the existing final
   `HirProjectModule::try_new` against the same `HirDatabase`. The complete
   canonical inventory is checked by final `HirProject::try_new`, then wrapped
   in `Arc` exactly once.
6. **Build the candidate from project views.** The sole
   `ProjectSymbolTable`, resolution, type checking, verification,
   runtime-plan, line-task, Style/View, and other semantic products consume
   the accepted module-preserving project view, or its executable view where
   clean modules are required. No pass receives a synthetic linked module.
7. **Assemble before publication.** `CompiledProject` retains the accepted
   project `Arc` and module-qualified products. LSP accepted-project
   construction validates source, module, world, and symbol identities against
   those compiler products; it does not call `linked_module`, lower source, or
   rerun project type checking locally.
8. **Publish once.** Existing `AcceptedProfileCandidate::try_new` pointer
   equality remains mandatory. Existing
   `LspProfileState::replace_accepted_with` performs the expected-current check
   and the single accepted-environment swap. Project/LSP-visible pending cache
   stores are finalized only after the complete candidate wins this
   publication transaction.

A failure in steps 1--7 leaves the previous accepted LSP environment visible.
Committed syntax/HIR snapshots may remain private session cache state, but no
failed candidate, partial semantic facts, or pending cache record becomes an
accepted project. If the document store has advanced while the semantic build
failed, semantic requests must report the existing typed stale/unavailable
result; they must not combine the new syntax snapshot with the old project.

## LSP ownership after publication

`DocumentSnapshot` must retain the exact current `ParsedSource` beside its URI,
version, document lease, and line index. Diagnostics and features borrow that
snapshot and the current `AcceptedProfileEnvironment`. They use the existing
request-stamp and accepted-source checks to prove that the document, module,
project allocation, environment generation, and symbol revision still agree.

The local parse/lower paths in LSP diagnostics, actions, cascade, hover,
inlay, character definition, entry-role presentation, and View metadata are
deleted. Syntax-only projection may use the current attached snapshot.
Semantic projection requires the exact accepted project generation and fails
closed on a mismatch; it never performs a local lower to make stale data look
current.

## Cache and persistence boundary

`SyntaxDatabaseId`, `SyntaxLineageId`, `SyntaxSnapshotId`, `SyntaxNodeId`,
`HirDatabaseId`, `HirSnapshotId`, and arena IDs are process/session-local.
They may key in-memory entries only while the owning databases remain alive.
An in-memory compile-unit hit is admissible only when it resolves to the exact
current module lease in the same `HirDatabase`; a fingerprint match from
another session cannot restore HIR IDs or `Arc<HirModule>` values.

Persistent compiler, bundle, save, checkpoint, replay, Agent, and debug data
stores stable source/artifact digests and canonical typed facts only. It never
serializes a syntax/HIR session ID, a `ParsedSource`, an arena ID, or an
accepted-project allocation identity. Reopening a project always reconstructs
the syntax/HIR session from exact source documents before stable cached facts
can be admitted.

## Deletion-driven implementation boundary

At the start of the local public-switch change, delete the obsolete production
entries and use the resulting compiler errors as the migration inventory:

- detached `source::ParsedSource`, `ast::items::TypedSyntaxTree`, the detached
  `ParsedSource::typed_tree()`, `parse_document_with_source`, and detached
  fragment dispatch;
- `lower_document_to_hir` and the clone-bearing old HIR/project model;
- `HirProject::linked_module`, `HirModule::append_module_body`, and
  `CompiledProject::linked_hir`; and
- legacy Dialogue carriers and success paths selected for direct replacement
  by the accepted attached Dialogue application contracts.

Repair callers only toward the ownership sequence above. Do not add a renamed
parser, attached-to-detached projection, linked-view helper, compatibility
wrapper, alias, fallback, source gate, or dual reader. The temporary red state
is local; no commit or push may publish both authorities or a missing consumer.

The first required caller repairs are the current duplicate owners:

- `crates/arcweft-project-loader/src/project.rs::scan_source` and
  `topology/loader.rs::load_module_dependencies`;
- `crates/arcweft-compiler/src/project.rs::{compile_project_with_cache,
  compile_project_units, compile_module}` and its compile-unit cache;
- `crates/arcweft-lsp/src/documents.rs::DocumentStore`;
- `crates/arcweft-lsp/src/profiles/accepted_project.rs::AcceptedProjectSnapshot::try_new`;
  and
- every LSP diagnostic/feature that currently parses or lowers locally.

## Required implementation evidence

Focused tests must prove:

- loader topology and compiler lowering observe the same
  `SyntaxDatabaseId`, `SyntaxSnapshotId`, and exact document lease;
- one LSP FULL edit reparses the existing lineage, and stale/failed edits leave
  syntax state unchanged;
- HIR lowering receives the retained `ParsedSource`, project construction
  retains exact current module leases, and all accepted holders satisfy
  `Arc::ptr_eq` for the project;
- a failed compile, failed accepted-project admission, or lost publication race
  leaves the old environment and its caches authoritative;
- no LSP feature lowers locally and no compiler product exposes linked HIR;
- cross-session in-memory HIR reuse is rejected, while stable persistent facts
  contain no session-only IDs; and
- recovered modules remain available to tooling but cannot enter executable
  views or executable caches.

The final coherent switch requires focused loader/compiler/LSP tests, public
API compile-fail tests for the deleted entries, workspace check, strict
Clippy, workspace tests, applicable Tier 2, and structural audit. None of those
commands was run for this documentation-only decision.

## Evidence inspected

- `crates/arcweft-lang-syntax/src/incremental/database.rs`;
- `crates/arcweft-project-loader/src/project.rs`;
- `crates/arcweft-project-loader/src/topology/loader.rs`;
- `crates/arcweft-lang-hir/src/{database.rs,lower.rs,final_lowering.rs,final_project.rs}`;
- `crates/arcweft-compiler/src/project.rs` and
  `crates/arcweft-compiler/src/project/cache_batch.rs`;
- `crates/arcweft-lsp/src/documents.rs`;
- `crates/arcweft-lsp/src/profiles/{accepted_project.rs,state.rs,environment.rs}`;
- the accepted Proof-concurrency v6.1.1 package's
  `MIGRATION_AND_DELETION.md`, `IMPLEMENTATION_PLAN.md`,
  `PROJECT_AND_SYMBOLS.md`, and `TEST_MATRIX.md`; and
- the implementation notes linked above.

Performed validation was a read-only owner, call-site, contract, and
dependency inspection. Rust compilation, tests, Clippy, Tier 2, and structural
audit were not run because this cut changes documentation only.
