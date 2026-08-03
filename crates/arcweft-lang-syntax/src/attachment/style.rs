//! Typed native Style ownership over the attached grammar tree.

use arcweft_source::SourceSpan;

pub use crate::grammar::style_projection::{
    StyleEnvironmentComparison as StyleEnvironmentComparisonKind, StyleEnvironmentConditionIssue,
    StyleEnvironmentField as StyleEnvironmentFieldKind, StyleIdForm, StylePropertyOperation,
    StyleSelectorRelation, StyleSyntaxName, StyleSyntaxNameIssue,
};
use crate::id_ref::SyntaxIdRefSyntax;

use super::node::{
    AstNode, CloseBraceKind, ColonKind, EqualsKind, ErrorNodeKind, MissingBodyKind,
    MissingExpressionKind, MissingNameKind, NameReferenceKind, OpenBraceKind, StyleBodyKind,
    StyleItemKind, StylePropertyDeclarationKind, StyleRuleKind, StyleSelectorKind,
    StyleSelectorSequenceKind, StyleTokenDeclarationKind,
};
use super::{AttachedExpressionNode, AttachedItemPrefix, AttachedTypeRefNode, SyntaxNodeHandle};

/// Parser-owned Style declaration ID or exact current-grammar recovery.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedStyleId {
    Authored {
        syntax: SyntaxNodeHandle,
        reference: SyntaxIdRefSyntax,
        form: StyleIdForm,
        canonical_style_family: bool,
    },
    Invalid {
        syntax: SyntaxNodeHandle,
        reference: SyntaxIdRefSyntax,
    },
    Missing {
        syntax: AstNode<MissingNameKind>,
        reference: SyntaxIdRefSyntax,
    },
}

impl AttachedStyleId {
    pub const fn reference(&self) -> Option<&SyntaxIdRefSyntax> {
        match self {
            Self::Authored { reference, .. }
            | Self::Invalid { reference, .. }
            | Self::Missing { reference, .. } => Some(reference),
        }
    }

    pub const fn form(&self) -> Option<StyleIdForm> {
        match self {
            Self::Authored { form, .. } => Some(*form),
            Self::Invalid { .. } | Self::Missing { .. } => None,
        }
    }

    pub const fn is_canonical_style_family(&self) -> bool {
        matches!(
            self,
            Self::Authored {
                canonical_style_family: true,
                ..
            }
        )
    }

    pub fn has_recovery(&self) -> bool {
        match self {
            Self::Authored {
                reference,
                canonical_style_family,
                ..
            } => reference.value().is_err() || !canonical_style_family,
            Self::Invalid { .. } | Self::Missing { .. } => true,
        }
    }

    /// Exact revision-bound source of the authored ID or recovery owner.
    pub fn source_span(&self) -> SourceSpan {
        match self {
            Self::Authored { syntax, .. } | Self::Invalid { syntax, .. } => syntax.source_span(),
            Self::Missing { syntax, .. } => syntax.source_span(),
        }
    }
}

/// One native Style name bound to its exact source node.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedStyleName {
    syntax: SyntaxNodeHandle,
    value: Result<StyleSyntaxName, StyleSyntaxNameIssue>,
    dotted_component_count: u32,
}

impl AttachedStyleName {
    pub const fn syntax(&self) -> &SyntaxNodeHandle {
        &self.syntax
    }

    pub const fn value(&self) -> Result<&StyleSyntaxName, &StyleSyntaxNameIssue> {
        self.value.as_ref()
    }

    pub const fn has_recovery(&self) -> bool {
        self.value.is_err()
    }

    /// Number of source-authored components separated by `.`.
    pub const fn dotted_component_count(&self) -> u32 {
        self.dotted_component_count
    }

