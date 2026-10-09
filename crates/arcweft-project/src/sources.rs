use crate::graph::{ModuleDependency, ModuleGraph, ModuleGraphError, ModuleNode};
use arcweft_lang_syntax::ast::module_path::CanonicalModulePath;
use arcweft_manifest_model::{
    BuildSpec, NormalizedProjectPath, NormalizedProjectPathError, PackageSpec,
};
use arcweft_source::{SourceDocument, SourceRevision};
use std::{
    collections::BTreeMap,
    path::{Component, Path, PathBuf},
    sync::Arc,
};
use thiserror::Error;

/// One loaded source and its resolved module dependencies.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectSourceFile {
    module: CanonicalModulePath,
    path: PathBuf,
    document: Arc<SourceDocument>,
    dependencies: Vec<ModuleDependency>,
}

/// Complete Sans I/O source inventory for one package.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectSources {
    manifest_path: PathBuf,
    project_root: PathBuf,
    package: PackageSpec,
    build: BuildSpec,
    manifest_document: Arc<SourceDocument>,
    modules: BTreeMap<CanonicalModulePath, ProjectSourceFile>,
    graph: ModuleGraph,
}

/// Invalid project source inventory.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ProjectSourcesError {
    #[error(transparent)]
    Graph(#[from] ModuleGraphError),
    #[error("project source set is empty")]
    Empty,
    #[error("project must contain `src/main.arcw` or `src/lib.arcw`")]
    MissingRootModule,
    #[error("module `{module}` has more than one source file")]
    DuplicateModule { module: CanonicalModulePath },
}

/// Invalid authored source path in an accepted package inventory.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ProjectSourcePathError {
    #[error("project has no source for module {module}")]
    MissingModule { module: CanonicalModulePath },
    #[error("source for module {module} is outside its project root")]
    OutsideRoot {
        module: CanonicalModulePath,
        project_root: PathBuf,
        path: PathBuf,
    },
    #[error("source path for module {module} is not valid UTF-8")]
    NonUtf8 {
        module: CanonicalModulePath,
        path: PathBuf,
    },
    #[error("source path for module {module} is not a portable project-relative path: {source}")]
    Invalid {
        module: CanonicalModulePath,
        #[source]
        source: NormalizedProjectPathError,
    },
}

impl ProjectSourceFile {
    pub fn new(
        module: CanonicalModulePath,
        path: PathBuf,
        document: Arc<SourceDocument>,
        dependencies: impl IntoIterator<Item = ModuleDependency>,
    ) -> Self {
        let dependencies = ModuleDependency::normalize(dependencies);
        Self {
            module,
            path,
            document,
            dependencies,
        }
    }

    pub const fn module(&self) -> &CanonicalModulePath {
        &self.module
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn source(&self) -> &str {
        self.document.text()
    }

    pub fn document(&self) -> &Arc<SourceDocument> {
        &self.document
    }

    /// Exact content revision owned by the source document.
    pub fn source_revision(&self) -> SourceRevision {
        self.document.identity().revision()
    }

    pub fn dependencies(&self) -> &[ModuleDependency] {
        &self.dependencies
    }
}

impl ProjectSources {
    pub fn new(
        manifest_path: PathBuf,
        project_root: PathBuf,
        package: PackageSpec,
        build: BuildSpec,
        manifest_document: Arc<SourceDocument>,
        modules: impl IntoIterator<Item = ProjectSourceFile>,
    ) -> Result<Self, ProjectSourcesError> {
        let mut module_map = BTreeMap::new();
        for source in modules {
            let module = source.module.clone();
            if module_map.insert(module.clone(), source).is_some() {
                return Err(ProjectSourcesError::DuplicateModule { module });
            }
        }
        if module_map.is_empty() {
            return Err(ProjectSourcesError::Empty);
        }
        if !module_map.contains_key(&CanonicalModulePath::crate_root()) {
            return Err(ProjectSourcesError::MissingRootModule);
        }
        let graph =
            ModuleGraph::new(module_map.values().map(|source| {
                ModuleNode::new(source.module.clone(), source.dependencies.clone())
            }))?;
        Ok(Self {
            manifest_path,
            project_root,
            package,
            build,
            manifest_document,
            modules: module_map,
            graph,
        })
    }

    pub fn manifest_path(&self) -> &Path {
        &self.manifest_path
    }

    pub fn project_root(&self) -> &Path {
        &self.project_root
    }

    pub const fn package(&self) -> &PackageSpec {
        &self.package
    }

    pub const fn build(&self) -> &BuildSpec {
        &self.build
    }

    pub const fn manifest_document(&self) -> &Arc<SourceDocument> {
        &self.manifest_document
    }

