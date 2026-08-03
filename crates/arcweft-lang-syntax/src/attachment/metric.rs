//! Typed Metric declaration ownership over the attached grammar tree.

use std::collections::BTreeSet;

use super::family::ExpressionFamily;
use super::node::{
    AstNode, CloseBraceKind, ColonKind, DeclarationHeaderKind, EqualsKind,
    ErrorDeclarationMemberKind, ErrorNodeKind, MetricBodyKind, MetricBucketsMemberKind,
    MetricDeclarationItemKind, MetricKindKind, MetricLabelKind, MetricLabelsBlockKind,
    MetricUnitMemberKind, MissingBodyKind, MissingMemberValueKind, OpenBraceKind,
};
use super::nominal::{punctuation, required_name, required_type};
use super::{
    AttachedExpressionChild, AttachedExpressionNode, AttachedItemPrefix, AttachedRequiredName,
    AttachedRequiredPunctuation, AttachedRetainedHeader, AttachedTypeFamily, AttachedTypeRefNode,
    SyntaxAccessError, SyntaxNodeHandle, TypedItemNode,
};
use crate::expressions::ExpressionProjection;
use crate::grammar::kinds::{MetricKindSyntaxValue, SyntaxKind, SyntaxRole, SyntaxRoleClass};
use crate::literal::{SyntaxLiteralFamily, SyntaxLiteralValue, SyntaxStringKind};
use crate::name::SyntaxName;

/// Duplicate and source-order recovery derived from the typed member list.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AttachedMetricMemberState {
    duplicate: bool,
    out_of_order: bool,
}

impl AttachedMetricMemberState {
    pub const fn is_duplicate(self) -> bool {
        self.duplicate
    }

    pub const fn is_out_of_order(self) -> bool {
        self.out_of_order
    }

    pub const fn has_recovery(self) -> bool {
        self.duplicate || self.out_of_order
    }
}

/// Closed Metric kind or its exact parser-owned recovery node.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedMetricKind {
    Counter(AstNode<MetricKindKind>),
    Gauge(AstNode<MetricKindKind>),
    Histogram(AstNode<MetricKindKind>),
    Missing(AstNode<MetricKindKind>),
    Unknown(AstNode<MetricKindKind>),
}

impl AttachedMetricKind {
    pub const fn syntax(&self) -> &AstNode<MetricKindKind> {
        match self {
            Self::Counter(syntax)
            | Self::Gauge(syntax)
            | Self::Histogram(syntax)
            | Self::Missing(syntax)
            | Self::Unknown(syntax) => syntax,
        }
    }

    pub const fn has_recovery(&self) -> bool {
        matches!(self, Self::Missing(_) | Self::Unknown(_))
    }
}

/// Decoded Metric unit or typed source recovery without a fabricated string.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedMetricUnitValue {
    Decoded {
        expression: AttachedExpressionNode,
        value: Box<str>,
    },
    RecoveredString(AttachedExpressionNode),
    NonString(AttachedExpressionNode),
    Missing(AstNode<MissingMemberValueKind>),
}

impl AttachedMetricUnitValue {
    pub const fn expression(&self) -> Option<&AttachedExpressionNode> {
        match self {
            Self::Decoded { expression, .. }
            | Self::RecoveredString(expression)
            | Self::NonString(expression) => Some(expression),
            Self::Missing(_) => None,
        }
    }

    pub fn decoded_value(&self) -> Option<&str> {
        match self {
            Self::Decoded { value, .. } => Some(value),
            Self::RecoveredString(_) | Self::NonString(_) | Self::Missing(_) => None,
        }
    }

    pub const fn has_recovery(&self) -> bool {
        !matches!(self, Self::Decoded { .. })
    }
}

/// One source-ordered `unit` member.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedMetricUnitMember {
    syntax: AstNode<MetricUnitMemberKind>,
    source_ordinal: u16,
    state: AttachedMetricMemberState,
    assignment: AttachedRequiredPunctuation,
    value: AttachedMetricUnitValue,
}

impl AttachedMetricUnitMember {
    pub const fn syntax(&self) -> &AstNode<MetricUnitMemberKind> {
        &self.syntax
    }

