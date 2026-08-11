# Final Rust shapes and ownership

The signatures below are normative API shapes. Field visibility may remain private; the named constructors/accessors and ownership boundaries are required.

## `arcweft-project::content` — shared Sans-I/O contract

```rust
use std::{collections::BTreeMap, sync::Arc};
use arcweft_character::{
    id::CharacterId,
    manifest::{CharacterAssetPath, registration::SourceBackedCharacterManifest},
    package::CharacterPackage,
};
use arcweft_id::EntityId;
use arcweft_manifest_model::{
    ContentRootRef, ContentUnitId, DependencyDemand, ManifestVisibility,
    NormalizedProjectPath, PackageId, PackageVersion, ProfileContentSpec, ProfileId,
};
use arcweft_launch::accepted::{
    ContentRootOccurrenceSource, ContentUnitManifestSource, ProfileContentManifestSource,
};
use arcweft_resource_model::identity::ResourceDeclarationIdentity;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProjectTopologyRevision(BuildDigest);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectBinaryResource {
    bytes: Arc<[u8]>,
    digest: BuildDigest,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ContentRootOccurrenceId {
    unit: ContentUnitId,
    ordinal: u32,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SourceContentRootFamily {
    Flow,
    View,
    Action,
    Activity,
    Source,
    Asset,
    Signal,
    Metric,
    Layer,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AcceptedContentRootTarget {
    Character { character: CharacterId },
    SourceEntity {
        entity: EntityId,
        family: SourceContentRootFamily,
    },
    ConfiguredResource {
        identity: ResourceDeclarationIdentity,
    },
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ContentRootReferenceKind {
    None,
    Profile,
    Runtime,
    ProfileAndRuntime,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AbsentCharacterRoot {
    character: CharacterId,
    package_root: NormalizedProjectPath,
    manifest_path: NormalizedProjectPath,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CandidateContentRootState {
    CharacterPresent { character: CharacterId },
    CharacterOptionalAbsent(AbsentCharacterRoot),
    SemanticPending,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AcceptedContentRootState {
    Present(AcceptedContentRootTarget),
    OptionalAbsent(AbsentCharacterRoot),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ContentUnitProfileSelection {
    Unselected,
    Selected {
        profile: ProfileId,
        policy: ProfileContentSpec,
        source: ProfileContentManifestSource,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CandidateContentRoot {
    occurrence: ContentRootOccurrenceId,
    authored: ContentRootRef,
    source: ContentRootOccurrenceSource,
    state: CandidateContentRootState,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CandidateContentUnit {
    id: ContentUnitId,
    visibility: ManifestVisibility,
    demand: DependencyDemand,
    profile: ContentUnitProfileSelection,
    source: ContentUnitManifestSource,
    roots: Vec<CandidateContentRoot>,
}

#[derive(Clone, Debug)]
pub struct AcceptedCharacterPackage {
    package: Arc<CharacterPackage>,
    source_manifest: Arc<SourceBackedCharacterManifest>,
    package_root: NormalizedProjectPath,
    manifest_path: NormalizedProjectPath,
    layer_paths: BTreeMap<CharacterAssetPath, NormalizedProjectPath>,
}

#[derive(Clone, Debug)]
pub struct ContentAdmissionCandidate {
    package_id: PackageId,
    package_version: PackageVersion,
    selected_profile: ProfileId,
    topology_revision: ProjectTopologyRevision,
    units: BTreeMap<ContentUnitId, CandidateContentUnit>,
    character_packages: BTreeMap<CharacterId, AcceptedCharacterPackage>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptedContentRoot {
    occurrence: ContentRootOccurrenceId,
    authored: ContentRootRef,
    source: ContentRootOccurrenceSource,
    state: AcceptedContentRootState,
    referenced_by: ContentRootReferenceKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptedContentUnit {
    id: ContentUnitId,
    visibility: ManifestVisibility,
    demand: DependencyDemand,
    profile: ContentUnitProfileSelection,
    source: ContentUnitManifestSource,
    roots: Vec<AcceptedContentRoot>,
}

#[derive(Clone, Debug)]
pub struct AcceptedContentInventory {
    package_id: PackageId,
    package_version: PackageVersion,
    selected_profile: ProfileId,
    topology_revision: ProjectTopologyRevision,
    units: BTreeMap<ContentUnitId, AcceptedContentUnit>,
    character_packages: BTreeMap<CharacterId, AcceptedCharacterPackage>,
}
```