    pub const fn graph(&self) -> &ModuleGraph {
        &self.graph
    }

    pub fn modules(&self) -> impl ExactSizeIterator<Item = &ProjectSourceFile> {
        self.modules.values()
    }

    pub fn module(&self, path: &CanonicalModulePath) -> Option<&ProjectSourceFile> {
        self.modules.get(path)
    }

    /// Package root source guaranteed by the constructor invariant.
    pub fn root_module(&self) -> &ProjectSourceFile {
        &self.modules[&CanonicalModulePath::crate_root()]
    }

    pub fn module_by_source_path(&self, path: &Path) -> Option<&ProjectSourceFile> {
        self.modules.values().find(|source| source.path() == path)
    }

    /// Portable authored coordinate derived from this package's exact file inventory.
    ///
    /// This lexical projection performs no filesystem access and leaves the
    /// source document's diagnostic display name unchanged. Components outside
    /// the package, parent traversal, prefixes and non-UTF-8 names are rejected.
    pub fn authored_source_path(
        &self,
        module: &CanonicalModulePath,
    ) -> Result<NormalizedProjectPath, ProjectSourcePathError> {
        let source = self
            .module(module)
            .ok_or_else(|| ProjectSourcePathError::MissingModule {
                module: module.clone(),
            })?;
        let relative = source
            .path()
            .strip_prefix(self.project_root())
            .map_err(|_| ProjectSourcePathError::OutsideRoot {
                module: module.clone(),
                project_root: self.project_root.clone(),
                path: source.path.clone(),
            })?;
        let mut components = Vec::new();
        for component in relative.components() {
            match component {
                Component::Normal(name) => {
                    let name = name
                        .to_str()
                        .ok_or_else(|| ProjectSourcePathError::NonUtf8 {
                            module: module.clone(),
                            path: source.path.clone(),
                        })?;
                    components.push(name);
                }
                Component::CurDir => {}
                Component::ParentDir => components.push(".."),
                Component::RootDir | Component::Prefix(_) => {
                    return Err(ProjectSourcePathError::OutsideRoot {
                        module: module.clone(),
                        project_root: self.project_root.clone(),
                        path: source.path.clone(),
                    });
                }
            }
        }
        NormalizedProjectPath::new(components.join("/")).map_err(|source| {
            ProjectSourcePathError::Invalid {
                module: module.clone(),
                source,
            }
        })
    }

    pub fn target_root(&self) -> PathBuf {
        self.project_root.join(self.build.target_dir.as_path())
    }
}

#[cfg(test)]
mod tests {
    use super::{ProjectSourceFile, ProjectSourcePathError, ProjectSources, SourceDocument};
    use crate::graph::ModuleDependency;
    use arcweft_lang_syntax::ast::module_path::CanonicalModulePath;
    use arcweft_manifest_model::{BuildSpec, PackageId, PackageSpec, PackageVersion};
    use arcweft_source::{SourceDocumentId, SourceName, SourceRevision};
    use std::{path::PathBuf, sync::Arc};

    fn source_file(id: &str, text: &str) -> ProjectSourceFile {
        let document = Arc::new(
            SourceDocument::try_new(
                SourceDocumentId::try_new(id).expect("document ID"),
                SourceName::path("src/main.arcw"),
                text,
            )
            .expect("source document"),
        );
        ProjectSourceFile::new(
            CanonicalModulePath::crate_root(),
            PathBuf::from("src/main.arcw"),
            document,
            std::iter::empty::<ModuleDependency>(),
        )
    }

    #[test]
    fn project_source_revision_is_the_document_revision() {
        let source = source_file("arcweft-project://revision/main.arcw", "fn main() {}\n");

        assert_eq!(
            source.source_revision(),
            source.document().identity().revision()
        );
        assert_eq!(
            source.source_revision(),
            SourceRevision::for_utf8(source.source())
        );
    }

    #[test]
    fn content_revision_does_not_duplicate_document_identity() {
        let first = source_file("arcweft-project://first/main.arcw", "fn main() {}\n");
        let second = source_file("arcweft-project://second/main.arcw", "fn main() {}\n");

        assert_eq!(first.source_revision(), second.source_revision());
        assert_ne!(first.document().identity(), second.document().identity());
    }

