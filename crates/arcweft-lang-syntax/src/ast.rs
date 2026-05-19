use crate::expr::Expr;
use crate::types::{FnSignature, TypeRef};

pub mod choice;
pub mod common;
pub mod dialogue;
pub mod ids;
pub mod items;
pub mod line_plan;
pub mod proof;
pub mod source;

pub use choice::{
    ChoiceAction, ChoiceBlock, ChoiceItem, ChoiceMatchArm, ChoiceOption, ChoicePlan,
    ChoicePlanItem, ChoiceUiField,
};
pub use common::{DocBlock, ModuleDecl, TextRange, UseItem, UseMode, Visibility};
pub(crate) use dialogue::LineOptionsInit;
pub use dialogue::{
    ContentCall, DialogueContent, DialogueDefaultOption, DialogueDefaultsItem, DialogueTag,
    DialogueToken, LineArg, LineMark, LineOptions, ScenarioCommand, SpeakerLine,
};
pub use ids::{
    EntityRef, EntityRefSyntax, FamilyRelativeEntityRef, IdRef, RelativeId, RelativeIdSpelling,
    WikiLink,
};
pub use items::{Attribute, Item, RawItem, RawSyntax, RawSyntaxFamily, TypedSyntaxTree};
pub use line_plan::{
    BlockStyle, CancelRuleSyntax, DeferOutcome, LinePlan, LinePlanItem, TriggerPattern,
};
pub use proof::{BenchItem, ProofClause, ProofItem, TestItem, TestKind, TrustedAxiomItem};
pub(crate) use source::SourceItemParts;
pub use source::{
    SourceBackpressurePolicy, SourceEventPattern, SourceHandler, SourceHeader, SourceItem,
    SourceOverflowPolicy, SourcePrivacyPolicy, SourceReplayPolicy,
};

/// Flow item with typed header and parsed flow body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Flow {
    doc: Option<DocBlock>,
    kind: FlowKind,
    visibility: Option<Visibility>,
    id: Option<IdRef>,
    name: Option<String>,
    signature_tail: String,
    signature: Option<crate::types::FnSignature>,
    contracts: Vec<ContractClause>,
    body: Vec<FlowItem>,
    range: TextRange,
}

/// Internal initializer for a flow-like item.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FlowInit {
    pub(crate) doc: Option<DocBlock>,
    pub(crate) kind: FlowKind,
    pub(crate) visibility: Option<Visibility>,
    pub(crate) id: Option<IdRef>,
    pub(crate) name: Option<String>,
    pub(crate) signature_tail: String,
    pub(crate) signature: Option<crate::types::FnSignature>,
    pub(crate) contracts: Vec<ContractClause>,
    pub(crate) body: Vec<FlowItem>,
    pub(crate) range: TextRange,
}

/// Flow/function contract clause.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ContractClause {
    Requires { mode: Option<String>, expr: Expr },
    Ensures { mode: Option<String>, expr: Expr },
    Invariant { mode: Option<String>, expr: Expr },
    Assume { expr: Expr },
    Reads(Vec<Expr>),
    Effects(Vec<Expr>),
    NoEffect(Expr),
    Modifies(Vec<Expr>),
    Decreases(Expr),
}

/// Top-level function item with parsed signature head and contract clauses.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FunctionItem {
    doc: Option<DocBlock>,
    kind: FunctionKind,
    visibility: Option<Visibility>,
    signature: FnSignature,
    signature_text: String,
    contracts: Vec<ContractClause>,
    body: String,
    body_statements: Vec<Stmt>,
    body_value: Option<Expr>,
    range: TextRange,
}

/// Top-level function category.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FunctionKind {
    /// Ordinary synchronous function.
    Function,
    /// Background task function that may await non-visible work.
    Task,
    /// Dialogue-safe function callable from dialogue content tags.
    Dialogue,
    /// Generator-like function that yields a stream/source of values.
    Stream,
}

/// Top-level entity declaration family with runtime-specific body preserved.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EntityDeclKind {
    Character,
    Component,
    Activity,
    Signal,
    Metric,
    Layer,
    Textbox,
    Voice,
    Se,
    Bgm,
    AudioBus,
    MixerSnapshot,
    Ducking,
    Motion,
    Rig,
}

/// Top-level entity declaration such as `character`, `component`, `activity`,
/// `signal`, or `layer`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EntityDeclItem {
    kind: EntityDeclKind,
    visibility: Option<Visibility>,
    id: EntityRef,
    name: Option<String>,
    surface_alias: Option<String>,
    signature_tail: String,
    body: Option<String>,
    range: TextRange,
}

/// External module import declaration such as
/// `extern rust mod path from crate "name" { ... }`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExternModItem {
    abi: String,
    path: String,
    source: Option<String>,
    body: String,
    range: TextRange,
}

/// Internal initializer for a function item.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FunctionInit {
    pub(crate) doc: Option<DocBlock>,
    pub(crate) kind: FunctionKind,
    pub(crate) visibility: Option<Visibility>,
    pub(crate) signature: FnSignature,
    pub(crate) signature_text: String,
    pub(crate) contracts: Vec<ContractClause>,
    pub(crate) body: String,
    pub(crate) body_statements: Vec<Stmt>,
    pub(crate) body_value: Option<Expr>,
    pub(crate) range: TextRange,
}

