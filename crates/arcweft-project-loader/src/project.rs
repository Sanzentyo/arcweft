//! Filesystem adapter for Arcweft package discovery and module graph loading.

use crate::project_limits::ProjectLoadLimits;
use arcweft_lang_syntax::{
    ast::{
        module_path::{
            CanonicalModulePath, ModulePath, ModulePathError, ModulePathRoot, ModuleSegment,
        },
        symbol_path::{ProjectSymbolPath, ProjectSymbolPathError, ProjectSymbolSegment},
    },
    attachment::{
        SyntaxAccessError, SyntaxLookupError, SyntaxNodeId,
        item::TypedItemNode,
        source_file::{AttachedPath, AttachedPathRoot, AttachedUseTree},
    },
    incremental::{ParseFailure, ParsedSource, SyntaxDatabase},
    parser::ParseOptions,
};
use arcweft_launch::{accepted::SourceBackedManifest, diagnostic::ManifestReport};
use arcweft_project::{
    graph::ModuleDependency,
    sources::{ProjectSourceFile, ProjectSources},
};
use arcweft_source::{
    SourceDocument, SourceDocumentError, SourceDocumentId, SourceDocumentIdError, SourceEdit,
    SourceName, SourceRange, SourceSpan, SourceSpanError, identity::SourceSnapshotId,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File},
    io::Read,
    path::{Component, Path, PathBuf},
    sync::Arc,
};
use thiserror::Error;

pub const PROJECT_MANIFEST_FILE: &str = "arcw.toml";

/// Fully loaded project metadata and source inventory.
#[derive(Clone, Debug)]
pub struct LoadedProject {
    sources: ProjectSources,
    module_parsed_sources: BTreeMap<CanonicalModulePath, ParsedSource>,
    manifest: Arc<SourceBackedManifest>,
}

/// Project discovery, source loading, or module resolution failure.
#[derive(Debug, Error)]
pub enum ProjectLoadError {
    #[error("could not find `{PROJECT_MANIFEST_FILE}` from `{start}` or any parent directory")]
    ManifestNotFound { start: PathBuf },
    #[error("failed to read `{path}`: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("project source `{path}` is not valid UTF-8: {source}")]
    InvalidUtf8 {
        path: PathBuf,
        #[source]
        source: std::string::FromUtf8Error,
    },
    #[error("failed to enumerate source directory `{path}`: {source}")]
    Enumerate {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to decode the project manifest: {0}")]
    Manifest(#[from] ManifestReport),
    #[error("source file `{path}` is outside source root `{source_root}`")]
    OutsideSourceRoot { path: PathBuf, source_root: PathBuf },
    #[error("project document `{path}` is outside project root `{project_root}`")]
    OutsideProjectRoot {
        path: PathBuf,
        project_root: PathBuf,
    },
    #[error("project-relative document path `{path}` is not valid UTF-8")]
    NonUtf8ProjectPath { path: PathBuf },
    #[error(transparent)]
    DocumentId(#[from] SourceDocumentIdError),
    #[error(transparent)]
    Document(#[from] SourceDocumentError),
    #[error("failed to bind syntax for project source `{path}`: {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: Box<ParseFailure>,
    },
    #[error("failed to read attached syntax for project source `{path}`: {source}")]
    SyntaxAccess {
        path: PathBuf,
        #[source]
        source: Box<SyntaxAccessError>,
    },
    #[error("failed to resolve the current syntax lineage for project source `{path}`: {source}")]
    SyntaxLookup {
        path: PathBuf,
        #[source]
        source: Box<SyntaxLookupError>,
    },
    #[error("project source `{path}` contains a recovered path at syntax identity {node:?}")]
    RecoveredPath { path: PathBuf, node: SyntaxNodeId },
    #[error("project source `{path}` has an invalid project-symbol path: {source}")]
    ProjectSymbolPath {
        path: PathBuf,
        #[source]
        source: ProjectSymbolPathError,
    },
    #[error(transparent)]
    SourceSpan(#[from] SourceSpanError),
    #[error(
        "syntax lineage for module `{module}` does not match source `{path}`: expected document `{expected_document}` named `{expected_name}`, found `{actual_document}` named `{actual_name}`"
    )]
    SourceIdentityMismatch {
        module: Box<CanonicalModulePath>,
        path: Box<PathBuf>,
        expected_document: Box<SourceDocumentId>,
        expected_name: String,
        actual_document: Box<SourceDocumentId>,
        actual_name: String,
    },
    #[error("parsed source for module `{module}` does not retain its exact project document")]
    ParsedDocumentMismatch { module: CanonicalModulePath },
    #[error("`mod.arcw` is not a supported module layout; use `{suggested}`")]
    ModFileLayout { suggested: PathBuf },
    #[error(transparent)]
    ModulePath(#[from] ModulePathError),
    #[error("source `{path}` has syntax errors: {diagnostics:?}")]
    Syntax {
        path: PathBuf,
        diagnostics: Vec<String>,
    },
    #[error("module source `{path}` must declare `mod {expected}`")]
    MissingModuleDeclaration {
        path: PathBuf,
        expected: CanonicalModulePath,
    },
    #[error("module source `{path}` declares `{declared}`, but its path maps to `{expected}`")]
    ModulePathMismatch {
        path: PathBuf,
        declared: CanonicalModulePath,
        expected: CanonicalModulePath,
    },
    #[error("module `{module}` imports unresolved path `{import}` in `{path}`")]
    UnresolvedImport {
        path: PathBuf,
        module: CanonicalModulePath,
        import: String,
    },
    #[error(transparent)]
    Sources(#[from] arcweft_project::sources::ProjectSourcesError),
    #[error("project load exceeded the {kind} limit: observed {observed}, maximum {maximum}")]
    LimitExceeded {
        kind: ProjectLoadLimitKind,
        observed: u64,
        maximum: u64,
    },
    #[error("project load {counter} counter overflowed")]
    ArithmeticOverflow { counter: ProjectLoadLimitKind },
}

/// Counter reported by bounded project loading failures.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectLoadLimitKind {
    Documents,
    SourceBytes,
}

impl ProjectLoadLimitKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Documents => "document",
            Self::SourceBytes => "source-byte",
        }
    }
}

