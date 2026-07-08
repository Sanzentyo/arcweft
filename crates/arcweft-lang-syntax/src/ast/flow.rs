use crate::expr::Expr;
use crate::types::TypeRef;

use super::choice::ChoiceBlock;
use super::common::{DocBlock, TextRange, Visibility};
use super::dialogue::{ContentCall, SpeakerLine};
use super::ids::{EntityRefSyntax, IdRef};
use super::items::{Attribute, RawSyntax};
use super::line_plan::{DeferOutcome, TriggerPattern};
use super::pattern::Pattern;
/// Flow item with typed header and parsed flow body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Flow {
    attrs: Vec<Attribute>,
    doc: Option<DocBlock>,
    visibility: Option<Visibility>,
    id: Option<IdRef>,
    name: Option<String>,
    explicit_name: bool,
    signature_tail: String,
    signature: Option<crate::types::FnSignature>,
    contracts: Vec<ContractClause>,
    body: Vec<FlowItem>,
    range: TextRange,
}

/// Internal initializer for a flow-like item.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FlowInit {
    pub(crate) attrs: Vec<Attribute>,
    pub(crate) doc: Option<DocBlock>,
    pub(crate) visibility: Option<Visibility>,
    pub(crate) id: Option<IdRef>,
    pub(crate) name: Option<String>,
    pub(crate) explicit_name: bool,
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

impl ContractClause {
    /// Explicit contract mode, when the clause supports one.
    pub fn mode(&self) -> Option<&str> {
        match self {
            Self::Requires { mode, .. }
            | Self::Ensures { mode, .. }
            | Self::Invariant { mode, .. } => mode.as_deref(),
            _ => None,
        }
    }

    /// Expression assumed by solver-backed verification.
    ///
    /// A missing mode and the explicit `prove` mode both participate in proof;
    /// runtime/debug/document-only modes and explicit `assume` clauses remain
    /// owned by semantic trust policy instead of silently strengthening SMT.
    pub fn solver_assumption(&self) -> Option<&Expr> {
        match self {
            Self::Requires { mode, expr } if Self::mode_requests_proof(mode.as_deref()) => {
                Some(expr)
            }
            _ => None,
        }
    }

    /// Proof-mode invariant clauses are recognized explicitly but are not yet
    /// lowered as pre/post state pairs by the scalar function verifier.
    pub fn solver_invariant(&self) -> Option<&Expr> {
        match self {
            Self::Invariant { mode, expr } if Self::mode_requests_proof(mode.as_deref()) => {
                Some(expr)
            }
            _ => None,
        }
    }

    /// Postcondition that must be proven by a solver backend.
    pub fn solver_claim(&self) -> Option<&Expr> {
        match self {
            Self::Ensures { mode, expr } if Self::mode_requests_proof(mode.as_deref()) => {
                Some(expr)
            }
            _ => None,
        }
    }

    fn mode_requests_proof(mode: Option<&str>) -> bool {
        matches!(mode, None | Some("prove"))
    }
}

/// Syntax allowed in a `flow` body and in top-level scenario snippets.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FlowItem {
    Stmt(Stmt),
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
    condition: AuthoredExpr,
    body: Vec<FlowItem>,
    else_body: Vec<FlowItem>,
    range: TextRange,
}

/// Typed `if let PAT = expr when guard { ... }` flow block.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IfLetBlock {
    pattern: Pattern,
    expr: AuthoredExpr,
    guard: Option<AuthoredExpr>,
    body: Vec<FlowItem>,
    else_body: Vec<FlowItem>,
    range: TextRange,
}

/// Typed `match expr { ... }` flow block.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MatchBlock {
    expr: AuthoredExpr,
    arms: Vec<MatchArm>,
    range: TextRange,
}

/// One `pattern => flow item` match arm.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MatchArm {
    pattern: Pattern,
    guard: Option<AuthoredExpr>,
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
    source: AuthoredExpr,
    body: Vec<FlowItem>,
    range: TextRange,
}

/// `while condition { ... }` statement loop.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WhileBlock {
    condition: AuthoredExpr,
    body: Vec<FlowItem>,
    range: TextRange,
}

/// `while let PAT = expr when guard { ... }` statement loop.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WhileLetBlock {
    pattern: Pattern,
    expr: AuthoredExpr,
    guard: Option<AuthoredExpr>,
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

