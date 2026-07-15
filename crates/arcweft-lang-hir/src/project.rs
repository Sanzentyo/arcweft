//! Multi-module HIR container and transitional crate-level link view.

use crate::model::{HirFlowItem, HirModule};
use crate::symbol::{
    CallablePackageId, CallablePackageIdError, ProjectExternalDeclarations,
    ProjectSymbolLinkOutput, ProjectSymbolLinkReport, ProjectSymbolTable,
};
use arcweft_lang_syntax::ast::module_path::CanonicalModulePath;
use arcweft_source::SourceDocumentIdentity;
use std::collections::BTreeMap;
use thiserror::Error;

/// One canonical module and its independently lowered HIR.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirProjectModule {
    module: CanonicalModulePath,
    source: SourceDocumentIdentity,
    hir: HirModule,
}

/// Module-preserving HIR for one Arcweft package.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirProject {
    package: CallablePackageId,
    modules: BTreeMap<CanonicalModulePath, HirModule>,
    sources: BTreeMap<CanonicalModulePath, SourceDocumentIdentity>,
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
    /// Binds one lowered HIR module to its canonical module and source identity.
    ///
    /// # Panics
    ///
    /// Panics when `hir` was not lowered from a revision-bound source document or when that
    /// document identity differs from `source`.
    pub fn new(
        module: CanonicalModulePath,
        source: SourceDocumentIdentity,
        mut hir: HirModule,
    ) -> Self {
        let bound_source = hir
            .source_identity()
            .expect("project HIR must be lowered from a revision-bound source document");
        assert_eq!(
            bound_source, &source,
            "project HIR source identity must match its module identity"
        );
        hir.assign_declaration_module(&module);
        Self {
            module,
            source,
            hir,
        }
    }

    pub const fn module(&self) -> &CanonicalModulePath {
        &self.module
    }

    pub const fn source(&self) -> &SourceDocumentIdentity {
        &self.source
    }

    pub const fn hir(&self) -> &HirModule {
        &self.hir
    }

    pub fn into_parts(self) -> (CanonicalModulePath, SourceDocumentIdentity, HirModule) {
        (self.module, self.source, self.hir)
    }
}

