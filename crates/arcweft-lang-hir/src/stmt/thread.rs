//! Contextual statement payloads shared by ordinary and Thread/Flow bodies.
//!
//! These records retain semantic values and qualified HIR IDs only. Attached
//! syntax, delimiters, source ranges, and diagnostic anchors remain in the
//! revision-bound source index.

use std::collections::BTreeSet;

use arcweft_id::LocaleTag;
use thiserror::Error;

use crate::expr::HirThreadBody;
use crate::identity::{ExprId, HirModuleId, LocalId, PatternId, ScopeId, StmtId};
use crate::leaf::{HirIdRefIssue, HirIdRefValue, HirName, HirNameInvariantError};

/// One nested statement body in its owning execution context.
///
/// Ordinary callable blocks contain statement IDs. Flow and Thread blocks use
/// the sole heterogeneous [`HirThreadBody`] owner and therefore cannot acquire
/// a parallel statement-only projection.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirContextualStmtBody {
    Ordinary {
        scope: ScopeId,
        statements: Box<[StmtId]>,
    },
    Thread(HirThreadBody),
}

impl HirContextualStmtBody {
    pub(crate) fn try_ordinary(
        scope: ScopeId,
        statements: Box<[StmtId]>,
    ) -> Result<Self, HirThreadStmtInvariantError> {
        validate_statements(scope.module(), &statements)?;
        reject_duplicate_ids(statements.iter().copied())?;
        Ok(Self::Ordinary { scope, statements })
    }

    pub(crate) fn try_thread(body: HirThreadBody) -> Result<Self, HirThreadStmtInvariantError> {
        body.validate_module(body.scope().module())
            .map_err(|actual| HirThreadStmtInvariantError::ForeignChild {
                expected: body.scope().module(),
                actual,
            })?;
        Ok(Self::Thread(body))
    }

    pub const fn scope(&self) -> ScopeId {
        match self {
            Self::Ordinary { scope, .. } => *scope,
            Self::Thread(body) => body.scope(),
        }
    }

    pub fn ordinary_statements(&self) -> Option<&[StmtId]> {
        match self {
            Self::Ordinary { statements, .. } => Some(statements),
            Self::Thread(_) => None,
        }
    }

    pub const fn thread_body(&self) -> Option<&HirThreadBody> {
        match self {
            Self::Ordinary { .. } => None,
            Self::Thread(body) => Some(body),
        }
    }

    pub(crate) fn thread_body_for_scope(&self, scope: ScopeId) -> Option<&HirThreadBody> {
        self.thread_body().filter(|body| body.scope() == scope)
    }

    pub(crate) fn validate_module(
        &self,
        expected: HirModuleId,
    ) -> Result<(), HirThreadStmtInvariantError> {
        validate_module(expected, self.scope().module())?;
        match self {
            Self::Ordinary { statements, .. } => validate_statements(expected, statements),
            Self::Thread(body) => body
                .validate_module(expected)
                .map_err(|actual| HirThreadStmtInvariantError::ForeignChild { expected, actual }),
        }
    }
}

/// Typed branch following an `else` keyword.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirConditionalElseBranch {
    Body(HirContextualStmtBody),
    ElseIf(StmtId),
}

impl HirConditionalElseBranch {
    pub const fn body(body: HirContextualStmtBody) -> Self {
        Self::Body(body)
    }

    pub const fn else_if(statement: StmtId) -> Self {
        Self::ElseIf(statement)
    }

    fn validate_module(&self, expected: HirModuleId) -> Result<(), HirThreadStmtInvariantError> {
        match self {
            Self::Body(body) => body.validate_module(expected),
            Self::ElseIf(statement) => validate_module(expected, statement.module()),
        }
    }

    pub(crate) fn thread_body_for_scope(&self, scope: ScopeId) -> Option<&HirThreadBody> {
        match self {
            Self::Body(body) => body.thread_body_for_scope(scope),
            Self::ElseIf(_) => None,
        }
    }
}