    fn project_with_source_path(root: PathBuf, source_path: PathBuf) -> ProjectSources {
        let document = Arc::new(
            SourceDocument::try_new(
                SourceDocumentId::try_new("arcweft-project://portable/src/chapter/序幕.arcw")
                    .unwrap(),
                SourceName::path(source_path.display().to_string()),
                "fn opening() {}\n",
            )
            .unwrap(),
        );
        let manifest = Arc::new(
            SourceDocument::try_new(
                SourceDocumentId::try_new("arcweft-project://portable/arcw.toml").unwrap(),
                SourceName::Memory,
                "",
            )
            .unwrap(),
        );
        ProjectSources::new(
            root.join("arcw.toml"),
            root,
            PackageSpec {
                id: PackageId::new("portable.tests").unwrap(),
                version: PackageVersion::new("0.0.0").unwrap(),
            },
            BuildSpec::default(),
            manifest,
            [ProjectSourceFile::new(
                CanonicalModulePath::crate_root(),
                source_path,
                document,
                [],
            )],
        )
        .unwrap()
    }

    #[test]
    fn authored_source_paths_are_portable_and_preserve_diagnostic_documents() {
        let root = PathBuf::from(std::path::MAIN_SEPARATOR.to_string()).join("portable-checkout");
        let original_path = root.join("src").join("chapter").join("序幕.arcw");
        let project = project_with_source_path(root, original_path.clone());
        let document = Arc::clone(project.root_module().document());
        let identity = document.identity().clone();
        assert_eq!(
            project
                .authored_source_path(&CanonicalModulePath::crate_root())
                .unwrap()
                .as_str(),
            "src/chapter/序幕.arcw"
        );
        assert_eq!(
            document.display_name(),
            &SourceName::path(original_path.display().to_string())
        );
        assert_eq!(document.identity(), &identity);
        assert!(Arc::ptr_eq(&document, project.root_module().document()));
    }

    #[test]
    fn authored_source_paths_are_identical_after_project_relocation() {
        let prefix = PathBuf::from(std::path::MAIN_SEPARATOR.to_string());
        let first_root = prefix.join("first-checkout");
        let second_root = prefix.join("second-checkout");
        let first =
            project_with_source_path(first_root.clone(), first_root.join("src/chapter/序幕.arcw"));
        let second = project_with_source_path(
            second_root.clone(),
            second_root.join("src/chapter/序幕.arcw"),
        );
        assert_eq!(
            first
                .authored_source_path(&CanonicalModulePath::crate_root())
                .unwrap(),
            second
                .authored_source_path(&CanonicalModulePath::crate_root())
                .unwrap()
        );
        assert_ne!(
            first.root_module().document().display_name(),
            second.root_module().document().display_name()
        );
        assert_eq!(
            first.root_module().document().identity(),
            second.root_module().document().identity()
        );
    }

    #[test]
    fn authored_source_paths_reject_foreign_parent_empty_and_unknown_coordinates() {
        let root = PathBuf::from("workspace");
        let foreign = project_with_source_path(root.clone(), PathBuf::from("other/src/main.arcw"));
        assert!(
            matches!(foreign.authored_source_path(&CanonicalModulePath::crate_root()), Err(ProjectSourcePathError::OutsideRoot { module, project_root, path }) if module == CanonicalModulePath::crate_root() && project_root == root && path == std::path::Path::new("other/src/main.arcw"))
        );
        for path in [
            root.join("../escape.arcw"),
            root.clone(),
            root.join("src/bad\n.arcw"),
        ] {
            let project = project_with_source_path(root.clone(), path);
            assert!(
                matches!(project.authored_source_path(&CanonicalModulePath::crate_root()), Err(ProjectSourcePathError::Invalid { module, .. }) if module == CanonicalModulePath::crate_root())
            );
        }
        let project = project_with_source_path(root.clone(), root.join("src/main.arcw"));
        let missing = CanonicalModulePath::from_segments([
            arcweft_lang_syntax::ast::module_path::ModuleSegment::new("missing").unwrap(),
        ]);
        assert_eq!(
            project.authored_source_path(&missing).unwrap_err(),
            ProjectSourcePathError::MissingModule { module: missing }
        );
    }

    #[cfg(any(unix, windows))]
    #[test]
    fn authored_source_paths_reject_non_utf8_components_without_lossy_conversion() {
        #[cfg(unix)]
        let component = {
            use std::os::unix::ffi::OsStringExt;
            std::ffi::OsString::from_vec(vec![0xff])
        };
        #[cfg(windows)]
        let component = {
            use std::os::windows::ffi::OsStringExt;
            std::ffi::OsString::from_wide(&[0xd800])
        };
        let root = PathBuf::from("workspace");
        let path = root.join("src").join(component);
        let project = project_with_source_path(root, path.clone());
        assert_eq!(
            project
                .authored_source_path(&CanonicalModulePath::crate_root())
                .unwrap_err(),
            ProjectSourcePathError::NonUtf8 {
                module: CanonicalModulePath::crate_root(),
                path
            }
        );
    }
}
