use super::{ProfileTopologyLogicalPath, ProfileTopologyOwnerId, ProfileTopologyResourceId};
use crate::{
    layout::{ContainedProjectLayout, ProjectLayoutContainmentError},
    project,
};
use arcweft_adapter_context::manifest::{
    AdapterCallableModelError, AdapterManifest, AdapterRegistry, AdapterSymbolPathError,
};
use arcweft_adapter_metadata::SourceBackedAdapterMetadata;
use arcweft_lang_sema::registration::CharacterDefinitionLimits;
use arcweft_lang_syntax::ast::module_path::CanonicalModulePath;
use arcweft_launch::{
    LaunchProfileSelection, accepted::SourceBackedManifest, diagnostic::ManifestReport,
    resolve::ResolvedLaunchProfile,
};
use arcweft_manifest_model::{
    ActivityId, AdapterExportId, ExternalModuleImportId, ExternalModuleImportSpec, PackageId,
    PackageVersion,
};
use arcweft_project::content::ProjectBinaryResource;
use arcweft_project::layout::ProjectLayoutSpec;
use arcweft_resource_manifest::PublishedResourceTypeManifestSetV1;
use arcweft_resource_model::registry::ResourceTypeRegistry;
use arcweft_source::{
    MAX_REGISTRATION_SOURCE_BYTES, SourceDocument, SourceDocumentId, SourceSetRevision, SourceSpan,
};
use std::{
    collections::BTreeMap,
    path::{Component, Path, PathBuf},
    sync::Arc,
};
use thiserror::Error;

/// Ownership class exposed to registration and LSP consumers.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum LoadedDocumentOwnership {
    Workspace,
    Dependency,
}

/// Host-observed access class for one retained document.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum LoadedDocumentAccess {
    Writable,
    ReadOnly,
    Unknown,
}

/// Semantic role of one topology resource.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProfileTopologyResourceKind {
    Manifest,
    ArcweftModule {
        module: CanonicalModulePath,
    },
    CharacterPackageManifest {
        character: arcweft_character::id::CharacterId,
    },
    CharacterLayerPayload {
        character: arcweft_character::id::CharacterId,
        asset: arcweft_character::manifest::CharacterAssetPath,
    },
    ExternalModuleMetadata {
        import: ExternalModuleImportId,
    },
    ResourceTypeManifest {
        package_id: PackageId,
        package_version: PackageVersion,
    },
}

/// Authority that supplied the exact retained bytes.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProfileTopologyResourceOrigin {
    Disk,
    Overlay,
}

/// One immutable source-backed topology resource.
#[derive(Clone, Debug)]
pub struct LoadedProfileTopologyResource {
    pub(super) id: ProfileTopologyResourceId,
    pub(super) kind: ProfileTopologyResourceKind,
    pub(super) path: PathBuf,
    pub(super) payload: LoadedProfileTopologyResourcePayload,
    pub(super) ownership: LoadedDocumentOwnership,
    pub(super) access: LoadedDocumentAccess,
    pub(super) origin: ProfileTopologyResourceOrigin,
}

/// Exact text or binary payload retained for one topology resource.
#[derive(Clone, Debug)]
pub enum LoadedProfileTopologyResourcePayload {
    Text(Arc<SourceDocument>),
    Binary(Arc<ProjectBinaryResource>),
}

impl LoadedProfileTopologyResource {
    pub const fn id(&self) -> &ProfileTopologyResourceId {
        &self.id
    }

