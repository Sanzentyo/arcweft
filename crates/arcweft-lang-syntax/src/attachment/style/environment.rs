//! Attached native Style environment model.

use arcweft_source::SourceSpan;

use crate::attachment::node::{
    AstNode, CloseParenKind, ErrorNodeKind, OpenParenKind, StyleEnvironmentBlockKind,
    StyleEnvironmentClauseKind, StyleEnvironmentConditionKind,
};

use super::{
    AttachedStyleBody, AttachedStyleExpression, AttachedStyleName, StyleEnvironmentComparisonKind,
    StyleEnvironmentConditionIssue, StyleEnvironmentFieldKind,
};

/// Closed field or typed unsupported/missing recovery.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedStyleEnvironmentField {
    Known {
        value: StyleEnvironmentFieldKind,
        name: AttachedStyleName,
    },
    Unsupported(AttachedStyleName),
    Missing(AttachedStyleName),
}

impl AttachedStyleEnvironmentField {
    pub const fn value(&self) -> Option<StyleEnvironmentFieldKind> {
        match self {
            Self::Known { value, .. } => Some(*value),
            Self::Unsupported(_) | Self::Missing(_) => None,
        }
    }

    pub const fn name(&self) -> &AttachedStyleName {
        match self {
            Self::Known { name, .. } | Self::Unsupported(name) | Self::Missing(name) => name,
        }
    }

    pub const fn has_recovery(&self) -> bool {
        !matches!(self, Self::Known { .. })
    }
}

/// Closed comparison or exact unsupported/missing recovery.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedStyleEnvironmentComparison {
    Known {
        value: StyleEnvironmentComparisonKind,
        source: SourceSpan,
    },
    Unsupported {
        source: SourceSpan,
    },
    Missing {
        insertion: SourceSpan,
    },
}

impl AttachedStyleEnvironmentComparison {
    pub const fn value(&self) -> Option<StyleEnvironmentComparisonKind> {
        match self {
            Self::Known { value, .. } => Some(*value),
            Self::Unsupported { .. } | Self::Missing { .. } => None,
        }
    }

    pub fn source_span(&self) -> SourceSpan {
        match self {
            Self::Known { source, .. } | Self::Unsupported { source } => source.clone(),
            Self::Missing { insertion } => insertion.clone(),
        }
    }

    pub const fn has_recovery(&self) -> bool {
        !matches!(self, Self::Known { .. })
    }
}

/// One ordered environment operand triple.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedStyleEnvironmentClause {
    pub(super) syntax: AstNode<StyleEnvironmentClauseKind>,
    pub(super) source_ordinal: u16,
    pub(super) field: AttachedStyleEnvironmentField,
    pub(super) comparison: AttachedStyleEnvironmentComparison,
    pub(super) value: AttachedStyleExpression,
}

impl AttachedStyleEnvironmentClause {
    pub const fn syntax(&self) -> &AstNode<StyleEnvironmentClauseKind> {
        &self.syntax
    }

    pub const fn source_ordinal(&self) -> u16 {
        self.source_ordinal
    }

    pub const fn field(&self) -> &AttachedStyleEnvironmentField {
        &self.field
    }

    pub const fn comparison(&self) -> &AttachedStyleEnvironmentComparison {
        &self.comparison
    }

    pub const fn value(&self) -> &AttachedStyleExpression {
        &self.value
    }

    pub fn has_recovery(&self) -> bool {
        self.field.has_recovery() || self.comparison.has_recovery() || self.value.has_recovery()
    }
}

/// Parenthesized environment condition and exact delimiter state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedStyleEnvironmentCondition {
    pub(super) syntax: AstNode<StyleEnvironmentConditionKind>,
    pub(super) open: AstNode<OpenParenKind>,
    pub(super) close: AstNode<CloseParenKind>,
    pub(super) clauses: Box<[AttachedStyleEnvironmentClause]>,
    pub(super) recoveries: Box<[AttachedStyleEnvironmentConditionRecovery]>,
}

/// One exact parser-owned malformed environment-condition list component.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedStyleEnvironmentConditionRecovery {
    pub(super) syntax: AstNode<ErrorNodeKind>,
    pub(super) source_ordinal: u32,
    pub(super) issue: StyleEnvironmentConditionIssue,
}

impl AttachedStyleEnvironmentConditionRecovery {
    pub const fn syntax(&self) -> &AstNode<ErrorNodeKind> {
        &self.syntax
    }

    pub const fn source_ordinal(&self) -> u32 {
        self.source_ordinal
    }

    pub const fn issue(&self) -> StyleEnvironmentConditionIssue {
        self.issue
    }
}

impl AttachedStyleEnvironmentCondition {
    pub const fn syntax(&self) -> &AstNode<StyleEnvironmentConditionKind> {
        &self.syntax
    }

    pub const fn open_delimiter(&self) -> &AstNode<OpenParenKind> {
        &self.open
    }

    pub const fn close_delimiter(&self) -> &AstNode<CloseParenKind> {
        &self.close
    }

    pub fn clauses(&self) -> &[AttachedStyleEnvironmentClause] {
        &self.clauses
    }

    pub fn recoveries(&self) -> &[AttachedStyleEnvironmentConditionRecovery] {
        &self.recoveries
    }

    pub fn has_recovery(&self) -> bool {
        self.open.range().is_empty()
            || self.close.range().is_empty()
            || !self.recoveries.is_empty()
            || self
                .clauses
                .iter()
                .any(AttachedStyleEnvironmentClause::has_recovery)
    }
}

/// One native environment wrapper.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedStyleEnvironment {
    pub(super) syntax: AstNode<StyleEnvironmentBlockKind>,
    pub(super) source_ordinal: u32,
    pub(super) intrinsic: AttachedStyleName,
    pub(super) condition: AttachedStyleEnvironmentCondition,
    pub(super) body: AttachedStyleBody,
}

impl AttachedStyleEnvironment {
    pub const fn syntax(&self) -> &AstNode<StyleEnvironmentBlockKind> {
        &self.syntax
    }

    pub const fn source_ordinal(&self) -> u32 {
        self.source_ordinal
    }

    pub const fn intrinsic(&self) -> &AttachedStyleName {
        &self.intrinsic
    }

    pub const fn condition(&self) -> &AttachedStyleEnvironmentCondition {
        &self.condition
    }

    pub const fn body(&self) -> &AttachedStyleBody {
        &self.body
    }

    pub fn has_recovery(&self) -> bool {
        self.intrinsic.has_recovery() || self.condition.has_recovery() || self.body.has_recovery()
    }
}