/// Function-like top-level item such as `reducer` or `view`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallableItem {
    kind: CallableKind,
    visibility: Option<Visibility>,
    name: String,
    signature_tail: String,
    contracts: Vec<ContractClause>,
    body: String,
    range: TextRange,
}

/// Function-like item category.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CallableKind {
    Reducer,
    View,
}

/// Root state declaration with typed fields and initializer expressions.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StateItem {
    visibility: Option<Visibility>,
    name: String,
    fields: Vec<StateField>,
    range: TextRange,
}

/// One state field, optionally public, with its default expression.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StateField {
    doc: Option<DocBlock>,
    visibility: Option<Visibility>,
    name: String,
    ty: TypeRef,
    default: Expr,
}

/// Trait declaration with associated type and function members.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TraitItem {
    visibility: Option<Visibility>,
    name: String,
    supertraits: Vec<String>,
    members: Vec<TraitMember>,
    range: TextRange,
}

/// Member allowed inside a trait declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TraitMember {
    AssociatedType {
        name: String,
        params: Vec<String>,
        value: Option<TypeRef>,
    },
    Function {
        signature: FnSignature,
    },
    Raw(String),
}

/// Member allowed inside an impl declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ImplMember {
    AssociatedType {
        name: String,
        params: Vec<String>,
        value: TypeRef,
    },
    Function {
        signature: FnSignature,
        body: String,
        body_statements: Vec<Stmt>,
        body_value: Option<Expr>,
    },
    Raw(String),
}

/// Impl declaration with structured members and original body text.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImplItem {
    visibility: Option<Visibility>,
    generics: Option<String>,
    trait_name: Option<String>,
    target: String,
    members: Vec<ImplMember>,
    body: String,
    range: TextRange,
}

/// Top-level algebraic data type declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnumItem {
    visibility: Option<Visibility>,
    name: String,
    variants: Vec<EnumVariant>,
    range: TextRange,
}

/// One enum variant row, preserving payload syntax for later lowering.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnumVariant {
    doc: Option<DocBlock>,
    name: String,
    payload: Option<String>,
}

/// Top-level struct declaration with typed fields.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StructItem {
    visibility: Option<Visibility>,
    name: String,
    fields: Vec<StructField>,
    range: TextRange,
}

/// One `name: Type` struct field.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StructField {
    doc: Option<DocBlock>,
    name: String,
    ty: TypeRef,
}

/// Newtype/type alias declaration with optional `where` contracts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeAliasItem {
    visibility: Option<Visibility>,
    name: String,
    target: TypeRef,
    where_clauses: Vec<Expr>,
    range: TextRange,
}

/// Top-level flow-like item kind.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FlowKind {
    Flow,
    Fragment,
}

/// Syntax allowed in a `flow` body and in top-level scenario snippets.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FlowItem {
    Stmt(Stmt),
    ScenarioCommand(ScenarioCommand),
    SpeakerLine(SpeakerLine),
    ContentCall(ContentCall),
    Choice(ChoiceBlock),
    If(IfBlock),
    IfLet(IfLetBlock),
    Match(MatchBlock),
    Loop(LoopBlock),
    While(WhileBlock),
    WhileLet(WhileLetBlock),
    For(ForBlock),
    Select(SelectBlock),
    BorrowBlock(BorrowBlock),
    SourceLocale(SourceLocaleBlock),
    Scope(ScopeBlock),
    Include(EntityRefSyntax),
    AwaitWith(AwaitWith),
    Raw(RawSyntax),
}

/// Scoped source-locale override for directly authored text.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceLocaleBlock {
    locale: String,
    body: Vec<FlowItem>,
    range: TextRange,
}

/// `scope name { ... }` lexical block, plus bare `{ ... }` sugar with no name.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScopeBlock {
    name: Option<String>,
    body: Vec<FlowItem>,
    range: TextRange,
}

/// `scope name? { ... }` used in expression position.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScopeExprBlock {
    name: Option<String>,
    statements: Vec<Stmt>,
    value: Option<Expr>,
    range: TextRange,
}

/// Typed `if expr { ... }` flow block.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IfBlock {
    condition: Expr,
    body: Vec<FlowItem>,
    range: TextRange,
}

/// Typed `if let PAT = expr when guard { ... }` flow block.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IfLetBlock {
    pattern: Pattern,
    expr: Expr,
    guard: Option<Expr>,
    body: Vec<FlowItem>,
    range: TextRange,
}

/// Typed `match expr { ... }` flow block.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MatchBlock {
    expr: Expr,
    arms: Vec<MatchArm>,
    range: TextRange,
}

/// One `pattern => flow item` match arm.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MatchArm {
    pattern: Pattern,
    guard: Option<Expr>,
    body: Vec<FlowItem>,
}

/// `loop { ... }` value-capable loop block.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoopBlock {
    label: Option<String>,
    body: Vec<FlowItem>,
    range: TextRange,
}