/// Statement-form `if` payload.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirIfStmt {
    condition: ExprId,
    then_body: HirContextualStmtBody,
    else_branch: Option<HirConditionalElseBranch>,
}

impl HirIfStmt {
    pub(crate) fn try_new(
        condition: ExprId,
        then_body: HirContextualStmtBody,
        else_branch: Option<HirConditionalElseBranch>,
    ) -> Result<Self, HirThreadStmtInvariantError> {
        let value = Self {
            condition,
            then_body,
            else_branch,
        };
        value.validate_module(value.then_body.scope().module())?;
        Ok(value)
    }

    pub const fn condition(&self) -> ExprId {
        self.condition
    }

    pub const fn then_body(&self) -> &HirContextualStmtBody {
        &self.then_body
    }

    pub const fn then_scope(&self) -> ScopeId {
        self.then_body.scope()
    }

    pub const fn else_branch(&self) -> Option<&HirConditionalElseBranch> {
        self.else_branch.as_ref()
    }

    pub(crate) fn thread_body_for_scope(&self, scope: ScopeId) -> Option<&HirThreadBody> {
        self.then_body
            .thread_body_for_scope(scope)
            .or_else(|| self.else_branch.as_ref()?.thread_body_for_scope(scope))
    }

    pub(crate) fn validate_module(
        &self,
        expected: HirModuleId,
    ) -> Result<(), HirThreadStmtInvariantError> {
        validate_module(expected, self.condition.module())?;
        self.then_body.validate_module(expected)?;
        if let Some(branch) = &self.else_branch {
            branch.validate_module(expected)?;
        }
        Ok(())
    }
}

/// Statement-form `if let` payload.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirIfLetStmt {
    pattern: PatternId,
    scrutinee: ExprId,
    guard: Option<ExprId>,
    then_body: HirContextualStmtBody,
    locals: Box<[LocalId]>,
    else_branch: Option<HirConditionalElseBranch>,
}

impl HirIfLetStmt {
    pub(crate) fn try_new(
        pattern: PatternId,
        scrutinee: ExprId,
        guard: Option<ExprId>,
        then_body: HirContextualStmtBody,
        locals: Box<[LocalId]>,
        else_branch: Option<HirConditionalElseBranch>,
    ) -> Result<Self, HirThreadStmtInvariantError> {
        let value = Self {
            pattern,
            scrutinee,
            guard,
            then_body,
            locals,
            else_branch,
        };
        value.validate_module(value.then_body.scope().module())?;
        reject_duplicate_ids(value.locals.iter().copied())?;
        Ok(value)
    }

    pub const fn pattern(&self) -> PatternId {
        self.pattern
    }

    pub const fn scrutinee(&self) -> ExprId {
        self.scrutinee
    }

    pub const fn guard(&self) -> Option<ExprId> {
        self.guard
    }

    pub fn locals(&self) -> &[LocalId] {
        &self.locals
    }

    pub const fn then_body(&self) -> &HirContextualStmtBody {
        &self.then_body
    }

    pub const fn then_scope(&self) -> ScopeId {
        self.then_body.scope()
    }

    pub const fn else_branch(&self) -> Option<&HirConditionalElseBranch> {
        self.else_branch.as_ref()
    }

    pub(crate) fn thread_body_for_scope(&self, scope: ScopeId) -> Option<&HirThreadBody> {
        self.then_body
            .thread_body_for_scope(scope)
            .or_else(|| self.else_branch.as_ref()?.thread_body_for_scope(scope))
    }

    pub(crate) fn validate_module(
        &self,
        expected: HirModuleId,
    ) -> Result<(), HirThreadStmtInvariantError> {
        validate_module(expected, self.pattern.module())?;
        validate_module(expected, self.scrutinee.module())?;
        validate_optional_expr(expected, self.guard)?;
        validate_locals(expected, &self.locals)?;
        self.then_body.validate_module(expected)?;
        if let Some(branch) = &self.else_branch {
            branch.validate_module(expected)?;
        }
        Ok(())
    }
}

