//! Typed callable ownership for ordinary functions, Proof declarations, and assertions.

mod function;

pub use self::function::{AttachedFunctionBody, AttachedFunctionDeclaration};

use crate::assertion::AssertionMode;
use crate::ast::common::TextRange;
use crate::expressions::{
    ExpressionComponentRole, ExpressionProjection, SyntaxCallArgumentListTerminator,
    SyntaxCallArgumentPart, SyntaxCallArgumentProjection, SyntaxRequiredTokenState,
};
use crate::grammar::callable_projection::{
    MethodReceiverSyntaxKind, PendingMethodReceiverProjection,
};
use crate::grammar::kinds::{SyntaxKind, SyntaxRole, SyntaxRoleClass};
use crate::literal::SyntaxLiteralValue;
use crate::patterns::{PatternComponentRole, PatternSyntaxFamily};

use super::declaration::{AttachedDeclarationIdentity, attach_declaration_identity};
use super::family::{ExpressionFamily, NameFamily, PatternFamily, TypeFamily};
use super::item_prefix::is_verify_trusted_attribute;
use super::node::{
    AssertionStatementKind, AstNode, CloseBraceKind, CloseParenKind, ColonKind, EffectsClauseKind,
    EnsuresClauseKind, EqualsKind, ErrorNodeKind, ExpressionBodyKind, FixedParameterGroupKind,
    FunctionBodyKind, FunctionItemKind, MissingBodyKind, OpenBraceKind, OpenParenKind,
    ParameterKind, PredicateBlockKind, PredicateBodyKind, PredicateItemKind, ProofBlockKind,
    ProofBodyKind, ProofItemKind, RequiresClauseKind, RestParameterMarkerKind, ReturnTypeKind,
    ThinArrowKind,
};
use super::nominal::{optional_generics, required_name, where_clauses};
use super::source_file::AttachedDelimiterState;
use super::{
    AttachedAttributeValue, AttachedDeclarationPublicId, AttachedExpressionNode,
    AttachedGenericParameterGroup, AttachedItemPrefix, AttachedOuterAttribute,
    AttachedOuterAttributeForm, AttachedPatternNode, AttachedRequiredName,
    AttachedRequiredPunctuation, AttachedTypeFamily, AttachedTypeRefNode, AttachedWhereClause,
    DeclarationBodyNode, NameNode, SyntaxAccessError, TypedItemNode,
};

/// Decoded, non-blank trusted-proof justification retained exactly as authored.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TrustReasonSyntax(Box<str>);

impl TrustReasonSyntax {
    /// Admits one lexer-decoded reason when Unicode trimming leaves content.
    pub fn try_new(decoded: impl Into<Box<str>>) -> Result<Self, TrustReasonSyntaxError> {
        let decoded = decoded.into();
        if decoded.trim().is_empty() {
            return Err(TrustReasonSyntaxError::Empty);
        }
        Ok(Self(decoded))
    }

    /// Returns the exact decoded reason bytes without trimming or normalization.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Invalid decoded trusted-proof reason.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TrustReasonSyntaxError {
    /// The decoded string was empty or Unicode whitespace only.
    Empty,
}

/// Final typed trust classification owned by one attached Proof declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProofTrustSyntax {
    /// Ordinary Proof whose body must be verified.
    Verified,
    /// Proof admitted directly by an authored trust attribute.
    Trusted {
        /// Exact decoded non-blank justification.
        reason: TrustReasonSyntax,
        /// Exact source extent of the consumed trust attribute.
        attribute_range: TextRange,
    },
}

impl ProofTrustSyntax {
    /// Whether this Proof is admitted directly instead of through verification.
    pub const fn is_directly_trusted(&self) -> bool {
        matches!(self, Self::Trusted { .. })
    }

    /// Returns the authored trust reason for a directly trusted Proof.
    pub const fn reason(&self) -> Option<&TrustReasonSyntax> {
        match self {
            Self::Verified => None,
            Self::Trusted { reason, .. } => Some(reason),
        }
    }

    /// Returns the exact trust-attribute range for a directly trusted Proof.
    pub const fn attribute_range(&self) -> Option<TextRange> {
        match self {
            Self::Verified => None,
            Self::Trusted {
                attribute_range, ..
            } => Some(*attribute_range),
        }
    }
}

/// One fixed Predicate parameter and its exact pattern/type children.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedCallableParameter {
    syntax: AstNode<ParameterKind>,
    source_ordinal: u16,
    group_ordinal: u16,
    parameter_ordinal: u16,
    pattern: AttachedPatternNode,
    colon: AttachedRequiredPunctuation,
    ty: AttachedTypeRefNode,
    kind: AttachedCallableParameterKind,
    default: Option<AttachedCallableParameterDefault>,
    recovery: Box<[AstNode<ErrorNodeKind>]>,
}

/// Parameter arity selected by the ordinary source grammar.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedCallableParameterKind {
    Fixed,
    Rest {
        marker: AstNode<RestParameterMarkerKind>,
    },
}

/// One authored ordinary-Function parameter default.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedCallableParameterDefault {
    equals: AstNode<EqualsKind>,
    value: AttachedExpressionNode,
}

impl AttachedCallableParameterDefault {
    pub const fn equals(&self) -> &AstNode<EqualsKind> {
        &self.equals
    }

    pub const fn value(&self) -> &AttachedExpressionNode {
        &self.value
    }

    pub fn has_recovery(&self) -> bool {
        self.value.projection().has_recovery()
    }
}

impl AttachedCallableParameter {
    pub const fn syntax(&self) -> &AstNode<ParameterKind> {
        &self.syntax
    }

    pub const fn source_ordinal(&self) -> u16 {
        self.source_ordinal
    }

    pub const fn group_ordinal(&self) -> u16 {
        self.group_ordinal
    }

    pub const fn parameter_ordinal(&self) -> u16 {
        self.parameter_ordinal
    }

    pub const fn pattern(&self) -> &AttachedPatternNode {
        &self.pattern
    }

    /// Exact authored `:` token or the parser-owned insertion when omitted.
    pub const fn colon(&self) -> &AttachedRequiredPunctuation {
        &self.colon
    }

    pub const fn ty(&self) -> &AttachedTypeRefNode {
        &self.ty
    }

    pub const fn kind(&self) -> &AttachedCallableParameterKind {
        &self.kind
    }

    pub const fn default(&self) -> Option<&AttachedCallableParameterDefault> {
        self.default.as_ref()
    }

    /// Typed recovery retained directly by this parameter owner.
    ///
    /// A grammar that does not admit defaults may keep the authored equals
    /// sign and expression here without exposing them as a callable default.
    pub fn recovery(&self) -> &[AstNode<ErrorNodeKind>] {
        &self.recovery
    }

    pub const fn is_rest(&self) -> bool {
        matches!(self.kind, AttachedCallableParameterKind::Rest { .. })
    }

    pub fn has_recovery(&self) -> bool {
        self.colon().is_missing()
            || !self.pattern().value().state().is_valid()
            || self.ty().family() == AttachedTypeFamily::Recovery
            || self
                .default()
                .is_some_and(AttachedCallableParameterDefault::has_recovery)
            || !self.recovery.is_empty()
    }
}

/// The exactly one fixed parameter group required by Predicate syntax.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedFixedParameterGroup {
    syntax: AstNode<FixedParameterGroupKind>,
    source_ordinal: u16,
    open: AstNode<OpenParenKind>,
    close: AstNode<CloseParenKind>,
    parameters: Box<[AttachedCallableParameter]>,
}

