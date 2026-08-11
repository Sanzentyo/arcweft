# Final Rust shapes

The following shapes are normative. Field order is normative for source review
and constructor validation; these values are internal typed products and do not
create a new persisted wire format.

## 1. `arcweft-project::content`

```rust
use std::{collections::BTreeMap, sync::Arc};

use arcweft_character::id::CharacterId;
use arcweft_lang_syntax::ast::module_path::CanonicalModulePath;
use arcweft_manifest_model::{
    ActivityId, ContentRootRef, ContentUnitId, DependencyDemand,
    ManifestVisibility, PackageId, PackageVersion, ProfileContentSpec, ProfileId,
};
use arcweft_resource_model::identity::ResourceDeclarationIdentity;
use arcweft_source::SourceSpan;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ContentRootFamily {
    Character,
    Resource,
    Activity,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AcceptedContentRootTarget {
    Character(CharacterId),
    Resource(ResourceDeclarationIdentity),
    Activity(ActivityId),
}

impl AcceptedContentRootTarget {
    pub const fn family(&self) -> ContentRootFamily {
        match self {
            Self::Character(_) => ContentRootFamily::Character,
            Self::Resource(_) => ContentRootFamily::Resource,
            Self::Activity(_) => ContentRootFamily::Activity,
        }
    }

    pub const fn is_file_backed(&self) -> bool {
        matches!(self, Self::Character(_))
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AcceptedContentRootFactId {
    content_unit: ContentUnitId,
    root_ordinal: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AcceptedContentRootPresence {
    Present,
    AbsentOptional(ProjectTopologyAbsenceRecord),
}

impl AcceptedContentRootPresence {
    pub const fn absence(&self) -> Option<&ProjectTopologyAbsenceRecord> {
        match self {
            Self::Present => None,
            Self::AbsentOptional(record) => Some(record),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptedContentRootFact {
    id: AcceptedContentRootFactId,
    authored_root: ContentRootRef,
    target: AcceptedContentRootTarget,
    declaration_source: SourceSpan,
    presence: AcceptedContentRootPresence,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManifestContentUnitSource {
    unit_table: SourceSpan,
    visibility: SourceSpan,
    demand: SourceSpan,
    profile_table: SourceSpan,
    residency: SourceSpan,
    placement: SourceSpan,
    compression: SourceSpan,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptedContentUnitFact {
    id: ContentUnitId,
    roots: Box<[AcceptedContentRootFact]>,
    visibility: ManifestVisibility,
    demand: DependencyDemand,
    profile_policy: ProfileContentSpec,
    source: ManifestContentUnitSource,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AcceptedContentReferenceKind {
    SourceEntityReference,
    ResourceValueDependency,
    SelectedEntryReference,
    ActivityBinding,
    GeneratedMetadataExport,
    ManifestProfileReference,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptedContentRootReference {
    target: AcceptedContentRootTarget,
    source: SourceSpan,
    kind: AcceptedContentReferenceKind,
    source_module: Option<CanonicalModulePath>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptedProjectContent {
    package: PackageId,
    package_version: PackageVersion,
    profile: ProfileId,
    topology_revision: ProjectTopologyRevision,
    units: BTreeMap<ContentUnitId, AcceptedContentUnitFact>,
    references: Box<[AcceptedContentRootReference]>,
}
```

### Constructor and derive rules

- All fields remain private.
- `AcceptedProjectContent::try_new`, `AcceptedContentUnitFact::try_new`, and
  `AcceptedContentRootFact::try_new` are `pub(crate)` in `arcweft-project`.
- Read-only accessors and iterators are inherent methods on their owner types.
- Constructors validate non-empty root arrays, contiguous zero-based ordinals,
  unit/key equality, one profile policy per selected content unit, exact
  source-document identity, and presence/family coherence.
- `AbsentOptional` is legal only for `Character` and only under optional demand.
- The inspected `ResourceDeclarationIdentity` already implements `Ord` and
  `PartialOrd`; reuse that original owner directly. Do not create a wrapper or
  extension trait merely to sort content targets.
- `AcceptedProjectContent::try_new` sorts reference records by target, source
  document identity, byte range, reference kind, and source module. Duplicate
  identical evidence is collapsed; distinct ranges are retained.

## 2. `arcweft-launch::ManifestTokenPath`

Extend the existing owning enum and its inherent `source_key` conversion:

```rust
pub enum ManifestTokenPath {
    // Existing variants remain.
    ContentUnitTable {
        content_unit: ContentUnitId,
    },
    ContentUnitRoot {
        content_unit: ContentUnitId,
        root_ordinal: u32,
    },
    ContentUnitVisibility {
        content_unit: ContentUnitId,
    },
    ContentUnitDemand {
        content_unit: ContentUnitId,
    },
    ProfileContentTable {
        profile: ProfileId,
        content_unit: ContentUnitId,
    },
    ProfileContentResidency {
        profile: ProfileId,
        content_unit: ContentUnitId,
    },
    ProfileContentPlacement {
        profile: ProfileId,
        content_unit: ContentUnitId,
    },
    ProfileContentCompression {
        profile: ProfileId,
        content_unit: ContentUnitId,
    },
}
```