/// Body of one statement-form match arm.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirStmtMatchArmBody {
    Expression(ExprId),
    Body(HirContextualStmtBody),
}

impl HirStmtMatchArmBody {
    pub(crate) fn thread_body_for_scope(&self, scope: ScopeId) -> Option<&HirThreadBody> {
        match self {
            Self::Expression(_) => None,
            Self::Body(body) => body.thread_body_for_scope(scope),
        }
    }

    fn validate_module(&self, expected: HirModuleId) -> Result<(), HirThreadStmtInvariantError> {
        match self {
            Self::Expression(expression) => validate_module(expected, expression.module()),
            Self::Body(body) => body.validate_module(expected),
        }
    }
}

/// One source-ordered statement match arm.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirStmtMatchArm {
    scope: ScopeId,
    pattern: PatternId,
    guard: Option<ExprId>,
    body: HirStmtMatchArmBody,
    locals: Box<[LocalId]>,
}

impl HirStmtMatchArm {
    pub(crate) fn try_new(
        scope: ScopeId,
        pattern: PatternId,
        guard: Option<ExprId>,
        body: HirStmtMatchArmBody,
        locals: Box<[LocalId]>,
    ) -> Result<Self, HirThreadStmtInvariantError> {
        let value = Self {
            scope,
            pattern,
            guard,
            body,
            locals,
        };
        value.validate_module(scope.module())?;
        if matches!(&value.body, HirStmtMatchArmBody::Body(body) if body.scope() != scope) {
            return Err(HirThreadStmtInvariantError::MismatchedBodyScope);
        }
        reject_duplicate_ids(value.locals.iter().copied())?;
        Ok(value)
    }

    pub const fn scope(&self) -> ScopeId {
        self.scope
    }

    pub const fn pattern(&self) -> PatternId {
        self.pattern
    }

    pub const fn guard(&self) -> Option<ExprId> {
        self.guard
    }

    pub fn locals(&self) -> &[LocalId] {
        &self.locals
    }

    pub const fn body(&self) -> &HirStmtMatchArmBody {
        &self.body
    }

    pub(crate) fn thread_body_for_scope(&self, scope: ScopeId) -> Option<&HirThreadBody> {
        self.body.thread_body_for_scope(scope)
    }

    fn validate_module(&self, expected: HirModuleId) -> Result<(), HirThreadStmtInvariantError> {
        validate_module(expected, self.scope.module())?;
        validate_module(expected, self.pattern.module())?;
        validate_optional_expr(expected, self.guard)?;
        validate_locals(expected, &self.locals)?;
        self.body.validate_module(expected)
    }
}

/// Statement-form `match` payload.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirMatchStmt {
    scrutinee: ExprId,
    arms: Box<[HirStmtMatchArm]>,
}

impl HirMatchStmt {
    pub(crate) fn try_new(
        scrutinee: ExprId,
        arms: Box<[HirStmtMatchArm]>,
    ) -> Result<Self, HirThreadStmtInvariantError> {
        for arm in &arms {
            arm.validate_module(scrutinee.module())?;
        }
        reject_duplicate_ids(arms.iter().map(HirStmtMatchArm::scope))?;
        Ok(Self { scrutinee, arms })
    }

    pub const fn scrutinee(&self) -> ExprId {
        self.scrutinee
    }

    pub fn arms(&self) -> &[HirStmtMatchArm] {
        &self.arms
    }

    pub(crate) fn thread_body_for_scope(&self, scope: ScopeId) -> Option<&HirThreadBody> {
        self.arms
            .iter()
            .find_map(|arm| arm.thread_body_for_scope(scope))
    }

    pub(crate) fn validate_module(
        &self,
        expected: HirModuleId,
    ) -> Result<(), HirThreadStmtInvariantError> {
        validate_module(expected, self.scrutinee.module())?;
        for arm in &self.arms {
            arm.validate_module(expected)?;
        }
        Ok(())
    }
}

