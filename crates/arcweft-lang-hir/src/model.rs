use arcweft_lang_syntax::{
    ast::module_path::CanonicalModulePath,
    ast::{
        choice::{ChoiceAction, ChoiceItem, ChoicePlan},
        common::{DocBlock, TextRange, UseItem, Visibility},
        dialogue::{DialogueContent, LineArg, SpeakerLineSurface},
        flow::{
            AuthoredExpr, AwaitBranchKind, ContractClause, FlowSignatureSource, SelectBranchHead,
            Stmt,
        },
        ids::{EntityRef, EntityRefSyntax},
        items::{
            Attribute, EntityDeclItem, EntityDeclKind, EnumItem, ExternCapabilityItem,
            ExternModItem, FunctionSignatureSource, ImplItem, StructItem, TraitItem, TypeAliasItem,
        },
        line_plan::LinePlan,
        pattern::Pattern,
        proof::{BenchItem, ProofItem, TestItem},
        source::SourceItem,
    },
    expr::Expr,
    types::FnSignature,
};
use arcweft_source::{
    Diagnostic, DiagnosticLabel, DiagnosticSeverity, SourceDocument, SourceDocumentIdentity,
    SourceRange, SourceSpan,
};
use std::collections::BTreeMap;
use thiserror::Error;

use crate::entry::HirEntryDecl;
use crate::style::{HirStyleDecl, HirStylePatch};
use crate::view_part::HirViewPartOwner;

/// HIR-facing module produced from parsed surface syntax.
///
/// This is intentionally still close to syntax. Its role is to prove that the
/// parser exposes enough typed structure for later semantic analysis without
/// re-parsing raw strings.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirModule {
    pub(crate) module_path: CanonicalModulePath,
    pub(crate) attributes: Vec<Attribute>,
    pub(crate) uses: Vec<UseItem>,
    pub(crate) source_len: Option<usize>,
    pub(crate) top_level_ranges: Vec<TextRange>,
    pub(crate) flows: Vec<HirFlow>,
    pub(crate) functions: Vec<HirFunction>,
    pub(crate) declarations: Vec<HirTopLevelDecl>,
    pub(crate) declaration_modules: Vec<CanonicalModulePath>,
    pub(crate) style_patches: Vec<HirStylePatch>,
    pub(crate) view_parts: Vec<HirViewPartOwner>,
    pub(crate) source_map: Option<HirSourceMap>,
}

/// Revision-bound spans created by the source document during lowering.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct HirSourceMap {
    document: SourceDocument,
    project_documents: BTreeMap<CanonicalModulePath, SourceDocument>,
}

/// HIR-facing flow.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirFlow {
    pub(crate) attributes: Vec<Attribute>,
    pub(crate) module_path: Option<CanonicalModulePath>,
    pub(crate) id: Option<EntityRef>,
    pub(crate) name: Option<String>,
    pub(crate) signature: Option<FnSignature>,
    pub(crate) signature_source: FlowSignatureSource,
    pub(crate) contracts: Vec<ContractClause>,
    pub(crate) body: Vec<HirFlowItem>,
    pub(crate) range: TextRange,
}

/// HIR-facing function body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirFunction {
    pub(crate) attributes: Vec<Attribute>,
    pub(crate) documentation: Option<DocBlock>,
    pub(crate) module_path: Option<CanonicalModulePath>,
    pub(crate) visibility: Option<Visibility>,
    pub(crate) signature: FnSignature,
    pub(crate) signature_source: FunctionSignatureSource,
    pub(crate) contracts: Vec<ContractClause>,
    pub(crate) statements: Vec<Stmt>,
    pub(crate) value: Option<AuthoredExpr>,
    pub(crate) range: TextRange,
}

/// HIR-facing top-level declaration preserved for later semantic passes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirTopLevelDecl {
    Trait(TraitItem),
    Impl(ImplItem),
    Enum(EnumItem),
    EntityDecl(EntityDeclItem),
    Entry(HirEntryDecl),
    ExternCapability(ExternCapabilityItem),
    ExternMod(ExternModItem),
    Struct(StructItem),
    TypeAlias(TypeAliasItem),
    Proof(ProofItem),
    Test(TestItem),
    Bench(BenchItem),
    Source(HirSource),
    Style(HirStyleDecl),
}

/// HIR-owned source declaration with its canonical project-module origin.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirSource {
    module_path: Option<CanonicalModulePath>,
    item: SourceItem,
}

