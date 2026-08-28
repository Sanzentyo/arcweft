//! Final semantic statement records owned by the qualified HIR arena.
//!
//! Statement payloads retain semantic values and qualified child IDs only.
//! Revision-bound source components, including `UnsafeAuditInsertion`, belong
//! to the HIR source index rather than these records.

mod child_edges;
mod thread;

pub use child_edges::{
    HirStatementBodyProjection, HirStatementBodyProjectionError, HirStatementBodyRole,
    HirStatementChild, HirStatementChildEdge, HirStatementChildEdgeError, HirStatementChildRole,
};

pub(crate) use self::thread::HirThreadStmtInvariantError;
pub(crate) use self::thread::reject_duplicate_ids;
pub use self::thread::{
    HirConditionalElseBranch, HirContextualStmtBody, HirForStmt, HirIfLetStmt, HirIfStmt,
    HirIncludeStmt, HirMatchStmt, HirScopeStmt, HirSelectBindingLocal, HirSelectBranch,
    HirSelectBranchHead, HirSelectStmt, HirSourceLocaleIssue, HirSourceLocaleStmt,
    HirSourceLocaleValue, HirStmtMatchArm, HirStmtMatchArmBody, HirThreadStmtBodyRole,
    HirThreadStmtChildRole, HirThreadStmtRecoveryIssue, HirWhileLetStmt, HirWhileStmt,
};

use arcweft_lang_syntax::assertion::AssertionMode;
use arcweft_lang_syntax::ast::line_plan::DeferOutcome;
use thiserror::Error;

use crate::body_edges::HirBodyChildEdge;
use crate::identity::{ExprId, HirModuleId, LocalId, PatternId, ScopeId, StmtId, TypeId};
use crate::leaf::{HirIdRefIssue, HirIdRefValue, HirName, HirNameInvariantError};

/// One immutable statement-arena record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirStmt {
    scope: ScopeId,
    kind: HirStmtKind,
    state: HirStmtPoisonState,
}

impl HirStmt {
    #[cfg(test)]
    pub(crate) fn try_new(
        scope: ScopeId,
        kind: HirStmtKind,
    ) -> Result<Self, HirStmtInvariantError> {
        let state = if matches!(&kind, HirStmtKind::Error) {
            HirStmtPoisonState::Poisoned(HirStmtRecoveryIssue::UnclassifiedSyntax)
        } else {
            HirStmtPoisonState::Clean
        };
        Self::try_new_with_state(scope, kind, state)
    }

    pub(crate) fn try_new_with_state(
        scope: ScopeId,
        kind: HirStmtKind,
        state: HirStmtPoisonState,
    ) -> Result<Self, HirStmtInvariantError> {
        kind.validate_module(scope.module())?;
        if !state_matches_kind(&kind, &state) {
            return Err(HirStmtInvariantError::InvalidPoisonState);
        }
        Ok(Self { scope, kind, state })
    }

    /// Returns the lexical scope in which this statement executes.
    pub const fn scope(&self) -> ScopeId {
        self.scope
    }

    /// Returns the closed semantic statement payload.
    pub const fn kind(&self) -> &HirStmtKind {
        &self.kind
    }

    /// Returns the typed recovery state retained by this statement family.
    pub const fn state(&self) -> &HirStmtPoisonState {
        &self.state
    }

    pub const fn is_poisoned(&self) -> bool {
        self.state.is_poisoned()
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "one exhaustive validator keeps every statement family paired with its exact recovery-state contract"
)]
fn state_matches_kind(kind: &HirStmtKind, state: &HirStmtPoisonState) -> bool {
    if matches!(kind, HirStmtKind::Error) {
        return matches!(
            state,
            HirStmtPoisonState::Poisoned(HirStmtRecoveryIssue::UnclassifiedSyntax)
        );
    }
    if matches!(
        state,
        HirStmtPoisonState::Poisoned(HirStmtRecoveryIssue::UnclassifiedSyntax)
    ) {
        return false;
    }

    if let HirStmtKind::Assertion { mode, conditions } = kind {
        return matches!(
            (mode, conditions.is_empty(), state),
            (
                HirAssertionMode::Recovered,
                _,
                HirStmtPoisonState::Poisoned(HirStmtRecoveryIssue::InvalidAssertionMode),
            ) | (
                HirAssertionMode::Resolved(_),
                true,
                HirStmtPoisonState::Poisoned(HirStmtRecoveryIssue::MissingAssertionCondition),
            ) | (
                HirAssertionMode::Resolved(_),
                false,
                HirStmtPoisonState::Clean
                    | HirStmtPoisonState::Poisoned(
                        HirStmtRecoveryIssue::RecoveredChild {
                            role: HirStmtChildRole::Condition,
                        } | HirStmtRecoveryIssue::MalformedAssertion,
                    ),
            )
        );
    }

    if matches!(
        state,
        HirStmtPoisonState::Poisoned(
            HirStmtRecoveryIssue::InvalidAssertionMode
                | HirStmtRecoveryIssue::MissingAssertionCondition
                | HirStmtRecoveryIssue::MalformedAssertion
        )
    ) {
        return false;
    }

    if let HirStmtPoisonState::Poisoned(HirStmtRecoveryIssue::Thread(issue)) = state {
        return thread_recovery_matches_kind(kind, *issue);
    }

    if matches!(
        state,
        HirStmtPoisonState::Poisoned(HirStmtRecoveryIssue::MalformedWait)
    ) {
        return matches!(kind, HirStmtKind::Wait { .. });
    }

    match kind {
        HirStmtKind::Out { label, .. } | HirStmtKind::Break { label, .. } => {
            return matches!(state, HirStmtPoisonState::Clean)
                || matches!(
                    state,
                    HirStmtPoisonState::Poisoned(HirStmtRecoveryIssue::RecoveredChild {
                        role: HirStmtChildRole::Initializer,
                    })
                )
                || label.is_none()
                    && matches!(
                        state,
                        HirStmtPoisonState::Poisoned(HirStmtRecoveryIssue::InvalidControlLabel(_))
                    );
        }
        HirStmtKind::Goto { .. } => {
            return matches!(
                state,
                HirStmtPoisonState::Clean
                    | HirStmtPoisonState::Poisoned(HirStmtRecoveryIssue::RecoveredChild {
                        role: HirStmtChildRole::Target,
                    })
            );
        }
        HirStmtKind::Defer { .. } => {
            return matches!(
                state,
                HirStmtPoisonState::Clean
                    | HirStmtPoisonState::Poisoned(HirStmtRecoveryIssue::RecoveredChild {
                        role: HirStmtChildRole::Initializer,
                    })
            );
        }
        HirStmtKind::Signal { .. } => {
            return matches!(
                state,
                HirStmtPoisonState::Clean
                    | HirStmtPoisonState::Poisoned(HirStmtRecoveryIssue::RecoveredChild {
                        role: HirStmtChildRole::Target | HirStmtChildRole::Initializer,
                    })
            );
        }
        HirStmtKind::Continue { label } => {
            return matches!(state, HirStmtPoisonState::Clean)
                || matches!(
                    state,
                    HirStmtPoisonState::Poisoned(HirStmtRecoveryIssue::MalformedContinue)
                )
                || label.is_none()
                    && matches!(
                        state,
                        HirStmtPoisonState::Poisoned(HirStmtRecoveryIssue::InvalidControlLabel(_))
                    );
        }
        _ => {}
    }

    let HirStmtKind::UnsafeLifetime { audit, body } = kind else {
        return !matches!(
            state,
            HirStmtPoisonState::Poisoned(
                HirStmtRecoveryIssue::InvalidAuditId(_)
                    | HirStmtRecoveryIssue::MissingBody
                    | HirStmtRecoveryIssue::UnclosedBody
                    | HirStmtRecoveryIssue::InvalidControlLabel(_)
                    | HirStmtRecoveryIssue::MalformedContinue
            )
        );
    };

    match audit.id().recovery_issue() {
        Some(issue) => {
            return matches!(
                state,
                HirStmtPoisonState::Poisoned(HirStmtRecoveryIssue::InvalidAuditId(actual))
                    if *actual == issue
            );
        }
        None if matches!(
            state,
            HirStmtPoisonState::Poisoned(HirStmtRecoveryIssue::InvalidAuditId(_))
        ) =>
        {
            return false;
        }
        None => {}
    }

    match body {
        HirUnsafeLifetimeBody::Missing => !matches!(state, HirStmtPoisonState::Clean),
        HirUnsafeLifetimeBody::Block { .. } => !matches!(
            state,
            HirStmtPoisonState::Poisoned(HirStmtRecoveryIssue::MissingBody)
        ),
    }
}