    pub const fn kind(&self) -> &ProfileTopologyResourceKind {
        &self.kind
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub const fn text_document(&self) -> Option<&Arc<SourceDocument>> {
        match &self.payload {
            LoadedProfileTopologyResourcePayload::Text(document) => Some(document),
            LoadedProfileTopologyResourcePayload::Binary(_) => None,
        }
    }

    pub const fn binary_resource(&self) -> Option<&Arc<ProjectBinaryResource>> {
        match &self.payload {
            LoadedProfileTopologyResourcePayload::Text(_) => None,
            LoadedProfileTopologyResourcePayload::Binary(resource) => Some(resource),
        }
    }

    pub const fn ownership(&self) -> LoadedDocumentOwnership {
        self.ownership
    }

    pub const fn access(&self) -> LoadedDocumentAccess {
        self.access
    }

    pub const fn origin(&self) -> ProfileTopologyResourceOrigin {
        self.origin
    }
}

/// Binary bytes supplied independently of text-document overlays.
#[derive(Clone, Debug)]
pub struct ProfileTopologyBinaryOverlaySeed {
    path: PathBuf,
    bytes: Arc<[u8]>,
}

impl ProfileTopologyBinaryOverlaySeed {
    pub fn try_new(
        path: impl Into<PathBuf>,
        bytes: impl Into<Arc<[u8]>>,
    ) -> Result<Self, ProfileTopologySeedError> {
        let path = path.into();
        validate_absolute_normalized_path(&path, "binary overlay path")?;
        Ok(Self {
            path,
            bytes: bytes.into(),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub const fn bytes(&self) -> &Arc<[u8]> {
        &self.bytes
    }
}

/// Open-document bytes supplied to the protocol-free topology loader.
#[derive(Clone, Debug)]
pub struct ProfileTopologyOverlaySeed {
    path: PathBuf,
    source: Arc<str>,
}

impl ProfileTopologyOverlaySeed {
    pub fn try_new(
        path: impl Into<PathBuf>,
        source: impl Into<Arc<str>>,
    ) -> Result<Self, ProfileTopologySeedError> {
        let path = path.into();
        validate_absolute_normalized_path(&path, "overlay path")?;
        Ok(Self {
            path,
            source: source.into(),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn source(&self) -> &Arc<str> {
        &self.source
    }
}

/// Explicit dependency-owned resource supplied by a dependency resolver.
#[derive(Clone, Debug)]
pub struct ProfileDependencyResourceSeed {
    id: ProfileTopologyResourceId,
    kind: ProfileTopologyResourceKind,
    root: PathBuf,
    path: PathBuf,
    source_id: SourceDocumentId,
}

/// Explicit dependency-owned binary resource supplied by a dependency resolver.
#[derive(Clone, Debug)]
pub struct ProfileDependencyBinaryResourceSeed {
    id: ProfileTopologyResourceId,
    kind: ProfileTopologyResourceKind,
    root: PathBuf,
    path: PathBuf,
}

impl ProfileDependencyBinaryResourceSeed {
    pub fn try_new(
        id: ProfileTopologyResourceId,
        kind: ProfileTopologyResourceKind,
        root: impl Into<PathBuf>,
        path: impl Into<PathBuf>,
    ) -> Result<Self, ProfileTopologySeedError> {
        if !matches!(id.owner(), ProfileTopologyOwnerId::Dependency { .. }) {
            return Err(ProfileTopologySeedError::DependencyOwnerRequired);
        }
        if !matches!(
            kind,
            ProfileTopologyResourceKind::CharacterLayerPayload { .. }
        ) {
            return Err(ProfileTopologySeedError::BinaryKindRequired);
        }
        let root = root.into();
        let path = path.into();
        validate_absolute_normalized_path(&root, "binary dependency root")?;
        validate_absolute_normalized_path(&path, "binary dependency path")?;
        let relative =
            path.strip_prefix(&root)
                .map_err(|_| ProfileTopologySeedError::OutsideRoot {
                    root: root.clone(),
                    path: path.clone(),
                })?;
        let logical = slash_relative_path(relative)?;
        if logical != id.path().as_str() {
            return Err(ProfileTopologySeedError::LogicalPathMismatch {
                expected: id.path().as_str().to_owned(),
                actual: logical,
            });
        }
        Ok(Self {
            id,
            kind,
            root,
            path,
        })
    }

    pub const fn id(&self) -> &ProfileTopologyResourceId {
        &self.id
    }

    pub const fn kind(&self) -> &ProfileTopologyResourceKind {
        &self.kind
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl ProfileDependencyResourceSeed {
    pub fn try_new(
        id: ProfileTopologyResourceId,
        kind: ProfileTopologyResourceKind,
        root: impl Into<PathBuf>,
        path: impl Into<PathBuf>,
        source_id: SourceDocumentId,
    ) -> Result<Self, ProfileTopologySeedError> {
        if !matches!(id.owner(), ProfileTopologyOwnerId::Dependency { .. }) {
            return Err(ProfileTopologySeedError::DependencyOwnerRequired);
        }
        let root = root.into();
        let path = path.into();
        validate_absolute_normalized_path(&root, "dependency root")?;
        validate_absolute_normalized_path(&path, "dependency path")?;
        let relative =
            path.strip_prefix(&root)
                .map_err(|_| ProfileTopologySeedError::OutsideRoot {
                    root: root.clone(),
                    path: path.clone(),
                })?;
        let logical = slash_relative_path(relative)?;
        if logical != id.path().as_str() {
            return Err(ProfileTopologySeedError::LogicalPathMismatch {
                expected: id.path().as_str().to_owned(),
                actual: logical,
            });
        }
        Ok(Self {
            id,
            kind,
            root,
            path,
            source_id,
        })
    }

    pub const fn id(&self) -> &ProfileTopologyResourceId {
        &self.id
    }

    pub const fn kind(&self) -> &ProfileTopologyResourceKind {
        &self.kind
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub const fn source_id(&self) -> &SourceDocumentId {
        &self.source_id
    }
}

/// Complete input for one immutable profile-topology load.
pub struct ProfileTopologyLoadRequest<'a> {
    pub(super) manifest_path: &'a Path,
    pub(super) workspace_owner: ProfileTopologyOwnerId,
    pub(super) selection: LaunchProfileSelection<'a>,
    pub(super) overlays: &'a [ProfileTopologyOverlaySeed],
    pub(super) binary_overlays: &'a [ProfileTopologyBinaryOverlaySeed],
    pub(super) dependency_resources: &'a [ProfileDependencyResourceSeed],
    pub(super) dependency_binary_resources: &'a [ProfileDependencyBinaryResourceSeed],
    pub(super) base_adapters: AdapterRegistry,
    pub(super) base_resource_types: Arc<ResourceTypeRegistry>,
    pub(super) layout: ProjectLayoutSpec,
}

impl<'a> ProfileTopologyLoadRequest<'a> {
    pub fn new(
        manifest_path: &'a Path,
        workspace_owner: ProfileTopologyOwnerId,
        selection: LaunchProfileSelection<'a>,
        overlays: &'a [ProfileTopologyOverlaySeed],
        base_adapters: AdapterRegistry,
        base_resource_types: Arc<ResourceTypeRegistry>,
    ) -> Self {
        Self {
            manifest_path,
            workspace_owner,
            selection,
            overlays,
            binary_overlays: &[],
            dependency_resources: &[],
            dependency_binary_resources: &[],
            base_adapters,
            base_resource_types,
            layout: ProjectLayoutSpec::default(),
        }
    }

    #[must_use]
    pub fn with_binary_overlays(
        mut self,
        overlays: &'a [ProfileTopologyBinaryOverlaySeed],
    ) -> Self {
        self.binary_overlays = overlays;
        self
    }

    #[must_use]
    pub fn with_dependency_resources(
        mut self,
        resources: &'a [ProfileDependencyResourceSeed],
    ) -> Self {
        self.dependency_resources = resources;
        self
    }

    #[must_use]
    pub fn with_dependency_binary_resources(
        mut self,
        resources: &'a [ProfileDependencyBinaryResourceSeed],
    ) -> Self {
        self.dependency_binary_resources = resources;
        self
    }

    #[must_use]
    pub fn with_layout(mut self, layout: ProjectLayoutSpec) -> Self {
        self.layout = layout;
        self
    }
}

/// Complete immutable product of one bounded topology transaction.
#[derive(Clone, Debug)]
pub struct LoadedProfileTopology {
    loaded_project: project::LoadedProject,
    manifest: Arc<SourceBackedManifest>,
    selected_profile: ResolvedLaunchProfile,
    layout: ContainedProjectLayout,
    external_modules: Arc<[LoadedExternalModuleMetadata]>,
    adapter: AdapterManifest,
    resource_type_manifests: PublishedResourceTypeManifestSetV1,
    registration_adapter_manifests: Arc<[AdapterManifest]>,
    resources: BTreeMap<ProfileTopologyResourceId, LoadedProfileTopologyResource>,
    character_packages: BTreeMap<arcweft_character::id::CharacterId, LoadedCharacterPackage>,
    consumed_overlay_ids: Arc<[ProfileTopologyResourceId]>,
    source_documents_revision: SourceSetRevision,
    watch_inventory: Arc<[ProfileTopologyWatchEntry]>,
    work: u64,
}

/// One complete loaded Character package and its source provenance.
#[derive(Clone, Debug)]
pub struct LoadedCharacterPackage {
    package: Arc<arcweft_character::package::CharacterPackage>,
    source_manifest: Arc<arcweft_character::manifest::registration::SourceBackedCharacterManifest>,
    package_root: PathBuf,
    manifest_path: PathBuf,
    layer_paths: BTreeMap<arcweft_character::manifest::CharacterAssetPath, PathBuf>,
}

/// Presence expectation for one exact topology watch path.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProfileTopologyWatchExpectation {
    MustExist,
    OptionalMayAppear,
}

/// Exact retained path consumed by host watcher adapters.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProfileTopologyWatchEntry {
    id: ProfileTopologyResourceId,
    host_path: PathBuf,
    kind: ProfileTopologyResourceKind,
    expectation: ProfileTopologyWatchExpectation,
}

/// One selected generated-module document after exact hash and identity checks.
#[derive(Clone, Debug)]
pub struct LoadedExternalModuleMetadata {
    import_id: ExternalModuleImportId,
    import: ExternalModuleImportSpec,
    document: Arc<SourceDocument>,
    metadata: SourceBackedAdapterMetadata,
}

impl LoadedExternalModuleMetadata {
    pub(super) fn new(
        import_id: ExternalModuleImportId,
        import: ExternalModuleImportSpec,
        document: Arc<SourceDocument>,
        metadata: SourceBackedAdapterMetadata,
    ) -> Self {
        Self {
            import_id,
            import,
            document,
            metadata,
        }
    }

    pub const fn import_id(&self) -> &ExternalModuleImportId {
        &self.import_id
    }

    pub const fn import(&self) -> &ExternalModuleImportSpec {
        &self.import
    }

    pub const fn document(&self) -> &Arc<SourceDocument> {
        &self.document
    }

    pub const fn metadata(&self) -> &SourceBackedAdapterMetadata {
        &self.metadata
    }
}

impl LoadedProfileTopology {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        loaded_project: project::LoadedProject,
        manifest: Arc<SourceBackedManifest>,
        selected_profile: ResolvedLaunchProfile,
        layout: ContainedProjectLayout,
        external_modules: Vec<LoadedExternalModuleMetadata>,
        adapter: AdapterManifest,
        resource_type_manifests: PublishedResourceTypeManifestSetV1,
        resources: BTreeMap<ProfileTopologyResourceId, LoadedProfileTopologyResource>,
        character_packages: BTreeMap<arcweft_character::id::CharacterId, LoadedCharacterPackage>,
        consumed_overlay_ids: Vec<ProfileTopologyResourceId>,
        source_documents_revision: SourceSetRevision,
        work: u64,
    ) -> Self {
        let registration_adapter_manifests = Arc::from([adapter.clone()]);
        let watch_inventory = resources
            .values()
            .map(|resource| ProfileTopologyWatchEntry {
                id: resource.id.clone(),
                host_path: resource.path.clone(),
                kind: resource.kind.clone(),
                expectation: ProfileTopologyWatchExpectation::MustExist,
            })
            .collect::<Vec<_>>()
            .into();
        Self {
            loaded_project,
            manifest,
            selected_profile,
            layout,
            external_modules: external_modules.into(),
            adapter,
            resource_type_manifests,
            registration_adapter_manifests,
            resources,
            character_packages,
            consumed_overlay_ids: consumed_overlay_ids.into(),
            source_documents_revision,
            watch_inventory,
            work,
        }
    }

    pub const fn loaded_project(&self) -> &project::LoadedProject {
        &self.loaded_project
    }

    pub const fn selected_profile(&self) -> &ResolvedLaunchProfile {
        &self.selected_profile
    }

    pub const fn manifest(&self) -> &Arc<SourceBackedManifest> {
        &self.manifest
    }

    pub const fn layout(&self) -> &ContainedProjectLayout {
        &self.layout
    }

    pub fn external_modules(&self) -> &[LoadedExternalModuleMetadata] {
        &self.external_modules
    }

    pub const fn adapter(&self) -> &AdapterManifest {
        &self.adapter
    }

    pub fn registration_adapter_manifests(&self) -> &[AdapterManifest] {
        &self.registration_adapter_manifests
    }

    pub const fn resource_type_manifests(&self) -> &PublishedResourceTypeManifestSetV1 {
        &self.resource_type_manifests
    }

    pub const fn resource_types(&self) -> &Arc<ResourceTypeRegistry> {
        self.resource_type_manifests.registry()
    }

    pub fn resources(&self) -> impl ExactSizeIterator<Item = &LoadedProfileTopologyResource> {
        self.resources.values()
    }

    pub fn resource(
        &self,
        id: &ProfileTopologyResourceId,
    ) -> Option<&LoadedProfileTopologyResource> {
        self.resources.get(id)
    }

    pub fn character_packages(
        &self,
    ) -> impl ExactSizeIterator<Item = (&arcweft_character::id::CharacterId, &LoadedCharacterPackage)>
    {
        self.character_packages.iter()
    }

    pub fn character_package(
        &self,
        character: &arcweft_character::id::CharacterId,
    ) -> Option<&LoadedCharacterPackage> {
        self.character_packages.get(character)
    }

    pub fn consumed_overlay_ids(
        &self,
    ) -> impl ExactSizeIterator<Item = &ProfileTopologyResourceId> {
        self.consumed_overlay_ids.iter()
    }

    pub const fn source_documents_revision(&self) -> SourceSetRevision {
        self.source_documents_revision
    }

    pub fn watch_inventory(&self) -> &[ProfileTopologyWatchEntry] {
        &self.watch_inventory
    }

    pub const fn work(&self) -> u64 {
        self.work
    }
}

impl LoadedCharacterPackage {
    pub(super) fn new(
        package: Arc<arcweft_character::package::CharacterPackage>,
        source_manifest: Arc<
            arcweft_character::manifest::registration::SourceBackedCharacterManifest,
        >,
        package_root: PathBuf,
        manifest_path: PathBuf,
        layer_paths: BTreeMap<arcweft_character::manifest::CharacterAssetPath, PathBuf>,
    ) -> Self {
        Self {
            package,
            source_manifest,
            package_root,
            manifest_path,
            layer_paths,
        }
    }

    pub const fn package(&self) -> &Arc<arcweft_character::package::CharacterPackage> {
        &self.package
    }

    pub const fn source_manifest(
        &self,
    ) -> &Arc<arcweft_character::manifest::registration::SourceBackedCharacterManifest> {
        &self.source_manifest
    }

    pub fn package_root(&self) -> &Path {
        &self.package_root
    }

    pub fn manifest_path(&self) -> &Path {
        &self.manifest_path
    }

    pub fn layer_paths(
        &self,
    ) -> impl ExactSizeIterator<Item = (&arcweft_character::manifest::CharacterAssetPath, &PathBuf)>
    {
        self.layer_paths.iter()
    }
}

impl ProfileTopologyWatchEntry {
    pub const fn id(&self) -> &ProfileTopologyResourceId {
        &self.id
    }

    pub fn host_path(&self) -> &Path {
        &self.host_path
    }

    pub const fn kind(&self) -> &ProfileTopologyResourceKind {
        &self.kind
    }

    pub const fn expectation(&self) -> ProfileTopologyWatchExpectation {
        self.expectation
    }
}

/// Fixed production limits for one topology transaction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProfileTopologyLimits;

impl ProfileTopologyLimits {
    pub const PRODUCTION: Self = Self;

    pub const fn resources(self) -> u64 {
        CharacterDefinitionLimits::PRODUCTION.documents() - 1
    }

    pub const fn source_bytes(self) -> u64 {
        MAX_REGISTRATION_SOURCE_BYTES
    }

    pub const fn overlay_bytes(self) -> u64 {
        MAX_REGISTRATION_SOURCE_BYTES
    }

    pub const fn diagnostics(self) -> u64 {
        CharacterDefinitionLimits::PRODUCTION.diagnostics()
    }

    pub const fn work(self) -> u64 {
        CharacterDefinitionLimits::PRODUCTION.build_work()
    }
}

/// Counter whose inclusive topology limit was exceeded.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProfileTopologyLimitKind {
    Resources,
    SourceBytes,
    OverlayBytes,
    Diagnostics,
    Work,
}

/// Stable machine-readable class of a topology load failure.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProfileTopologyErrorCode {
    ManifestNotFound,
    ResourceRead,
    ResourceUtf8,
    ResourceTypeManifest,
    Manifest,
    ProjectLayout,
    ProfileSelection,
    UnownedResourcePath,
    DuplicateLogicalId,
    DuplicatePath,
    ModuleSyntax,
    ModuleDeclaration,
    ModuleImport,
    CharacterManifest,
    AdapterSelection,
    ExternalModuleMetadata,
    DependencySeed,
    Limit,
    ArithmeticOverflow,
}

/// Invalid overlay or dependency seed.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ProfileTopologySeedError {
    #[error("{field} `{path}` must be absolute and lexically normalized")]
    Path { field: &'static str, path: PathBuf },
    #[error("dependency resource IDs must have dependency ownership")]
    DependencyOwnerRequired,
    #[error("dependency path `{path}` is outside root `{root}`")]
    OutsideRoot { root: PathBuf, path: PathBuf },
    #[error("dependency logical path mismatch: expected `{expected}`, got `{actual}`")]
    LogicalPathMismatch { expected: String, actual: String },
    #[error("dependency relative path `{path}` is not valid UTF-8")]
    NonUtf8 { path: PathBuf },
    #[error("topology path `{path}` cannot be represented as a logical resource path: {source}")]
    LogicalPath {
        path: PathBuf,
        #[source]
        source: super::ProfileTopologyPathError,
    },
    #[error("overlay path `{path}` occurs more than once")]
    DuplicateOverlayPath { path: PathBuf },
    #[error("dependency seed path `{path}` and role occur more than once")]
    DuplicateDependencySeed { path: PathBuf },
    #[error("the same overlay path `{path}` was supplied as both text and binary")]
    OverlayKindConflict { path: PathBuf },
    #[error("binary dependency resource kind must be a Character layer payload")]
    BinaryKindRequired,
    #[error("binary overlay `{path}` was not consumed by an exact Character layer")]
    UnconsumedBinaryOverlay { path: PathBuf },
}

/// Failure while projecting accepted generated metadata into mounted project facts.
#[derive(Debug, Error)]
pub enum ExternalModuleFactsError {
    #[error(
        "Activity binding `{activity}` selects generated import `{import}` that was not admitted"
    )]
    ActivityImportMissing {
        activity: ActivityId,
        import: ExternalModuleImportId,
    },
    #[error(
        "generated import `{import}` has an invalid mounted symbol for export `{export}`: {source}"
    )]
    Symbol {
        import: ExternalModuleImportId,
        export: String,
        #[source]
        source: AdapterSymbolPathError,
    },
    #[error(
        "generated import `{import}` has an invalid mounted nominal path for export `{export}`: {source}"
    )]
    NominalPath {
        import: ExternalModuleImportId,
        export: String,
        #[source]
        source: arcweft_adapter_context::manifest::AdapterNominalPathError,
    },
    #[error("generated import `{import}` could not register nominal export `{export}`: {source}")]
    NominalDeclaration {
        import: ExternalModuleImportId,
        export: String,
        #[source]
        source: Box<arcweft_adapter_context::manifest::AdapterManifestModelError>,
    },
    #[error(
        "generated import `{import}` has an invalid mounted callable for export `{export}`: {source}"
    )]
    Callable {
        import: ExternalModuleImportId,
        export: String,
        #[source]
        source: Box<AdapterCallableModelError>,
    },
    #[error(
        "generated import `{import}` export `{export}` has unsupported type reference `{reference}`"
    )]
    TypeReference {
        import: ExternalModuleImportId,
        export: String,
        reference: String,
    },
    #[error(
        "generated import `{import}` export `{export}` type reference exceeded its {kind:?} limit: observed {observed}, maximum {maximum}"
    )]
    TypeReferenceLimit {
        import: ExternalModuleImportId,
        export: String,
        kind: TypeReferenceLimitKind,
        observed: usize,
        maximum: usize,
    },
    #[error("generated mounted identity `{identity}` occurs more than once")]
    DuplicateMountedIdentity { identity: String },
    #[error(
        "generated import `{import}` function `{function}` declares purity `{purity}` inconsistent with its effects"
    )]
    FunctionPurity {
        import: ExternalModuleImportId,
        function: String,
        purity: &'static str,
    },
    #[error(
        "Activity binding `{activity}` selects missing export `{export}` from generated import `{import}`"
    )]
    ActivityExportMissing {
        activity: ActivityId,
        import: ExternalModuleImportId,
        export: AdapterExportId,
    },
    #[error(
        "Activity binding `{expected}` selects generated export `{export}` from import `{import}`, but that export declares `{actual}`"
    )]
    ActivityIdentityMismatch {
        import: ExternalModuleImportId,
        export: AdapterExportId,
        expected: ActivityId,
        actual: ActivityId,
    },
    #[error("generated module projection lost its adapter while {operation}")]
    ProjectionState { operation: &'static str },
}

