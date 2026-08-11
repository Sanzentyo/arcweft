# Normative Rust shapes and ownership

The signatures in this document are normative. Fields may remain private, but
the named owners, closed variants, constructors/accessors, and revision checks
are required. Existing safe-substrate APIs not repeated here remain as selected
by Lang-01.5.1.2.

## 1. `arcweft-project::content` — shared Sans-I/O accepted inventory

`arcweft-project` owns the cross-consumer content contract. It uses identities
from their existing owner crates and source provenance projected by
`arcweft-launch`; it performs no I/O.

```rust
use std::{collections::BTreeMap, sync::Arc};

use arcweft_character::{
    id::CharacterId,
    manifest::{CharacterAssetPath, registration::SourceBackedCharacterManifest},
    package::CharacterPackage,
};
use arcweft_id::EntityId;
use arcweft_launch::accepted::{
    ContentRootOccurrenceSource,
    ContentUnitManifestSource,
    ProfileContentManifestSource,
};
use arcweft_manifest_model::{
    ContentRootRef,
    ContentUnitId,
    DependencyDemand,
    ManifestVisibility,
    NormalizedProjectPath,
    PackageId,
    PackageVersion,
    ProfileContentSpec,
    ProfileId,
};
use arcweft_resource_model::identity::{
    ResourceDeclarationIdentity,
    ResourceTypeId,
};

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ContentRootOccurrenceId {
    unit: ContentUnitId,
    ordinal: u32,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AuthoredContentRootFamily {
    Flow,
    View,
    Action,
    Activity,
    Asset,
    Signal,
    Metric,
    Layer,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ContentRootFamily {
    Character,
    AuthoredEntity(AuthoredContentRootFamily),
    ConfiguredResource {
        resource_type: ResourceTypeId,
    },
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AcceptedContentRootTarget {
    Character {
        character: CharacterId,
    },
    AuthoredEntity {
        entity: EntityId,
        family: AuthoredContentRootFamily,
    },
    ConfiguredResource {
        identity: ResourceDeclarationIdentity,
    },
}

impl AcceptedContentRootTarget {
    pub fn family(&self) -> ContentRootFamily;
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
    CharacterPresent {
        character: CharacterId,
    },
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

There is deliberately no alias for the old family type and no legacy target
variant. The only final spellings are `AuthoredContentRootFamily` and
`AcceptedContentRootTarget::AuthoredEntity`.

Required inherent projections include:

```rust
impl ContentRootOccurrenceId {
    pub const fn unit(&self) -> &ContentUnitId;
    pub const fn ordinal(&self) -> u32;
}

impl AcceptedContentRoot {
    pub const fn occurrence(&self) -> &ContentRootOccurrenceId;
    pub const fn authored(&self) -> &ContentRootRef;
    pub const fn source(&self) -> &ContentRootOccurrenceSource;
    pub const fn state(&self) -> &AcceptedContentRootState;
    pub const fn referenced_by(&self) -> ContentRootReferenceKind;
}

impl AcceptedContentInventory {
    pub const fn topology_revision(&self) -> ProjectTopologyRevision;
    pub fn units(
        &self,
    ) -> impl ExactSizeIterator<Item = (&ContentUnitId, &AcceptedContentUnit)>;
    pub fn character_package(
        &self,
        id: &CharacterId,
    ) -> Option<&AcceptedCharacterPackage>;
}
```

## 2. Existing binary/topology substrate remains exact

The existing owner shapes remain authoritative:

```rust
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProjectTopologyRevision(BuildDigest);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectBinaryResource {
    bytes: Arc<[u8]>,
    digest: BuildDigest,
}