fn thread_recovery_matches_kind(kind: &HirStmtKind, issue: HirThreadStmtRecoveryIssue) -> bool {
    use HirThreadStmtBodyRole as Body;
    use HirThreadStmtChildRole as Child;
    use HirThreadStmtRecoveryIssue as Issue;

    match issue {
        Issue::RecoveredChild { role } => match role {
            Child::Condition => matches!(kind, HirStmtKind::If(_) | HirStmtKind::While(_)),
            Child::Pattern | Child::Scrutinee | Child::Guard => matches!(
                kind,
                HirStmtKind::IfLet(_)
                    | HirStmtKind::Match(_)
                    | HirStmtKind::WhileLet(_)
                    | HirStmtKind::For(_)
            ),
            Child::Source | Child::Iterator | Child::NextValue => {
                matches!(kind, HirStmtKind::For(_))
            }
            Child::SelectBinding { .. }
            | Child::SelectSource { .. }
            | Child::SelectPattern { .. }
            | Child::SelectBranchStatement { .. } => {
                matches!(kind, HirStmtKind::Select(_))
            }
            Child::Locale => matches!(kind, HirStmtKind::SourceLocale(_)),
            Child::IncludeTarget => matches!(kind, HirStmtKind::Include(_)),
        },
        Issue::MissingBody { role } | Issue::UnclosedBody { role } => match role {
            Body::Then | Body::Else => matches!(kind, HirStmtKind::If(_) | HirStmtKind::IfLet(_)),
            Body::Match | Body::MatchArm { .. } => matches!(kind, HirStmtKind::Match(_)),
            Body::While => matches!(kind, HirStmtKind::While(_)),
            Body::WhileLet => matches!(kind, HirStmtKind::WhileLet(_)),
            Body::For => matches!(kind, HirStmtKind::For(_)),
            Body::Select | Body::SelectBranch { .. } => matches!(kind, HirStmtKind::Select(_)),
            Body::SourceLocale => matches!(kind, HirStmtKind::SourceLocale(_)),
            Body::Scope => matches!(kind, HirStmtKind::Scope(_)),
        },
        Issue::EmptyMatch => matches!(kind, HirStmtKind::Match(_)),
        Issue::EmptySelect | Issue::RecoveredSelectBranch { .. } => {
            matches!(kind, HirStmtKind::Select(_))
        }
        Issue::InvalidSourceLocale(_) => matches!(kind, HirStmtKind::SourceLocale(_)),
        Issue::InvalidScopeName(_) => matches!(kind, HirStmtKind::Scope(_)),
        Issue::InvalidIncludeTarget(_) => matches!(kind, HirStmtKind::Include(_)),
    }
}

impl crate::arena::HirArenaPayload for HirStmt {
    fn is_poisoned(&self) -> bool {
        self.is_poisoned()
    }
}

/// Executability state of one recognized statement family.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirStmtPoisonState {
    Clean,
    Poisoned(HirStmtRecoveryIssue),
}

impl HirStmtPoisonState {
    pub const fn is_poisoned(&self) -> bool {
        matches!(self, Self::Poisoned(_))
    }
}

/// Deterministic semantic child role used by statement recovery.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirStmtChildRole {
    Pattern,
    Initializer,
    Expression,
    Target,
    Condition,
    Scrutinee,
    Guard,
    Reason,
    ThenBranch,
    ElseBranch,
    BodyStatement { ordinal: u32 },
    MatchArmPattern { arm: u32 },
    MatchArmGuard { arm: u32 },
    MatchArmBody { arm: u32 },
    MatchArmBodyStatement { arm: u32, statement: u32 },
}

/// Primary typed issue retained by a known statement family.
#[derive(Clone, Copy, Debug, Eq, Error, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirStmtRecoveryIssue {
    #[error("statement child {role:?} contains recovery")]
    RecoveredChild { role: HirStmtChildRole },
    #[error("unsafe-audit identity is invalid")]
    InvalidAuditId(HirIdRefIssue),
    #[error("unsafe lifetime statement is missing its required body")]
    MissingBody,
    #[error("unsafe lifetime statement body is missing its closing delimiter")]
    UnclosedBody,
    #[error(transparent)]
    Thread(HirThreadStmtRecoveryIssue),
    #[error("assertion statement has no canonical mode")]
    InvalidAssertionMode,
    #[error("assertion statement requires at least one condition")]
    MissingAssertionCondition,
    #[error("assertion statement punctuation or recovery syntax is malformed")]
    MalformedAssertion,
    #[error("Wait statement punctuation is malformed")]
    MalformedWait,
    #[error("statement control label is not one valid name")]
    InvalidControlLabel(HirNameInvariantError),
    #[error("Continue statement has an unexpected value or suffix")]
    MalformedContinue,
    #[error("statement syntax was not classified into a known family")]
    UnclassifiedSyntax,
}

/// Semantic statement context retained while a value-block body is lowered
/// and independently re-derived while its source graph is frozen.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum HirStatementContext {
    Ordinary,
    Predicate,
    Proof,
    Thread,
}

impl HirStatementContext {
    pub(crate) const fn let_binding_policy(self) -> crate::scope::HirPatternBindingPolicy {
        match self {
            Self::Ordinary | Self::Thread => crate::scope::HirPatternBindingPolicy::LetBinding,
            Self::Predicate => crate::scope::HirPatternBindingPolicy::PredicateLet,
            Self::Proof => crate::scope::HirPatternBindingPolicy::ProofLet,
        }
    }

    pub(crate) const fn let_else_binding_policy(self) -> crate::scope::HirPatternBindingPolicy {
        match self {
            Self::Ordinary | Self::Thread => crate::scope::HirPatternBindingPolicy::LetElseBinding,
            Self::Predicate => crate::scope::HirPatternBindingPolicy::PredicateLetElse,
            Self::Proof => crate::scope::HirPatternBindingPolicy::ProofLetElse,
        }
    }
}

/// Exact base statement inventory plus the locally accepted dedicated if-let.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirStmtKind {
    Assertion {
        mode: HirAssertionMode,
        conditions: Box<[ExprId]>,
    },
    Let {
        pattern: PatternId,
        annotation: Option<TypeId>,
        initializer: ExprId,
        locals: Box<[LocalId]>,
    },
    Assign {
        target: ExprId,
        value: ExprId,
    },
    LetElse {
        pattern: PatternId,
        annotation: Option<TypeId>,
        initializer: ExprId,
        else_scope: ScopeId,
        else_body: Box<[StmtId]>,
        locals: Box<[LocalId]>,
    },
    LetChoice {
        pattern: PatternId,
        choice: ExprId,
        locals: Box<[LocalId]>,
    },
    LetScope {
        pattern: PatternId,
        scope_expr: ExprId,
        locals: Box<[LocalId]>,
    },
    LetActionReceive {
        pattern: PatternId,
        action: ExprId,
        locals: Box<[LocalId]>,
    },
    Return {
        value: ExprId,
    },
    Out {
        label: Option<HirName>,
        value: ExprId,
    },
    Goto {
        target: ExprId,
    },
    DeferBlock {
        outcome: DeferOutcome,
        scope: ScopeId,
        body: Box<[StmtId]>,
    },
    Defer {
        outcome: DeferOutcome,
        expression: ExprId,
    },
    Yield {
        expression: ExprId,
    },
    Signal {
        target: ExprId,
        value: ExprId,
    },
    LifetimeSet {
        target: ExprId,
        value: ExprId,
    },
    Wait {
        target: ExprId,
    },
    On {
        trigger: HirTriggerPattern,
        scope: ScopeId,
        body: Box<[StmtId]>,
    },
    UnsafeLifetime {
        audit: HirUnsafeAudit,
        body: HirUnsafeLifetimeBody,
    },
    Choice {
        choice: ExprId,
    },
    If(HirIfStmt),
    IfLet(HirIfLetStmt),
    Match(HirMatchStmt),
    While(HirWhileStmt),
    WhileLet(HirWhileLetStmt),
    For(HirForStmt),
    Close {
        target: ExprId,
    },
    Select(HirSelectStmt),
    SourceLocale(HirSourceLocaleStmt),
    Scope(HirScopeStmt),
    Include(HirIncludeStmt),
    Break {
        label: Option<HirName>,
        value: Option<ExprId>,
    },
    Continue {
        label: Option<HirName>,
    },
    Expression {
        expression: ExprId,
    },
    ProofCall {
        call: ExprId,
    },
    Error,
}

