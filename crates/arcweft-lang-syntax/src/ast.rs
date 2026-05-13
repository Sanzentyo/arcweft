use crate::expr::Expr;
use crate::types::{FnSignature, TypeRef};
use core::ops::Range;

/// Half-open byte range in the original source.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TextRange {
    start: usize,
    end: usize,
}

/// Parsed `.awft` source with module/use headers and syntax items.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxTree {
    source: String,
    module: Option<ModuleDecl>,
    uses: Vec<UseItem>,
    items: Vec<Item>,
    wiki_links: Vec<WikiLink>,
}

/// `mod game::routes::opening`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModuleDecl {
    path: String,
    range: TextRange,
}

/// `use`, `lazy use`, or `eager use` import.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UseItem {
    visibility: Option<Visibility>,
    mode: Option<UseMode>,
    tree: String,
    range: TextRange,
}

/// Arcweft visibility qualifier.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Visibility {
    Public,
    Crate,
    Super,
}

/// Import realization mode.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UseMode {
    Lazy,
    Eager,
}

/// Top-level syntax item.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Item {
    Attribute(Attribute),
    Flow(Flow),
    Function(FunctionItem),
    Callable(CallableItem),
    State(StateItem),
    Trait(TraitItem),
    Impl(ImplItem),
    Enum(EnumItem),
    Struct(StructItem),
    TypeAlias(TypeAliasItem),
    EntityDecl(EntityDeclItem),
    ExternMod(ExternModItem),
    Hook(HookItem),
    MemoFn(MemoFn),
    Parser(ParserItem),
    Source(SourceItem),
    FlowItem(FlowItem),
    Raw(RawItem),
}

/// Raw top-level item preserved for grammar families not lowered yet.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RawItem {
    head: String,
    body: Option<String>,
    range: TextRange,
}

/// Entity reference such as `#flow.opening` or `#<flow.opening@sem:abc>`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EntityRef {
    body: String,
    delimited: bool,
    relative: bool,
    range: TextRange,
}

/// Documentation/RAG link written as `[[...]]`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WikiLink {
    body: String,
    range: TextRange,
}

/// Attribute syntax such as `@derive(...)`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Attribute {
    name: String,
    args: Option<String>,
    range: TextRange,
}

/// Flow item with typed header and parsed flow body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Flow {
    kind: FlowKind,
    visibility: Option<Visibility>,
    id: Option<EntityRef>,
    name: Option<String>,
    signature_tail: String,
    contracts: Vec<ContractClause>,
    body: Vec<FlowItem>,
    range: TextRange,
}

/// Internal initializer for a flow-like item.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FlowInit {
    pub(crate) kind: FlowKind,
    pub(crate) visibility: Option<Visibility>,
    pub(crate) id: Option<EntityRef>,
    pub(crate) name: Option<String>,
    pub(crate) signature_tail: String,
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
    Layer,
}

/// Top-level entity declaration such as `character`, `component`, `activity`,
/// `signal`, or `layer`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EntityDeclItem {
    kind: EntityDeclKind,
    visibility: Option<Visibility>,
    id: EntityRef,
    name: Option<String>,
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
    Block(LexicalBlock),
    Scope(ScopeBlock),
    Include(EntityRef),
    AwaitWith(AwaitWith),
    Raw(String),
}

/// Scoped source-locale override for directly authored text.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceLocaleBlock {
    locale: String,
    body: Vec<FlowItem>,
    range: TextRange,
}

/// Bare `{ ... }` lexical block used as a statement.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LexicalBlock {
    body: Vec<FlowItem>,
    range: TextRange,
}

/// `scope name { ... }` lexical block that also names generated IDs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScopeBlock {
    name: String,
    body: Vec<FlowItem>,
    range: TextRange,
}

/// `scope name { ... }` used in expression position.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScopeExprBlock {
    name: String,
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
        expr: Expr,
    },
    /// `let PAT = EXPR else { ... }` binding whose else block must diverge.
    LetElse {
        pattern: Pattern,
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
    Spawn(Expr),
    Defer(Expr),
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
    Emit {
        event: Expr,
        fields: Vec<(String, Expr)>,
    },
    /// `on head => stmt` event branch used by source and plan-like bodies.
    On {
        head: String,
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
    Raw(String),
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
    List {
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

/// Compact `@bg`, `@show`, and similar scenario command.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScenarioCommand {
    name: String,
    args: Vec<Expr>,
    range: TextRange,
}

/// Parsed dialogue content.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DialogueContent {
    raw: String,
    tokens: Vec<DialogueToken>,
    range: TextRange,
}

/// Token emitted inside dialogue text mode.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DialogueToken {
    Text(String),
    Tag(DialogueTag),
    EndTag(String),
    Expr(Expr),
    Ruby { base: String, ruby: String },
    Escape(char),
}