These variants map to the already-existing internal path segments
`ContentUnit`, `ContentUnitField`, `ProfileContent`, `ProfileContentField`, and
`Index`. No second map is added.

## 3. `arcweft-lang-sema::project_index::content`

```rust
use arcweft_id::PublicId;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ResolvedContentRootSymbol {
    Character(CharacterId),
    Resource(ResourceDeclarationIdentity),
    Activity(ActivityId),
    Entity {
        public_id: PublicId,
        kind: EntityKind,
        declaration: SourceAnchor,
    },
    Callable {
        declaration: CallableDeclarationId,
        source: SourceAnchor,
    },
}

impl ResolvedContentRootSymbol {
    pub(crate) fn into_accepted_target(
        self,
    ) -> Result<AcceptedContentRootTarget, ResolvedWrongContentRootFamily> {
        match self {
            Self::Character(id) => Ok(AcceptedContentRootTarget::Character(id)),
            Self::Resource(id) => Ok(AcceptedContentRootTarget::Resource(id)),
            Self::Activity(id) => Ok(AcceptedContentRootTarget::Activity(id)),
            Self::Entity { public_id, kind, declaration } => {
                Err(ResolvedWrongContentRootFamily::Entity {
                    public_id,
                    kind,
                    declaration,
                })
            }
            Self::Callable { declaration, source } => {
                Err(ResolvedWrongContentRootFamily::Callable {
                    declaration,
                    source,
                })
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ProjectContentReferenceCandidate {
    reference: EntityIdRef,
    source: SourceSpan,
    kind: AcceptedContentReferenceKind,
    source_module: Option<CanonicalModulePath>,
}

pub(crate) struct ContentRootResolver<'a> {
    symbols: &'a ProjectSymbolTable,
    resources: &'a RegisteredResourceCatalog,
    characters: &'a RegisteredCharacterCatalog,
    activities: &'a RegisteredActivityCatalog,
    limits: ContentAdmissionLimits,
}
```

`ContentRootResolver::resolve` uses existing typed lookup, alias, import,
re-export, collision, and visibility APIs. The family conversion is an inherent
method on `ResolvedContentRootSymbol`; no free `match` helper or extension
trait is allowed.

## 4. Final `ProjectSemanticIndex`

```rust
pub struct ProjectSemanticIndex {
    schema_version: u32,
    program_hash: ProgramHash,
    bundle_hash: Option<BundleHash>,
    // Existing entity/callable/nominal/type/debug/relation fields remain.
    accepted_content: Arc<AcceptedProjectContent>,
}

impl ProjectSemanticIndex {
    pub(crate) fn new(
        program_hash: ProgramHash,
        accepted_content: Arc<AcceptedProjectContent>,
    ) -> Self;

    pub fn accepted_content(&self) -> &Arc<AcceptedProjectContent>;

    pub fn topology_revision(&self) -> ProjectTopologyRevision;

    pub fn content_unit(
        &self,
        id: &ContentUnitId,
    ) -> Option<&AcceptedContentUnitFact>;

    pub fn content_references(
        &self,
        target: &AcceptedContentRootTarget,
    ) -> impl Iterator<Item = &AcceptedContentRootReference>;
}
```

`ProjectGraphRelationKind::ContentRoot` is removed. Manifest-owned facts are
not copied into the general relation vector.

## 5. `arcweft-project-loader::profile_topology`