    pub const fn source_ordinal(&self) -> u16 {
        self.source_ordinal
    }

    pub const fn state(&self) -> AttachedMetricMemberState {
        self.state
    }

    pub const fn assignment(&self) -> &AttachedRequiredPunctuation {
        &self.assignment
    }

    pub const fn value(&self) -> &AttachedMetricUnitValue {
        &self.value
    }

    pub const fn has_recovery(&self) -> bool {
        self.state.has_recovery() || self.assignment.is_missing() || self.value.has_recovery()
    }
}

/// One typed label in source order across all authored labels blocks.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedMetricLabel {
    syntax: AstNode<MetricLabelKind>,
    source_ordinal: u16,
    name: AttachedRequiredName,
    colon: AttachedRequiredPunctuation,
    ty: AttachedTypeRefNode,
    duplicate: bool,
}

impl AttachedMetricLabel {
    pub const fn syntax(&self) -> &AstNode<MetricLabelKind> {
        &self.syntax
    }

    pub const fn source_ordinal(&self) -> u16 {
        self.source_ordinal
    }

    pub const fn name(&self) -> &AttachedRequiredName {
        &self.name
    }

    pub const fn colon(&self) -> &AttachedRequiredPunctuation {
        &self.colon
    }

    pub const fn ty(&self) -> &AttachedTypeRefNode {
        &self.ty
    }

    pub const fn is_duplicate(&self) -> bool {
        self.duplicate
    }

    pub fn has_recovery(&self) -> bool {
        self.name.is_missing()
            || self.colon.is_missing()
            || self.ty.family() == AttachedTypeFamily::Recovery
            || self.duplicate
    }
}

/// Missing or braced label schema owned by one `labels` member.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedMetricLabelsBody {
    Missing(AstNode<MissingMemberValueKind>),
    Braced {
        open: AstNode<OpenBraceKind>,
        close: AstNode<CloseBraceKind>,
        labels: Box<[AttachedMetricLabel]>,
    },
}

impl AttachedMetricLabelsBody {
    pub fn labels(&self) -> &[AttachedMetricLabel] {
        match self {
            Self::Missing(_) => &[],
            Self::Braced { labels, .. } => labels,
        }
    }

    pub fn has_recovery(&self) -> bool {
        match self {
            Self::Missing(_) => true,
            Self::Braced { close, labels, .. } => {
                close.range().is_empty() || labels.iter().any(AttachedMetricLabel::has_recovery)
            }
        }
    }
}

/// One source-ordered `labels` member.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedMetricLabelsMember {
    syntax: AstNode<MetricLabelsBlockKind>,
    source_ordinal: u16,
    state: AttachedMetricMemberState,
    body: AttachedMetricLabelsBody,
}

impl AttachedMetricLabelsMember {
    pub const fn syntax(&self) -> &AstNode<MetricLabelsBlockKind> {
        &self.syntax
    }

    pub const fn source_ordinal(&self) -> u16 {
        self.source_ordinal
    }

    pub const fn state(&self) -> AttachedMetricMemberState {
        self.state
    }

    pub const fn body(&self) -> &AttachedMetricLabelsBody {
        &self.body
    }

    pub fn has_recovery(&self) -> bool {
        self.state.has_recovery() || self.body.has_recovery()
    }
}

/// Bracket-sequence buckets or their exact typed recovery.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedMetricBucketsValue {
    Sequence(AttachedExpressionNode),
    NonSequence(AttachedExpressionNode),
    Missing(AstNode<MissingMemberValueKind>),
}

impl AttachedMetricBucketsValue {
    pub const fn expression(&self) -> Option<&AttachedExpressionNode> {
        match self {
            Self::Sequence(expression) | Self::NonSequence(expression) => Some(expression),
            Self::Missing(_) => None,
        }
    }

    pub fn bucket_expressions(&self) -> &[AttachedExpressionChild] {
        match self {
            Self::Sequence(expression) => expression.children(),
            Self::NonSequence(_) | Self::Missing(_) => &[],
        }
    }

    pub fn is_empty_sequence(&self) -> bool {
        matches!(self, Self::Sequence(expression) if expression.children().is_empty())
    }