impl AttachedFixedParameterGroup {
    pub const fn syntax(&self) -> &AstNode<FixedParameterGroupKind> {
        &self.syntax
    }

    pub const fn source_ordinal(&self) -> u16 {
        self.source_ordinal
    }

    pub const fn open(&self) -> &AstNode<OpenParenKind> {
        &self.open
    }

    pub const fn close(&self) -> &AstNode<CloseParenKind> {
        &self.close
    }

    pub const fn parameters(&self) -> &[AttachedCallableParameter] {
        &self.parameters
    }

    pub fn open_state(&self) -> AttachedRequiredPunctuation {
        punctuation(&self.open)
    }

    pub fn close_state(&self) -> AttachedRequiredPunctuation {
        punctuation(&self.close)
    }

    pub fn has_recovery(&self) -> bool {
        self.open_state().is_missing()
            || self.close_state().is_missing()
            || self
                .parameters
                .iter()
                .any(AttachedCallableParameter::has_recovery)
    }

    /// Zero-width anchor immediately after the fixed parameter group.
    pub fn end_source_span(&self) -> arcweft_source::SourceSpan {
        let end = self.close.range().end();
        self.close
            .syntax()
            .source_span_for_range(arcweft_source::SourceRange::new(end, end))
    }
}

/// Semantic ownership mode of one method receiver.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AttachedMethodReceiverKind {
    Owned,
    SharedReference,
    MutableReference,
}

/// One method receiver and its exact binding Pattern/source components.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedMethodReceiver {
    syntax: AstNode<ParameterKind>,
    source_ordinal: u16,
    group_ordinal: u16,
    parameter_ordinal: u16,
    kind: AttachedMethodReceiverKind,
    pattern: AttachedPatternNode,
    whole_source: arcweft_source::SourceSpan,
    ampersand_source: Option<arcweft_source::SourceSpan>,
    mut_keyword_source: Option<arcweft_source::SourceSpan>,
    self_keyword_source: arcweft_source::SourceSpan,
}

impl AttachedMethodReceiver {
    pub const fn syntax(&self) -> &AstNode<ParameterKind> {
        &self.syntax
    }

    pub const fn source_ordinal(&self) -> u16 {
        self.source_ordinal
    }

    pub const fn group_ordinal(&self) -> u16 {
        self.group_ordinal
    }

    pub const fn parameter_ordinal(&self) -> u16 {
        self.parameter_ordinal
    }

    pub const fn kind(&self) -> AttachedMethodReceiverKind {
        self.kind
    }

    pub const fn pattern(&self) -> &AttachedPatternNode {
        &self.pattern
    }

    pub const fn whole_source(&self) -> &arcweft_source::SourceSpan {
        &self.whole_source
    }

    pub const fn ampersand_source(&self) -> Option<&arcweft_source::SourceSpan> {
        self.ampersand_source.as_ref()
    }

    pub const fn mut_keyword_source(&self) -> Option<&arcweft_source::SourceSpan> {
        self.mut_keyword_source.as_ref()
    }

    pub const fn self_keyword_source(&self) -> &arcweft_source::SourceSpan {
        &self.self_keyword_source
    }
}

/// One source-ordered method parameter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedMethodParameter {
    Receiver(Box<AttachedMethodReceiver>),
    Typed(Box<AttachedCallableParameter>),
}

impl AttachedMethodParameter {
    pub const fn source_ordinal(&self) -> u16 {
        match self {
            Self::Receiver(receiver) => receiver.source_ordinal(),
            Self::Typed(parameter) => parameter.source_ordinal(),
        }
    }

    pub const fn group_ordinal(&self) -> u16 {
        match self {
            Self::Receiver(receiver) => receiver.group_ordinal(),
            Self::Typed(parameter) => parameter.group_ordinal(),
        }
    }

    pub const fn parameter_ordinal(&self) -> u16 {
        match self {
            Self::Receiver(receiver) => receiver.parameter_ordinal(),
            Self::Typed(parameter) => parameter.parameter_ordinal(),
        }
    }

    pub const fn receiver(&self) -> Option<&AttachedMethodReceiver> {
        match self {
            Self::Receiver(receiver) => Some(receiver),
            Self::Typed(_) => None,
        }
    }

    pub const fn typed(&self) -> Option<&AttachedCallableParameter> {
        match self {
            Self::Receiver(_) => None,
            Self::Typed(parameter) => Some(parameter),
        }
    }

    pub fn has_recovery(&self) -> bool {
        match self {
            Self::Receiver(_) => false,
            Self::Typed(parameter) => parameter.has_recovery(),
        }
    }
}

/// One source-ordered fixed parameter group owned by a Trait/Impl method.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedMethodParameterGroup {
    syntax: AstNode<FixedParameterGroupKind>,
    source_ordinal: u16,
    open: AstNode<OpenParenKind>,
    close: AstNode<CloseParenKind>,
    parameters: Box<[AttachedMethodParameter]>,
}

impl AttachedMethodParameterGroup {
    pub const fn syntax(&self) -> &AstNode<FixedParameterGroupKind> {
        &self.syntax
    }

    pub const fn source_ordinal(&self) -> u16 {
        self.source_ordinal
    }

    pub const fn open(&self) -> &AstNode<OpenParenKind> {
        &self.open
    }

    pub const fn close(&self) -> &AstNode<CloseParenKind> {
        &self.close
    }

    pub const fn parameters(&self) -> &[AttachedMethodParameter] {
        &self.parameters
    }

    pub fn has_recovery(&self) -> bool {
        self.open.range().is_empty()
            || self.close.range().is_empty()
            || self
                .parameters
                .iter()
                .any(AttachedMethodParameter::has_recovery)
    }
}

pub(super) fn parameter_shape_has_recovery(groups: &[AttachedFixedParameterGroup]) -> bool {
    let final_group = groups.len().saturating_sub(1);
    let mut saw_rest = false;
    for (group_ordinal, group) in groups.iter().enumerate() {
        let final_parameter = group.parameters().len().saturating_sub(1);
        for (parameter_ordinal, parameter) in group.parameters().iter().enumerate() {
            if !parameter.is_rest() {
                continue;
            }
            if saw_rest
                || group_ordinal != final_group
                || parameter_ordinal != final_parameter
                || parameter.default().is_some()
            {
                return true;
            }
            saw_rest = true;
        }
    }
    false
}

impl AstNode<FixedParameterGroupKind> {
    /// Binds this exact fixed group to the shared callable parameter owner.
    pub(crate) fn callable_semantics(
        &self,
        group_ordinal: u16,
        next_parameter_ordinal: &mut u16,
    ) -> Result<AttachedFixedParameterGroup, SyntaxAccessError> {
        attach_parameter_group(self.clone(), group_ordinal, next_parameter_ordinal)
    }
}

/// One source-ordered Predicate contract clause.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedCallableContractClause {
    Requires {
        syntax: AstNode<RequiresClauseKind>,
        source_ordinal: u16,
        family_ordinal: u16,
        condition: AttachedExpressionNode,
        out_of_order: bool,
    },
    Ensures {
        syntax: AstNode<EnsuresClauseKind>,
        source_ordinal: u16,
        family_ordinal: u16,
        condition: AttachedExpressionNode,
    },
    Effects {
        syntax: AstNode<EffectsClauseKind>,
        source_ordinal: u16,
        family_ordinal: u16,
        open: Option<AstNode<OpenBraceKind>>,
        operands: Box<[AttachedExpressionNode]>,
        close: Option<AstNode<CloseBraceKind>>,
    },
}

