use super::{ProfileTopologyLogicalPath, ProfileTopologyOwnerId, ProfileTopologyResourceId};
use crate::{
    adapter_manifest::{self, LoadedAdapterManifest},
    character_manifest, project,
    rust_metadata::{self, LoadedRustMetadata},
};
use arcweft_adapter_context::manifest::{
    AdapterCallableModelError, AdapterManifest, AdapterRegistry, AdapterRegistryError,
};
use arcweft_lang_sema::registration::CharacterDefinitionLimits;
use arcweft_lang_syntax::ast::module_path::CanonicalModulePath;
use arcweft_launch::{
    LaunchDocumentError, LaunchProfileError, LaunchProfileSelection, ResolvedLaunchProfile,
};
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
    ArcweftModule { module: CanonicalModulePath },
    CharacterManifest,
    AdapterManifest,
    RustMetadata,
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
    pub(super) document: Arc<SourceDocument>,
    pub(super) ownership: LoadedDocumentOwnership,
    pub(super) access: LoadedDocumentAccess,
    pub(super) origin: ProfileTopologyResourceOrigin,
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

    pub fn document(&self) -> &Arc<SourceDocument> {
        &self.document
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
    pub(super) dependency_resources: &'a [ProfileDependencyResourceSeed],
    pub(super) base_adapters: AdapterRegistry,
}

impl<'a> ProfileTopologyLoadRequest<'a> {
    pub fn new(
        manifest_path: &'a Path,
        workspace_owner: ProfileTopologyOwnerId,
        selection: LaunchProfileSelection<'a>,
        overlays: &'a [ProfileTopologyOverlaySeed],
        base_adapters: AdapterRegistry,
    ) -> Self {
        Self {
            manifest_path,
            workspace_owner,
            selection,
            overlays,
            dependency_resources: &[],
            base_adapters,
        }
    }

    #[must_use]
    pub fn with_dependency_resources(
        mut self,
        resources: &'a [ProfileDependencyResourceSeed],
    ) -> Self {
        self.dependency_resources = resources;
        self
    }
}

/// Complete immutable product of one bounded topology transaction.
#[derive(Clone, Debug)]
pub struct LoadedProfileTopology {
    loaded_project: project::LoadedProject,
    selected_profile: ResolvedLaunchProfile,
    adapter_sources: Arc<[LoadedAdapterManifest]>,
    adapter: AdapterManifest,
    registration_adapter_manifests: Arc<[AdapterManifest]>,
    rust_metadata_sources: Arc<[LoadedRustMetadata]>,
    resources: BTreeMap<ProfileTopologyResourceId, LoadedProfileTopologyResource>,
    consumed_overlay_ids: Arc<[ProfileTopologyResourceId]>,
    source_revision: SourceSetRevision,
    work: u64,
}

