//! Filesystem adapter for Arcweft package discovery and module graph loading.

use arcweft_lang_syntax::{
    ast::{
        common::UseTreeKind,
        module_path::{CanonicalModulePath, ModulePath, ModulePathError, ModuleSegment},
        symbol_path::ProjectSymbolPath,
    },
    parser::parse_source,
};
use arcweft_launch::SourceBackedLaunchManifest;
use arcweft_project::{
    graph::ModuleDependency,
    manifest::{AuthoredResourceRoots, ProjectManifest, ResourceManifest},
    sources::{ProjectSourceFile, ProjectSources},
};
use arcweft_source::{
    SourceDocument, SourceDocumentError, SourceDocumentId, SourceDocumentIdError, SourceName,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Component, Path, PathBuf},
    sync::Arc,
};
use thiserror::Error;

pub const PROJECT_MANIFEST_FILE: &str = "arcw.toml";

/// Fully loaded project metadata and source inventory.
#[derive(Clone, Debug)]
pub struct LoadedProject {
    sources: ProjectSources,
    manifest_document: Arc<SourceDocument>,
    module_documents: BTreeMap<CanonicalModulePath, Arc<SourceDocument>>,
    launch: SourceBackedLaunchManifest,
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
    #[error("failed to enumerate source directory `{path}`: {source}")]
    Enumerate {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error(transparent)]
    ProjectManifest(#[from] arcweft_project::manifest::ProjectManifestError),
    #[error("failed to parse launch profiles: {0}")]
    LaunchManifest(#[from] arcweft_launch::LaunchProfileError),
    #[error("failed to parse source-backed launch manifest: {0}")]
    LaunchDocument(#[from] arcweft_launch::LaunchDocumentError),
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
}

impl LoadedProject {
    pub(crate) fn from_exact_documents(
        manifest_path: PathBuf,
        project_root: PathBuf,
        manifest: ProjectManifest,
        manifest_document: Arc<SourceDocument>,
        launch: SourceBackedLaunchManifest,
        modules: Vec<ProjectSourceFile>,
    ) -> Result<Self, ProjectLoadError> {
        let module_documents = modules
            .iter()
            .map(|module| (module.module().clone(), Arc::clone(module.document())))
            .collect();
        let sources = ProjectSources::new(manifest_path, project_root, manifest, modules)?;
        Ok(Self {
            sources,
            manifest_document,
            module_documents,
            launch,
        })
    }

    pub const fn sources(&self) -> &ProjectSources {
        &self.sources
    }

    pub const fn launch(&self) -> &SourceBackedLaunchManifest {
        &self.launch
    }

    pub fn manifest_document(&self) -> &Arc<SourceDocument> {
        &self.manifest_document
    }

    pub fn module_documents(
        &self,
    ) -> impl ExactSizeIterator<Item = (&CanonicalModulePath, &Arc<SourceDocument>)> {
        self.module_documents.iter()
    }

    pub fn module_document(&self, module: &CanonicalModulePath) -> Option<&Arc<SourceDocument>> {
        self.module_documents.get(module)
    }

    pub fn into_sources(self) -> ProjectSources {
        self.sources
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
pub fn load_discovered(start: &Path) -> Result<LoadedProject, ProjectLoadError> {
    load(&discover_manifest(start)?)
}

/// Loads only the authored asset/content roots from an explicit `arcw.toml`.
///
/// This accepts launch-only manifests without requiring a package source tree.
pub fn load_authored_resource_roots(
    manifest_path: &Path,
) -> Result<AuthoredResourceRoots, ProjectLoadError> {
    let manifest_source = read_to_string(manifest_path)?;
    let resources = ResourceManifest::parse_project_toml(&manifest_source)?;
    let project_root = manifest_path.parent().unwrap_or_else(|| Path::new("."));
    Ok(resources.resolve(project_root))
}

/// Loads package metadata from an explicit `arcw.toml` without requiring its
/// default source root to exist.
///
/// Launch-profile manifests may point directly at a source outside `src/`.
/// Callers that only need package identity must not trigger project source
/// discovery as a side effect.
pub fn load_project_manifest(manifest_path: &Path) -> Result<ProjectManifest, ProjectLoadError> {
    let manifest_source = read_to_string(manifest_path)?;
    ProjectManifest::parse_toml(&manifest_source).map_err(ProjectLoadError::from)
}

/// Loads one explicit `arcw.toml` and all `.arcw` sources under its source root.
pub fn load(manifest_path: &Path) -> Result<LoadedProject, ProjectLoadError> {
    let manifest_source = read_to_string(manifest_path)?;
    let manifest = ProjectManifest::parse_toml(&manifest_source)?;
    let project_root = manifest_path.parent().unwrap_or_else(|| Path::new("."));
    let package = manifest.package().name().as_str();
    let manifest_document = Arc::new(SourceDocument::try_new(
        project_document_id(package, project_root, manifest_path)?,
        SourceName::path(manifest_path.display().to_string()),
        manifest_source,
    )?);
    let launch = SourceBackedLaunchManifest::parse_document(&manifest_document)?;
    let source_root = manifest.source_root(project_root);
    let source_paths = collect_arcw_files(&source_root)?;
    let scanned = source_paths
        .into_iter()
        .map(|path| scan_source(package, project_root, &source_root, path))
        .collect::<Result<Vec<_>, _>>()?;
    let module_paths = scanned
        .iter()
        .map(|source| source.module.clone())
        .collect::<BTreeSet<_>>();
    let loaded_modules = scanned
        .into_iter()
        .map(|source| source.finish(&module_paths))
        .collect::<Vec<_>>();
    let mut modules = Vec::with_capacity(loaded_modules.len());
    let mut module_documents = BTreeMap::new();
    for (module, document) in loaded_modules {
        module_documents.insert(module.module().clone(), document);
        modules.push(module);
    }
    let sources = ProjectSources::new(
        manifest_path.to_path_buf(),
        project_root.to_path_buf(),
        manifest,
        modules,
    )?;
    Ok(LoadedProject {
        sources,
        manifest_document,
        module_documents,
        launch,
    })
}

#[derive(Clone, Debug)]
struct ScannedSource {
    path: PathBuf,
    document: Arc<SourceDocument>,
    module: CanonicalModulePath,
    imports: Vec<PendingImport>,
}

#[derive(Clone, Debug)]
struct PendingImport {
    path: ProjectSymbolPath,
}

impl ScannedSource {
    fn finish(
        self,
        modules: &BTreeSet<CanonicalModulePath>,
    ) -> (ProjectSourceFile, Arc<SourceDocument>) {
        let dependencies = self
            .imports
            .into_iter()
            .filter_map(|import| {
                longest_known_module_prefix(&import.path, &self.module, modules)
                    .filter(|target| target != &self.module)
                    .map(ModuleDependency::new)
            })
            .collect::<Vec<_>>();
        (
            ProjectSourceFile::new(
                self.module,
                self.path,
                Arc::clone(&self.document),
                dependencies,
            ),
            self.document,
        )
    }
}

fn scan_source(
    package: &str,
    project_root: &Path,
    source_root: &Path,
    path: PathBuf,
) -> Result<ScannedSource, ProjectLoadError> {
    let source = read_to_string(&path)?;
    let parsed = parse_source(&source);
    if !parsed.errors().is_empty() {
        return Err(ProjectLoadError::Syntax {
            path,
            diagnostics: parsed
                .errors()
                .iter()
                .map(|error| error.message().to_owned())
                .collect(),
        });
    }
    let tree = parsed.typed_tree();
    let inferred = inferred_module_path(source_root, &path)?;
    let module = match tree.module() {
        Some(declaration) => {
            let declared = declaration
                .module_path()?
                .resolve_declaration_for(&inferred)?;
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
    let imports = tree
        .uses()
        .iter()
        .map(|item| PendingImport {
            path: match item.tree().kind() {
                UseTreeKind::Path { path, .. } => path.path(),
                UseTreeKind::Glob { module } | UseTreeKind::Group { module, .. } => module.path(),
            }
            .clone(),
        })
        .collect();
    let document = Arc::new(SourceDocument::try_new(
        project_document_id(package, project_root, &path)?,
        SourceName::path(path.display().to_string()),
        source,
    )?);
    Ok(ScannedSource {
        path,
        document,
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

fn collect_arcw_files(source_root: &Path) -> Result<Vec<PathBuf>, ProjectLoadError> {
    fn visit(directory: &Path, output: &mut Vec<PathBuf>) -> Result<(), ProjectLoadError> {
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
                visit(&path, output)?;
            } else if file_type.is_file()
                && path
                    .extension()
                    .is_some_and(|extension| extension == "arcw")
            {
                output.push(path);
            }
        }
        Ok(())
    }

    let mut files = Vec::new();
    visit(source_root, &mut files)?;
    files.sort();
    Ok(files)
}

fn read_to_string(path: &Path) -> Result<String, ProjectLoadError> {
    fs::read_to_string(path).map_err(|source| ProjectLoadError::Read {
        path: path.to_path_buf(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        ProjectLoadError, inferred_module_path, load_project_manifest, project_document_id,
    };
    use std::{fs, path::Path};

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
    fn loads_package_metadata_without_enumerating_default_source_root() {
        let unique = format!(
            "arcweft-project-manifest-metadata-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock follows epoch")
                .as_nanos()
        );
        let root = std::env::temp_dir().join(unique);
        fs::create_dir_all(&root).expect("fixture root creates");
        let manifest_path = root.join("arcw.toml");
        fs::write(
            &manifest_path,
            r#"
[package]
name = "launch-only"
version = "0.1.0"

[profiles.main]
kind = "game"
source = "demo.arcw"
"#,
        )
        .expect("fixture manifest writes");

        let manifest =
            load_project_manifest(&manifest_path).expect("package metadata loads without src");
        assert_eq!(manifest.package().name().as_str(), "launch-only");

        fs::remove_dir_all(root).expect("fixture root removes");
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
}