impl AttachedCallableContractClause {
    pub const fn source_ordinal(&self) -> u16 {
        match self {
            Self::Requires { source_ordinal, .. }
            | Self::Ensures { source_ordinal, .. }
            | Self::Effects { source_ordinal, .. } => *source_ordinal,
        }
    }

    pub const fn family_ordinal(&self) -> u16 {
        match self {
            Self::Requires { family_ordinal, .. }
            | Self::Ensures { family_ordinal, .. }
            | Self::Effects { family_ordinal, .. } => *family_ordinal,
        }
    }

    pub const fn condition(&self) -> Option<&AttachedExpressionNode> {
        match self {
            Self::Requires { condition, .. } | Self::Ensures { condition, .. } => Some(condition),
            Self::Effects { .. } => None,
        }
    }

    pub const fn effects(&self) -> Option<&[AttachedExpressionNode]> {
        match self {
            Self::Effects { operands, .. } => Some(operands),
            Self::Requires { .. } | Self::Ensures { .. } => None,
        }
    }

    pub const fn is_requires(&self) -> bool {
        matches!(self, Self::Requires { .. })
    }

    pub const fn is_ensures(&self) -> bool {
        matches!(self, Self::Ensures { .. })
    }

    pub const fn is_effects(&self) -> bool {
        matches!(self, Self::Effects { .. })
    }

    pub const fn is_out_of_order(&self) -> bool {
        matches!(
            self,
            Self::Requires {
                out_of_order: true,
                ..
            }
        )
    }

    pub fn syntax_source_span(&self) -> arcweft_source::SourceSpan {
        match self {
            Self::Requires { syntax, .. } => syntax.source_span(),
            Self::Ensures { syntax, .. } => syntax.source_span(),
            Self::Effects { syntax, .. } => syntax.source_span(),
        }
    }

    /// Exact authored contract keyword, retained from the attached token.
    ///
    /// This projection never searches source text. The contract node grammar
    /// owns one leading keyword token, so downstream source manifests can
    /// distinguish an authored empty `effects {}` clause from omission.
    ///
    /// # Panics
    ///
    /// Panics if the already-validated attached contract has lost its required
    /// keyword token.
    pub fn keyword_source_span(&self) -> arcweft_source::SourceSpan {
        let syntax = match self {
            Self::Requires { syntax, .. } => syntax.syntax(),
            Self::Ensures { syntax, .. } => syntax.syntax(),
            Self::Effects { syntax, .. } => syntax.syntax(),
        };
        let projection = syntax
            .contract_clause_projection()
            .expect("attached callable contract retains its parser projection");
        syntax.source_span_for_range(projection.clause_keyword())
    }

    /// Zero-width anchor at the authored contract keyword.
    pub fn keyword_start_source_span(&self) -> arcweft_source::SourceSpan {
        let syntax = match self {
            Self::Requires { syntax, .. } => syntax.syntax(),
            Self::Ensures { syntax, .. } => syntax.syntax(),
            Self::Effects { syntax, .. } => syntax.syntax(),
        };
        let start = syntax.range().start();
        syntax.source_span_for_range(arcweft_source::SourceRange::new(start, start))
    }

    /// Zero-width anchor at the start of the authored condition.
    pub fn condition_start_source_span(&self) -> Option<arcweft_source::SourceSpan> {
        let syntax = self.condition()?.syntax().syntax();
        let start = syntax.range().start();
        Some(syntax.source_span_for_range(arcweft_source::SourceRange::new(start, start)))
    }

    pub fn has_recovery(&self) -> bool {
        self.is_out_of_order()
            || match self {
                Self::Requires { condition, .. } | Self::Ensures { condition, .. } => {
                    condition.projection().has_recovery()
                }
                Self::Effects {
                    open,
                    operands,
                    close,
                    ..
                } => {
                    open.is_none()
                        || close.is_none()
                        || close.as_ref().is_some_and(|close| close.range().is_empty())
                        || operands
                            .iter()
                            .any(|operand| operand.projection().has_recovery())
                }
            }
    }
}

/// Forbidden authored Predicate return retained only as recovery evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedPredicateReturnRecovery {
    syntax: AstNode<ReturnTypeKind>,
    ty: AttachedTypeRefNode,
}

impl AttachedPredicateReturnRecovery {
    pub const fn syntax(&self) -> &AstNode<ReturnTypeKind> {
        &self.syntax
    }

    pub const fn ty(&self) -> &AttachedTypeRefNode {
        &self.ty
    }
}

/// Exact Predicate body family selected by the parser.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedPredicateBody {
    Expression {
        syntax: AstNode<PredicateBodyKind>,
        expression: Box<AttachedExpressionNode>,
    },
    Block {
        syntax: AstNode<PredicateBodyKind>,
        block: AstNode<PredicateBlockKind>,
    },
    Missing {
        syntax: AstNode<PredicateBodyKind>,
        missing: AstNode<MissingBodyKind>,
    },
}

impl AttachedPredicateBody {
    pub const fn syntax(&self) -> &AstNode<PredicateBodyKind> {
        match self {
            Self::Expression { syntax, .. }
            | Self::Block { syntax, .. }
            | Self::Missing { syntax, .. } => syntax,
        }
    }

    pub const fn expression(&self) -> Option<&AttachedExpressionNode> {
        match self {
            Self::Expression { expression, .. } => Some(expression),
            Self::Block { .. } | Self::Missing { .. } => None,
        }
    }

    pub const fn block(&self) -> Option<&AstNode<PredicateBlockKind>> {
        match self {
            Self::Block { block, .. } => Some(block),
            Self::Expression { .. } | Self::Missing { .. } => None,
        }
    }

    pub const fn missing(&self) -> Option<&AstNode<MissingBodyKind>> {
        match self {
            Self::Missing { missing, .. } => Some(missing),
            Self::Expression { .. } | Self::Block { .. } => None,
        }
    }

    pub fn has_recovery(&self) -> bool {
        match self {
            Self::Expression { expression, .. } => expression.projection().has_recovery(),
            Self::Block { block, .. } => match block.close_delimiter() {
                Ok(close) => close.range().is_empty(),
                Err(_) => true,
            },
            Self::Missing { .. } => true,
        }
    }
}

/// Complete source-bound Predicate declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedPredicateDeclaration {
    syntax: AstNode<PredicateItemKind>,
    prefix: AttachedItemPrefix,
    name: AttachedRequiredName,
    generics: Option<AttachedGenericParameterGroup>,
    parameter_group: AttachedFixedParameterGroup,
    where_clauses: Box<[AttachedWhereClause]>,
    contracts: Box<[AttachedCallableContractClause]>,
    authored_return: Option<AttachedPredicateReturnRecovery>,
    body: AttachedPredicateBody,
    trailing_recovery: Box<[AstNode<ErrorNodeKind>]>,
}

