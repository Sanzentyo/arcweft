//! Typed HIR nodes for borrow, dereference, and reference types.

use crate::identity::{ExprId, TypeId};
use arcweft_lang_syntax::ast::common::TextRange;
use arcweft_lang_syntax::reference::BorrowKind;
use arcweft_lang_syntax::types::LifetimeName;

/// Lowered lifetime region retained without display-text parsing.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirRegion {
    /// No lifetime was authored; the anchor is its insertion point.
    Elided { anchor: TextRange },
    /// Authored lifetime name and exact token range.
    Named {
        name: LifetimeName,
        range: TextRange,
    },
}

impl HirRegion {
    /// Returns the exact authored range or elision anchor.
    pub const fn range(&self) -> TextRange {
        match self {
            Self::Elided { anchor } => *anchor,
            Self::Named { range, .. } => *range,
        }
    }

    /// Returns the authored lifetime name, if present.
    pub const fn name(&self) -> Option<&LifetimeName> {
        match self {
            Self::Elided { .. } => None,
            Self::Named { name, .. } => Some(name),
        }
    }
}

/// Lowered prefix-borrow expression payload.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirBorrowExpr {
    kind: BorrowKind,
    operand: ExprId,
    operator_range: TextRange,
}

impl HirBorrowExpr {
    /// Returns shared or mutable borrow permission.
    pub const fn kind(&self) -> BorrowKind {
        self.kind
    }

    /// Returns the borrowed expression identity.
    pub const fn operand(&self) -> ExprId {
        self.operand
    }

    /// Returns the range covering `&` and an optional `mut`.
    pub const fn operator_range(&self) -> TextRange {
        self.operator_range
    }
}

/// Lowered prefix-dereference expression payload.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirDerefExpr {
    operand: ExprId,
    operator_range: TextRange,
}

impl HirDerefExpr {
    /// Returns the dereferenced expression identity.
    pub const fn operand(&self) -> ExprId {
        self.operand
    }

    /// Returns the exact `*` operator range.
    pub const fn operator_range(&self) -> TextRange {
        self.operator_range
    }
}

/// Lowered reference type payload.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirReferenceType {
    kind: BorrowKind,
    region: HirRegion,
    referent: TypeId,
    amp_range: TextRange,
    mut_range: Option<TextRange>,
    range: TextRange,
}

impl HirReferenceType {
    /// Returns shared or mutable reference permission.
    pub const fn kind(&self) -> BorrowKind {
        self.kind
    }

    /// Returns the lowered lifetime region.
    pub const fn region(&self) -> &HirRegion {
        &self.region
    }

    /// Returns the referent type identity.
    pub const fn referent(&self) -> TypeId {
        self.referent
    }

    /// Returns the exact `&` token range.
    pub const fn amp_range(&self) -> TextRange {
        self.amp_range
    }

    /// Returns the exact contextual `mut` range when present.
    pub const fn mut_range(&self) -> Option<TextRange> {
        self.mut_range
    }

    /// Returns the complete reference-type range.
    pub const fn range(&self) -> TextRange {
        self.range
    }
}