/// HIR-facing flow item.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirFlowItem {
    Stmt(Stmt),
    Dialogue(Box<HirDialogue>),
    Choice(HirChoice),
    LetChoice {
        pattern: Pattern,
        choice: HirChoice,
    },
    LetScope {
        pattern: Pattern,
        scope: HirScopeExpr,
    },
    LetLoop {
        pattern: Pattern,
        block: HirLoop,
    },
    LetAwait {
        pattern: Pattern,
        await_with: HirAwait,
    },
    Thread(HirThread),
    If(HirIf),
    IfLet(HirIfLet),
    Match(HirMatch),
    Loop(HirLoop),
    While(HirWhile),
    WhileLet(HirWhileLet),
    For(HirFor),
    Select(HirSelect),
    SourceLocale(HirSourceLocale),
    Scope(HirScope),
    Include(EntityRef),
    Await(HirAwait),
}

/// Dialogue call normalized enough for type checking to resolve speaker symbols.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirDialogue {
    pub(crate) source_module: Option<CanonicalModulePath>,
    pub(crate) speaker_surface: Option<SpeakerLineSurface>,
    pub(crate) callee: String,
    pub(crate) id: Option<EntityRef>,
    pub(crate) text_key: Option<EntityRef>,
    pub(crate) voice: Option<Expr>,
    pub(crate) look: Option<Expr>,
    pub(crate) stage: Option<Expr>,
    pub(crate) portrait: Option<Expr>,
    pub(crate) focus: Option<Expr>,
    pub(crate) cleanup: Option<Expr>,
    pub(crate) view: Option<EntityRef>,
    pub(crate) source_locale: Option<String>,
    pub(crate) hooks: Vec<Expr>,
    pub(crate) style: Option<Expr>,
    pub(crate) style_raw: Option<String>,
    pub(crate) style_range: Option<TextRange>,
    pub(crate) rich_text: Option<Expr>,
    pub(crate) rich_text_raw: Option<String>,
    pub(crate) rich_text_range: Option<TextRange>,
    pub(crate) args: Vec<LineArg>,
    pub(crate) content: DialogueContent,
    pub(crate) plan: Option<LinePlan>,
}

/// HIR-facing choice block.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirChoice {
    pub(crate) id: Option<EntityRef>,
    pub(crate) items: Vec<ChoiceItem>,
    pub(crate) options: Vec<HirChoiceOption>,
    pub(crate) plan: Option<ChoicePlan>,
}

/// HIR-facing choice option.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirChoiceOption {
    pub(crate) id: Option<EntityRef>,
    pub(crate) label: String,
    pub(crate) condition: Option<Expr>,
    pub(crate) action: ChoiceAction,
    pub(crate) value: Option<Expr>,
    pub(crate) label_text_key: Option<EntityRef>,
}

/// HIR-facing source-locale block.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirSourceLocale {
    pub(crate) locale: String,
    pub(crate) body: Vec<HirFlowItem>,
}

/// HIR-facing lexical scope. Named scopes also affect generated relative IDs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirScope {
    pub(crate) name: Option<String>,
    pub(crate) body: Vec<HirFlowItem>,
}

/// HIR-facing scope expression. Named scopes also affect generated relative IDs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirScopeExpr {
    pub(crate) name: Option<String>,
    pub(crate) statements: Vec<Stmt>,
    pub(crate) value: Option<Expr>,
}

/// HIR-facing if block.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirIf {
    pub(crate) condition: AuthoredExpr,
    pub(crate) body: Vec<HirFlowItem>,
    pub(crate) else_body: Vec<HirFlowItem>,
}

/// HIR-facing if-let block.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirIfLet {
    pub(crate) pattern: Pattern,
    pub(crate) expr: AuthoredExpr,
    pub(crate) guard: Option<AuthoredExpr>,
    pub(crate) body: Vec<HirFlowItem>,
    pub(crate) else_body: Vec<HirFlowItem>,
}

/// HIR-facing match block.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirMatch {
    pub(crate) expr: AuthoredExpr,
    pub(crate) arms: Vec<HirMatchArm>,
}

/// HIR-facing match arm.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirMatchArm {
    pub(crate) pattern: Pattern,
    pub(crate) guard: Option<AuthoredExpr>,
    pub(crate) body: Vec<HirFlowItem>,
}

