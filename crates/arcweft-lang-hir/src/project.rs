//! Multi-module HIR container and transitional crate-level link view.

use crate::model::HirModule;
use crate::symbol::{
    CallableLinkError, CallablePackageId, CallablePackageIdError, CallableSymbolTable,
};
use arcweft_lang_syntax::ast::module_path::CanonicalModulePath;
use std::collections::BTreeMap;
use thiserror::Error;

/// One canonical module and its independently lowered HIR.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirProjectModule {
    path: CanonicalModulePath,
    hir: HirModule,
}

/// Module-preserving HIR for one Arcweft package.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirProject {
    package: CallablePackageId,
    modules: BTreeMap<CanonicalModulePath, HirModule>,
}

/// Invalid module-preserving HIR project.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum HirProjectError {
    #[error(transparent)]
    InvalidPackage(#[from] CallablePackageIdError),
    #[error("HIR project contains duplicate module `{module}`")]
    DuplicateModule { module: CanonicalModulePath },
    #[error("HIR project does not contain the crate root module")]
    MissingRootModule,
}

impl HirProjectModule {
    pub fn new(path: CanonicalModulePath, mut hir: HirModule) -> Self {
        hir.assign_declaration_module(&path);
        Self { path, hir }
    }

    pub const fn path(&self) -> &CanonicalModulePath {
        &self.path
    }

    pub const fn hir(&self) -> &HirModule {
        &self.hir
    }

    pub fn into_parts(self) -> (CanonicalModulePath, HirModule) {
        (self.path, self.hir)
    }
}

impl HirProject {
    pub fn new(
        package: impl Into<String>,
        modules: impl IntoIterator<Item = HirProjectModule>,
    ) -> Result<Self, HirProjectError> {
        let package = CallablePackageId::try_new(package)?;
        let mut module_map = BTreeMap::new();
        for module in modules {
            let (path, hir) = module.into_parts();
            if module_map.insert(path.clone(), hir).is_some() {
                return Err(HirProjectError::DuplicateModule { module: path });
            }
        }
        if !module_map.contains_key(&CanonicalModulePath::crate_root()) {
            return Err(HirProjectError::MissingRootModule);
        }
        Ok(Self {
            package,
            modules: module_map,
        })
    }

    pub const fn package(&self) -> &CallablePackageId {
        &self.package
    }

    pub fn modules(&self) -> impl ExactSizeIterator<Item = (&CanonicalModulePath, &HirModule)> {
        self.modules.iter()
    }

    pub fn module(&self, path: &CanonicalModulePath) -> Option<&HirModule> {
        self.modules.get(path)
    }

    /// Builds the package's ordinary/Fx callable alias table.
    pub fn callable_symbols(&self) -> Result<CallableSymbolTable, Vec<CallableLinkError>> {
        CallableSymbolTable::build(self)
    }

    /// Builds the current crate-global semantic-pass view.
    ///
    /// Module boundaries remain preserved in `HirProject`. This linked view is
    /// intentionally transitional until resolver/type-checker entry points
    /// consume `HirProject` directly. Only crate-root source attributes become
    /// crate attributes; child-module attributes remain on their source HIR.
    ///
    /// # Panics
    ///
    /// Panics only if the constructor invariant is broken and the required
    /// crate-root module is missing from an already-constructed `HirProject`.
    pub fn linked_module(&self) -> HirModule {
        let root_path = CanonicalModulePath::crate_root();
        let mut linked = self
            .modules
            .get(&root_path)
            .expect("HIR project constructor requires a root module")
            .clone();
        for (path, module) in &self.modules {
            if path != &root_path {
                linked.append_module_body(module.clone());
            }
        }
        linked
    }
}

impl HirModule {
    fn assign_declaration_module(&mut self, path: &CanonicalModulePath) {
        for flow in &mut self.flows {
            flow.module_path = Some(path.clone());
        }
        for function in &mut self.functions {
            function.module_path = Some(path.clone());
        }
        for agent in &mut self.agents {
            agent.module_path = Some(path.clone());
        }
    }

    /// Appends declarations and executable bodies from another source module.
    ///
    /// Source-level attributes are intentionally not promoted to crate-level
    /// attributes. The module-preserving owner remains [`HirProject`].
    pub fn append_module_body(&mut self, mut module: Self) {
        self.source_len = None;
        self.top_level_ranges.clear();
        self.flows.append(&mut module.flows);
        self.functions.append(&mut module.functions);
        self.agents.append(&mut module.agents);
        self.declarations.append(&mut module.declarations);
        self.top_level_items.append(&mut module.top_level_items);
    }
}

#[cfg(test)]
mod tests {
    use super::{HirProject, HirProjectModule};
    use crate::lower::lower_to_hir;
    use arcweft_lang_syntax::{
        ast::module_path::{CanonicalModulePath, ModuleSegment},
        parser::parse_source,
    };

    #[test]
    fn linked_view_preserves_root_attributes_and_appends_child_body() {
        let root = lower_to_hir(
            &parse_source("#![generated(tool)]\nflow @root root {}").into_typed_tree(),
        )
        .unwrap();
        let child = lower_to_hir(
            &parse_source("flow @child child {}\npub fn helper() -> i32 { 1 }").into_typed_tree(),
        )
        .unwrap();
        let child_path =
            CanonicalModulePath::crate_root().join(ModuleSegment::new("child").unwrap());
        let project = HirProject::new(
            "game",
            [
                HirProjectModule::new(CanonicalModulePath::crate_root(), root),
                HirProjectModule::new(child_path, child),
            ],
        )
        .unwrap();
        let linked = project.linked_module();
        assert_eq!(linked.attributes().len(), 1);
        assert_eq!(linked.flows().len(), 2);
        assert_eq!(project.package().as_str(), "game");
        let child = project
            .modules()
            .find_map(|(path, module)| (!path.is_crate_root()).then_some(module))
            .expect("child module");
        assert_eq!(child.functions()[0].qualified_name(), "child.helper");
    }
}