/// Bracket tag such as `[p]`, `[hook ...]`, or `[ruby rt="..."]`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DialogueTag {
    name: String,
    attrs: String,
}

/// `alice(args): ...` speaker-line sugar for a character dialogue call.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SpeakerLine {
    speaker: String,
    args: Option<String>,
    options: LineOptions,
    content: DialogueContent,
    plan: Option<LinePlan>,
    range: TextRange,
}

/// Canonical `alice.say(args)[...]` content call, plus `alice[...]` shorthand.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContentCall {
    callee: String,
    args: Option<String>,
    options: LineOptions,
    content: DialogueContent,
    plan: Option<LinePlan>,
    range: TextRange,
}

/// Structured dialogue line options parsed from the raw call argument list.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LineOptions {
    id: Option<EntityRef>,
    text_key: Option<EntityRef>,
    source_locale: Option<String>,
}

/// `choice #choice.id { ... }` flow item with option rows.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChoiceBlock {
    id: Option<EntityRef>,
    items: Vec<ChoiceItem>,
    options: Vec<ChoiceOption>,
    plan: Option<ChoicePlan>,
    range: TextRange,
}

/// Choice lifecycle plan attached with `with { ... }` or `with:`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChoicePlan {
    style: BlockStyle,
    items: Vec<ChoicePlanItem>,
    range: TextRange,
}

/// Item inside a choice lifecycle plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ChoicePlanItem {
    Option { name: String, value: Expr },
    Timeout { duration: Expr, body: Vec<Stmt> },
    Cancel { trigger: String, body: Vec<Stmt> },
    OnSelect { pattern: Pattern, body: Vec<Stmt> },
    Raw(String),
}

/// Item inside a choice body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ChoiceItem {
    Let {
        pattern: Pattern,
        expr: Expr,
    },
    If {
        condition: Expr,
        items: Vec<ChoiceItem>,
    },
    For {
        pattern: Pattern,
        source: Expr,
        items: Vec<ChoiceItem>,
    },
    Match {
        expr: Expr,
        arms: Vec<ChoiceMatchArm>,
    },
    Option(Box<ChoiceOption>),
    Raw(String),
}

/// One branch of a `match` item inside a choice body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChoiceMatchArm {
    pattern: Pattern,
    guard: Option<Expr>,
    items: Vec<ChoiceItem>,
}

/// One option in a choice block.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChoiceOption {
    id: Option<EntityRef>,
    id_expr: Option<Expr>,
    label: String,
    label_text_key: Option<EntityRef>,
    value: Option<Expr>,
    enabled: Option<Expr>,
    visible: Option<Expr>,
    order: Option<Expr>,
    hotkey: Option<Expr>,
    ui_fields: Vec<ChoiceUiField>,
    action: ChoiceAction,
    range: TextRange,
}

/// UI state propagated from a choice option to rendering, accessibility, and Agent observation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChoiceUiField {
    name: String,
    value: Expr,
}

/// Action performed by a selected choice option.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ChoiceAction {
    Goto(EntityRef),
    Out(Expr),
    SelectBlock(Vec<Stmt>),
    None,
}

/// Canonical `with { ... }` line plan, plus `with:` indentation sugar.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LinePlan {
    style: BlockStyle,
    label: Option<String>,
    items: Vec<LinePlanItem>,
    range: TextRange,
}

/// Source style used for a parsed block.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BlockStyle {
    Brace,
    Indent,
}

/// Item allowed inside a line plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LinePlanItem {
    Option {
        name: String,
        value: Expr,
    },
    Let {
        pattern: Pattern,
        expr: Expr,
    },
    Out(Expr),
    CancelRule(CancelRuleSyntax),
    TimedCue {
        anchor: Expr,
        body: Expr,
    },
    StartGroup(Vec<LinePlanItem>),
    TogetherGroup(Vec<LinePlanItem>),
    Memo {
        name: String,
        options: Vec<(String, Expr)>,
    },
    Assert {
        debug: bool,
        expr: Expr,
    },
    Expr(Expr),
    Raw(String),
}

