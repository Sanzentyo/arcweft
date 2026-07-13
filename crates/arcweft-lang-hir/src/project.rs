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
    /// attributes. Inline style patch ordinals are rebased from the appended
    /// module's local ordinal space into this linked module's global ordinal
    /// space. HIR does not yet own a separate style-application reference ABI;
    /// d.2 compiler lowering resolves applications from checked catalogs and
    /// the source View structure. The module-preserving owner remains
    /// [`HirProject`].
    ///
    /// # Panics
    ///
    /// Panics if the linked module would contain more inline style patches
    /// than a `u32` ordinal can identify.
    pub fn append_module_body(&mut self, mut module: Self) {
        self.source_len = None;
        self.top_level_ranges.clear();
        let style_patch_base = u32::try_from(self.style_patches.len())
            .expect("linked HIR contains more inline style patches than u32 can identify");
        module
            .style_patches
            .iter_mut()
            .for_each(|patch| patch.rebase_ordinal(style_patch_base));
        self.flows.append(&mut module.flows);
        self.functions.append(&mut module.functions);
        self.agents.append(&mut module.agents);
        self.declarations.append(&mut module.declarations);
        self.style_patches.append(&mut module.style_patches);
        self.top_level_items.append(&mut module.top_level_items);
    }
}

#[cfg(test)]
mod tests {
    use super::{HirProject, HirProjectModule};
    use crate::lower::lower_to_hir;
    use crate::style::HirStylePatch;
    use arcweft_lang_syntax::{
        ast::{
            common::TextRange,
            module_path::{CanonicalModulePath, ModuleSegment},
        },
        parser::parse_source,
    };
    use std::collections::BTreeSet;

    struct LinkedStyleProject {
        project: HirProject,
        alpha_path: CanonicalModulePath,
        omega_path: CanonicalModulePath,
        patch_ranges: [TextRange; 4],
    }

    fn linked_style_project() -> LinkedStyleProject {
        let root = lower_to_hir(
            &parse_source(
                r#"pub view Root() {
    Button("root").style { opacity = 100milli }
}
"#,
            )
            .into_typed_tree(),
        )
        .unwrap();
        let alpha = lower_to_hir(
            &parse_source(
                r#"pub view Alpha() {
    Button("alpha")
        .style { outline-width = 2px }
        .style { opacity = 200milli }
}
"#,
            )
            .into_typed_tree(),
        )
        .unwrap();
        let omega = lower_to_hir(
            &parse_source(
                r#"pub view Omega() {
    Button("omega").style { width = 3px }
}
"#,
            )
            .into_typed_tree(),
        )
        .unwrap();
        let patch_ranges = [
            root.style_patches()[0].range(),
            alpha.style_patches()[0].range(),
            alpha.style_patches()[1].range(),
            omega.style_patches()[0].range(),
        ];
        let alpha_path =
            CanonicalModulePath::crate_root().join(ModuleSegment::new("alpha").unwrap());
        let omega_path =
            CanonicalModulePath::crate_root().join(ModuleSegment::new("omega").unwrap());
        let project = HirProject::new(
            "game",
            [
                HirProjectModule::new(omega_path.clone(), omega),
                HirProjectModule::new(CanonicalModulePath::crate_root(), root),
                HirProjectModule::new(alpha_path.clone(), alpha),
            ],
        )
        .unwrap();
        LinkedStyleProject {
            project,
            alpha_path,
            omega_path,
            patch_ranges,
        }
    }

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

    #[test]
    fn linked_view_preserves_and_rebases_inline_style_patches_in_module_order() {
        let fixture = linked_style_project();

        assert_eq!(
            fixture
                .project
                .module(&fixture.alpha_path)
                .expect("alpha module")
                .style_patches()
                .iter()
                .map(HirStylePatch::ordinal)
                .collect::<Vec<_>>(),
            [0, 1]
        );
        assert_eq!(
            fixture
                .project
                .module(&fixture.omega_path)
                .expect("omega module")
                .style_patches()[0]
                .ordinal(),
            0
        );

        let linked = fixture.project.linked_module();
        assert_eq!(linked.style_patches().len(), 4);
        assert_eq!(
            linked
                .style_patches()
                .iter()
                .map(HirStylePatch::ordinal)
                .collect::<Vec<_>>(),
            [0, 1, 2, 3]
        );
        assert_eq!(
            linked
                .style_patches()
                .iter()
                .map(HirStylePatch::ordinal)
                .collect::<BTreeSet<_>>()
                .len(),
            linked.style_patches().len(),
            "downstream ordinal-based patch references must remain collision-free"
        );
    }

    #[test]
    fn linked_view_preserves_inline_patch_bodies_and_source_ranges() {
        let fixture = linked_style_project();
        let linked = fixture.project.linked_module();
        assert_eq!(
            linked
                .style_patches()
                .iter()
                .map(HirStylePatch::range)
                .collect::<Vec<_>>(),
            fixture.patch_ranges
        );
        assert_eq!(linked.style_patches()[1].declarations().len(), 1);
        assert_eq!(
            linked.style_patches()[1].declarations()[0].value().source(),
            "2px"
        );
        assert_eq!(linked.style_patches()[2].declarations().len(), 1);
        assert_eq!(
            linked.style_patches()[3].declarations()[0].value().source(),
            "3px"
        );
    }
}
