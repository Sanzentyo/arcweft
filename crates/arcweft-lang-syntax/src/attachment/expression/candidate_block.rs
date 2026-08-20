//! Typed value-block and statement views borrowed from one candidate graph.

use arcweft_source::SourceSpan;

use super::{AttachedCandidateNode, AttachedCandidatePatternProjection};
use crate::assertion::AssertionMode;
use crate::expressions::ExpressionProjection;
use crate::grammar::kinds::{SyntaxKind, SyntaxRole};

mod keyword_statement;

pub use keyword_statement::{AttachedCandidateControlLabel, AttachedCandidateKeywordStatement};

/// Two typed operands retained by `Assignment` and `LifetimeSet` statements.
#[derive(Clone, Copy)]
pub struct AttachedCandidateAssignment<'a> {
    statement: AttachedCandidateStatement<'a>,
    target: AttachedCandidateStatementExpression<'a>,
    value: AttachedCandidateStatementExpression<'a>,
}

impl<'a> AttachedCandidateAssignment<'a> {
    /// Candidate statement that owns these operands.
    pub const fn statement(self) -> AttachedCandidateStatement<'a> {
        self.statement
    }

    /// Exact target expression relation.
    pub const fn target(self) -> AttachedCandidateStatementExpression<'a> {
        self.target
    }

    /// Exact assigned value relation.
    pub const fn value(self) -> AttachedCandidateStatementExpression<'a> {
        self.value
    }
}

/// One required operand retained by Return, Yield, Wait, Close, or ordinary Select.
#[derive(Clone, Copy)]
pub struct AttachedCandidateRequiredOperand<'a> {
    statement: AttachedCandidateStatement<'a>,
    operand: AttachedCandidateStatementExpression<'a>,
    punctuation_recovery: bool,
}

impl<'a> AttachedCandidateRequiredOperand<'a> {
    /// Candidate statement that owns this operand.
    pub const fn statement(self) -> AttachedCandidateStatement<'a> {
        self.statement
    }

    /// Exact authored or parser-owned missing expression relation.
    pub const fn operand(self) -> AttachedCandidateStatementExpression<'a> {
        self.operand
    }

    /// Whether the Wait grammar inserted either required parenthesis.
    pub const fn has_punctuation_recovery(self) -> bool {
        self.punctuation_recovery
    }
}

/// Complete candidate-local assertion payload.
#[derive(Clone)]
pub struct AttachedCandidateAssertion<'a> {
    statement: AttachedCandidateStatement<'a>,
    mode: Option<AssertionMode>,
    conditions: Box<[AttachedCandidateStatementExpression<'a>]>,
    open: AttachedCandidateNode<'a>,
    close: AttachedCandidateNode<'a>,
    has_recovery: bool,
}

impl<'a> AttachedCandidateAssertion<'a> {
    /// Candidate statement that owns the assertion.
    pub const fn statement(&self) -> AttachedCandidateStatement<'a> {
        self.statement
    }

    /// Canonical assertion mode, absent only for parser-owned recovery.
    pub const fn mode(&self) -> Option<AssertionMode> {
        self.mode
    }

    /// Source-ordered assertion conditions.
    pub fn conditions(&self) -> &[AttachedCandidateStatementExpression<'a>] {
        &self.conditions
    }

    /// Exact opening delimiter node.
    pub const fn open_delimiter(&self) -> AttachedCandidateNode<'a> {
        self.open
    }

    /// Exact closing delimiter or zero-width missing-delimiter node.
    pub const fn close_delimiter(&self) -> AttachedCandidateNode<'a> {
        self.close
    }

    /// Whether mode, delimiters, conditions, or explicit recovery are malformed.
    pub const fn has_recovery(&self) -> bool {
        self.has_recovery
    }
}

/// Exact condition form selected for one candidate `if` statement.
#[derive(Clone)]
pub enum AttachedCandidateIfHead<'a> {
    /// Ordinary boolean condition.
    Condition(AttachedCandidateStatementExpression<'a>),
    /// Pattern condition with a binding scope owned by the then branch.
    Let {
        pattern: AttachedCandidatePatternProjection<'a>,
        scrutinee: AttachedCandidateStatementExpression<'a>,
        guard: Option<AttachedCandidateStatementExpression<'a>>,
    },
}

/// Exact branch following an authored candidate `else`.
#[derive(Clone)]
pub enum AttachedCandidateIfElse<'a> {
    /// Braced statement block.
    Block(AttachedCandidateStatementBlock<'a>),
    /// Nested `if` statement retained without flattening its owner.
    If(AttachedCandidateStatement<'a>),
}

/// Complete candidate-local `if` statement relation.
#[derive(Clone)]
pub struct AttachedCandidateIf<'a> {
    statement: AttachedCandidateStatement<'a>,
    head: AttachedCandidateIfHead<'a>,
    then_branch: AttachedCandidateStatementBlock<'a>,
    else_branch: Option<AttachedCandidateIfElse<'a>>,
}