/// Statement-form `while` payload.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirWhileStmt {
    condition: ExprId,
    body: HirContextualStmtBody,
}

impl HirWhileStmt {
    pub(crate) fn try_new(
        condition: ExprId,
        body: HirContextualStmtBody,
    ) -> Result<Self, HirThreadStmtInvariantError> {
        validate_module(body.scope().module(), condition.module())?;
        Ok(Self { condition, body })
    }

    pub const fn condition(&self) -> ExprId {
        self.condition
    }

    pub const fn body(&self) -> &HirContextualStmtBody {
        &self.body
    }

    pub(crate) fn thread_body_for_scope(&self, scope: ScopeId) -> Option<&HirThreadBody> {
        self.body.thread_body_for_scope(scope)
    }

    pub(crate) fn validate_module(
        &self,
        expected: HirModuleId,
    ) -> Result<(), HirThreadStmtInvariantError> {
        validate_module(expected, self.condition.module())?;
        self.body.validate_module(expected)
    }
}

/// Statement-form `while let` payload.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirWhileLetStmt {
    pattern: PatternId,
    scrutinee: ExprId,
    guard: Option<ExprId>,
    locals: Box<[LocalId]>,
    body: HirContextualStmtBody,
}

impl HirWhileLetStmt {
    pub(crate) fn try_new(
        pattern: PatternId,
        scrutinee: ExprId,
        guard: Option<ExprId>,
        locals: Box<[LocalId]>,
        body: HirContextualStmtBody,
    ) -> Result<Self, HirThreadStmtInvariantError> {
        let value = Self {
            pattern,
            scrutinee,
            guard,
            locals,
            body,
        };
        value.validate_module(value.body.scope().module())?;
        reject_duplicate_ids(value.locals.iter().copied())?;
        Ok(value)
    }

    pub const fn pattern(&self) -> PatternId {
        self.pattern
    }

    pub const fn scrutinee(&self) -> ExprId {
        self.scrutinee
    }

    pub const fn guard(&self) -> Option<ExprId> {
        self.guard
    }

    pub fn locals(&self) -> &[LocalId] {
        &self.locals
    }

    pub const fn body(&self) -> &HirContextualStmtBody {
        &self.body
    }

    pub(crate) fn thread_body_for_scope(&self, scope: ScopeId) -> Option<&HirThreadBody> {
        self.body.thread_body_for_scope(scope)
    }

    pub(crate) fn validate_module(
        &self,
        expected: HirModuleId,
    ) -> Result<(), HirThreadStmtInvariantError> {
        validate_module(expected, self.pattern.module())?;
        validate_module(expected, self.scrutinee.module())?;
        validate_optional_expr(expected, self.guard)?;
        validate_locals(expected, &self.locals)?;
        self.body.validate_module(expected)
    }
}

/// Statement-form `for` payload including its deterministic synthetic values.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirForStmt {
    source: ExprId,
    iterator: ExprId,
    next_value: ExprId,
    pattern: PatternId,
    locals: Box<[LocalId]>,
    body: HirContextualStmtBody,
}

impl HirForStmt {
    pub(crate) fn try_new(
        source: ExprId,
        iterator: ExprId,
        next_value: ExprId,
        pattern: PatternId,
        locals: Box<[LocalId]>,
        body: HirContextualStmtBody,
    ) -> Result<Self, HirThreadStmtInvariantError> {
        let value = Self {
            source,
            iterator,
            next_value,
            pattern,
            locals,
            body,
        };
        value.validate_module(value.body.scope().module())?;
        reject_duplicate_ids(value.locals.iter().copied())?;
        Ok(value)
    }

    pub const fn source(&self) -> ExprId {
        self.source
    }

    pub const fn iterator(&self) -> ExprId {
        self.iterator
    }

    pub const fn next_value(&self) -> ExprId {
        self.next_value
    }

    pub const fn pattern(&self) -> PatternId {
        self.pattern
    }

    pub fn locals(&self) -> &[LocalId] {
        &self.locals
    }

