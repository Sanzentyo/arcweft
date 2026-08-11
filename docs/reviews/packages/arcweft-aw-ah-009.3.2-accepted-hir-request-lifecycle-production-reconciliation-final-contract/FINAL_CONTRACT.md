# Final contract

## 1. Status and normative language

This contract is implementation-ready and has no result-changing open decision. The words **shall**, **must**, and **only** are normative. Types not explicitly marked public are crate-private. None of the request-lifecycle or accepted-generation carriers is serialized.

The selected architecture has exactly one accepted HIR carrier, one request acquisition route, one request cancellation owner, and one publication gate. It does not add a signature-specific parser, syntax database, `HirProject`, type-check environment, or fallback resolver.

## 2. Canonical accepted-generation carrier

### 2.1 Module ownership

The carrier belongs to `arcweft-lsp`:

```text
crates/arcweft-lsp/src/profiles/accepted_project.rs
```

It is an accepted LSP artifact retained by `AcceptedProfileEnvironment`. It is not a field of `RegisteredSemanticWorld`; sema registration remains independent of editor URI/version policy.

The exact typed keys are split by owner:

```rust
// crates/arcweft-lsp/src/uri_key.rs
use std::sync::Arc;

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct LspUriKey(Arc<str>);

impl LspUriKey {
    pub(crate) fn from_uri(uri: &lsp_types::Uri) -> Self;
    pub(crate) fn as_str(&self) -> &str;
}

// crates/arcweft-lsp/src/profiles/state.rs
use arcweft_launch::ProfileId;
use std::collections::BTreeMap;

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AcceptedProfileKey {
    workspace_uri: LspUriKey,
    manifest_uri: LspUriKey,
    profile_id: ProfileId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AcceptedOverlayEntry {
    version: i32,
    logical_identity: SourceDocumentIdentity,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct AcceptedOverlaySet {
    entries: BTreeMap<LspUriKey, AcceptedOverlayEntry>,
}

// crates/arcweft-lsp/src/profiles/accepted_project.rs
use arcweft_lang_hir::{model::HirModule, project::HirProject};
use arcweft_lang_syntax::ast::module_path::CanonicalModulePath;
use arcweft_source::SourceDocumentIdentity;
use std::{collections::BTreeMap, sync::Arc};

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct AcceptedModuleKey {
    module: CanonicalModulePath,
    source: SourceDocumentIdentity,
}

impl AcceptedModuleKey {
    pub(crate) fn module(&self) -> &CanonicalModulePath;
    pub(crate) fn source(&self) -> &SourceDocumentIdentity;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AcceptedProjectFootprint {
    documents: u64,
    modules: u64,
    source_bytes: u64,
}

impl AcceptedProjectFootprint {
    pub(crate) const fn documents(self) -> u64;
    pub(crate) const fn modules(self) -> u64;
    pub(crate) const fn source_bytes(self) -> u64;
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum AcceptedProjectLimitKind {
    Documents,
    Modules,
    SourceBytes,
}

impl AcceptedProjectLimitKind {
    pub(crate) const fn as_str(self) -> &'static str;
}

#[derive(Debug)]
pub(crate) struct AcceptedProjectSnapshot {
    hir: Arc<HirProject>,
    sources: AcceptedSourceDocuments,
    module_by_source: BTreeMap<SourceDocumentIdentity, CanonicalModulePath>,
    footprint: AcceptedProjectFootprint,
}
```

`LspUriKey` has exactly one constructor, `from_uri(&lsp_types::Uri)`. It stores the exact validated protocol spelling returned by `Uri::as_str()`. There is no `From<String>`, string parser, public tuple constructor, or path-derived URI key. The same key replaces every `BTreeMap<String, ...>` URI authority in `DocumentStore`, profile mapping, accepted source lookup, and overlay lookup. `as_str()` is transport/diagnostic projection only and is never accepted by a lookup API.

`AcceptedProfileKey` reuses the existing `arcweft_launch::ProfileId`; `ProfileId` receives `Hash` in its owning derive list. No LSP-local profile-ID wrapper or string parser is added. The API is exact:

```rust
impl AcceptedProfileKey {
    pub fn new(
        workspace_uri: &lsp_types::Uri,
        manifest_uri: &lsp_types::Uri,
        profile_id: arcweft_launch::ProfileId,
    ) -> Self;

    pub fn workspace_uri(&self) -> &str;
    pub fn manifest_uri(&self) -> &str;
    pub fn profile_id(&self) -> &arcweft_launch::ProfileId;

    pub(crate) fn workspace_key(&self) -> &LspUriKey;
    pub(crate) fn manifest_key(&self) -> &LspUriKey;
}
```

The string accessors are transport/diagnostic projections only. All internal maps and equality checks use `LspUriKey` and `ProfileId`.

`AcceptedOverlaySet` is constructed before candidate construction and owns duplicate rejection:

```rust
impl AcceptedOverlayEntry {
    pub(crate) fn new(
        version: i32,
        logical_identity: SourceDocumentIdentity,
    ) -> Self;
    pub(crate) fn version(&self) -> i32;
    pub(crate) fn logical_identity(&self) -> &SourceDocumentIdentity;
}

impl AcceptedOverlaySet {
    pub(crate) fn try_new(
        entries: impl IntoIterator<Item = (LspUriKey, AcceptedOverlayEntry)>,
    ) -> Result<Self, AcceptedOverlaySetError>;
    pub(crate) fn get(&self, uri: &LspUriKey) -> Option<&AcceptedOverlayEntry>;
    pub(crate) fn iter(
        &self,
    ) -> impl ExactSizeIterator<Item = (&LspUriKey, &AcceptedOverlayEntry)>;
}

#[derive(Clone, Debug, Eq, thiserror::Error, PartialEq)]
pub(crate) enum AcceptedOverlaySetError {
    #[error("accepted overlay set contains duplicate URI")]
    DuplicateUri { uri: LspUriKey },
}
```

`try_new` inserts entries one at a time and rejects the first repeated `LspUriKey`; collection never overwrites a duplicate. An entry has one exact open-document `i32` version and one logical `SourceDocumentIdentity`. Closed documents do not appear in the set.

`AcceptedModuleKey` has no public constructor. `AcceptedProjectSnapshot` creates it only after proving that the source identity maps to exactly one canonical module and that the immutable `HirProject` module carries that same source identity.

`AcceptedSourceDocuments` remains the canonical accepted source metadata/index type but becomes an owned field of `AcceptedProjectSnapshot`; it is no longer independently published as another Arc. Its URI and identity maps are exactly:

```rust
BTreeMap<LspUriKey, SourceDocumentIdentity>
BTreeMap<SourceDocumentIdentity, AcceptedSourceDocument>
```

### 2.2 Environment and candidate shape