impl<'a> AttachedCandidateIf<'a> {
    /// Candidate statement that owns this conditional relation.
    pub const fn statement(&self) -> AttachedCandidateStatement<'a> {
        self.statement
    }

    /// Exact ordinary-condition or pattern-binding head.
    pub const fn head(&self) -> &AttachedCandidateIfHead<'a> {
        &self.head
    }

    /// Authored then-branch statement block.
    pub const fn then_branch(&self) -> &AttachedCandidateStatementBlock<'a> {
        &self.then_branch
    }

    /// Optional authored block or nested `if` owner.
    pub const fn else_branch(&self) -> Option<&AttachedCandidateIfElse<'a>> {
        self.else_branch.as_ref()
    }
}

/// Exact candidate Match-arm body form.
#[derive(Clone)]
pub enum AttachedCandidateMatchArmBody<'a> {
    /// Authored or recovered value expression.
    Expression(AttachedCandidateStatementExpression<'a>),
    /// Statement-only block.
    Block(AttachedCandidateStatementBlock<'a>),
}

/// One source-ordered candidate Match arm.
#[derive(Clone)]
pub struct AttachedCandidateMatchArmStatement<'a> {
    node: AttachedCandidateNode<'a>,
    ordinal: u32,
    pattern: AttachedCandidatePatternProjection<'a>,
    guard: Option<AttachedCandidateStatementExpression<'a>>,
    body: AttachedCandidateMatchArmBody<'a>,
}

impl<'a> AttachedCandidateMatchArmStatement<'a> {
    /// Candidate-local Match-arm wrapper node.
    pub const fn node(&self) -> AttachedCandidateNode<'a> {
        self.node
    }

    /// Source-ordered arm ordinal.
    pub const fn ordinal(&self) -> u32 {
        self.ordinal
    }

    /// Exact typed pattern owned by this arm.
    pub const fn pattern(&self) -> AttachedCandidatePatternProjection<'a> {
        self.pattern
    }

    /// Optional guard evaluated inside the arm scope.
    pub const fn guard(&self) -> Option<AttachedCandidateStatementExpression<'a>> {
        self.guard
    }

    /// Exact expression or statement-block arm body.
    pub const fn body(&self) -> &AttachedCandidateMatchArmBody<'a> {
        &self.body
    }
}

/// Candidate Match body, including the exact missing-body recovery form.
#[derive(Clone)]
pub enum AttachedCandidateMatchBody<'a> {
    /// Authored block with ordered arms.
    Block {
        node: AttachedCandidateNode<'a>,
        open: AttachedCandidateNode<'a>,
        close: AttachedCandidateNode<'a>,
        arms: Box<[AttachedCandidateMatchArmStatement<'a>]>,
    },
    /// Exact zero-width required-body omission.
    Missing { node: AttachedCandidateNode<'a> },
}

impl<'a> AttachedCandidateMatchBody<'a> {
    /// Source-ordered arms, or an empty slice for a missing body.
    pub fn arms(&self) -> &[AttachedCandidateMatchArmStatement<'a>] {
        match self {
            Self::Block { arms, .. } => arms,
            Self::Missing { .. } => &[],
        }
    }

    /// Whether an authored body retains its closing delimiter.
    pub fn is_closed(&self) -> bool {
        match self {
            Self::Block { close, .. } => !close.source_span().range().is_empty(),
            Self::Missing { .. } => true,
        }
    }

    /// Whether the required Match body is absent.
    pub const fn is_missing(&self) -> bool {
        matches!(self, Self::Missing { .. })
    }
}

/// Complete candidate-local Match statement relation.
#[derive(Clone)]
pub struct AttachedCandidateMatchStatement<'a> {
    statement: AttachedCandidateStatement<'a>,
    scrutinee: AttachedCandidateStatementExpression<'a>,
    body: AttachedCandidateMatchBody<'a>,
}

impl<'a> AttachedCandidateMatchStatement<'a> {
    /// Candidate statement that owns this Match relation.
    pub const fn statement(&self) -> AttachedCandidateStatement<'a> {
        self.statement
    }

    /// Exact scrutinee relation, including typed recovery.
    pub const fn scrutinee(&self) -> AttachedCandidateStatementExpression<'a> {
        self.scrutinee
    }

    /// Authored or missing Match body.
    pub const fn body(&self) -> &AttachedCandidateMatchBody<'a> {
        &self.body
    }
}