Required inherent APIs:

```rust
impl ProjectBinaryResource {
    pub fn new(bytes: impl Into<Arc<[u8]>>) -> Self;
    pub fn bytes(&self) -> &[u8];
    pub const fn digest(&self) -> BuildDigest;
}

impl ProjectTopologyRevision {
    pub fn try_for_inventory(
        package: (&PackageId, &PackageVersion),
        profile: &ProfileId,
        records: impl IntoIterator<Item = ProjectTopologyResourceRecord>,
        semantic_records: impl IntoIterator<Item = ProjectTopologySemanticRecord>,
        absences: impl IntoIterator<Item = ProjectTopologyAbsenceRecord>,
    ) -> Result<Self, ProjectTopologyRevisionError>;
    pub const fn digest(self) -> BuildDigest;
}
```

`try_for_inventory` rejects duplicate canonical keys; it must not use `NamedDigest::canonicalize` because that helper intentionally keeps one duplicate entry.

## `arcweft-character::package` — extend the owner in place

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CharacterLayerPayload {
    path: CharacterAssetPath,
    bytes: Arc<[u8]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CharacterPackage {
    manifest: CharacterManifest,
    manifest_bytes: Arc<[u8]>,
    layer_payloads: BTreeMap<CharacterAssetPath, CharacterLayerPayload>,
}

impl CharacterPackage {
    pub fn from_source_backed_manifest(
        document: &SourceDocument,
        source_manifest: &SourceBackedCharacterManifest,
        payloads: impl IntoIterator<Item = CharacterLayerPayload>,
    ) -> Result<Self, CharacterPackageError>;
}
```

Additional owner-local errors:

```rust
pub enum CharacterPackageError {
    // existing variants remain
    ManifestSourceIdentityMismatch,
    InvalidLayerPng {
        path: CharacterAssetPath,
        message: String,
    },
    LayerDimensionsMismatch {
        path: CharacterAssetPath,
        expected_width: u32,
        expected_height: u32,
        actual_width: u32,
        actual_height: u32,
    },
}
```

Validation reads the complete PNG frame with the existing workspace `png = 0.18.1` dependency and no I/O. Header-only validation is insufficient. Encoded bytes are preserved unchanged after validation.

## `arcweft-project-loader::topology` — I/O and host-path ownership

```rust
#[derive(Clone, Debug)]
pub struct ProfileTopologyBinaryOverlaySeed {
    path: PathBuf,
    bytes: Arc<[u8]>,
}

#[derive(Clone, Debug)]
pub enum LoadedProfileTopologyResourcePayload {
    Text(Arc<SourceDocument>),
    Binary(Arc<ProjectBinaryResource>),
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProfileTopologyResourceKind {
    Manifest,
    ArcweftModule { module: CanonicalModulePath },
    CharacterPackageManifest { character: CharacterId },
    CharacterLayerPayload {
        character: CharacterId,
        asset: CharacterAssetPath,
    },
    ExternalModuleMetadata { import: ExternalModuleImportId },
}

pub struct ProfileTopologyLoadRequest<'a> {
    // existing fields
    text_overlays: &'a [ProfileTopologyOverlaySeed],
    binary_overlays: &'a [ProfileTopologyBinaryOverlaySeed],
    dependency_text_resources: &'a [ProfileDependencyResourceSeed],
    dependency_binary_resources: &'a [ProfileDependencyBinaryResourceSeed],
}

pub struct LoadedProfileTopology {
    // existing accepted manifest/profile/layout/metadata/source closure
    resources: BTreeMap<ProfileTopologyResourceId, LoadedProfileTopologyResource>,
    source_documents_revision: SourceSetRevision,
    content_candidate: Arc<ContentAdmissionCandidate>,
    watch_inventory: Arc<[ProfileTopologyWatchEntry]>,
}
```

`LoadedProfileTopologyResource` exposes `text_document()` and `binary_resource()` accessors. The old unconditional `document()` accessor is removed rather than made lossy or panicking.

## `arcweft-launch::accepted` — manifest source-provenance owner

The source-map projections live with `SourceBackedManifest`; `arcweft-launch` does not depend on `arcweft-project`.

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContentRootOccurrenceSource {
    value: SourceSpan,
    selection: SourceSpan,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContentUnitManifestSource {
    unit_key: SourceSpan,
    table: SourceSpan,
    visibility: SourceSpan,
    demand: SourceSpan,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProfileContentManifestSource {
    unit_key: SourceSpan,
    table: SourceSpan,
    residency: SourceSpan,
    placement: SourceSpan,
    compression: SourceSpan,
}

impl SourceBackedManifest {
    pub fn content_unit_source(&self, unit: &ContentUnitId)
        -> Option<ContentUnitManifestSource>;
    pub fn content_root_source(
        &self,
        unit: &ContentUnitId,
        root_index: usize,
    ) -> Option<ContentRootOccurrenceSource>;
    pub fn selected_profile_content_source(
        &self,
        profile: &ProfileId,
        unit: &ContentUnitId,
    ) -> Option<ProfileContentManifestSource>;
}
```

These methods project existing typed token paths. They do not expose Taplo nodes and do not parse text.

## `arcweft-lang-sema` and `arcweft-compiler`

Add the root-classification behavior to the owning enum:

```rust
impl EntityKind {
    pub const fn content_root_class(&self) -> BuiltinContentRootClass;
}
```

No extension trait or loader-local string match is permitted. Unknown families are classified with the accepted resource declaration registry at the semantic boundary.

Sema owns exact reference collection and admission finalization:

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContentRootReferenceFact {
    target: AcceptedContentRootTarget,
    reference: SourceSpan,
    selection: SourceSpan,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContentRootReferenceInventory {
    world: ProjectSymbolWorldId,
    symbol_revision: ProjectSymbolRevision,
    topology_revision: ProjectTopologyRevision,
    facts: Vec<ContentRootReferenceFact>,
}

pub struct ContentAdmissionFinalizeInput<'a> {
    pub candidate: &'a ContentAdmissionCandidate,
    pub symbols: &'a ProjectSymbolTable,
    pub resources: &'a AcceptedResourceDeclarationIndex,
    pub references: &'a ContentRootReferenceInventory,
}

pub fn finalize_content_admission(
    input: ContentAdmissionFinalizeInput<'_>,
) -> Result<AcceptedContentInventory, ContentAdmissionError>;
```

Reference collection walks typed HIR/judgment facts for the exact selected source/runtime closure and canonicalizes aliases/reexports through the sole symbol/resource world. Optional-absence Character IDs are supplied as typed reservations to this collector: an exact reference becomes `OptionalRootReferencedMissing`, not a generic unknown-owner diagnostic. The reservation is not a symbol, catalog entry, or executable runtime node.

Every exact typed occurrence counts as runtime-referenced, including an occurrence in a branch later proven unreachable. This is the fail-closed static contract. The compiler then consumes the accepted inventory and reuses its existing `LinkGraph::reachability` and `ContentPartitionPlan` only for bundle/startup/on-demand inclusion; it does not decide whether absent content is acceptable.

The compiler-local duplicate `ContentUnitId` is replaced by the manifest-model owner type. This is a boundary correction required to carry one identity from manifest through project facts and partitioning.

## `ProjectIndex` construction

```rust
pub struct ProjectSemanticIndexInput<'a> {
    pub project: &'a HirProject,
    pub program_hash: ProgramHash,
    pub checked_entries: &'a CheckedProjectEntries,
    pub content: &'a AcceptedContentInventory,
}

pub fn project_semantic_index_from_checked_project(
    input: ProjectSemanticIndexInput<'_>,
) -> ProjectSemanticIndex;
```

`ProgramHash` receives an inherent constructor/projection that includes `ProjectTopologyRevision`; consumers do not format ad hoc cache-key strings.

## Dependency direction

```text
arcweft-manifest-model / arcweft-source
                   |
            arcweft-launch (source provenance)
                   |
arcweft-character--+--> arcweft-project::content <--arcweft-resource-model
                              |
                 +------------+-------------+
                 |                          |
      arcweft-project-loader          arcweft-lang-sema
                                             |
                                      arcweft-compiler
                                             |
                              bundle / CLI / LSP / watch adapters
```

- `arcweft-core`, `arcweft-manifest-model`, `arcweft-launch`, `arcweft-character`, `arcweft-project`, and bundle models remain Sans I/O.
- Only project-loader/CLI/LSP/host adapters open files or maintain watches.
- No lower-level crate depends on LSP, CLI, compiler, filesystem, or platform adapters.