/// Explicit statement algebra used by evaluation consumers. The plan itself
/// is closed over all semantic statement families; consumers never need to
/// rematch [`HirStmtKind`] to recover Return/Out/control or binding meaning.
#[derive(Debug, Eq, PartialEq)]
pub enum HirStmtEvaluationPlan<'stmt> {
    Assertion {
        mode: HirAssertionMode,
        conditions: &'stmt [ExprId],
    },
    Binding {
        kind: HirStmtBindingPlanKind,
        pattern: PatternId,
        annotation: Option<TypeId>,
        input: ExprId,
        locals: &'stmt [LocalId],
    },
    OrderedPair {
        kind: HirStmtOrderedPairPlanKind,
        first: ExprId,
        second: ExprId,
    },
    Value {
        kind: HirStmtValuePlanKind,
        expression: Option<ExprId>,
        label: Option<&'stmt HirName>,
        outcome: Option<DeferOutcome>,
    },
    DeferredBody {
        kind: HirStmtDeferredBodyPlanKind,
        scope: ScopeId,
        body: &'stmt [StmtId],
        outcome: DeferOutcome,
    },
    EventBody {
        trigger: HirStmtTriggerEvaluationPlan,
        scope: ScopeId,
        body: &'stmt [StmtId],
    },
    UnsafeLifetime {
        audit: &'stmt HirUnsafeAudit,
        body: &'stmt HirUnsafeLifetimeBody,
    },
    LetElse {
        pattern: PatternId,
        annotation: Option<TypeId>,
        initializer: ExprId,
        else_scope: ScopeId,
        else_body: &'stmt [StmtId],
        success_locals: &'stmt [LocalId],
    },
    If {
        condition: ExprId,
        then_body: &'stmt HirContextualStmtBody,
        else_branch: Option<&'stmt HirConditionalElseBranch>,
    },
    IfLet {
        pattern: PatternId,
        scrutinee: ExprId,
        guard: Option<ExprId>,
        branch_locals: &'stmt [LocalId],
        then_body: &'stmt HirContextualStmtBody,
        else_branch: Option<&'stmt HirConditionalElseBranch>,
    },
    Match {
        scrutinee: ExprId,
        arms: &'stmt [HirStmtMatchArm],
    },
    While {
        condition: ExprId,
        body: &'stmt HirContextualStmtBody,
    },
    WhileLet {
        pattern: PatternId,
        scrutinee: ExprId,
        guard: Option<ExprId>,
        branch_locals: &'stmt [LocalId],
        body: &'stmt HirContextualStmtBody,
    },
    For {
        source: ExprId,
        iterator: ExprId,
        next_value: ExprId,
        pattern: PatternId,
        branch_locals: &'stmt [LocalId],
        body: &'stmt HirContextualStmtBody,
    },
    Select {
        scope: Option<ScopeId>,
        plan: HirStmtSelectEvaluationPlan<'stmt>,
    },
    SourceLocale {
        locale: &'stmt HirSourceLocaleValue,
        body: &'stmt HirContextualStmtBody,
    },
    Scope {
        name: Option<&'stmt HirName>,
        body: &'stmt HirContextualStmtBody,
    },
    Include {
        target: &'stmt HirIdRefValue,
    },
    Continue {
        label: Option<&'stmt HirName>,
    },
    Recovered,
}

/// One source-ordered step emitted by a statement evaluation plan.
///
/// Unlike structural child edges, this stream interleaves ordinary statement
/// bodies, heterogeneous Thread bodies, else-if statements, match arms, and
/// Select branches at their authored evaluation boundaries. Consumers that
/// need expression reachability borrow this one stream instead of rebuilding a
/// second statement/body graph.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirStmtEvaluationStep<'stmt> {
    Expression {
        role: HirStatementChildRole,
        expression: ExprId,
    },
    Statement {
        role: HirStatementChildRole,
        statement: StmtId,
    },
    ThreadBody {
        role: HirStatementBodyRole,
        edge: HirBodyChildEdge,
    },
    Pattern {
        role: HirStatementChildRole,
        pattern: PatternId,
    },
    Type {
        role: HirStatementChildRole,
        ty: TypeId,
    },
    Local {
        role: HirStatementChildRole,
        local: LocalId,
    },
    Publication {
        role: HirStmtEvaluationPublicationRole,
        locals: &'stmt [LocalId],
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirStmtEvaluationPublicationRole {
    Binding { kind: HirStmtBindingPlanKind },
    LetElseSuccess,
    Branch { kind: HirStmtBranchPublicationKind },
    TriggerPattern { pattern: PatternId },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirStmtBranchPublicationKind {
    IfLet,
    MatchArm { arm: u32 },
    WhileLet,
    For,
    SelectBranch { branch: u32 },
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum HirStmtEvaluationStepError {
    #[error("a statement evaluation step ordinal does not fit u32")]
    OrdinalOverflow,
}

#[derive(Debug, Eq, PartialEq)]
pub enum HirStmtSelectEvaluationPlan<'stmt> {
    Operand {
        expression: ExprId,
    },
    Branches {
        branches: HirStmtSelectBranches<'stmt>,
    },
}

#[derive(Debug, Eq, PartialEq)]
pub struct HirStmtSelectBranches<'stmt> {
    branches: &'stmt [HirSelectBranch],
}

impl HirStmtSelectBranches<'_> {
    pub const fn len(&self) -> usize {
        self.branches.len()
    }

    pub const fn is_empty(&self) -> bool {
        self.branches.is_empty()
    }

    pub fn entries(&self) -> impl Iterator<Item = HirStmtSelectBranchEvaluation<'_>> {
        self.branches.iter().map(HirStmtSelectBranchEvaluation::new)
    }
}