impl AttachedPredicateDeclaration {
    pub const fn syntax(&self) -> &AstNode<PredicateItemKind> {
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

    pub const fn parameter_group(&self) -> &AttachedFixedParameterGroup {
        &self.parameter_group
    }

    pub const fn where_clauses(&self) -> &[AttachedWhereClause] {
        &self.where_clauses
    }

    pub const fn contracts(&self) -> &[AttachedCallableContractClause] {
        &self.contracts
    }

    pub const fn authored_return(&self) -> Option<&AttachedPredicateReturnRecovery> {
        self.authored_return.as_ref()
    }

    pub const fn body(&self) -> &AttachedPredicateBody {
        &self.body
    }

    pub const fn trailing_recovery(&self) -> &[AstNode<ErrorNodeKind>] {
        &self.trailing_recovery
    }

    pub fn requires_scope_source_span(&self) -> arcweft_source::SourceSpan {
        self.contracts
            .iter()
            .find(|clause| clause.is_requires() && !clause.has_recovery())
            .map_or_else(
                || self.parameter_group.end_source_span(),
                AttachedCallableContractClause::keyword_start_source_span,
            )
    }

    pub fn ensures_scope_source_span(&self) -> arcweft_source::SourceSpan {
        self.contracts
            .iter()
            .find(|clause| clause.is_ensures() && !clause.has_recovery())
            .map_or_else(
                || self.parameter_group.end_source_span(),
                AttachedCallableContractClause::keyword_start_source_span,
            )
    }

    /// Source anchor for the synthetic postcondition `result` local.
    pub fn postcondition_result_source_span(&self) -> Option<arcweft_source::SourceSpan> {
        self.contracts
            .iter()
            .any(AttachedCallableContractClause::is_ensures)
            .then(|| {
                self.contracts
                    .iter()
                    .find(|clause| clause.is_ensures() && !clause.has_recovery())
                    .and_then(AttachedCallableContractClause::condition_start_source_span)
                    .unwrap_or_else(|| self.parameter_group.end_source_span())
            })
    }
}

impl AstNode<PredicateItemKind> {
    /// Binds the complete Predicate declaration without a detached reader.
    pub fn semantics(&self) -> Result<AttachedPredicateDeclaration, SyntaxAccessError> {
        let item = TypedItemNode::Predicate(self.clone());
        let mut next_parameter_ordinal = 0;
        let parameter_group = attach_parameter_group(
            self.required_exact_child::<FixedParameterGroupKind>(SyntaxRole::ParameterGroup)?,
            0,
            &mut next_parameter_ordinal,
        )?;
        let authored_return = self
            .optional_exact_child::<ReturnTypeKind>(SyntaxRole::ReturnType)?
            .map(|syntax| {
                let ty = syntax
                    .required_family_child::<TypeFamily>(SyntaxRole::Type)?
                    .semantic()?;
                Ok::<_, SyntaxAccessError>(AttachedPredicateReturnRecovery { syntax, ty })
            })
            .transpose()?;
        let body = attach_predicate_body(
            self.required_exact_child::<PredicateBodyKind>(SyntaxRole::Body)?,
        )?;
        Ok(AttachedPredicateDeclaration {
            syntax: self.clone(),
            prefix: item.attached_prefix()?,
            name: required_name(&item.syntax(), false)?,
            generics: optional_generics(&item.syntax())?,
            parameter_group,
            where_clauses: where_clauses(&item.syntax())?,
            contracts: attach_contracts(&item)?,
            authored_return,
            body,
            trailing_recovery: self
                .ordered_exact_children::<ErrorNodeKind>(SyntaxRoleClass::Recovery)?
                .into_boxed_slice(),
        })
    }
}

/// Authored callable return wrapper and its exact semantic type child.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedCallableReturn {
    syntax: AstNode<ReturnTypeKind>,
    arrow: AttachedRequiredPunctuation,
    ty: AttachedTypeRefNode,
}

impl AttachedCallableReturn {
    pub const fn syntax(&self) -> &AstNode<ReturnTypeKind> {
        &self.syntax
    }

    /// Exact parser-owned `->` token site.
    pub const fn arrow(&self) -> &AttachedRequiredPunctuation {
        &self.arrow
    }

    pub const fn ty(&self) -> &AttachedTypeRefNode {
        &self.ty
    }

    pub fn has_recovery(&self) -> bool {
        self.ty.family() == AttachedTypeFamily::Recovery
    }

    /// Zero-width anchor immediately after the authored return type.
    pub fn end_source_span(&self) -> arcweft_source::SourceSpan {
        let syntax = self.ty.syntax();
        let end = syntax.range().end();
        syntax.source_span_for_range(arcweft_source::SourceRange::new(end, end))
    }
}

impl AstNode<ReturnTypeKind> {
    /// Binds one authored return wrapper to the shared callable return owner.
    pub(crate) fn callable_semantics(&self) -> Result<AttachedCallableReturn, SyntaxAccessError> {
        Ok(AttachedCallableReturn {
            syntax: self.clone(),
            arrow: punctuation(&self.required_exact_child::<ThinArrowKind>(SyntaxRole::Token)?),
            ty: self
                .required_family_child::<TypeFamily>(SyntaxRole::Type)?
                .semantic()?,
        })
    }
}

/// Exact Proof body family selected by the parser.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedProofBody {
    Expression {
        syntax: AstNode<ProofBodyKind>,
        expression: Box<AttachedExpressionNode>,
    },
    Block {
        syntax: AstNode<ProofBodyKind>,
        block: AstNode<ProofBlockKind>,
    },
    Missing {
        syntax: AstNode<ProofBodyKind>,
        missing: AstNode<MissingBodyKind>,
    },
}

impl AttachedProofBody {
    pub const fn syntax(&self) -> &AstNode<ProofBodyKind> {
        match self {
            Self::Expression { syntax, .. }
            | Self::Block { syntax, .. }
            | Self::Missing { syntax, .. } => syntax,
        }
    }

    pub fn has_recovery(&self) -> bool {
        match self {
            Self::Expression { expression, .. } => expression.projection().has_recovery(),
            Self::Block { block, .. } => match block.close_delimiter() {
                Ok(close) => close.range().is_empty(),
                Err(_) => true,
            },
            Self::Missing { .. } => true,
        }
    }
}

/// Complete source-bound Proof declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedProofDeclaration {
    syntax: AstNode<ProofItemKind>,
    prefix: AttachedItemPrefix,
    trust: Option<ProofTrustSyntax>,
    trust_attribute_source: Option<arcweft_source::SourceSpan>,
    trust_reason_source: Option<arcweft_source::SourceSpan>,
    identity: AttachedDeclarationIdentity,
    name: AttachedRequiredName,
    generics: Option<AttachedGenericParameterGroup>,
    parameter_group: AttachedFixedParameterGroup,
    where_clauses: Box<[AttachedWhereClause]>,
    contracts: Box<[AttachedCallableContractClause]>,
    authored_return: Option<AttachedCallableReturn>,
    body: AttachedProofBody,
    trailing_recovery: Box<[AstNode<ErrorNodeKind>]>,
}

impl AttachedProofDeclaration {
    pub const fn syntax(&self) -> &AstNode<ProofItemKind> {
        &self.syntax
    }

    pub const fn prefix(&self) -> &AttachedItemPrefix {
        &self.prefix
    }

    /// Final trust projection. `None` is explicit malformed-metadata recovery;
    /// it must never be interpreted as ordinary verification.
    pub const fn trust(&self) -> Option<&ProofTrustSyntax> {
        self.trust.as_ref()
    }

    /// Whether explicit malformed trust metadata poisoned this declaration.
    pub const fn has_trust_recovery(&self) -> bool {
        self.trust.is_none()
    }

    /// Exact accepted trust-attribute span, absent for verified or recovered metadata.
    pub const fn trust_attribute_source_span(&self) -> Option<&arcweft_source::SourceSpan> {
        self.trust_attribute_source.as_ref()
    }

    /// Exact accepted reason-expression span, absent for verified or recovered metadata.
    pub const fn trust_reason_source_span(&self) -> Option<&arcweft_source::SourceSpan> {
        self.trust_reason_source.as_ref()
    }

    pub const fn identity(&self) -> &AttachedDeclarationIdentity {
        &self.identity
    }

    pub const fn public_id(&self) -> &AttachedDeclarationPublicId {
        self.identity.public_id()
    }