    pub fn has_recovery(&self) -> bool {
        match self {
            Self::Sequence(expression) => {
                expression.children().is_empty()
                    || expression
                        .children()
                        .iter()
                        .any(|child| child.missing().is_some())
            }
            Self::NonSequence(_) | Self::Missing(_) => true,
        }
    }
}

/// One source-ordered `buckets` member.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedMetricBucketsMember {
    syntax: AstNode<MetricBucketsMemberKind>,
    source_ordinal: u16,
    state: AttachedMetricMemberState,
    assignment: AttachedRequiredPunctuation,
    value: AttachedMetricBucketsValue,
}

impl AttachedMetricBucketsMember {
    pub const fn syntax(&self) -> &AstNode<MetricBucketsMemberKind> {
        &self.syntax
    }

    pub const fn source_ordinal(&self) -> u16 {
        self.source_ordinal
    }

    pub const fn state(&self) -> AttachedMetricMemberState {
        self.state
    }

    pub const fn assignment(&self) -> &AttachedRequiredPunctuation {
        &self.assignment
    }

    pub const fn value(&self) -> &AttachedMetricBucketsValue {
        &self.value
    }

    pub fn has_recovery(&self) -> bool {
        self.state.has_recovery() || self.assignment.is_missing() || self.value.has_recovery()
    }
}

/// Closed Metric body-entry inventory in exact source order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedMetricEntry {
    Unit(AttachedMetricUnitMember),
    Labels(AttachedMetricLabelsMember),
    Buckets(AttachedMetricBucketsMember),
    Recovery {
        source_ordinal: u16,
        syntax: AstNode<ErrorDeclarationMemberKind>,
    },
}

impl AttachedMetricEntry {
    pub const fn source_ordinal(&self) -> u16 {
        match self {
            Self::Unit(member) => member.source_ordinal(),
            Self::Labels(member) => member.source_ordinal(),
            Self::Buckets(member) => member.source_ordinal(),
            Self::Recovery { source_ordinal, .. } => *source_ordinal,
        }
    }

    pub fn has_recovery(&self) -> bool {
        match self {
            Self::Unit(member) => member.has_recovery(),
            Self::Labels(member) => member.has_recovery(),
            Self::Buckets(member) => member.has_recovery(),
            Self::Recovery { .. } => true,
        }
    }
}

/// Missing or authored Metric body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedMetricBody {
    Missing(AstNode<MissingBodyKind>),
    Braced {
        syntax: AstNode<MetricBodyKind>,
        open: AstNode<OpenBraceKind>,
        close: AstNode<CloseBraceKind>,
        entries: Box<[AttachedMetricEntry]>,
    },
}

impl AttachedMetricBody {
    pub fn entries(&self) -> &[AttachedMetricEntry] {
        match self {
            Self::Missing(_) => &[],
            Self::Braced { entries, .. } => entries,
        }
    }

    pub const fn is_missing(&self) -> bool {
        matches!(self, Self::Missing(_))
    }

    pub fn is_unclosed(&self) -> bool {
        matches!(self, Self::Braced { close, .. } if close.range().is_empty())
    }

    pub fn has_recovery(&self) -> bool {
        self.is_missing()
            || self.is_unclosed()
            || self.entries().iter().any(AttachedMetricEntry::has_recovery)
    }
}

/// One source-bound Metric declaration and all syntax-owned schema members.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedMetricDeclaration {
    syntax: AstNode<MetricDeclarationItemKind>,
    prefix: AttachedItemPrefix,
    header: AttachedRetainedHeader,
    kind: AttachedMetricKind,
    colon: AttachedRequiredPunctuation,
    value_type: AttachedTypeRefNode,
    body: AttachedMetricBody,
    declaration_recoveries: Box<[AstNode<ErrorNodeKind>]>,
}

impl AttachedMetricDeclaration {
    pub const fn syntax(&self) -> &AstNode<MetricDeclarationItemKind> {
        &self.syntax
    }

    pub const fn prefix(&self) -> &AttachedItemPrefix {
        &self.prefix
    }

    pub const fn header(&self) -> &AttachedRetainedHeader {
        &self.header
    }

    pub const fn kind(&self) -> &AttachedMetricKind {
        &self.kind
    }