/// Typed unsafe-audit identity relation.
#[derive(Clone, Copy)]
pub enum AttachedCandidateUnsafeAuditId<'a> {
    /// Authored or recovered entity-reference expression.
    Reference(AttachedCandidateNode<'a>),
    /// Exact zero-width required-reference omission.
    Missing(AttachedCandidateNode<'a>),
}

impl<'a> AttachedCandidateUnsafeAuditId<'a> {
    /// Candidate-local node that owns the audit identity relation.
    pub const fn node(self) -> AttachedCandidateNode<'a> {
        match self {
            Self::Reference(node) | Self::Missing(node) => node,
        }
    }
}

/// Exact unsafe-lifetime body form.
#[derive(Clone)]
pub enum AttachedCandidateUnsafeBody<'a> {
    /// Authored statement block and parser-classified safety documentation.
    Block(AttachedCandidateStatementBlock<'a>),
    /// Exact zero-width required-body omission.
    Missing(AttachedCandidateNode<'a>),
}

/// Complete candidate-local unsafe-lifetime statement relation.
#[derive(Clone)]
pub struct AttachedCandidateUnsafeLifetime<'a> {
    statement: AttachedCandidateStatement<'a>,
    audit_id: AttachedCandidateUnsafeAuditId<'a>,
    reason: Option<AttachedCandidateStatementExpression<'a>>,
    body: AttachedCandidateUnsafeBody<'a>,
}

impl<'a> AttachedCandidateUnsafeLifetime<'a> {
    /// Candidate statement that owns this unsafe-lifetime relation.
    pub const fn statement(&self) -> AttachedCandidateStatement<'a> {
        self.statement
    }

    /// Typed authored or missing audit identity.
    pub const fn audit_id(&self) -> AttachedCandidateUnsafeAuditId<'a> {
        self.audit_id
    }

    /// Optional authored or recovered reason expression.
    pub const fn reason(&self) -> Option<AttachedCandidateStatementExpression<'a>> {
        self.reason
    }

    /// Authored block or exact missing-body owner.
    pub const fn body(&self) -> &AttachedCandidateUnsafeBody<'a> {
        &self.body
    }
}

/// Authored value tail or the exact parser-owned omission site.
#[derive(Clone, Copy)]
pub enum AttachedCandidateBlockTail<'a> {
    /// Authored or recovered expression tail.
    Expression(AttachedCandidateStatementExpression<'a>),
    /// Zero-width omitted-tail node retained by the value-block grammar.
    Omitted { node: AttachedCandidateNode<'a> },
}

impl<'a> AttachedCandidateBlockTail<'a> {
    /// Candidate-local node that owns this tail relation.
    pub const fn node(self) -> AttachedCandidateNode<'a> {
        match self {
            Self::Expression(expression) => expression.node(),
            Self::Omitted { node } => node,
        }
    }

    /// Exact tail span in the accepted outer revision.
    pub fn source_span(self) -> SourceSpan {
        self.node().source_span()
    }
}

/// One expression relation owned by a candidate statement or block tail.
#[derive(Clone, Copy)]
pub enum AttachedCandidateStatementExpression<'a> {
    /// Authored expression with a recognized semantic family.
    Authored(AttachedCandidateNode<'a>),
    /// Authored slot retained as the ordinary current-grammar Error family.
    Recovered(AttachedCandidateNode<'a>),
    /// Exact zero-width required-expression omission.
    Missing(AttachedCandidateNode<'a>),
}

impl<'a> AttachedCandidateStatementExpression<'a> {
    /// Candidate-local expression node.
    pub const fn node(self) -> AttachedCandidateNode<'a> {
        match self {
            Self::Authored(node) | Self::Recovered(node) | Self::Missing(node) => node,
        }
    }

    /// Exact expression span in the accepted outer revision.
    pub fn source_span(self) -> SourceSpan {
        self.node().source_span()
    }

    /// Whether the grammar retained a required zero-width expression slot.
    pub const fn is_missing(self) -> bool {
        matches!(self, Self::Missing(_))
    }

    /// Whether the expression is authored but semantically recovered.
    pub const fn is_recovered(self) -> bool {
        matches!(self, Self::Recovered(_))
    }

    /// Whether this expression relation contains parser-owned typed recovery.
    pub fn has_recovery(self) -> bool {
        self.is_missing()
            || self
                .node()
                .expression_projection()
                .is_some_and(ExpressionProjection::has_recovery)
    }
}

/// One source-ordered statement inside a candidate block.
#[derive(Clone, Copy)]
pub struct AttachedCandidateStatement<'a> {
    node: AttachedCandidateNode<'a>,
    ordinal: u32,
}

