//! Borrowed selection grammar shared by expression and statement Match owners.

use crate::expr::{HirExprKind, HirMatchArm, HirMatchExpr};
use crate::identity::{ExprId, HirModuleId, LocalId, PatternId, StmtId};
use crate::module::HirModule;
use crate::stmt::{
    HirContextualStmtBody, HirMatchStmt, HirStmtKind, HirStmtMatchArm, HirStmtMatchArmBody,
};

/// A generation-qualified Match source. Construction does not admit its kind.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirMatchOwner {
    Expression(ExprId),
    Statement(StmtId),
}

impl From<ExprId> for HirMatchOwner {
    fn from(value: ExprId) -> Self {
        Self::Expression(value)
    }
}

impl From<StmtId> for HirMatchOwner {
    fn from(value: StmtId) -> Self {
        Self::Statement(value)
    }
}

impl From<HirMatchOwner> for super::HirSemanticPathOwnerId {
    fn from(value: HirMatchOwner) -> Self {
        match value {
            HirMatchOwner::Expression(owner) => owner.into(),
            HirMatchOwner::Statement(owner) => owner.into(),
        }
    }
}

#[derive(Clone, Copy, Debug, thiserror::Error)]
#[error("HIR owner {owner:?} is not a live Match in this module")]
pub struct HirMatchOwnerError {
    pub owner: HirMatchOwner,
}

impl HirMatchOwner {
    pub const fn module(self) -> HirModuleId {
        match self {
            Self::Expression(owner) => owner.module(),
            Self::Statement(owner) => owner.module(),
        }
    }

    /// Resolves the actual source payload; no child or binding inventory is copied.
    pub fn resolve(self, module: &HirModule) -> Result<HirMatchView<'_>, HirMatchOwnerError> {
        let invalid = || HirMatchOwnerError { owner: self };
        match self {
            Self::Expression(owner) => {
                match module.resolve_expr(owner).map_err(|_| invalid())?.kind() {
                    HirExprKind::Match(value) => Ok(HirMatchView::Expression(value)),
                    _ => Err(invalid()),
                }
            }
            Self::Statement(owner) => {
                match module.resolve_stmt(owner).map_err(|_| invalid())?.kind() {
                    HirStmtKind::Match(value) => Ok(HirMatchView::Statement(value)),
                    _ => Err(invalid()),
                }
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum HirMatchView<'a> {
    Expression(&'a HirMatchExpr),
    Statement(&'a HirMatchStmt),
}

impl<'a> HirMatchView<'a> {
    pub const fn scrutinee(self) -> ExprId {
        match self {
            Self::Expression(value) => value.scrutinee(),
            Self::Statement(value) => value.scrutinee(),
        }
    }
    pub fn arms(self) -> HirMatchArms<'a> {
        match self {
            Self::Expression(value) => HirMatchArms::Expression(value.arms().iter()),
            Self::Statement(value) => HirMatchArms::Statement(value.arms().iter()),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum HirMatchArmBodyView<'a> {
    Value(ExprId),
    Body(&'a HirContextualStmtBody),
}

#[derive(Clone, Copy, Debug)]
pub enum HirMatchArmView<'a> {
    Expression(&'a HirMatchArm),
    Statement(&'a HirStmtMatchArm),
}

impl<'a> HirMatchArmView<'a> {
    pub const fn pattern(self) -> PatternId {
        match self {
            Self::Expression(value) => value.pattern(),
            Self::Statement(value) => value.pattern(),
        }
    }
    pub const fn guard(self) -> Option<ExprId> {
        match self {
            Self::Expression(value) => value.guard(),
            Self::Statement(value) => value.guard(),
        }
    }
    pub fn locals(self) -> &'a [LocalId] {
        match self {
            Self::Expression(value) => value.locals(),
            Self::Statement(value) => value.locals(),
        }
    }
    pub const fn body(self) -> HirMatchArmBodyView<'a> {
        match self {
            Self::Expression(value) => HirMatchArmBodyView::Value(value.value()),
            Self::Statement(value) => match value.body() {
                HirStmtMatchArmBody::Expression(value) => HirMatchArmBodyView::Value(*value),
                HirStmtMatchArmBody::Body(value) => HirMatchArmBodyView::Body(value),
            },
        }
    }
}

/// Exact-size source-ordered arm traversal without an allocated adapter table.
pub enum HirMatchArms<'a> {
    Expression(std::slice::Iter<'a, HirMatchArm>),
    Statement(std::slice::Iter<'a, HirStmtMatchArm>),
}

impl<'a> Iterator for HirMatchArms<'a> {
    type Item = HirMatchArmView<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Expression(values) => values.next().map(HirMatchArmView::Expression),
            Self::Statement(values) => values.next().map(HirMatchArmView::Statement),
        }
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len(), Some(self.len()))
    }
}

impl ExactSizeIterator for HirMatchArms<'_> {
    fn len(&self) -> usize {
        match self {
            Self::Expression(values) => values.len(),
            Self::Statement(values) => values.len(),
        }
    }
}