/// `for pattern in expr { ... }` sequence loop.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ForBlock {
    pattern: Pattern,
    source: Expr,
    body: Vec<FlowItem>,
    range: TextRange,
}

/// `while condition { ... }` statement loop.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WhileBlock {
    condition: Expr,
    body: Vec<FlowItem>,
    range: TextRange,
}

/// `while let PAT = expr when guard { ... }` statement loop.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WhileLetBlock {
    pattern: Pattern,
    expr: Expr,
    guard: Option<Expr>,
    body: Vec<FlowItem>,
    range: TextRange,
}

/// Source-aware `select { ... }` block.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelectBlock {
    branches: Vec<SelectBranch>,
    range: TextRange,
}

/// One select branch with a parsed head and nested flow body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelectBranch {
    head: SelectBranchHead,
    body: Vec<FlowItem>,
}

/// Select branch head categories documented for source consumption.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SelectBranchHead {
    Bind {
        name: String,
        source: Expr,
        propagates_error: bool,
    },
    Frame(Pattern),
    Event(Pattern),
    Raw(String),
}

/// One `match` arm inside a typed statement block.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StmtMatchArm {
    pattern: Pattern,
    guard: Option<Expr>,
    body: Vec<Stmt>,
}

/// `borrow expr as name: Type { ... }` zero-copy borrow block.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BorrowBlock {
    source: Expr,
    binding: Pattern,
    body: Vec<FlowItem>,
    range: TextRange,
}

/// Typed Arcweft statement inside a flow body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Stmt {
    Let {
        pattern: Pattern,
        ty: Option<TypeRef>,
        expr: Expr,
    },
    /// `let PAT = EXPR else { ... }` binding whose else block must diverge.
    LetElse {
        pattern: Pattern,
        ty: Option<TypeRef>,
        expr: Expr,
        else_body: Vec<Stmt>,
    },
    /// `let PAT = choice ... { ... }` choice expression binding.
    LetChoice {
        pattern: Pattern,
        choice: ChoiceBlock,
    },
    /// `let PAT = scope name { ... }` named block expression binding.
    LetScope {
        pattern: Pattern,
        scope: ScopeExprBlock,
    },
    /// `let PAT = loop { ... }` loop expression binding.
    LetLoop {
        pattern: Pattern,
        block: LoopBlock,
    },
    /// `let PAT = try await EXPR with ...` wait-view expression binding.
    LetAwait {
        pattern: Pattern,
        await_with: AwaitWith,
    },
    Return(Expr),
    /// `out expr` or `out 'label expr` from a line/cue/content continuation.
    Out {
        label: Option<String>,
        expr: Expr,
    },
    Goto(Expr),
    /// `thread name { ... }` / `thread name:` scoped VM child task.
    Thread(ThreadBlock),
    /// `defer { ... }` cleanup block registered on the current runtime scope.
    DeferBlock {
        outcome: DeferOutcome,
        statements: Vec<Stmt>,
    },
    Defer {
        outcome: DeferOutcome,
        expr: Expr,
    },
    Yield(Expr),
    Panic(Expr),
    Fail(Expr),
    /// `bail expr` constructs and returns an error from the current continuation.
    Bail(Expr),
    /// `ensure cond, msg` checks a recoverable invariant and bails on failure.
    Ensure {
        condition: Expr,
        message: Expr,
    },
    Signal {
        target: Expr,
        value: Expr,
    },
    /// `'line.key <- expr` stores a scoped handle in the named lifetime registry.
    LifetimeSet {
        target: Expr,
        expr: Expr,
    },
    /// `wait mark .name` or `wait 0.35s` waits inside a line-local task.
    Wait(WaitTarget),
    /// `on head => stmt` event branch used by source and plan-like bodies.
    On {
        trigger: TriggerPattern,
        body: Vec<Stmt>,
    },
    /// `unsafe lifetime @unsafe.id reason = "..." { ... }` audit region.
    UnsafeLifetime {
        id: IdRef,
        reason: Option<Expr>,
        has_safety_doc: bool,
        body: Vec<Stmt>,
    },
    Command(ScenarioCommand),
    If {
        condition: Expr,
        body: Vec<Stmt>,
    },
    /// `loop { ... }` inside typed statement bodies.
    Loop {
        body: Vec<Stmt>,
    },
    /// `while expr { ... }` inside typed statement bodies.
    While {
        condition: Expr,
        body: Vec<Stmt>,
    },
    /// `while let PAT = EXPR when GUARD { ... }` inside typed statement bodies.
    WhileLet {
        pattern: Pattern,
        expr: Expr,
        guard: Option<Expr>,
        body: Vec<Stmt>,
    },
    /// `for PAT in EXPR { ... }` inside typed statement bodies.
    For {
        pattern: Pattern,
        source: Expr,
        body: Vec<Stmt>,
    },
    Match {
        expr: Expr,
        arms: Vec<StmtMatchArm>,
    },
    Close(Expr),
    Select(Expr),
    /// `break`, `break expr`, or `break 'label expr`.
    Break {
        label: Option<String>,
        expr: Option<Expr>,
    },
    /// `continue` or `continue 'label`.
    Continue {
        label: Option<String>,
    },
    Expr(Expr),
    Raw(RawSyntax),
}