    pub const fn name(&self) -> &AttachedRequiredName {
        &self.name
    }

    pub const fn generics(&self) -> Option<&AttachedGenericParameterGroup> {
        self.generics.as_ref()
    }

    pub const fn parameter_group(&self) -> &AttachedFixedParameterGroup {
        &self.parameter_group
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

    pub const fn body(&self) -> &AttachedProofBody {
        &self.body
    }

    pub const fn trailing_recovery(&self) -> &[AstNode<ErrorNodeKind>] {
        &self.trailing_recovery
    }

    pub fn requires_scope_source_span(&self) -> arcweft_source::SourceSpan {
        self.contracts
            .iter()
            .find(|clause| clause.is_requires() && !clause.has_recovery())
            .map_or_else(
                || self.parameter_group.end_source_span(),
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
                            || self.parameter_group.end_source_span(),
                            AttachedCallableReturn::end_source_span,
                        )
                },
                AttachedCallableContractClause::keyword_start_source_span,
            )
    }

    /// Source anchor for the synthetic postcondition `result` local.
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
                                || self.parameter_group.end_source_span(),
                                AttachedCallableReturn::end_source_span,
                            )
                    })
            })
    }

    /// Return insertion anchor used when no return type is authored.
    pub fn implicit_return_source_span(&self) -> arcweft_source::SourceSpan {
        self.parameter_group.end_source_span()
    }
}

impl AstNode<ProofItemKind> {
    /// Binds the complete Proof declaration without a detached reader.
    pub fn semantics(&self) -> Result<AttachedProofDeclaration, SyntaxAccessError> {
        let item = TypedItemNode::Proof(self.clone());
        let mut prefix = item.attached_prefix_including_proof_trust()?;
        let trust = attach_proof_trust(&prefix);
        prefix.remove_proof_trust_attributes();
        let pending = self
            .syntax()
            .declaration_header_projection()
            .cloned()
            .ok_or(SyntaxAccessError::MissingDeclarationHeaderProjection { id: self.id() })?;
        let identity = attach_declaration_identity(&self.syntax(), &pending)?;
        let mut next_parameter_ordinal = 0;
        let parameter_group = attach_parameter_group(
            self.required_exact_child::<FixedParameterGroupKind>(SyntaxRole::ParameterGroup)?,
            0,
            &mut next_parameter_ordinal,
        )?;
        let authored_return = self
            .optional_exact_child::<ReturnTypeKind>(SyntaxRole::ReturnType)?
            .map(|syntax| syntax.callable_semantics())
            .transpose()?;
        let body =
            attach_proof_body(self.required_exact_child::<ProofBodyKind>(SyntaxRole::Body)?)?;
        Ok(AttachedProofDeclaration {
            syntax: self.clone(),
            prefix,
            trust: trust.trust,
            trust_attribute_source: trust.attribute_source,
            trust_reason_source: trust.reason_source,
            identity,
            name: required_name(&item.syntax(), false)?,
            generics: optional_generics(&item.syntax())?,
            parameter_group,
            where_clauses: where_clauses(&item.syntax())?,
            contracts: attach_contracts(&item)?,
            authored_return,
            body,
            trailing_recovery: self
                .ordered_exact_children::<ErrorNodeKind>(SyntaxRoleClass::Recovery)?
                .into_boxed_slice(),
        })
    }
}

struct AttachedProofTrust {
    trust: Option<ProofTrustSyntax>,
    attribute_source: Option<arcweft_source::SourceSpan>,
    reason_source: Option<arcweft_source::SourceSpan>,
}

fn attach_proof_trust(prefix: &AttachedItemPrefix) -> AttachedProofTrust {
    let attributes = prefix
        .attributes()
        .iter()
        .filter(|attribute| is_verify_trusted_attribute(attribute))
        .collect::<Vec<_>>();
    let [] = attributes.as_slice() else {
        return attach_authored_proof_trust(&attributes);
    };
    AttachedProofTrust {
        trust: Some(ProofTrustSyntax::Verified),
        attribute_source: None,
        reason_source: None,
    }
}

fn attach_authored_proof_trust(attributes: &[&AttachedOuterAttribute]) -> AttachedProofTrust {
    let [attribute] = attributes else {
        return AttachedProofTrust {
            trust: None,
            attribute_source: None,
            reason_source: None,
        };
    };
    let attribute_source = attribute.syntax().source_span();
    let recovery = || AttachedProofTrust {
        trust: None,
        attribute_source: None,
        reason_source: None,
    };
    if attribute.issue().is_some()
        || attribute.recovery().is_some()
        || matches!(attribute.close_state(), AttachedDelimiterState::Missing(_))
    {
        return recovery();
    }
    let AttachedOuterAttributeForm::Parenthesized {
        arguments,
        terminator: SyntaxCallArgumentListTerminator::Closed,
    } = attribute.form()
    else {
        return recovery();
    };
    let [argument] = arguments.as_ref() else {
        return recovery();
    };
    let SyntaxCallArgumentProjection::Named {
        name: Ok(name),
        equals: SyntaxRequiredTokenState::Present,
        ..
    } = argument.projection()
    else {
        return recovery();
    };
    if name.as_str() != "reason" {
        return recovery();
    }
    let AttachedAttributeValue::Authored(expression) = argument.value() else {
        return recovery();
    };
    let ExpressionProjection::Literal(literal) = expression.projection() else {
        return recovery();
    };
    let SyntaxLiteralValue::String { value, .. } = literal.value() else {
        return recovery();
    };
    let Ok(reason) = TrustReasonSyntax::try_new(value.clone()) else {
        return recovery();
    };
    let SyntaxRole::Argument(argument_ordinal) = argument.syntax().role() else {
        return recovery();
    };
    let Some(reason_source) = attribute
        .component(ExpressionComponentRole::CallArgument {
            argument: argument_ordinal,
            part: SyntaxCallArgumentPart::Value,
        })
        .cloned()
    else {
        return recovery();
    };
    let range = attribute.syntax().range();
    AttachedProofTrust {
        trust: Some(ProofTrustSyntax::Trusted {
            reason,
            attribute_range: TextRange::new(range.start(), range.end()),
        }),
        attribute_source: Some(attribute_source),
        reason_source: Some(reason_source),
    }
}

impl AstNode<FunctionItemKind> {
    /// Binds the complete ordinary function without detached syntax or body alternatives.
    pub fn semantics(&self) -> Result<AttachedFunctionDeclaration, SyntaxAccessError> {
        let item = TypedItemNode::Function(self.clone());
        let parameter_groups = self.callable_parameter_groups()?;
        let authored_return = self
            .optional_exact_child::<ReturnTypeKind>(SyntaxRole::ReturnType)?
            .map(|syntax| syntax.callable_semantics())
            .transpose()?;
        let body =
            attach_function_body(self.required_exact_child::<FunctionBodyKind>(SyntaxRole::Body)?)?;
        Ok(AttachedFunctionDeclaration {
            syntax: self.clone(),
            prefix: item.attached_prefix()?,
            name: required_name(&item.syntax(), false)?,
            generics: optional_generics(&item.syntax())?,
            parameter_groups,
            where_clauses: where_clauses(&item.syntax())?,
            contracts: attach_contracts(&item)?,
            authored_return,
            body,
            trailing_recovery: self
                .ordered_exact_children::<ErrorNodeKind>(SyntaxRoleClass::Recovery)?
                .into_boxed_slice(),
        })
    }

