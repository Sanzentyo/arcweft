//! Closure, block, conditional, and match expression payload records.

use std::collections::BTreeSet;

use super::{
    HirExprInvariantError, validate_expr, validate_module, validate_optional_expr,
    validate_optional_type, validate_pattern, validate_scope, validate_statements,
};
use crate::identity::{
    CaptureId, ExprId, HirModuleId, LocalId, PatternId, ScopeId, StmtId, TypeId,
};
use crate::leaf::{HirName, HirNameInvariantError};

/// Closure scope, parameters, result annotation, body, and captures.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirClosureExpr {
    scope: ScopeId,
    parameters: Box<[HirClosureParameter]>,
    result_type: Option<TypeId>,
    body: ExprId,
    captures: Box<[CaptureId]>,
}

impl HirClosureExpr {
    pub(crate) const fn new(
        scope: ScopeId,
        parameters: Box<[HirClosureParameter]>,
        result_type: Option<TypeId>,
        body: ExprId,
        captures: Box<[CaptureId]>,
    ) -> Self {
        Self {
            scope,
            parameters,
            result_type,
            body,
            captures,
        }
    }

    pub const fn scope(&self) -> ScopeId {
        self.scope
    }

    pub fn parameters(&self) -> &[HirClosureParameter] {
        &self.parameters
    }

    pub const fn result_type(&self) -> Option<TypeId> {
        self.result_type
    }

    pub const fn body(&self) -> ExprId {
        self.body
    }

    pub fn captures(&self) -> &[CaptureId] {
        &self.captures
    }

    pub(super) fn validate_module(
        &self,
        expected: HirModuleId,
    ) -> Result<(), HirExprInvariantError> {
        validate_scope(expected, self.scope)?;
        for parameter in &self.parameters {
            parameter.validate_module(expected)?;
        }
        validate_optional_type(expected, self.result_type)?;
        validate_expr(expected, self.body)?;
        for capture in &self.captures {
            validate_module(expected, capture.module())?;
        }
        Ok(())
    }
}

/// Ordinary block expression with one explicit or synthetic tail.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirBlockExpr {
    scope: ScopeId,
    statements: Box<[StmtId]>,
    tail: ExprId,
}

impl HirBlockExpr {
    pub(crate) const fn new(scope: ScopeId, statements: Box<[StmtId]>, tail: ExprId) -> Self {
        Self {
            scope,
            statements,
            tail,
        }
    }

    pub const fn scope(&self) -> ScopeId {
        self.scope
    }

    pub fn statements(&self) -> &[StmtId] {
        &self.statements
    }

    pub const fn tail(&self) -> ExprId {
        self.tail
    }
}

/// Result, task, sequence, or stream computation block.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirComputationBlockExpr {
    kind: HirComputationBlockKind,
    scope: ScopeId,
    statements: Box<[StmtId]>,
    tail: ExprId,
}

impl HirComputationBlockExpr {
    pub(crate) const fn new(
        kind: HirComputationBlockKind,
        scope: ScopeId,
        statements: Box<[StmtId]>,
        tail: ExprId,
    ) -> Self {
        Self {
            kind,
            scope,
            statements,
            tail,
        }
    }

    pub const fn kind(&self) -> HirComputationBlockKind {
        self.kind
    }

    pub const fn scope(&self) -> ScopeId {
        self.scope
    }

    pub fn statements(&self) -> &[StmtId] {
        &self.statements
    }

    pub const fn tail(&self) -> ExprId {
        self.tail
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirComputationBlockKind {
    Result,
    Option,
    Seq,
    Stream,
}

/// Named block expression with one explicit or synthetic tail.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirNamedBlockExpr {
    name: HirNamedBlockName,
    scope: ScopeId,
    statements: Box<[StmtId]>,
    tail: ExprId,
}

/// Value-producing loop expression.
///
/// The loop owns one ordinary value block. Break/continue statements remain
/// in that block's statement inventory; semantic analysis resolves their
/// exact loop target through this expression owner.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirLoopExpr {
    scope: ScopeId,
    statements: Box<[StmtId]>,
    tail: ExprId,
}