/// HIR-facing value-capable `loop` block.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirLoop {
    pub(crate) label: Option<String>,
    pub(crate) body: Vec<HirFlowItem>,
}

/// HIR-facing sequence loop.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirFor {
    pub(crate) pattern: Pattern,
    pub(crate) source: AuthoredExpr,
    pub(crate) body: Vec<HirFlowItem>,
}

/// HIR-facing `while` statement loop.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirWhile {
    pub(crate) condition: AuthoredExpr,
    pub(crate) body: Vec<HirFlowItem>,
}

/// HIR-facing `while let` statement loop.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirWhileLet {
    pub(crate) pattern: Pattern,
    pub(crate) expr: AuthoredExpr,
    pub(crate) guard: Option<AuthoredExpr>,
    pub(crate) body: Vec<HirFlowItem>,
}

/// HIR-facing source select block.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirSelect {
    pub(crate) branches: Vec<HirSelectBranch>,
}

/// HIR-facing select branch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirSelectBranch {
    pub(crate) head: SelectBranchHead,
    pub(crate) body: Vec<HirFlowItem>,
}

/// HIR-facing await-with block.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirAwait {
    pub(crate) expr: AuthoredExpr,
    pub(crate) applies_try: bool,
    pub(crate) branches: Vec<HirAwaitBranch>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirThread {
    pub(crate) name: Option<String>,
    pub(crate) detached: bool,
    pub(crate) body: Vec<HirFlowItem>,
}

/// HIR-facing wait-view branch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirAwaitBranch {
    pub(crate) kind: AwaitBranchKind,
    pub(crate) pattern: Pattern,
    pub(crate) body: Vec<HirFlowItem>,
}

/// Lowering failure for syntax that is still too raw for HIR.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("{message}")]
pub struct HirLowerError {
    pub(crate) message: String,
    pub(crate) range: Option<TextRange>,
}

impl HirModule {
    pub(crate) fn bind_source_document(
        &mut self,
        document: &SourceDocument,
    ) -> Result<(), HirLowerError> {
        if self.source_len != Some(document.text().len()) {
            return Err(HirLowerError::new(
                "HIR source length does not match the source document",
                None,
            ));
        }
        self.source_map = Some(HirSourceMap {
            document: document.clone(),
            project_documents: BTreeMap::new(),
        });
        Ok(())
    }

    /// Exact source-document revision used by document-bound lowering.
    pub fn source_identity(&self) -> Option<&SourceDocumentIdentity> {
        self.source_map
            .as_ref()
            .map(|source| source.document.identity())
    }

    /// Exact source document retained by document-bound lowering.
    pub fn source_document(&self) -> Option<&SourceDocument> {
        self.source_map.as_ref().map(|source| &source.document)
    }

    /// Exact source document retained for one canonical module in linked HIR.
    ///
    /// A standalone module exposes its own document through its canonical
    /// module path. Linked projects expose every merged project document
    /// without requiring a semantic query to parse or lower source again.
    pub fn project_source_document(&self, module: &CanonicalModulePath) -> Option<&SourceDocument> {
        let source_map = self.source_map.as_ref()?;
        source_map
            .project_documents
            .get(module)
            .or_else(|| (module == &self.module_path).then_some(&source_map.document))
    }

    /// Canonical module represented by this HIR module.
    pub const fn module_path(&self) -> &CanonicalModulePath {
        &self.module_path
    }

    pub(crate) fn bind_project_module(&mut self, module: &CanonicalModulePath) {
        if let Some(source_map) = &mut self.source_map {
            source_map
                .project_documents
                .insert(module.clone(), source_map.document.clone());
        }
    }

    pub(crate) fn merge_project_sources(&mut self, appended: &mut Self) {
        if let (Some(linked), Some(appended)) = (&mut self.source_map, appended.source_map.take()) {
            linked.project_documents.extend(appended.project_documents);
        }
    }

    /// Binds one authored HIR range to the exact document revision lowered into this module.
    pub fn source_span(&self, range: TextRange) -> Option<SourceSpan> {
        self.source_map
            .as_ref()?
            .document
            .span(SourceRange::new(range.start(), range.end()))
            .ok()
    }

    /// Binds one authored range through the canonical module that owns it in a linked project.
    pub fn project_source_span(
        &self,
        module: &CanonicalModulePath,
        range: TextRange,
    ) -> Option<SourceSpan> {
        self.source_map
            .as_ref()?
            .project_documents
            .get(module)?
            .span(SourceRange::new(range.start(), range.end()))
            .ok()
    }