impl LoadedProfileTopology {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        loaded_project: project::LoadedProject,
        selected_profile: ResolvedLaunchProfile,
        adapter_sources: Vec<LoadedAdapterManifest>,
        adapter: AdapterManifest,
        rust_metadata_sources: Vec<LoadedRustMetadata>,
        resources: BTreeMap<ProfileTopologyResourceId, LoadedProfileTopologyResource>,
        consumed_overlay_ids: Vec<ProfileTopologyResourceId>,
        source_revision: SourceSetRevision,
        work: u64,
    ) -> Self {
        let registration_adapter_manifests = Arc::from([adapter.clone()]);
        Self {
            loaded_project,
            selected_profile,
            adapter_sources: adapter_sources.into(),
            adapter,
            registration_adapter_manifests,
            rust_metadata_sources: rust_metadata_sources.into(),
            resources,
            consumed_overlay_ids: consumed_overlay_ids.into(),
            source_revision,
            work,
        }
    }

    pub const fn loaded_project(&self) -> &project::LoadedProject {
        &self.loaded_project
    }

    pub const fn selected_profile(&self) -> &ResolvedLaunchProfile {
        &self.selected_profile
    }

    pub fn adapter_sources(&self) -> &[LoadedAdapterManifest] {
        &self.adapter_sources
    }

    pub const fn adapter(&self) -> &AdapterManifest {
        &self.adapter
    }

    pub fn registration_adapter_manifests(&self) -> &[AdapterManifest] {
        &self.registration_adapter_manifests
    }

    pub fn rust_metadata_sources(&self) -> &[LoadedRustMetadata] {
        &self.rust_metadata_sources
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

    pub fn consumed_overlay_ids(
        &self,
    ) -> impl ExactSizeIterator<Item = &ProfileTopologyResourceId> {
        self.consumed_overlay_ids.iter()
    }

    pub const fn source_revision(&self) -> SourceSetRevision {
        self.source_revision
    }

    pub const fn work(&self) -> u64 {
        self.work
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
    ProjectManifest,
    LaunchManifest,
    ProfileSelection,
    UnownedResourcePath,
    DuplicateLogicalId,
    DuplicatePath,
    ModuleSyntax,
    ModuleDeclaration,
    ModuleImport,
    CharacterManifest,
    AdapterManifest,
    DuplicateAdapterId,
    AdapterSelection,
    RustMetadata,
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
    #[error("overlay path `{path}` occurs more than once")]
    DuplicateOverlayPath { path: PathBuf },
    #[error("dependency seed path `{path}` and role occur more than once")]
    DuplicateDependencySeed { path: PathBuf },
}

/// Fatal failure of one all-or-nothing topology transaction.
#[derive(Debug, Error)]
pub enum ProfileTopologyLoadError {
    #[error("primary launch manifest was not found at `{path}`")]
    ManifestNotFound { path: PathBuf },
    #[error("failed to read topology resource `{path}`: {source}")]
    ResourceRead {
        id: ProfileTopologyResourceId,
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("topology resource `{path}` is not valid UTF-8")]
    ResourceUtf8 {
        id: ProfileTopologyResourceId,
        path: PathBuf,
    },
    #[error("failed to parse project manifest `{path}`: {source}")]
    ProjectManifest {
        id: ProfileTopologyResourceId,
        path: PathBuf,
        #[source]
        source: Box<arcweft_project::manifest::ProjectManifestError>,
    },
    #[error("failed to parse source-backed launch manifest `{path}`: {source}")]
    LaunchManifest {
        id: ProfileTopologyResourceId,
        path: PathBuf,
        #[source]
        source: Box<LaunchDocumentError>,
    },
    #[error("failed to select or resolve a launch profile: {source}")]
    ProfileSelection {
        #[source]
        source: LaunchProfileError,
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
    #[error("topology path is claimed by distinct logical IDs")]
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
        source: Box<character_manifest::LoadError>,
    },
    #[error("failed to decode adapter manifest `{path}`: {source}")]
    AdapterManifest {
        id: ProfileTopologyResourceId,
        path: PathBuf,
        #[source]
        source: Box<adapter_manifest::LoadError>,
    },
    #[error(transparent)]
    DuplicateAdapterId(#[from] AdapterRegistryError),
    #[error("selected adapter `{id}` was not found in the complete checked registry")]
    AdapterSelection { id: String },
    #[error("failed to decode Rust metadata `{path}`: {source}")]
    RustMetadata {
        id: ProfileTopologyResourceId,
        path: PathBuf,
        #[source]
        source: Box<rust_metadata::LoadError>,
    },
    #[error("Rust metadata `{path}` contains an invalid callable: {source}")]
    RustCallableModel {
        path: PathBuf,
        source: AdapterCallableModelError,
    },
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
            Self::ProjectManifest { .. } => ProfileTopologyErrorCode::ProjectManifest,
            Self::LaunchManifest { .. } => ProfileTopologyErrorCode::LaunchManifest,
            Self::ProfileSelection { .. } => ProfileTopologyErrorCode::ProfileSelection,
            Self::UnownedResourcePath { .. } => ProfileTopologyErrorCode::UnownedResourcePath,
            Self::DuplicateLogicalId { .. } => ProfileTopologyErrorCode::DuplicateLogicalId,
            Self::DuplicatePath { .. } => ProfileTopologyErrorCode::DuplicatePath,
            Self::ModuleSyntax { .. } => ProfileTopologyErrorCode::ModuleSyntax,
            Self::ModuleDeclaration { .. } => ProfileTopologyErrorCode::ModuleDeclaration,
            Self::ModuleImport { .. } => ProfileTopologyErrorCode::ModuleImport,
            Self::CharacterManifest { .. } => ProfileTopologyErrorCode::CharacterManifest,
            Self::AdapterManifest { .. } => ProfileTopologyErrorCode::AdapterManifest,
            Self::DuplicateAdapterId(_) => ProfileTopologyErrorCode::DuplicateAdapterId,
            Self::AdapterSelection { .. } => ProfileTopologyErrorCode::AdapterSelection,
            Self::RustMetadata { .. } | Self::RustCallableModel { .. } => {
                ProfileTopologyErrorCode::RustMetadata
            }
            Self::DependencySeed { .. } => ProfileTopologyErrorCode::DependencySeed,
            Self::Limit { .. } => ProfileTopologyErrorCode::Limit,
            Self::ArithmeticOverflow { .. } => ProfileTopologyErrorCode::ArithmeticOverflow,
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