impl HirLoopExpr {
    pub(crate) const fn new(scope: ScopeId, statements: Box<[StmtId]>, tail: ExprId) -> Self {
        Self {
            scope,
            statements,
            tail,
        }
    }

    pub const fn scope(&self) -> ScopeId {
        self.scope
    }

    pub fn statements(&self) -> &[StmtId] {
        &self.statements
    }

    pub const fn tail(&self) -> ExprId {
        self.tail
    }

    pub(super) fn validate_module(
        &self,
        expected: HirModuleId,
    ) -> Result<(), HirExprInvariantError> {
        validate_scope(expected, self.scope)?;
        validate_statements(expected, &self.statements)?;
        validate_expr(expected, self.tail)
    }
}

impl HirNamedBlockExpr {
    pub(crate) const fn new(
        name: HirNamedBlockName,
        scope: ScopeId,
        statements: Box<[StmtId]>,
        tail: ExprId,
    ) -> Self {
        Self {
            name,
            scope,
            statements,
            tail,
        }
    }

    pub const fn name(&self) -> &HirNamedBlockName {
        &self.name
    }

    pub const fn scope(&self) -> ScopeId {
        self.scope
    }

    pub fn statements(&self) -> &[StmtId] {
        &self.statements
    }

    pub const fn tail(&self) -> ExprId {
        self.tail
    }
}

/// Authored name state for a named block.
///
/// A missing name is deliberately absent: `scope { ... }` lowers as an
/// ordinary [`HirBlockExpr`]. Only an invalid-present spelling keeps the named
/// block family without fabricating a valid identifier.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirNamedBlockName {
    Resolved(HirName),
    InvalidPresent(HirNameInvariantError),
}

impl HirNamedBlockName {
    /// Returns the typed invalid-present issue retained by recovery.
    pub const fn recovery_issue(&self) -> Option<HirNameInvariantError> {
        match self {
            Self::Resolved(_) => None,
            Self::InvalidPresent(issue) => Some(*issue),
        }
    }
}

/// If expression whose else child is always explicit in HIR.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirIfExpr {
    condition: ExprId,
    then_branch: ExprId,
    else_branch: ExprId,
}

impl HirIfExpr {
    pub(crate) const fn new(condition: ExprId, then_branch: ExprId, else_branch: ExprId) -> Self {
        Self {
            condition,
            then_branch,
            else_branch,
        }
    }

    pub const fn condition(&self) -> ExprId {
        self.condition
    }

    pub const fn then_branch(&self) -> ExprId {
        self.then_branch
    }

    pub const fn else_branch(&self) -> ExprId {
        self.else_branch
    }
}

/// If-let expression with one binding scope and explicit branches.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirIfLetExpr {
    scope: ScopeId,
    pattern: PatternId,
    scrutinee: ExprId,
    guard: Option<ExprId>,
    then_branch: ExprId,
    else_branch: ExprId,
}

impl HirIfLetExpr {
    pub(crate) const fn new(
        scope: ScopeId,
        pattern: PatternId,
        scrutinee: ExprId,
        guard: Option<ExprId>,
        then_branch: ExprId,
        else_branch: ExprId,
    ) -> Self {
        Self {
            scope,
            pattern,
            scrutinee,
            guard,
            then_branch,
            else_branch,
        }
    }

    pub const fn scope(&self) -> ScopeId {
        self.scope
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

    pub const fn then_branch(&self) -> ExprId {
        self.then_branch
    }

    pub const fn else_branch(&self) -> ExprId {
        self.else_branch
    }

    pub(super) fn validate_module(
        &self,
        expected: HirModuleId,
    ) -> Result<(), HirExprInvariantError> {
        validate_scope(expected, self.scope)?;
        validate_pattern(expected, self.pattern)?;
        validate_expr(expected, self.scrutinee)?;
        validate_optional_expr(expected, self.guard)?;
        validate_expr(expected, self.then_branch)?;
        validate_expr(expected, self.else_branch)
    }
}

/// Match expression and its source-ordered arms.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirMatchExpr {
    scrutinee: ExprId,
    arms: Box<[HirMatchArm]>,
}