impl std::fmt::Display for ProjectLoadLimitKind {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug)]
struct ProjectLoadBudget {
    limits: ProjectLoadLimits,
    documents: u64,
    source_bytes: u64,
}

impl ProjectLoadBudget {
    const fn new(limits: ProjectLoadLimits) -> Self {
        Self {
            limits,
            documents: 0,
            source_bytes: 0,
        }
    }

    fn read_document(&mut self, path: &Path) -> Result<String, ProjectLoadError> {
        let documents =
            self.documents
                .checked_add(1)
                .ok_or(ProjectLoadError::ArithmeticOverflow {
                    counter: ProjectLoadLimitKind::Documents,
                })?;
        if documents > self.limits.documents() {
            return Err(ProjectLoadError::LimitExceeded {
                kind: ProjectLoadLimitKind::Documents,
                observed: documents,
                maximum: self.limits.documents(),
            });
        }

        let (source, observed) =
            read_utf8_bounded(path, self.source_bytes, self.limits.source_bytes())?;
        let source_bytes = self.source_bytes.checked_add(observed).ok_or(
            ProjectLoadError::ArithmeticOverflow {
                counter: ProjectLoadLimitKind::SourceBytes,
            },
        )?;
        if source_bytes > self.limits.source_bytes() {
            return Err(ProjectLoadError::LimitExceeded {
                kind: ProjectLoadLimitKind::SourceBytes,
                observed: source_bytes,
                maximum: self.limits.source_bytes(),
            });
        }
        self.documents = documents;
        self.source_bytes = source_bytes;
        Ok(source)
    }
}

impl LoadedProject {
    pub(crate) fn from_bound_modules(
        manifest_path: PathBuf,
        project_root: PathBuf,
        manifest: Arc<SourceBackedManifest>,
        modules: Vec<(ProjectSourceFile, ParsedSource)>,
    ) -> Result<Self, ProjectLoadError> {
        for (module, parsed) in &modules {
            if !Arc::ptr_eq(module.document(), parsed.document_lease()) {
                return Err(ProjectLoadError::ParsedDocumentMismatch {
                    module: module.module().clone(),
                });
            }
        }
        let module_parsed_sources = modules
            .iter()
            .map(|(module, parsed)| (module.module().clone(), parsed.clone()))
            .collect();
        let sources = ProjectSources::new(
            manifest_path,
            project_root,
            manifest.manifest().package().clone(),
            manifest.manifest().build().clone(),
            Arc::clone(manifest.document()),
            modules.into_iter().map(|(module, _)| module),
        )?;
        Ok(Self {
            sources,
            module_parsed_sources,
            manifest,
        })
    }

    pub const fn sources(&self) -> &ProjectSources {
        &self.sources
    }

    pub const fn manifest(&self) -> &Arc<SourceBackedManifest> {
        &self.manifest
    }

    pub fn manifest_document(&self) -> &Arc<SourceDocument> {
        self.manifest.document()
    }

    pub fn module_documents(
        &self,
    ) -> impl ExactSizeIterator<Item = (&CanonicalModulePath, &Arc<SourceDocument>)> {
        self.module_parsed_sources
            .iter()
            .map(|(module, parsed)| (module, parsed.document_lease()))
    }

    pub fn module_document(&self, module: &CanonicalModulePath) -> Option<&Arc<SourceDocument>> {
        self.module_parsed_sources
            .get(module)
            .map(ParsedSource::document_lease)
    }

    /// Exact attached syntax leases retained for every canonical module.
    pub fn module_parsed_sources(
        &self,
    ) -> impl ExactSizeIterator<Item = (&CanonicalModulePath, &ParsedSource)> {
        self.module_parsed_sources.iter()
    }

    /// Complete canonical module-to-snapshot map owned by this load transaction.
    pub const fn module_parsed_source_map(&self) -> &BTreeMap<CanonicalModulePath, ParsedSource> {
        &self.module_parsed_sources
    }

    /// Exact attached syntax lease retained for one canonical module.
    pub fn module_parsed_source(&self, module: &CanonicalModulePath) -> Option<&ParsedSource> {
        self.module_parsed_sources.get(module)
    }
}

/// Searches `start` and its parents for `arcw.toml`.
pub fn discover_manifest(start: &Path) -> Result<PathBuf, ProjectLoadError> {
    let start = if start.is_file() {
        start.parent().unwrap_or_else(|| Path::new("."))
    } else {
        start
    };
    start
        .ancestors()
        .map(|directory| directory.join(PROJECT_MANIFEST_FILE))
        .find(|candidate| candidate.is_file())
        .ok_or_else(|| ProjectLoadError::ManifestNotFound {
            start: start.to_path_buf(),
        })
}

/// Discovers and loads the project containing `start`.
pub fn load_discovered(
    syntax: &mut SyntaxDatabase,
    start: &Path,
) -> Result<LoadedProject, ProjectLoadError> {
    load(syntax, &discover_manifest(start)?)
}