/// A scoped VM child task owned by the nearest runtime scope.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ThreadBlock {
    modifiers: Vec<ThreadModifier>,
    name: Option<String>,
    body: Vec<Stmt>,
}

/// Modifier attached to a `thread` block.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ThreadModifier {
    Detached,
}

/// Target accepted by a structured `wait` statement.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WaitTarget {
    Duration(Expr),
    Mark(String),
    Expr(Expr),
}

/// Pattern syntax used by `let` and line-plan return destructuring.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Pattern {
    Ident(String),
    MutIdent(String),
    Literal(Expr),
    Entity(EntityRef),
    Variant {
        path: Option<String>,
        name: String,
        payload: Option<VariantPatternPayload>,
    },
    Discard,
    Tuple(Vec<Pattern>),
    Record {
        path: Option<String>,
        fields: Vec<RecordPatternField>,
        rest: bool,
    },
    BracketSeq {
        items: Vec<Pattern>,
        rest: Option<String>,
    },
    Whole {
        name: String,
        pattern: Box<Pattern>,
    },
    Typed {
        name: String,
        ty: TypeRef,
    },
    Raw(String),
}

/// One field inside a record/struct pattern.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecordPatternField {
    name: String,
    pattern: Pattern,
}

/// Payload attached to an enum variant pattern.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VariantPatternPayload {
    Tuple(Vec<Pattern>),
    Record {
        fields: Vec<RecordPatternField>,
        rest: bool,
    },
}

impl RecordPatternField {
    pub(crate) fn new(name: impl Into<String>, pattern: Pattern) -> Self {
        Self {
            name: name.into(),
            pattern,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub const fn pattern(&self) -> &Pattern {
        &self.pattern
    }
}

impl ThreadBlock {
    pub(crate) fn new(
        modifiers: Vec<ThreadModifier>,
        name: Option<String>,
        body: Vec<Stmt>,
    ) -> Self {
        Self {
            modifiers,
            name,
            body,
        }
    }

    pub fn modifiers(&self) -> &[ThreadModifier] {
        &self.modifiers
    }

    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    pub fn body(&self) -> &[Stmt] {
        &self.body
    }

    pub fn is_detached(&self) -> bool {
        self.modifiers.contains(&ThreadModifier::Detached)
    }
}

impl ThreadModifier {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Detached => "detached",
        }
    }
}

/// `await expr with ...` or `try await expr with ...` wait-view syntax.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AwaitWith {
    expr: Expr,
    applies_try: bool,
    branches: Vec<AwaitBranch>,
}

/// One branch in an `await ... with` wait-view block.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AwaitBranch {
    kind: AwaitBranchKind,
    pattern: Pattern,
    body: Vec<FlowItem>,
}

/// Wait-view branch kind.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AwaitBranchKind {
    Pending,
    Ready,
    Error,
    Denied,
}

/// Hook item syntax.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HookItem {
    visibility: Option<Visibility>,
    id: EntityRef,
    target: String,
    phase: String,
    when: Option<Expr>,
    priority: Option<i64>,
    once: bool,
    effects: Vec<Expr>,
    body: String,
    body_statements: Vec<Stmt>,
    range: TextRange,
}

/// Internal initializer for hook syntax.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct HookInit {
    pub(crate) visibility: Option<Visibility>,
    pub(crate) id: EntityRef,
    pub(crate) target: String,
    pub(crate) phase: String,
    pub(crate) when: Option<Expr>,
    pub(crate) priority: Option<i64>,
    pub(crate) once: bool,
    pub(crate) effects: Vec<Expr>,
    pub(crate) body: String,
    pub(crate) body_statements: Vec<Stmt>,
    pub(crate) range: TextRange,
}

/// Memoized function item.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoFn {
    visibility: Option<Visibility>,
    signature: String,
    options: Vec<String>,
    body: String,
    body_statements: Vec<Stmt>,
    body_value: Option<Expr>,
    range: TextRange,
}

/// User-defined parser item.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParserItem {
    visibility: Option<Visibility>,
    name: String,
    signature_tail: String,
    body: String,
    body_statements: Vec<Stmt>,
    body_value: Option<Expr>,
    range: TextRange,
}

impl Flow {
    pub(crate) fn new(init: FlowInit) -> Self {
        Self {
            doc: init.doc,
            kind: init.kind,
            visibility: init.visibility,
            id: init.id,
            name: init.name,
            signature_tail: init.signature_tail,
            signature: init.signature,
            contracts: init.contracts,
            body: init.body,
            range: init.range,
        }
    }

    pub const fn kind(&self) -> FlowKind {
        self.kind
    }

    pub const fn doc(&self) -> Option<&DocBlock> {
        self.doc.as_ref()
    }

    pub const fn visibility(&self) -> Option<Visibility> {
        self.visibility
    }

    pub const fn id(&self) -> Option<&IdRef> {
        self.id.as_ref()
    }

    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    pub fn signature_tail(&self) -> &str {
        &self.signature_tail
    }