```rust
pub struct AcceptedProfileCandidate {
    profile: AcceptedProfileKey,
    world: Arc<RegisteredSemanticWorld>,
    project: Arc<AcceptedProjectSnapshot>,
    overlays: AcceptedOverlaySet,
}

pub struct AcceptedProfileEnvironment {
    generation: AcceptedEnvironmentGeneration,
    profile: AcceptedProfileKey,
    world: Arc<RegisteredSemanticWorld>,
    project: Arc<AcceptedProjectSnapshot>,
    overlays: AcceptedOverlaySet,
    caches: ProfileSemanticCaches,
}
```

The old `AcceptedProfileEnvironment::sources()` accessor is deleted. All callers use `accepted.project().sources()` so there is one source authority and no compatibility projection. `project()` is crate-private and returns `&Arc<AcceptedProjectSnapshot>`; `world()` remains the public semantic-world accessor required by existing integration tests. No public API exposes the crate-private snapshot type.

Candidate construction is exact:

```rust
impl AcceptedProfileCandidate {
    pub(crate) fn try_new(
        profile: AcceptedProfileKey,
        world: Arc<RegisteredSemanticWorld>,
        project: Arc<AcceptedProjectSnapshot>,
        overlays: AcceptedOverlaySet,
    ) -> Result<Self, AcceptedProfileCandidateError>;

    pub(crate) fn try_from_unchanged_project(
        current: &Arc<AcceptedProfileEnvironment>,
        overlays: AcceptedOverlaySet,
    ) -> Result<Self, AcceptedProfileCandidateError>;
}
```

`try_from_unchanged_project` is the only metadata-only constructor. It performs no I/O, parse, lower, link, or registration; it clones the exact current world/project Arcs and revalidates profile/overlay/source coverage. Every successful `replace_accepted` creates a new `Arc<AcceptedProfileEnvironment>`, increments `AcceptedEnvironmentGeneration`, and starts with a fresh generation-owned cache namespace, including metadata-only publication.

```rust
#[derive(Debug, thiserror::Error)]
pub(crate) enum AcceptedProfileCandidateError {
    #[error("candidate world ID differs from the accepted project")]
    WorldMismatch {
        expected: ProjectSymbolWorldId,
        actual: ProjectSymbolWorldId,
    },
    #[error("candidate symbol revision differs from the accepted project")]
    SymbolRevisionMismatch {
        expected: ProjectSymbolRevision,
        actual: ProjectSymbolRevision,
    },
    #[error("candidate character source revision differs from accepted sources")]
    CharacterSourceRevisionMismatch {
        expected: SourceSetRevision,
        actual: SourceSetRevision,
    },
    #[error("candidate overlay URI is absent from accepted sources")]
    UnknownOverlayUri { uri: LspUriKey },
    #[error("candidate overlay identity differs from accepted URI identity")]
    OverlayIdentityMismatch {
        uri: LspUriKey,
        expected: SourceDocumentIdentity,
        actual: SourceDocumentIdentity,
    },
}
```

## 3. HIR construction and exact binding

### 3.1 Panic-free module construction

The current panicking `HirProjectModule::new` is replaced directly on its owning type; no helper trait or wrapper is added:

```rust
#[derive(Clone, Debug, Eq, thiserror::Error, PartialEq)]
pub enum HirProjectModuleError {
    #[error("HIR module `{module}` is not bound to a source document")]
    MissingSourceDocument { module: CanonicalModulePath },
    #[error("HIR module `{module}` is bound to another source revision")]
    SourceIdentityMismatch {
        module: CanonicalModulePath,
        expected: SourceDocumentIdentity,
        actual: SourceDocumentIdentity,
    },
}

impl HirProjectModule {
    pub fn try_new(
        module: CanonicalModulePath,
        source: SourceDocumentIdentity,
        hir: HirModule,
    ) -> Result<Self, HirProjectModuleError>;
}
```

All workspace callers migrate in the same compiling cut and the panicking `new` entry point is deleted.

`HirModule` gains the inherent document accessor on the repository-owned type:

```rust
impl HirModule {
    pub fn source_document(&self) -> Option<&arcweft_source::SourceDocument>;
}
```

It returns the exact revision-bound document retained by `HirSourceMap`. `SourceDocument::clone` shares the existing `Arc<SourceDocumentIdentity>` and `Arc<str>`, so the standard lowering route does not duplicate UTF-8 allocations merely to expose this accessor.

### 3.2 One build, two consumers

The profile rebuild transaction runs the existing whole-project path exactly once:

1. project-loader discovers and bounded-reads the selected project source inventory;
2. accepted open overlay bytes replace disk bytes by the already-selected logical document ID;
3. `parse_source` runs in `profiles::environment::register_profile_environment_with_overlays`, never in `features::signature` or a cache miss;
4. `lower_document_to_hir` runs once per selected project module;
5. `HirProjectModule::try_new` binds each lowered module;
6. `HirProject::new` produces one value, immediately wrapped as `Arc<HirProject>`;
7. `CharacterRegistrar::register` borrows `project.as_ref()`;
8. `AcceptedProjectSnapshot::try_new` retains `Arc::clone(&project)` after registration succeeds;
9. one candidate containing the world and accepted project is offered for publication.

A transient linked HIR view created internally by existing registration is compiler work and is released before publication. No second retained project is created for LSP.

### 3.3 Snapshot constructor and invariants

```rust
impl AcceptedProjectSnapshot {
    pub(crate) fn try_new(
        hir: Arc<HirProject>,
        world: &RegisteredSemanticWorld,
        source_seeds: Vec<AcceptedSourceDocumentSeed>,
    ) -> Result<Self, AcceptedProjectSnapshotError>;
}
```

The snapshot exposes only typed lookup:

```rust
impl AcceptedProjectSnapshot {
    pub(crate) fn hir_project(&self) -> &Arc<HirProject>;
    pub(crate) fn sources(&self) -> &AcceptedSourceDocuments;
    pub(crate) fn source_identity_by_uri(
        &self,
        uri: &LspUriKey,
    ) -> Option<&SourceDocumentIdentity>;
    pub(crate) fn source(
        &self,
        identity: &SourceDocumentIdentity,
    ) -> Option<&AcceptedSourceDocument>;
    pub(crate) fn module_key(
        &self,
        source: &SourceDocumentIdentity,
    ) -> Option<AcceptedModuleKey>;
    pub(crate) fn hir(
        &self,
        key: &AcceptedModuleKey,
    ) -> Result<&HirModule, AcceptedHirLookupError>;
    pub(crate) fn footprint(&self) -> AcceptedProjectFootprint;
}

#[derive(Clone, Debug, Eq, thiserror::Error, PartialEq)]
pub(crate) enum AcceptedHirLookupError {
    #[error("accepted HIR module is absent")]
    MissingModule { key: AcceptedModuleKey },
    #[error("accepted HIR source identity differs from its key")]
    SourceIdentityMismatch {
        key: AcceptedModuleKey,
        actual: Option<SourceDocumentIdentity>,
    },
    #[error("accepted HIR source document is absent")]
    MissingSourceDocument { key: AcceptedModuleKey },
    #[error("accepted HIR source document differs from its key")]
    SourceDocumentMismatch {
        key: AcceptedModuleKey,
        actual: SourceDocumentIdentity,
    },
}
```