impl<'stmt> HirStmtEvaluationPlan<'stmt> {
    /// Returns the one source-ordered evaluation stream owned by this plan.
    ///
    /// The stream is deliberately richer than expression reachability: typed
    /// pattern/type and binding-publication steps make visibility boundaries
    /// explicit, while statement and Thread-body steps retain the exact place
    /// where nested evaluation resumes.
    #[allow(
        clippy::too_many_lines,
        reason = "the exhaustive statement-family plan is the single evaluation-order authority"
    )]
    pub fn try_visit_evaluation_steps(
        &self,
        mut visitor: impl FnMut(HirStmtEvaluationStep<'stmt>),
    ) -> Result<(), HirStmtEvaluationStepError> {
        match self {
            Self::Assertion { conditions, .. } => {
                for (ordinal, expression) in conditions.iter().copied().enumerate() {
                    visitor(HirStmtEvaluationStep::Expression {
                        role: HirStatementChildRole::AssertionCondition {
                            ordinal: evaluation_ordinal(ordinal)?,
                        },
                        expression,
                    });
                }
            }
            Self::Binding {
                kind,
                pattern,
                annotation,
                input,
                locals,
            } => {
                if let Some(ty) = annotation {
                    visitor(HirStmtEvaluationStep::Type {
                        role: HirStatementChildRole::Annotation,
                        ty: *ty,
                    });
                }
                visitor(HirStmtEvaluationStep::Expression {
                    role: kind.input_role(),
                    expression: *input,
                });
                visitor(HirStmtEvaluationStep::Pattern {
                    role: HirStatementChildRole::Pattern,
                    pattern: *pattern,
                });
                visitor(HirStmtEvaluationStep::Publication {
                    role: HirStmtEvaluationPublicationRole::Binding { kind: *kind },
                    locals,
                });
            }
            Self::OrderedPair {
                kind,
                first,
                second,
            } => {
                let (first_role, second_role) = match kind {
                    HirStmtOrderedPairPlanKind::Assign
                    | HirStmtOrderedPairPlanKind::Signal
                    | HirStmtOrderedPairPlanKind::LifetimeSet => {
                        (HirStatementChildRole::Target, HirStatementChildRole::Value)
                    }
                };
                visitor(HirStmtEvaluationStep::Expression {
                    role: first_role,
                    expression: *first,
                });
                visitor(HirStmtEvaluationStep::Expression {
                    role: second_role,
                    expression: *second,
                });
            }
            Self::Value {
                kind, expression, ..
            } => {
                if let Some(expression) = expression {
                    visitor(HirStmtEvaluationStep::Expression {
                        role: kind.expression_role(),
                        expression: *expression,
                    });
                }
            }
            Self::DeferredBody { body, .. } => {
                visit_statement_steps(&mut visitor, HirStatementBodyRole::Defer, body)?;
            }
            Self::EventBody { trigger, body, .. } => {
                visit_trigger_steps(&mut visitor, trigger);
                visit_statement_steps(&mut visitor, HirStatementBodyRole::On, body)?;
            }
            Self::UnsafeLifetime { audit, body } => {
                if let Some(reason) = audit.reason() {
                    visitor(HirStmtEvaluationStep::Expression {
                        role: HirStatementChildRole::UnsafeReason,
                        expression: reason,
                    });
                }
                if let HirUnsafeLifetimeBody::Block { statements, .. } = body {
                    visit_statement_steps(
                        &mut visitor,
                        HirStatementBodyRole::UnsafeLifetime,
                        statements,
                    )?;
                }
            }
            Self::LetElse {
                pattern,
                annotation,
                initializer,
                else_body,
                success_locals,
                ..
            } => {
                if let Some(ty) = annotation {
                    visitor(HirStmtEvaluationStep::Type {
                        role: HirStatementChildRole::Annotation,
                        ty: *ty,
                    });
                }
                visitor(HirStmtEvaluationStep::Expression {
                    role: HirStatementChildRole::Initializer,
                    expression: *initializer,
                });
                visitor(HirStmtEvaluationStep::Pattern {
                    role: HirStatementChildRole::Pattern,
                    pattern: *pattern,
                });
                visit_statement_steps(&mut visitor, HirStatementBodyRole::LetElse, else_body)?;
                visitor(HirStmtEvaluationStep::Publication {
                    role: HirStmtEvaluationPublicationRole::LetElseSuccess,
                    locals: success_locals,
                });
            }
            Self::If {
                condition,
                then_body,
                else_branch,
            } => {
                visitor(HirStmtEvaluationStep::Expression {
                    role: HirStatementChildRole::Condition,
                    expression: *condition,
                });
                visit_contextual_steps(&mut visitor, HirStatementBodyRole::Then, then_body)?;
                visit_else_steps(&mut visitor, else_branch.as_deref())?;
            }
            Self::IfLet {
                pattern,
                scrutinee,
                guard,
                branch_locals,
                then_body,
                else_branch,
            } => {
                visitor(HirStmtEvaluationStep::Expression {
                    role: HirStatementChildRole::Scrutinee,
                    expression: *scrutinee,
                });
                visitor(HirStmtEvaluationStep::Pattern {
                    role: HirStatementChildRole::Pattern,
                    pattern: *pattern,
                });
                visitor(HirStmtEvaluationStep::Publication {
                    role: HirStmtEvaluationPublicationRole::Branch {
                        kind: HirStmtBranchPublicationKind::IfLet,
                    },
                    locals: branch_locals,
                });
                if let Some(guard) = guard {
                    visitor(HirStmtEvaluationStep::Expression {
                        role: HirStatementChildRole::Guard,
                        expression: *guard,
                    });
                }
                visit_contextual_steps(&mut visitor, HirStatementBodyRole::Then, then_body)?;
                visit_else_steps(&mut visitor, else_branch.as_deref())?;
            }
            Self::Match { scrutinee, arms } => {
                visitor(HirStmtEvaluationStep::Expression {
                    role: HirStatementChildRole::Scrutinee,
                    expression: *scrutinee,
                });
                for (arm, value) in arms.iter().enumerate() {
                    let arm = evaluation_ordinal(arm)?;
                    visitor(HirStmtEvaluationStep::Pattern {
                        role: HirStatementChildRole::MatchPattern { arm },
                        pattern: value.pattern(),
                    });
                    visitor(HirStmtEvaluationStep::Publication {
                        role: HirStmtEvaluationPublicationRole::Branch {
                            kind: HirStmtBranchPublicationKind::MatchArm { arm },
                        },
                        locals: value.locals(),
                    });
                    if let Some(guard) = value.guard() {
                        visitor(HirStmtEvaluationStep::Expression {
                            role: HirStatementChildRole::MatchGuard { arm },
                            expression: guard,
                        });
                    }
                    match value.body() {
                        HirStmtMatchArmBody::Expression(expression) => {
                            visitor(HirStmtEvaluationStep::Expression {
                                role: HirStatementChildRole::MatchValue { arm },
                                expression: *expression,
                            });
                        }
                        HirStmtMatchArmBody::Body(body) => visit_contextual_steps(
                            &mut visitor,
                            HirStatementBodyRole::MatchArm { arm },
                            body,
                        )?,
                    }
                }
            }
            Self::While { condition, body } => {
                visitor(HirStmtEvaluationStep::Expression {
                    role: HirStatementChildRole::Condition,
                    expression: *condition,
                });
                visit_contextual_steps(&mut visitor, HirStatementBodyRole::While, body)?;
            }
            Self::WhileLet {
                pattern,
                scrutinee,
                guard,
                branch_locals,
                body,
            } => {
                visitor(HirStmtEvaluationStep::Expression {
                    role: HirStatementChildRole::Scrutinee,
                    expression: *scrutinee,
                });
                visitor(HirStmtEvaluationStep::Pattern {
                    role: HirStatementChildRole::Pattern,
                    pattern: *pattern,
                });
                visitor(HirStmtEvaluationStep::Publication {
                    role: HirStmtEvaluationPublicationRole::Branch {
                        kind: HirStmtBranchPublicationKind::WhileLet,
                    },
                    locals: branch_locals,
                });
                if let Some(guard) = guard {
                    visitor(HirStmtEvaluationStep::Expression {
                        role: HirStatementChildRole::Guard,
                        expression: *guard,
                    });
                }
                visit_contextual_steps(&mut visitor, HirStatementBodyRole::WhileLet, body)?;
            }
            Self::For {
                source,
                iterator,
                next_value,
                pattern,
                branch_locals,
                body,
            } => {
                for (role, expression) in [
                    (HirStatementChildRole::ForSource, *source),
                    (HirStatementChildRole::ForIterator, *iterator),
                    (HirStatementChildRole::ForNextValue, *next_value),
                ] {
                    visitor(HirStmtEvaluationStep::Expression { role, expression });
                }
                visitor(HirStmtEvaluationStep::Pattern {
                    role: HirStatementChildRole::Pattern,
                    pattern: *pattern,
                });
                visitor(HirStmtEvaluationStep::Publication {
                    role: HirStmtEvaluationPublicationRole::Branch {
                        kind: HirStmtBranchPublicationKind::For,
                    },
                    locals: branch_locals,
                });
                visit_contextual_steps(&mut visitor, HirStatementBodyRole::For, body)?;
            }
            Self::Select { plan, .. } => match plan {
                HirStmtSelectEvaluationPlan::Operand { expression } => {
                    visitor(HirStmtEvaluationStep::Expression {
                        role: HirStatementChildRole::SelectOperand,
                        expression: *expression,
                    });
                }
                HirStmtSelectEvaluationPlan::Branches { branches } => {
                    for (branch, value) in branches.branches.iter().enumerate() {
                        let branch = evaluation_ordinal(branch)?;
                        match value.head() {
                            HirSelectBranchHead::Bind {
                                binding, source, ..
                            } => {
                                visitor(HirStmtEvaluationStep::Expression {
                                    role: HirStatementChildRole::SelectSource { branch },
                                    expression: *source,
                                });
                                if let Some(local) = binding.resolved() {
                                    visitor(HirStmtEvaluationStep::Local {
                                        role: HirStatementChildRole::SelectBinding { branch },
                                        local,
                                    });
                                }
                                visitor(HirStmtEvaluationStep::Publication {
                                    role: HirStmtEvaluationPublicationRole::Branch {
                                        kind: HirStmtBranchPublicationKind::SelectBranch { branch },
                                    },
                                    locals: &[],
                                });
                            }
                            HirSelectBranchHead::Frame { pattern, locals }
                            | HirSelectBranchHead::Event { pattern, locals } => {
                                visitor(HirStmtEvaluationStep::Pattern {
                                    role: HirStatementChildRole::SelectPattern { branch },
                                    pattern: *pattern,
                                });
                                visitor(HirStmtEvaluationStep::Publication {
                                    role: HirStmtEvaluationPublicationRole::Branch {
                                        kind: HirStmtBranchPublicationKind::SelectBranch { branch },
                                    },
                                    locals,
                                });
                            }
                            HirSelectBranchHead::Recovered => {}
                        }
                        visit_contextual_steps(
                            &mut visitor,
                            HirStatementBodyRole::SelectBranch { branch },
                            value.body(),
                        )?;
                    }
                }
            },
            Self::SourceLocale { body, .. } => {
                visit_contextual_steps(&mut visitor, HirStatementBodyRole::SourceLocale, body)?;
            }
            Self::Scope { body, .. } => {
                visit_contextual_steps(&mut visitor, HirStatementBodyRole::Scope, body)?;
            }
            Self::Include { .. } | Self::Continue { .. } | Self::Recovered => {}
        }
        Ok(())
    }
}