    fn token_id(&self) -> crate::id_ref::SyntaxIdRefSyntax {
        match self.value() {
            Ok(name) => name.relative_token_id().clone(),
            Err(StyleSyntaxNameIssue::Missing) if self.dotted_component_count == 0 => {
                crate::id_ref::SyntaxIdRefSyntax::new(
                    Err(crate::id_ref::SyntaxIdRefIssue::MissingSuffix),
                    crate::id_ref::SyntaxIdRefShape::new(false, false, 0, 0),
                )
            }
            Err(issue) => issue.invalid_token_id(self.dotted_component_count),
        }
    }
}

/// Canonical `=` token, missing insertion, or unsupported authored operator.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedStyleAssignment {
    equals: AstNode<EqualsKind>,
    source: SourceSpan,
    state: AttachedStyleAssignmentState,
    unsupported: Option<AstNode<ErrorNodeKind>>,
}

/// Closed attachment state for one required Style assignment.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AttachedStyleAssignmentState {
    Authored,
    Missing,
    Unsupported,
}

impl AttachedStyleAssignment {
    pub const fn equals(&self) -> &AstNode<EqualsKind> {
        &self.equals
    }

    pub fn source_span(&self) -> SourceSpan {
        self.source.clone()
    }

    pub const fn state(&self) -> AttachedStyleAssignmentState {
        self.state
    }

    pub const fn unsupported_syntax(&self) -> Option<&AstNode<ErrorNodeKind>> {
        self.unsupported.as_ref()
    }

    pub const fn has_recovery(&self) -> bool {
        !matches!(self.state, AttachedStyleAssignmentState::Authored)
    }
}

/// Optional typed annotation of one Style token.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedStyleTypeAnnotation {
    colon: AstNode<ColonKind>,
    value: AttachedTypeRefNode,
}

/// Ordinary attached expression or its exact zero-width missing recovery.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedStyleExpression {
    Authored(Box<AttachedExpressionNode>),
    Missing(AstNode<MissingExpressionKind>),
}

impl AttachedStyleExpression {
    pub const fn authored(&self) -> Option<&AttachedExpressionNode> {
        match self {
            Self::Authored(expression) => Some(expression),
            Self::Missing(_) => None,
        }
    }

    pub const fn missing(&self) -> Option<&AstNode<MissingExpressionKind>> {
        match self {
            Self::Missing(recovery) => Some(recovery),
            Self::Authored(_) => None,
        }
    }

    pub fn source_span(&self) -> SourceSpan {
        match self {
            Self::Authored(expression) => expression.syntax().source_span(),
            Self::Missing(recovery) => recovery.source_span(),
        }
    }

    pub fn has_recovery(&self) -> bool {
        match self {
            Self::Authored(expression) => expression.projection().has_recovery(),
            Self::Missing(_) => true,
        }
    }
}

impl AttachedStyleTypeAnnotation {
    pub const fn colon(&self) -> &AstNode<ColonKind> {
        &self.colon
    }

    pub const fn value(&self) -> &AttachedTypeRefNode {
        &self.value
    }

    pub fn has_recovery(&self) -> bool {
        self.colon.range().is_empty()
            || matches!(self.value.family(), super::AttachedTypeFamily::Recovery)
    }
}

/// One ordered native Style token declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedStyleToken {
    syntax: AstNode<StyleTokenDeclarationKind>,
    source_ordinal: u32,
    name: AttachedStyleName,
    id: SyntaxIdRefSyntax,
    type_annotation: Option<AttachedStyleTypeAnnotation>,
    assignment: AttachedStyleAssignment,
    value: AttachedStyleExpression,
    allowed_at_this_depth: bool,
}

impl AttachedStyleToken {
    pub const fn syntax(&self) -> &AstNode<StyleTokenDeclarationKind> {
        &self.syntax
    }

    pub const fn source_ordinal(&self) -> u32 {
        self.source_ordinal
    }

    pub const fn name(&self) -> &AttachedStyleName {
        &self.name
    }