`hir()` never accepts a bare path, URI, string, or raw integer: it looks up only `key.module()` and then checks `key.source()` against the returned HIR source/document. The HIR instance identity for request validation is exactly the pair **(`Arc<HirProject>` pointer, `AcceptedModuleKey`)**. Because an immutable `HirProject` uniquely owns one module value per canonical path, no raw `*const HirModule`, integer address, `Arc<HirModule>`, or signature-specific HIR ID is introduced.

Construction is all-or-nothing. Before returning `Ok`, it shall prove, in this order:

1. strict accepted source construction: a repeated `SourceDocumentIdentity` is an error even when metadata is equal;
2. a repeated logical `SourceDocumentId` with another revision/length is an error;
3. a repeated `LspUriKey` is an error;
4. aggregate unique document count and source bytes fit the production limits with checked arithmetic;
5. accepted source world and symbol revision match the registered world; every `CharacterDefinitionIndex` document exists with exact identity/text, and the recomputed character-document `SourceSetRevision` matches the registered index source revision;
6. `HirProject` module paths and `ProjectSymbolTable::modules()` are equal sets, including modules with no declarations;
7. for every canonical module, `HirProject::source(module)`, `HirModule::source_identity()`, and `ProjectSymbolTable::source_identity(module)` are equal;
8. the module source exists in accepted sources;
9. `HirModule::source_document()` exists, has the same identity, has exact byte-for-byte text equality with the accepted source document, and has the same source length;
10. one source identity maps to no more than one canonical module;
11. the final reverse map, footprint, and character-document source-set revision are computed without overflow.

Identity equality is not accepted as a substitute for text equality; the explicit text comparison rejects a hypothetical digest collision.

The exact error enum is:

```rust
#[derive(Debug, thiserror::Error)]
pub(crate) enum AcceptedProjectSnapshotError {
    #[error("accepted project contains duplicate source identity")]
    DuplicateSourceIdentity { source: SourceDocumentIdentity },
    #[error("accepted project contains conflicting revisions for one source id")]
    ConflictingSourceId {
        id: arcweft_source::SourceDocumentId,
        first: SourceDocumentIdentity,
        conflicting: SourceDocumentIdentity,
    },
    #[error("accepted project contains duplicate URI mapping")]
    DuplicateUri {
        uri: LspUriKey,
        first: SourceDocumentIdentity,
        conflicting: SourceDocumentIdentity,
    },
    #[error("accepted project limit exceeded")]
    Limit {
        kind: AcceptedProjectLimitKind,
        observed: u64,
        maximum: u64,
    },
    #[error("accepted project counter overflowed")]
    ArithmeticOverflow { counter: AcceptedProjectLimitKind },
    #[error("accepted source registry does not match the registered world")]
    WorldMismatch {
        expected: arcweft_lang_hir::symbol::ProjectSymbolWorldId,
        actual: arcweft_lang_hir::symbol::ProjectSymbolWorldId,
    },
    #[error("accepted source registry does not match the symbol revision")]
    SymbolRevisionMismatch {
        expected: arcweft_lang_hir::symbol::ProjectSymbolRevision,
        actual: arcweft_lang_hir::symbol::ProjectSymbolRevision,
    },
    #[error("accepted character-document source revision does not match the registered index")]
    CharacterSourceRevisionMismatch {
        expected: arcweft_source::SourceSetRevision,
        actual: arcweft_source::SourceSetRevision,
    },
    #[error("HIR and symbol module inventories differ")]
    ModuleInventoryMismatch {
        hir_only: Box<[CanonicalModulePath]>,
        symbol_only: Box<[CanonicalModulePath]>,
    },
    #[error("HIR project module is missing its source identity")]
    MissingProjectSource { module: CanonicalModulePath },
    #[error("symbol table module is missing its source identity")]
    MissingSymbolSource { module: CanonicalModulePath },
    #[error("module source is absent from accepted documents")]
    MissingModuleDocument {
        module: CanonicalModulePath,
        source: SourceDocumentIdentity,
    },
    #[error("module HIR is not bound to a source document")]
    MissingHirSource { module: CanonicalModulePath },
    #[error("module source identities disagree")]
    ModuleSourceMismatch {
        module: CanonicalModulePath,
        project: SourceDocumentIdentity,
        hir: SourceDocumentIdentity,
        symbols: SourceDocumentIdentity,
    },
    #[error("module HIR text differs from the accepted source")]
    HirTextMismatch {
        module: CanonicalModulePath,
        source: SourceDocumentIdentity,
    },
    #[error("one accepted source is bound to multiple modules")]
    ConflictingModuleMapping {
        source: SourceDocumentIdentity,
        first: CanonicalModulePath,
        conflicting: CanonicalModulePath,
    },
    #[error(transparent)]
    SourceSet(#[from] arcweft_source::SourceSetRevisionError),
}
```

`AcceptedProjectLimitKind` is an Arcweft-owned enum and receives its label/accessor behavior in its inherent implementation, not through an extension trait.

### 3.4 Declaration-free, dependency, and generated sources

The reverse index is built from `HirProject::modules()`, never from declarations. A root or non-root module with zero declarations is therefore queryable.

Ownership and writability do not determine queryability. A workspace, dependency, or generated source is queryable exactly when all three facts exist in the accepted snapshot:

- one explicit accepted URI mapping;
- one accepted source identity;
- one `HirProject` module carrying that source identity.

A dependency module is normally read-only but remains queryable. A generated or registration-only source with no URI, or with no `HirProject` module, remains accepted source metadata but is not assigned a fabricated URI or module. Acquisition returns `SourceHasNoHirModule`; no parse/lower fallback runs.

## 4. Overlay acceptance

### 4.1 Transactional changed-byte policy

Changed open bytes are included in the candidate's project parse/lower and world registration transaction. Until that complete candidate is published, signature acquisition for the changed URI fails as `DocumentNotAccepted` and maps to LSP `ContentModified`.

A successful changed-byte rebuild publishes one new generation containing matching overlay document, HIR, symbols, character inventory, source registry, and cache namespace.

A failed changed-byte rebuild publishes nothing. The old accepted environment and its cache remain valid for unchanged URIs, while the changed live URI cannot acquire the old document/HIR because its rebound identity or exact text differs. Failure never causes a partial overlay, partial HIR registry, or cache namespace change.

### 4.2 Open and identical-byte version publication

`didOpen` is part of acceptance rather than a side table:

- if the opened bytes equal an accepted source for the mapped profile, the session synchronously publishes a metadata-only candidate that adds the URI/version overlay before completing `didOpen`;
- if they differ, the URI is immediately marked pending, old bound requests are cancelled, and a full transactional rebuild is required before signature acquisition;
- a request cannot use a disk-only accepted document while an open overlay for the same URI is pending.

An LSP `didChange` carrying byte-identical text and a new version also does not parse or lower. Under the session write lock, the server:

1. rebinds the live bytes through the current accepted source logical ID;
2. proves identity and exact text equality;
3. constructs `AcceptedProfileCandidate::try_from_unchanged_project`, cloning the exact world and project Arcs and replacing only the complete accepted overlay set;
4. publishes it as a new generation with fresh caches before completing the notification.

A request arriving before that metadata generation exists receives `OverlayVersionNotAccepted`; it cannot reuse an older-version stamp merely because bytes are equal.

On `didClose`, the live snapshot and overlay authority are removed first, matching requests are cancelled, and a disk/remaining-overlay rebuild is scheduled. The historical overlay entry may exist only inside the immutable old generation until replacement; acquisition requires an open snapshot and therefore never serves the closed URI. Other unchanged URIs may continue to use the last successful generation while the close rebuild runs.

Session publication recomputes the exact `BTreeMap<LspUriKey, AcceptedOverlayEntry>` expected from all currently open documents mapped to that profile and requires equality with the candidate overlay set. No duplicate, omitted open URI, extra closed URI, version mismatch, or logical-identity mismatch is accepted.

## 5. Exact request acquisition

### 5.1 Lease

```rust
pub(crate) struct AcceptedDocumentHirLease {
    environment: Arc<AcceptedProfileEnvironment>,
    document: Arc<arcweft_source::SourceDocument>,
    uri: LspUriKey,
    module: AcceptedModuleKey,
}

impl AcceptedDocumentHirLease {
    pub(crate) fn document(&self) -> &arcweft_source::SourceDocument;
    pub(crate) fn module(&self) -> &CanonicalModulePath;
    pub(crate) fn world(&self) -> &RegisteredSemanticWorld;
    pub(crate) fn hir(&self) -> Result<&HirModule, SignatureAcquireError>;
}
```

The lease is not self-referential. It owns the accepted environment Arc and source document Arc and delegates to `environment.project().hir(&module)`. `hir()` maps `AcceptedHirLookupError` to an acquisition/integrity error; it never indexes with a panic. The lease and stamp retain the project/environment Arcs, not a borrowed HIR pointer.

### 5.2 Handler input and method sequence

`server::run_connection` parses `lsp_types::SignatureHelpParams` and, while holding a session read guard, calls:

```rust
ArcweftLspSession::prepare_signature_request(
    &self,
    request_id: lsp_server::RequestId,
    params: lsp_types::SignatureHelpParams,
    requests: &Arc<RequestRegistry>,
) -> Result<PreparedSignatureRequest, SignatureAcquireError>
```

The prepared carrier is exact and owns every borrow source until worker completion:

```rust
pub(crate) struct PreparedSignatureRequest {
    request_id: lsp_server::RequestId,
    position: lsp_types::Position,
    snapshot: DocumentSnapshot,
    lease: AcceptedDocumentHirLease,
    stamp: SignatureRequestStamp,
    active: ActiveRequest,
}

impl PreparedSignatureRequest {
    pub(crate) fn request_id(&self) -> &lsp_server::RequestId;
    pub(crate) fn position(&self) -> lsp_types::Position;
    pub(crate) fn lease(&self) -> &AcceptedDocumentHirLease;
    pub(crate) fn stamp(&self) -> &SignatureRequestStamp;
    pub(crate) fn control(&self) -> &RequestControl;
}
```

The method performs exactly this sequence while the session read guard and `LspProfileState::accepted_read()` guard prevent document/profile publication mutation:

1. derive `LspUriKey` from `params.text_document_position_params.text_document.uri`;
2. clone the exact open `DocumentSnapshot` from typed `DocumentStore`;
3. resolve the URI to one `LspProfile` and clone its one `Arc<LspProfileState>`;
4. require active profile admission and clone one current `Arc<AcceptedProfileEnvironment>`;
5. require the accepted profile key to equal the mapped profile key;
6. resolve URI to one accepted `SourceDocumentIdentity` through `AcceptedProjectSnapshot`;
7. clone that one accepted `Arc<SourceDocument>`;
8. require one accepted overlay entry for the open URI and exact version equality;
9. rebind live bytes to the accepted logical document ID, then require identity and exact text equality with the accepted document;
10. resolve accepted source identity to one `AcceptedModuleKey`;
11. resolve one `&HirModule` under `AcceptedModuleKey::module()`, checking its source identity and retained source document again;
12. create the lease and immutable stamp;
13. admit the request into `RequestRegistry` with its complete URI/workspace/profile/accepted binding and deadline;
14. return `PreparedSignatureRequest` containing the protocol position, open snapshot, lease, stamp, and `ActiveRequest` guard.

The server queues the prepared request only after this method returns. Because the message-intake thread does not read the next protocol message during preparation, registration occurs before any subsequent `$/cancelRequest` can be observed; cancellation-before-work is therefore not lost.

### 5.3 Acquisition failures

```rust
#[derive(Debug, thiserror::Error)]
pub(crate) enum SignatureAcquireError {
    #[error(transparent)]
    Admission(#[from] RequestAdmissionError),
    #[error("document is not open")]
    DocumentNotOpen { uri: LspUriKey },
    #[error("document has no Arcweft profile mapping")]
    ProfileNotMapped { uri: LspUriKey },
    #[error("profile request admission is closed")]
    ProfileClosing,
    #[error("profile has no accepted environment")]
    NoAcceptedEnvironment,
    #[error("accepted profile key differs from the mapped profile")]
    ProfileKeyMismatch,
    #[error("URI is not present in the accepted source registry")]
    UriNotAccepted { uri: LspUriKey },
    #[error("open URI is absent from the accepted overlay set")]
    OverlayNotAccepted { uri: LspUriKey },
    #[error("open document version is not the accepted version")]
    OverlayVersionNotAccepted {
        uri: LspUriKey,
        expected: i32,
        actual: i32,
    },
    #[error("open bytes are not the accepted document revision")]
    DocumentNotAccepted {
        uri: LspUriKey,
        expected: SourceDocumentIdentity,
        actual: SourceDocumentIdentity,
    },
    #[error("equal source identity has unequal text")]
    SourceDigestCollision { source: SourceDocumentIdentity },
    #[error("accepted source has no canonical HIR module")]
    SourceHasNoHirModule { source: SourceDocumentIdentity },
    #[error("accepted module is absent from the retained HIR project")]
    MissingHirModule { module: AcceptedModuleKey },
    #[error("retained HIR module no longer matches its accepted key")]
    HirIdentityMismatch { module: AcceptedModuleKey },
}
```

Disposition is fixed:

- `ProfileNotMapped`, `NoAcceptedEnvironment`, `UriNotAccepted`, and `SourceHasNoHirModule` are normal not-applicable outcomes and produce LSP `null` without caching;
- `DocumentNotOpen`, `OverlayNotAccepted`, `OverlayVersionNotAccepted`, `DocumentNotAccepted`, and profile/URI temporal mismatches produce `ContentModified` (`-32801`);
- client cancellation produces `RequestCancelled` (`-32800`); profile/global closure and queue shutdown produce `ServerCancelled` (`-32802`);
- active-limit and other bounded admission refusal produce `RequestFailed` (`-32803`);
- `SourceDigestCollision`, `MissingHirModule`, and `HirIdentityMismatch` produce `InternalError` (`-32603`) and a typed rebuild diagnostic.