    pub fn attributes(&self) -> &[Attribute] {
        &self.attributes
    }

    /// Source-level imports retained for module-aware symbol linking.
    pub fn uses(&self) -> &[UseItem] {
        &self.uses
    }

    pub const fn source_len(&self) -> Option<usize> {
        self.source_len
    }

    pub fn top_level_ranges(&self) -> &[TextRange] {
        &self.top_level_ranges
    }

    /// Returns an empty insertion range after the last typed top-level item.
    ///
    /// If HIR was not lowered from one concrete source document, or if there is
    /// no typed top-level boundary, callers must keep repair actions as host
    /// commands instead of emitting a speculative source edit.
    pub fn safe_top_level_insertion_range(&self) -> Option<TextRange> {
        let source_len = self.source_len?;
        let end = self
            .top_level_ranges
            .iter()
            .map(TextRange::end)
            .max()
            .filter(|end| *end <= source_len)?;
        Some(TextRange::new(end, end))
    }

    pub fn has_attribute(&self, name: &str) -> bool {
        self.attributes
            .iter()
            .any(|attribute| attribute.name() == name)
    }

    pub fn flows(&self) -> &[HirFlow] {
        &self.flows
    }

    pub fn functions(&self) -> &[HirFunction] {
        &self.functions
    }

    pub fn declarations(&self) -> &[HirTopLevelDecl] {
        &self.declarations
    }

    /// Top-level declarations paired with the canonical module that authored
    /// each declaration.
    ///
    /// A linked HIR module combines declarations whose local byte ranges may
    /// overlap. Keeping the typed module owner alongside each declaration lets
    /// semantic consumers bind those ranges through the correct source
    /// document without inspecting source text or guessing from a range.
    pub fn declarations_with_modules(
        &self,
    ) -> impl ExactSizeIterator<Item = (&CanonicalModulePath, &HirTopLevelDecl)> {
        debug_assert_eq!(self.declaration_modules.len(), self.declarations.len());
        self.declaration_modules.iter().zip(&self.declarations)
    }

    /// Typed authored View declarations in deterministic linked-HIR order.
    ///
    /// Consumers use this inventory instead of rediscovering View owners from
    /// source text or declaration spellings.
    pub fn view_declarations(&self) -> impl Iterator<Item = &EntityDeclItem> {
        self.declarations
            .iter()
            .filter_map(HirTopLevelDecl::as_view)
    }

    /// Inline style patches in deterministic source order.
    pub fn style_patches(&self) -> &[HirStylePatch] {
        &self.style_patches
    }

    /// Owner-qualified private/public View-part declarations.
    pub fn view_parts(&self) -> &[HirViewPartOwner] {
        &self.view_parts
    }
}

impl HirTopLevelDecl {
    /// Returns the retained typed View owner represented by this declaration.
    pub const fn as_view(&self) -> Option<&EntityDeclItem> {
        match self {
            Self::EntityDecl(item) if matches!(item.kind(), EntityDeclKind::View) => Some(item),
            _ => None,
        }
    }
}

impl HirSource {
    pub(crate) fn new(item: SourceItem, module_path: Option<CanonicalModulePath>) -> Self {
        Self { module_path, item }
    }

    /// Canonical project module that owns this declaration after project binding.
    pub const fn module_path(&self) -> Option<&CanonicalModulePath> {
        self.module_path.as_ref()
    }

    /// Parsed source declaration retained at the syntax-to-HIR boundary.
    pub const fn item(&self) -> &SourceItem {
        &self.item
    }

    pub(crate) fn bind_project_module(&mut self, module: &CanonicalModulePath) {
        self.module_path = Some(module.clone());
    }
}

impl HirFlow {
    pub fn attributes(&self) -> &[Attribute] {
        &self.attributes
    }

    pub fn has_attribute(&self, name: &str) -> bool {
        self.attributes
            .iter()
            .any(|attribute| attribute.name() == name)
    }

    pub const fn module_path(&self) -> Option<&CanonicalModulePath> {
        self.module_path.as_ref()
    }

    pub const fn id(&self) -> Option<&EntityRef> {
        self.id.as_ref()
    }

    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    pub const fn signature(&self) -> Option<&FnSignature> {
        self.signature.as_ref()
    }