    pub const fn colon(&self) -> &AttachedRequiredPunctuation {
        &self.colon
    }

    pub const fn value_type(&self) -> &AttachedTypeRefNode {
        &self.value_type
    }

    pub const fn body(&self) -> &AttachedMetricBody {
        &self.body
    }

    pub const fn declaration_recoveries(&self) -> &[AstNode<ErrorNodeKind>] {
        &self.declaration_recoveries
    }

    pub fn has_recovery(&self) -> bool {
        self.kind.has_recovery()
            || self.colon.is_missing()
            || self.value_type.family() == AttachedTypeFamily::Recovery
            || self.body.has_recovery()
            || !self.declaration_recoveries.is_empty()
    }
}

impl AstNode<MetricDeclarationItemKind> {
    /// Binds the one-pass Metric grammar without source-text rediscovery.
    pub fn semantics(&self) -> Result<AttachedMetricDeclaration, SyntaxAccessError> {
        let item = TypedItemNode::Metric(self.clone());
        let header_syntax =
            self.required_exact_child::<DeclarationHeaderKind>(SyntaxRole::Element(0))?;
        Ok(AttachedMetricDeclaration {
            syntax: self.clone(),
            prefix: item.attached_prefix()?,
            header: header_syntax.retained_semantics()?,
            kind: attach_kind(&header_syntax)?,
            colon: punctuation(
                &header_syntax.required_exact_child::<ColonKind>(SyntaxRole::Colon)?,
            ),
            value_type: required_type(&header_syntax.syntax(), SyntaxRole::Type)?,
            body: attach_body(self)?,
            declaration_recoveries: self
                .ordered_exact_children::<ErrorNodeKind>(SyntaxRoleClass::Recovery)?
                .into_boxed_slice(),
        })
    }
}

fn attach_kind(
    header: &AstNode<DeclarationHeaderKind>,
) -> Result<AttachedMetricKind, SyntaxAccessError> {
    let kinds = header
        .syntax()
        .children()
        .into_iter()
        .filter(|child| child.role().class() == SyntaxRoleClass::Kind)
        .map(|child| child.cast::<MetricKindKind>())
        .collect::<Result<Vec<_>, _>>()?;
    let [syntax] = kinds.as_slice() else {
        return Err(SyntaxAccessError::InvalidItemProjection { id: header.id() });
    };
    match syntax.role() {
        SyntaxRole::MetricKindValue(MetricKindSyntaxValue::Counter) => {
            Ok(AttachedMetricKind::Counter(syntax.clone()))
        }
        SyntaxRole::MetricKindValue(MetricKindSyntaxValue::Gauge) => {
            Ok(AttachedMetricKind::Gauge(syntax.clone()))
        }
        SyntaxRole::MetricKindValue(MetricKindSyntaxValue::Histogram) => {
            Ok(AttachedMetricKind::Histogram(syntax.clone()))
        }
        SyntaxRole::Kind if syntax.range().is_empty() => {
            Ok(AttachedMetricKind::Missing(syntax.clone()))
        }
        SyntaxRole::Kind => Ok(AttachedMetricKind::Unknown(syntax.clone())),
        _ => Err(SyntaxAccessError::InvalidItemProjection { id: header.id() }),
    }
}