No disposition invokes a parser, project loader, adapter lookup, or second resolver.

## 6. Request stamp and final validation

### 6.1 Exact stamp

```rust
pub(crate) struct SignatureRequestStamp {
    profile_state: Arc<LspProfileState>,
    accepted: Arc<AcceptedProfileEnvironment>,
    project: Arc<AcceptedProjectSnapshot>,
    hir_project: Arc<HirProject>,
    world: Arc<RegisteredSemanticWorld>,
    accepted_document: Arc<arcweft_source::SourceDocument>,

    profile: AcceptedProfileKey,
    generation: AcceptedEnvironmentGeneration,
    world_id: arcweft_lang_hir::symbol::ProjectSymbolWorldId,
    symbol_revision: arcweft_lang_hir::symbol::ProjectSymbolRevision,
    character_digest: arcweft_lang_sema::registration::CharacterInventoryDigest,
    character_revision: arcweft_lang_sema::registration::CharacterInventoryRevision,

    uri: LspUriKey,
    protocol_document: SourceDocumentIdentity,
    accepted_document_identity: SourceDocumentIdentity,
    lsp_version: i32,
    module: AcceptedModuleKey,
}
```

The stamp contains no forged `SourceSnapshotId`. The exact AW-AH-009.3 cache key/result remain unchanged; the stamp is a request validity guard around that cache.

### 6.2 Validation points

The same validator runs:

- before any cache lookup or query construction;
- immediately before a cache hit is returned; and
- after sema computation, immediately before any cache insertion or response publication.

For the pre-work/cache path, the worker acquires the session read lock, the exact profile accepted read guard, the request publication gate, and then the signature cache mutex. It validates before lookup and again before returning a hit while those guards remain held. On a miss it releases cache, gate, profile, and session guards before the long sema computation; the immutable lease/stamp and control Arc remain sufficient. After computation it reacquires session, profile, gate, and cache in that order and validates again. The validator checks in deterministic order chosen so every required value change is observable before an otherwise-equal replacement pointer:

1. the request gate is `Active`, its atomic loads `false` with `Acquire`, and `Instant::now()` is strictly before the deadline;
2. global session admission remains open, otherwise `SessionClosing`; stamped profile-state admission remains open, otherwise `ProfileClosing`;
3. URI is still open; current protocol document identity and exact `i32` version equal the stamp;
4. URI still maps to the stamped profile key, then to a profile-state Arc satisfying `Arc::ptr_eq`;
5. the state still has a current accepted environment; its profile key and generation values equal the stamp;
6. current world ID, symbol revision, character digest, and character revision equal the stamp, then its world Arc satisfies `Arc::ptr_eq`;
7. current accepted project maps the URI to the stamped accepted document identity and maps that source to the stamped `AcceptedModuleKey`;
8. rebound live bytes still equal the stamped accepted identity and exact accepted text;
9. current accepted document identity/text equal the stamp, then its source-document Arc satisfies `Arc::ptr_eq`;
10. current project `Arc<HirProject>` satisfies `Arc::ptr_eq` with the stamped HIR project; failure is `HirChanged` because this pointer plus `AcceptedModuleKey` is the complete HIR instance identity;
11. fresh typed HIR lookup under `AcceptedModuleKey::module()` succeeds and its source identity and retained source document equal the module key and accepted document;
12. current accepted-project snapshot Arc satisfies `Arc::ptr_eq` with the stamped snapshot;
13. current accepted-environment Arc satisfies `Arc::ptr_eq` with the stamped environment;
14. the atomic/gate/deadline checks from step 1 are repeated while the publication gate remains held.

No validation failure causes lookup in the new environment or insertion into its cache. The prepared request is permanently bound to the stamped accepted environment. A value-equal new `HirProject` Arc is still `HirChanged`; a value-equal new accepted-project wrapper around the same HIR Arc is `ProjectArcChanged`; a value-equal new environment around the same world/project Arcs is `AcceptedReplaced`.

```rust
#[derive(Clone, Debug, Eq, thiserror::Error, PartialEq)]
pub(crate) enum SignatureRequestStale {
    #[error("signature session admission is closing")]
    SessionClosing,
    #[error("signature profile admission is closing")]
    ProfileClosing,
    #[error("signature document was closed")]
    DocumentClosed { uri: LspUriKey },
    #[error("signature document bytes changed")]
    DocumentChanged {
        expected: SourceDocumentIdentity,
        actual: SourceDocumentIdentity,
    },
    #[error("signature document version changed")]
    DocumentVersionChanged { expected: i32, actual: i32 },
    #[error("signature URI was remapped to another profile")]
    ProfileRemapped {
        expected: AcceptedProfileKey,
        actual: Option<AcceptedProfileKey>,
    },
    #[error("signature profile state was replaced")]
    ProfileStateReplaced,
    #[error("signature accepted environment was replaced")]
    AcceptedReplaced,
    #[error("signature generation changed")]
    GenerationChanged {
        expected: AcceptedEnvironmentGeneration,
        actual: AcceptedEnvironmentGeneration,
    },
    #[error("signature accepted profile key changed")]
    ProfileKeyChanged {
        expected: AcceptedProfileKey,
        actual: AcceptedProfileKey,
    },
    #[error("signature registered world allocation changed")]
    WorldArcChanged,
    #[error("signature world identity changed")]
    WorldIdentityChanged {
        expected: ProjectSymbolWorldId,
        actual: ProjectSymbolWorldId,
    },
    #[error("signature symbol revision changed")]
    SymbolRevisionChanged {
        expected: ProjectSymbolRevision,
        actual: ProjectSymbolRevision,
    },
    #[error("signature character digest changed")]
    CharacterDigestChanged {
        expected: CharacterInventoryDigest,
        actual: CharacterInventoryDigest,
    },
    #[error("signature character revision changed")]
    CharacterRevisionChanged {
        expected: CharacterInventoryRevision,
        actual: CharacterInventoryRevision,
    },
    #[error("signature accepted project wrapper changed")]
    ProjectArcChanged,
    #[error("signature URI maps to another accepted document")]
    UriRemapped {
        expected: SourceDocumentIdentity,
        actual: Option<SourceDocumentIdentity>,
    },
    #[error("signature accepted document changed")]
    AcceptedDocumentChanged {
        expected: SourceDocumentIdentity,
        actual: Option<SourceDocumentIdentity>,
    },
    #[error("signature source maps to another module")]
    ModuleChanged {
        expected: AcceptedModuleKey,
        actual: Option<AcceptedModuleKey>,
    },
    #[error("signature HIR project/module instance changed")]
    HirChanged { module: AcceptedModuleKey },
    #[error("signature request was cancelled")]
    Cancelled { reason: SignatureCancellationReason },
    #[error("signature request deadline elapsed")]
    DeadlineExceeded { deadline: std::time::Instant },
}
```