impl ExternalModuleFactsError {
    pub(super) fn callable(
        import: &ExternalModuleImportId,
        export: &str,
        source: AdapterCallableModelError,
    ) -> Self {
        Self::Callable {
            import: import.clone(),
            export: export.to_owned(),
            source: Box::new(source),
        }
    }
}

/// Explicit resource limits for projecting one generated type reference.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TypeReferenceLimits {
    bytes: usize,
    nesting_depth: usize,
    work: usize,
}

impl TypeReferenceLimits {
    /// Production envelope for one generated callable type reference.
    pub const PRODUCTION: Self = Self::new(4_096, 64, 1_024);

    pub const fn new(bytes: usize, nesting_depth: usize, work: usize) -> Self {
        Self {
            bytes,
            nesting_depth,
            work,
        }
    }

    pub const fn bytes(self) -> usize {
        self.bytes
    }

    pub const fn nesting_depth(self) -> usize {
        self.nesting_depth
    }

    pub const fn work(self) -> usize {
        self.work
    }
}

/// Counter whose inclusive generated type-reference limit was exceeded.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TypeReferenceLimitKind {
    Bytes,
    NestingDepth,
    Work,
}

/// Fatal failure of one all-or-nothing topology transaction.
#[derive(Debug, Error)]
pub enum ProfileTopologyLoadError {
    #[error("primary launch manifest was not found at `{path}`")]
    ManifestNotFound { path: PathBuf },
    #[error("failed to read topology resource `{path}`: {source}")]
    ResourceRead {
        id: Box<ProfileTopologyResourceId>,
        kind: ProfileTopologyResourceKind,
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("topology resource `{path}` is not valid UTF-8")]
    ResourceUtf8 {
        id: Box<ProfileTopologyResourceId>,
        kind: ProfileTopologyResourceKind,
        path: PathBuf,
    },
    #[error("resource type manifest `{path}` is not valid UTF-8 at byte {offset}")]
    ResourceTypeManifestUtf8 {
        id: Box<ProfileTopologyResourceId>,
        path: PathBuf,
        offset: usize,
    },
    #[error(
        "selected resource type package `{package_id}` version `{package_version}` is unresolved at `{path}`"
    )]
    UnresolvedResourceTypePackage {
        id: Box<ProfileTopologyResourceId>,
        path: PathBuf,
        package_id: PackageId,
        package_version: PackageVersion,
    },
    #[error("failed to decode resource type manifest `{path}`: {source}")]
    ResourceTypeManifest {
        id: Box<ProfileTopologyResourceId>,
        path: PathBuf,
        #[source]
        source: arcweft_resource_manifest::ResourceManifestReport,
    },
    #[error("failed to publish the selected resource type manifests: {source}")]
    ResourceTypePublication {
        #[source]
        source: arcweft_resource_manifest::ResourceManifestReport,
    },
    #[error("failed to decode manifest `{path}`: {source}")]
    Manifest {
        id: Box<ProfileTopologyResourceId>,
        path: PathBuf,
        #[source]
        source: ManifestReport,
    },
    #[error("failed to select or resolve a launch profile: {source}")]
    ProfileSelection {
        #[source]
        source: ManifestReport,
    },
    #[error("failed to contain the project layout: {source}")]
    ProjectLayout {
        #[source]
        source: ProjectLayoutContainmentError,
    },
    #[error("topology path `{path}` is not owned by the workspace or an exact dependency seed")]
    UnownedResourcePath {
        path: PathBuf,
        kind: ProfileTopologyResourceKind,
    },
    #[error("topology logical ID occurs more than once")]
    DuplicateLogicalId {
        first: Box<LoadedProfileTopologyResource>,
        conflicting: Box<LoadedProfileTopologyResource>,
    },
    #[error("topology path is claimed by distinct resources")]
    DuplicatePath {
        first: Box<LoadedProfileTopologyResource>,
        conflicting: Box<LoadedProfileTopologyResource>,
    },
    #[error("module resource `{path}` has syntax errors: {diagnostics:?}")]
    ModuleSyntax {
        id: ProfileTopologyResourceId,
        path: PathBuf,
        diagnostics: Box<[String]>,
        truncated: bool,
    },
    #[error("module resource `{path}` has an invalid declaration: {source}")]
    ModuleDeclaration {
        id: ProfileTopologyResourceId,
        path: PathBuf,
        #[source]
        source: Box<project::ProjectLoadError>,
    },
    #[error("module `{module}` imports unresolved path `{import}` in `{path}`")]
    ModuleImport {
        id: Box<ProfileTopologyResourceId>,
        path: PathBuf,
        module: Box<CanonicalModulePath>,
        import: Box<str>,
        span: Option<Box<SourceSpan>>,
    },
    #[error("failed to decode character manifest `{path}`: {source}")]
    CharacterManifest {
        id: ProfileTopologyResourceId,
        path: PathBuf,
        #[source]
        source: Box<arcweft_character::manifest::diagnostic::CharacterRegistrationDecodeError>,
    },
    #[error("invalid character content root `{reference}`: {source}")]
    CharacterReference {
        reference: String,
        #[source]
        source: arcweft_character::id::CharacterIdError,
    },
    #[error("character package `{path}` declares `{actual}`, but content selected `{expected}`")]
    CharacterIdentityMismatch {
        path: PathBuf,
        expected: arcweft_character::id::CharacterId,
        actual: arcweft_character::id::CharacterId,
    },
    #[error("failed to construct complete Character package `{path}`: {source}")]
    CharacterPackage {
        path: PathBuf,
        #[source]
        source: Box<arcweft_character::package::CharacterPackageError>,
    },
    #[error("selected adapter `{id}` was not found in the complete checked registry")]
    AdapterSelection { id: String },
    #[error("generated metadata raw hash does not match import `{import}`")]
    ExternalModuleMetadataHash {
        import: ExternalModuleImportId,
        id: ProfileTopologyResourceId,
    },
    #[error("failed to decode generated metadata for import `{import}`: {source}")]
    ExternalModuleMetadataDecode {
        import: ExternalModuleImportId,
        id: ProfileTopologyResourceId,
        #[source]
        source: Box<arcweft_adapter_metadata::AdapterMetadataCodecError>,
    },
    #[error("generated metadata import `{import}` expected {field} `{expected}`, found `{actual}`")]
    ExternalModuleMetadataExpectation {
        import: ExternalModuleImportId,
        field: &'static str,
        expected: String,
        actual: String,
    },
    #[error(transparent)]
    ExternalModuleFacts(#[from] ExternalModuleFactsError),
    #[error("invalid dependency or overlay seed: {source}")]
    DependencySeed {
        #[source]
        source: ProfileTopologySeedError,
    },
    #[error("topology {kind:?} limit exceeded: observed {observed}, maximum {maximum}")]
    Limit {
        kind: ProfileTopologyLimitKind,
        observed: u64,
        maximum: u64,
    },
    #[error("topology {kind:?} counter overflowed")]
    ArithmeticOverflow { kind: ProfileTopologyLimitKind },
}