    pub const fn signature(&self) -> Option<&crate::types::FnSignature> {
        self.signature.as_ref()
    }

    pub fn contracts(&self) -> &[ContractClause] {
        &self.contracts
    }

    pub fn body(&self) -> &[FlowItem] {
        &self.body
    }

    pub const fn range(&self) -> &TextRange {
        &self.range
    }
}

impl FunctionItem {
    pub(crate) fn new(init: FunctionInit) -> Self {
        Self {
            doc: init.doc,
            kind: init.kind,
            visibility: init.visibility,
            signature: init.signature,
            signature_text: init.signature_text,
            contracts: init.contracts,
            body: init.body,
            body_statements: init.body_statements,
            body_value: init.body_value,
            range: init.range,
        }
    }

    pub const fn kind(&self) -> FunctionKind {
        self.kind
    }

    pub const fn doc(&self) -> Option<&DocBlock> {
        self.doc.as_ref()
    }

    pub const fn visibility(&self) -> Option<Visibility> {
        self.visibility
    }

    pub const fn signature(&self) -> &FnSignature {
        &self.signature
    }

    pub fn signature_text(&self) -> &str {
        &self.signature_text
    }

    pub fn contracts(&self) -> &[ContractClause] {
        &self.contracts
    }

    pub fn body(&self) -> &str {
        &self.body
    }

    pub fn body_statements(&self) -> &[Stmt] {
        &self.body_statements
    }

    pub const fn body_value(&self) -> Option<&Expr> {
        self.body_value.as_ref()
    }

    pub const fn range(&self) -> &TextRange {
        &self.range
    }
}

impl EntityDeclItem {
    #[allow(clippy::too_many_arguments)]
    pub(crate) const fn new(
        kind: EntityDeclKind,
        visibility: Option<Visibility>,
        id: EntityRef,
        name: Option<String>,
        surface_alias: Option<String>,
        signature_tail: String,
        body: Option<String>,
        range: TextRange,
    ) -> Self {
        Self {
            kind,
            visibility,
            id,
            name,
            surface_alias,
            signature_tail,
            body,
            range,
        }
    }

    pub const fn kind(&self) -> EntityDeclKind {
        self.kind
    }

    pub const fn visibility(&self) -> Option<Visibility> {
        self.visibility
    }

    pub const fn id(&self) -> &EntityRef {
        &self.id
    }

    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    pub fn surface_alias(&self) -> Option<&str> {
        self.surface_alias.as_deref()
    }

    pub fn signature_tail(&self) -> &str {
        &self.signature_tail
    }

    pub fn body(&self) -> Option<&str> {
        self.body.as_deref()
    }

    pub const fn range(&self) -> &TextRange {
        &self.range
    }
}

impl ExternModItem {
    pub(crate) const fn new(
        abi: String,
        path: String,
        source: Option<String>,
        body: String,
        range: TextRange,
    ) -> Self {
        Self {
            abi,
            path,
            source,
            body,
            range,
        }
    }

    pub fn abi(&self) -> &str {
        &self.abi
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn source(&self) -> Option<&str> {
        self.source.as_deref()
    }

    pub fn body(&self) -> &str {
        &self.body
    }

    pub const fn range(&self) -> &TextRange {
        &self.range
    }
}

impl CallableItem {
    pub(crate) fn new(
        kind: CallableKind,
        visibility: Option<Visibility>,
        name: String,
        signature_tail: String,
        contracts: Vec<ContractClause>,
        body: String,
        range: TextRange,
    ) -> Self {
        Self {
            kind,
            visibility,
            name,
            signature_tail,
            contracts,
            body,
            range,
        }
    }

    pub const fn kind(&self) -> CallableKind {
        self.kind
    }

    pub const fn visibility(&self) -> Option<Visibility> {
        self.visibility
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn signature_tail(&self) -> &str {
        &self.signature_tail
    }

    pub fn contracts(&self) -> &[ContractClause] {
        &self.contracts
    }

    pub fn body(&self) -> &str {
        &self.body
    }

    pub const fn range(&self) -> &TextRange {
        &self.range
    }
}

impl StateItem {
    pub(crate) const fn new(
        visibility: Option<Visibility>,
        name: String,
        fields: Vec<StateField>,
        range: TextRange,
    ) -> Self {
        Self {
            visibility,
            name,
            fields,
            range,
        }
    }

    pub const fn visibility(&self) -> Option<Visibility> {
        self.visibility
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn fields(&self) -> &[StateField] {
        &self.fields
    }

    pub const fn range(&self) -> &TextRange {
        &self.range
    }
}

impl StateField {
    pub(crate) const fn new(
        doc: Option<DocBlock>,
        visibility: Option<Visibility>,
        name: String,
        ty: TypeRef,
        default: Expr,
    ) -> Self {
        Self {
            doc,
            visibility,
            name,
            ty,
            default,
        }
    }

    pub const fn visibility(&self) -> Option<Visibility> {
        self.visibility
    }