/// Complete candidate-local `let PATTERN = VALUE else { ... }` relation.
#[derive(Clone)]
pub struct AttachedCandidateLetElse<'a> {
    statement: AttachedCandidateStatement<'a>,
    pattern: AttachedCandidatePatternProjection<'a>,
    initializer: AttachedCandidateStatementExpression<'a>,
    else_branch: AttachedCandidateStatementBlock<'a>,
}

impl<'a> AttachedCandidateLetElse<'a> {
    pub const fn statement(&self) -> AttachedCandidateStatement<'a> {
        self.statement
    }

    pub const fn pattern(&self) -> AttachedCandidatePatternProjection<'a> {
        self.pattern
    }

    pub const fn initializer(&self) -> AttachedCandidateStatementExpression<'a> {
        self.initializer
    }

    pub const fn else_branch(&self) -> &AttachedCandidateStatementBlock<'a> {
        &self.else_branch
    }
}

impl<'a> AttachedCandidateStatement<'a> {
    /// Candidate-local statement node.
    pub const fn node(self) -> AttachedCandidateNode<'a> {
        self.node
    }

    /// Source-ordered ordinal within the owning block.
    pub const fn ordinal(self) -> u32 {
        self.ordinal
    }

    /// Exact parser-selected statement family.
    pub fn kind(self) -> SyntaxKind {
        self.node.kind()
    }

    /// Exact statement span in the accepted outer revision.
    pub fn source_span(self) -> SourceSpan {
        self.node.source_span()
    }

    /// One required direct expression relation.
    pub fn required_expression(
        self,
        role: SyntaxRole,
    ) -> Option<AttachedCandidateStatementExpression<'a>> {
        candidate_expression(exact_required_child(self.node, role)?)
    }

    /// One optional direct expression relation.
    ///
    /// `None` means the candidate shape is invalid; `Some(None)` means the
    /// grammar omitted the optional relation.
    #[allow(
        clippy::option_option,
        reason = "outer absence rejects duplicate candidate relations while inner absence is a valid optional grammar slot"
    )]
    fn optional_expression(
        self,
        role: SyntaxRole,
    ) -> Option<Option<AttachedCandidateStatementExpression<'a>>> {
        match exact_optional_child(self.node, role)? {
            Some(node) => Some(Some(candidate_expression(node)?)),
            None => Some(None),
        }
    }

    /// One required direct Pattern root.
    pub fn required_pattern(
        self,
        role: SyntaxRole,
    ) -> Option<AttachedCandidatePatternProjection<'a>> {
        exact_required_child(self.node, role)?.pattern_root()
    }

    /// Complete typed relation for one candidate-local LetElse statement.
    pub fn let_else_view(self) -> Option<AttachedCandidateLetElse<'a>> {
        if self.kind() != SyntaxKind::LetElseStatement {
            return None;
        }
        let pattern = self.required_pattern(SyntaxRole::Pattern)?;
        let initializer = self.required_expression(SyntaxRole::Initializer)?;
        let else_branch = AttachedCandidateStatementBlock::from_node(exact_required_child(
            self.node,
            SyntaxRole::ElseBranch,
        )?)?;
        if !else_branch.safety_documentation().is_empty()
            || self.node.children().any(|child| {
                !matches!(
                    child.role(),
                    SyntaxRole::Pattern | SyntaxRole::Initializer | SyntaxRole::ElseBranch
                )
            })
        {
            return None;
        }
        Some(AttachedCandidateLetElse {
            statement: self,
            pattern,
            initializer,
            else_branch,
        })
    }

    /// Complete `Assignment` or `LifetimeSet` operand relation.
    pub fn assignment_view(self) -> Option<AttachedCandidateAssignment<'a>> {
        if !matches!(
            self.kind(),
            SyntaxKind::AssignmentStatement | SyntaxKind::LifetimeSetStatement
        ) {
            return None;
        }
        let target = self.required_expression(SyntaxRole::Target)?;
        let value = self.required_expression(SyntaxRole::Initializer)?;
        if self
            .node
            .children()
            .any(|child| !matches!(child.role(), SyntaxRole::Target | SyntaxRole::Initializer))
        {
            return None;
        }
        Some(AttachedCandidateAssignment {
            statement: self,
            target,
            value,
        })
    }

    /// Complete required-operand relation for locally accepted unary statements.
    ///
    /// Block-family Select operands remain reserved for the frozen Flow/Thread
    /// authority and are not exposed as ordinary Select statements.
    pub fn required_operand_view(self) -> Option<AttachedCandidateRequiredOperand<'a>> {
        if !matches!(
            self.kind(),
            SyntaxKind::ReturnStatement
                | SyntaxKind::YieldStatement
                | SyntaxKind::WaitStatement
                | SyntaxKind::CloseStatement
                | SyntaxKind::SelectStatement
        ) {
            return None;
        }
        let operand = self.required_expression(SyntaxRole::Operand)?;
        if self.kind() == SyntaxKind::SelectStatement
            && operand
                .node()
                .expression_projection()
                .is_some_and(ExpressionProjection::is_value_block)
        {
            return None;
        }

        let punctuation_recovery = if self.kind() == SyntaxKind::WaitStatement {
            let open = exact_required_child(self.node, SyntaxRole::OpenDelimiter)?;
            let close = exact_required_child(self.node, SyntaxRole::CloseDelimiter)?;
            if open.kind() != SyntaxKind::OpenParenNode
                || close.kind() != SyntaxKind::CloseParenNode
                || self.node.children().any(|child| {
                    !matches!(
                        child.role(),
                        SyntaxRole::OpenDelimiter
                            | SyntaxRole::Operand
                            | SyntaxRole::CloseDelimiter
                    )
                })
            {
                return None;
            }
            open.source_span().range().is_empty() || close.source_span().range().is_empty()
        } else {
            if self
                .node
                .children()
                .any(|child| child.role() != SyntaxRole::Operand)
            {
                return None;
            }
            false
        };

        Some(AttachedCandidateRequiredOperand {
            statement: self,
            operand,
            punctuation_recovery,
        })
    }

    /// Complete assertion relation selected without consulting source text.
    pub fn assertion_view(self) -> Option<AttachedCandidateAssertion<'a>> {
        if self.kind() != SyntaxKind::AssertionStatement {
            return None;
        }
        let mode = self.node.assertion_projection()?.mode();
        let name = exact_required_child(self.node, SyntaxRole::Name)?;
        if !matches!(
            name.kind(),
            SyntaxKind::NameReference | SyntaxKind::MissingName
        ) {
            return None;
        }
        let open = exact_required_child(self.node, SyntaxRole::OpenDelimiter)?;
        let close = exact_required_child(self.node, SyntaxRole::CloseDelimiter)?;
        if open.kind() != SyntaxKind::OpenParenNode || close.kind() != SyntaxKind::CloseParenNode {
            return None;
        }
        let mut conditions = Vec::new();
        for child in self.node.children() {
            if child.role() == SyntaxRole::Condition {
                conditions.push(candidate_expression(child)?);
            }
        }
        let mut next_recovery = 0_u32;
        for child in self.node.children() {
            match child.role() {
                SyntaxRole::Name
                | SyntaxRole::OpenDelimiter
                | SyntaxRole::CloseDelimiter
                | SyntaxRole::Condition => {}
                SyntaxRole::Recovery(ordinal)
                    if ordinal == next_recovery && child.kind() == SyntaxKind::ErrorNode =>
                {
                    next_recovery = next_recovery.checked_add(1)?;
                }
                _ => return None,
            }
        }
        let has_recovery = mode.is_none()
            || open.source_span().range().is_empty()
            || close.source_span().range().is_empty()
            || conditions.is_empty()
            || conditions
                .iter()
                .copied()
                .any(AttachedCandidateStatementExpression::has_recovery)
            || next_recovery != 0;
        Some(AttachedCandidateAssertion {
            statement: self,
            mode,
            conditions: conditions.into_boxed_slice(),
            open,
            close,
            has_recovery,
        })
    }

    /// Complete ordinary or pattern `if` relation.
    pub fn if_view(self) -> Option<AttachedCandidateIf<'a>> {
        if self.kind() != SyntaxKind::IfStatement {
            return None;
        }
        let condition = exact_optional_child(self.node, SyntaxRole::Condition)?;
        let pattern = exact_optional_child(self.node, SyntaxRole::Pattern)?;
        let scrutinee = exact_optional_child(self.node, SyntaxRole::Scrutinee)?;
        let guard = exact_optional_child(self.node, SyntaxRole::Guard)?;
        let head = match (condition, pattern, scrutinee, guard) {
            (Some(condition), None, None, None) => {
                AttachedCandidateIfHead::Condition(candidate_expression(condition)?)
            }
            (None, Some(pattern), Some(scrutinee), guard) => AttachedCandidateIfHead::Let {
                pattern: pattern.pattern_root()?,
                scrutinee: candidate_expression(scrutinee)?,
                guard: match guard {
                    Some(guard) => Some(candidate_expression(guard)?),
                    None => None,
                },
            },
            _ => return None,
        };
        let then_branch = AttachedCandidateStatementBlock::from_node(exact_required_child(
            self.node,
            SyntaxRole::ThenBranch,
        )?)?;
        if !then_branch.safety_documentation().is_empty() {
            return None;
        }
        let else_branch = match exact_optional_child(self.node, SyntaxRole::ElseBranch)? {
            None => None,
            Some(node) if node.kind() == SyntaxKind::Block => {
                let block = AttachedCandidateStatementBlock::from_node(node)?;
                if !block.safety_documentation().is_empty() {
                    return None;
                }
                Some(AttachedCandidateIfElse::Block(block))
            }
            Some(node) if node.kind() == SyntaxKind::IfStatement => {
                Some(AttachedCandidateIfElse::If(Self { node, ordinal: 0 }))
            }
            Some(_) => return None,
        };
        for child in self.node.children() {
            if !matches!(
                child.role(),
                SyntaxRole::Condition
                    | SyntaxRole::Pattern
                    | SyntaxRole::Scrutinee
                    | SyntaxRole::Guard
                    | SyntaxRole::ThenBranch
                    | SyntaxRole::ElseBranch
            ) {
                return None;
            }
        }
        Some(AttachedCandidateIf {
            statement: self,
            head,
            then_branch,
            else_branch,
        })
    }

    /// Complete Match relation with arm wrapper scopes preserved.
    pub fn match_view(self) -> Option<AttachedCandidateMatchStatement<'a>> {
        if self.kind() != SyntaxKind::MatchStatement {
            return None;
        }
        let scrutinee = self.required_expression(SyntaxRole::Scrutinee)?;
        let body_node = exact_required_child(self.node, SyntaxRole::Body)?;
        let body = if body_node.kind() == SyntaxKind::MissingBody {
            if !body_node.source_span().range().is_empty() || body_node.children().next().is_some()
            {
                return None;
            }
            AttachedCandidateMatchBody::Missing { node: body_node }
        } else {
            candidate_match_body(body_node)?
        };
        if self
            .node
            .children()
            .any(|child| !matches!(child.role(), SyntaxRole::Scrutinee | SyntaxRole::Body))
        {
            return None;
        }
        Some(AttachedCandidateMatchStatement {
            statement: self,
            scrutinee,
            body,
        })
    }

    /// Complete unsafe-lifetime relation with typed audit identity and body.
    pub fn unsafe_lifetime_view(self) -> Option<AttachedCandidateUnsafeLifetime<'a>> {
        if self.kind() != SyntaxKind::UnsafeLifetimeStatement {
            return None;
        }
        let audit_node = exact_required_child(self.node, SyntaxRole::Reference(0))?;
        let audit_id = match (audit_node.kind(), audit_node.expression_projection()) {
            (
                SyntaxKind::EntityReferenceExpression,
                Some(ExpressionProjection::EntityReference(_)),
            ) => AttachedCandidateUnsafeAuditId::Reference(audit_node),
            (SyntaxKind::MissingExpression, _) if audit_node.source_span().range().is_empty() => {
                AttachedCandidateUnsafeAuditId::Missing(audit_node)
            }
            _ => return None,
        };
        let reason = self.optional_expression(SyntaxRole::Initializer)?;
        let body_node = exact_required_child(self.node, SyntaxRole::Body)?;
        let body = match body_node.kind() {
            SyntaxKind::Block => AttachedCandidateUnsafeBody::Block(
                AttachedCandidateStatementBlock::from_node(body_node)?,
            ),
            SyntaxKind::MissingBody
                if body_node.source_span().range().is_empty()
                    && body_node.children().next().is_none() =>
            {
                AttachedCandidateUnsafeBody::Missing(body_node)
            }
            _ => return None,
        };
        if self.node.children().any(|child| {
            !matches!(
                child.role(),
                SyntaxRole::Reference(0) | SyntaxRole::Initializer | SyntaxRole::Body
            )
        }) {
            return None;
        }
        Some(AttachedCandidateUnsafeLifetime {
            statement: self,
            audit_id,
            reason,
            body,
        })
    }
}

