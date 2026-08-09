//! Thread expression and direct source-ordered flow-body records.

use std::collections::BTreeSet;

use super::{HirExprInvariantError, validate_scope};
use crate::identity::{
    CaptureId, ExprId, HirLimit, HirModuleId, ItemId, ScopeId, StmtId, SyntheticOwner,
};
use crate::leaf::HirName;

/// Closure-backed thread expression with a source-ordered flow body.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirThreadExpr {
    name: Option<HirName>,
    mode: HirThreadMode,
    body: HirThreadBody,
}

impl HirThreadExpr {
    pub(crate) fn new(name: Option<HirName>, mode: HirThreadMode, body: HirThreadBody) -> Self {
        Self { name, mode, body }
    }

    pub const fn name(&self) -> Option<&HirName> {
        self.name.as_ref()
    }

    pub const fn mode(&self) -> HirThreadMode {
        self.mode
    }

    pub const fn scope(&self) -> ScopeId {
        self.body.scope()
    }

    pub const fn body(&self) -> &HirThreadBody {
        &self.body
    }

    pub(super) fn validate_module(
        &self,
        expected: HirModuleId,
    ) -> Result<(), HirExprInvariantError> {
        validate_scope(expected, self.body.scope())?;
        self.body
            .validate_module(expected)
            .map_err(|actual| HirExprInvariantError::ForeignChild { expected, actual })
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirThreadMode {
    Attached,
    Detached,
}

/// Typed semantic owner of one shared Flow/Thread statement body.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirThreadBodyOwner {
    Flow(ItemId),
    ThreadExpression(ExprId),
    NestedScope(ScopeId),
}

impl HirThreadBodyOwner {
    pub(crate) const fn module(self) -> HirModuleId {
        match self {
            Self::Flow(owner) => owner.module(),
            Self::ThreadExpression(owner) => owner.module(),
            Self::NestedScope(owner) => owner.module(),
        }
    }
}

/// Ordered statement-only body shared by Flow, Thread, and nested flow scopes.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirThreadBody {
    scope: ScopeId,
    items: Box<[HirThreadFlowItem]>,
}

impl HirThreadBody {
    pub(crate) fn try_new(
        owner: HirThreadBodyOwner,
        scope: ScopeId,
        items: Box<[HirThreadFlowItem]>,
    ) -> Result<Self, HirThreadBodyInvariantError> {
        let expected = owner.module();
        let maximum = HirLimit::ThreadFlowItems.maximum();
        if items.len() > maximum {
            return Err(HirThreadBodyInvariantError::ItemLimit {
                observed: items.len(),
                maximum,
            });
        }
        if scope.module() != expected {
            return Err(HirThreadBodyInvariantError::ForeignReference {
                expected,
                actual: scope.module(),
            });
        }
        if matches!(owner, HirThreadBodyOwner::NestedScope(owner_scope) if owner_scope != scope) {
            return Err(HirThreadBodyInvariantError::MismatchedNestedScope);
        }

        let mut unique = BTreeSet::new();
        for item in &items {
            if item.module() != expected {
                return Err(HirThreadBodyInvariantError::ForeignReference {
                    expected,
                    actual: item.module(),
                });
            }
            if !unique.insert(item.owner()) {
                return Err(HirThreadBodyInvariantError::DuplicateChild);
            }
        }
        Ok(Self { scope, items })
    }

    pub const fn scope(&self) -> ScopeId {
        self.scope
    }

    pub fn items(&self) -> &[HirThreadFlowItem] {
        &self.items
    }

    pub(crate) fn validate_module(&self, expected: HirModuleId) -> Result<(), HirModuleId> {
        if self.scope.module() != expected {
            return Err(self.scope.module());
        }
        self.items
            .iter()
            .find_map(|item| (item.module() != expected).then(|| item.module()))
            .map_or(Ok(()), Err)
    }
}

/// Source-ordered flow-item inventory accepted inside a thread body.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirThreadFlowItem {
    Statement(StmtId),
    DialogueApplication(ExprId),
    Choice(StmtId),
    If(StmtId),
    IfLet(StmtId),
    Match(StmtId),
    Loop(StmtId),
    While(StmtId),
    WhileLet(StmtId),
    For(StmtId),
    Select(StmtId),
    SourceLocale(StmtId),
    Scope(StmtId),
    Include(StmtId),
    AwaitWith(StmtId),
    Error(StmtId),
}

impl HirThreadFlowItem {
    pub(crate) const fn module(&self) -> HirModuleId {
        match self {
            Self::DialogueApplication(expression) => expression.module(),
            Self::Statement(statement)
            | Self::Choice(statement)
            | Self::If(statement)
            | Self::IfLet(statement)
            | Self::Match(statement)
            | Self::Loop(statement)
            | Self::While(statement)
            | Self::WhileLet(statement)
            | Self::For(statement)
            | Self::Select(statement)
            | Self::SourceLocale(statement)
            | Self::Scope(statement)
            | Self::Include(statement)
            | Self::AwaitWith(statement)
            | Self::Error(statement) => statement.module(),
        }
    }

    pub(crate) const fn owner(&self) -> SyntheticOwner {
        match self {
            Self::DialogueApplication(expression) => SyntheticOwner::Expr(*expression),
            Self::Statement(statement)
            | Self::Choice(statement)
            | Self::If(statement)
            | Self::IfLet(statement)
            | Self::Match(statement)
            | Self::Loop(statement)
            | Self::While(statement)
            | Self::WhileLet(statement)
            | Self::For(statement)
            | Self::Select(statement)
            | Self::SourceLocale(statement)
            | Self::Scope(statement)
            | Self::Include(statement)
            | Self::AwaitWith(statement)
            | Self::Error(statement) => SyntheticOwner::Stmt(*statement),
        }
    }
}

/// Structural failure while assembling one shared Flow/Thread body.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum HirThreadBodyInvariantError {
    ItemLimit {
        observed: usize,
        maximum: usize,
    },
    ForeignReference {
        expected: HirModuleId,
        actual: HirModuleId,
    },
    MismatchedNestedScope,
    DuplicateChild,
}

/// Typed thread-family poison and semantic admission failures.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirThreadIssue {
    InvalidName,
    RecoveredBodyChild { ordinal: u32 },
    UnclosedBody,
    DetachedBorrowedCapture { capture: CaptureId },
    DetachedEphemeralRegistryAccess,
    MissingBody,
}
