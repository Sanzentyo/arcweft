use crate::graph::{ModuleDependency, ModuleGraph, ModuleGraphError, ModuleNode};
use arcweft_lang_syntax::ast::module_path::CanonicalModulePath;
use arcweft_manifest_model::{BuildSpec, PackageSpec};
use arcweft_source::{SourceDocument, SourceRevision};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
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

    pub fn target_root(&self) -> PathBuf {
        self.project_root.join(self.build.target_dir.as_path())
    }
}

#[cfg(test)]
mod tests {
    use super::{ProjectSourceFile, SourceDocument};
    use crate::graph::ModuleDependency;
    use arcweft_lang_syntax::ast::module_path::CanonicalModulePath;
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
}