/// Parsed cancellation syntax.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CancelRuleSyntax {
    trigger: String,
    action: Vec<Stmt>,
}

/// Hook item syntax.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HookItem {
    visibility: Option<Visibility>,
    id: EntityRef,
    target: String,
    phase: String,
    check: Option<String>,
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
    pub(crate) check: Option<String>,
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

/// Declarative `source` stream declaration.
///
/// Source declarations are syntax-only at this layer. They preserve the source
/// id or function-like name plus parsed policy/event statements so HIR and
/// later semantic passes do not need to reparse the body text.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceItem {
    visibility: Option<Visibility>,
    id: Option<EntityRef>,
    name: Option<String>,
    signature_tail: String,
    body: String,
    body_statements: Vec<Stmt>,
    range: TextRange,
}

impl TextRange {
    /// Builds a half-open byte range.
    pub const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    /// Start byte offset.
    pub const fn start(&self) -> usize {
        self.start
    }

    /// End byte offset.
    pub const fn end(&self) -> usize {
        self.end
    }

    /// Converts to the standard range type.
    pub fn as_range(&self) -> Range<usize> {
        self.start..self.end
    }
}

impl SyntaxTree {
    pub(crate) fn new(
        source: String,
        module: Option<ModuleDecl>,
        uses: Vec<UseItem>,
        items: Vec<Item>,
        wiki_links: Vec<WikiLink>,
    ) -> Self {
        Self {
            source,
            module,
            uses,
            items,
            wiki_links,
        }
    }

    /// Original source text.
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Optional module declaration.
    pub const fn module(&self) -> Option<&ModuleDecl> {
        self.module.as_ref()
    }

    /// Parsed use declarations.
    pub fn uses(&self) -> &[UseItem] {
        &self.uses
    }

    /// Parsed top-level items.
    pub fn items(&self) -> &[Item] {
        &self.items
    }

    /// Wiki links discovered in comments.
    pub fn wiki_links(&self) -> &[WikiLink] {
        &self.wiki_links
    }
}

impl ModuleDecl {
    pub(crate) const fn new(path: String, range: TextRange) -> Self {
        Self { path, range }
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub const fn range(&self) -> &TextRange {
        &self.range
    }
}

impl UseItem {
    pub(crate) const fn new(
        visibility: Option<Visibility>,
        mode: Option<UseMode>,
        tree: String,
        range: TextRange,
    ) -> Self {
        Self {
            visibility,
            mode,
            tree,
            range,
        }
    }

    pub const fn visibility(&self) -> Option<Visibility> {
        self.visibility
    }

    pub const fn mode(&self) -> Option<UseMode> {
        self.mode
    }

    pub fn tree(&self) -> &str {
        &self.tree
    }

    pub const fn range(&self) -> &TextRange {
        &self.range
    }
}

impl EntityRef {
    pub(crate) const fn new(body: String, delimited: bool, range: TextRange) -> Self {
        Self {
            body,
            delimited,
            relative: false,
            range,
        }
    }

    pub(crate) const fn new_relative(body: String, range: TextRange) -> Self {
        Self {
            body,
            delimited: false,
            relative: true,
            range,
        }
    }

    pub fn body(&self) -> &str {
        &self.body
    }

    pub const fn is_delimited(&self) -> bool {
        self.delimited
    }

    pub const fn is_relative(&self) -> bool {
        self.relative
    }

    pub const fn range(&self) -> &TextRange {
        &self.range
    }
}

impl WikiLink {
    pub(crate) const fn new(body: String, range: TextRange) -> Self {
        Self { body, range }
    }

    pub fn body(&self) -> &str {
        &self.body
    }

    pub const fn range(&self) -> &TextRange {
        &self.range
    }
}

impl Attribute {
    pub(crate) const fn new(name: String, args: Option<String>, range: TextRange) -> Self {
        Self { name, args, range }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn args(&self) -> Option<&str> {
        self.args.as_deref()
    }

    pub const fn range(&self) -> &TextRange {
        &self.range
    }
}

impl Flow {
    pub(crate) fn new(init: FlowInit) -> Self {
        Self {
            kind: init.kind,
            visibility: init.visibility,
            id: init.id,
            name: init.name,
            signature_tail: init.signature_tail,
            contracts: init.contracts,
            body: init.body,
            range: init.range,
        }
    }