    /// Parser-owned relative ID, retaining the exact recovery shape.
    pub const fn id(&self) -> &SyntaxIdRefSyntax {
        &self.id
    }

    pub const fn type_annotation(&self) -> Option<&AttachedStyleTypeAnnotation> {
        self.type_annotation.as_ref()
    }

    pub const fn assignment(&self) -> &AttachedStyleAssignment {
        &self.assignment
    }

    pub const fn value(&self) -> &AttachedStyleExpression {
        &self.value
    }

    pub const fn is_allowed_at_this_depth(&self) -> bool {
        self.allowed_at_this_depth
    }

    pub fn has_recovery(&self) -> bool {
        self.name.has_recovery()
            || self
                .type_annotation
                .as_ref()
                .is_some_and(AttachedStyleTypeAnnotation::has_recovery)
            || self.assignment.has_recovery()
            || self.value.has_recovery()
            || !self.allowed_at_this_depth
    }
}

/// Exact selector relation and its authored separator bytes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedStyleSelectorRelation {
    value: StyleSelectorRelation,
    source: SourceSpan,
}

impl AttachedStyleSelectorRelation {
    pub const fn value(&self) -> StyleSelectorRelation {
        self.value
    }

    pub fn source_span(&self) -> SourceSpan {
        self.source.clone()
    }
}

/// Optional `.part` component of a selector sequence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedStyleSelectorPart {
    separator: SourceSpan,
    name: AttachedStyleName,
}

impl AttachedStyleSelectorPart {
    pub fn separator_span(&self) -> SourceSpan {
        self.separator.clone()
    }

    pub const fn name(&self) -> &AttachedStyleName {
        &self.name
    }
}

/// One ordered `:predicate` component.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedStylePredicate {
    source_ordinal: u16,
    colon: SourceSpan,
    name: AttachedStyleName,
}

impl AttachedStylePredicate {
    pub const fn source_ordinal(&self) -> u16 {
        self.source_ordinal
    }

    pub fn colon_span(&self) -> SourceSpan {
        self.colon.clone()
    }

    pub const fn name(&self) -> &AttachedStyleName {
        &self.name
    }
}

/// One compound selector with its exact preceding relation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedStyleSelectorSequence {
    syntax: AstNode<StyleSelectorSequenceKind>,
    source_ordinal: u32,
    relation: Option<AttachedStyleSelectorRelation>,
    element: Option<AttachedStyleName>,
    part: Option<AttachedStyleSelectorPart>,
    predicates: Box<[AttachedStylePredicate]>,
    recovery: Option<AstNode<ErrorNodeKind>>,
}

impl AttachedStyleSelectorSequence {
    pub const fn syntax(&self) -> &AstNode<StyleSelectorSequenceKind> {
        &self.syntax
    }

    pub const fn source_ordinal(&self) -> u32 {
        self.source_ordinal
    }

    pub const fn relation(&self) -> Option<&AttachedStyleSelectorRelation> {
        self.relation.as_ref()
    }

    pub const fn element(&self) -> Option<&AttachedStyleName> {
        self.element.as_ref()
    }

    pub const fn part(&self) -> Option<&AttachedStyleSelectorPart> {
        self.part.as_ref()
    }

    pub fn predicates(&self) -> &[AttachedStylePredicate] {
        &self.predicates
    }

    pub fn has_recovery(&self) -> bool {
        self.recovery.is_some()
            || self
                .element
                .as_ref()
                .is_some_and(AttachedStyleName::has_recovery)
            || self
                .part
                .as_ref()
                .is_some_and(|part| part.name().has_recovery())
            || self
                .predicates
                .iter()
                .any(|predicate| predicate.name().has_recovery())
    }
}

/// Complete ordered selector projection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedStyleSelector {
    syntax: AstNode<StyleSelectorKind>,
    sequences: Box<[AttachedStyleSelectorSequence]>,
    recoveries: Box<[AstNode<ErrorNodeKind>]>,
    missing: Option<AstNode<MissingNameKind>>,
}