fn evaluation_ordinal(value: usize) -> Result<u32, HirStmtEvaluationStepError> {
    u32::try_from(value).map_err(|_| HirStmtEvaluationStepError::OrdinalOverflow)
}

fn visit_statement_steps<'stmt>(
    visitor: &mut impl FnMut(HirStmtEvaluationStep<'stmt>),
    body: HirStatementBodyRole,
    statements: &[StmtId],
) -> Result<(), HirStmtEvaluationStepError> {
    for (ordinal, statement) in statements.iter().copied().enumerate() {
        visitor(HirStmtEvaluationStep::Statement {
            role: HirStatementChildRole::BodyItem {
                body,
                ordinal: evaluation_ordinal(ordinal)?,
            },
            statement,
        });
    }
    Ok(())
}

fn visit_contextual_steps<'stmt>(
    visitor: &mut impl FnMut(HirStmtEvaluationStep<'stmt>),
    body_role: HirStatementBodyRole,
    body: &HirContextualStmtBody,
) -> Result<(), HirStmtEvaluationStepError> {
    if let Some(statements) = body.ordinary_statements() {
        return visit_statement_steps(visitor, body_role, statements);
    }
    if let Some(body) = body.thread_body() {
        for edge in body
            .try_child_edges()
            .map_err(|_| HirStmtEvaluationStepError::OrdinalOverflow)?
        {
            visitor(HirStmtEvaluationStep::ThreadBody {
                role: body_role,
                edge,
            });
        }
    }
    Ok(())
}

fn visit_else_steps<'stmt>(
    visitor: &mut impl FnMut(HirStmtEvaluationStep<'stmt>),
    branch: Option<&HirConditionalElseBranch>,
) -> Result<(), HirStmtEvaluationStepError> {
    match branch {
        Some(HirConditionalElseBranch::Body(body)) => {
            visit_contextual_steps(visitor, HirStatementBodyRole::Else, body)
        }
        Some(HirConditionalElseBranch::ElseIf(statement)) => {
            visitor(HirStmtEvaluationStep::Statement {
                role: HirStatementChildRole::ElseIf,
                statement: *statement,
            });
            Ok(())
        }
        None => Ok(()),
    }
}

fn visit_trigger_steps<'stmt>(
    visitor: &mut impl FnMut(HirStmtEvaluationStep<'stmt>),
    trigger: &HirStmtTriggerEvaluationPlan,
) {
    match trigger {
        HirStmtTriggerEvaluationPlan::Pattern { pattern } => {
            visitor(HirStmtEvaluationStep::Pattern {
                role: HirStatementChildRole::TriggerPattern,
                pattern: *pattern,
            });
            visitor(HirStmtEvaluationStep::Publication {
                role: HirStmtEvaluationPublicationRole::TriggerPattern { pattern: *pattern },
                locals: &[],
            });
        }
        HirStmtTriggerEvaluationPlan::Signal { target, value } => {
            visitor(HirStmtEvaluationStep::Expression {
                role: HirStatementChildRole::TriggerSignalTarget,
                expression: *target,
            });
            if let Some(pattern) = value {
                visitor(HirStmtEvaluationStep::Pattern {
                    role: HirStatementChildRole::TriggerSignalValue,
                    pattern: *pattern,
                });
                visitor(HirStmtEvaluationStep::Publication {
                    role: HirStmtEvaluationPublicationRole::TriggerPattern { pattern: *pattern },
                    locals: &[],
                });
            }
        }
        HirStmtTriggerEvaluationPlan::Expression { expression } => {
            visitor(HirStmtEvaluationStep::Expression {
                role: HirStatementChildRole::TriggerExpression,
                expression: *expression,
            });
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct HirStmtSelectBranchEvaluation<'stmt> {
    branch: &'stmt HirSelectBranch,
}

impl HirStmtSelectBranchEvaluation<'_> {
    const fn new(branch: &HirSelectBranch) -> HirStmtSelectBranchEvaluation<'_> {
        HirStmtSelectBranchEvaluation { branch }
    }

    pub fn head(&self) -> HirStmtSelectHeadEvaluation<'_> {
        self.branch.evaluation_head()
    }

    pub const fn body(&self) -> &HirContextualStmtBody {
        self.branch.body()
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum HirStmtSelectHeadEvaluation<'stmt> {
    Bind {
        binding: &'stmt HirSelectBindingLocal,
        source: ExprId,
        propagates_error: bool,
    },
    Frame {
        pattern: PatternId,
        locals: &'stmt [LocalId],
    },
    Event {
        pattern: PatternId,
        locals: &'stmt [LocalId],
    },
    Recovered,
}