/// Statement-only block used by conditional and unsafe-audit statements.
#[derive(Clone)]
pub struct AttachedCandidateStatementBlock<'a> {
    node: AttachedCandidateNode<'a>,
    statements: Box<[AttachedCandidateStatement<'a>]>,
    open: AttachedCandidateNode<'a>,
    close: AttachedCandidateNode<'a>,
    safety_documentation: Box<[AttachedCandidateNode<'a>]>,
}

impl<'a> AttachedCandidateStatementBlock<'a> {
    fn from_node(node: AttachedCandidateNode<'a>) -> Option<Self> {
        let parts = candidate_block_parts(node)?;
        parts.tail.is_none().then_some(Self {
            node,
            statements: parts.statements,
            open: parts.open,
            close: parts.close,
            safety_documentation: parts.safety_documentation,
        })
    }

    /// Candidate-local Block node.
    pub const fn node(&self) -> AttachedCandidateNode<'a> {
        self.node
    }

    /// Source-ordered statements.
    pub fn statements(&self) -> &[AttachedCandidateStatement<'a>] {
        &self.statements
    }

    /// Exact opening delimiter node.
    pub const fn open_delimiter(&self) -> AttachedCandidateNode<'a> {
        self.open
    }

    /// Exact closing delimiter or zero-width missing-delimiter node.
    pub const fn close_delimiter(&self) -> AttachedCandidateNode<'a> {
        self.close
    }

    /// Parser-recognized `SAFETY` documentation in authored order.
    pub fn safety_documentation(&self) -> &[AttachedCandidateNode<'a>] {
        &self.safety_documentation
    }

    /// Whether the closing delimiter was authored.
    pub fn is_closed(&self) -> bool {
        !self.close.source_span().range().is_empty()
    }
}

