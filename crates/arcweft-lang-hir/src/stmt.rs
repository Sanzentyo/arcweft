//! Final semantic statement records owned by the qualified HIR arena.
//!
//! Statement payloads retain semantic values and qualified child IDs only.
//! Revision-bound source components, including `UnsafeAuditInsertion`, belong
//! to the HIR source index rather than these records.

mod thread;

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
    /// Returns every type-arena root attached directly to this statement.
    pub(crate) fn direct_type_roots(&self) -> Vec<TypeId> {
        match self {
            Self::Let { annotation, .. } | Self::LetElse { annotation, .. } => {
                annotation.iter().copied().collect()
            }
            Self::Assertion { .. }
            | Self::Assign { .. }
            | Self::LetChoice { .. }
            | Self::LetScope { .. }
            | Self::LetActionReceive { .. }
            | Self::Return { .. }
            | Self::Out { .. }
            | Self::Goto { .. }
            | Self::DeferBlock { .. }
            | Self::Defer { .. }
            | Self::Yield { .. }
            | Self::Signal { .. }
            | Self::LifetimeSet { .. }
            | Self::Wait { .. }
            | Self::On { .. }
            | Self::UnsafeLifetime { .. }
            | Self::Choice { .. }
            | Self::If(_)
            | Self::IfLet(_)
            | Self::Match(_)
            | Self::While(_)
            | Self::WhileLet(_)
            | Self::For(_)
            | Self::Close { .. }
            | Self::Select(_)
            | Self::SourceLocale(_)
            | Self::Scope(_)
            | Self::Include(_)
            | Self::Break { .. }
            | Self::Continue { .. }
            | Self::Expression { .. }
            | Self::ProofCall { .. }
            | Self::Error => Vec::new(),
        }
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