```rust
#[derive(Clone, Debug, Default)]
pub struct ProfileTopologyOverlaySet {
    text: Arc<[ProfileTopologyOverlaySeed]>,
    binary: Arc<[ProfileTopologyBinaryOverlaySeed]>,
}

impl ProfileTopologyOverlaySet {
    pub fn try_new(
        text: impl IntoIterator<Item = ProfileTopologyOverlaySeed>,
        binary: impl IntoIterator<Item = ProfileTopologyBinaryOverlaySeed>,
    ) -> Result<Self, ProfileTopologyOverlaySetError>;

    pub fn text(&self) -> &[ProfileTopologyOverlaySeed];
    pub fn binary(&self) -> &[ProfileTopologyBinaryOverlaySeed];
}

pub struct LoadedProfileTopology {
    // Existing fields remain.
    topology_revision: ProjectTopologyRevision,
}

pub struct LoadedCharacterPackage {
    package: Arc<CharacterPackage>,
    source_manifest: Arc<SourceBackedCharacterManifest>,
    logical_package_root: NormalizedProjectPath,
    logical_manifest_path: NormalizedProjectPath,
    logical_layer_paths: BTreeMap<CharacterAssetPath, NormalizedProjectPath>,
    host_package_root: PathBuf,
    host_manifest_path: PathBuf,
    host_layer_paths: BTreeMap<CharacterAssetPath, PathBuf>,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProfileTopologyWatchTarget {
    Resource(ProfileTopologyResourceId),
    OptionalCharacterManifest {
        content_unit: ContentUnitId,
        root_ordinal: u32,
        character: CharacterId,
    },
}

pub struct ProfileTopologyWatchEntry {
    target: ProfileTopologyWatchTarget,
    host_path: PathBuf,
    kind: ProfileTopologyResourceKind,
    expectation: ProfileTopologyWatchExpectation,
}

#[derive(Clone, Debug)]
pub struct AcceptedProfileProject {
    topology: Arc<LoadedProfileTopology>,
    project_index: Arc<ProjectSemanticIndex>,
}

impl AcceptedProfileProject {
    pub(crate) fn try_new(
        topology: Arc<LoadedProfileTopology>,
        project_index: Arc<ProjectSemanticIndex>,
    ) -> Result<Self, AcceptedProfileProjectError>;

    pub fn topology(&self) -> &Arc<LoadedProfileTopology>;
    pub fn project_index(&self) -> &Arc<ProjectSemanticIndex>;
    pub fn revision(&self) -> ProjectTopologyRevision;
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum AcceptedProfileProjectError {
    #[error(transparent)]
    IdentityMismatch(#[from] AcceptedProjectIdentityMismatch),
    #[error(transparent)]
    Invariant(#[from] AcceptedProfileProjectInvariant),
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum AcceptedProjectIdentityMismatch {
    #[error("topology and project index package IDs differ")]
    PackageId { topology: PackageId, project_index: PackageId },
    #[error("topology and project index package versions differ")]
    PackageVersion { topology: PackageVersion, project_index: PackageVersion },
    #[error("topology and project index profile IDs differ")]
    Profile { topology: ProfileId, project_index: ProfileId },
    #[error("topology and project index topology revisions differ")]
    TopologyRevision {
        topology: ProjectTopologyRevision,
        project_index: ProjectTopologyRevision,
    },
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum AcceptedProfileProjectInvariant {
    #[error("present Character `{character}` has no loaded package")]
    PresentCharacterPackageMissing { character: CharacterId },
    #[error("absent Character `{character}` unexpectedly has a loaded package")]
    AbsentCharacterPackagePresent {
        fact: AcceptedContentRootFactId,
        character: CharacterId,
    },
    #[error("loaded Character `{character}` is not referenced by a present root fact")]
    UnreferencedCharacterPackage { character: CharacterId },
    #[error("Character `{character}` is missing topology resource `{path}`")]
    CharacterTopologyResourceMissing {
        character: CharacterId,
        path: NormalizedProjectPath,
    },
    #[error("accepted optional absence is missing from the revision transcript")]
    AbsenceRevisionRecordMissing { fact: AcceptedContentRootFactId },
    #[error("accepted content source span belongs to the wrong manifest document")]
    ManifestSourceIdentityMismatch {
        expected: arcweft_source::SourceDocumentIdentity,
        actual: arcweft_source::SourceDocumentIdentity,
    },
}
```

Logical and host paths are deliberately distinct responsibilities; this is not
a duplicate identity authority. Bundle uses logical paths, host watcher/LSP URI
adaptation uses host paths, and `CharacterPackage` owns exact bytes.

## 6. Admission diagnostics

```rust
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ContentAdmissionError {
    #[error("manifest source evidence is missing for content unit `{content_unit}`")]
    ManifestSourceEvidenceMissing {
        content_unit: ContentUnitId,
        path: ManifestTokenPath,
    },
    #[error("unknown content root `{authored}`")]
    UnknownContentRoot {
        authored: ContentRootRef,
        primary: SourceSpan,
    },
    #[error("ambiguous content root `{authored}`")]
    AmbiguousContentRoot {
        authored: ContentRootRef,
        primary: SourceSpan,
        candidates: Box<[ContentRootCandidateEvidence]>,
    },
    #[error("content root `{authored}` is not visible")]
    InvisibleContentRoot {
        authored: ContentRootRef,
        primary: SourceSpan,
        declaration: SourceSpan,
    },
    #[error("content root `{authored}` resolves to unsupported family `{actual}`")]
    WrongContentRootFamily {
        authored: ContentRootRef,
        primary: SourceSpan,
        actual: ResolvedContentRootFamily,
        declaration: Option<SourceSpan>,
    },
    #[error("required content root `{authored}` is absent")]
    RequiredContentRootAbsent {
        fact: AcceptedContentRootFactId,
        authored: ContentRootRef,
        character: CharacterId,
        expected_manifest: NormalizedProjectPath,
        primary: SourceSpan,
        demand: SourceSpan,
    },
    #[error("optional content root `{authored}` is absent but referenced")]
    ReferencedOptionalContentRootAbsent {
        fact: AcceptedContentRootFactId,
        authored: ContentRootRef,
        character: CharacterId,
        expected_manifest: NormalizedProjectPath,
        primary: SourceSpan,
        references: Box<[AcceptedContentRootReference]>,
    },
    #[error(transparent)]
    CharacterPackage(#[from] CharacterPackageAdmissionError),
}
```

Diagnostics expose stable code strings listed in
`DIAGNOSTICS_AND_FAILURE_PRECEDENCE.md`; display text is not diagnostic
identity.