    pub const fn body(&self) -> &HirContextualStmtBody {
        &self.body
    }

    pub(crate) fn thread_body_for_scope(&self, scope: ScopeId) -> Option<&HirThreadBody> {
        self.body.thread_body_for_scope(scope)
    }

    pub(crate) fn validate_module(
        &self,
        expected: HirModuleId,
    ) -> Result<(), HirThreadStmtInvariantError> {
        for expression in [self.source, self.iterator, self.next_value] {
            validate_module(expected, expression.module())?;
        }
        validate_module(expected, self.pattern.module())?;
        validate_locals(expected, &self.locals)?;
        self.body.validate_module(expected)
    }
}

/// Required Select binding identity or exact typed name recovery.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirSelectBindingLocal {
    Resolved(LocalId),
    Missing,
    Invalid(HirNameInvariantError),
}

impl HirSelectBindingLocal {
    pub const fn resolved(&self) -> Option<LocalId> {
        match self {
            Self::Resolved(local) => Some(*local),
            Self::Missing | Self::Invalid(_) => None,
        }
    }

    fn validate_module(&self, expected: HirModuleId) -> Result<(), HirThreadStmtInvariantError> {
        match self {
            Self::Resolved(local) => validate_module(expected, local.module()),
            Self::Missing | Self::Invalid(_) => Ok(()),
        }
    }
}

/// Semantic head of one source-ordered Select statement branch.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirSelectBranchHead {
    Bind {
        binding: HirSelectBindingLocal,
        source: ExprId,
    },
    Frame {
        pattern: PatternId,
        locals: Box<[LocalId]>,
    },
    Event {
        pattern: PatternId,
        locals: Box<[LocalId]>,
    },
    Recovered,
}

impl HirSelectBranchHead {
    fn validate_module(&self, expected: HirModuleId) -> Result<(), HirThreadStmtInvariantError> {
        match self {
            Self::Bind { binding, source } => {
                binding.validate_module(expected)?;
                validate_module(expected, source.module())
            }
            Self::Frame { pattern, locals } | Self::Event { pattern, locals } => {
                validate_module(expected, pattern.module())?;
                validate_locals(expected, locals)?;
                reject_duplicate_ids(locals.iter().copied())
            }
            Self::Recovered => Ok(()),
        }
    }
}

/// One source-ordered Select branch and its isolated nested body.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirSelectBranch {
    head: HirSelectBranchHead,
    body: HirContextualStmtBody,
}

impl HirSelectBranch {
    pub(crate) fn try_new(
        head: HirSelectBranchHead,
        body: HirContextualStmtBody,
    ) -> Result<Self, HirThreadStmtInvariantError> {
        let value = Self { head, body };
        value.validate_module(value.body.scope().module())?;
        Ok(value)
    }

    pub const fn head(&self) -> &HirSelectBranchHead {
        &self.head
    }

    pub const fn body(&self) -> &HirContextualStmtBody {
        &self.body
    }

    pub(crate) fn thread_body_for_scope(&self, scope: ScopeId) -> Option<&HirThreadBody> {
        self.body.thread_body_for_scope(scope)
    }

    fn validate_module(&self, expected: HirModuleId) -> Result<(), HirThreadStmtInvariantError> {
        self.head.validate_module(expected)?;
        self.body.validate_module(expected)
    }
}

/// Unary or branch-block Select statement payload.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirSelectStmt {
    Operand(ExprId),
    Branches {
        scope: ScopeId,
        branches: Box<[HirSelectBranch]>,
    },
}

impl HirSelectStmt {
    pub(crate) const fn operand(expression: ExprId) -> Self {
        Self::Operand(expression)
    }

    pub(crate) fn try_branches(
        scope: ScopeId,
        branches: Box<[HirSelectBranch]>,
    ) -> Result<Self, HirThreadStmtInvariantError> {
        for branch in &branches {
            branch.validate_module(scope.module())?;
        }
        reject_duplicate_ids(branches.iter().map(|branch| branch.body.scope()))?;
        Ok(Self::Branches { scope, branches })
    }

