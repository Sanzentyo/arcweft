use crate::expr::Expr;
use crate::types::TypeRef;
use core::ops::Range;

/// Half-open byte range in the original source.
#[derive(Clone, Debug, Eq, PartialEq)]
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
    Hook(HookItem),
    MemoFn(MemoFn),
    Parser(ParserItem),
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
    Effects(Vec<Expr>),
    Modifies(Vec<Expr>),
    Decreases(Expr),
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
    Match(MatchBlock),
    Include(EntityRef),
    AwaitWith(AwaitWith),
    Raw(String),
}

/// Typed `if expr { ... }` flow block.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IfBlock {
    condition: Expr,
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
    body: Vec<FlowItem>,
}

/// Typed Arcweft statement inside a flow body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Stmt {
    Let { pattern: Pattern, expr: Expr },
    Return(Expr),
    Goto(Expr),
    Spawn(Expr),
    Defer(Expr),
    Yield(Expr),
    Expr(Expr),
    Raw(String),
}

/// Pattern syntax used by `let` and line-plan return destructuring.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Pattern {
    Ident(String),
    Discard,
    Tuple(Vec<Pattern>),
    Typed { name: String, ty: TypeRef },
    Raw(String),
}

/// `await expr? with { ... }` surface syntax.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AwaitWith {
    expr: Expr,
    propagates_error: bool,
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

/// `alice(args): ...` speaker line.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SpeakerLine {
    speaker: String,
    args: Option<String>,
    content: DialogueContent,
    plan: Option<LinePlan>,
    range: TextRange,
}

/// `alice.say(args)[...]` or `alice[...]` content call.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContentCall {
    callee: String,
    args: Option<String>,
    content: DialogueContent,
    plan: Option<LinePlan>,
    range: TextRange,
}

/// `@choice` block with option rows.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChoiceBlock {
    id: Option<EntityRef>,
    options: Vec<ChoiceOption>,
    range: TextRange,
}

/// One option in a choice block.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChoiceOption {
    id: Option<EntityRef>,
    label: String,
    condition: Option<Expr>,
    target: EntityRef,
    range: TextRange,
}

/// `with { ... }` or `with:` line plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LinePlan {
    style: BlockStyle,
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
    Option { name: String, value: Expr },
    Let { pattern: Pattern, expr: Expr },
    Return(Expr),
    CancelRule(CancelRuleSyntax),
    TimedCue { anchor: Expr, body: Expr },
    StartGroup(String),
    TogetherGroup(String),
    Memo(String),
    Assert(String),
    Raw(String),
}

/// Parsed cancellation syntax.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CancelRuleSyntax {
    trigger: String,
    action: String,
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
    range: TextRange,
}

/// Memoized function item.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoFn {
    visibility: Option<Visibility>,
    signature: String,
    options: Vec<String>,
    body: String,
    range: TextRange,
}

/// User-defined parser item.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParserItem {
    visibility: Option<Visibility>,
    name: String,
    signature_tail: String,
    body: String,
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
            range,
        }
    }

    pub fn body(&self) -> &str {
        &self.body
    }

    pub const fn is_delimited(&self) -> bool {
        self.delimited
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
    pub(crate) const fn new(pattern: Pattern, body: Vec<FlowItem>) -> Self {
        Self { pattern, body }
    }

    pub const fn pattern(&self) -> &Pattern {
        &self.pattern
    }

    pub fn body(&self) -> &[FlowItem] {
        &self.body
    }
}

impl AwaitWith {
    pub(crate) const fn new(
        expr: Expr,
        propagates_error: bool,
        branches: Vec<AwaitBranch>,
    ) -> Self {
        Self {
            expr,
            propagates_error,
            branches,
        }
    }

    pub const fn expr(&self) -> &Expr {
        &self.expr
    }

    pub const fn propagates_error(&self) -> bool {
        self.propagates_error
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
        condition: Option<Expr>,
        target: EntityRef,
        range: TextRange,
    ) -> Self {
        Self {
            id,
            label,
            condition,
            target,
            range,
        }
    }

    pub const fn id(&self) -> Option<&EntityRef> {
        self.id.as_ref()
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub const fn condition(&self) -> Option<&Expr> {
        self.condition.as_ref()
    }

    pub const fn target(&self) -> &EntityRef {
        &self.target
    }

    pub const fn range(&self) -> &TextRange {
        &self.range
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
        content: DialogueContent,
        plan: Option<LinePlan>,
        range: TextRange,
    ) -> Self {
        Self {
            speaker,
            args,
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
        content: DialogueContent,
        plan: Option<LinePlan>,
        range: TextRange,
    ) -> Self {
        Self {
            callee,
            args,
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

impl ChoiceBlock {
    pub(crate) const fn new(
        id: Option<EntityRef>,
        options: Vec<ChoiceOption>,
        range: TextRange,
    ) -> Self {
        Self { id, options, range }
    }

    pub const fn id(&self) -> Option<&EntityRef> {
        self.id.as_ref()
    }

    pub fn options(&self) -> &[ChoiceOption] {
        &self.options
    }

    pub const fn range(&self) -> &TextRange {
        &self.range
    }
}

impl LinePlan {
    pub(crate) const fn new(style: BlockStyle, items: Vec<LinePlanItem>, range: TextRange) -> Self {
        Self {
            style,
            items,
            range,
        }
    }

    pub const fn style(&self) -> BlockStyle {
        self.style
    }

    pub fn items(&self) -> &[LinePlanItem] {
        &self.items
    }

    pub const fn range(&self) -> &TextRange {
        &self.range
    }
}

impl CancelRuleSyntax {
    pub(crate) const fn new(trigger: String, action: String) -> Self {
        Self { trigger, action }
    }

    pub fn trigger(&self) -> &str {
        &self.trigger
    }

    pub fn action(&self) -> &str {
        &self.action
    }
}

impl HookItem {
    pub(crate) const fn new(
        visibility: Option<Visibility>,
        id: EntityRef,
        target: String,
        phase: String,
        check: Option<String>,
        body: String,
        range: TextRange,
    ) -> Self {
        Self {
            visibility,
            id,
            target,
            phase,
            check,
            body,
            range,
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
        range: TextRange,
    ) -> Self {
        Self {
            visibility,
            signature,
            options,
            body,
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
        range: TextRange,
    ) -> Self {
        Self {
            visibility,
            name,
            signature_tail,
            body,
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