fn attach_body(
    declaration: &AstNode<MetricDeclarationItemKind>,
) -> Result<AttachedMetricBody, SyntaxAccessError> {
    let body = declaration
        .syntax()
        .optional_unique_child(SyntaxRole::Body)?
        .ok_or(SyntaxAccessError::InvalidItemProjection {
            id: declaration.id(),
        })?;
    if body.kind() == SyntaxKind::MissingBody {
        return Ok(AttachedMetricBody::Missing(body.cast()?));
    }
    let body = body.cast::<MetricBodyKind>()?;
    let mut seen_members = [false; 3];
    let mut highest_rank = None;
    let mut seen_labels = BTreeSet::<SyntaxName>::new();
    let entries = body
        .syntax()
        .ordered_children(SyntaxRoleClass::Member)?
        .into_iter()
        .enumerate()
        .map(|(index, syntax)| {
            let source_ordinal =
                u16::try_from(index).map_err(|_| SyntaxAccessError::InvalidItemProjection {
                    id: declaration.id(),
                })?;
            if syntax.role() != SyntaxRole::Member(source_ordinal) {
                return Err(SyntaxAccessError::InvalidItemProjection {
                    id: declaration.id(),
                });
            }
            let Some(rank) = member_rank(syntax.kind()) else {
                return if syntax.kind() == SyntaxKind::ErrorDeclarationMember {
                    Ok(AttachedMetricEntry::Recovery {
                        source_ordinal,
                        syntax: syntax.cast()?,
                    })
                } else {
                    Err(SyntaxAccessError::InvalidItemProjection {
                        id: declaration.id(),
                    })
                };
            };
            let state = AttachedMetricMemberState {
                duplicate: seen_members[rank],
                out_of_order: highest_rank.is_some_and(|highest| rank < highest),
            };
            seen_members[rank] = true;
            highest_rank = Some(highest_rank.map_or(rank, |highest: usize| highest.max(rank)));
            attach_entry(
                &syntax,
                source_ordinal,
                state,
                &mut seen_labels,
                declaration.id(),
            )
        })
        .collect::<Result<Vec<_>, _>>()?
        .into_boxed_slice();
    Ok(AttachedMetricBody::Braced {
        open: body.required_exact_child::<OpenBraceKind>(SyntaxRole::OpenDelimiter)?,
        close: body.required_exact_child::<CloseBraceKind>(SyntaxRole::CloseDelimiter)?,
        syntax: body,
        entries,
    })
}

const fn member_rank(kind: SyntaxKind) -> Option<usize> {
    match kind {
        SyntaxKind::MetricUnitMember => Some(0),
        SyntaxKind::MetricLabelsBlock => Some(1),
        SyntaxKind::MetricBucketsMember => Some(2),
        _ => None,
    }
}

fn attach_entry(
    syntax: &SyntaxNodeHandle,
    source_ordinal: u16,
    state: AttachedMetricMemberState,
    seen_labels: &mut BTreeSet<SyntaxName>,
    declaration: super::SyntaxNodeId,
) -> Result<AttachedMetricEntry, SyntaxAccessError> {
    match syntax.kind() {
        SyntaxKind::MetricUnitMember => {
            let syntax = syntax.cast::<MetricUnitMemberKind>()?;
            Ok(AttachedMetricEntry::Unit(AttachedMetricUnitMember {
                assignment: punctuation(
                    &syntax.required_exact_child::<EqualsKind>(SyntaxRole::Equals)?,
                ),
                value: attach_unit_value(&syntax)?,
                syntax,
                source_ordinal,
                state,
            }))
        }
        SyntaxKind::MetricLabelsBlock => {
            let syntax = syntax.cast::<MetricLabelsBlockKind>()?;
            Ok(AttachedMetricEntry::Labels(AttachedMetricLabelsMember {
                body: attach_labels_body(&syntax, seen_labels, declaration)?,
                syntax,
                source_ordinal,
                state,
            }))
        }
        SyntaxKind::MetricBucketsMember => {
            let syntax = syntax.cast::<MetricBucketsMemberKind>()?;
            Ok(AttachedMetricEntry::Buckets(AttachedMetricBucketsMember {
                assignment: punctuation(
                    &syntax.required_exact_child::<EqualsKind>(SyntaxRole::Equals)?,
                ),
                value: attach_buckets_value(&syntax)?,
                syntax,
                source_ordinal,
                state,
            }))
        }
        _ => Err(SyntaxAccessError::InvalidItemProjection { id: declaration }),
    }
}

