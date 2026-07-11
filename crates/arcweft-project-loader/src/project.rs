//! Filesystem adapter for Arcweft package discovery and module graph loading.

use arcweft_lang_syntax::{
    ast::module_path::{CanonicalModulePath, ModulePath, ModulePathError, ModuleSegment},
    parser::parse_source,
};
use arcweft_launch::LaunchProfileManifest;
use arcweft_project::{
    graph::ModuleDependency,
    manifest::{AuthoredResourceRoots, ProjectManifest, ResourceManifest},
    sources::{ProjectSourceFile, ProjectSources},
};
use std::{
    collections::BTreeSet,
    fs,
    path::{Component, Path, PathBuf},
};
use thiserror::Error;

pub const PROJECT_MANIFEST_FILE: &str = "arcw.toml";

/// Fully loaded project metadata and source inventory.
#[derive(Clone, Debug)]
pub struct LoadedProject {
    sources: ProjectSources,
    launch: LaunchProfileManifest,
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
    #[error("source file `{path}` is outside source root `{source_root}`")]
    OutsideSourceRoot { path: PathBuf, source_root: PathBuf },
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
    pub const fn sources(&self) -> &ProjectSources {
        &self.sources
    }

    pub const fn launch(&self) -> &LaunchProfileManifest {
        &self.launch
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
    let launch = LaunchProfileManifest::parse_toml(&manifest_source)?;
    let project_root = manifest_path.parent().unwrap_or_else(|| Path::new("."));
    let source_root = manifest.source_root(project_root);
    let source_paths = collect_arcw_files(&source_root)?;
    let scanned = source_paths
        .into_iter()
        .map(|path| scan_source(&source_root, path))
        .collect::<Result<Vec<_>, _>>()?;
    let module_paths = scanned
        .iter()
        .map(|source| source.module.clone())
        .collect::<BTreeSet<_>>();
    let modules = scanned
        .into_iter()
        .map(|source| source.finish(&module_paths))
        .collect::<Result<Vec<_>, _>>()?;
    let sources = ProjectSources::new(
        manifest_path.to_path_buf(),
        project_root.to_path_buf(),
        manifest,
        modules,
    )?;
    Ok(LoadedProject { sources, launch })
}

#[derive(Clone, Debug)]
struct ScannedSource {
    path: PathBuf,
    source: String,
    module: CanonicalModulePath,
    imports: Vec<PendingImport>,
}

#[derive(Clone, Debug)]
struct PendingImport {
    spelling: String,
    path: ModulePath,
    exact_module_prefix: bool,
}

impl ScannedSource {
    fn finish(
        self,
        modules: &BTreeSet<CanonicalModulePath>,
    ) -> Result<ProjectSourceFile, ProjectLoadError> {
        let dependencies = self
            .imports
            .into_iter()
            .filter_map(|import| {
                let resolved = match import.path.resolve_from(&self.module) {
                    Ok(path) => path,
                    Err(error) => return Some(Err(ProjectLoadError::ModulePath(error))),
                };
                let target = if import.exact_module_prefix {
                    modules.contains(&resolved).then_some(resolved)
                } else {
                    resolved
                        .ancestors_inclusive()
                        .find(|candidate| modules.contains(candidate))
                };
                match target {
                    Some(target) if target != self.module => {
                        Some(Ok(ModuleDependency::new(target)))
                    }
                    Some(_) => None,
                    None => Some(Err(ProjectLoadError::UnresolvedImport {
                        path: self.path.clone(),
                        module: self.module.clone(),
                        import: import.spelling,
                    })),
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(ProjectSourceFile::new(
            self.module,
            self.path,
            self.source,
            dependencies,
        ))
    }
}

fn scan_source(source_root: &Path, path: PathBuf) -> Result<ScannedSource, ProjectLoadError> {
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
        .map(|item| {
            Ok(PendingImport {
                spelling: item.tree().source().to_owned(),
                path: item.tree().module_path_prefix().clone(),
                exact_module_prefix: item.tree().module_path_is_exact(),
            })
        })
        .collect::<Result<Vec<_>, ModulePathError>>()?;
    Ok(ScannedSource {
        path,
        source,
        module,
        imports,
    })
}

fn inferred_module_path(
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
    use super::{ProjectLoadError, inferred_module_path, load_project_manifest};
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
}