/// Loads one explicit `arcw.toml` and all `.arcw` sources under its source root.
pub fn load(
    syntax: &mut SyntaxDatabase,
    manifest_path: &Path,
) -> Result<LoadedProject, ProjectLoadError> {
    load_with_limits(
        syntax,
        manifest_path,
        ProjectLoadLimits::new(u64::MAX, u64::MAX),
    )
}

/// Loads one explicit project while bounding all accepted UTF-8 documents before parsing.
pub fn load_with_limits(
    syntax: &mut SyntaxDatabase,
    manifest_path: &Path,
    limits: ProjectLoadLimits,
) -> Result<LoadedProject, ProjectLoadError> {
    load_with_previous(syntax, None, manifest_path, limits)
}

/// Reloads one project through the same syntax session and exact prior module leases.
pub fn reload(
    syntax: &mut SyntaxDatabase,
    previous: &LoadedProject,
    manifest_path: &Path,
) -> Result<LoadedProject, ProjectLoadError> {
    reload_with_limits(
        syntax,
        previous,
        manifest_path,
        ProjectLoadLimits::new(u64::MAX, u64::MAX),
    )
}

/// Reloads one bounded project through the same syntax session and prior module leases.
pub fn reload_with_limits(
    syntax: &mut SyntaxDatabase,
    previous: &LoadedProject,
    manifest_path: &Path,
    limits: ProjectLoadLimits,
) -> Result<LoadedProject, ProjectLoadError> {
    load_with_previous(syntax, Some(previous), manifest_path, limits)
}