    /// Exact source ranges retained from the authored flow signature.
    pub const fn signature_source(&self) -> FlowSignatureSource {
        self.signature_source
    }

    pub fn body(&self) -> &[HirFlowItem] {
        &self.body
    }

    pub const fn range(&self) -> &TextRange {
        &self.range
    }

    pub fn contracts(&self) -> &[ContractClause] {
        &self.contracts
    }
}

impl HirFunction {
    pub fn attributes(&self) -> &[Attribute] {
        &self.attributes
    }

    pub const fn documentation(&self) -> Option<&DocBlock> {
        self.documentation.as_ref()
    }

    pub fn has_attribute(&self, name: &str) -> bool {
        self.attributes
            .iter()
            .any(|attribute| attribute.name() == name)
    }

    pub const fn visibility(&self) -> Option<Visibility> {
        self.visibility
    }

    pub fn name(&self) -> &str {
        self.signature.name()
    }

    /// Original qualified declaration name used as the stable function identity.
    pub fn qualified_name(&self) -> String {
        self.module_path.as_ref().map_or_else(
            || self.name().to_owned(),
            |module| crate::symbol::qualified_name(module, self.name()),
        )
    }

    pub const fn module_path(&self) -> Option<&CanonicalModulePath> {
        self.module_path.as_ref()
    }

    pub const fn signature(&self) -> &FnSignature {
        &self.signature
    }

    pub const fn signature_source(&self) -> &FunctionSignatureSource {
        &self.signature_source
    }

    pub fn contracts(&self) -> &[ContractClause] {
        &self.contracts
    }

    pub fn statements(&self) -> &[Stmt] {
        &self.statements
    }

    pub const fn value(&self) -> Option<&AuthoredExpr> {
        self.value.as_ref()
    }

    pub const fn range(&self) -> &TextRange {
        &self.range
    }
}

impl HirDialogue {
    pub fn expression_call(
        callee: String,
        content: DialogueContent,
        plan: Option<LinePlan>,
    ) -> Self {
        Self {
            source_module: None,
            speaker_surface: None,
            callee,
            id: None,
            text_key: None,
            voice: None,
            look: None,
            stage: None,
            portrait: None,
            focus: None,
            cleanup: None,
            view: None,
            source_locale: None,
            hooks: Vec::new(),
            style: None,
            style_raw: None,
            style_range: None,
            rich_text: None,
            rich_text_raw: None,
            rich_text_range: None,
            args: Vec::new(),
            content,
            plan,
        }
    }

    pub fn callee(&self) -> &str {
        &self.callee
    }

    /// Canonical source module assigned when this dialogue enters a HIR project.
    pub const fn source_module(&self) -> Option<&CanonicalModulePath> {
        self.source_module.as_ref()
    }

    /// Parser-owned authored speaker-line ranges, when this came from speaker sugar.
    pub const fn speaker_surface(&self) -> Option<&SpeakerLineSurface> {
        self.speaker_surface.as_ref()
    }

    pub const fn id(&self) -> Option<&EntityRef> {
        self.id.as_ref()
    }

    pub const fn text_key(&self) -> Option<&EntityRef> {
        self.text_key.as_ref()
    }

    pub const fn voice(&self) -> Option<&Expr> {
        self.voice.as_ref()
    }

    pub const fn look(&self) -> Option<&Expr> {
        self.look.as_ref()
    }

    pub const fn stage(&self) -> Option<&Expr> {
        self.stage.as_ref()
    }

    pub const fn portrait(&self) -> Option<&Expr> {
        self.portrait.as_ref()
    }

    pub const fn focus(&self) -> Option<&Expr> {
        self.focus.as_ref()
    }

    pub const fn cleanup(&self) -> Option<&Expr> {
        self.cleanup.as_ref()
    }

    pub const fn view(&self) -> Option<&EntityRef> {
        self.view.as_ref()
    }

    pub fn source_locale(&self) -> Option<&str> {
        self.source_locale.as_deref()
    }

    pub fn hooks(&self) -> &[Expr] {
        &self.hooks
    }

    pub const fn style(&self) -> Option<&Expr> {
        self.style.as_ref()
    }

    pub fn style_raw(&self) -> Option<&str> {
        self.style_raw.as_deref()
    }

    pub const fn style_range(&self) -> Option<&TextRange> {
        self.style_range.as_ref()
    }