impl HirSelectBranch {
    /// Projects one source-ordered branch head without copying its binding
    /// identity or local slice. The raw branch remains the HIR storage owner;
    /// this wrapper is the typed evaluation view.
    pub fn evaluation_head(&self) -> HirStmtSelectHeadEvaluation<'_> {
        match self.head() {
            HirSelectBranchHead::Bind {
                binding,
                source,
                propagates_error,
            } => HirStmtSelectHeadEvaluation::Bind {
                binding,
                source: *source,
                propagates_error: *propagates_error,
            },
            HirSelectBranchHead::Frame { pattern, locals } => HirStmtSelectHeadEvaluation::Frame {
                pattern: *pattern,
                locals,
            },
            HirSelectBranchHead::Event { pattern, locals } => HirStmtSelectHeadEvaluation::Event {
                pattern: *pattern,
                locals,
            },
            HirSelectBranchHead::Recovered => HirStmtSelectHeadEvaluation::Recovered,
        }
    }

    pub const fn evaluation_body(&self) -> &HirContextualStmtBody {
        self.body()
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum HirStmtTriggerEvaluationPlan {
    Pattern {
        pattern: PatternId,
    },
    Signal {
        target: ExprId,
        value: Option<PatternId>,
    },
    Expression {
        expression: ExprId,
    },
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirStmtBindingPlanKind {
    Let,
    LetChoice,
    LetScope,
    LetActionReceive,
}

impl HirStmtBindingPlanKind {
    /// Returns the exact child role used to evaluate this binding's input.
    pub const fn input_role(self) -> HirStatementChildRole {
        match self {
            Self::Let => HirStatementChildRole::Initializer,
            Self::LetChoice | Self::LetScope | Self::LetActionReceive => {
                HirStatementChildRole::Input
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirStmtOrderedPairPlanKind {
    Assign,
    Signal,
    LifetimeSet,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirStmtValuePlanKind {
    Return,
    Out,
    Defer,
    Yield,
    Goto,
    Wait,
    Close,
    Choice,
    Expression,
    ProofCall,
    Break,
}

impl HirStmtValuePlanKind {
    /// Returns the exact child role used to evaluate this value expression.
    pub const fn expression_role(self) -> HirStatementChildRole {
        match self {
            Self::Goto | Self::Wait | Self::Close => HirStatementChildRole::Target,
            Self::Choice => HirStatementChildRole::Input,
            Self::Return
            | Self::Out
            | Self::Defer
            | Self::Yield
            | Self::Break
            | Self::Expression
            | Self::ProofCall => HirStatementChildRole::Value,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirStmtDeferredBodyPlanKind {
    DeferBlock,
}

fn trigger_evaluation_plan(trigger: &HirTriggerPattern) -> HirStmtTriggerEvaluationPlan {
    match trigger {
        HirTriggerPattern::Input(pattern)
        | HirTriggerPattern::Event(pattern)
        | HirTriggerPattern::Mark(pattern)
        | HirTriggerPattern::Select(pattern)
        | HirTriggerPattern::Task(pattern)
        | HirTriggerPattern::Scope(pattern) => {
            HirStmtTriggerEvaluationPlan::Pattern { pattern: *pattern }
        }
        HirTriggerPattern::Signal { target, value } => HirStmtTriggerEvaluationPlan::Signal {
            target: *target,
            value: *value,
        },
        HirTriggerPattern::Timeout(expression) | HirTriggerPattern::Expr(expression) => {
            HirStmtTriggerEvaluationPlan::Expression {
                expression: *expression,
            }
        }
    }
}

fn select_evaluation_plan(select: &HirSelectStmt) -> HirStmtSelectEvaluationPlan<'_> {
    match select {
        HirSelectStmt::Operand(expression) => HirStmtSelectEvaluationPlan::Operand {
            expression: *expression,
        },
        HirSelectStmt::Branches { branches, .. } => HirStmtSelectEvaluationPlan::Branches {
            branches: HirStmtSelectBranches { branches },
        },
    }
}

/// Canonical assertion mode or typed recovery from the same source family.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirAssertionMode {
    Resolved(AssertionMode),
    Recovered,
}

impl HirAssertionMode {
    pub const fn resolved(self) -> Option<AssertionMode> {
        match self {
            Self::Resolved(mode) => Some(mode),
            Self::Recovered => None,
        }
    }
}

impl HirStmtKind {
    /// Returns the stable semantic constructor tag used by checked
    /// statement transcripts.
    pub const fn semantic_transcript_tag(&self) -> u16 {
        match self {
            Self::Assertion { .. } => 0x0700,
            Self::Let { .. } => 0x0701,
            Self::Assign { .. } => 0x0702,
            Self::LetElse { .. } => 0x0703,
            Self::LetChoice { .. } => 0x0704,
            Self::LetScope { .. } => 0x0705,
            Self::LetActionReceive { .. } => 0x0706,
            Self::Return { .. } => 0x0707,
            Self::Out { .. } => 0x0708,
            Self::Goto { .. } => 0x0709,
            Self::DeferBlock { .. } => 0x070A,
            Self::Defer { .. } => 0x070B,
            Self::Yield { .. } => 0x070C,
            Self::Signal { .. } => 0x070D,
            Self::LifetimeSet { .. } => 0x070E,
            Self::Wait { .. } => 0x070F,
            Self::On { .. } => 0x0710,
            Self::UnsafeLifetime { .. } => 0x0711,
            Self::Choice { .. } => 0x0712,
            Self::If(_) => 0x0713,
            Self::IfLet(_) => 0x0714,
            Self::Match(_) => 0x0715,
            Self::While(_) => 0x0716,
            Self::WhileLet(_) => 0x0717,
            Self::For(_) => 0x0718,
            Self::Close { .. } => 0x0719,
            Self::Select(_) => 0x071A,
            Self::SourceLocale(_) => 0x071B,
            Self::Scope(_) => 0x071C,
            Self::Include(_) => 0x071D,
            Self::Break { .. } => 0x071E,
            Self::Continue { .. } => 0x071F,
            Self::Expression { .. } => 0x0720,
            Self::ProofCall { .. } => 0x0721,
            Self::Error => 0x0722,
        }
    }

    /// Returns the typed evaluation order for every statement family.
    ///
    /// This projection is borrowed from the statement payload and is not a
    /// second structural-child inventory. In particular, input expressions
    /// precede binding publication, and branch-local bindings are visible
    /// only to their matching guard/body steps.
    #[allow(
        clippy::too_many_lines,
        reason = "the closed 35-family statement algebra has one explicit evaluation-order authority"
    )]
    /// Returns the closed typed evaluation plan for all 35 statement
    /// families. Branch-bearing rows retain their success/failure binding
    /// scopes instead of flattening them into a visibility-tagged sequence.
    pub fn evaluation_plan(&self) -> HirStmtEvaluationPlan<'_> {
        use HirStmtEvaluationPlan as Plan;
        match self {
            Self::Assertion { mode, conditions } => Plan::Assertion {
                mode: *mode,
                conditions,
            },
            Self::Let {
                pattern,
                annotation,
                initializer,
                locals,
            } => Plan::Binding {
                kind: HirStmtBindingPlanKind::Let,
                pattern: *pattern,
                annotation: *annotation,
                input: *initializer,
                locals,
            },
            Self::Assign { target, value } => Plan::OrderedPair {
                kind: HirStmtOrderedPairPlanKind::Assign,
                first: *target,
                second: *value,
            },
            Self::LetElse {
                pattern,
                annotation,
                initializer,
                else_scope,
                else_body,
                locals,
            } => Plan::LetElse {
                pattern: *pattern,
                annotation: *annotation,
                initializer: *initializer,
                else_scope: *else_scope,
                else_body,
                success_locals: locals,
            },
            Self::LetChoice {
                pattern,
                choice,
                locals,
            } => Plan::Binding {
                kind: HirStmtBindingPlanKind::LetChoice,
                pattern: *pattern,
                annotation: None,
                input: *choice,
                locals,
            },
            Self::LetScope {
                pattern,
                scope_expr,
                locals,
            } => Plan::Binding {
                kind: HirStmtBindingPlanKind::LetScope,
                pattern: *pattern,
                annotation: None,
                input: *scope_expr,
                locals,
            },
            Self::LetActionReceive {
                pattern,
                action,
                locals,
            } => Plan::Binding {
                kind: HirStmtBindingPlanKind::LetActionReceive,
                pattern: *pattern,
                annotation: None,
                input: *action,
                locals,
            },
            Self::Return { value } => Plan::Value {
                kind: HirStmtValuePlanKind::Return,
                expression: Some(*value),
                label: None,
                outcome: None,
            },
            Self::Out { label, value } => Plan::Value {
                kind: HirStmtValuePlanKind::Out,
                expression: Some(*value),
                label: label.as_ref(),
                outcome: None,
            },
            Self::Goto { target } => Plan::Value {
                kind: HirStmtValuePlanKind::Goto,
                expression: Some(*target),
                label: None,
                outcome: None,
            },
            Self::DeferBlock {
                outcome,
                scope,
                body,
            } => Plan::DeferredBody {
                kind: HirStmtDeferredBodyPlanKind::DeferBlock,
                scope: *scope,
                body,
                outcome: *outcome,
            },
            Self::Defer {
                outcome,
                expression,
            } => Plan::Value {
                kind: HirStmtValuePlanKind::Defer,
                expression: Some(*expression),
                label: None,
                outcome: Some(*outcome),
            },
            Self::Yield { expression } => Plan::Value {
                kind: HirStmtValuePlanKind::Yield,
                expression: Some(*expression),
                label: None,
                outcome: None,
            },
            Self::Signal { target, value } => Plan::OrderedPair {
                kind: HirStmtOrderedPairPlanKind::Signal,
                first: *target,
                second: *value,
            },
            Self::LifetimeSet { target, value } => Plan::OrderedPair {
                kind: HirStmtOrderedPairPlanKind::LifetimeSet,
                first: *target,
                second: *value,
            },
            Self::Wait { target } => Plan::Value {
                kind: HirStmtValuePlanKind::Wait,
                expression: Some(*target),
                label: None,
                outcome: None,
            },
            Self::On {
                trigger,
                scope,
                body,
            } => Plan::EventBody {
                trigger: trigger_evaluation_plan(trigger),
                scope: *scope,
                body,
            },
            Self::UnsafeLifetime { audit, body } => Plan::UnsafeLifetime { audit, body },
            Self::Choice { choice } => Plan::Value {
                kind: HirStmtValuePlanKind::Choice,
                expression: Some(*choice),
                label: None,
                outcome: None,
            },
            Self::If(statement) => Plan::If {
                condition: statement.condition(),
                then_body: statement.then_body(),
                else_branch: statement.else_branch(),
            },
            Self::IfLet(statement) => Plan::IfLet {
                pattern: statement.pattern(),
                scrutinee: statement.scrutinee(),
                guard: statement.guard(),
                branch_locals: statement.locals(),
                then_body: statement.then_body(),
                else_branch: statement.else_branch(),
            },
            Self::Match(statement) => Plan::Match {
                scrutinee: statement.scrutinee(),
                arms: statement.arms(),
            },
            Self::While(statement) => Plan::While {
                condition: statement.condition(),
                body: statement.body(),
            },
            Self::WhileLet(statement) => Plan::WhileLet {
                pattern: statement.pattern(),
                scrutinee: statement.scrutinee(),
                guard: statement.guard(),
                branch_locals: statement.locals(),
                body: statement.body(),
            },
            Self::For(statement) => Plan::For {
                source: statement.source(),
                iterator: statement.iterator(),
                next_value: statement.next_value(),
                pattern: statement.pattern(),
                branch_locals: statement.locals(),
                body: statement.body(),
            },
            Self::Close { target } => Plan::Value {
                kind: HirStmtValuePlanKind::Close,
                expression: Some(*target),
                label: None,
                outcome: None,
            },
            Self::Select(select) => Plan::Select {
                scope: select.scope(),
                plan: select_evaluation_plan(select),
            },
            Self::SourceLocale(statement) => Plan::SourceLocale {
                locale: statement.locale(),
                body: statement.body(),
            },
            Self::Scope(statement) => Plan::Scope {
                name: statement.name(),
                body: statement.body(),
            },
            Self::Include(include) => Plan::Include {
                target: include.target(),
            },
            Self::Break { label, value } => Plan::Value {
                kind: HirStmtValuePlanKind::Break,
                expression: *value,
                label: label.as_ref(),
                outcome: None,
            },
            Self::Continue { label } => Plan::Continue {
                label: label.as_ref(),
            },
            Self::Expression { expression } => Plan::Value {
                kind: HirStmtValuePlanKind::Expression,
                expression: Some(*expression),
                label: None,
                outcome: None,
            },
            Self::ProofCall { call } => Plan::Value {
                kind: HirStmtValuePlanKind::ProofCall,
                expression: Some(*call),
                label: None,
                outcome: None,
            },
            Self::Error => Plan::Recovered,
        }
    }

    /// Returns every type-arena root attached directly to this statement.
    #[allow(
        dead_code,
        reason = "retained only as the differential projection of typed statement child edges"
    )]
    pub(crate) fn direct_type_roots(&self) -> Vec<TypeId> {
        self.child_edges()
            .into_iter()
            .filter_map(|edge| match edge.child() {
                HirStatementChild::Type(ty) => Some(ty),
                HirStatementChild::Expression(_)
                | HirStatementChild::Statement(_)
                | HirStatementChild::Pattern(_)
                | HirStatementChild::Local(_) => None,
            })
            .collect()
    }

    /// Returns locals whose names become visible only after this complete
    /// statement has finished evaluating.
    ///
    /// These `let` families evaluate their input in the pre-binding scope.
    /// Control-flow bindings such as `if let`, `while let`, `for`, and match
    /// arms instead belong to their child scope and therefore do not appear
    /// here.
    pub(crate) fn post_statement_locals(&self) -> &[LocalId] {
        match self {
            Self::Let { locals, .. }
            | Self::LetElse { locals, .. }
            | Self::LetChoice { locals, .. }
            | Self::LetScope { locals, .. }
            | Self::LetActionReceive { locals, .. } => locals,
            _ => &[],
        }
    }

    pub(crate) fn thread_body_for_scope(
        &self,
        scope: ScopeId,
    ) -> Option<&crate::expr::HirThreadBody> {
        match self {
            Self::If(statement) => statement.thread_body_for_scope(scope),
            Self::IfLet(statement) => statement.thread_body_for_scope(scope),
            Self::Match(statement) => statement.thread_body_for_scope(scope),
            Self::While(statement) => statement.thread_body_for_scope(scope),
            Self::WhileLet(statement) => statement.thread_body_for_scope(scope),
            Self::For(statement) => statement.thread_body_for_scope(scope),
            Self::Select(statement) => statement.thread_body_for_scope(scope),
            Self::SourceLocale(statement) => statement.thread_body_for_scope(scope),
            Self::Scope(statement) => statement.thread_body_for_scope(scope),
            _ => None,
        }
    }

    #[allow(
        clippy::too_many_lines,
        reason = "one exhaustive statement-family validator preserves the closed typed-ID ownership matrix"
    )]
    fn validate_module(&self, expected: HirModuleId) -> Result<(), HirStmtInvariantError> {
        match self {
            Self::Assertion { conditions, .. } => validate_exprs(expected, conditions),
            Self::Let {
                pattern,
                annotation,
                initializer,
                locals,
            } => {
                validate_pattern(expected, *pattern)?;
                validate_optional_type(expected, *annotation)?;
                validate_expr(expected, *initializer)?;
                validate_locals(expected, locals)
            }
            Self::Assign { target, value }
            | Self::Signal { target, value }
            | Self::LifetimeSet { target, value } => {
                validate_expr(expected, *target)?;
                validate_expr(expected, *value)
            }
            Self::LetElse {
                pattern,
                annotation,
                initializer,
                else_scope,
                else_body,
                locals,
            } => {
                validate_pattern(expected, *pattern)?;
                validate_optional_type(expected, *annotation)?;
                validate_expr(expected, *initializer)?;
                validate_scope(expected, *else_scope)?;
                validate_statements(expected, else_body)?;
                validate_locals(expected, locals)
            }
            Self::LetChoice {
                pattern,
                choice,
                locals,
            }
            | Self::LetScope {
                pattern,
                scope_expr: choice,
                locals,
            }
            | Self::LetActionReceive {
                pattern,
                action: choice,
                locals,
            } => {
                validate_pattern(expected, *pattern)?;
                validate_expr(expected, *choice)?;
                validate_locals(expected, locals)
            }
            Self::Return { value } | Self::Out { value, .. } => validate_expr(expected, *value),
            Self::Goto { target } | Self::Wait { target } | Self::Close { target } => {
                validate_expr(expected, *target)
            }
            Self::DeferBlock { scope, body, .. } => {
                validate_scope(expected, *scope)?;
                validate_statements(expected, body)
            }
            Self::Defer { expression, .. }
            | Self::Yield { expression }
            | Self::Expression { expression }
            | Self::ProofCall { call: expression } => validate_expr(expected, *expression),
            Self::On {
                trigger,
                scope,
                body,
            } => {
                trigger.validate_module(expected)?;
                validate_scope(expected, *scope)?;
                validate_statements(expected, body)
            }
            Self::UnsafeLifetime { audit, body } => {
                audit.validate_module(expected)?;
                body.validate_module(expected)
            }
            Self::Choice { choice } => validate_expr(expected, *choice),
            Self::If(statement) => statement.validate_module(expected).map_err(Into::into),
            Self::IfLet(statement) => statement.validate_module(expected).map_err(Into::into),
            Self::Match(statement) => statement.validate_module(expected).map_err(Into::into),
            Self::While(statement) => statement.validate_module(expected).map_err(Into::into),
            Self::WhileLet(statement) => statement.validate_module(expected).map_err(Into::into),
            Self::For(statement) => statement.validate_module(expected).map_err(Into::into),
            Self::Select(statement) => statement.validate_module(expected).map_err(Into::into),
            Self::SourceLocale(statement) => {
                statement.validate_module(expected).map_err(Into::into)
            }
            Self::Scope(statement) => statement.validate_module(expected).map_err(Into::into),
            Self::Include(_) | Self::Continue { .. } | Self::Error => Ok(()),
            Self::Break { value, .. } => validate_optional_expr(expected, *value),
        }
    }
}

/// Typed-ID projection of the closed trigger-pattern authority.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirTriggerPattern {
    Input(PatternId),
    Event(PatternId),
    Signal {
        target: ExprId,
        value: Option<PatternId>,
    },
    Timeout(ExprId),
    Mark(PatternId),
    Select(PatternId),
    Task(PatternId),
    Scope(PatternId),
    Expr(ExprId),
}