/// Exact source replacement range used to insert unsafe lifetime audit metadata.
///
/// The parser creates this only for syntax forms whose editable boundary is
/// known precisely, currently the opening brace of a braced `unsafe lifetime`
/// block.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UnsafeAuditInsertion {
    replacement_range: TextRange,
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
        expr_source: Option<String>,
        expr_range: Option<TextRange>,
    },
    /// `target = expr` mutation statement.
    Assign {
        target: AuthoredExpr,
        expr: AuthoredExpr,
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
    /// `let PAT = receive action(@action.id)` waits for a typed semantic action.
    LetActionReceive {
        pattern: Pattern,
        action: AuthoredExpr,
    },
    Return {
        expr: Expr,
        expr_source: Option<String>,
        expr_range: Option<TextRange>,
    },
    /// `out expr` or `out 'label expr` from a line/cue/content continuation.
    Out {
        label: Option<String>,
        expr: Expr,
    },
    Goto(AuthoredExpr),
    /// `thread name { ... }` / `thread name:` scoped VM child task.
    Thread(ThreadBlock),
    /// `defer { ... }` cleanup block registered on the current runtime scope.
    DeferBlock {
        outcome: DeferOutcome,
        statements: Vec<Stmt>,
    },
    Defer {
        outcome: DeferOutcome,
        expr: AuthoredExpr,
    },
    Yield(AuthoredExpr),
    Signal {
        target: Expr,
        value: Expr,
    },
    /// `'line.key <- expr` stores a scoped handle in the named lifetime registry.
    LifetimeSet {
        target: Expr,
        expr: Expr,
    },
    /// `wait(mark(.name))` or `wait(0.35s)` waits inside a line-local task.
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
        audit_insertion: Option<UnsafeAuditInsertion>,
        body: Vec<Stmt>,
    },
    If {
        condition: AuthoredExpr,
        body: Vec<Stmt>,
        else_body: Vec<Stmt>,
    },
    /// `loop { ... }` inside typed statement bodies.
    Loop {
        body: Vec<Stmt>,
    },
    /// `while expr { ... }` inside typed statement bodies.
    While {
        condition: AuthoredExpr,
        body: Vec<Stmt>,
    },
    /// `while let PAT = EXPR when GUARD { ... }` inside typed statement bodies.
    WhileLet {
        pattern: Pattern,
        expr: AuthoredExpr,
        guard: Option<AuthoredExpr>,
        body: Vec<Stmt>,
    },
    /// `for PAT in EXPR { ... }` inside typed statement bodies.
    For {
        pattern: Pattern,
        source: AuthoredExpr,
        body: Vec<Stmt>,
    },
    Match {
        expr: AuthoredExpr,
        arms: Vec<StmtMatchArm>,
    },
    Close(AuthoredExpr),
    Select(AuthoredExpr),
    /// `break`, `break expr`, or `break 'label expr`.
    Break {
        label: Option<String>,
        expr: Option<Expr>,
    },
    /// `continue` or `continue 'label`.
    Continue {
        label: Option<String>,
    },
    Expr {
        expr: Expr,
        expr_source: Option<String>,
        expr_range: Option<TextRange>,
    },
    Raw(RawSyntax),
}

/// Expression payload whose original authored source slice is known.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthoredExpr {
    expr: Expr,
    source: Option<String>,
    range: Option<TextRange>,
}

/// A scoped VM child task owned by the nearest runtime scope.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ThreadBlock {
    modifiers: Vec<ThreadModifier>,
    name: Option<String>,
    body: Vec<FlowItem>,
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
    Expr(Expr),
}

impl AuthoredExpr {
    pub const fn new(expr: Expr) -> Self {
        Self {
            expr,
            source: None,
            range: None,
        }
    }

    pub fn with_source(expr: Expr, source: String, range: Option<TextRange>) -> Self {
        Self {
            expr,
            source: Some(source),
            range,
        }
    }

    pub const fn expr(&self) -> &Expr {
        &self.expr
    }

    pub fn source(&self) -> Option<&str> {
        self.source.as_deref()
    }

    pub const fn range(&self) -> Option<TextRange> {
        self.range
    }
}

impl From<Expr> for AuthoredExpr {
    fn from(expr: Expr) -> Self {
        Self::new(expr)
    }
}