Protocol mapping is exact:

- `Cancelled { reason: ClientCancelled }` maps to `RequestCancelled` (`-32800`);
- `DocumentChanged`, `DocumentClosed`, `ProfileRemapped`, `AcceptedReplaced`, and the corresponding lifecycle `Cancelled` reasons map to `ContentModified` (`-32801`);
- `DeadlineExceeded`, `SessionClosing`, `ProfileClosing`, and `Cancelled { reason: DeadlineExceeded | ProfileClosing | WorkspaceRemoved | SessionShutdown }` map to `ServerCancelled` (`-32802`);
- every other stamp mismatch maps to `ContentModified` (`-32801`).

The first cancellation reason recorded under the publication gate is terminal; later cancellation attempts do not replace it.

## 7. Cancellation, deadline, and request execution

### 7.1 Single owner

`RequestControl`, in `crates/arcweft-lsp/src/requests/control.rs`, is the only owner of the cancellation flag borrowed by the sema query:

```rust
pub(crate) struct SignatureRequestBinding {
    uri: LspUriKey,
    workspace: LspUriKey,
    profile_state: std::sync::Weak<LspProfileState>,
    accepted: std::sync::Weak<AcceptedProfileEnvironment>,
    document: SourceDocumentIdentity,
}

pub(crate) struct RequestControl {
    cancelled: std::sync::atomic::AtomicBool,
    deadline: std::time::Instant,
    binding: SignatureRequestBinding,
    gate: std::sync::Mutex<RequestGateState>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SignatureCancellationReason {
    ClientCancelled,
    DeadlineExceeded,
    DocumentChanged,
    DocumentClosed,
    ProfileRemapped,
    ProfileClosing,
    WorkspaceRemoved,
    AcceptedReplaced,
    SessionShutdown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RequestGateState {
    Active,
    Cancelled(SignatureCancellationReason),
    Finished,
}

impl RequestControl {
    pub(crate) fn cancellation_flag(&self) -> &std::sync::atomic::AtomicBool;
    pub(crate) fn deadline(&self) -> std::time::Instant;
    pub(crate) fn binding(&self) -> &SignatureRequestBinding;
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct DeadlineToken(u64);

pub(crate) struct ActiveRequest {
    registry: Arc<RequestRegistry>,
    id: lsp_server::RequestId,
    control: Arc<RequestControl>,
    deadline_token: DeadlineToken,
}

impl ActiveRequest {
    pub(crate) fn control(&self) -> &Arc<RequestControl>;
}
```

`ActiveRequest` is not cloneable. Its `Drop` is the sole active-map/deadline-token cleanup path. `RequestControl::cancellation_flag()` returns `&AtomicBool`. AW-AH-009.3 `SignatureQuery::try_new` receives that exact reference. There is no copied flag and no session-level cancelled-ID set. Registry filtering compares URI/workspace/document values and uses `Weak::ptr_eq` against downgraded state/environment Arcs; bindings never keep an accepted generation alive. The prepared request/lease/stamp are the only request-scoped strong owners and are bounded by admission.

### 7.2 Admission and deadline constants

```rust
pub(crate) const SIGNATURE_REQUEST_DEADLINE: Duration = Duration::from_millis(250);
pub(crate) const SIGNATURE_WORKER_COUNT: usize = 4;
pub(crate) const MAX_ACTIVE_SIGNATURE_REQUESTS: usize = 32;
```

The deadline begins at successful registry admission and includes queue time. It is not client-configurable. `Instant::checked_add` failure rejects admission as `DeadlineOverflow`.

`RequestRegistry` is owned by `run_connection` as one `Arc<RequestRegistry>`. Its active map is bounded to 32 admitted requests, including queued and running work. A standard-library bounded queue feeds four fixed workers. No external crate is added.

Global admission closes only when the LSP shutdown request is accepted or connection teardown begins. A profile-bound admission is rejected while its `LspProfileState` is closing/closed or its workspace has been removed. Reaching 32 rejects only that admission attempt; it does not permanently close the registry.

```rust
#[derive(Debug, thiserror::Error)]
pub(crate) enum RequestAdmissionError {
    #[error("request id is already active")]
    DuplicateRequestId { id: lsp_server::RequestId },
    #[error("signature active-request limit exceeded")]
    ActiveLimit { observed: usize, maximum: usize },
    #[error("global signature admission is closed")]
    AdmissionClosed,
    #[error("profile signature admission is closed")]
    ProfileClosing,
    #[error("signature worker queue is closed")]
    QueueClosed,
    #[error("signature deadline could not be represented")]
    DeadlineOverflow,
    #[error("signature deadline token exhausted")]
    DeadlineTokenOverflow,
}
```

`DuplicateRequestId`, `ActiveLimit`, `DeadlineOverflow`, and `DeadlineTokenOverflow` map to `RequestFailed` (`-32803`). `AdmissionClosed`, `ProfileClosing`, and `QueueClosed` map to `ServerCancelled` (`-32802`). A rejected admission leaves no active-map, queue, deadline, control, or accepted-generation entry.

The registry API is closed and direct:

```rust
impl RequestRegistry {
    pub(crate) fn admit(
        self: &Arc<Self>,
        id: lsp_server::RequestId,
        binding: SignatureRequestBinding,
    ) -> Result<ActiveRequest, RequestAdmissionError>;

    pub(crate) fn cancel(
        &self,
        id: &lsp_server::RequestId,
        reason: SignatureCancellationReason,
    );
    pub(crate) fn cancel_uri(&self, uri: &LspUriKey, reason: SignatureCancellationReason);
    pub(crate) fn cancel_workspace(
        &self,
        workspace: &LspUriKey,
        reason: SignatureCancellationReason,
    );
    pub(crate) fn cancel_profile_state(
        &self,
        state: &Arc<LspProfileState>,
        reason: SignatureCancellationReason,
    );
    pub(crate) fn cancel_accepted(
        &self,
        accepted: &Arc<AcceptedProfileEnvironment>,
        reason: SignatureCancellationReason,
    );
    pub(crate) fn close_admission(&self);
    pub(crate) fn shutdown(&self);
}
```

Each bulk-cancel method clones matching control Arcs while holding only the active-map mutex, releases that mutex, then cancels controls one by one. `close_admission` rejects future admissions without changing existing controls. `RequestRegistry::shutdown` is called only by `SignatureRequestRuntime::shutdown` after the executor has closed and joined; it closes/joins the deadline scheduler and verifies that guard cleanup emptied the active map.

The executor is standard-library-only and exact:

```rust
struct SignatureRequestExecutor {
    shared: Arc<SignatureExecutorShared>,
    workers: Mutex<Vec<std::thread::JoinHandle<()>>>,
}

struct SignatureExecutorShared {
    queue: Mutex<SignatureExecutorQueue>,
    available: Condvar,
}

struct SignatureExecutorQueue {
    closed: bool,
    jobs: VecDeque<PreparedSignatureRequest>,
}

pub(crate) struct SignatureRequestRuntime {
    registry: Arc<RequestRegistry>,
    executor: SignatureRequestExecutor,
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum RequestRuntimeError {
    #[error("failed to spawn a signature worker")]
    WorkerSpawn(#[source] std::io::Error),
    #[error("failed to spawn the signature deadline scheduler")]
    DeadlineSchedulerSpawn(#[source] std::io::Error),
}

impl SignatureRequestRuntime {
    pub(crate) fn new(
        connection: &lsp_server::Connection,
    ) -> Result<Self, RequestRuntimeError>;
    pub(crate) fn registry(&self) -> &Arc<RequestRegistry>;
    pub(crate) fn submit(
        &self,
        request: PreparedSignatureRequest,
    ) -> Result<(), RequestAdmissionError>;
    pub(crate) fn shutdown(self);
}
```

`run_connection` owns exactly one `SignatureRequestRuntime`. `shutdown` closes registry admission, cancels all controls as `SessionShutdown`, closes/drains the executor queue, joins the four workers, and only then closes/joins the deadline scheduler and asserts the active map is empty. The active-request ceiling bounds queued plus running jobs at 32, and exactly four worker threads drain FIFO order. Queue close atomically rejects new jobs and drains queued jobs outside the queue lock so their `ActiveRequest` guards clean up. Every worker invocation is wrapped in `std::panic::catch_unwind(AssertUnwindSafe(...))`; a panic is converted to one `InternalError` response when the gate is still active, and guard drop still removes active/deadline entries. Running sema work remains cooperatively bounded rather than force-killed.

### 7.3 `$/cancelRequest` route

The message-intake thread handles `$/cancelRequest` by calling `RequestRegistry::cancel(&RequestId, ClientCancelled)`. The registry clones the matching `Arc<RequestControl>` while holding its map mutex, releases the map mutex, then calls `RequestControl::cancel`. Unknown IDs are ignored and are never stored for later, so cancellation notifications cannot accumulate unbounded tombstones.

`cancel` locks the publication gate. If state is `Active`, it stores `true` into the `AtomicBool` with `Release` ordering and records the reason. Query checkpoints, worker admission checks, and final validation load it with `Acquire`. If state is `Finished`, cancellation is too late and changes nothing.

Before cache lookup or query construction, a worker acquires session read, profile accepted read, and then the control gate; it verifies `Active`, performs an `Acquire` cancellation load, checks the exact deadline, and runs the complete pre-work stamp validation. Only then may it lock the cache. A queued request cancelled or expired before its worker starts therefore performs zero query work and zero cache access. On a cache miss, all session/profile/gate/cache guards are released before sema work begins.

### 7.4 Deadline scheduler and cleanup

One registry-owned scheduler thread maintains a `BTreeMap<(Instant, u64), Weak<RequestControl>>` and sleeps on a `Condvar` until the earliest deadline or a registration change. It upgrades the weak reference and applies `DeadlineExceeded` at the deadline. The worker also checks `Instant::now() >= deadline` under the publication gate, so scheduler wakeup latency cannot permit a late publication.

Admission returns an `ActiveRequest` guard containing the control Arc and exact deadline token. On every exit path, including queue rejection, normal result, typed error, unwind, or response-send failure, `Drop`:

1. removes the active map entry only when `Arc::ptr_eq` identifies the same control;
2. removes the exact deadline token from the scheduler map;
3. drops the request's accepted environment/project/document Arcs.

The scheduler stores only `Weak`, and shutdown closes and joins both scheduler and workers. There is no leaked cancellation entry or retained accepted generation in the scheduler.

### 7.5 Publication linearization

Cancellation and result publication serialize on `RequestControl::gate`.

- Cancellation obtains the gate first: the flag/reason become terminal and cache publication fails.
- Publication obtains the gate first, validates the exact stamp and deadline, and—while session/profile/gate/cache guards remain held—enqueues the final protocol response on the connection's cloned response channel. For a computed cacheable result it then inserts into the exact stamped cache and changes state to `Finished`; for a cache hit it changes state to `Finished` after the enqueue. Actual socket I/O occurs outside these locks. If response enqueue fails, no cache insertion or `Finished` transition occurs and guard cleanup removes the request. A later cancellation after a successful enqueue/finish is too late.

This gate plus response enqueue is the single linearization point for the "cancel immediately before publication" race; no lifecycle mutation can occur between final validation and observable response publication.

## 8. Cache validity and lifecycle hooks

The existing AW-AH-009.3 result, key, ordering, and bounded-cache semantics are unchanged. `LspProfileState` exposes crate-private guard access so validation holds the actual lock rather than cloning and releasing the current pointer:

```rust
impl LspProfileState {
    pub(crate) fn accepted_read(
        &self,
    ) -> std::sync::RwLockReadGuard<'_, Option<Arc<AcceptedProfileEnvironment>>>;

    pub(crate) fn accepted_write(
        &self,
    ) -> std::sync::RwLockWriteGuard<'_, Option<Arc<AcceptedProfileEnvironment>>>;
}
```

The contract adds direct lifecycle APIs only:

```rust
impl ProfileSemanticCaches {
    pub(crate) fn invalidate_signature_document(
        &self,
        document: &SourceDocumentIdentity,
    );
    pub(crate) fn clear_signature(&self);
}

impl ArcweftLspSession {
    pub(crate) fn open_document(
        &mut self,
        snapshot: DocumentSnapshot,
        requests: &RequestRegistry,
    ) -> Result<(), AcceptedPublicationError>;

    pub(crate) fn change_document(
        &mut self,
        uri: &LspUriKey,
        version: i32,
        text: String,
        requests: &RequestRegistry,
    ) -> Result<(), AcceptedPublicationError>;

    pub(crate) fn close_document(
        &mut self,
        uri: &LspUriKey,
        requests: &RequestRegistry,
    );

    pub(crate) fn remove_workspace(
        &mut self,
        workspace: &LspUriKey,
        requests: &RequestRegistry,
    );

    pub(crate) fn publish_accepted_candidate(
        &mut self,
        state: &Arc<LspProfileState>,
        expected: Option<&Arc<AcceptedProfileEnvironment>>,
        candidate: AcceptedProfileCandidate,
        requests: &RequestRegistry,
    ) -> Result<Arc<AcceptedProfileEnvironment>, AcceptedPublicationError>;

    pub(crate) fn record_failed_replacement(
        &mut self,
        state: &Arc<LspProfileState>,
        expected: &Arc<AcceptedProfileEnvironment>,
    ) -> Result<(), AcceptedPublicationError>;

    pub(crate) fn begin_shutdown(&mut self, requests: &RequestRegistry);
}
```

