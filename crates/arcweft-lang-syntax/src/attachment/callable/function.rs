//! Attached ordinary-Function declaration and body ownership.

use super::{
    AttachedCallableContractClause, AttachedCallableParameter, AttachedCallableReturn,
    AttachedFixedParameterGroup, parameter_shape_has_recovery,
};
use crate::attachment::node::{
    AstNode, BlockKind, ErrorNodeKind, FunctionBodyKind, FunctionItemKind, MissingBodyKind,
};
use crate::attachment::{
    AttachedGenericParameterGroup, AttachedItemPrefix, AttachedRequiredName, AttachedWhereClause,
};

/// Exact ordinary-function body family selected by the parser.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedFunctionBody {
    Block {
        syntax: AstNode<FunctionBodyKind>,
        block: AstNode<BlockKind>,
    },
    Missing {
        syntax: AstNode<FunctionBodyKind>,
        missing: AstNode<MissingBodyKind>,
    },
}

impl AttachedFunctionBody {
    pub const fn syntax(&self) -> &AstNode<FunctionBodyKind> {
        match self {
            Self::Block { syntax, .. } | Self::Missing { syntax, .. } => syntax,
        }
    }

    pub const fn block(&self) -> Option<&AstNode<BlockKind>> {
        match self {
            Self::Block { block, .. } => Some(block),
            Self::Missing { .. } => None,
        }
    }

    pub const fn missing(&self) -> Option<&AstNode<MissingBodyKind>> {
        match self {
            Self::Missing { missing, .. } => Some(missing),
            Self::Block { .. } => None,
        }
    }

    pub fn has_recovery(&self) -> bool {
        match self {
            Self::Block { block, .. } => block
                .close_delimiter()
                .map_or(true, |close| close.range().is_empty()),
            Self::Missing { .. } => true,
        }
    }
}

/// Complete source-bound ordinary function declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedFunctionDeclaration {
    pub(super) syntax: AstNode<FunctionItemKind>,
    pub(super) prefix: AttachedItemPrefix,
    pub(super) name: AttachedRequiredName,
    pub(super) generics: Option<AttachedGenericParameterGroup>,
    pub(super) parameter_groups: Box<[AttachedFixedParameterGroup]>,
    pub(super) where_clauses: Box<[AttachedWhereClause]>,
    pub(super) contracts: Box<[AttachedCallableContractClause]>,
    pub(super) authored_return: Option<AttachedCallableReturn>,
    pub(super) body: AttachedFunctionBody,
    pub(super) trailing_recovery: Box<[AstNode<ErrorNodeKind>]>,
}

impl AttachedFunctionDeclaration {
    pub const fn syntax(&self) -> &AstNode<FunctionItemKind> {
        &self.syntax
    }

    pub const fn prefix(&self) -> &AttachedItemPrefix {
        &self.prefix
    }

    pub const fn name(&self) -> &AttachedRequiredName {
        &self.name
    }

    pub const fn generics(&self) -> Option<&AttachedGenericParameterGroup> {
        self.generics.as_ref()
    }

    pub const fn parameter_groups(&self) -> &[AttachedFixedParameterGroup] {
        &self.parameter_groups
    }

    pub fn parameters(&self) -> impl Iterator<Item = &AttachedCallableParameter> {
        self.parameter_groups
            .iter()
            .flat_map(AttachedFixedParameterGroup::parameters)
    }

    /// Whether positional-rest structure violates the ordinary-function grammar.
    ///
    /// The attached parameter kind and default expression remain available for
    /// exact recovery even when this returns `true`.
    pub fn has_parameter_shape_recovery(&self) -> bool {
        parameter_shape_has_recovery(&self.parameter_groups)
    }

    pub const fn where_clauses(&self) -> &[AttachedWhereClause] {
        &self.where_clauses
    }

    pub const fn contracts(&self) -> &[AttachedCallableContractClause] {
        &self.contracts
    }

    pub const fn authored_return(&self) -> Option<&AttachedCallableReturn> {
        self.authored_return.as_ref()
    }

    pub const fn body(&self) -> &AttachedFunctionBody {
        &self.body
    }

    pub const fn trailing_recovery(&self) -> &[AstNode<ErrorNodeKind>] {
        &self.trailing_recovery
    }

    /// Zero-width source site immediately after the final parameter group.
    ///
    /// # Panics
    ///
    /// Panics only if a malformed attached declaration bypasses the grammar,
    /// which always emits one authored or missing parameter group.
    pub fn parameter_end_source_span(&self) -> arcweft_source::SourceSpan {
        self.parameter_groups
            .last()
            .expect("grammar always emits one authored or missing parameter group")
            .end_source_span()
    }

    pub fn requires_scope_source_span(&self) -> arcweft_source::SourceSpan {
        self.contracts
            .iter()
            .find(|clause| clause.is_requires() && !clause.has_recovery())
            .map_or_else(
                || self.parameter_end_source_span(),
                AttachedCallableContractClause::keyword_start_source_span,
            )
    }

    pub fn ensures_scope_source_span(&self) -> arcweft_source::SourceSpan {
        self.contracts
            .iter()
            .find(|clause| clause.is_ensures() && !clause.has_recovery())
            .map_or_else(
                || {
                    self.authored_return
                        .as_ref()
                        .filter(|authored| !authored.has_recovery())
                        .map_or_else(
                            || self.parameter_end_source_span(),
                            AttachedCallableReturn::end_source_span,
                        )
                },
                AttachedCallableContractClause::keyword_start_source_span,
            )
    }

    pub fn postcondition_result_source_span(&self) -> Option<arcweft_source::SourceSpan> {
        self.contracts
            .iter()
            .any(AttachedCallableContractClause::is_ensures)
            .then(|| {
                self.contracts
                    .iter()
                    .find(|clause| clause.is_ensures() && !clause.has_recovery())
                    .and_then(AttachedCallableContractClause::condition_start_source_span)
                    .unwrap_or_else(|| {
                        self.authored_return
                            .as_ref()
                            .filter(|authored| !authored.has_recovery())
                            .map_or_else(
                                || self.parameter_end_source_span(),
                                AttachedCallableReturn::end_source_span,
                            )
                    })
            })
    }
}
