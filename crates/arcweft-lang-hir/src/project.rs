//! Multi-module HIR container and transitional crate-level link view.

use crate::callable_source::{
    HirCallableEffects, HirCallableParameterSource, HirCallableSignatureSource, HirEffectName,
};
use crate::model::{HirFlowItem, HirFunction, HirModule, HirTopLevelDecl};
use crate::symbol::{
    CallableDeclarationId, CallablePackageId, CallablePackageIdError, ProjectExternalDeclarations,
    ProjectSymbolLinkOutput, ProjectSymbolLinkReport, ProjectSymbolTable,
};
use arcweft_lang_syntax::ast::{
    flow::ContractClause,
    items::{CapabilityFn, ExternCapabilityItem},
    module_path::{CanonicalModulePath, ModulePathRoot, ModuleSegment},
    symbol_path::SymbolPath,
};
use arcweft_source::SourceDocumentIdentity;
use std::{collections::BTreeMap, ops::Range};
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
    callable_signature_sources: Vec<HirCallableSignatureSource>,
    callable_signature_ranges: BTreeMap<CanonicalModulePath, Range<usize>>,
}

type CallableSignaturePublication = (
    Vec<HirCallableSignatureSource>,
    BTreeMap<CanonicalModulePath, Range<usize>>,
);

/// Invalid module-preserving HIR project.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum HirProjectError {
    #[error(transparent)]
    InvalidPackage(#[from] CallablePackageIdError),
    #[error("HIR project contains duplicate module `{module}`")]
    DuplicateModule { module: CanonicalModulePath },
    #[error("HIR project does not contain the crate root module")]
    MissingRootModule,
    #[error("HIR callable `{name}` in module `{module}` has invalid source publication: {reason}")]
    InvalidCallableSource {
        module: CanonicalModulePath,
        name: String,
        reason: String,
    },
}

/// Invalid binding between one canonical module and its lowered HIR source.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum HirProjectModuleError {
    #[error("HIR module `{module}` is not bound to a source document")]
    MissingSourceDocument { module: CanonicalModulePath },
    #[error("HIR module `{module}` is bound to another source revision")]
    SourceIdentityMismatch {
        module: CanonicalModulePath,
        expected: SourceDocumentIdentity,
        actual: SourceDocumentIdentity,
    },
}