    /// Binds every source-ordered fixed group without requiring a function body.
    pub(crate) fn callable_parameter_groups(
        &self,
    ) -> Result<Box<[AttachedFixedParameterGroup]>, SyntaxAccessError> {
        let mut next_parameter_ordinal = 0;
        let groups = self
            .syntax()
            .children()
            .into_iter()
            .filter(|child| child.kind() == SyntaxKind::FixedParameterGroup)
            .enumerate()
            .map(|(group_ordinal, child)| {
                if child.role() != SyntaxRole::ParameterGroup {
                    return Err(SyntaxAccessError::InvalidItemProjection { id: child.id() });
                }
                let group_ordinal = u16::try_from(group_ordinal)
                    .map_err(|_| SyntaxAccessError::InvalidItemProjection { id: self.id() })?;
                child
                    .cast::<FixedParameterGroupKind>()?
                    .callable_semantics(group_ordinal, &mut next_parameter_ordinal)
            })
            .collect::<Result<Vec<_>, _>>()?;
        if groups.is_empty() {
            return Err(SyntaxAccessError::InvalidItemProjection { id: self.id() });
        }
        Ok(groups.into_boxed_slice())
    }

    /// Binds every method parameter group, preserving receiver parameters as a
    /// distinct typed family without fabricating a type child.
    pub(crate) fn method_parameter_groups(
        &self,
    ) -> Result<Box<[AttachedMethodParameterGroup]>, SyntaxAccessError> {
        let mut next_parameter_ordinal = 0;
        let groups = self
            .syntax()
            .children()
            .into_iter()
            .filter(|child| child.kind() == SyntaxKind::FixedParameterGroup)
            .enumerate()
            .map(|(group_ordinal, child)| {
                if child.role() != SyntaxRole::ParameterGroup {
                    return Err(SyntaxAccessError::InvalidItemProjection { id: child.id() });
                }
                let group_ordinal = u16::try_from(group_ordinal)
                    .map_err(|_| SyntaxAccessError::InvalidItemProjection { id: self.id() })?;
                attach_method_parameter_group(
                    child.cast()?,
                    group_ordinal,
                    &mut next_parameter_ordinal,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        if groups.is_empty() {
            return Err(SyntaxAccessError::InvalidItemProjection { id: self.id() });
        }
        Ok(groups.into_boxed_slice())
    }
}

fn attach_parameter_group(
    syntax: AstNode<FixedParameterGroupKind>,
    group_ordinal: u16,
    next_parameter_ordinal: &mut u16,
) -> Result<AttachedFixedParameterGroup, SyntaxAccessError> {
    let parameters = syntax
        .parameters()?
        .into_iter()
        .enumerate()
        .map(|(ordinal, parameter)| {
            let parameter_ordinal = u16::try_from(ordinal)
                .map_err(|_| SyntaxAccessError::InvalidItemProjection { id: syntax.id() })?;
            if parameter.role() != SyntaxRole::Parameter(parameter_ordinal) {
                return Err(SyntaxAccessError::InvalidItemProjection { id: parameter.id() });
            }
            let source_ordinal = *next_parameter_ordinal;
            *next_parameter_ordinal = next_parameter_ordinal
                .checked_add(1)
                .ok_or(SyntaxAccessError::InvalidItemProjection { id: syntax.id() })?;
            attach_callable_parameter(parameter, source_ordinal, group_ordinal, parameter_ordinal)
        })
        .collect::<Result<Vec<_>, _>>()?
        .into_boxed_slice();
    Ok(AttachedFixedParameterGroup {
        open: syntax.required_exact_child::<OpenParenKind>(SyntaxRole::OpenDelimiter)?,
        close: syntax.required_exact_child::<CloseParenKind>(SyntaxRole::CloseDelimiter)?,
        syntax,
        source_ordinal: group_ordinal,
        parameters,
    })
}

fn attach_method_parameter_group(
    syntax: AstNode<FixedParameterGroupKind>,
    group_ordinal: u16,
    next_parameter_ordinal: &mut u16,
) -> Result<AttachedMethodParameterGroup, SyntaxAccessError> {
    let parameters = syntax
        .parameters()?
        .into_iter()
        .enumerate()
        .map(|(ordinal, parameter)| {
            let parameter_ordinal = u16::try_from(ordinal)
                .map_err(|_| SyntaxAccessError::InvalidItemProjection { id: syntax.id() })?;
            if parameter.role() != SyntaxRole::Parameter(parameter_ordinal) {
                return Err(SyntaxAccessError::InvalidItemProjection { id: parameter.id() });
            }
            let source_ordinal = *next_parameter_ordinal;
            *next_parameter_ordinal = next_parameter_ordinal
                .checked_add(1)
                .ok_or(SyntaxAccessError::InvalidItemProjection { id: syntax.id() })?;
            if let Some(receiver) = parameter.syntax().method_receiver_projection() {
                attach_method_receiver(
                    parameter,
                    source_ordinal,
                    group_ordinal,
                    parameter_ordinal,
                    receiver,
                )
                .map(Box::new)
                .map(AttachedMethodParameter::Receiver)
            } else {
                if parameter
                    .optional_family_child::<TypeFamily>(SyntaxRole::ParameterType)?
                    .is_none()
                {
                    return Err(SyntaxAccessError::MissingMethodReceiverProjection {
                        id: parameter.id(),
                    });
                }
                attach_callable_parameter(
                    parameter,
                    source_ordinal,
                    group_ordinal,
                    parameter_ordinal,
                )
                .map(Box::new)
                .map(AttachedMethodParameter::Typed)
            }
        })
        .collect::<Result<Vec<_>, _>>()?
        .into_boxed_slice();
    Ok(AttachedMethodParameterGroup {
        open: syntax.required_exact_child::<OpenParenKind>(SyntaxRole::OpenDelimiter)?,
        close: syntax.required_exact_child::<CloseParenKind>(SyntaxRole::CloseDelimiter)?,
        syntax,
        source_ordinal: group_ordinal,
        parameters,
    })
}

fn attach_callable_parameter(
    parameter: AstNode<ParameterKind>,
    source_ordinal: u16,
    group_ordinal: u16,
    parameter_ordinal: u16,
) -> Result<AttachedCallableParameter, SyntaxAccessError> {
    let pattern = parameter
        .required_family_child::<PatternFamily>(SyntaxRole::ParameterPattern)?
        .semantic()?;
    let ty = parameter
        .required_family_child::<TypeFamily>(SyntaxRole::ParameterType)?
        .semantic()?;
    let colon = punctuation(&parameter.required_exact_child::<ColonKind>(SyntaxRole::Colon)?);
    Ok(AttachedCallableParameter {
        pattern,
        colon,
        ty,
        kind: parameter
            .optional_exact_child::<RestParameterMarkerKind>(SyntaxRole::Kind)?
            .map_or(AttachedCallableParameterKind::Fixed, |marker| {
                AttachedCallableParameterKind::Rest { marker }
            }),
        default: parameter
            .optional_exact_child::<EqualsKind>(SyntaxRole::Equals)?
            .map(|equals| {
                Ok::<_, SyntaxAccessError>(AttachedCallableParameterDefault {
                    equals,
                    value: parameter
                        .required_family_child::<ExpressionFamily>(SyntaxRole::Value)?
                        .semantic()?,
                })
            })
            .transpose()?,
        recovery: parameter
            .ordered_exact_children::<ErrorNodeKind>(SyntaxRoleClass::Recovery)?
            .into_boxed_slice(),
        syntax: parameter,
        source_ordinal,
        group_ordinal,
        parameter_ordinal,
    })
}

fn attach_method_receiver(
    parameter: AstNode<ParameterKind>,
    source_ordinal: u16,
    group_ordinal: u16,
    parameter_ordinal: u16,
    projection: &PendingMethodReceiverProjection,
) -> Result<AttachedMethodReceiver, SyntaxAccessError> {
    let invalid = || SyntaxAccessError::InvalidMethodReceiverProjection { id: parameter.id() };
    if parameter
        .optional_family_child::<TypeFamily>(SyntaxRole::ParameterType)?
        .is_some()
        || parameter
            .optional_exact_child::<RestParameterMarkerKind>(SyntaxRole::Kind)?
            .is_some()
        || parameter
            .optional_exact_child::<EqualsKind>(SyntaxRole::Equals)?
            .is_some()
        || parameter
            .optional_family_child::<ExpressionFamily>(SyntaxRole::Value)?
            .is_some()
    {
        return Err(invalid());
    }
    let pattern = parameter
        .required_family_child::<PatternFamily>(SyntaxRole::ParameterPattern)?
        .semantic()?;
    let pattern_whole = pattern.whole_source_span();
    let pattern_name = pattern
        .component(PatternComponentRole::Name)
        .ok_or_else(invalid)?;
    let pattern_mut = pattern.component(PatternComponentRole::MutKeyword);
    let expected_family = match projection {
        PendingMethodReceiverProjection::Owned {
            mut_keyword: Some(expected),
            ..
        } => {
            if pattern_mut.as_ref().map(arcweft_source::SourceSpan::range) != Some(*expected) {
                return Err(invalid());
            }
            PatternSyntaxFamily::MutableBinding
        }
        PendingMethodReceiverProjection::Owned {
            mut_keyword: None, ..
        }
        | PendingMethodReceiverProjection::SharedReference { .. }
        | PendingMethodReceiverProjection::MutableReference { .. } => {
            if pattern_mut.is_some() {
                return Err(invalid());
            }
            PatternSyntaxFamily::Binding
        }
    };
    if projection.whole() != pattern_whole.range()
        || projection.self_keyword() != pattern_name.range()
        || pattern.family() != expected_family
    {
        return Err(invalid());
    }
    let source_span = |range| parameter.syntax().source_span_for_range(range);
    Ok(AttachedMethodReceiver {
        kind: match projection.kind() {
            MethodReceiverSyntaxKind::Owned => AttachedMethodReceiverKind::Owned,
            MethodReceiverSyntaxKind::SharedReference => {
                AttachedMethodReceiverKind::SharedReference
            }
            MethodReceiverSyntaxKind::MutableReference => {
                AttachedMethodReceiverKind::MutableReference
            }
        },
        whole_source: source_span(projection.whole()),
        ampersand_source: projection.ampersand().map(source_span),
        mut_keyword_source: projection.mut_keyword().map(source_span),
        self_keyword_source: source_span(projection.self_keyword()),
        syntax: parameter,
        source_ordinal,
        group_ordinal,
        parameter_ordinal,
        pattern,
    })
}

/// Whether receiver placement or positional-rest structure violates the
/// method-callable grammar while retaining every typed source parameter.
pub(super) fn method_parameter_shape_has_recovery(groups: &[AttachedMethodParameterGroup]) -> bool {
    let final_group = groups.len().saturating_sub(1);
    let mut saw_receiver = false;
    let mut saw_rest = false;
    for (group_ordinal, group) in groups.iter().enumerate() {
        let final_parameter = group.parameters().len().saturating_sub(1);
        for (parameter_ordinal, parameter) in group.parameters().iter().enumerate() {
            match parameter {
                AttachedMethodParameter::Receiver(_) => {
                    if saw_receiver || group_ordinal != 0 || parameter_ordinal != 0 {
                        return true;
                    }
                    saw_receiver = true;
                }
                AttachedMethodParameter::Typed(parameter) if parameter.is_rest() => {
                    if saw_rest
                        || group_ordinal != final_group
                        || parameter_ordinal != final_parameter
                        || parameter.default().is_some()
                    {
                        return true;
                    }
                    saw_rest = true;
                }
                AttachedMethodParameter::Typed(_) => {}
            }
        }
    }
    false
}

fn attach_contracts(
    item: &TypedItemNode,
) -> Result<Box<[AttachedCallableContractClause]>, SyntaxAccessError> {
    let mut source_ordinal = 0_u16;
    let mut requires_ordinal = 0_u16;
    let mut ensures_ordinal = 0_u16;
    let mut effects_ordinal = 0_u16;
    let mut saw_ensures = false;
    let mut contracts = Vec::new();
    for child in item.syntax().children().into_iter().filter(|child| {
        matches!(
            child.kind(),
            SyntaxKind::RequiresClause | SyntaxKind::EnsuresClause | SyntaxKind::EffectsClause
        )
    }) {
        if role_ordinal(child.id(), child.role(), SyntaxRoleClass::ContractClause)?
            != source_ordinal
        {
            return Err(SyntaxAccessError::InvalidItemProjection { id: child.id() });
        }
        let contract = match child.kind() {
            SyntaxKind::RequiresClause => {
                let condition = attached_scalar_contract_condition(&child)?;
                let family_ordinal = requires_ordinal;
                AttachedCallableContractClause::Requires {
                    syntax: child.cast()?,
                    source_ordinal,
                    family_ordinal,
                    condition,
                    out_of_order: saw_ensures,
                }
            }
            SyntaxKind::EnsuresClause => {
                let condition = attached_scalar_contract_condition(&child)?;
                saw_ensures = true;
                let family_ordinal = ensures_ordinal;
                AttachedCallableContractClause::Ensures {
                    syntax: child.cast()?,
                    source_ordinal,
                    family_ordinal,
                    condition,
                }
            }
            SyntaxKind::EffectsClause => {
                let operands = child
                    .ordered_children(SyntaxRoleClass::ContractOperand)?
                    .into_iter()
                    .map(|operand| {
                        super::family::FamilyNode::<ExpressionFamily>::new(operand)?.semantic()
                    })
                    .collect::<Result<Vec<_>, SyntaxAccessError>>()?
                    .into_boxed_slice();
                let open = child
                    .optional_unique_child(SyntaxRole::OpenDelimiter)?
                    .map(|open| open.cast())
                    .transpose()?;
                let close = child
                    .optional_unique_child(SyntaxRole::CloseDelimiter)?
                    .map(|close| close.cast())
                    .transpose()?;
                if open.is_some() != close.is_some() {
                    return Err(SyntaxAccessError::InvalidItemProjection { id: child.id() });
                }
                AttachedCallableContractClause::Effects {
                    syntax: child.cast()?,
                    source_ordinal,
                    family_ordinal: effects_ordinal,
                    open,
                    operands,
                    close,
                }
            }
            _ => unreachable!("contract filter admits requires, ensures, and effects"),
        };
        match child.kind() {
            SyntaxKind::RequiresClause => {
                requires_ordinal = requires_ordinal
                    .checked_add(1)
                    .ok_or(SyntaxAccessError::InvalidItemProjection { id: item.id() })?;
            }
            SyntaxKind::EnsuresClause => {
                ensures_ordinal = ensures_ordinal
                    .checked_add(1)
                    .ok_or(SyntaxAccessError::InvalidItemProjection { id: item.id() })?;
            }
            SyntaxKind::EffectsClause => {
                effects_ordinal = effects_ordinal
                    .checked_add(1)
                    .ok_or(SyntaxAccessError::InvalidItemProjection { id: item.id() })?;
            }
            _ => unreachable!("contract filter admits requires, ensures, and effects"),
        }
        contracts.push(contract);
        source_ordinal = source_ordinal
            .checked_add(1)
            .ok_or(SyntaxAccessError::InvalidItemProjection { id: item.id() })?;
    }
    Ok(contracts.into_boxed_slice())
}

fn attached_scalar_contract_condition(
    clause: &super::SyntaxNodeHandle,
) -> Result<AttachedExpressionNode, SyntaxAccessError> {
    let condition = clause
        .optional_unique_child(SyntaxRole::ContractOperand(0))?
        .ok_or(SyntaxAccessError::InvalidItemProjection { id: clause.id() })?;
    super::family::FamilyNode::<ExpressionFamily>::new(condition)?.semantic()
}

fn role_ordinal(
    owner: super::SyntaxNodeId,
    role: SyntaxRole,
    expected: SyntaxRoleClass,
) -> Result<u16, SyntaxAccessError> {
    if role.class() != expected {
        return Err(SyntaxAccessError::InvalidItemProjection { id: owner });
    }
    u16::try_from(role.ordinal().unwrap_or(u32::MAX))
        .map_err(|_| SyntaxAccessError::InvalidItemProjection { id: owner })
}

fn attach_predicate_body(
    syntax: AstNode<PredicateBodyKind>,
) -> Result<AttachedPredicateBody, SyntaxAccessError> {
    match syntax.content()? {
        DeclarationBodyNode::Missing(missing) => {
            Ok(AttachedPredicateBody::Missing { syntax, missing })
        }
        DeclarationBodyNode::Body(body) if body.kind() == SyntaxKind::ExpressionBody => {
            let body = body.cast::<ExpressionBodyKind>()?;
            Ok(AttachedPredicateBody::Expression {
                syntax,
                expression: Box::new(body.expression()?.semantic()?),
            })
        }
        DeclarationBodyNode::Body(body) if body.kind() == SyntaxKind::PredicateBlock => {
            Ok(AttachedPredicateBody::Block {
                syntax,
                block: body.cast()?,
            })
        }
        DeclarationBodyNode::Body(_) => {
            Err(SyntaxAccessError::InvalidItemProjection { id: syntax.id() })
        }
    }
}

fn attach_proof_body(
    syntax: AstNode<ProofBodyKind>,
) -> Result<AttachedProofBody, SyntaxAccessError> {
    match syntax.content()? {
        DeclarationBodyNode::Missing(missing) => Ok(AttachedProofBody::Missing { syntax, missing }),
        DeclarationBodyNode::Body(body) if body.kind() == SyntaxKind::ExpressionBody => {
            let body = body.cast::<ExpressionBodyKind>()?;
            Ok(AttachedProofBody::Expression {
                syntax,
                expression: Box::new(body.expression()?.semantic()?),
            })
        }
        DeclarationBodyNode::Body(body) if body.kind() == SyntaxKind::ProofBlock => {
            Ok(AttachedProofBody::Block {
                syntax,
                block: body.cast()?,
            })
        }
        DeclarationBodyNode::Body(_) => {
            Err(SyntaxAccessError::InvalidItemProjection { id: syntax.id() })
        }
    }
}

pub(super) fn attach_function_body(
    syntax: AstNode<FunctionBodyKind>,
) -> Result<AttachedFunctionBody, SyntaxAccessError> {
    match syntax.content()? {
        DeclarationBodyNode::Missing(missing) => {
            Ok(AttachedFunctionBody::Missing { syntax, missing })
        }
        DeclarationBodyNode::Body(body) if body.kind() == SyntaxKind::Block => {
            Ok(AttachedFunctionBody::Block {
                syntax,
                block: body.cast()?,
            })
        }
        DeclarationBodyNode::Body(_) => {
            Err(SyntaxAccessError::InvalidItemProjection { id: syntax.id() })
        }
    }
}

/// A canonical assertion mode or exact recovered mode node.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedAssertionMode {
    Resolved {
        syntax: NameNode,
        value: AssertionMode,
    },
    Recovered {
        syntax: NameNode,
    },
}

impl AttachedAssertionMode {
    pub const fn value(&self) -> Option<AssertionMode> {
        match self {
            Self::Resolved { value, .. } => Some(*value),
            Self::Recovered { .. } => None,
        }
    }

    pub const fn syntax(&self) -> &NameNode {
        match self {
            Self::Resolved { syntax, .. } | Self::Recovered { syntax } => syntax,
        }
    }

    pub const fn is_recovered(&self) -> bool {
        matches!(self, Self::Recovered { .. })
    }
}

/// Complete typed assertion statement payload.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedAssertionStatement {
    syntax: AstNode<AssertionStatementKind>,
    mode: AttachedAssertionMode,
    open: AstNode<OpenParenKind>,
    close: AstNode<CloseParenKind>,
    conditions: Box<[AttachedExpressionNode]>,
    has_recovery: bool,
}

impl AttachedAssertionStatement {
    pub const fn syntax(&self) -> &AstNode<AssertionStatementKind> {
        &self.syntax
    }