impl HirTriggerPattern {
    pub(crate) fn validate_module(
        &self,
        expected: HirModuleId,
    ) -> Result<(), HirStmtInvariantError> {
        match self {
            Self::Input(pattern)
            | Self::Event(pattern)
            | Self::Mark(pattern)
            | Self::Select(pattern)
            | Self::Task(pattern)
            | Self::Scope(pattern) => validate_pattern(expected, *pattern),
            Self::Signal { target, value } => {
                validate_expr(expected, *target)?;
                if let Some(value) = value {
                    validate_pattern(expected, *value)?;
                }
                Ok(())
            }
            Self::Timeout(expression) | Self::Expr(expression) => {
                validate_expr(expected, *expression)
            }
        }
    }
}

/// Semantic unsafe-lifetime audit data without revision-bound source ranges.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirUnsafeAudit {
    id: HirIdRefValue,
    reason: Option<ExprId>,
    has_safety_doc: bool,
}

/// Exact semantic body retained by an unsafe-lifetime statement.
///
/// A missing required body is not assigned a fabricated lexical scope. An
/// authored block keeps its source-backed scope and ordered statements even
/// when its closing delimiter or one of its children is recovered.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirUnsafeLifetimeBody {
    Block {
        scope: ScopeId,
        statements: Box<[StmtId]>,
    },
    Missing,
}