impl AttachedStyleSelector {
    pub const fn syntax(&self) -> &AstNode<StyleSelectorKind> {
        &self.syntax
    }

    pub fn sequences(&self) -> &[AttachedStyleSelectorSequence] {
        &self.sequences
    }

    pub fn recoveries(&self) -> &[AstNode<ErrorNodeKind>] {
        &self.recoveries
    }

    pub const fn missing(&self) -> Option<&AstNode<MissingNameKind>> {
        self.missing.as_ref()
    }

    pub fn has_recovery(&self) -> bool {
        self.missing.is_some()
            || !self.recoveries.is_empty()
            || self
                .sequences
                .iter()
                .any(AttachedStyleSelectorSequence::has_recovery)
    }
}

/// One canonical or recovered property declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedStyleProperty {
    syntax: AstNode<StylePropertyDeclarationKind>,
    source_ordinal: u32,
    name: AttachedStyleName,
    operation: StylePropertyOperation,
    append_keyword: Option<AstNode<NameReferenceKind>>,
    assignment: AttachedStyleAssignment,
    value: AttachedStyleExpression,
}

impl AttachedStyleProperty {
    pub const fn syntax(&self) -> &AstNode<StylePropertyDeclarationKind> {
        &self.syntax
    }

    pub const fn source_ordinal(&self) -> u32 {
        self.source_ordinal
    }

    pub const fn name(&self) -> &AttachedStyleName {
        &self.name
    }

    pub const fn operation(&self) -> StylePropertyOperation {
        self.operation
    }

    pub const fn append_keyword(&self) -> Option<&AstNode<NameReferenceKind>> {
        self.append_keyword.as_ref()
    }

    pub const fn assignment(&self) -> &AttachedStyleAssignment {
        &self.assignment
    }

    pub const fn value(&self) -> &AttachedStyleExpression {
        &self.value
    }

    pub fn has_recovery(&self) -> bool {
        self.name.has_recovery() || self.assignment.has_recovery() || self.value.has_recovery()
    }
}

/// Braced property body of one selector rule.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedStyleRuleBody {
    syntax: AstNode<StyleBodyKind>,
    open: AstNode<OpenBraceKind>,
    close: AstNode<CloseBraceKind>,
    declarations: Box<[AttachedStyleProperty]>,
}

impl AttachedStyleRuleBody {
    pub const fn syntax(&self) -> &AstNode<StyleBodyKind> {
        &self.syntax
    }

    pub const fn open_delimiter(&self) -> &AstNode<OpenBraceKind> {
        &self.open
    }

    pub const fn close_delimiter(&self) -> &AstNode<CloseBraceKind> {
        &self.close
    }

    pub fn declarations(&self) -> &[AttachedStyleProperty] {
        &self.declarations
    }

    pub fn has_recovery(&self) -> bool {
        self.close.range().is_empty()
            || self
                .declarations
                .iter()
                .any(AttachedStyleProperty::has_recovery)
    }
}

/// One ordered native selector rule.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedStyleRule {
    syntax: AstNode<StyleRuleKind>,
    source_ordinal: u32,
    selector: AttachedStyleSelector,
    body: AttachedStyleRuleBody,
}

impl AttachedStyleRule {
    pub const fn syntax(&self) -> &AstNode<StyleRuleKind> {
        &self.syntax
    }

    pub const fn source_ordinal(&self) -> u32 {
        self.source_ordinal
    }

    pub const fn selector(&self) -> &AttachedStyleSelector {
        &self.selector
    }

    pub const fn body(&self) -> &AttachedStyleRuleBody {
        &self.body
    }

    pub fn has_recovery(&self) -> bool {
        self.selector.has_recovery() || self.body.has_recovery()
    }
}

