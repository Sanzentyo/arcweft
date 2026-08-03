//! Scalar and non-scoping expression payload records.

use crate::identity::{ExprId, LocalId};
use crate::leaf::{HirName, HirPath};

/// Semantic role of an authored placeholder.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirPlaceholderKind {
    PartialApplication,
    PipeLeft,
}

/// Ordered tuple elements.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirTupleExpr {
    elements: Box<[ExprId]>,
}

impl HirTupleExpr {
    pub(crate) fn new(elements: Box<[ExprId]>) -> Self {
        Self { elements }
    }

    pub fn elements(&self) -> &[ExprId] {
        &self.elements
    }
}

/// Ordered general bracket-sequence elements.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirBracketSequenceExpr {
    elements: Box<[ExprId]>,
}

impl HirBracketSequenceExpr {
    pub(crate) fn new(elements: Box<[ExprId]>) -> Self {
        Self { elements }
    }

    pub fn elements(&self) -> &[ExprId] {
        &self.elements
    }
}

/// Repeated array value and length expressions.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirArrayRepeatExpr {
    value: ExprId,
    length: ExprId,
}

impl HirArrayRepeatExpr {
    pub(crate) const fn new(value: ExprId, length: ExprId) -> Self {
        Self { value, length }
    }

    pub const fn value(&self) -> ExprId {
        self.value
    }

    pub const fn length(&self) -> ExprId {
        self.length
    }
}

/// Member selection from an expression target.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirSelectedMember {
    Name(HirName),
    Missing,
}

/// Member selection from an expression target.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirSelectExpr {
    target: ExprId,
    member: HirSelectedMember,
}

impl HirSelectExpr {
    pub(crate) const fn new(target: ExprId, member: HirSelectedMember) -> Self {
        Self { target, member }
    }

    pub const fn target(&self) -> ExprId {
        self.target
    }

    pub const fn member(&self) -> &HirSelectedMember {
        &self.member
    }
}

/// Index operation over one target.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirIndexExpr {
    target: ExprId,
    index: ExprId,
}

impl HirIndexExpr {
    pub(crate) const fn new(target: ExprId, index: ExprId) -> Self {
        Self { target, index }
    }

    pub const fn target(&self) -> ExprId {
        self.target
    }

    pub const fn index(&self) -> ExprId {
        self.index
    }
}

/// Left-to-right pipeline operands.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirPipeExpr {
    left: ExprId,
    right: ExprId,
}

impl HirPipeExpr {
    pub(crate) const fn new(left: ExprId, right: ExprId) -> Self {
        Self { left, right }
    }

    pub const fn left(&self) -> ExprId {
        self.left
    }

    pub const fn right(&self) -> ExprId {
        self.right
    }
}

/// Try operand with its exact authored semantic form.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirTryExpr {
    operand: ExprId,
    form: HirTryForm,
}

impl HirTryExpr {
    pub(crate) const fn new(operand: ExprId, form: HirTryForm) -> Self {
        Self { operand, form }
    }

    pub const fn operand(&self) -> ExprId {
        self.operand
    }

    pub const fn form(&self) -> HirTryForm {
        self.form
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirTryForm {
    PrefixTry,
    PostfixQuestion,
}

/// Await operand and error-propagation semantics.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirAwaitExpr {
    operand: ExprId,
    propagation: HirAwaitPropagation,
}

impl HirAwaitExpr {
    pub(crate) const fn new(operand: ExprId, propagation: HirAwaitPropagation) -> Self {
        Self {
            operand,
            propagation,
        }
    }

    pub const fn operand(&self) -> ExprId {
        self.operand
    }

    pub const fn propagation(&self) -> HirAwaitPropagation {
        self.propagation
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirAwaitPropagation {
    PreserveResult,
    PropagateError,
}

/// Optional range endpoints and inclusive-end semantics.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirRangeExpr {
    start: Option<ExprId>,
    end: Option<ExprId>,
    inclusive: bool,
}

impl HirRangeExpr {
    pub(crate) const fn new(start: Option<ExprId>, end: Option<ExprId>, inclusive: bool) -> Self {
        Self {
            start,
            end,
            inclusive,
        }
    }

    pub const fn start(&self) -> Option<ExprId> {
        self.start
    }

    pub const fn end(&self) -> Option<ExprId> {
        self.end
    }

    pub const fn inclusive(&self) -> bool {
        self.inclusive
    }
}

/// Path-qualified record construction.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirRecordExpr {
    path: HirPath,
    fields: Box<[HirRecordField]>,
}

impl HirRecordExpr {
    pub(crate) const fn new(path: HirPath, fields: Box<[HirRecordField]>) -> Self {
        Self { path, fields }
    }