    pub const fn scope(&self) -> Option<ScopeId> {
        match self {
            Self::Operand(_) => None,
            Self::Branches { scope, .. } => Some(*scope),
        }
    }

    pub fn branches(&self) -> &[HirSelectBranch] {
        match self {
            Self::Operand(_) => &[],
            Self::Branches { branches, .. } => branches,
        }
    }

    pub(crate) fn validate_module(
        &self,
        expected: HirModuleId,
    ) -> Result<(), HirThreadStmtInvariantError> {
        match self {
            Self::Operand(expression) => validate_module(expected, expression.module()),
            Self::Branches { scope, branches } => {
                validate_module(expected, scope.module())?;
                for branch in branches {
                    branch.validate_module(expected)?;
                }
                Ok(())
            }
        }
    }

    pub(crate) fn thread_body_for_scope(&self, scope: ScopeId) -> Option<&HirThreadBody> {
        match self {
            Self::Operand(_) => None,
            Self::Branches { branches, .. } => branches
                .iter()
                .find_map(|branch| branch.thread_body_for_scope(scope)),
        }
    }
}

/// Canonical locale or typed recovery from the same required locale slot.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirSourceLocaleValue {
    Resolved(LocaleTag),
    Recovered(HirSourceLocaleIssue),
}

/// Typed locale recovery without retaining a raw source spelling.
#[derive(Clone, Copy, Debug, Eq, Error, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirSourceLocaleIssue {
    #[error("source locale is missing its required LocaleTag")]
    Missing,
    #[error("source locale does not contain one canonical LocaleTag")]
    Invalid,
}

/// `source locale LocaleTag { ... }` statement payload.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirSourceLocaleStmt {
    locale: HirSourceLocaleValue,
    body: HirContextualStmtBody,
}

impl HirSourceLocaleStmt {
    pub(crate) fn try_new(
        locale: HirSourceLocaleValue,
        body: HirContextualStmtBody,
    ) -> Result<Self, HirThreadStmtInvariantError> {
        body.validate_module(body.scope().module())?;
        Ok(Self { locale, body })
    }

    pub const fn locale(&self) -> &HirSourceLocaleValue {
        &self.locale
    }

    pub const fn body(&self) -> &HirContextualStmtBody {
        &self.body
    }

    pub(crate) fn thread_body_for_scope(&self, scope: ScopeId) -> Option<&HirThreadBody> {
        self.body.thread_body_for_scope(scope)
    }

    pub(crate) fn validate_module(
        &self,
        expected: HirModuleId,
    ) -> Result<(), HirThreadStmtInvariantError> {
        self.body.validate_module(expected)
    }
}

/// Named or anonymous lexical Scope statement payload.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirScopeStmt {
    name: Option<HirName>,
    body: HirContextualStmtBody,
}

impl HirScopeStmt {
    pub(crate) fn try_new(
        name: Option<HirName>,
        body: HirContextualStmtBody,
    ) -> Result<Self, HirThreadStmtInvariantError> {
        body.validate_module(body.scope().module())?;
        Ok(Self { name, body })
    }

    pub const fn name(&self) -> Option<&HirName> {
        self.name.as_ref()
    }

    pub const fn body(&self) -> &HirContextualStmtBody {
        &self.body
    }

    pub(crate) fn thread_body_for_scope(&self, scope: ScopeId) -> Option<&HirThreadBody> {
        self.body.thread_body_for_scope(scope)
    }

    pub(crate) fn validate_module(
        &self,
        expected: HirModuleId,
    ) -> Result<(), HirThreadStmtInvariantError> {
        self.body.validate_module(expected)
    }
}

/// Typed Flow-reference target of an Include statement.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirIncludeStmt {
    target: HirIdRefValue,
}

impl HirIncludeStmt {
    pub(crate) const fn new(target: HirIdRefValue) -> Self {
        Self { target }
    }