/// Closed Style body member inventory in exact source order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedStyleMember {
    Token(Box<AttachedStyleToken>),
    Rule(Box<AttachedStyleRule>),
    Environment(Box<AttachedStyleEnvironment>),
    Error {
        source_ordinal: u32,
        syntax: AstNode<ErrorNodeKind>,
    },
}

impl AttachedStyleMember {
    pub const fn source_ordinal(&self) -> u32 {
        match self {
            Self::Token(token) => token.source_ordinal(),
            Self::Rule(rule) => rule.source_ordinal(),
            Self::Environment(environment) => environment.source_ordinal(),
            Self::Error { source_ordinal, .. } => *source_ordinal,
        }
    }

    pub fn has_recovery(&self) -> bool {
        match self {
            Self::Token(token) => token.has_recovery(),
            Self::Rule(rule) => rule.has_recovery(),
            Self::Environment(environment) => environment.has_recovery(),
            Self::Error { .. } => true,
        }
    }
}

/// Missing or braced native Style body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedStyleBody {
    Missing(AstNode<MissingBodyKind>),
    Braced {
        syntax: AstNode<StyleBodyKind>,
        open: AstNode<OpenBraceKind>,
        close: AstNode<CloseBraceKind>,
        members: Box<[AttachedStyleMember]>,
    },
}

impl AttachedStyleBody {
    /// Concrete body owner, absent only for a missing body recovery.
    pub const fn syntax(&self) -> Option<&AstNode<StyleBodyKind>> {
        match self {
            Self::Missing(_) => None,
            Self::Braced { syntax, .. } => Some(syntax),
        }
    }

    /// Opening delimiter of a concrete body.
    pub const fn open_delimiter(&self) -> Option<&AstNode<OpenBraceKind>> {
        match self {
            Self::Missing(_) => None,
            Self::Braced { open, .. } => Some(open),
        }
    }

    /// Closing delimiter, including its zero-width missing-token owner.
    pub const fn close_delimiter(&self) -> Option<&AstNode<CloseBraceKind>> {
        match self {
            Self::Missing(_) => None,
            Self::Braced { close, .. } => Some(close),
        }
    }

    /// Exact revision-bound span of the body or missing-body recovery.
    pub fn source_span(&self) -> SourceSpan {
        match self {
            Self::Missing(syntax) => syntax.source_span(),
            Self::Braced { syntax, .. } => syntax.source_span(),
        }
    }

    pub fn members(&self) -> &[AttachedStyleMember] {
        match self {
            Self::Missing(_) => &[],
            Self::Braced { members, .. } => members,
        }
    }

    pub fn is_closed(&self) -> bool {
        matches!(self, Self::Braced { close, .. } if !close.range().is_empty())
    }

    pub fn has_recovery(&self) -> bool {
        matches!(self, Self::Missing(_))
            || !self.is_closed()
            || self.members().iter().any(AttachedStyleMember::has_recovery)
    }
}

/// One source-bound native Style declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedStyleDeclaration {
    syntax: AstNode<StyleItemKind>,
    prefix: AttachedItemPrefix,
    id: AttachedStyleId,
    body: AttachedStyleBody,
    trailing_header_recovery: Option<AstNode<ErrorNodeKind>>,
}

impl AttachedStyleDeclaration {
    pub const fn syntax(&self) -> &AstNode<StyleItemKind> {
        &self.syntax
    }

    pub const fn prefix(&self) -> &AttachedItemPrefix {
        &self.prefix
    }

    pub const fn id(&self) -> &AttachedStyleId {
        &self.id
    }

    pub const fn body(&self) -> &AttachedStyleBody {
        &self.body
    }

    pub const fn has_header_trailing_recovery(&self) -> bool {
        self.trailing_header_recovery.is_some()
    }

    pub fn has_recovery(&self) -> bool {
        self.id.has_recovery()
            || self.body.has_recovery()
            || self.trailing_header_recovery.is_some()
    }
}

mod attach;
mod environment;

pub use environment::*;

#[cfg(test)]
mod tests;