    pub const fn path(&self) -> &HirPath {
        &self.path
    }

    pub fn fields(&self) -> &[HirRecordField] {
        &self.fields
    }
}

/// Pathless record literal construction.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirRecordLiteralExpr {
    fields: Box<[HirRecordField]>,
}

impl HirRecordLiteralExpr {
    pub(crate) const fn new(fields: Box<[HirRecordField]>) -> Self {
        Self { fields }
    }

    pub fn fields(&self) -> &[HirRecordField] {
        &self.fields
    }
}

/// Binary operation over two expression children.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirBinaryExpr {
    left: ExprId,
    operator: HirBinaryOp,
    right: ExprId,
}

impl HirBinaryExpr {
    pub(crate) const fn new(left: ExprId, operator: HirBinaryOp, right: ExprId) -> Self {
        Self {
            left,
            operator,
            right,
        }
    }

    pub const fn left(&self) -> ExprId {
        self.left
    }

    pub const fn operator(&self) -> HirBinaryOp {
        self.operator
    }

    pub const fn right(&self) -> ExprId {
        self.right
    }
}

/// HIR-owned binary operator vocabulary.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirBinaryOp {
    Implies,
    Or,
    And,
    In,
    Equal,
    NotEqual,
    GreaterOrEqual,
    LessOrEqual,
    Greater,
    Less,
    Merge,
    Add,
    Subtract,
    Multiply,
    Divide,
    Remainder,
}

/// Borrow operation over one expression.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirBorrowExpr {
    kind: HirBorrowKind,
    operand: ExprId,
}

impl HirBorrowExpr {
    pub(crate) const fn new(kind: HirBorrowKind, operand: ExprId) -> Self {
        Self { kind, operand }
    }

    pub const fn kind(&self) -> HirBorrowKind {
        self.kind
    }

    pub const fn operand(&self) -> ExprId {
        self.operand
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirBorrowKind {
    Shared,
    Mutable,
}

/// Dereference operation over one expression.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirDereferenceExpr {
    operand: ExprId,
}

impl HirDereferenceExpr {
    pub(crate) const fn new(operand: ExprId) -> Self {
        Self { operand }
    }

    pub const fn operand(&self) -> ExprId {
        self.operand
    }
}

/// Unary operation over one expression.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirUnaryExpr {
    operator: HirUnaryOp,
    operand: ExprId,
}

impl HirUnaryExpr {
    pub(crate) const fn new(operator: HirUnaryOp, operand: ExprId) -> Self {
        Self { operator, operand }
    }

    pub const fn operator(&self) -> HirUnaryOp {
        self.operator
    }

    pub const fn operand(&self) -> ExprId {
        self.operand
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirUnaryOp {
    Not,
    Negate,
}

/// One explicit, shorthand, or typed-invalid record field.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirRecordField {
    Explicit { name: HirName, value: ExprId },
    Shorthand { name: HirName, local: LocalId },
    Invalid { issue: HirRecordFieldIssue },
}

impl HirRecordField {
    pub(crate) const fn explicit(name: HirName, value: ExprId) -> Self {
        Self::Explicit { name, value }
    }

    pub(crate) const fn shorthand(name: HirName, local: LocalId) -> Self {
        Self::Shorthand { name, local }
    }

    pub(crate) const fn invalid(issue: HirRecordFieldIssue) -> Self {
        Self::Invalid { issue }
    }

    pub const fn name(&self) -> Option<&HirName> {
        match self {
            Self::Explicit { name, .. } | Self::Shorthand { name, .. } => Some(name),
            Self::Invalid { .. } => None,
        }
    }

    pub const fn value(&self) -> Option<ExprId> {
        match self {
            Self::Explicit { value, .. } => Some(*value),
            Self::Shorthand { .. } | Self::Invalid { .. } => None,
        }
    }

    pub const fn local(&self) -> Option<LocalId> {
        match self {
            Self::Shorthand { local, .. } => Some(*local),
            Self::Explicit { .. } | Self::Invalid { .. } => None,
        }
    }

    pub const fn issue(&self) -> Option<HirRecordFieldIssue> {
        match self {
            Self::Invalid { issue } => Some(*issue),
            Self::Explicit { .. } | Self::Shorthand { .. } => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirRecordFieldIssue {
    MissingName,
    MissingValue,
    DuplicateName,
    ForeignChild,
}