impl HirUnsafeLifetimeBody {
    pub const fn scope(&self) -> Option<ScopeId> {
        match self {
            Self::Block { scope, .. } => Some(*scope),
            Self::Missing => None,
        }
    }

    pub fn statements(&self) -> &[StmtId] {
        match self {
            Self::Block { statements, .. } => statements,
            Self::Missing => &[],
        }
    }

    fn validate_module(&self, expected: HirModuleId) -> Result<(), HirStmtInvariantError> {
        match self {
            Self::Block { scope, statements } => {
                validate_scope(expected, *scope)?;
                validate_statements(expected, statements)
            }
            Self::Missing => Ok(()),
        }
    }
}

impl HirUnsafeAudit {
    pub(crate) const fn new(
        id: HirIdRefValue,
        reason: Option<ExprId>,
        has_safety_doc: bool,
    ) -> Self {
        Self {
            id,
            reason,
            has_safety_doc,
        }
    }

    /// Returns the typed unsafe-audit identity.
    pub const fn id(&self) -> &HirIdRefValue {
        &self.id
    }

    /// Returns the optional typed reason expression.
    pub const fn reason(&self) -> Option<ExprId> {
        self.reason
    }

    /// Returns whether an authored `SAFETY` document is present.
    pub const fn has_safety_doc(&self) -> bool {
        self.has_safety_doc
    }

    fn validate_module(&self, expected: HirModuleId) -> Result<(), HirStmtInvariantError> {
        validate_optional_expr(expected, self.reason)
    }
}

/// Statement record rejected before arena publication.
#[derive(Clone, Copy, Debug, Eq, Error, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum HirStmtInvariantError {
    #[error("statement child belongs to module {actual:?}, expected {expected:?}")]
    ForeignChild {
        expected: HirModuleId,
        actual: HirModuleId,
    },
    #[error(transparent)]
    Thread(#[from] HirThreadStmtInvariantError),
    #[error("statement poison state does not match its payload family")]
    InvalidPoisonState,
}

fn validate_exprs(
    expected: HirModuleId,
    expressions: &[ExprId],
) -> Result<(), HirStmtInvariantError> {
    for expression in expressions {
        validate_expr(expected, *expression)?;
    }
    Ok(())
}

fn validate_statements(
    expected: HirModuleId,
    statements: &[StmtId],
) -> Result<(), HirStmtInvariantError> {
    for statement in statements {
        validate_statement(expected, *statement)?;
    }
    Ok(())
}

fn validate_locals(expected: HirModuleId, locals: &[LocalId]) -> Result<(), HirStmtInvariantError> {
    for local in locals {
        validate_module(expected, local.module())?;
    }
    Ok(())
}

fn validate_expr(expected: HirModuleId, expression: ExprId) -> Result<(), HirStmtInvariantError> {
    validate_module(expected, expression.module())
}

fn validate_optional_expr(
    expected: HirModuleId,
    expression: Option<ExprId>,
) -> Result<(), HirStmtInvariantError> {
    if let Some(expression) = expression {
        validate_expr(expected, expression)?;
    }
    Ok(())
}

fn validate_pattern(
    expected: HirModuleId,
    pattern: PatternId,
) -> Result<(), HirStmtInvariantError> {
    validate_module(expected, pattern.module())
}

fn validate_optional_type(
    expected: HirModuleId,
    ty: Option<TypeId>,
) -> Result<(), HirStmtInvariantError> {
    if let Some(ty) = ty {
        validate_module(expected, ty.module())?;
    }
    Ok(())
}

fn validate_scope(expected: HirModuleId, scope: ScopeId) -> Result<(), HirStmtInvariantError> {
    validate_module(expected, scope.module())
}

fn validate_statement(
    expected: HirModuleId,
    statement: StmtId,
) -> Result<(), HirStmtInvariantError> {
    validate_module(expected, statement.module())
}

fn validate_module(
    expected: HirModuleId,
    actual: HirModuleId,
) -> Result<(), HirStmtInvariantError> {
    if expected == actual {
        Ok(())
    } else {
        Err(HirStmtInvariantError::ForeignChild { expected, actual })
    }
}

#[cfg(test)]
#[path = "stmt/tests.rs"]
mod tests;