fn attach_unit_value(
    owner: &AstNode<MetricUnitMemberKind>,
) -> Result<AttachedMetricUnitValue, SyntaxAccessError> {
    let value = owner
        .syntax()
        .optional_unique_child(SyntaxRole::Initializer)?
        .ok_or(SyntaxAccessError::InvalidItemProjection { id: owner.id() })?;
    if value.kind() == SyntaxKind::MissingMemberValue {
        return Ok(AttachedMetricUnitValue::Missing(value.cast()?));
    }
    let expression = super::family::FamilyNode::<ExpressionFamily>::new(value)?.semantic()?;
    let (decoded, recovered_string) = match expression.projection() {
        ExpressionProjection::Literal(literal) => match literal.value() {
            SyntaxLiteralValue::String {
                kind: SyntaxStringKind::Quoted,
                value,
            } => (Some(value.clone()), false),
            SyntaxLiteralValue::Invalid(issue) if issue.family() == SyntaxLiteralFamily::String => {
                (None, true)
            }
            _ => (None, false),
        },
        _ => (None, false),
    };
    if let Some(value) = decoded {
        Ok(AttachedMetricUnitValue::Decoded { expression, value })
    } else if recovered_string {
        Ok(AttachedMetricUnitValue::RecoveredString(expression))
    } else {
        Ok(AttachedMetricUnitValue::NonString(expression))
    }
}

fn attach_labels_body(
    owner: &AstNode<MetricLabelsBlockKind>,
    seen_labels: &mut BTreeSet<SyntaxName>,
    declaration: super::SyntaxNodeId,
) -> Result<AttachedMetricLabelsBody, SyntaxAccessError> {
    if let Some(missing) =
        owner.optional_exact_child::<MissingMemberValueKind>(SyntaxRole::Initializer)?
    {
        return Ok(AttachedMetricLabelsBody::Missing(missing));
    }
    let labels = owner
        .ordered_exact_children::<MetricLabelKind>(SyntaxRoleClass::Label)?
        .into_iter()
        .enumerate()
        .map(|(index, syntax)| attach_label(syntax, index, seen_labels, declaration))
        .collect::<Result<Vec<_>, _>>()?
        .into_boxed_slice();
    Ok(AttachedMetricLabelsBody::Braced {
        open: owner.required_exact_child::<OpenBraceKind>(SyntaxRole::OpenDelimiter)?,
        close: owner.required_exact_child::<CloseBraceKind>(SyntaxRole::CloseDelimiter)?,
        labels,
    })
}

fn attach_label(
    syntax: AstNode<MetricLabelKind>,
    index: usize,
    seen_labels: &mut BTreeSet<SyntaxName>,
    declaration: super::SyntaxNodeId,
) -> Result<AttachedMetricLabel, SyntaxAccessError> {
    let source_ordinal = u16::try_from(index)
        .map_err(|_| SyntaxAccessError::InvalidItemProjection { id: declaration })?;
    if syntax.role() != SyntaxRole::Label(source_ordinal) {
        return Err(SyntaxAccessError::InvalidItemProjection { id: declaration });
    }
    let name = required_name(&syntax.syntax(), false)?;
    let duplicate = name
        .value()
        .is_some_and(|name| !seen_labels.insert(name.clone()));
    Ok(AttachedMetricLabel {
        colon: punctuation(&syntax.required_exact_child::<ColonKind>(SyntaxRole::Colon)?),
        ty: required_type(&syntax.syntax(), SyntaxRole::Type)?,
        syntax,
        source_ordinal,
        name,
        duplicate,
    })
}