    pub const fn doc(&self) -> Option<&DocBlock> {
        self.doc.as_ref()
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub const fn ty(&self) -> &TypeRef {
        &self.ty
    }

    pub const fn default(&self) -> &Expr {
        &self.default
    }
}

impl TraitItem {
    pub(crate) const fn new(
        visibility: Option<Visibility>,
        name: String,
        supertraits: Vec<String>,
        members: Vec<TraitMember>,
        range: TextRange,
    ) -> Self {
        Self {
            visibility,
            name,
            supertraits,
            members,
            range,
        }
    }

    pub const fn visibility(&self) -> Option<Visibility> {
        self.visibility
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn supertraits(&self) -> &[String] {
        &self.supertraits
    }

    pub fn members(&self) -> &[TraitMember] {
        &self.members
    }

    pub const fn range(&self) -> &TextRange {
        &self.range
    }
}

impl ImplItem {
    pub(crate) const fn new(
        visibility: Option<Visibility>,
        generics: Option<String>,
        trait_name: Option<String>,
        target: String,
        members: Vec<ImplMember>,
        body: String,
        range: TextRange,
    ) -> Self {
        Self {
            visibility,
            generics,
            trait_name,
            target,
            members,
            body,
            range,
        }
    }

    pub const fn visibility(&self) -> Option<Visibility> {
        self.visibility
    }

    pub fn generics(&self) -> Option<&str> {
        self.generics.as_deref()
    }

    pub fn trait_name(&self) -> Option<&str> {
        self.trait_name.as_deref()
    }

    pub fn target(&self) -> &str {
        &self.target
    }

    pub fn members(&self) -> &[ImplMember] {
        &self.members
    }

    pub fn body(&self) -> &str {
        &self.body
    }

    pub const fn range(&self) -> &TextRange {
        &self.range
    }
}

impl EnumItem {
    pub(crate) const fn new(
        visibility: Option<Visibility>,
        name: String,
        variants: Vec<EnumVariant>,
        range: TextRange,
    ) -> Self {
        Self {
            visibility,
            name,
            variants,
            range,
        }
    }

    pub const fn visibility(&self) -> Option<Visibility> {
        self.visibility
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn variants(&self) -> &[EnumVariant] {
        &self.variants
    }

    pub const fn range(&self) -> &TextRange {
        &self.range
    }
}

impl EnumVariant {
    pub(crate) const fn new(doc: Option<DocBlock>, name: String, payload: Option<String>) -> Self {
        Self { doc, name, payload }
    }

    pub const fn doc(&self) -> Option<&DocBlock> {
        self.doc.as_ref()
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn payload(&self) -> Option<&str> {
        self.payload.as_deref()
    }
}

impl StructItem {
    pub(crate) const fn new(
        visibility: Option<Visibility>,
        name: String,
        fields: Vec<StructField>,
        range: TextRange,
    ) -> Self {
        Self {
            visibility,
            name,
            fields,
            range,
        }
    }

    pub const fn visibility(&self) -> Option<Visibility> {
        self.visibility
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn fields(&self) -> &[StructField] {
        &self.fields
    }

    pub const fn range(&self) -> &TextRange {
        &self.range
    }
}

impl StructField {
    pub(crate) const fn new(doc: Option<DocBlock>, name: String, ty: TypeRef) -> Self {
        Self { doc, name, ty }
    }

    pub const fn doc(&self) -> Option<&DocBlock> {
        self.doc.as_ref()
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub const fn ty(&self) -> &TypeRef {
        &self.ty
    }
}

impl TypeAliasItem {
    pub(crate) const fn new(
        visibility: Option<Visibility>,
        name: String,
        target: TypeRef,
        where_clauses: Vec<Expr>,
        range: TextRange,
    ) -> Self {
        Self {
            visibility,
            name,
            target,
            where_clauses,
            range,
        }
    }

    pub const fn visibility(&self) -> Option<Visibility> {
        self.visibility
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub const fn target(&self) -> &TypeRef {
        &self.target
    }

    pub fn where_clauses(&self) -> &[Expr] {
        &self.where_clauses
    }

    pub const fn range(&self) -> &TextRange {
        &self.range
    }
}

impl IfBlock {
    pub(crate) const fn new(condition: Expr, body: Vec<FlowItem>, range: TextRange) -> Self {
        Self {
            condition,
            body,
            range,
        }
    }

    pub const fn condition(&self) -> &Expr {
        &self.condition
    }

    pub fn body(&self) -> &[FlowItem] {
        &self.body
    }

    pub const fn range(&self) -> &TextRange {
        &self.range
    }
}

impl IfLetBlock {
    pub(crate) const fn new(
        pattern: Pattern,
        expr: Expr,
        guard: Option<Expr>,
        body: Vec<FlowItem>,
        range: TextRange,
    ) -> Self {
        Self {
            pattern,
            expr,
            guard,
            body,
            range,
        }
    }

    pub const fn pattern(&self) -> &Pattern {
        &self.pattern
    }

    pub const fn expr(&self) -> &Expr {
        &self.expr
    }

    pub const fn guard(&self) -> Option<&Expr> {
        self.guard.as_ref()
    }

    pub fn body(&self) -> &[FlowItem] {
        &self.body
    }