impl HirProject {
    pub fn new(
        package: impl Into<String>,
        modules: impl IntoIterator<Item = HirProjectModule>,
    ) -> Result<Self, HirProjectError> {
        let package = CallablePackageId::try_new(package)?;
        let mut module_map = BTreeMap::new();
        let mut sources = BTreeMap::new();
        for module in modules {
            let (path, source, hir) = module.into_parts();
            if module_map.insert(path.clone(), hir).is_some() {
                return Err(HirProjectError::DuplicateModule { module: path });
            }
            sources.insert(path, source);
        }
        if !module_map.contains_key(&CanonicalModulePath::crate_root()) {
            return Err(HirProjectError::MissingRootModule);
        }
        Ok(Self {
            package,
            modules: module_map,
            sources,
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

    pub fn source(&self, path: &CanonicalModulePath) -> Option<&SourceDocumentIdentity> {
        self.sources.get(path)
    }

    pub(crate) fn source_identities(
        &self,
    ) -> impl ExactSizeIterator<Item = (&CanonicalModulePath, &SourceDocumentIdentity)> {
        self.sources.iter()
    }

    /// Links project declarations, imports, and typed external declarations.
    pub fn project_symbols(
        &self,
        externals: &ProjectExternalDeclarations,
    ) -> Result<ProjectSymbolLinkOutput, ProjectSymbolLinkReport> {
        ProjectSymbolTable::link(self, externals)
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
        self.module_path.clone_from(path);
        self.bind_project_module(path);
        for flow in &mut self.flows {
            flow.module_path = Some(path.clone());
            assign_flow_item_modules(&mut flow.body, path);
        }
        for function in &mut self.functions {
            function.module_path = Some(path.clone());
        }
        for agent in &mut self.agents {
            agent.module_path = Some(path.clone());
        }
        for declaration in &mut self.declarations {
            if let crate::model::HirTopLevelDecl::Source(source) = declaration {
                source.bind_project_module(path);
            }
        }
        self.view_parts
            .iter_mut()
            .for_each(|owner| owner.assign_module(path));
        assign_flow_item_modules(&mut self.top_level_items, path);
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
        self.merge_project_sources(&mut module);
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
        self.view_parts.append(&mut module.view_parts);
        self.top_level_items.append(&mut module.top_level_items);
    }
}

fn assign_flow_item_modules(items: &mut [HirFlowItem], path: &CanonicalModulePath) {
    for item in items {
        assign_flow_item_module(item, path);
    }
}

fn assign_flow_item_module(item: &mut HirFlowItem, path: &CanonicalModulePath) {
    match item {
        HirFlowItem::Dialogue(dialogue) => dialogue.source_module = Some(path.clone()),
        HirFlowItem::Thread(thread) => assign_flow_item_modules(&mut thread.body, path),
        HirFlowItem::If(block) => {
            assign_flow_item_modules(&mut block.body, path);
            assign_flow_item_modules(&mut block.else_body, path);
        }
        HirFlowItem::IfLet(block) => {
            assign_flow_item_modules(&mut block.body, path);
            assign_flow_item_modules(&mut block.else_body, path);
        }
        HirFlowItem::Match(block) => block
            .arms
            .iter_mut()
            .for_each(|arm| assign_flow_item_modules(&mut arm.body, path)),
        HirFlowItem::Loop(block) | HirFlowItem::LetLoop { block, .. } => {
            assign_flow_item_modules(&mut block.body, path);
        }
        HirFlowItem::While(block) => assign_flow_item_modules(&mut block.body, path),
        HirFlowItem::WhileLet(block) => assign_flow_item_modules(&mut block.body, path),
        HirFlowItem::For(block) => assign_flow_item_modules(&mut block.body, path),
        HirFlowItem::Select(block) => block
            .branches
            .iter_mut()
            .for_each(|branch| assign_flow_item_modules(&mut branch.body, path)),
        HirFlowItem::SourceLocale(block) => assign_flow_item_modules(&mut block.body, path),
        HirFlowItem::Scope(block) => assign_flow_item_modules(&mut block.body, path),
        HirFlowItem::Await(block)
        | HirFlowItem::LetAwait {
            await_with: block, ..
        } => block
            .branches
            .iter_mut()
            .for_each(|branch| assign_flow_item_modules(&mut branch.body, path)),
        HirFlowItem::Stmt(_)
        | HirFlowItem::Choice(_)
        | HirFlowItem::LetChoice { .. }
        | HirFlowItem::LetScope { .. }
        | HirFlowItem::Include(_) => {}
    }
}

#[cfg(test)]
mod tests {
    use super::{HirProject, HirProjectModule};
    use crate::lower::lower_document_to_hir;
    use crate::model::HirFlowItem;
    use crate::style::HirStylePatch;
    use arcweft_lang_syntax::{
        ast::{
            common::TextRange,
            module_path::{CanonicalModulePath, ModuleSegment},
        },
        parser::parse_source,
    };
    use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};
    use std::collections::BTreeSet;

    struct LinkedStyleProject {
        project: HirProject,
        alpha_path: CanonicalModulePath,
        omega_path: CanonicalModulePath,
        patch_ranges: [TextRange; 4],
    }

    fn linked_style_project() -> LinkedStyleProject {
        let root_source = r#"pub view Root() {
    Button("root").style { opacity = 100milli }
}
"#;
        let (root_document, root) = lower_bound("root", root_source);
        let alpha_source = r#"pub view Alpha() {
    Button("alpha")
        .style { outline-width = 2px }
        .style { opacity = 200milli }
}
"#;
        let (alpha_document, alpha) = lower_bound("alpha", alpha_source);
        let omega_source = r#"pub view Omega() {
    Button("omega").style { width = 3px }
}
"#;
        let (omega_document, omega) = lower_bound("omega", omega_source);
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
                HirProjectModule::new(omega_path.clone(), omega_document.identity().clone(), omega),
                HirProjectModule::new(
                    CanonicalModulePath::crate_root(),
                    root_document.identity().clone(),
                    root,
                ),
                HirProjectModule::new(alpha_path.clone(), alpha_document.identity().clone(), alpha),
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
        let root_source = "#![generated(tool)]\nflow @root root {}";
        let (root_document, root) = lower_bound("root-linked", root_source);
        let child_source = "flow @child child {}\npub fn helper() -> i32 { 1 }";
        let (child_document, child) = lower_bound("child-linked", child_source);
        let child_path =
            CanonicalModulePath::crate_root().join(ModuleSegment::new("child").unwrap());
        let project = HirProject::new(
            "game",
            [
                HirProjectModule::new(
                    CanonicalModulePath::crate_root(),
                    root_document.identity().clone(),
                    root,
                ),
                HirProjectModule::new(child_path, child_document.identity().clone(), child),
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
    fn project_module_assigns_source_path_through_nested_flow_families() {
        let source = r"flow opening {
    scope outer {
        if true {
            thread worker {
                alice: Inside[p]
            }
        } else {
            alice: Else[p]
        }
    }
}
";
        let parsed = parse_source(source);
        assert_eq!(parsed.errors(), &[]);
        let document = source_document("nested-flow", source);
        let hir =
            lower_document_to_hir(&document, parsed.typed_tree()).expect("nested flow lowers");
        let child_path =
            CanonicalModulePath::crate_root().join(ModuleSegment::new("child").unwrap());
        let module = HirProjectModule::new(child_path.clone(), document.identity().clone(), hir);

        let HirFlowItem::Scope(scope) = &module.hir().flows()[0].body()[0] else {
            panic!("outer scope must lower");
        };
        let HirFlowItem::If(if_block) = &scope.body()[0] else {
            panic!("if block must lower");
        };
        let HirFlowItem::Thread(thread) = &if_block.body()[0] else {
            panic!("thread must lower");
        };
        let HirFlowItem::Dialogue(inside) = &thread.body()[0] else {
            panic!("thread dialogue must lower");
        };
        let HirFlowItem::Dialogue(otherwise) = &if_block.else_body()[0] else {
            panic!("else dialogue must lower");
        };
        assert_eq!(inside.source_module(), Some(&child_path));
        assert_eq!(otherwise.source_module(), Some(&child_path));
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

    fn source_document(label: &str, source: &str) -> SourceDocument {
        SourceDocument::try_new(
            SourceDocumentId::try_new(format!("memory:///{label}.arcw")).unwrap(),
            SourceName::Generated,
            source,
        )
        .expect("source document")
    }

    fn lower_bound(label: &str, source: &str) -> (SourceDocument, crate::model::HirModule) {
        let document = source_document(label, source);
        let parsed = parse_source(source);
        let hir = lower_document_to_hir(&document, parsed.typed_tree()).expect("source lowers");
        (document, hir)
    }

    #[test]
    fn document_bound_hir_exposes_exact_source_and_module_identity() {
        let (document, hir) = lower_bound("identity", "");

        assert_eq!(hir.source_identity(), Some(document.identity()));
        assert_eq!(hir.module_path(), &CanonicalModulePath::crate_root());

        let child_path =
            CanonicalModulePath::crate_root().join(ModuleSegment::new("child").unwrap());
        let child = HirProjectModule::new(child_path.clone(), document.identity().clone(), hir);
        assert_eq!(child.hir().module_path(), &child_path);
        assert_eq!(child.hir().source_identity(), Some(document.identity()));
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
