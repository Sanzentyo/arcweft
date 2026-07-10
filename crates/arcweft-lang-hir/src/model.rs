use arcweft_lang_syntax::{
    ast::{
        choice::{ChoiceAction, ChoiceItem, ChoicePlan},
        common::{TextRange, Visibility},
        decoration::DecorationItem,
        dialogue::{DialogueContent, DialogueDefaultsItem, LineArg},
        flow::{AuthoredExpr, AwaitBranchKind, ContractClause, SelectBranchHead, Stmt},
        ids::{EntityRef, EntityRefSyntax},
        items::{
            AgentItem, Attribute, CallableItem, EntityDeclItem, EntryDeclItem, EnumItem,
            ExternCapabilityItem, ExternModItem, FunctionKind, HookItem, ImplItem, MemoFn,
            ParserItem, StateItem, StructItem, StyleItem, TraitItem, TypeAliasItem,
        },
        line_plan::LinePlan,
        pattern::Pattern,
        proof::{BenchItem, ProofItem, TestItem, TrustedAxiomItem},
        source::SourceItem,
    },
    expr::Expr,
    types::FnSignature,
};
use arcweft_source::{
    Diagnostic, DiagnosticLabel, DiagnosticSeverity, SourceName, SourceRange, SourceSpan,
};
use thiserror::Error;

/// HIR-facing module produced from parsed surface syntax.
///
/// This is intentionally still close to syntax. Its role is to prove that the
/// parser exposes enough typed structure for later semantic analysis without
/// re-parsing raw strings.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirModule {
    pub(crate) attributes: Vec<Attribute>,
    pub(crate) source_len: Option<usize>,
    pub(crate) top_level_ranges: Vec<TextRange>,
    pub(crate) flows: Vec<HirFlow>,
    pub(crate) functions: Vec<HirFunction>,
    pub(crate) agents: Vec<HirAgent>,
    pub(crate) declarations: Vec<HirTopLevelDecl>,
    pub(crate) top_level_items: Vec<HirFlowItem>,
}

/// HIR-facing flow.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirFlow {
    pub(crate) attributes: Vec<Attribute>,
    pub(crate) id: Option<EntityRef>,
    pub(crate) name: Option<String>,
    pub(crate) signature: Option<FnSignature>,
    pub(crate) contracts: Vec<ContractClause>,
    pub(crate) body: Vec<HirFlowItem>,
    pub(crate) range: TextRange,
}

/// HIR-facing function body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirFunction {
    pub(crate) attributes: Vec<Attribute>,
    pub(crate) kind: FunctionKind,
    pub(crate) visibility: Option<Visibility>,
    pub(crate) signature: FnSignature,
    pub(crate) contracts: Vec<ContractClause>,
    pub(crate) statements: Vec<Stmt>,
    pub(crate) value: Option<AuthoredExpr>,
    pub(crate) range: TextRange,
}

/// HIR-facing Agent controller item.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirAgent {
    pub(crate) attributes: Vec<Attribute>,
    pub(crate) item: AgentItem,
}

/// HIR-facing top-level declaration preserved for later semantic passes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirTopLevelDecl {
    Callable(CallableItem),
    Decoration(DecorationItem),
    State(StateItem),
    Trait(TraitItem),
    Impl(ImplItem),
    Enum(EnumItem),
    EntityDecl(EntityDeclItem),
    Entry(EntryDeclItem),
    ExternCapability(ExternCapabilityItem),
    ExternMod(ExternModItem),
    DialogueDefaults(DialogueDefaultsItem),
    Struct(StructItem),
    TypeAlias(TypeAliasItem),
    Hook(HookItem),
    MemoFn(MemoFn),
    Proof(ProofItem),
    TrustedAxiom(TrustedAxiomItem),
    Test(TestItem),
    Bench(BenchItem),
    Parser(ParserItem),
    Source(SourceItem),
    Style(StyleItem),
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
    Borrow(HirBorrow),
    SourceLocale(HirSourceLocale),
    Scope(HirScope),
    Include(EntityRef),
    Await(HirAwait),
}

/// Dialogue call normalized enough for type checking to resolve speaker symbols.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirDialogue {
    pub(crate) callee: String,
    pub(crate) id: Option<EntityRef>,
    pub(crate) text_key: Option<EntityRef>,
    pub(crate) voice: Option<Expr>,
    pub(crate) look: Option<Expr>,
    pub(crate) stage: Option<Expr>,
    pub(crate) portrait: Option<Expr>,
    pub(crate) focus: Option<Expr>,
    pub(crate) cleanup: Option<Expr>,
    pub(crate) window: Option<EntityRef>,
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

/// HIR-facing zero-copy borrow block.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirBorrow {
    pub(crate) source: Expr,
    pub(crate) binding: Pattern,
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
    pub fn attributes(&self) -> &[Attribute] {
        &self.attributes
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

    pub fn agents(&self) -> &[HirAgent] {
        &self.agents
    }

    pub fn declarations(&self) -> &[HirTopLevelDecl] {
        &self.declarations
    }

    pub fn top_level_items(&self) -> &[HirFlowItem] {
        &self.top_level_items
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

    pub const fn id(&self) -> Option<&EntityRef> {
        self.id.as_ref()
    }

    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    pub const fn signature(&self) -> Option<&FnSignature> {
        self.signature.as_ref()
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

    pub fn has_attribute(&self, name: &str) -> bool {
        self.attributes
            .iter()
            .any(|attribute| attribute.name() == name)
    }

    pub const fn kind(&self) -> FunctionKind {
        self.kind
    }

    pub const fn visibility(&self) -> Option<Visibility> {
        self.visibility
    }

    pub fn name(&self) -> &str {
        self.signature.name()
    }

    pub const fn signature(&self) -> &FnSignature {
        &self.signature
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

impl HirAgent {
    pub fn attributes(&self) -> &[Attribute] {
        &self.attributes
    }

    pub fn has_attribute(&self, name: &str) -> bool {
        self.attributes
            .iter()
            .any(|attribute| attribute.name() == name)
    }

    pub const fn item(&self) -> &AgentItem {
        &self.item
    }
}

impl HirDialogue {
    pub fn expression_call(
        callee: String,
        content: DialogueContent,
        plan: Option<LinePlan>,
    ) -> Self {
        Self {
            callee,
            id: None,
            text_key: None,
            voice: None,
            look: None,
            stage: None,
            portrait: None,
            focus: None,
            cleanup: None,
            window: None,
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

    pub const fn window(&self) -> Option<&EntityRef> {
        self.window.as_ref()
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

impl HirBorrow {
    pub const fn source(&self) -> &Expr {
        &self.source
    }

    pub const fn binding(&self) -> &Pattern {
        &self.binding
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
    pub fn diagnostic(&self, source: &SourceName) -> Diagnostic {
        let mut diagnostic =
            Diagnostic::new(DiagnosticSeverity::Error, self.message.clone()).with_code("hir.lower");
        if let Some(range) = self.range.as_ref() {
            let span =
                SourceSpan::new(source.clone(), SourceRange::new(range.start(), range.end()));
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