impl ProfileTopologyLoadError {
    pub const fn code(&self) -> ProfileTopologyErrorCode {
        match self {
            Self::ManifestNotFound { .. } => ProfileTopologyErrorCode::ManifestNotFound,
            Self::ResourceRead { .. } => ProfileTopologyErrorCode::ResourceRead,
            Self::ResourceUtf8 { .. } => ProfileTopologyErrorCode::ResourceUtf8,
            Self::ResourceTypeManifestUtf8 { .. }
            | Self::UnresolvedResourceTypePackage { .. }
            | Self::ResourceTypeManifest { .. }
            | Self::ResourceTypePublication { .. } => {
                ProfileTopologyErrorCode::ResourceTypeManifest
            }
            Self::Manifest { .. } => ProfileTopologyErrorCode::Manifest,
            Self::ProfileSelection { .. } => ProfileTopologyErrorCode::ProfileSelection,
            Self::ProjectLayout { .. } => ProfileTopologyErrorCode::ProjectLayout,
            Self::UnownedResourcePath { .. } => ProfileTopologyErrorCode::UnownedResourcePath,
            Self::DuplicateLogicalId { .. } => ProfileTopologyErrorCode::DuplicateLogicalId,
            Self::DuplicatePath { .. } => ProfileTopologyErrorCode::DuplicatePath,
            Self::ModuleSyntax { .. } => ProfileTopologyErrorCode::ModuleSyntax,
            Self::ModuleDeclaration { .. } => ProfileTopologyErrorCode::ModuleDeclaration,
            Self::ModuleImport { .. } => ProfileTopologyErrorCode::ModuleImport,
            Self::CharacterManifest { .. }
            | Self::CharacterReference { .. }
            | Self::CharacterIdentityMismatch { .. }
            | Self::CharacterPackage { .. } => ProfileTopologyErrorCode::CharacterManifest,
            Self::AdapterSelection { .. } => ProfileTopologyErrorCode::AdapterSelection,
            Self::ExternalModuleMetadataHash { .. }
            | Self::ExternalModuleMetadataDecode { .. }
            | Self::ExternalModuleMetadataExpectation { .. }
            | Self::ExternalModuleFacts(_) => ProfileTopologyErrorCode::ExternalModuleMetadata,
            Self::DependencySeed { .. } => ProfileTopologyErrorCode::DependencySeed,
            Self::Limit { .. } => ProfileTopologyErrorCode::Limit,
            Self::ArithmeticOverflow { .. } => ProfileTopologyErrorCode::ArithmeticOverflow,
        }
    }