impl HirProjectModule {
    /// Binds one lowered HIR module to its canonical module and source identity.
    #[allow(
        clippy::result_large_err,
        reason = "the exact module-binding error preserves both complete source identities"
    )]
    pub fn try_new(
        module: CanonicalModulePath,
        source: SourceDocumentIdentity,
        mut hir: HirModule,
    ) -> Result<Self, HirProjectModuleError> {
        let bound_source =
            hir.source_identity()
                .ok_or_else(|| HirProjectModuleError::MissingSourceDocument {
                    module: module.clone(),
                })?;
        if bound_source != &source {
            return Err(HirProjectModuleError::SourceIdentityMismatch {
                module,
                expected: source,
                actual: bound_source.clone(),
            });
        }
        hir.assign_declaration_module(&module);
        Ok(Self {
            module,
            source,
            hir,
        })
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
        let (callable_signature_sources, callable_signature_ranges) =
            build_callable_signature_sources(&package, &module_map)?;
        Ok(Self {
            package,
            modules: module_map,
            sources,
            callable_signature_sources,
            callable_signature_ranges,
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

    /// Callable signatures in canonical module order and source declaration order.
    pub fn callable_signature_sources(
        &self,
    ) -> impl ExactSizeIterator<Item = &HirCallableSignatureSource> {
        self.callable_signature_sources.iter()
    }

    /// Callable signatures owned by one canonical module, including empty modules.
    pub fn module_callable_signature_sources(
        &self,
        module: &CanonicalModulePath,
    ) -> Option<&[HirCallableSignatureSource]> {
        let range = self.callable_signature_ranges.get(module)?.clone();
        self.callable_signature_sources.get(range)
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

fn build_callable_signature_sources(
    package: &CallablePackageId,
    modules: &BTreeMap<CanonicalModulePath, HirModule>,
) -> Result<CallableSignaturePublication, HirProjectError> {
    let mut records = Vec::new();
    let mut ranges = BTreeMap::new();
    for (module, hir) in modules {
        let start = records.len();
        for function in hir.functions() {
            records.push(build_callable_signature_source(
                package, module, hir, function,
            )?);
        }
        for declaration in hir.declarations() {
            let HirTopLevelDecl::ExternCapability(capability) = declaration else {
                continue;
            };
            for function in capability.functions() {
                records.push(build_capability_signature_source(
                    package, module, hir, capability, function,
                )?);
            }
        }
        ranges.insert(module.clone(), start..records.len());
    }
    Ok((records, ranges))
}

fn build_capability_signature_source(
    package: &CallablePackageId,
    module: &CanonicalModulePath,
    hir: &HirModule,
    capability: &ExternCapabilityItem,
    function: &CapabilityFn,
) -> Result<HirCallableSignatureSource, HirProjectError> {
    let invalid = |reason: String| HirProjectError::InvalidCallableSource {
        module: module.clone(),
        name: format!("{}.{}", capability.id(), function.signature().name()),
        reason,
    };
    let capability_segment =
        ModuleSegment::new(capability.id()).map_err(|error| invalid(error.to_string()))?;
    let declaration = CallableDeclarationId::try_new_in_owner_path(
        package.clone(),
        module.clone(),
        crate::symbol::CallableDeclarationOwner::ExternCapability,
        [capability_segment.clone()],
        function.signature().name(),
    )
    .map_err(|error| invalid(error.to_string()))?;
    let path = SymbolPath::try_new(
        ModulePathRoot::ImplicitCrate,
        vec![capability_segment],
        function.signature().name(),
    )
    .map_err(|error| invalid(error.to_string()))?;
    let source = function.signature_source();
    let span = |range| {
        hir.source_span(range)
            .ok_or_else(|| invalid("source range is not bound to the lowered document".to_owned()))
    };
    let parameter_spans = source
        .parameters()
        .iter()
        .map(|parameter| {
            Ok(HirCallableParameterSource::new(
                parameter.group(),
                parameter.parameter(),
                span(parameter.whole())?,
                parameter.name().map(&span).transpose()?,
                parameter.ty().map(&span).transpose()?,
                parameter.default().map(&span).transpose()?,
            ))
        })
        .collect::<Result<Vec<_>, HirProjectError>>()?;
    let declared_effects = function
        .effects()
        .iter()
        .map(|effect| {
            effect
                .dotted_selector_label()
                .ok_or_else(|| {
                    invalid("declared effect is not a dotted capability path".to_owned())
                })
                .and_then(|label| {
                    HirEffectName::try_new(label).map_err(|error| invalid(error.to_string()))
                })
        })
        .collect::<Result<Vec<_>, HirProjectError>>()?;
    Ok(HirCallableSignatureSource::new(
        declaration,
        package.clone(),
        module.clone(),
        path,
        function.signature().clone(),
        None,
        span(*function.range())?,
        span(source.name())?,
        span(source.signature())?,
        source.result().map(&span).transpose()?,
        parameter_spans,
        HirCallableEffects::new(declared_effects),
    ))
}

fn build_callable_signature_source(
    package: &CallablePackageId,
    module: &CanonicalModulePath,
    hir: &HirModule,
    function: &HirFunction,
) -> Result<HirCallableSignatureSource, HirProjectError> {
    let invalid = |reason: String| HirProjectError::InvalidCallableSource {
        module: module.clone(),
        name: function.name().to_owned(),
        reason,
    };
    let declaration = CallableDeclarationId::for_function(package, function)
        .map_err(|error| invalid(error.to_string()))?;
    let path = SymbolPath::try_new(
        ModulePathRoot::ImplicitCrate,
        module.segments().to_vec(),
        function.name(),
    )
    .map_err(|error| invalid(error.to_string()))?;
    let source = function.signature_source();
    let span = |range| {
        hir.source_span(range)
            .ok_or_else(|| invalid("source range is not bound to the lowered document".to_owned()))
    };
    let parameter_spans = source
        .parameters()
        .iter()
        .map(|parameter| {
            Ok(HirCallableParameterSource::new(
                parameter.group(),
                parameter.parameter(),
                span(parameter.whole())?,
                parameter.name().map(&span).transpose()?,
                parameter.ty().map(&span).transpose()?,
                parameter.default().map(&span).transpose()?,
            ))
        })
        .collect::<Result<Vec<_>, HirProjectError>>()?;
    let mut declared_effects = Vec::new();
    for contract in function.contracts() {
        let ContractClause::Effects(effects) = contract else {
            continue;
        };
        for effect in effects {
            let label = effect.dotted_selector_label().ok_or_else(|| {
                invalid("declared effect is not a dotted capability path".to_owned())
            })?;
            declared_effects
                .push(HirEffectName::try_new(label).map_err(|error| invalid(error.to_string()))?);
        }
    }
    Ok(HirCallableSignatureSource::new(
        declaration,
        package.clone(),
        module.clone(),
        path,
        function.signature().clone(),
        function.documentation().cloned(),
        span(*function.range())?,
        span(source.name())?,
        span(source.signature())?,
        source.result().map(&span).transpose()?,
        parameter_spans,
        HirCallableEffects::new(declared_effects),
    ))
}

impl HirModule {
    fn assign_declaration_module(&mut self, path: &CanonicalModulePath) {
        self.module_path.clone_from(path);
        self.bind_project_module(path);
        self.declaration_modules = vec![path.clone(); self.declarations.len()];
        for flow in &mut self.flows {
            flow.module_path = Some(path.clone());
            assign_flow_item_modules(&mut flow.body, path);
        }
        for function in &mut self.functions {
            function.module_path = Some(path.clone());
        }
        for declaration in &mut self.declarations {
            match declaration {
                crate::model::HirTopLevelDecl::Source(source) => {
                    source.bind_project_module(path);
                }
                crate::model::HirTopLevelDecl::Entry(entry) => {
                    entry.bind_project_module(path);
                }
                _ => {}
            }
        }
        self.view_parts
            .iter_mut()
            .for_each(|owner| owner.assign_module(path));
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
        self.declarations.append(&mut module.declarations);
        self.declaration_modules
            .append(&mut module.declaration_modules);
        self.style_patches.append(&mut module.style_patches);
        self.view_parts.append(&mut module.view_parts);
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
    use super::{HirProject, HirProjectModule, HirProjectModuleError};
    use crate::lower::lower_document_to_hir;
    use crate::model::HirFlowItem;
    use crate::style::HirStylePatch;
    use arcweft_lang_syntax::{
        ast::{
            common::TextRange,
            module_path::{CanonicalModulePath, ModuleSegment},
        },
        parser::{ParseOptions, parse_document_with_source},
    };
    use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};
    use std::{collections::BTreeSet, sync::Arc};

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
                HirProjectModule::try_new(
                    omega_path.clone(),
                    omega_document.identity().clone(),
                    omega,
                )
                .expect("omega module binding"),
                HirProjectModule::try_new(
                    CanonicalModulePath::crate_root(),
                    root_document.identity().clone(),
                    root,
                )
                .expect("root module binding"),
                HirProjectModule::try_new(
                    alpha_path.clone(),
                    alpha_document.identity().clone(),
                    alpha,
                )
                .expect("alpha module binding"),
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
        let root_source = "#![generated(tool)]\nflow @root root {}\nstruct RootState {}";
        let (root_document, root) = lower_bound("root-linked", root_source);
        let child_source =
            "flow @child child {}\npub fn helper() -> i32 { 1 }\nenum ChildEvent { Ready }";
        let (child_document, child) = lower_bound("child-linked", child_source);
        let child_path =
            CanonicalModulePath::crate_root().join(ModuleSegment::new("child").unwrap());
        let project = HirProject::new(
            "game",
            [
                HirProjectModule::try_new(
                    CanonicalModulePath::crate_root(),
                    root_document.identity().clone(),
                    root,
                )
                .expect("root module binding"),
                HirProjectModule::try_new(
                    child_path.clone(),
                    child_document.identity().clone(),
                    child,
                )
                .expect("child module binding"),
            ],
        )
        .unwrap();
        let linked = project.linked_module();
        assert_eq!(linked.attributes().len(), 1);
        assert_eq!(linked.flows().len(), 2);
        assert_eq!(
            linked
                .declarations_with_modules()
                .map(|(module, _)| module.clone())
                .collect::<Vec<_>>(),
            [CanonicalModulePath::crate_root(), child_path.clone()]
        );
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
        let (document, hir) = lower_bound("nested-flow", source);
        let child_path =
            CanonicalModulePath::crate_root().join(ModuleSegment::new("child").unwrap());
        let module =
            HirProjectModule::try_new(child_path.clone(), document.identity().clone(), hir)
                .expect("nested flow module binding");

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

    fn source_document(label: &str, source: &str) -> Arc<SourceDocument> {
        Arc::new(
            SourceDocument::try_new(
                SourceDocumentId::try_new(format!("memory:///{label}.arcw")).unwrap(),
                SourceName::Generated,
                source,
            )
            .expect("source document"),
        )
    }

    fn lower_bound(label: &str, source: &str) -> (Arc<SourceDocument>, crate::model::HirModule) {
        let document = source_document(label, source);
        let parsed = parse_document_with_source(Arc::clone(&document), ParseOptions::default());
        let hir =
            lower_document_to_hir(parsed.document(), parsed.typed_tree()).expect("source lowers");
        (document, hir)
    }

    #[test]
    fn document_bound_hir_exposes_exact_source_and_module_identity() {
        let (document, hir) = lower_bound("identity", "");

        assert_eq!(hir.source_identity(), Some(document.identity()));
        assert_eq!(hir.module_path(), &CanonicalModulePath::crate_root());

        let child_path =
            CanonicalModulePath::crate_root().join(ModuleSegment::new("child").unwrap());
        let child = HirProjectModule::try_new(child_path.clone(), document.identity().clone(), hir)
            .expect("child module binding");
        assert_eq!(child.hir().module_path(), &child_path);
        assert_eq!(child.hir().source_identity(), Some(document.identity()));
        assert_eq!(child.hir().source_document(), Some(document.as_ref()));
    }

    #[test]
    fn project_module_rejects_another_source_revision() {
        let (expected_document, hir) = lower_bound("expected-source", "");
        let actual_document = source_document("actual-source", "flow different {}");
        let module = CanonicalModulePath::crate_root();

        assert_eq!(
            HirProjectModule::try_new(module.clone(), actual_document.identity().clone(), hir,),
            Err(HirProjectModuleError::SourceIdentityMismatch {
                module,
                expected: actual_document.identity().clone(),
                actual: expected_document.identity().clone(),
            })
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

    #[test]
    fn callable_signature_publication_preserves_exact_typed_source_rows() {
        let root_source = r#"/// Summarizes a curried request.
pub fn summarize(first: i32 = 7)(rest: ...String) -> String
effects { agent.observe }
{
    "done"
}
"#;
        let (root_document, root) = lower_bound("callable-root", root_source);
        let empty_source = "";
        let (empty_document, empty) = lower_bound("callable-empty", empty_source);
        let empty_path =
            CanonicalModulePath::crate_root().join(ModuleSegment::new("empty").unwrap());
        let project = HirProject::new(
            "game",
            [
                HirProjectModule::try_new(
                    empty_path.clone(),
                    empty_document.identity().clone(),
                    empty,
                )
                .unwrap(),
                HirProjectModule::try_new(
                    CanonicalModulePath::crate_root(),
                    root_document.identity().clone(),
                    root,
                )
                .unwrap(),
            ],
        )
        .unwrap();

        assert!(
            project
                .module_callable_signature_sources(&empty_path)
                .is_some_and(<[crate::callable_source::HirCallableSignatureSource]>::is_empty)
        );
        let records = project.callable_signature_sources().collect::<Vec<_>>();
        assert_eq!(records.len(), 1);
        let record = records[0];
        assert_eq!(record.declaration().name(), "summarize");
        assert_eq!(
            record.documentation().unwrap().text(),
            "Summarizes a curried request."
        );
        assert_eq!(source_slice(root_source, record.name_span()), "summarize");
        assert_eq!(
            source_slice(root_source, record.result_span().unwrap()),
            "String"
        );
        assert_eq!(record.signature().param_groups().len(), 2);
        assert_eq!(record.parameter_spans().len(), 2);
        assert_eq!(
            source_slice(root_source, record.parameter_spans()[0].whole()),
            "first: i32 = 7"
        );
        assert_eq!(
            source_slice(root_source, record.parameter_spans()[0].name().unwrap()),
            "first"
        );
        assert_eq!(
            source_slice(root_source, record.parameter_spans()[0].ty().unwrap()),
            "i32"
        );
        assert_eq!(
            source_slice(root_source, record.parameter_spans()[0].default().unwrap()),
            "7"
        );
        assert_eq!(
            source_slice(root_source, record.parameter_spans()[1].whole()),
            "rest: ...String"
        );
        assert_eq!(record.parameter_spans()[1].group(), 1);
        assert_eq!(record.parameter_spans()[1].parameter(), 0);
        assert_eq!(record.effects().declared()[0].as_str(), "agent.observe");
    }

    #[test]
    fn extern_capability_functions_publish_typed_owned_callable_sources() {
        let source = r"extern capability fs {
    fn read_text(path: VirtualPath) -> String effects { fs.read }
}
";
        let (document, hir) = lower_bound("capability-callable", source);
        let project = HirProject::new(
            "game",
            [HirProjectModule::try_new(
                CanonicalModulePath::crate_root(),
                document.identity().clone(),
                hir,
            )
            .expect("root module binding")],
        )
        .expect("capability project");

        let records = project.callable_signature_sources().collect::<Vec<_>>();
        assert_eq!(records.len(), 1);
        let record = records[0];
        assert_eq!(
            record.declaration().owner(),
            crate::symbol::CallableDeclarationOwner::ExternCapability
        );
        assert_eq!(record.declaration().qualified_name(), "fs.read_text");
        assert_eq!(record.declaration().owner_path().len(), 1);
        assert_eq!(record.declaration().owner_path()[0].as_str(), "fs");
        assert_eq!(record.path().to_string(), "fs.read_text");
        assert_eq!(source_slice(source, record.name_span()), "read_text");
        assert_eq!(
            source_slice(source, record.parameter_spans()[0].whole()),
            "path: VirtualPath"
        );
        assert_eq!(
            source_slice(source, record.result_span().unwrap()),
            "String"
        );
        assert_eq!(record.effects().declared()[0].as_str(), "fs.read");
    }

    fn source_slice<'a>(source: &'a str, span: &arcweft_source::SourceSpan) -> &'a str {
        &source[span.range().start()..span.range().end()]
    }
}