    pub const fn mode(&self) -> &AttachedAssertionMode {
        &self.mode
    }

    pub const fn conditions(&self) -> &[AttachedExpressionNode] {
        &self.conditions
    }

    pub const fn open(&self) -> &AstNode<OpenParenKind> {
        &self.open
    }

    pub const fn close(&self) -> &AstNode<CloseParenKind> {
        &self.close
    }

    pub const fn has_recovery(&self) -> bool {
        self.has_recovery
    }
}

impl AstNode<AssertionStatementKind> {
    /// Binds mode, delimiters, and conditions without reparsing statement text.
    pub fn semantics(&self) -> Result<AttachedAssertionStatement, SyntaxAccessError> {
        let mode_syntax = self.required_family_child::<NameFamily>(SyntaxRole::Name)?;
        let projection = self
            .syntax()
            .assertion_projection()
            .ok_or(SyntaxAccessError::InvalidItemProjection { id: self.id() })?;
        let mode = match (projection.mode(), mode_syntax.kind()) {
            (Some(value), SyntaxKind::NameReference) => AttachedAssertionMode::Resolved {
                syntax: mode_syntax.clone(),
                value,
            },
            (None, SyntaxKind::NameReference | SyntaxKind::MissingName) => {
                AttachedAssertionMode::Recovered {
                    syntax: mode_syntax.clone(),
                }
            }
            _ => return Err(SyntaxAccessError::InvalidItemProjection { id: self.id() }),
        };
        let open = self.required_exact_child::<OpenParenKind>(SyntaxRole::OpenDelimiter)?;
        let close = self.required_exact_child::<CloseParenKind>(SyntaxRole::CloseDelimiter)?;
        let conditions = self
            .conditions()?
            .into_iter()
            .map(|condition| condition.semantic())
            .collect::<Result<Vec<_>, _>>()?
            .into_boxed_slice();
        let has_recovery = mode.is_recovered()
            || open.range().is_empty()
            || close.range().is_empty()
            || conditions.is_empty()
            || conditions
                .iter()
                .any(|condition| condition.projection().has_recovery())
            || !self
                .ordered_exact_children::<ErrorNodeKind>(SyntaxRoleClass::Recovery)?
                .is_empty();
        Ok(AttachedAssertionStatement {
            syntax: self.clone(),
            mode,
            open,
            close,
            conditions,
            has_recovery,
        })
    }
}

fn punctuation<K: super::node::ExactAstKind>(syntax: &AstNode<K>) -> AttachedRequiredPunctuation {
    if syntax.range().is_empty() {
        AttachedRequiredPunctuation::Missing(syntax.source_span())
    } else {
        AttachedRequiredPunctuation::Authored(syntax.source_span())
    }
}

#[cfg(test)]
#[path = "callable/tests.rs"]
mod tests;