impl ThreadBlock {
    pub(crate) fn new(
        modifiers: Vec<ThreadModifier>,
        name: Option<String>,
        body: Vec<FlowItem>,
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

    pub fn body(&self) -> &[FlowItem] {
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

impl UnsafeAuditInsertion {
    pub(crate) const fn new(replacement_range: TextRange) -> Self {
        Self { replacement_range }
    }

    pub const fn replacement_range(&self) -> &TextRange {
        &self.replacement_range
    }
}

impl Flow {
    pub(crate) fn new(init: FlowInit) -> Self {
        Self {
            attrs: init.attrs,
            doc: init.doc,
            visibility: init.visibility,
            id: init.id,
            name: init.name,
            explicit_name: init.explicit_name,
            signature_tail: init.signature_tail,
            signature: init.signature,
            contracts: init.contracts,
            body: init.body,
            range: init.range,
        }
    }

    pub fn attrs(&self) -> &[Attribute] {
        &self.attrs
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

    pub const fn has_explicit_name(&self) -> bool {
        self.explicit_name
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

impl IfBlock {
    pub(crate) fn new(
        condition: impl Into<AuthoredExpr>,
        body: Vec<FlowItem>,
        else_body: Vec<FlowItem>,
        range: TextRange,
    ) -> Self {
        Self {
            condition: condition.into(),
            body,
            else_body,
            range,
        }
    }

    pub const fn condition(&self) -> &Expr {
        self.condition.expr()
    }

    pub const fn condition_authored(&self) -> &AuthoredExpr {
        &self.condition
    }

    pub fn body(&self) -> &[FlowItem] {
        &self.body
    }

    pub fn else_body(&self) -> &[FlowItem] {
        &self.else_body
    }

    pub const fn range(&self) -> &TextRange {
        &self.range
    }
}

impl IfLetBlock {
    pub(crate) fn new(
        pattern: Pattern,
        expr: impl Into<AuthoredExpr>,
        guard: Option<AuthoredExpr>,
        body: Vec<FlowItem>,
        else_body: Vec<FlowItem>,
        range: TextRange,
    ) -> Self {
        Self {
            pattern,
            expr: expr.into(),
            guard,
            body,
            else_body,
            range,
        }
    }

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

    pub fn body(&self) -> &[FlowItem] {
        &self.body
    }

    pub fn else_body(&self) -> &[FlowItem] {
        &self.else_body
    }

    pub const fn range(&self) -> &TextRange {
        &self.range
    }
}

impl MatchBlock {
    pub(crate) fn new(
        expr: impl Into<AuthoredExpr>,
        arms: Vec<MatchArm>,
        range: TextRange,
    ) -> Self {
        Self {
            expr: expr.into(),
            arms,
            range,
        }
    }

    pub const fn expr(&self) -> &Expr {
        self.expr.expr()
    }

    pub const fn expr_authored(&self) -> &AuthoredExpr {
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
    pub(crate) fn new(pattern: Pattern, guard: Option<AuthoredExpr>, body: Vec<FlowItem>) -> Self {
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
        match self.guard.as_ref() {
            Some(guard) => Some(guard.expr()),
            None => None,
        }
    }

    pub fn guard_authored(&self) -> Option<&AuthoredExpr> {
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
    pub(crate) fn new(
        pattern: Pattern,
        source: impl Into<AuthoredExpr>,
        body: Vec<FlowItem>,
        range: TextRange,
    ) -> Self {
        Self {
            pattern,
            source: source.into(),
            body,
            range,
        }
    }

    pub const fn pattern(&self) -> &Pattern {
        &self.pattern
    }

    pub const fn source(&self) -> &Expr {
        self.source.expr()
    }

    pub const fn source_authored(&self) -> &AuthoredExpr {
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
    pub(crate) fn new(
        condition: impl Into<AuthoredExpr>,
        body: Vec<FlowItem>,
        range: TextRange,
    ) -> Self {
        Self {
            condition: condition.into(),
            body,
            range,
        }
    }

    pub const fn condition(&self) -> &Expr {
        self.condition.expr()
    }

    pub const fn condition_authored(&self) -> &AuthoredExpr {
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
    pub(crate) fn new(
        pattern: Pattern,
        expr: impl Into<AuthoredExpr>,
        guard: Option<AuthoredExpr>,
        body: Vec<FlowItem>,
        range: TextRange,
    ) -> Self {
        Self {
            pattern,
            expr: expr.into(),
            guard,
            body,
            range,
        }
    }

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