impl ProjectBinaryResource {
    pub fn new(bytes: impl Into<Arc<[u8]>>) -> Self;
    pub fn bytes(&self) -> &[u8];
    pub fn shared_bytes(&self) -> Arc<[u8]>;
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

Duplicate canonical keys are rejected. No `NamedDigest` first/last-wins helper,
debug formatting, or generic Serde transcript is used.

## 3. `arcweft-launch::accepted` — exact manifest source owner

These existing source projections are retained without moving bytes into
`SourceDocument` or reparsing TOML:

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
    pub fn content_unit_source(
        &self,
        unit: &ContentUnitId,
    ) -> Option<ContentUnitManifestSource>;

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

`value` is the complete manifest scalar/array-element span. `selection` is the
exact string-content span used for a root diagnostic. Neither is synthesized
from a host path.

## 4. `arcweft-id` and `arcweft-lang-sema` — owner-local classification

`arcweft-id::RetainedIdentityFamily` already owns retained global identity
families. Add content-root behavior to that owner when needed; do not create a
second string table in the loader.

The semantic entity owner exposes the final classification as inherent
behavior:

```rust
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum BuiltinContentRootClass {
    CharacterFileBacked,
    Authored(AuthoredContentRootFamily),
    WrongFamily,
}

impl EntityKind {
    pub const fn content_root_class(&self) -> BuiltinContentRootClass;
}
```

The exhaustive mapping is:

```text
Character -> CharacterFileBacked
Flow      -> Authored(Flow)
View      -> Authored(View)
Action    -> Authored(Action)
Activity  -> Authored(Activity)
Asset     -> Authored(Asset)
Signal    -> Authored(Signal)
Metric    -> Authored(Metric)
Layer     -> Authored(Layer)
all other final EntityKind values -> WrongFamily
```

The final `EntityKind` has no Source variant, so there is no Source match arm,
compatibility arm, or `Other("source")` promotion.

Configured-resource lookup is implemented on the accepted resource declaration
index owner. It returns an exact `ResourceDeclarationIdentity`; it does not
return a family guess.

## 5. Sema reference collection and admission finalization

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

`reference` is the full typed occurrence span. `selection` is the exact selected
entity/resource token span. The source document may be an Arcweft module,
accepted generated metadata document, or accepted configured-resource
document; provenance is recovered from the span's document identity, not an
open string tag.

The finalizer requires exact equality among:

- candidate topology revision;
- reference-inventory topology revision;
- symbol world and symbol revision;
- resource declaration/index accepted world;
- selected package/profile identity.

A mismatch is a typed stale-world error before presence reconciliation.

Optional-absent Character identities are supplied to reference collection as
typed reservations. A reservation is not inserted into `ProjectSymbolTable`,
the entity index, resource index, Character catalog, ProjectIndex, bundle, or
LSP symbol list.

## 6. No callable target shape

The following final types do not exist:

```text
ContentRootFamily::Callable
AcceptedContentRootTarget::Callable
StreamContentRootFamily
GeneratorContentRootFamily
ExternalStreamContentRootFamily
```

The resolver may produce a transient typed candidate whose actual category is
`Callable`, `External`, `Nominal`, or `Module` solely to report the ordinary
wrong-symbol-kind diagnostic. Such a candidate is never converted to an
accepted content target.

## 7. Manifest-owned ProjectIndex facts

The final ProjectIndex stores content facts directly. It does not fabricate a
source `content` entity.

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectContentRootFact {
    occurrence: ContentRootOccurrenceId,
    authored: ContentRootRef,
    source: ContentRootOccurrenceSource,
    state: AcceptedContentRootState,
    referenced_by: ContentRootReferenceKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectContentUnitFact {
    id: ContentUnitId,
    visibility: ManifestVisibility,
    demand: DependencyDemand,
    profile: ContentUnitProfileSelection,
    source: ContentUnitManifestSource,
    roots: Vec<ProjectContentRootFact>,
}

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

`ProjectSemanticIndex` gains:

```rust
content_units: BTreeMap<ContentUnitId, ProjectContentUnitFact>,
```

The existing graph endpoint enum is extended in place rather than wrapped by a
root-local helper:

```rust
pub enum ProjectGraphSymbolRef {
    Entity(PublicId),
    Callable(QualifiedName),
    ContentUnit(ContentUnitId),
    ContentRootOccurrence(ContentRootOccurrenceId),
    ContentRootTarget(AcceptedContentRootTarget),
}

pub enum ProjectGraphDependencyRelationKind {
    CallsCallable,
    ReferencesEntity,
    ContainsContentRoot,
    ResolvesContentRoot,
}
```

Relations are:

```text
ContentUnit --ContainsContentRoot--> ContentRootOccurrence
ContentRootOccurrence --ResolvesContentRoot--> ContentRootTarget
```

The second relation exists only for a present target. An accepted optional
absence is represented by `ProjectContentRootFact::state` and never by a fake
target symbol.

The old PublicId-to-PublicId `ProjectGraphRelationKind::ContentRoot` path owned
by source `content` is deleted rather than fed manifest data through a
compatibility entity.

## 8. Compiler identity

`ProgramHash` receives an inherent constructor/projection that includes the
accepted topology revision:

```rust
impl ProgramHash {
    pub fn for_accepted_project(
        compiler: &CompilerBuildIdentity,
        topology: ProjectTopologyRevision,
        checked_program: &CheckedProgramDigest,
    ) -> Self;
}
```

Consumers do not create cache keys by formatting strings. The compiler receives
the same `Arc<AcceptedContentInventory>` used by ProjectIndex and partition
planning.

## 9. Loader and Character owners

The existing text/binary payload split, `CharacterPackage`, and exact accessors
remain unchanged. `LoadedProfileTopology` gains the candidate before semantic
finalization and the accepted inventory only after the atomic final stage:

```rust
pub struct LoadedProfileTopology {
    // existing exact manifest/profile/layout/source/metadata/resources
    content_candidate: Arc<ContentAdmissionCandidate>,
    watch_inventory: Arc<[ProfileTopologyWatchEntry]>,
}

pub struct AcceptedProfileTopology {
    loaded: Arc<LoadedProfileTopology>,
    content: Arc<AcceptedContentInventory>,
    project_index: Arc<ProjectSemanticIndex>,
    program_hash: ProgramHash,
}
```

`AcceptedProfileTopology` is the exact post-finalize loader/compiler boundary.
It is constructed only after semantic finalization and consumer-independent
identity checks succeed. A failed candidate cannot be exposed as
`AcceptedProfileTopology`.

## 10. Dependency direction

```text
arcweft-manifest-model / arcweft-source / arcweft-id
               |                    |
        arcweft-launch       arcweft-character
               |                    |
               +----> arcweft-project::content <---- arcweft-resource-model
                               |
                     arcweft-project-loader
                               |
                      arcweft-lang-sema
                               |
                       arcweft-compiler
                               |
              bundle / watch / LSP / CLI adapters
```

No core/data-format owner performs I/O. Only project-loader and host adapters
open files or maintain watches. No lower-level crate depends on LSP, CLI,
filesystem, or platform adapters.