    pub const fn range(&self) -> &TextRange {
        &self.range
    }
}

impl MatchBlock {
    pub(crate) const fn new(expr: Expr, arms: Vec<MatchArm>, range: TextRange) -> Self {
        Self { expr, arms, range }
    }

    pub const fn expr(&self) -> &Expr {
        &self.expr
    }

    pub fn arms(&self) -> &[MatchArm] {
        &self.arms
    }

    pub const fn range(&self) -> &TextRange {
        &self.range
    }
}

impl MatchArm {
    pub(crate) const fn new(pattern: Pattern, guard: Option<Expr>, body: Vec<FlowItem>) -> Self {
        Self {
            pattern,
            guard,
            body,
        }
    }

    pub const fn pattern(&self) -> &Pattern {
        &self.pattern
    }

    pub const fn guard(&self) -> Option<&Expr> {
        self.guard.as_ref()
    }

    pub fn body(&self) -> &[FlowItem] {
        &self.body
    }
}

impl LoopBlock {
    pub(crate) const fn new(label: Option<String>, body: Vec<FlowItem>, range: TextRange) -> Self {
        Self { label, body, range }
    }

    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }

    pub fn body(&self) -> &[FlowItem] {
        &self.body
    }

    pub const fn range(&self) -> &TextRange {
        &self.range
    }
}

impl ForBlock {
    pub(crate) const fn new(
        pattern: Pattern,
        source: Expr,
        body: Vec<FlowItem>,
        range: TextRange,
    ) -> Self {
        Self {
            pattern,
            source,
            body,
            range,
        }
    }

    pub const fn pattern(&self) -> &Pattern {
        &self.pattern
    }

    pub const fn source(&self) -> &Expr {
        &self.source
    }

    pub fn body(&self) -> &[FlowItem] {
        &self.body
    }

    pub const fn range(&self) -> &TextRange {
        &self.range
    }
}

impl WhileBlock {
    pub(crate) const fn new(condition: Expr, body: Vec<FlowItem>, range: TextRange) -> Self {
        Self {
            condition,
            body,
            range,
        }
    }

    pub const fn condition(&self) -> &Expr {
        &self.condition
    }

    pub fn body(&self) -> &[FlowItem] {
        &self.body
    }

    pub const fn range(&self) -> &TextRange {
        &self.range
    }
}

impl WhileLetBlock {
    pub(crate) const fn new(
        pattern: Pattern,
        expr: Expr,
        guard: Option<Expr>,
        body: Vec<FlowItem>,
        range: TextRange,
    ) -> Self {
        Self {
            pattern,
            expr,
            guard,
            body,
            range,
        }
    }

    pub const fn pattern(&self) -> &Pattern {
        &self.pattern
    }

    pub const fn expr(&self) -> &Expr {
        &self.expr
    }

    pub const fn guard(&self) -> Option<&Expr> {
        self.guard.as_ref()
    }

    pub fn body(&self) -> &[FlowItem] {
        &self.body
    }

    pub const fn range(&self) -> &TextRange {
        &self.range
    }
}

impl SelectBlock {
    pub(crate) const fn new(branches: Vec<SelectBranch>, range: TextRange) -> Self {
        Self { branches, range }
    }

    pub fn branches(&self) -> &[SelectBranch] {
        &self.branches
    }

    pub const fn range(&self) -> &TextRange {
        &self.range
    }
}

impl SelectBranch {
    pub(crate) const fn new(head: SelectBranchHead, body: Vec<FlowItem>) -> Self {
        Self { head, body }
    }

    pub const fn head(&self) -> &SelectBranchHead {
        &self.head
    }

    pub fn body(&self) -> &[FlowItem] {
        &self.body
    }
}

impl StmtMatchArm {
    pub(crate) const fn new(pattern: Pattern, guard: Option<Expr>, body: Vec<Stmt>) -> Self {
        Self {
            pattern,
            guard,
            body,
        }
    }

    pub const fn pattern(&self) -> &Pattern {
        &self.pattern
    }

    pub const fn guard(&self) -> Option<&Expr> {
        self.guard.as_ref()
    }

    pub fn body(&self) -> &[Stmt] {
        &self.body
    }
}

impl BorrowBlock {
    pub(crate) const fn new(
        source: Expr,
        binding: Pattern,
        body: Vec<FlowItem>,
        range: TextRange,
    ) -> Self {
        Self {
            source,
            binding,
            body,
            range,
        }
    }

    pub const fn source(&self) -> &Expr {
        &self.source
    }

    pub const fn binding(&self) -> &Pattern {
        &self.binding
    }

    pub fn body(&self) -> &[FlowItem] {
        &self.body
    }

    pub const fn range(&self) -> &TextRange {
        &self.range
    }
}

impl AwaitWith {
    pub(crate) const fn new(expr: Expr, applies_try: bool, branches: Vec<AwaitBranch>) -> Self {
        Self {
            expr,
            applies_try,
            branches,
        }
    }

    pub const fn expr(&self) -> &Expr {
        &self.expr
    }

    pub const fn applies_try(&self) -> bool {
        self.applies_try
    }