/// Value block owned by a candidate Block, computation block, or named block.
#[derive(Clone)]
pub struct AttachedCandidateValueBlock<'a> {
    expression: AttachedCandidateNode<'a>,
    block: AttachedCandidateNode<'a>,
    statements: Box<[AttachedCandidateStatement<'a>]>,
    tail: AttachedCandidateBlockTail<'a>,
    open: AttachedCandidateNode<'a>,
    close: AttachedCandidateNode<'a>,
}

impl<'a> AttachedCandidateValueBlock<'a> {
    /// Candidate expression that owns this lexical value block.
    pub const fn expression(&self) -> AttachedCandidateNode<'a> {
        self.expression
    }

    /// Candidate-local structural Block node.
    pub const fn block(&self) -> AttachedCandidateNode<'a> {
        self.block
    }

    /// Source-ordered statements.
    pub fn statements(&self) -> &[AttachedCandidateStatement<'a>] {
        &self.statements
    }

    /// Authored expression tail or exact omitted-tail marker.
    pub const fn tail(&self) -> AttachedCandidateBlockTail<'a> {
        self.tail
    }

    /// Exact opening delimiter node.
    pub const fn open_delimiter(&self) -> AttachedCandidateNode<'a> {
        self.open
    }

    /// Exact closing delimiter or zero-width missing-delimiter node.
    pub const fn close_delimiter(&self) -> AttachedCandidateNode<'a> {
        self.close
    }

    /// Whether the closing delimiter was authored.
    pub fn is_closed(&self) -> bool {
        !self.close.source_span().range().is_empty()
    }
}