fn attach_buckets_value(
    owner: &AstNode<MetricBucketsMemberKind>,
) -> Result<AttachedMetricBucketsValue, SyntaxAccessError> {
    let value = owner
        .syntax()
        .optional_unique_child(SyntaxRole::Initializer)?
        .ok_or(SyntaxAccessError::InvalidItemProjection { id: owner.id() })?;
    if value.kind() == SyntaxKind::MissingMemberValue {
        return Ok(AttachedMetricBucketsValue::Missing(value.cast()?));
    }
    let expression = super::family::FamilyNode::<ExpressionFamily>::new(value)?.semantic()?;
    if matches!(
        expression.projection(),
        ExpressionProjection::BracketSequence(_)
    ) {
        Ok(AttachedMetricBucketsValue::Sequence(expression))
    } else {
        Ok(AttachedMetricBucketsValue::NonSequence(expression))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::num::NonZeroU64;
    use std::sync::Arc;

    use arcweft_source::identity::SourceSnapshotId;
    use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};

    use super::{
        AstNode, AttachedMetricBody, AttachedMetricBucketsValue, AttachedMetricEntry,
        AttachedMetricKind, AttachedMetricLabelsBody, AttachedMetricUnitValue,
        MetricDeclarationItemKind,
    };
    use crate::attachment::{
        GrammarIdentityMap, SyntaxDatabaseId, SyntaxLineageId, SyntaxNodeId, SyntaxSnapshotData,
        SyntaxSnapshotId, attach_typed_tree,
    };
    use crate::grammar::kinds::{SyntaxKind, SyntaxRole};
    use crate::parser::{ParseOptions, parse_shadow_document};

    fn attach(text: &str) -> Arc<SyntaxSnapshotData> {
        let document = Arc::new(
            SourceDocument::try_new(
                SourceDocumentId::try_new("arcw:/metric-attachment-test").unwrap(),
                SourceName::path("metric-attachment-test.arcw"),
                text,
            )
            .unwrap(),
        );
        let build = parse_shadow_document(&document, ParseOptions::default()).unwrap();
        let database = SyntaxDatabaseId::from_raw_for_test(NonZeroU64::new(149).unwrap());
        let lineage = SyntaxLineageId::from_raw_for_test(database, NonZeroU64::new(1).unwrap());
        let snapshot = SyntaxSnapshotId::new(
            lineage,
            SourceSnapshotId::initial(document.display_name().clone()),
        );
        let identities = build
            .index()
            .entries()
            .iter()
            .enumerate()
            .map(|(index, entry)| {
                (
                    entry.path().clone(),
                    SyntaxNodeId::new(
                        lineage,
                        NonZeroU64::new(u64::try_from(index).unwrap() + 1).unwrap(),
                    ),
                )
            })
            .collect::<HashMap<_, _>>();
        attach_typed_tree(
            &build,
            &GrammarIdentityMap::new(identities),
            snapshot,
            document,
        )
        .unwrap()
    }

    fn metrics(snapshot: &Arc<SyntaxSnapshotData>) -> Vec<AstNode<MetricDeclarationItemKind>> {
        snapshot
            .nodes()
            .filter(|node| node.kind() == SyntaxKind::MetricDeclarationItem)
            .map(|node| node.cast().unwrap())
            .collect()
    }

    #[test]
    fn metric_attachment_owns_closed_kind_type_and_source_ordered_schema() {
        let snapshot = attach(concat!(
            "/// Frame duration\n",
            "#[test.fixture]\n",
            "pub metric gauge @metric.frame_time frame_time: f32 {\n",
            "    unit = \"ms\"\n",
            "    labels {\n",
            "        scene: String\n",
            "        quality: RenderQuality\n",
            "    }\n",
            "}\n",
            "metric histogram latency: f64 {\n",
            "    buckets = [1.0, 2.0, 4.0]\n",
            "}\n",
        ));
        let declarations = metrics(&snapshot)
            .iter()
            .map(AstNode::<MetricDeclarationItemKind>::semantics)
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(declarations.len(), 2);

        let gauge = &declarations[0];
        assert!(matches!(gauge.kind(), AttachedMetricKind::Gauge(_)));
        assert_eq!(
            gauge.kind().syntax().role(),
            SyntaxRole::MetricKindValue(crate::grammar::MetricKindSyntaxValue::Gauge)
        );
        assert_eq!(
            gauge.prefix().documentation().unwrap().markdown(),
            "Frame duration"
        );
        assert_eq!(gauge.prefix().attributes().len(), 1);
        assert!(gauge.prefix().visibility().is_some());
        assert!(!gauge.colon().is_missing());
        assert_eq!(gauge.value_type().family(), super::AttachedTypeFamily::Path);
        assert!(!gauge.has_recovery());

        let entries = gauge.body().entries();
        assert_eq!(entries.len(), 2);
        let AttachedMetricEntry::Unit(unit) = &entries[0] else {
            panic!("first Metric member must be unit");
        };
        assert_eq!(unit.source_ordinal(), 0);
        assert!(!unit.state().has_recovery());
        assert_eq!(unit.value().decoded_value(), Some("ms"));
        assert_eq!(
            unit.value().expression().unwrap().snapshot_id(),
            gauge.syntax().snapshot_id()
        );
        let AttachedMetricEntry::Labels(labels) = &entries[1] else {
            panic!("second Metric member must be labels");
        };
        let AttachedMetricLabelsBody::Braced { labels, .. } = labels.body() else {
            panic!("canonical labels member must be braced");
        };
        assert_eq!(labels.len(), 2);
        assert!(labels.iter().all(|label| !label.has_recovery()));

        let histogram = &declarations[1];
        assert!(matches!(histogram.kind(), AttachedMetricKind::Histogram(_)));
        let AttachedMetricEntry::Buckets(buckets) = &histogram.body().entries()[0] else {
            panic!("histogram must retain buckets");
        };
        assert_eq!(buckets.value().bucket_expressions().len(), 3);
        assert!(!buckets.value().is_empty_sequence());
        assert!(!buckets.has_recovery());
    }

    #[test]
    fn metric_attachment_retains_unknown_duplicate_order_and_value_recovery() {
        let snapshot = attach(concat!(
            "metric mystery broken: f32 {\n",
            "    labels {\n",
            "        scene: String\n",
            "        scene bool\n",
            "    }\n",
            "    unit milliseconds\n",
            "    extra = true\n",
            "    buckets = []\n",
            "    buckets = [1.0]\n",
            "}\n",
        ));
        let declaration = metrics(&snapshot)[0].semantics().unwrap();
        assert!(matches!(declaration.kind(), AttachedMetricKind::Unknown(_)));
        assert!(declaration.has_recovery());
        let entries = declaration.body().entries();
        assert_eq!(entries.len(), 5);

        let AttachedMetricEntry::Labels(labels) = &entries[0] else {
            panic!("first member must be labels");
        };
        assert!(!labels.state().has_recovery());
        assert!(labels.body().labels()[1].is_duplicate());
        assert!(labels.body().labels()[1].colon().is_missing());

        let AttachedMetricEntry::Unit(unit) = &entries[1] else {
            panic!("second member must be unit");
        };
        assert!(unit.state().is_out_of_order());
        assert!(unit.assignment().is_missing());
        assert!(matches!(
            unit.value(),
            AttachedMetricUnitValue::NonString(_)
        ));
        assert!(matches!(entries[2], AttachedMetricEntry::Recovery { .. }));

        let AttachedMetricEntry::Buckets(empty) = &entries[3] else {
            panic!("fourth member must be buckets");
        };
        assert!(matches!(
            empty.value(),
            AttachedMetricBucketsValue::Sequence(_)
        ));
        assert!(empty.value().is_empty_sequence());
        let AttachedMetricEntry::Buckets(duplicate) = &entries[4] else {
            panic!("fifth member must be duplicate buckets");
        };
        assert!(duplicate.state().is_duplicate());
    }

    #[test]
    fn metric_attachment_retains_missing_kind_type_body_and_member_values() {
        let snapshot = attach(concat!(
            "metric @metric.missing missing\n",
            "metric histogram recovered: f64 {\n",
            "    unit\n",
            "    labels\n",
            "    buckets\n",
            "}\n",
        ));
        let declarations = metrics(&snapshot)
            .iter()
            .map(AstNode::<MetricDeclarationItemKind>::semantics)
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        let missing = &declarations[0];
        assert!(matches!(missing.kind(), AttachedMetricKind::Missing(_)));
        assert!(missing.colon().is_missing());
        assert_eq!(
            missing.value_type().family(),
            super::AttachedTypeFamily::Recovery
        );
        assert!(matches!(missing.body(), AttachedMetricBody::Missing(_)));

        let entries = declarations[1].body().entries();
        let AttachedMetricEntry::Unit(unit) = &entries[0] else {
            panic!("unit recovery retained");
        };
        assert!(unit.assignment().is_missing());
        assert!(matches!(unit.value(), AttachedMetricUnitValue::Missing(_)));
        let AttachedMetricEntry::Labels(labels) = &entries[1] else {
            panic!("labels recovery retained");
        };
        assert!(matches!(
            labels.body(),
            AttachedMetricLabelsBody::Missing(_)
        ));
        let AttachedMetricEntry::Buckets(buckets) = &entries[2] else {
            panic!("buckets recovery retained");
        };
        assert!(buckets.assignment().is_missing());
        assert!(matches!(
            buckets.value(),
            AttachedMetricBucketsValue::Missing(_)
        ));
    }
}