    pub const fn target(&self) -> &HirIdRefValue {
        &self.target
    }
}

/// Source-independent recovery class for the F04-F15 statement payloads.
#[derive(Clone, Copy, Debug, Eq, Error, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirThreadStmtRecoveryIssue {
    #[error("statement child {role:?} contains recovery")]
    RecoveredChild { role: HirThreadStmtChildRole },
    #[error("statement body {role:?} is missing")]
    MissingBody { role: HirThreadStmtBodyRole },
    #[error("statement body {role:?} is missing its closing delimiter")]
    UnclosedBody { role: HirThreadStmtBodyRole },
    #[error("Match statement has no complete arm")]
    EmptyMatch,
    #[error("Select statement has no complete branch")]
    EmptySelect,
    #[error("Select branch {ordinal} has a recovered head")]
    RecoveredSelectBranch { ordinal: u32 },
    #[error("source locale is invalid")]
    InvalidSourceLocale(HirSourceLocaleIssue),
    #[error("Scope statement name is invalid")]
    InvalidScopeName(HirNameInvariantError),
    #[error("Include target is invalid")]
    InvalidIncludeTarget(HirIdRefIssue),
}

/// Semantic child selected as the primary recovery owner.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirThreadStmtChildRole {
    Condition,
    Pattern,
    Scrutinee,
    Guard,
    Source,
    Iterator,
    NextValue,
    SelectBinding { branch: u32 },
    SelectSource { branch: u32 },
    SelectPattern { branch: u32 },
    SelectBranchStatement { branch: u32, statement: u32 },
    Locale,
    IncludeTarget,
}

/// Required nested body selected as the primary recovery owner.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirThreadStmtBodyRole {
    Then,
    Else,
    Match,
    MatchArm { ordinal: u32 },
    While,
    WhileLet,
    For,
    Select,
    SelectBranch { ordinal: u32 },
    SourceLocale,
    Scope,
}

/// Structural failure detected before one statement payload is published.
#[derive(Clone, Copy, Debug, Eq, Error, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum HirThreadStmtInvariantError {
    #[error("statement child belongs to module {actual:?}, expected {expected:?}")]
    ForeignChild {
        expected: HirModuleId,
        actual: HirModuleId,
    },
    #[error("one statement payload repeats a child identity")]
    DuplicateChild,
    #[error("one contextual match-arm body does not use its arm scope")]
    MismatchedBodyScope,
    #[error("an Await branch kind disagrees with its pattern and locals")]
    InvalidAwaitBranchShape,
}

fn validate_optional_expr(
    expected: HirModuleId,
    expression: Option<ExprId>,
) -> Result<(), HirThreadStmtInvariantError> {
    expression.map_or(Ok(()), |expression| {
        validate_module(expected, expression.module())
    })
}

fn validate_statements(
    expected: HirModuleId,
    statements: &[StmtId],
) -> Result<(), HirThreadStmtInvariantError> {
    for statement in statements {
        validate_module(expected, statement.module())?;
    }
    Ok(())
}

fn validate_locals(
    expected: HirModuleId,
    locals: &[LocalId],
) -> Result<(), HirThreadStmtInvariantError> {
    for local in locals {
        validate_module(expected, local.module())?;
    }
    Ok(())
}

fn validate_module(
    expected: HirModuleId,
    actual: HirModuleId,
) -> Result<(), HirThreadStmtInvariantError> {
    if expected == actual {
        Ok(())
    } else {
        Err(HirThreadStmtInvariantError::ForeignChild { expected, actual })
    }
}

pub(crate) fn reject_duplicate_ids<I>(
    ids: impl IntoIterator<Item = I>,
) -> Result<(), HirThreadStmtInvariantError>
where
    I: Ord,
{
    let mut unique = BTreeSet::new();
    if ids.into_iter().all(|id| unique.insert(id)) {
        Ok(())
    } else {
        Err(HirThreadStmtInvariantError::DuplicateChild)
    }
}

#[cfg(test)]
mod tests;