fn load_with_previous(
    syntax: &mut SyntaxDatabase,
    previous: Option<&LoadedProject>,
    manifest_path: &Path,
    limits: ProjectLoadLimits,
) -> Result<LoadedProject, ProjectLoadError> {
    let mut budget = ProjectLoadBudget::new(limits);
    let manifest_source = budget.read_document(manifest_path)?;
    let project_root = manifest_path.parent().unwrap_or_else(|| Path::new("."));
    let manifest_document = Arc::new(SourceDocument::try_new(
        manifest_document_id(manifest_path)?,
        SourceName::path(manifest_path.display().to_string()),
        manifest_source,
    )?);
    let manifest = Arc::new(SourceBackedManifest::decode(Arc::clone(
        &manifest_document,
    ))?);
    let package = manifest.manifest().package().id.as_str();
    let source_root = project_root.join(manifest.manifest().build().source_dir.as_path());
    let source_paths = collect_arcw_files(&source_root, limits.documents(), budget.documents)?;
    let scanned = source_paths
        .into_iter()
        .map(|path| {
            scan_source(
                syntax,
                previous,
                package,
                project_root,
                &source_root,
                path,
                &mut budget,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let module_paths = scanned
        .iter()
        .map(|source| source.module.clone())
        .collect::<BTreeSet<_>>();
    let loaded_modules = scanned
        .into_iter()
        .map(|source| source.finish(&module_paths))
        .collect::<Vec<_>>();
    LoadedProject::from_bound_modules(
        manifest_path.to_path_buf(),
        project_root.to_path_buf(),
        manifest,
        loaded_modules,
    )
}

pub(crate) fn manifest_document_id(
    manifest_path: &Path,
) -> Result<SourceDocumentId, ProjectLoadError> {
    let digest = blake3::hash(manifest_path.to_string_lossy().as_bytes());
    SourceDocumentId::try_new(format!(
        "arcweft-project://manifest/{}/arcw.toml",
        digest.to_hex()
    ))
    .map_err(ProjectLoadError::from)
}

/// Binds one canonical module document to the caller-selected syntax session.
pub(crate) fn bind_module_source(
    syntax: &mut SyntaxDatabase,
    module: &CanonicalModulePath,
    path: &Path,
    document: Arc<SourceDocument>,
    previous: Option<&ParsedSource>,
) -> Result<ParsedSource, ProjectLoadError> {
    let current = if let Some(previous) = previous {
        validate_source_identity(module, path, previous.document(), &document)?;
        Some(
            syntax
                .current(previous.snapshot_id().lineage())
                .map_err(|source| ProjectLoadError::SyntaxLookup {
                    path: path.to_path_buf(),
                    source: Box::new(source),
                })?,
        )
    } else {
        syntax.current_for_source(document.display_name())
    };
    let result = if let Some(current) = current {
        validate_source_identity(module, path, current.document(), &document)?;
        if current.source() == document.text() {
            syntax.reparse(&current, &[], ParseOptions::default())
        } else {
            let whole = current
                .document()
                .span(SourceRange::new(0, current.source().len()))?;
            syntax.reparse(
                &current,
                &[SourceEdit::new(whole, document.text())],
                ParseOptions::default(),
            )
        }
    } else {
        syntax.parse_initial(
            SourceSnapshotId::initial(document.display_name().clone()),
            document,
            ParseOptions::default(),
        )
    };
    result.map_err(|source| ProjectLoadError::Parse {
        path: path.to_path_buf(),
        source: Box::new(source),
    })
}

fn validate_source_identity(
    module: &CanonicalModulePath,
    path: &Path,
    expected: &SourceDocument,
    actual: &SourceDocument,
) -> Result<(), ProjectLoadError> {
    if expected.identity().id() != actual.identity().id()
        || expected.display_name() != actual.display_name()
    {
        return Err(ProjectLoadError::SourceIdentityMismatch {
            module: Box::new(module.clone()),
            path: Box::new(path.to_path_buf()),
            expected_document: Box::new(expected.identity().id().clone()),
            expected_name: expected.display_name().display_name().to_owned(),
            actual_document: Box::new(actual.identity().id().clone()),
            actual_name: actual.display_name().display_name().to_owned(),
        });
    }
    Ok(())
}

/// One parser-owned module/import projection from an exact attached snapshot.
#[derive(Clone, Debug)]
pub(crate) struct ModuleSourceInventory {
    declaration: Option<ModulePath>,
    imports: Vec<ModuleSourceImport>,
}

impl ModuleSourceInventory {
    pub(crate) const fn declaration(&self) -> Option<&ModulePath> {
        self.declaration.as_ref()
    }

    pub(crate) fn imports(&self) -> &[ModuleSourceImport] {
        &self.imports
    }
}

/// One typed import path and its exact attached source span.
#[derive(Clone, Debug)]
pub(crate) struct ModuleSourceImport {
    path: ProjectSymbolPath,
    source: SourceSpan,
}

impl ModuleSourceImport {
    pub(crate) const fn path(&self) -> &ProjectSymbolPath {
        &self.path
    }

    pub(crate) const fn source(&self) -> &SourceSpan {
        &self.source
    }
}

/// Reads module topology only from the retained attached source-file tree.
pub(crate) fn read_module_inventory(
    path: &Path,
    parsed: &ParsedSource,
) -> Result<ModuleSourceInventory, ProjectLoadError> {
    let items = parsed
        .items()
        .map_err(|source| ProjectLoadError::SyntaxAccess {
            path: path.to_path_buf(),
            source: Box::new(source),
        })?;
    let mut declaration = None;
    let mut imports = Vec::new();
    for item in items {
        match item {
            TypedItemNode::Module(module) if declaration.is_none() => {
                let attached = module
                    .path()
                    .map_err(|source| ProjectLoadError::SyntaxAccess {
                        path: path.to_path_buf(),
                        source: Box::new(source),
                    })?;
                declaration = Some(attached_module_path(path, &attached)?);
            }
            TypedItemNode::Use(import) => {
                let tree = import
                    .tree()
                    .map_err(|source| ProjectLoadError::SyntaxAccess {
                        path: path.to_path_buf(),
                        source: Box::new(source),
                    })?;
                let attached = match tree {
                    AttachedUseTree::Path { path, .. } => path,
                    AttachedUseTree::Glob { module, .. }
                    | AttachedUseTree::Group { module, .. } => module,
                };
                imports.push(ModuleSourceImport {
                    path: attached_project_symbol_path(path, &attached)?,
                    source: attached.syntax().source_span(),
                });
            }
            _ => {}
        }
    }
    Ok(ModuleSourceInventory {
        declaration,
        imports,
    })
}

fn attached_module_path(
    path: &Path,
    attached: &AttachedPath,
) -> Result<ModulePath, ProjectLoadError> {
    reject_recovered_path(path, attached)?;
    let segments = attached
        .segments()
        .iter()
        .map(|segment| ModuleSegment::new(segment.source_text().to_owned()))
        .collect::<Result<Vec<_>, _>>()?;
    ModulePath::new(attached_module_root(attached.root()), segments).map_err(ProjectLoadError::from)
}

fn attached_project_symbol_path(
    path: &Path,
    attached: &AttachedPath,
) -> Result<ProjectSymbolPath, ProjectLoadError> {
    reject_recovered_path(path, attached)?;
    let segments = attached
        .segments()
        .iter()
        .map(|segment| ProjectSymbolSegment::try_new(segment.source_text().to_owned()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|source| ProjectLoadError::ProjectSymbolPath {
            path: path.to_path_buf(),
            source,
        })?;
    ProjectSymbolPath::new(attached_module_root(attached.root()), segments).map_err(|source| {
        ProjectLoadError::ProjectSymbolPath {
            path: path.to_path_buf(),
            source,
        }
    })
}

fn reject_recovered_path(path: &Path, attached: &AttachedPath) -> Result<(), ProjectLoadError> {
    if attached.has_recovery() {
        return Err(ProjectLoadError::RecoveredPath {
            path: path.to_path_buf(),
            node: attached.syntax().id(),
        });
    }
    Ok(())
}

const fn attached_module_root(root: &AttachedPathRoot) -> ModulePathRoot {
    match root {
        AttachedPathRoot::ImplicitCrate => ModulePathRoot::ImplicitCrate,
        AttachedPathRoot::Crate { .. } => ModulePathRoot::Crate,
        AttachedPathRoot::SelfModule { .. } => ModulePathRoot::SelfModule,
        AttachedPathRoot::Super { levels } => ModulePathRoot::Super(levels.len()),
    }
}

#[derive(Clone, Debug)]
struct ScannedSource {
    path: PathBuf,
    parsed: ParsedSource,
    module: CanonicalModulePath,
    imports: Vec<PendingImport>,
}

#[derive(Clone, Debug)]
struct PendingImport {
    path: ProjectSymbolPath,
}

impl ScannedSource {
    fn finish(self, modules: &BTreeSet<CanonicalModulePath>) -> (ProjectSourceFile, ParsedSource) {
        let dependencies = self
            .imports
            .into_iter()
            .filter_map(|import| {
                longest_known_module_prefix(&import.path, &self.module, modules)
                    .filter(|target| target != &self.module)
                    .map(ModuleDependency::new)
            })
            .collect::<Vec<_>>();
        let document = Arc::clone(self.parsed.document_lease());
        (
            ProjectSourceFile::new(self.module, self.path, document, dependencies),
            self.parsed,
        )
    }
}

fn scan_source(
    syntax: &mut SyntaxDatabase,
    previous: Option<&LoadedProject>,
    package: &str,
    project_root: &Path,
    source_root: &Path,
    path: PathBuf,
    budget: &mut ProjectLoadBudget,
) -> Result<ScannedSource, ProjectLoadError> {
    let source = budget.read_document(&path)?;
    let document = Arc::new(SourceDocument::try_new(
        project_document_id(package, project_root, &path)?,
        SourceName::path(path.display().to_string()),
        source,
    )?);
    let inferred = inferred_module_path(source_root, &path)?;
    let previous = previous.and_then(|loaded| loaded.module_parsed_source(&inferred));
    let parsed = bind_module_source(syntax, &inferred, &path, document, previous)?;
    if !parsed.diagnostics().is_empty() {
        return Err(ProjectLoadError::Syntax {
            path,
            diagnostics: parsed
                .diagnostics()
                .iter()
                .map(|error| error.message().to_owned())
                .collect(),
        });
    }
    let inventory = read_module_inventory(&path, &parsed)?;
    let module = match inventory.declaration() {
        Some(declaration) => {
            let declared = declaration.resolve_declaration_for(&inferred)?;
            if declared != inferred {
                return Err(ProjectLoadError::ModulePathMismatch {
                    path,
                    declared,
                    expected: inferred,
                });
            }
            declared
        }
        None if inferred.is_crate_root() => inferred,
        None => {
            return Err(ProjectLoadError::MissingModuleDeclaration {
                path,
                expected: inferred,
            });
        }
    };
    let imports = inventory
        .imports()
        .iter()
        .map(|item| PendingImport {
            path: item.path().clone(),
        })
        .collect();
    Ok(ScannedSource {
        path,
        parsed,
        module,
        imports,
    })
}

pub(crate) fn project_document_id(
    package: &str,
    project_root: &Path,
    path: &Path,
) -> Result<SourceDocumentId, ProjectLoadError> {
    let relative =
        path.strip_prefix(project_root)
            .map_err(|_| ProjectLoadError::OutsideProjectRoot {
                path: path.to_path_buf(),
                project_root: project_root.to_path_buf(),
            })?;
    let mut segments = Vec::new();
    for component in relative.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(segment) => {
                let segment =
                    segment
                        .to_str()
                        .ok_or_else(|| ProjectLoadError::NonUtf8ProjectPath {
                            path: path.to_path_buf(),
                        })?;
                segments.push(segment);
            }
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(ProjectLoadError::OutsideProjectRoot {
                    path: path.to_path_buf(),
                    project_root: project_root.to_path_buf(),
                });
            }
        }
    }
    SourceDocumentId::try_new(format!(
        "arcweft-project://{package}/{}",
        segments.join("/")
    ))
    .map_err(ProjectLoadError::from)
}

fn longest_known_module_prefix(
    path: &ProjectSymbolPath,
    importer: &CanonicalModulePath,
    modules: &BTreeSet<CanonicalModulePath>,
) -> Option<CanonicalModulePath> {
    let mut module_segments = Vec::new();
    for segment in path.segments() {
        let Ok(segment) = segment.try_as_module_segment() else {
            break;
        };
        module_segments.push(segment);
    }
    (1..=module_segments.len()).rev().find_map(|length| {
        ModulePath::new(path.root(), module_segments[..length].iter().cloned())
            .ok()?
            .resolve_from(importer)
            .ok()
            .filter(|candidate| modules.contains(candidate))
    })
}

pub(crate) fn inferred_module_path(
    source_root: &Path,
    path: &Path,
) -> Result<CanonicalModulePath, ProjectLoadError> {
    let relative =
        path.strip_prefix(source_root)
            .map_err(|_| ProjectLoadError::OutsideSourceRoot {
                path: path.to_path_buf(),
                source_root: source_root.to_path_buf(),
            })?;
    let mut components = relative.components().collect::<Vec<_>>();
    let Some(Component::Normal(file_name)) = components.pop() else {
        return Err(ProjectLoadError::OutsideSourceRoot {
            path: path.to_path_buf(),
            source_root: source_root.to_path_buf(),
        });
    };
    let file_name = Path::new(file_name);
    let stem = file_name
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or_default();
    if stem == "mod" {
        let suggested = path.parent().and_then(Path::file_name).map_or_else(
            || source_root.join("module.arcw"),
            |name| {
                path.parent()
                    .and_then(Path::parent)
                    .unwrap_or(source_root)
                    .join(name)
                    .with_extension("arcw")
            },
        );
        return Err(ProjectLoadError::ModFileLayout { suggested });
    }
    if components.is_empty() && matches!(stem, "main" | "lib") {
        return Ok(CanonicalModulePath::crate_root());
    }
    let segments = components
        .into_iter()
        .map(|component| match component {
            Component::Normal(segment) => {
                ModuleSegment::new(segment.to_string_lossy().into_owned())
            }
            _ => Err(ModulePathError::InvalidSegment {
                segment: component.as_os_str().to_string_lossy().into_owned(),
            }),
        })
        .chain(core::iter::once(ModuleSegment::new(stem.to_owned())))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(CanonicalModulePath::from_segments(segments))
}

fn collect_arcw_files(
    source_root: &Path,
    maximum_documents: u64,
    already_observed: u64,
) -> Result<Vec<PathBuf>, ProjectLoadError> {
    fn visit(
        directory: &Path,
        output: &mut Vec<PathBuf>,
        observed: &mut u64,
        maximum: u64,
    ) -> Result<(), ProjectLoadError> {
        let entries = fs::read_dir(directory).map_err(|source| ProjectLoadError::Enumerate {
            path: directory.to_path_buf(),
            source,
        })?;
        for entry in entries {
            let entry = entry.map_err(|source| ProjectLoadError::Enumerate {
                path: directory.to_path_buf(),
                source,
            })?;
            let path = entry.path();
            let file_type = entry
                .file_type()
                .map_err(|source| ProjectLoadError::Enumerate {
                    path: path.clone(),
                    source,
                })?;
            if file_type.is_dir() {
                visit(&path, output, observed, maximum)?;
            } else if file_type.is_file()
                && path
                    .extension()
                    .is_some_and(|extension| extension == "arcw")
            {
                *observed =
                    observed
                        .checked_add(1)
                        .ok_or(ProjectLoadError::ArithmeticOverflow {
                            counter: ProjectLoadLimitKind::Documents,
                        })?;
                if *observed > maximum {
                    return Err(ProjectLoadError::LimitExceeded {
                        kind: ProjectLoadLimitKind::Documents,
                        observed: *observed,
                        maximum,
                    });
                }
                output.push(path);
            }
        }
        Ok(())
    }

    let mut files = Vec::new();
    let mut observed = already_observed;
    visit(source_root, &mut files, &mut observed, maximum_documents)?;
    files.sort();
    Ok(files)
}

fn read_utf8_bounded(
    path: &Path,
    already_observed: u64,
    maximum_bytes: u64,
) -> Result<(String, u64), ProjectLoadError> {
    let file = File::open(path).map_err(|source| ProjectLoadError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    let remaining = maximum_bytes.checked_sub(already_observed).ok_or(
        ProjectLoadError::ArithmeticOverflow {
            counter: ProjectLoadLimitKind::SourceBytes,
        },
    )?;
    let evidence_limit = remaining.saturating_add(1);
    let mut bytes = Vec::new();
    file.take(evidence_limit)
        .read_to_end(&mut bytes)
        .map_err(|source| ProjectLoadError::Read {
            path: path.to_path_buf(),
            source,
        })?;
    let observed =
        u64::try_from(bytes.len()).map_err(|_| ProjectLoadError::ArithmeticOverflow {
            counter: ProjectLoadLimitKind::SourceBytes,
        })?;
    let aggregate =
        already_observed
            .checked_add(observed)
            .ok_or(ProjectLoadError::ArithmeticOverflow {
                counter: ProjectLoadLimitKind::SourceBytes,
            })?;
    if aggregate > maximum_bytes {
        return Err(ProjectLoadError::LimitExceeded {
            kind: ProjectLoadLimitKind::SourceBytes,
            observed: aggregate,
            maximum: maximum_bytes,
        });
    }
    String::from_utf8(bytes)
        .map(|source| (source, observed))
        .map_err(|source| ProjectLoadError::InvalidUtf8 {
            path: path.to_path_buf(),
            source,
        })
}

#[cfg(test)]
mod tests {
    use super::{
        ProjectLoadBudget, ProjectLoadError, ProjectLoadLimitKind, inferred_module_path,
        load_with_limits, project_document_id, reload_with_limits,
    };
    use crate::project_limits::ProjectLoadLimits;
    use arcweft_lang_syntax::{ast::module_path::CanonicalModulePath, incremental::SyntaxDatabase};
    use arcweft_source::SourceName;
    use std::{fs, path::Path, sync::Arc};

    struct ProjectFixture {
        root: std::path::PathBuf,
        manifest: std::path::PathBuf,
        manifest_source: String,
        module_source: String,
    }

    impl ProjectFixture {
        fn new(module_source: &str) -> Self {
            let unique = format!(
                "arcweft-project-limits-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .expect("system clock follows epoch")
                    .as_nanos()
            );
            let root = std::env::temp_dir().join(unique);
            let source_root = root.join("src");
            fs::create_dir_all(&source_root).expect("fixture source root creates");
            let manifest = root.join("arcw.toml");
            let manifest_source = "schema = 1\n[package]\nid = \"org.arcweft.fixtures.project-limits\"\nversion = \"0.1.0\"\n".to_owned();
            fs::write(&manifest, &manifest_source).expect("fixture manifest writes");
            fs::write(source_root.join("main.arcw"), module_source).expect("fixture module writes");
            Self {
                root,
                manifest,
                manifest_source,
                module_source: module_source.to_owned(),
            }
        }

        fn source_bytes(&self) -> u64 {
            u64::try_from(self.manifest_source.len() + self.module_source.len())
                .expect("fixture byte length fits u64")
        }
    }

    impl Drop for ProjectFixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn maps_flat_files_without_mod_rs_layout() {
        let root = Path::new("src");
        assert_eq!(
            inferred_module_path(root, Path::new("src/main.arcw"))
                .unwrap()
                .to_string(),
            "crate"
        );
        assert_eq!(
            inferred_module_path(root, Path::new("src/game/routes/opening.arcw"))
                .unwrap()
                .to_string(),
            "crate.game.routes.opening"
        );
        assert!(matches!(
            inferred_module_path(root, Path::new("src/game/mod.arcw")),
            Err(ProjectLoadError::ModFileLayout { .. })
        ));
    }

    #[test]
    fn project_document_ids_are_package_relative_and_separator_stable() {
        let root = Path::new("D:/workspace/game");
        assert_eq!(
            project_document_id(
                "story-game",
                root,
                Path::new("D:/workspace/game/src/routes/opening.arcw"),
            )
            .expect("project-relative id")
            .as_str(),
            "arcweft-project://story-game/src/routes/opening.arcw"
        );
        assert!(matches!(
            project_document_id(
                "story-game",
                root,
                Path::new("D:/workspace/other/main.arcw"),
            ),
            Err(ProjectLoadError::OutsideProjectRoot { .. })
        ));
    }

    #[test]
    fn bounded_project_load_accepts_exact_document_and_source_byte_limits() {
        let fixture = ProjectFixture::new("flow opening {}\n");
        let mut syntax = SyntaxDatabase::try_new().expect("syntax database");

        let loaded = load_with_limits(
            &mut syntax,
            &fixture.manifest,
            ProjectLoadLimits::new(2, fixture.source_bytes()),
        )
        .expect("exact inclusive limits load");

        assert_eq!(loaded.module_documents().len(), 1);
    }

    #[test]
    fn bounded_project_load_stops_enumeration_at_one_document_over_limit() {
        let fixture = ProjectFixture::new("this source must not be parsed");
        let mut syntax = SyntaxDatabase::try_new().expect("syntax database");

        assert!(matches!(
            load_with_limits(
                &mut syntax,
                &fixture.manifest,
                ProjectLoadLimits::new(1, fixture.source_bytes()),
            ),
            Err(ProjectLoadError::LimitExceeded {
                kind: ProjectLoadLimitKind::Documents,
                observed: 2,
                maximum: 1,
            })
        ));
    }

    #[test]
    fn bounded_project_load_reads_only_one_byte_over_remaining_limit() {
        let fixture = ProjectFixture::new("flow opening {}\n");
        let maximum = fixture.source_bytes() - 1;
        let mut syntax = SyntaxDatabase::try_new().expect("syntax database");

        assert!(matches!(
            load_with_limits(
                &mut syntax,
                &fixture.manifest,
                ProjectLoadLimits::new(2, maximum),
            ),
            Err(ProjectLoadError::LimitExceeded {
                kind: ProjectLoadLimitKind::SourceBytes,
                observed,
                maximum: actual_maximum,
            }) if observed == maximum + 1 && actual_maximum == maximum
        ));
    }

    #[test]
    fn reload_reuses_the_exact_lineage_and_publishes_the_new_document_lease() {
        let mut fixture = ProjectFixture::new("flow opening {}\n");
        let mut syntax = SyntaxDatabase::try_new().expect("syntax database");
        let initial = load_with_limits(
            &mut syntax,
            &fixture.manifest,
            ProjectLoadLimits::new(2, fixture.source_bytes()),
        )
        .expect("initial load");
        let root = CanonicalModulePath::crate_root();
        let initial_parsed = initial
            .module_parsed_source(&root)
            .expect("initial root parse")
            .clone();

        fixture.module_source = "flow revised {}\n".to_owned();
        fs::write(fixture.root.join("src/main.arcw"), &fixture.module_source)
            .expect("revised module writes");
        let revised = reload_with_limits(
            &mut syntax,
            &initial,
            &fixture.manifest,
            ProjectLoadLimits::new(2, fixture.source_bytes()),
        )
        .expect("reload succeeds");
        let revised_parsed = revised
            .module_parsed_source(&root)
            .expect("revised root parse");

        assert_eq!(
            revised_parsed.snapshot_id().lineage(),
            initial_parsed.snapshot_id().lineage()
        );
        assert_eq!(revised_parsed.source_snapshot_id().generation().get(), 2);
        assert_eq!(revised_parsed.source(), fixture.module_source);
        assert!(Arc::ptr_eq(
            revised_parsed.document_lease(),
            revised
                .module_document(&root)
                .expect("revised module document")
        ));
        assert!(Arc::ptr_eq(
            revised_parsed.document_lease(),
            revised
                .sources()
                .module(&root)
                .expect("revised project source")
                .document()
        ));
    }

    #[test]
    fn unchanged_reload_retains_the_exact_parsed_snapshot_and_document_lease() {
        let fixture = ProjectFixture::new("flow opening {}\n");
        let mut syntax = SyntaxDatabase::try_new().expect("syntax database");
        let initial = load_with_limits(
            &mut syntax,
            &fixture.manifest,
            ProjectLoadLimits::new(2, fixture.source_bytes()),
        )
        .expect("initial load");
        let root = CanonicalModulePath::crate_root();
        let initial_parsed = initial
            .module_parsed_source(&root)
            .expect("initial root parse");

        let unchanged = reload_with_limits(
            &mut syntax,
            &initial,
            &fixture.manifest,
            ProjectLoadLimits::new(2, fixture.source_bytes()),
        )
        .expect("unchanged reload succeeds");
        let unchanged_parsed = unchanged
            .module_parsed_source(&root)
            .expect("unchanged root parse");

        assert!(initial_parsed.is_same_snapshot(unchanged_parsed));
        assert!(Arc::ptr_eq(
            initial_parsed.document_lease(),
            unchanged_parsed.document_lease()
        ));
    }

    #[test]
    fn reload_does_not_fallback_when_a_prior_module_identity_changes() {
        let fixture = ProjectFixture::new("flow opening {}\n");
        let mut syntax = SyntaxDatabase::try_new().expect("syntax database");
        let initial = load_with_limits(
            &mut syntax,
            &fixture.manifest,
            ProjectLoadLimits::new(2, fixture.source_bytes()),
        )
        .expect("initial load");
        let changed_manifest = fixture.manifest_source.replace(
            "org.arcweft.fixtures.project-limits",
            "org.arcweft.fixtures.project-rebound",
        );
        fs::write(&fixture.manifest, changed_manifest).expect("changed manifest writes");

        assert!(matches!(
            reload_with_limits(
                &mut syntax,
                &initial,
                &fixture.manifest,
                ProjectLoadLimits::new(u64::MAX, u64::MAX),
            ),
            Err(ProjectLoadError::SourceIdentityMismatch { module, .. })
                if module.as_ref() == &CanonicalModulePath::crate_root()
        ));
    }

    #[test]
    fn reload_continues_from_a_private_recovered_generation_after_rejection() {
        let mut fixture = ProjectFixture::new("flow opening {}\n");
        let mut syntax = SyntaxDatabase::try_new().expect("syntax database");
        let initial = load_with_limits(
            &mut syntax,
            &fixture.manifest,
            ProjectLoadLimits::new(2, fixture.source_bytes()),
        )
        .expect("initial load");
        let root = CanonicalModulePath::crate_root();
        let initial_parsed = initial
            .module_parsed_source(&root)
            .expect("initial root parse");

        fixture.module_source = "flow opening {\n".to_owned();
        fs::write(fixture.root.join("src/main.arcw"), &fixture.module_source)
            .expect("recovered module writes");
        assert!(matches!(
            reload_with_limits(
                &mut syntax,
                &initial,
                &fixture.manifest,
                ProjectLoadLimits::new(2, fixture.source_bytes()),
            ),
            Err(ProjectLoadError::Syntax { .. })
        ));
        let rejected = syntax
            .current(initial_parsed.snapshot_id().lineage())
            .expect("recovered generation remains private");
        assert_eq!(rejected.source_snapshot_id().generation().get(), 2);

        fixture.module_source = "flow revised {}\n".to_owned();
        fs::write(fixture.root.join("src/main.arcw"), &fixture.module_source)
            .expect("corrected module writes");
        let revised = reload_with_limits(
            &mut syntax,
            &initial,
            &fixture.manifest,
            ProjectLoadLimits::new(2, fixture.source_bytes()),
        )
        .expect("reload continues from the current private syntax generation");
        let revised_parsed = revised
            .module_parsed_source(&root)
            .expect("revised root parse");

        assert_eq!(
            revised_parsed.snapshot_id().lineage(),
            initial_parsed.snapshot_id().lineage()
        );
        assert_eq!(revised_parsed.source_snapshot_id().generation().get(), 3);
        assert_eq!(revised_parsed.source(), fixture.module_source);
    }

    #[test]
    fn reload_reuses_a_rejected_new_module_lineage_without_a_published_sidecar() {
        let fixture = ProjectFixture::new("flow opening {}\n");
        let mut syntax = SyntaxDatabase::try_new().expect("syntax database");
        let initial = load_with_limits(
            &mut syntax,
            &fixture.manifest,
            ProjectLoadLimits::new(u64::MAX, u64::MAX),
        )
        .expect("initial load");
        let source_root = fixture.root.join("src");
        let added_path = source_root.join("routes").join("added.arcw");
        fs::create_dir_all(added_path.parent().expect("added module parent"))
            .expect("added module directory");
        fs::write(&added_path, "mod routes.added\nflow added {\n")
            .expect("recovered new module writes");
        let added_module = inferred_module_path(&source_root, &added_path).expect("module path");

        assert!(matches!(
            reload_with_limits(
                &mut syntax,
                &initial,
                &fixture.manifest,
                ProjectLoadLimits::new(u64::MAX, u64::MAX),
            ),
            Err(ProjectLoadError::Syntax { .. })
        ));
        assert!(initial.module_parsed_source(&added_module).is_none());
        let added_name = SourceName::path(added_path.display().to_string());
        let rejected = syntax
            .current_for_source(&added_name)
            .expect("rejected new module remains syntax-session private");
        assert_eq!(rejected.source_snapshot_id().generation().get(), 1);
        assert!(!rejected.diagnostics().is_empty());

        fs::write(&added_path, "mod routes.added\nflow added {}\n")
            .expect("corrected new module writes");
        let revised = reload_with_limits(
            &mut syntax,
            &initial,
            &fixture.manifest,
            ProjectLoadLimits::new(u64::MAX, u64::MAX),
        )
        .expect("corrected new module reload succeeds");
        let accepted = revised
            .module_parsed_source(&added_module)
            .expect("new module accepted");
        let project_source = revised
            .sources()
            .module(&added_module)
            .expect("new project module accepted");

        assert_eq!(
            accepted.snapshot_id().lineage(),
            rejected.snapshot_id().lineage()
        );
        assert_eq!(accepted.source_snapshot_id().generation().get(), 2);
        assert!(accepted.diagnostics().is_empty());
        assert!(Arc::ptr_eq(
            accepted.document_lease(),
            project_source.document()
        ));
    }

    #[test]
    fn bounded_project_load_reports_checked_counter_overflow() {
        let fixture = ProjectFixture::new("");
        let mut document_overflow = ProjectLoadBudget {
            limits: ProjectLoadLimits::new(u64::MAX, u64::MAX),
            documents: u64::MAX,
            source_bytes: 0,
        };
        assert!(matches!(
            document_overflow.read_document(&fixture.manifest),
            Err(ProjectLoadError::ArithmeticOverflow {
                counter: ProjectLoadLimitKind::Documents,
            })
        ));

        let mut byte_overflow = ProjectLoadBudget {
            limits: ProjectLoadLimits::new(1, u64::MAX),
            documents: 0,
            source_bytes: u64::MAX,
        };
        assert!(matches!(
            byte_overflow.read_document(&fixture.manifest),
            Err(ProjectLoadError::ArithmeticOverflow {
                counter: ProjectLoadLimitKind::SourceBytes,
            })
        ));
    }
}