impl<'a> AttachedCandidateNode<'a> {
    /// Typed candidate value-block view selected from exact graph relations.
    pub fn value_block_view(self) -> Option<AttachedCandidateValueBlock<'a>> {
        match self.expression_projection()? {
            ExpressionProjection::Block
            | ExpressionProjection::ComputationBlock(_)
            | ExpressionProjection::NamedBlock(_) => {}
            _ => return None,
        }
        let block = exact_required_child(self, SyntaxRole::Body)?;
        let parts = candidate_block_parts(block)?;
        let tail = parts.tail?;
        Some(AttachedCandidateValueBlock {
            expression: self,
            block,
            statements: parts.statements,
            tail,
            open: parts.open,
            close: parts.close,
        })
    }
}

struct CandidateBlockParts<'a> {
    statements: Box<[AttachedCandidateStatement<'a>]>,
    tail: Option<AttachedCandidateBlockTail<'a>>,
    open: AttachedCandidateNode<'a>,
    close: AttachedCandidateNode<'a>,
    safety_documentation: Box<[AttachedCandidateNode<'a>]>,
}

fn candidate_block_parts(node: AttachedCandidateNode<'_>) -> Option<CandidateBlockParts<'_>> {
    if node.kind() != SyntaxKind::Block {
        return None;
    }
    let open = exact_required_child(node, SyntaxRole::OpenDelimiter)?;
    let close = exact_required_child(node, SyntaxRole::CloseDelimiter)?;
    if open.kind() != SyntaxKind::OpenBraceNode || close.kind() != SyntaxKind::CloseBraceNode {
        return None;
    }

    let mut statements = Vec::new();
    let mut safety_documentation = Vec::new();
    let mut tail = None;
    for child in node.children() {
        match child.role() {
            SyntaxRole::OpenDelimiter | SyntaxRole::CloseDelimiter => {}
            SyntaxRole::Statement(ordinal) => {
                let expected = u32::try_from(statements.len()).ok()?;
                if ordinal != expected || !child.kind().is_statement() {
                    return None;
                }
                statements.push(AttachedCandidateStatement {
                    node: child,
                    ordinal,
                });
            }
            SyntaxRole::Tail => {
                if tail.replace(candidate_block_tail(child)?).is_some() {
                    return None;
                }
            }
            SyntaxRole::Documentation if child.kind() == SyntaxKind::DocBlock => {
                safety_documentation.push(child);
            }
            _ => return None,
        }
    }
    Some(CandidateBlockParts {
        statements: statements.into_boxed_slice(),
        tail,
        open,
        close,
        safety_documentation: safety_documentation.into_boxed_slice(),
    })
}

fn candidate_match_body(node: AttachedCandidateNode<'_>) -> Option<AttachedCandidateMatchBody<'_>> {
    if node.kind() != SyntaxKind::Block {
        return None;
    }
    let open = exact_required_child(node, SyntaxRole::OpenDelimiter)?;
    let close = exact_required_child(node, SyntaxRole::CloseDelimiter)?;
    if open.kind() != SyntaxKind::OpenBraceNode || close.kind() != SyntaxKind::CloseBraceNode {
        return None;
    }
    if node.children().any(|child| {
        !matches!(
            child.role(),
            SyntaxRole::OpenDelimiter | SyntaxRole::CloseDelimiter | SyntaxRole::MatchArm(_)
        )
    }) {
        return None;
    }
    let mut arms = Vec::new();
    for child in node
        .children()
        .filter(|child| matches!(child.role(), SyntaxRole::MatchArm(_)))
    {
        let ordinal = u32::try_from(arms.len()).ok()?;
        if child.kind() != SyntaxKind::MatchArm || child.role() != SyntaxRole::MatchArm(ordinal) {
            return None;
        }
        let pattern = exact_required_child(child, SyntaxRole::Pattern)?.pattern_root()?;
        let guard = match exact_optional_child(child, SyntaxRole::Guard)? {
            Some(guard) => Some(candidate_expression(guard)?),
            None => None,
        };
        let body_node = exact_required_child(child, SyntaxRole::Body)?;
        let body = if body_node.kind() == SyntaxKind::Block {
            let block = AttachedCandidateStatementBlock::from_node(body_node)?;
            if !block.safety_documentation().is_empty() {
                return None;
            }
            AttachedCandidateMatchArmBody::Block(block)
        } else {
            AttachedCandidateMatchArmBody::Expression(candidate_expression(body_node)?)
        };
        if child.children().any(|nested| {
            !matches!(
                nested.role(),
                SyntaxRole::Pattern | SyntaxRole::Guard | SyntaxRole::Body
            )
        }) {
            return None;
        }
        arms.push(AttachedCandidateMatchArmStatement {
            node: child,
            ordinal,
            pattern,
            guard,
            body,
        });
    }
    Some(AttachedCandidateMatchBody::Block {
        node,
        open,
        close,
        arms: arms.into_boxed_slice(),
    })
}

fn candidate_block_tail(node: AttachedCandidateNode<'_>) -> Option<AttachedCandidateBlockTail<'_>> {
    if node.kind() == SyntaxKind::OmittedBlockTail {
        return node
            .source_span()
            .range()
            .is_empty()
            .then_some(AttachedCandidateBlockTail::Omitted { node });
    }
    candidate_expression(node).map(AttachedCandidateBlockTail::Expression)
}

fn candidate_expression(
    node: AttachedCandidateNode<'_>,
) -> Option<AttachedCandidateStatementExpression<'_>> {
    match (node.kind(), node.expression_projection()) {
        (SyntaxKind::MissingExpression, _) => {
            Some(AttachedCandidateStatementExpression::Missing(node))
        }
        (kind, Some(ExpressionProjection::Error)) if kind != SyntaxKind::MissingExpression => {
            Some(AttachedCandidateStatementExpression::Recovered(node))
        }
        (kind, Some(_)) if kind != SyntaxKind::MissingExpression => {
            Some(AttachedCandidateStatementExpression::Authored(node))
        }
        _ => None,
    }
}

fn exact_required_child(
    owner: AttachedCandidateNode<'_>,
    role: SyntaxRole,
) -> Option<AttachedCandidateNode<'_>> {
    exact_optional_child(owner, role)?
}

#[allow(
    clippy::option_option,
    reason = "outer absence rejects duplicate candidate relations while inner absence is a valid optional grammar slot"
)]
fn exact_optional_child(
    owner: AttachedCandidateNode<'_>,
    role: SyntaxRole,
) -> Option<Option<AttachedCandidateNode<'_>>> {
    let mut matches = owner.children().filter(|child| child.role() == role);
    let first = matches.next();
    matches.next().is_none().then_some(first)
}