    pub fn resource_manifest_code(
        &self,
    ) -> Option<arcweft_resource_manifest::ResourceManifestDiagnosticCode> {
        use arcweft_resource_manifest::ResourceManifestDiagnosticCode as Code;
        match self {
            Self::ResourceTypeManifestUtf8 { .. } => Some(Code::InvalidUtf8),
            Self::UnresolvedResourceTypePackage { .. } => Some(Code::UnresolvedPackage),
            Self::ResourceTypeManifest { source, .. }
            | Self::ResourceTypePublication { source } => source
                .diagnostics()
                .first()
                .map(arcweft_resource_manifest::ResourceManifestDiagnostic::code),
            _ => None,
        }
    }
}

pub(super) fn validate_absolute_normalized_path(
    path: &Path,
    field: &'static str,
) -> Result<(), ProfileTopologySeedError> {
    if !path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
    {
        return Err(ProfileTopologySeedError::Path {
            field,
            path: path.to_path_buf(),
        });
    }
    Ok(())
}

pub(super) fn slash_relative_path(path: &Path) -> Result<String, ProfileTopologySeedError> {
    let mut segments = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(segment) => segments.push(segment.to_str().ok_or_else(|| {
                ProfileTopologySeedError::NonUtf8 {
                    path: path.to_path_buf(),
                }
            })?),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(ProfileTopologySeedError::Path {
                    field: "relative path",
                    path: path.to_path_buf(),
                });
            }
        }
    }
    let value = segments.join("/");
    ProfileTopologyLogicalPath::try_new(value.clone()).map_err(|_| {
        ProfileTopologySeedError::Path {
            field: "logical path",
            path: path.to_path_buf(),
        }
    })?;
    Ok(value)
}