    pub const fn kind(&self) -> FlowKind {
        self.kind
    }

    pub const fn visibility(&self) -> Option<Visibility> {
        self.visibility
    }

    pub const fn id(&self) -> Option<&EntityRef> {
        self.id.as_ref()
    }

    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    pub fn signature_tail(&self) -> &str {
        &self.signature_tail
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
    pub(crate) const fn new(
        kind: EntityDeclKind,
        visibility: Option<Visibility>,
        id: EntityRef,
        name: Option<String>,
        signature_tail: String,
        body: Option<String>,
        range: TextRange,
    ) -> Self {
        Self {
            kind,
            visibility,
            id,
            name,
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
        visibility: Option<Visibility>,
        name: String,
        ty: TypeRef,
        default: Expr,
    ) -> Self {
        Self {
            visibility,
            name,
            ty,
            default,
        }
    }

    pub const fn visibility(&self) -> Option<Visibility> {
        self.visibility
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
    pub(crate) const fn new(name: String, payload: Option<String>) -> Self {
        Self { name, payload }
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
    pub(crate) const fn new(name: String, ty: TypeRef) -> Self {
        Self { name, ty }
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

impl ScenarioCommand {
    pub(crate) const fn new(name: String, args: Vec<Expr>, range: TextRange) -> Self {
        Self { name, args, range }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn args(&self) -> &[Expr] {
        &self.args
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

impl ChoiceOption {
    pub(crate) const fn new(
        id: Option<EntityRef>,
        label: String,
        action: ChoiceAction,
        range: TextRange,
    ) -> Self {
        Self {
            id,
            id_expr: None,
            label,
            label_text_key: None,
            value: None,
            enabled: None,
            visible: None,
            order: None,
            hotkey: None,
            ui_fields: Vec::new(),
            action,
            range,
        }
    }

    pub(crate) fn with_id_expr(mut self, id_expr: Expr) -> Self {
        self.id_expr = Some(id_expr);
        self
    }

    pub(crate) fn with_enabled(mut self, enabled: Expr) -> Self {
        self.enabled = Some(enabled);
        self
    }

    pub(crate) fn with_label_text_key(mut self, text_key: EntityRef) -> Self {
        self.label_text_key = Some(text_key);
        self
    }

    pub(crate) fn with_value(mut self, value: Expr) -> Self {
        self.value = Some(value);
        self
    }

    pub(crate) fn with_visible(mut self, visible: Expr) -> Self {
        self.visible = Some(visible);
        self
    }

    pub(crate) fn with_order(mut self, order: Expr) -> Self {
        self.order = Some(order);
        self
    }

    pub(crate) fn with_hotkey(mut self, hotkey: Expr) -> Self {
        self.hotkey = Some(hotkey);
        self
    }

    pub(crate) fn with_ui_fields(mut self, ui_fields: Vec<ChoiceUiField>) -> Self {
        self.ui_fields = ui_fields;
        self
    }

    pub const fn id(&self) -> Option<&EntityRef> {
        self.id.as_ref()
    }

    pub const fn id_expr(&self) -> Option<&Expr> {
        self.id_expr.as_ref()
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub const fn label_text_key(&self) -> Option<&EntityRef> {
        self.label_text_key.as_ref()
    }

    pub const fn value(&self) -> Option<&Expr> {
        self.value.as_ref()
    }

    pub const fn condition(&self) -> Option<&Expr> {
        self.enabled.as_ref()
    }

    pub const fn enabled(&self) -> Option<&Expr> {
        self.enabled.as_ref()
    }

    pub const fn visible(&self) -> Option<&Expr> {
        self.visible.as_ref()
    }

    pub const fn order(&self) -> Option<&Expr> {
        self.order.as_ref()
    }

    pub const fn hotkey(&self) -> Option<&Expr> {
        self.hotkey.as_ref()
    }

    pub fn ui_fields(&self) -> &[ChoiceUiField] {
        &self.ui_fields
    }

    pub const fn action(&self) -> &ChoiceAction {
        &self.action
    }

    pub const fn target(&self) -> Option<&EntityRef> {
        match &self.action {
            ChoiceAction::Goto(target) => Some(target),
            _ => None,
        }
    }

    pub const fn range(&self) -> &TextRange {
        &self.range
    }
}

impl ChoiceUiField {
    pub(crate) const fn new(name: String, value: Expr) -> Self {
        Self { name, value }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub const fn value(&self) -> &Expr {
        &self.value
    }
}

impl DialogueContent {
    pub(crate) const fn new(raw: String, tokens: Vec<DialogueToken>, range: TextRange) -> Self {
        Self { raw, tokens, range }
    }

    pub fn raw(&self) -> &str {
        &self.raw
    }

    pub fn tokens(&self) -> &[DialogueToken] {
        &self.tokens
    }

    pub const fn range(&self) -> &TextRange {
        &self.range
    }
}

impl DialogueTag {
    pub(crate) const fn new(name: String, attrs: String) -> Self {
        Self { name, attrs }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn attrs(&self) -> &str {
        &self.attrs
    }
}

impl SpeakerLine {
    pub(crate) const fn new(
        speaker: String,
        args: Option<String>,
        options: LineOptions,
        content: DialogueContent,
        plan: Option<LinePlan>,
        range: TextRange,
    ) -> Self {
        Self {
            speaker,
            args,
            options,
            content,
            plan,
            range,
        }
    }

    pub fn speaker(&self) -> &str {
        &self.speaker
    }

    pub fn args(&self) -> Option<&str> {
        self.args.as_deref()
    }

    pub const fn options(&self) -> &LineOptions {
        &self.options
    }

    pub const fn content(&self) -> &DialogueContent {
        &self.content
    }

    pub const fn plan(&self) -> Option<&LinePlan> {
        self.plan.as_ref()
    }

    pub const fn range(&self) -> &TextRange {
        &self.range
    }
}

impl ContentCall {
    pub(crate) const fn new(
        callee: String,
        args: Option<String>,
        options: LineOptions,
        content: DialogueContent,
        plan: Option<LinePlan>,
        range: TextRange,
    ) -> Self {
        Self {
            callee,
            args,
            options,
            content,
            plan,
            range,
        }
    }

    pub fn callee(&self) -> &str {
        &self.callee
    }

    pub fn args(&self) -> Option<&str> {
        self.args.as_deref()
    }

    pub const fn options(&self) -> &LineOptions {
        &self.options
    }

    pub const fn content(&self) -> &DialogueContent {
        &self.content
    }

    pub const fn plan(&self) -> Option<&LinePlan> {
        self.plan.as_ref()
    }

    pub const fn range(&self) -> &TextRange {
        &self.range
    }
}

impl LineOptions {
    pub(crate) const fn new(
        id: Option<EntityRef>,
        text_key: Option<EntityRef>,
        source_locale: Option<String>,
    ) -> Self {
        Self {
            id,
            text_key,
            source_locale,
        }
    }

    pub const fn id(&self) -> Option<&EntityRef> {
        self.id.as_ref()
    }

    pub const fn text_key(&self) -> Option<&EntityRef> {
        self.text_key.as_ref()
    }

    pub fn source_locale(&self) -> Option<&str> {
        self.source_locale.as_deref()
    }
}

impl ChoiceBlock {
    pub(crate) fn new(
        id: Option<EntityRef>,
        items: Vec<ChoiceItem>,
        plan: Option<ChoicePlan>,
        range: TextRange,
    ) -> Self {
        let options = collect_choice_options(&items);
        Self {
            id,
            items,
            options,
            plan,
            range,
        }
    }

    pub const fn id(&self) -> Option<&EntityRef> {
        self.id.as_ref()
    }

    pub fn options(&self) -> &[ChoiceOption] {
        &self.options
    }

    pub fn items(&self) -> &[ChoiceItem] {
        &self.items
    }

    pub const fn plan(&self) -> Option<&ChoicePlan> {
        self.plan.as_ref()
    }

    pub const fn range(&self) -> &TextRange {
        &self.range
    }
}

impl ChoicePlan {
    pub(crate) const fn new(
        style: BlockStyle,
        items: Vec<ChoicePlanItem>,
        range: TextRange,
    ) -> Self {
        Self {
            style,
            items,
            range,
        }
    }

    pub const fn style(&self) -> BlockStyle {
        self.style
    }

    pub fn items(&self) -> &[ChoicePlanItem] {
        &self.items
    }

    pub const fn range(&self) -> &TextRange {
        &self.range
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

impl LexicalBlock {
    pub(crate) const fn new(body: Vec<FlowItem>, range: TextRange) -> Self {
        Self { body, range }
    }

    pub fn body(&self) -> &[FlowItem] {
        &self.body
    }

    pub const fn range(&self) -> &TextRange {
        &self.range
    }
}

impl ScopeBlock {
    pub(crate) const fn new(name: String, body: Vec<FlowItem>, range: TextRange) -> Self {
        Self { name, body, range }
    }

    pub fn name(&self) -> &str {
        &self.name
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
        name: String,
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

    pub fn name(&self) -> &str {
        &self.name
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

fn collect_choice_options(items: &[ChoiceItem]) -> Vec<ChoiceOption> {
    items
        .iter()
        .flat_map(|item| match item {
            ChoiceItem::Option(option) => vec![option.as_ref().clone()],
            ChoiceItem::If { items, .. } | ChoiceItem::For { items, .. } => {
                collect_choice_options(items)
            }
            ChoiceItem::Match { arms, .. } => arms
                .iter()
                .flat_map(|arm| collect_choice_options(arm.items()))
                .collect(),
            ChoiceItem::Let { .. } | ChoiceItem::Raw(_) => Vec::new(),
        })
        .collect()
}

impl ChoiceMatchArm {
    pub(crate) const fn new(pattern: Pattern, guard: Option<Expr>, items: Vec<ChoiceItem>) -> Self {
        Self {
            pattern,
            guard,
            items,
        }
    }

    pub const fn pattern(&self) -> &Pattern {
        &self.pattern
    }

    pub const fn guard(&self) -> Option<&Expr> {
        self.guard.as_ref()
    }

    pub fn items(&self) -> &[ChoiceItem] {
        &self.items
    }
}

impl LinePlan {
    pub(crate) const fn new(style: BlockStyle, items: Vec<LinePlanItem>, range: TextRange) -> Self {
        Self {
            style,
            label: None,
            items,
            range,
        }
    }

    pub(crate) fn with_label(mut self, label: String) -> Self {
        self.label = Some(label);
        self
    }

    pub const fn style(&self) -> BlockStyle {
        self.style
    }

    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }

    pub fn items(&self) -> &[LinePlanItem] {
        &self.items
    }

    pub const fn range(&self) -> &TextRange {
        &self.range
    }
}

impl CancelRuleSyntax {
    pub(crate) const fn new(trigger: String, action: Vec<Stmt>) -> Self {
        Self { trigger, action }
    }

    pub fn trigger(&self) -> &str {
        &self.trigger
    }

    pub fn action(&self) -> &[Stmt] {
        &self.action
    }
}

impl HookItem {
    pub(crate) fn new(init: HookInit) -> Self {
        Self {
            visibility: init.visibility,
            id: init.id,
            target: init.target,
            phase: init.phase,
            check: init.check,
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

    pub fn check(&self) -> Option<&str> {
        self.check.as_deref()
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

impl SourceItem {
    pub(crate) const fn new(
        visibility: Option<Visibility>,
        id: Option<EntityRef>,
        name: Option<String>,
        signature_tail: String,
        body: String,
        body_statements: Vec<Stmt>,
        range: TextRange,
    ) -> Self {
        Self {
            visibility,
            id,
            name,
            signature_tail,
            body,
            body_statements,
            range,
        }
    }

    pub const fn visibility(&self) -> Option<Visibility> {
        self.visibility
    }

    pub const fn id(&self) -> Option<&EntityRef> {
        self.id.as_ref()
    }

    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
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

    pub const fn range(&self) -> &TextRange {
        &self.range
    }
}

impl RawItem {
    pub(crate) const fn new(head: String, body: Option<String>, range: TextRange) -> Self {
        Self { head, body, range }
    }

    pub fn head(&self) -> &str {
        &self.head
    }

    pub fn body(&self) -> Option<&str> {
        self.body.as_deref()
    }

    pub const fn range(&self) -> &TextRange {
        &self.range
    }
}
