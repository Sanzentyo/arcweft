use crate::{
    graph::{ModuleDependency, ModuleGraph, ModuleGraphError, ModuleNode},
    manifest::ProjectManifest,
};
use arcweft_lang_syntax::ast::module_path::CanonicalModulePath;
use arcweft_source::SourceDocument;
use std::{
    collections::BTreeMap,
    fmt::Write as _,
    path::{Path, PathBuf},
    sync::Arc,
};
use thiserror::Error;

/// Content hash used by deterministic incremental build keys.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ModuleSourceHash([u8; 32]);

/// One loaded source and its resolved module dependencies.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectSourceFile {
    module: CanonicalModulePath,
    path: PathBuf,
    document: Arc<SourceDocument>,
    source_hash: ModuleSourceHash,
    dependencies: Vec<ModuleDependency>,
}

/// Complete Sans I/O source inventory for one package.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectSources {
    manifest_path: PathBuf,
    project_root: PathBuf,
    manifest: ProjectManifest,
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

impl ModuleSourceHash {
    pub fn from_source(source: &str) -> Self {
        Self(*blake3::hash(source.as_bytes()).as_bytes())
    }

    pub const fn as_bytes(self) -> [u8; 32] {
        self.0
    }

    pub fn to_hex(self) -> String {
        self.0
            .iter()
            .fold(String::with_capacity(64), |mut hex, byte| {
                write!(&mut hex, "{byte:02x}").expect("writing to String cannot fail");
                hex
            })
    }
}

impl ProjectSourceFile {
    pub fn new(
        module: CanonicalModulePath,
        path: PathBuf,
        document: Arc<SourceDocument>,
        dependencies: impl IntoIterator<Item = ModuleDependency>,
    ) -> Self {
        let source_hash = ModuleSourceHash::from_source(document.text());
        let dependencies = ModuleDependency::normalize(dependencies);
        Self {
            module,
            path,
            document,
            source_hash,
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

    pub const fn source_hash(&self) -> ModuleSourceHash {
        self.source_hash
    }

    pub fn dependencies(&self) -> &[ModuleDependency] {
        &self.dependencies
    }
}

impl ProjectSources {
    pub fn new(
        manifest_path: PathBuf,
        project_root: PathBuf,
        manifest: ProjectManifest,
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
            manifest,
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

    pub const fn manifest(&self) -> &ProjectManifest {
        &self.manifest
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
        self.manifest.target_root(&self.project_root)
    }
}