    pub const fn rich_text(&self) -> Option<&Expr> {
        self.rich_text.as_ref()
    }

    pub fn rich_text_raw(&self) -> Option<&str> {
        self.rich_text_raw.as_deref()
    }

    pub const fn rich_text_range(&self) -> Option<&TextRange> {
        self.rich_text_range.as_ref()
    }

    pub fn args(&self) -> &[LineArg] {
        &self.args
    }

    /// Ordinary expression-valued line setup options checked before runtime
    /// cleanup.
    ///
    /// This includes reserved singleton options with an existing semantic
    /// owner, repeated hooks, and custom named options. Legacy presentation
    /// `style` and `rich_text` expressions are excluded until the typed
    /// `RichText` authority replaces their provisional raw runtime-plan
    /// interpretation. Focus and cleanup remain separate because semantic
    /// checking establishes the focus lifetime before cleanup.
    pub fn checked_line_setup_expressions(&self) -> impl Iterator<Item = &Expr> {
        [
            self.voice.as_ref(),
            self.look.as_ref(),
            self.stage.as_ref(),
            self.portrait.as_ref(),
        ]
        .into_iter()
        .flatten()
        .chain(self.hooks.iter())
        .chain(self.args.iter().map(LineArg::value))
    }

    pub const fn content(&self) -> &DialogueContent {
        &self.content
    }

    pub const fn plan(&self) -> Option<&LinePlan> {
        self.plan.as_ref()
    }
}

impl HirChoice {
    pub const fn id(&self) -> Option<&EntityRef> {
        self.id.as_ref()
    }

    pub fn options(&self) -> &[HirChoiceOption] {
        &self.options
    }

    pub fn items(&self) -> &[ChoiceItem] {
        &self.items
    }

    pub const fn plan(&self) -> Option<&ChoicePlan> {
        self.plan.as_ref()
    }
}

impl HirScope {
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    pub fn body(&self) -> &[HirFlowItem] {
        &self.body
    }
}

impl HirScopeExpr {
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    pub fn statements(&self) -> &[Stmt] {
        &self.statements
    }

    pub const fn value(&self) -> Option<&Expr> {
        self.value.as_ref()
    }
}

impl HirLoop {
    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }

    pub fn body(&self) -> &[HirFlowItem] {
        &self.body
    }
}

impl HirChoiceOption {
    pub const fn id(&self) -> Option<&EntityRef> {
        self.id.as_ref()
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub const fn condition(&self) -> Option<&Expr> {
        self.condition.as_ref()
    }

    pub const fn target(&self) -> Option<&EntityRef> {
        match &self.action {
            ChoiceAction::Goto(EntityRefSyntax::Absolute(target)) => Some(target),
            _ => None,
        }
    }

    pub const fn action(&self) -> &ChoiceAction {
        &self.action
    }

    pub const fn value(&self) -> Option<&Expr> {
        self.value.as_ref()
    }

    pub const fn label_text_key(&self) -> Option<&EntityRef> {
        self.label_text_key.as_ref()
    }
}

impl HirSourceLocale {
    pub fn locale(&self) -> &str {
        &self.locale
    }

    pub fn body(&self) -> &[HirFlowItem] {
        &self.body
    }
}

impl HirIf {
    pub const fn condition(&self) -> &Expr {
        self.condition.expr()
    }

    pub const fn condition_authored(&self) -> &AuthoredExpr {
        &self.condition
    }

    pub fn body(&self) -> &[HirFlowItem] {
        &self.body
    }

    pub fn else_body(&self) -> &[HirFlowItem] {
        &self.else_body
    }
}

impl HirIfLet {
    pub const fn pattern(&self) -> &Pattern {
        &self.pattern
    }

    pub const fn expr(&self) -> &Expr {
        self.expr.expr()
    }

    pub const fn expr_authored(&self) -> &AuthoredExpr {
        &self.expr
    }

    pub const fn guard(&self) -> Option<&Expr> {
        match self.guard.as_ref() {
            Some(guard) => Some(guard.expr()),
            None => None,
        }
    }

    pub fn guard_authored(&self) -> Option<&AuthoredExpr> {
        self.guard.as_ref()
    }

    pub fn body(&self) -> &[HirFlowItem] {
        &self.body
    }