impl HirMatchExpr {
    pub(crate) fn try_new(
        scrutinee: ExprId,
        arms: Box<[HirMatchArm]>,
    ) -> Result<Self, HirExprInvariantError> {
        let expected = scrutinee.module();
        let mut scopes = BTreeSet::new();
        for arm in &arms {
            arm.validate_module(expected)?;
            if !scopes.insert(arm.scope()) {
                return Err(HirExprInvariantError::DuplicateMatchArmScope { scope: arm.scope() });
            }
        }
        Ok(Self { scrutinee, arms })
    }

    pub const fn scrutinee(&self) -> ExprId {
        self.scrutinee
    }

    pub fn arms(&self) -> &[HirMatchArm] {
        &self.arms
    }

    pub(super) fn validate_module(
        &self,
        expected: HirModuleId,
    ) -> Result<(), HirExprInvariantError> {
        validate_expr(expected, self.scrutinee)?;
        for arm in &self.arms {
            arm.validate_module(expected)?;
        }
        Ok(())
    }
}

/// Generic expression recovery retained only for unclassifiable syntax.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirExprError {
    issue: HirGenericExprIssue,
}

impl HirExprError {
    pub(crate) const fn new(issue: HirGenericExprIssue) -> Self {
        Self { issue }
    }

    pub const fn issue(&self) -> HirGenericExprIssue {
        self.issue
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirGenericExprIssue {
    UnclassifiedSyntax,
    TransactionalChildFailure,
}

/// One closure parameter and its binding scope.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirClosureParameter {
    pattern: PatternId,
    ty: Option<TypeId>,
    local_scope: ScopeId,
}

impl HirClosureParameter {
    pub(crate) fn try_new(
        pattern: PatternId,
        ty: Option<TypeId>,
        local_scope: ScopeId,
    ) -> Result<Self, HirExprInvariantError> {
        let expected = local_scope.module();
        validate_pattern(expected, pattern)?;
        validate_optional_type(expected, ty)?;
        Ok(Self {
            pattern,
            ty,
            local_scope,
        })
    }

    pub const fn pattern(&self) -> PatternId {
        self.pattern
    }

    pub const fn ty(&self) -> Option<TypeId> {
        self.ty
    }

    pub const fn local_scope(&self) -> ScopeId {
        self.local_scope
    }

    pub(super) fn validate_module(
        &self,
        expected: HirModuleId,
    ) -> Result<(), HirExprInvariantError> {
        validate_pattern(expected, self.pattern)?;
        validate_optional_type(expected, self.ty)?;
        validate_scope(expected, self.local_scope)
    }
}

/// One source-ordered match arm and its distinct binding scope.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirMatchArm {
    scope: ScopeId,
    pattern: PatternId,
    guard: Option<ExprId>,
    value: ExprId,
    locals: Box<[LocalId]>,
}

impl HirMatchArm {
    pub(crate) fn try_new(
        scope: ScopeId,
        pattern: PatternId,
        guard: Option<ExprId>,
        value: ExprId,
        locals: Box<[LocalId]>,
    ) -> Result<Self, HirExprInvariantError> {
        let arm = Self {
            scope,
            pattern,
            guard,
            value,
            locals,
        };
        arm.validate_module(scope.module())?;
        Ok(arm)
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

    pub const fn value(&self) -> ExprId {
        self.value
    }

    pub fn locals(&self) -> &[LocalId] {
        &self.locals
    }

    pub(super) fn validate_module(
        &self,
        expected: HirModuleId,
    ) -> Result<(), HirExprInvariantError> {
        validate_scope(expected, self.scope)?;
        validate_pattern(expected, self.pattern)?;
        validate_optional_expr(expected, self.guard)?;
        validate_expr(expected, self.value)?;
        for local in &self.locals {
            validate_module(expected, local.module())?;
        }
        Ok(())
    }
}