```rust
#[derive(Debug, thiserror::Error)]
pub(crate) enum AcceptedPublicationError {
    #[error(transparent)]
    Overlay(#[from] AcceptedOverlaySetError),
    #[error(transparent)]
    Candidate(#[from] AcceptedProfileCandidateError),
    #[error("session publication admission is closed")]
    SessionClosing,
    #[error("URI no longer maps to the expected profile state")]
    ProfileStateReplaced,
    #[error("profile publication admission is closed")]
    ProfileClosing,
    #[error("expected accepted environment is no longer current")]
    AcceptedReplaced,
    #[error("candidate profile key differs from mapped profile")]
    ProfileKeyMismatch,
    #[error("candidate overlays differ from current open profile overlays")]
    OverlayCoverageMismatch {
        missing: Box<[LspUriKey]>,
        extra: Box<[LspUriKey]>,
        mismatched: Box<[LspUriKey]>,
    },
    #[error("accepted environment generation overflowed")]
    GenerationOverflow,
}
```

Behavior is exact:

- **Document open:** install the live snapshot under the session write lock, cancel any prior URI-bound controls, and synchronously publish the metadata-only candidate when bytes equal the accepted source; otherwise mark the URI pending and start a full transaction. `didOpen` returns only after the selected immediate publication/pending state is visible.
- **Document change:** under session write lock, cancel requests bound to the URI as `DocumentChanged` and invalidate that accepted document's signature entries before attempting metadata publication or rebuild.
- **Document close:** cancel URI requests, invalidate that document, remove the live document/profile mapping immediately, and schedule a disk/remaining-overlay rebuild. A profile state is shut down only if no URI/workspace owner retains it. The old immutable overlay entry is never queryable without an open snapshot and disappears on the next successful publication.
- **Workspace removal:** close affected profile admission, cancel all bound requests, clear all old caches, clear accepted environments, and remove mappings/documents/analyses belonging to the workspace.
- **Accepted replacement:** while holding session write and profile accepted write locks, cancel controls bound to the old accepted Arc, clear the old signature cache, validate overlay coverage, create the next generation, and swap once. The new environment starts with empty caches.
- **Failed replacement:** verify that `expected` is still current and perform no accepted-environment or cache mutation. Changed live bytes already block acquisition, and the document-change hook already cancelled old in-flight work.
- **Shutdown:** under the session write lock, `begin_shutdown` closes global/profile admission, cancels every active request as `SessionShutdown`, and clears every cache/environment/mapping; after releasing the session lock, `run_connection` consumes `SignatureRequestRuntime::shutdown` to close/drain the queue, join cooperative bounded workers, and finally join the deadline scheduler.

A prepared request inserts only through `publish_signature_result`, which always targets its stamped accepted Arc. It never switches to a current/new cache. Replacement or invalidation that linearizes first therefore prevents old insertion; publication that linearizes first is immediately cleared by the later lifecycle operation. Lifecycle cancellation reasons `DocumentChanged`, `DocumentClosed`, `ProfileRemapped`, and `AcceptedReplaced` map to `ContentModified`; `ProfileClosing`, `WorkspaceRemoved`, and `SessionShutdown` map to `ServerCancelled`; `ClientCancelled` alone maps to `RequestCancelled`.

## 9. Lock order

Every implementation path uses this order and never reverses it:

1. session `RwLock` read/write guard;
2. `LspProfileState` accepted `RwLock` read/write guard;
3. `RequestControl` publication gate mutex;
4. signature cache mutex.

`RequestRegistry` releases its active-map mutex before it locks a control. The deadline scheduler releases its scheduler mutex before it cancels a control. This prevents registry/control and session/control inversion.

## 10. Limits, work, and retained memory

### 10.1 Accepted build limits

Accepted HIR construction is outside `SignatureQuery`'s per-query budget. It occurs only during complete profile rebuild or the no-HIR metadata-only path.

The LSP build uses these existing production authorities:

- unique accepted documents: `CharacterRegistrationLimits::PRODUCTION.documents()` = 4,096;
- aggregate unique accepted UTF-8 bytes: `MAX_REGISTRATION_SOURCE_BYTES` = 8,388,608;
- project symbol link work: `ProjectSymbolLimits::PRODUCTION.work()` = 262,144;
- project symbol diagnostics: `ProjectSymbolLimits::PRODUCTION.diagnostics()` = 128.

`arcweft-project-loader` gains this domain-neutral typed input:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProjectLoadLimits {
    documents: u64,
    source_bytes: u64,
}

impl ProjectLoadLimits {
    pub const fn new(documents: u64, source_bytes: u64) -> Self;
    pub const fn documents(self) -> u64;
    pub const fn source_bytes(self) -> u64;
}
```

The LSP constructs it from the first two existing authorities above. These are inclusive caller-supplied ceilings; this cut adds aggregate pre-parse enforcement to project-loader, not a signature-specific semantic budget. Source enumeration stops at maximum + 1. Disk reads use remaining-budget + 1 bounded reads, not unbounded `read_to_string`; overlay lengths are charged before parsing. The complete accepted snapshot rechecks final unique documents/bytes after generated and registration documents are added. Every sum uses checked arithmetic and reports the exact observed/maximum or overflow counter.

Candidate construction remains serialized in the existing connection/profile rebuild path: at most one unpublished candidate exists for one connection/profile transaction, and no unbounded background build queue is introduced. There is no signature-specific successful fallback when a limit fails. The candidate is rejected and the prior accepted generation remains.

### 10.2 Memory ownership

The retained project is one `Arc<HirProject>`. Registration borrows it and LSP retains it; no HIR clone is added to `RegisteredSemanticWorld`, source registry, cache, request stamp, or lease. `SourceDocument` cloning in existing HIR source maps shares its identity and UTF-8 `Arc`s.

Each accepted snapshot records exact document/module/source-byte footprint. An accepted environment owns one project Arc. Generation replacement removes the current-state strong reference to the old environment and clears its caches.

Old generations can then be retained only by admitted `PreparedSignatureRequest`/lease/stamp contexts. The global maximum is 32 such contexts; only four can execute at once, queued cancelled jobs are discarded, every context has a 250 ms deadline, and the query retains its original finite work budget/checkpoints. `SignatureRequestBinding` and the scheduler hold only `Weak` state/environment/control references where appropriate. Thus old-generation count and accepted input size are both hard-bounded, and completed work releases immediately through `ActiveRequest::drop`.

## 11. Connection to AW-AH-009.3

Only after AW-AH-009.3.1 lands its exact authored-call carrier does the signature worker, after the pre-work gate/deadline/stamp check, invoke the original sema query:

```rust
let hir = prepared.lease().hir()?;
let query = SignatureQuery::try_new(
    prepared.lease().document(),
    hir,
    prepared.lease().world(),
    /* exact AW-AH-009.3.1 call carrier, unchanged by this contract */
    prepared.control().cancellation_flag(),
    SignatureQueryLimits::PRODUCTION,
)?;
```

The source position is converted through the existing exact `LineIndex`; this contract does not choose the authored call syntax/range representation. The word-at-position/Rust-adapter fallback is deleted only in the final integration cut after the sema path compiles and passes direct tests.

No source substring parser, word parser, source search, `SourceSnapshotId` fabrication, or second resolver is permitted on a cache miss or acquisition failure.
