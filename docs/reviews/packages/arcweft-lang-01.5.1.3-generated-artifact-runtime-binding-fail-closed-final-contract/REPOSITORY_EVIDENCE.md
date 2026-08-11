# Repository evidence

## Inspected authority

- Repository: `Sanzentyo/arcweft`
- Full Git commit: `0c8cb74dd96116a8b987cc419c9a280b6cabe4a4`
- Inspection date: 2026-08-08 (Asia/Tokyo)
- Access mode: immutable commit-addressed GitHub/raw source inspection. No repository working tree was created in the execution environment; therefore working-tree dirty/clean state is not applicable/observable. No repository file, branch, patch, PR, or production overlay was modified.

Current main was resolved to the commit above immediately before the design investigation. Every cited path below was inspected at that immutable commit.

## Policy read completely

- root `AGENTS.md`;
- `crates/AGENTS.md`;
- `docs/README.md` and applicable documentation instructions;
- `docs/implementation/AGENTS.md`;
- `docs/reviews/README.md` and `docs/reviews/AGENTS.md`;
- the complete attached Lang-01.5.1.3 request;
- the complete supplied Rust skill.

Key policy consequences applied here:

- design-only archive, no production edits;
- one final typed authority; no parallel model, fallback, dual reader, or compatibility wrapper;
- add missing behavior to Arcweft-owned types/inherent contexts;
- preserve layer direction and Sans-I/O crates;
- `READY_FOR_IMPLEMENTATION` only with every result-changing decision closed and `OPEN_QUESTIONS.md` exactly `none`;
- internal archive manifest, hashes, request copy, status, matrices, traceability, and verification.

## Current production evidence inspected

### Accepted metadata facts

`crates/arcweft-adapter-metadata/src/model.rs` retains:

- exact metadata format/schema and target ABI markers;
- `AdapterTarget::{Rust, Wasm, Process}` with triple/world/transport;
- package/module identities;
- artifact path/size/raw hash;
- complete function and Activity exports;
- metadata ABI and payload hashes.

### Import and selected topology facts

`crates/arcweft-manifest-model/src/schema.rs` retains `ExternalModuleImportSpec` with mount, metadata path/hash, expected package/version/module/family/ABI hash, visibility, and demand.

`crates/arcweft-project-loader/src/topology/model.rs` retains:

- `LoadedProfileTopology` with selected profile, admitted external modules, adapter, resources, and `SourceSetRevision`;
- `LoadedExternalModuleMetadata { import_id, import, document, metadata }`.

### Current projection gap

`crates/arcweft-project-loader/src/topology/external.rs` currently:

- extends the selected adapter separately from Activity validation;
- mounts non-private generated functions through ordinary `with_function_signature`;
- validates selected Activity module/export/Activity ID but does not project the retained `ResolvedActivityBinding::implementation_id()` into a runtime binding carrier;
- does not project a runtime binding requirement or preserve generated origin on the callable.

### Current runtime string boundary

`crates/arcweft-core/src/value.rs` currently has:

- `RuntimeCallTarget::Intrinsic` and `RuntimeCallTarget::Named(String)` only;
- `RuntimeCallTarget::from_label` and `as_label`;
- `RuntimeFunctionBody::{Expr, Awbc}` only.

`crates/arcweft-runtime-plan/src/typed_evidence.rs` currently retains callable names as strings in function-value/reference/partial/effect evidence, with no generated artifact ID.

### Compiler and revision carriers

`crates/arcweft-compiler/src/project/registration.rs` currently has `AcceptedLaunchProfileInput` with manifest, profile ID, resolved profile, source revision, and resource types, but no generated binding product. Its enclosing `ProjectCompilationContext` already models the whole accepted launch input as `Option<AcceptedLaunchProfileInput>` and initializes it to `None`; this is the authority for no-profile compilation and is why this contract forbids a fabricated empty product.

`crates/arcweft-source/src/document.rs` defines strict typed `SourceDocumentIdentity`, `SourceRevision`, and canonical `SourceSetRevision` serde.

`crates/arcweft-lsp/src/profiles/state.rs` defines monotonic `AcceptedEnvironmentGeneration` and one `AcceptedProfileEnvironment` atomically grouping the compiled project, world, project snapshot, overlays, and caches.

### Host boundary

`crates/arcweft-runtime-host/src/activity_host.rs` defines `ActivityHostRegistry` for concrete instances keyed by `InteractionTarget`; it has no generated artifact metadata correlation.

`crates/arcweft-runtime-driver/src/lib.rs` explicitly owns no filesystem, clock, thread pool, GPU, audio device, window, or browser API. `crates/arcweft-runtime-host/src/lib.rs` is the host-side execution boundary.

### Maintained implementation note

`docs/implementation/2026-07-20-lang-01-5-1-single-manifest-decoder-wip.md` states that E-19 waits on Lang-01.5.1.3 and forbids binding by callable spelling, Activity spelling, mount, basename, or adapter profile; the implementation must consume the exact accepted metadata/artifact/export/revision key and fail before host work.

## Evidence-based design consequences

1. The complete required facts already exist and should be projected, not decoded again.
2. The exact product cannot live only in `arcweft-core` without reversing/bloating lower-layer dependencies; a dedicated Sans-I/O shared crate is the clean owner.
3. Runtime values/plans need only a foundational typed ID, while the full key stays in a compiled launch sidecar.
4. Generated origin must be added directly to `AdapterFunction` and propagated, because current string lowering otherwise destroys identity.
5. `ResolvedActivityBinding` already retains the selected `ActivityImplementationId`; the exact key/selection must carry it rather than reconstructing it from export spelling.
6. `SourceSetRevision` plus exact metadata document/raw/hash evidence closes stale overlay reuse without inventing a speculative broader topology digest.
7. LSP generation remains process-local ownership and should not pollute the serialized key.

## Materials not claimed as inspected

- No full local Git checkout or working-tree status was available.
- No unreferenced predecessor ZIP was attached to this request, and the request itself names no predecessor archive to ingest.
- No production build/tests were run because this is a design-only archive and no repository checkout/code changes were made.

These limits do not leave a result-changing design question open; all required decisions are closed from the immutable current source and request authority.
