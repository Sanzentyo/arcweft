//! One namespace projection for statement and expression lexical scopes.

use crate::arena::HirArenaError;
use crate::expr::{HirExpr, HirExprKind, HirNamedBlockName};
use crate::identity::{ExprId, IdResolveError, ScopeId, StmtId};
use crate::leaf::HirName;
use crate::scope::{HirScope, HirScopeOwner};
use crate::slot::HirSlotError;
use crate::source_index::{HirExprSourceRole, HirSourceQuery, HirStmtSourceRole};
use crate::stmt::{HirStmt, HirStmtKind};
use thiserror::Error;

use super::HirModule;

/// Exact accepted name and source owner of one named lexical scope.
#[derive(Clone, Debug)]
pub struct HirNamedScope<'module> {
    name: &'module HirName,
    declaration: HirSourceQuery,
}

impl<'module> HirNamedScope<'module> {
    pub const fn name(&self) -> &'module HirName {
        self.name
    }
    pub const fn declaration(&self) -> &HirSourceQuery {
        &self.declaration
    }
}

#[derive(Debug, Error)]
pub enum HirScopeNamespaceError {
    #[error("scope namespace owner cannot be resolved: {0}")]
    Resolve(#[from] IdResolveError),
    #[error("scope namespace prepared arena violates its module invariant")]
    InvalidArena,
    #[error("scope namespace owner does not own the selected scope")]
    WrongScope,
    #[error("scope namespace name is recovered")]
    RecoveredName,
}

impl From<HirArenaError> for HirScopeNamespaceError {
    fn from(error: HirArenaError) -> Self {
        match error {
            HirArenaError::Slot(HirSlotError::Resolve(error)) => Self::Resolve(error),
            _ => Self::InvalidArena,
        }
    }
}

impl HirModule {
    /// Projects a name only when the exact lexical scope is owned by a named
    /// Scope statement or NamedBlock expression. Anonymous scopes add no segment.
    pub fn scope_namespace(
        &self,
        scope: ScopeId,
    ) -> Result<Option<HirNamedScope<'_>>, HirScopeNamespaceError> {
        let resolve = HirScopeNamespaceError::Resolve;
        project_namespace(
            scope,
            self.resolve_scope(scope).map_err(resolve)?,
            |owner| self.resolve_expr(owner).map_err(resolve),
            |owner| self.resolve_stmt(owner).map_err(resolve),
        )
    }

    pub(crate) fn prepared_scope_namespace(
        &self,
        scope: ScopeId,
    ) -> Result<Option<HirNamedScope<'_>>, HirScopeNamespaceError> {
        project_namespace(
            scope,
            self.arenas.scopes.resolve_prepared(&self.slots, scope)?,
            |owner| {
                self.arenas
                    .expressions
                    .resolve_prepared(&self.slots, owner)
                    .map_err(Into::into)
            },
            |owner| {
                self.arenas
                    .statements
                    .resolve_prepared(&self.slots, owner)
                    .map_err(Into::into)
            },
        )
    }
}

fn project_namespace<'module>(
    scope: ScopeId,
    node: &HirScope,
    expression: impl Fn(ExprId) -> Result<&'module HirExpr, HirScopeNamespaceError>,
    statement: impl Fn(StmtId) -> Result<&'module HirStmt, HirScopeNamespaceError>,
) -> Result<Option<HirNamedScope<'module>>, HirScopeNamespaceError> {
    match *node.owner() {
        HirScopeOwner::Expr(owner) => {
            if let HirExprKind::NamedBlock(block) = expression(owner)?.kind() {
                if block.scope() != scope {
                    return Err(HirScopeNamespaceError::WrongScope);
                }
                let HirNamedBlockName::Resolved(name) = block.name() else {
                    return Err(HirScopeNamespaceError::RecoveredName);
                };
                return Ok(Some(HirNamedScope {
                    name,
                    declaration: HirSourceQuery::Expr {
                        owner,
                        role: HirExprSourceRole::Name,
                    },
                }));
            }
        }
        HirScopeOwner::Stmt(owner) => {
            if let HirStmtKind::Scope(block) = statement(owner)?.kind() {
                if block.body().scope() != scope {
                    return Err(HirScopeNamespaceError::WrongScope);
                }
                return Ok(block.name().map(|name| HirNamedScope {
                    name,
                    declaration: HirSourceQuery::Stmt {
                        owner,
                        role: HirStmtSourceRole::Whole,
                    },
                }));
            }
        }
        HirScopeOwner::Module(_) | HirScopeOwner::Item(_) => {}
    }
    Ok(None)
}