    pub fn branches(&self) -> &[AwaitBranch] {
        &self.branches
    }

    pub fn pending(&self) -> Option<&AwaitBranch> {
        self.branches
            .iter()
            .find(|branch| branch.kind == AwaitBranchKind::Pending)
    }
}

impl AwaitBranch {
    pub(crate) const fn new(kind: AwaitBranchKind, pattern: Pattern, body: Vec<FlowItem>) -> Self {
        Self {
            kind,
            pattern,
            body,
        }
    }

    pub const fn kind(&self) -> AwaitBranchKind {
        self.kind
    }

    pub const fn pattern(&self) -> &Pattern {
        &self.pattern
    }

    pub fn body(&self) -> &[FlowItem] {
        &self.body
    }
}

impl SourceLocaleBlock {
    pub(crate) const fn new(locale: String, body: Vec<FlowItem>, range: TextRange) -> Self {
        Self {
            locale,
            body,
            range,
        }
    }

    pub fn locale(&self) -> &str {
        &self.locale
    }

    pub fn body(&self) -> &[FlowItem] {
        &self.body
    }

    pub const fn range(&self) -> &TextRange {
        &self.range
    }
}

impl ScopeBlock {
    pub(crate) const fn new(name: Option<String>, body: Vec<FlowItem>, range: TextRange) -> Self {
        Self { name, body, range }
    }

    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    pub fn body(&self) -> &[FlowItem] {
        &self.body
    }

    pub const fn range(&self) -> &TextRange {
        &self.range
    }
}

impl ScopeExprBlock {
    pub(crate) const fn new(
        name: Option<String>,
        statements: Vec<Stmt>,
        value: Option<Expr>,
        range: TextRange,
    ) -> Self {
        Self {
            name,
            statements,
            value,
            range,
        }
    }

    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    pub fn statements(&self) -> &[Stmt] {
        &self.statements
    }

    pub const fn value(&self) -> Option<&Expr> {
        self.value.as_ref()
    }

    pub const fn range(&self) -> TextRange {
        self.range
    }
}

impl HookItem {
    pub(crate) fn new(init: HookInit) -> Self {
        Self {
            visibility: init.visibility,
            id: init.id,
            target: init.target,
            phase: init.phase,
            when: init.when,
            priority: init.priority,
            once: init.once,
            effects: init.effects,
            body: init.body,
            body_statements: init.body_statements,
            range: init.range,
        }
    }

    pub const fn visibility(&self) -> Option<Visibility> {
        self.visibility
    }

    pub const fn id(&self) -> &EntityRef {
        &self.id
    }

    pub fn target(&self) -> &str {
        &self.target
    }

    pub fn phase(&self) -> &str {
        &self.phase
    }

    pub const fn when(&self) -> Option<&Expr> {
        self.when.as_ref()
    }

    pub const fn priority(&self) -> Option<i64> {
        self.priority
    }

    pub const fn once(&self) -> bool {
        self.once
    }

    pub fn effects(&self) -> &[Expr] {
        &self.effects
    }

    pub fn body(&self) -> &str {
        &self.body
    }

    pub fn body_statements(&self) -> &[Stmt] {
        &self.body_statements
    }

    pub const fn range(&self) -> &TextRange {
        &self.range
    }
}

impl MemoFn {
    pub(crate) const fn new(
        visibility: Option<Visibility>,
        signature: String,
        options: Vec<String>,
        body: String,
        body_statements: Vec<Stmt>,
        body_value: Option<Expr>,
        range: TextRange,
    ) -> Self {
        Self {
            visibility,
            signature,
            options,
            body,
            body_statements,
            body_value,
            range,
        }
    }

    pub const fn visibility(&self) -> Option<Visibility> {
        self.visibility
    }

    pub fn signature(&self) -> &str {
        &self.signature
    }

    pub fn options(&self) -> &[String] {
        &self.options
    }

    pub fn body(&self) -> &str {
        &self.body
    }

    pub fn body_statements(&self) -> &[Stmt] {
        &self.body_statements
    }

    pub const fn body_value(&self) -> Option<&Expr> {
        self.body_value.as_ref()
    }

    pub const fn range(&self) -> &TextRange {
        &self.range
    }
}

impl ParserItem {
    pub(crate) const fn new(
        visibility: Option<Visibility>,
        name: String,
        signature_tail: String,
        body: String,
        body_statements: Vec<Stmt>,
        body_value: Option<Expr>,
        range: TextRange,
    ) -> Self {
        Self {
            visibility,
            name,
            signature_tail,
            body,
            body_statements,
            body_value,
            range,
        }
    }

    pub const fn visibility(&self) -> Option<Visibility> {
        self.visibility
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn signature_tail(&self) -> &str {
        &self.signature_tail
    }

    pub fn body(&self) -> &str {
        &self.body
    }

    pub fn body_statements(&self) -> &[Stmt] {
        &self.body_statements
    }

    pub const fn body_value(&self) -> Option<&Expr> {
        self.body_value.as_ref()
    }

    pub const fn range(&self) -> &TextRange {
        &self.range
    }
}