    pub fn else_body(&self) -> &[HirFlowItem] {
        &self.else_body
    }
}

impl HirMatch {
    pub const fn expr(&self) -> &Expr {
        self.expr.expr()
    }

    pub const fn expr_authored(&self) -> &AuthoredExpr {
        &self.expr
    }

    pub fn arms(&self) -> &[HirMatchArm] {
        &self.arms
    }
}

impl HirMatchArm {
    pub const fn pattern(&self) -> &Pattern {
        &self.pattern
    }

    pub const fn guard(&self) -> Option<&Expr> {
        match self.guard.as_ref() {
            Some(guard) => Some(guard.expr()),
            None => None,
        }
    }

    pub fn guard_authored(&self) -> Option<&AuthoredExpr> {
        self.guard.as_ref()
    }

    pub fn body(&self) -> &[HirFlowItem] {
        &self.body
    }
}

impl HirFor {
    pub const fn pattern(&self) -> &Pattern {
        &self.pattern
    }

    pub const fn source(&self) -> &Expr {
        self.source.expr()
    }

    pub const fn source_authored(&self) -> &AuthoredExpr {
        &self.source
    }

    pub fn body(&self) -> &[HirFlowItem] {
        &self.body
    }
}

impl HirWhile {
    pub const fn condition(&self) -> &Expr {
        self.condition.expr()
    }

    pub const fn condition_authored(&self) -> &AuthoredExpr {
        &self.condition
    }

    pub fn body(&self) -> &[HirFlowItem] {
        &self.body
    }
}

impl HirWhileLet {
    pub const fn pattern(&self) -> &Pattern {
        &self.pattern
    }

    pub const fn expr(&self) -> &Expr {
        self.expr.expr()
    }

    pub const fn expr_authored(&self) -> &AuthoredExpr {
        &self.expr
    }

    pub const fn guard(&self) -> Option<&Expr> {
        match self.guard.as_ref() {
            Some(guard) => Some(guard.expr()),
            None => None,
        }
    }

    pub fn guard_authored(&self) -> Option<&AuthoredExpr> {
        self.guard.as_ref()
    }

    pub fn body(&self) -> &[HirFlowItem] {
        &self.body
    }
}

impl HirSelect {
    pub fn branches(&self) -> &[HirSelectBranch] {
        &self.branches
    }
}

impl HirSelectBranch {
    pub const fn head(&self) -> &SelectBranchHead {
        &self.head
    }

    pub fn body(&self) -> &[HirFlowItem] {
        &self.body
    }
}

impl HirAwait {
    pub const fn expr(&self) -> &Expr {
        self.expr.expr()
    }

    pub const fn expr_authored(&self) -> &AuthoredExpr {
        &self.expr
    }

    pub const fn applies_try(&self) -> bool {
        self.applies_try
    }

    pub fn branches(&self) -> &[HirAwaitBranch] {
        &self.branches
    }
}

impl HirThread {
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    pub const fn is_detached(&self) -> bool {
        self.detached
    }

    pub fn body(&self) -> &[HirFlowItem] {
        &self.body
    }
}

impl HirAwaitBranch {
    pub const fn kind(&self) -> AwaitBranchKind {
        self.kind
    }

    pub const fn pattern(&self) -> &Pattern {
        &self.pattern
    }

    pub fn body(&self) -> &[HirFlowItem] {
        &self.body
    }
}

impl HirLowerError {
    pub(crate) fn new(message: impl Into<String>, range: Option<TextRange>) -> Self {
        Self {
            message: message.into(),
            range,
        }
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    /// Builds the shared diagnostic representation for compiler, CLI, LSP, and Agent surfaces.
    ///
    /// # Panics
    ///
    /// Panics when the supplied document is not the revision-bound document from which this
    /// lowering error was produced.
    pub fn diagnostic(&self, document: &SourceDocument) -> Diagnostic {
        let mut diagnostic =
            Diagnostic::new(DiagnosticSeverity::Error, self.message.clone()).with_code("hir.lower");
        if let Some(range) = self.range.as_ref() {
            let span = document
                .span(SourceRange::new(range.start(), range.end()))
                .expect("a HIR lowering range belongs to the document that was lowered");
            diagnostic = diagnostic.with_label(DiagnosticLabel::primary(
                span,
                Some("HIR lowering failed here".to_owned()),
            ));
        }
        diagnostic
    }

    pub const fn range(&self) -> Option<&TextRange> {
        self.range.as_ref()
    }
}
