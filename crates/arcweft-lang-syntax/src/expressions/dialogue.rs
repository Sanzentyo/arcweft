//! Parser-selected dialogue-content and generic postfix-bracket projections.
//!
//! These records retain semantic values and exact typed recovery selected by
//! the shared document transaction. They never own source text, detached AST
//! nodes, or a second public syntax arena.

use std::collections::BTreeMap;

mod rebase;

use arcweft_source::SourceRange;

use super::{PendingExpressionComponent, PendingExpressionProjection, SyntaxExpressionSlot};
use crate::grammar::assertion_projection::PendingAssertionProjection;
use crate::grammar::event::{
    ExpectedToken, PendingPatternProjection, PendingSyntaxDiagnostic, PendingTypeProjection,
};
use crate::grammar::keyword_statement_projection::PendingKeywordStatementProjection;
use crate::grammar::kinds::{SyntaxKind, SyntaxRole};
use crate::grammar::source_projection::PendingPathProjection;
use crate::id_ref::{
    AuthoredIdRoot, AuthoredIdSegment, SyntaxIdRefComponent, SyntaxIdRefIssue, SyntaxIdRefShape,
    SyntaxIdRefSyntax,
};
use crate::incremental::ParseStatus;
use crate::name::{SyntaxName, SyntaxNameIssue};
use crate::patterns::PatternNodePath;
use crate::text::RichTextArgumentIssue;
use crate::types::TypeRefNodePath;

/// Exact outer close state for a bracket application.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SyntaxBracketTerminator {
    Closed,
    RecoveredMissing(SyntaxPostfixBracketRecoveryBoundary),
}

impl SyntaxBracketTerminator {
    pub const fn has_recovery(&self) -> bool {
        matches!(self, Self::RecoveredMissing(_))
    }
}

/// Boundary selected once when the outer `]` is missing.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SyntaxPostfixBracketRecoveryBoundary {
    EndOfExpression {
        anchor: usize,
    },
    LineEnding {
        range: SourceRange,
    },
    OwnerEnd {
        anchor: usize,
    },
    Token {
        token: SyntaxPostfixBoundaryToken,
        range: SourceRange,
    },
    PlanKeyword {
        range: SourceRange,
    },
}

/// Parent token that terminates a recovered postfix bracket.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SyntaxPostfixBoundaryToken {
    Comma,
    Semicolon,
    CloseParen,
    CloseBracket,
    CloseBrace,
    FatArrow,
}

/// Selected ordinary-index payload for one generic postfix bracket.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxIndexProjection {
    target: SyntaxExpressionSlot,
    index: SyntaxExpressionSlot,
    terminator: SyntaxBracketTerminator,
}

impl SyntaxIndexProjection {
    pub(crate) const fn new(
        target: SyntaxExpressionSlot,
        index: SyntaxExpressionSlot,
        terminator: SyntaxBracketTerminator,
    ) -> Self {
        Self {
            target,
            index,
            terminator,
        }
    }

    pub const fn target(&self) -> SyntaxExpressionSlot {
        self.target
    }

    pub const fn index(&self) -> SyntaxExpressionSlot {
        self.index
    }

    pub const fn terminator(&self) -> &SyntaxBracketTerminator {
        &self.terminator
    }

    pub const fn has_recovery(&self) -> bool {
        self.target.is_missing() || self.index.is_missing() || self.terminator.has_recovery()
    }
}

/// Source spelling retained separately from semantic E33 HIR.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SyntaxAttachedContentApplicationForm {
    Bracket {
        terminator: SyntaxBracketTerminator,
    },
    /// A `#` content application.  The hash marker is syntax-owned by the
    /// enclosing expression; its target is the ordinary expression following
    /// `#`, and an optional bracket body is retained as the same typed content
    /// projection used by bracket dialogue applications.
    Hash,
    Colon,
}

/// Complete parser-selected E33 payload.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxAttachedContentApplicationProjection {
    form: SyntaxAttachedContentApplicationForm,
    content: SyntaxDialogueContentProjection,
    has_plan: bool,
}

impl SyntaxAttachedContentApplicationProjection {
    pub(crate) const fn new(
        form: SyntaxAttachedContentApplicationForm,
        content: SyntaxDialogueContentProjection,
        has_plan: bool,
    ) -> Self {
        Self {
            form,
            content,
            has_plan,
        }
    }

    pub const fn form(&self) -> &SyntaxAttachedContentApplicationForm {
        &self.form
    }

    pub const fn content(&self) -> &SyntaxDialogueContentProjection {
        &self.content
    }

    pub const fn has_plan(&self) -> bool {
        self.has_plan
    }

    pub fn has_recovery(&self) -> bool {
        matches!(
            self.form,
            SyntaxAttachedContentApplicationForm::Bracket {
                terminator: SyntaxBracketTerminator::RecoveredMissing(_)
            }
        ) || match (&self.form, &self.content) {
            // A hash without an attached body is the valid compact content
            // insertion form (`#name`).  `Inline` is the source-owned marker
            // for that omission; all other missing/present recovery states
            // still propagate normally.
            (
                SyntaxAttachedContentApplicationForm::Hash,
                SyntaxDialogueContentProjection::Missing {
                    boundary: SyntaxDialogueContentRecoveryBoundary::Inline { .. },
                },
            ) => false,
            (_, content) => content.has_recovery(),
        }
    }
}

/// Present or source-owned missing content.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SyntaxDialogueContentProjection {
    Present(SyntaxDialogueContent),
    /// A `#raw()[...]` body.  The body is an opaque literal owned by the
    /// attached application and is deliberately not passed through the
    /// dialogue-content grammar.
    RawLiteral(SyntaxRawLiteralBody),
    Missing {
        boundary: SyntaxDialogueContentRecoveryBoundary,
    },
}

impl SyntaxDialogueContentProjection {
    pub fn has_recovery(&self) -> bool {
        match self {
            Self::Present(content) => content.has_recovery(),
            Self::RawLiteral(_) => false,
            Self::Missing { .. } => true,
        }
    }
}

/// Typed opaque body for the canonical `#raw()[...]` content call.
///
/// The decoded value is retained once by the syntax transaction.  No nested
/// `DialogueContent` projection is created for it, so bracket-looking bytes
/// remain literal and cannot publish controls, calls, or further content
/// applications.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxRawLiteralBody {
    value: Box<str>,
    range: SourceRange,
}

impl SyntaxRawLiteralBody {
    pub(crate) fn new(value: impl Into<Box<str>>, range: SourceRange) -> Self {
        Self {
            value: value.into(),
            range,
        }
    }

    /// Returns the literal body without its `[`/`]` delimiters.
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Returns the exact authored body range.
    pub const fn range(&self) -> SourceRange {
        self.range
    }
}

/// Typed control actions admitted by bracket dialogue syntax.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SyntaxDialogueControl {
    Page,
    LineWait,
    HardBreak,
    TimedWait,
    Clear,
    Reset,
    Speed,
}

impl SyntaxDialogueControl {
    pub(crate) const fn from_source_name(source: &str) -> Option<Self> {
        match source.as_bytes() {
            b"p" => Some(Self::Page),
            b"l" => Some(Self::LineWait),
            b"r" => Some(Self::HardBreak),
            b"w" => Some(Self::TimedWait),
            b"clear" => Some(Self::Clear),
            b"reset" => Some(Self::Reset),
            b"speed" => Some(Self::Speed),
            _ => None,
        }
    }
}

/// Typed bracket action identity.  Only controls, marks, and host events
/// produce point actions; body-bearing modifier forms are represented as
/// syntax recovery instead.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SyntaxDialoguePointActionIdentity {
    Control(SyntaxDialogueControl),
    Mark(SyntaxDialogueMarkName),
    Host(SyntaxRichTextHostEvent),
    Invalid(SyntaxRichTextIssue),
}

/// Payload family for a zero-width point action.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SyntaxDialoguePointActionPayload {
    None,
    Call(SyntaxExpressionSlot),
    TimedCue(SyntaxExpressionSlot),
}

/// Complete typed zero-width bracket action.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxDialoguePointActionProjection {
    identity: SyntaxDialoguePointActionIdentity,
    arguments: Box<[SyntaxDialogueActionArgumentProjection]>,
    payload: SyntaxDialoguePointActionPayload,
}

impl SyntaxDialoguePointActionProjection {
    pub(crate) fn new(
        identity: SyntaxDialoguePointActionIdentity,
        arguments: impl Into<Box<[SyntaxDialogueActionArgumentProjection]>>,
        payload: SyntaxDialoguePointActionPayload,
    ) -> Self {
        Self {
            identity,
            arguments: arguments.into(),
            payload,
        }
    }

    pub const fn identity(&self) -> &SyntaxDialoguePointActionIdentity {
        &self.identity
    }

    pub const fn arguments(&self) -> &[SyntaxDialogueActionArgumentProjection] {
        &self.arguments
    }

    pub const fn payload(&self) -> SyntaxDialoguePointActionPayload {
        self.payload
    }

    pub fn has_recovery(&self) -> bool {
        matches!(self.identity, SyntaxDialoguePointActionIdentity::Invalid(_))
            || matches!(
                &self.identity,
                SyntaxDialoguePointActionIdentity::Mark(mark) if mark.has_recovery()
            )
            || self
                .arguments
                .iter()
                .any(SyntaxDialogueActionArgumentProjection::has_recovery)
            || matches!(
                self.payload,
                SyntaxDialoguePointActionPayload::Call(SyntaxExpressionSlot::Missing)
                    | SyntaxDialoguePointActionPayload::TimedCue(SyntaxExpressionSlot::Missing)
            )
    }
}

/// Exact reason no dialogue content was present.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SyntaxDialogueContentRecoveryBoundary {
    CloseBracket { range: SourceRange },
    MissingBracketClose { insertion: usize },
    Inline { insertion: usize },
    Indented { insertion: usize },
}

/// Complete ordered dialogue content.
///
/// Every source atom is represented in source order.  In particular, a
/// bracket point action owns its typed payload directly; there is no second
/// tag table or opening/closing pairing table for consumers to reconcile.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxDialogueContent {
    nodes: Box<[SyntaxDialogueNodeProjection]>,
}

impl SyntaxDialogueContent {
    pub(crate) fn new(nodes: impl Into<Box<[SyntaxDialogueNodeProjection]>>) -> Self {
        Self {
            nodes: nodes.into(),
        }
    }

    pub const fn nodes(&self) -> &[SyntaxDialogueNodeProjection] {
        &self.nodes
    }

    pub fn has_recovery(&self) -> bool {
        self.nodes
            .iter()
            .any(SyntaxDialogueNodeProjection::has_recovery)
    }
}

/// One source-ordered dialogue atom.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SyntaxDialogueNodeProjection {
    Text(Box<str>),
    Escape(char),
    /// Retained Ruby sugar is lowered as a canonical attached content call by
    /// HIR.  The surface projection keeps the decoded operands only until
    /// that lowering step; it is never an executable HIR leaf.
    Ruby {
        base: Box<str>,
        ruby: Box<str>,
    },
    Interpolation(SyntaxExpressionSlot),
    /// A `#` escape whose ordinary expression child may itself be a
    /// `AttachedContentApplication` when an attached body is present.
    ContentApplication(SyntaxExpressionSlot),
    /// A zero-width bracket action.  The action owns its typed identity,
    /// arguments, and optional call payload directly; no end-tag or pairing
    /// record exists for this node family.
    PointAction(SyntaxDialoguePointActionProjection),
    LineBreak(SyntaxLineBreakKind),
    Error(SyntaxDialogueContentIssue),
}

impl SyntaxDialogueNodeProjection {
    pub fn has_recovery(&self) -> bool {
        matches!(
            self,
            Self::Interpolation(SyntaxExpressionSlot::Missing)
                | Self::ContentApplication(SyntaxExpressionSlot::Missing)
                | Self::Error(_)
        ) || matches!(self, Self::PointAction(action) if action.has_recovery())
    }
}

/// One marker selector admitted by a dialogue surface or trigger pattern.
///
/// Marker selectors reuse the lexer-owned entity-reference projection. The
/// marker boundary only accepts a zero-depth relative reference with one
/// suffix segment; all other reference shapes remain attached as typed
/// recovery instead of being reduced to a source string.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SyntaxDialogueMarkName {
    reference: SyntaxIdRefSyntax,
    range: SourceRange,
    components: Box<[SyntaxIdRefComponent]>,
    recovery: Option<SyntaxDialogueMarkNameIssue>,
}

/// Typed recovery for a marker selector's required entity-reference shape.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SyntaxDialogueMarkNameIssue {
    /// The authored selector omitted the required `@` entity-reference root.
    MissingReference,
    /// The authored selector omitted its suffix segment.
    MissingSuffix,
    /// The entity-reference lexer retained a malformed reference value.
    InvalidReference(SyntaxIdRefIssue),
    /// The selector used an absolute or family-qualified root, or a non-zero
    /// parent depth, rather than a mark-local relative root.
    InvalidRoot,
    /// The selector used more than one suffix segment.
    MultipleSegments,
    /// A quoted value cannot be a marker entity reference.
    Quoted,
    Attributed,
    MultipleArguments,
    Malformed,
}

impl SyntaxDialogueMarkName {
    /// Wraps one lexer-owned entity-reference projection and applies the
    /// marker-specific root and suffix cardinality contract.
    pub(crate) fn from_reference(
        reference: SyntaxIdRefSyntax,
        range: SourceRange,
        components: impl Into<Box<[SyntaxIdRefComponent]>>,
    ) -> Self {
        let recovery = match reference.value() {
            Err(issue) => Some(SyntaxDialogueMarkNameIssue::InvalidReference(issue.clone())),
            Ok(reference)
                if !matches!(
                    reference.root(),
                    AuthoredIdRoot::Relative { parent_depth: 0 }
                ) =>
            {
                Some(SyntaxDialogueMarkNameIssue::InvalidRoot)
            }
            Ok(reference) if reference.segments().len() != 1 => {
                Some(SyntaxDialogueMarkNameIssue::MultipleSegments)
            }
            Ok(_) => None,
        };
        Self {
            reference,
            range,
            components: components.into(),
            recovery,
        }
    }

    pub(crate) fn recovered(issue: SyntaxDialogueMarkNameIssue, range: SourceRange) -> Self {
        Self {
            reference: SyntaxIdRefSyntax::new(
                Err(SyntaxIdRefIssue::MissingSuffix),
                SyntaxIdRefShape::new(false, false, 0, 0),
            ),
            range,
            components: Box::new([]),
            recovery: Some(issue),
        }
    }

    /// Returns the lexer-owned entity-reference projection.
    pub const fn reference(&self) -> &SyntaxIdRefSyntax {
        &self.reference
    }

    /// Returns the exact lexer-owned entity-reference components retained for
    /// this selector. Recovery without an authored entity reference has no
    /// fabricated components.
    pub fn components(&self) -> &[SyntaxIdRefComponent] {
        &self.components
    }

    /// Returns the accepted one-segment suffix, when the marker shape is
    /// valid. The returned segment is the original typed ID component rather
    /// than a reparsed `SyntaxName`.
    pub fn name(&self) -> Option<&AuthoredIdSegment> {
        if self.recovery.is_some() {
            return None;
        }
        match self.reference.value() {
            Ok(reference) => match reference.segments() {
                [segment] => Some(segment),
                _ => None,
            },
            Err(_) => None,
        }
    }

    pub const fn issue(&self) -> Option<&SyntaxDialogueMarkNameIssue> {
        self.recovery.as_ref()
    }

    pub const fn range(&self) -> SourceRange {
        self.range
    }

    pub const fn has_recovery(&self) -> bool {
        self.recovery.is_some()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SyntaxDialogueActionArgumentProjection {
    Positional {
        value: SyntaxDialogueActionValue,
    },
    Named {
        name: Result<SyntaxName, SyntaxNameIssue>,
        value: SyntaxDialogueActionValue,
    },
    Invalid {
        issue: RichTextArgumentIssue,
        authored_parts: SyntaxDialogueActionArgumentParts,
    },
}

impl SyntaxDialogueActionArgumentProjection {
    pub const fn has_recovery(&self) -> bool {
        matches!(
            self,
            Self::Named { name: Err(_), .. } | Self::Invalid { .. }
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxDialogueActionValue(Box<str>);

impl SyntaxDialogueActionValue {
    pub(crate) fn new(decoded: impl Into<Box<str>>) -> Self {
        Self(decoded.into())
    }

    pub fn decoded(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SyntaxDialogueActionArgumentParts {
    name: bool,
    equals: bool,
    value: bool,
}

impl SyntaxDialogueActionArgumentParts {
    pub(crate) const fn new(name: bool, equals: bool, value: bool) -> Self {
        Self {
            name,
            equals,
            value,
        }
    }

    pub const fn has_name(self) -> bool {
        self.name
    }

    pub const fn has_equals(self) -> bool {
        self.equals
    }

    pub const fn has_value(self) -> bool {
        self.value
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SyntaxRichTextIssue {
    InvalidPayload,
    ForeignNestedExpression,
    Argument(RichTextArgumentIssue),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SyntaxDialogueContentIssue {
    UnclassifiedToken,
    InvalidPointAction,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SyntaxLineBreakKind {
    Line,
    Paragraph,
    Page,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SyntaxRichTextHostEvent {
    Voice,
    Face,
    Pose,
    Show,
    Hide,
    Move,
    Scale,
    Rotate,
    Animation,
    StageShake,
    TimedCue,
    Call,
    Signal,
}

/// Candidate-local index. It is never a source or HIR identity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct CandidateNodeIndex(u32);

impl CandidateNodeIndex {
    pub(crate) fn try_new(index: usize) -> Option<Self> {
        u32::try_from(index).ok().map(Self)
    }

    pub(crate) const fn as_usize(self) -> usize {
        self.0 as usize
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct CandidateEdgeRange {
    start: u32,
    len: u32,
}

impl CandidateEdgeRange {
    fn try_new(start: usize, len: usize) -> Option<Self> {
        Some(Self {
            start: u32::try_from(start).ok()?,
            len: u32::try_from(len).ok()?,
        })
    }

    fn as_range(self) -> core::ops::Range<usize> {
        let start = self.start as usize;
        start..start + self.len as usize
    }
}

/// One tokenless candidate semantic node in local preorder.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PendingCandidateNode {
    kind: SyntaxKind,
    role: SyntaxRole,
    parent: Option<CandidateNodeIndex>,
    children: CandidateEdgeRange,
    source: SourceRange,
    semantic: PendingCandidateSemantic,
}

impl PendingCandidateNode {
    pub(crate) const fn new(
        kind: SyntaxKind,
        role: SyntaxRole,
        parent: Option<CandidateNodeIndex>,
        source: SourceRange,
        semantic: PendingCandidateSemantic,
    ) -> Self {
        Self {
            kind,
            role,
            parent,
            children: CandidateEdgeRange { start: 0, len: 0 },
            source,
            semantic,
        }
    }

    pub(crate) const fn kind(&self) -> SyntaxKind {
        self.kind
    }

    pub(crate) const fn role(&self) -> SyntaxRole {
        self.role
    }

    pub(crate) const fn parent(&self) -> Option<CandidateNodeIndex> {
        self.parent
    }

    pub(crate) const fn source(&self) -> SourceRange {
        self.source
    }

    pub(crate) const fn semantic(&self) -> &PendingCandidateSemantic {
        &self.semantic
    }
}

/// Parser-selected semantic payload retained after candidate events are consumed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PendingCandidateSemantic {
    Expression(PendingExpressionProjection),
    Assertion(PendingAssertionProjection),
    KeywordStatement(PendingKeywordStatementProjection),
    Type(PendingTypeProjection),
    Pattern(PendingPatternProjection),
    Path(PendingPathProjection),
    KindOnly,
}

/// Tokenless local adjacency graph for one discarded-but-retained candidate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PendingCandidateGraph {
    roots: Box<[CandidateNodeIndex]>,
    nodes: Box<[PendingCandidateNode]>,
    child_edges: Box<[CandidateNodeIndex]>,
    type_index: BTreeMap<(u64, TypeRefNodePath), CandidateNodeIndex>,
    pattern_index: BTreeMap<(u64, PatternNodePath), CandidateNodeIndex>,
    missing_tokens: Box<[(ExpectedToken, usize)]>,
    diagnostics: Box<[PendingSyntaxDiagnostic]>,
}

impl PendingCandidateGraph {
    pub(crate) fn try_new(
        mut nodes: Vec<PendingCandidateNode>,
        missing_tokens: Vec<(ExpectedToken, usize)>,
        diagnostics: Vec<PendingSyntaxDiagnostic>,
    ) -> Result<Self, PendingCandidateGraphError> {
        let mut roots = Vec::new();
        let mut children = vec![Vec::new(); nodes.len()];
        let mut type_index = BTreeMap::new();
        let mut pattern_index = BTreeMap::new();

        for (position, node) in nodes.iter().enumerate() {
            let index = CandidateNodeIndex::try_new(position)
                .ok_or(PendingCandidateGraphError::NodeCountExceeded)?;
            if let Some(parent) = node.parent {
                if parent.as_usize() >= position {
                    return Err(PendingCandidateGraphError::InvalidParent {
                        node: index,
                        parent,
                    });
                }
                children[parent.as_usize()].push(index);
            } else {
                roots.push(index);
            }
            match &node.semantic {
                PendingCandidateSemantic::Type(projection) => {
                    if projection.authored().value_at(projection.path()).is_none() {
                        return Err(PendingCandidateGraphError::InvalidTypeProjection);
                    }
                    if type_index
                        .insert((projection.tree(), projection.path().clone()), index)
                        .is_some()
                    {
                        return Err(PendingCandidateGraphError::DuplicateTypeProjection);
                    }
                }
                PendingCandidateSemantic::Pattern(projection) => {
                    if projection.authored().value_at(projection.path()).is_none() {
                        return Err(PendingCandidateGraphError::InvalidPatternProjection);
                    }
                    if pattern_index
                        .insert((projection.tree(), projection.path().clone()), index)
                        .is_some()
                    {
                        return Err(PendingCandidateGraphError::DuplicatePatternProjection);
                    }
                }
                PendingCandidateSemantic::KeywordStatement(projection) => {
                    if !projection.accepts_kind(node.kind()) {
                        return Err(PendingCandidateGraphError::InvalidKeywordStatementProjection);
                    }
                }
                PendingCandidateSemantic::Assertion(_)
                    if node.kind() != SyntaxKind::AssertionStatement =>
                {
                    return Err(PendingCandidateGraphError::InvalidAssertionProjection);
                }
                PendingCandidateSemantic::Expression(_)
                | PendingCandidateSemantic::Assertion(_)
                | PendingCandidateSemantic::Path(_)
                | PendingCandidateSemantic::KindOnly => {}
            }
        }

        let mut child_edges = Vec::new();
        for (node, child_nodes) in nodes.iter_mut().zip(children) {
            node.children = CandidateEdgeRange::try_new(child_edges.len(), child_nodes.len())
                .ok_or(PendingCandidateGraphError::NodeCountExceeded)?;
            child_edges.extend(child_nodes);
        }

        Ok(Self {
            roots: roots.into_boxed_slice(),
            nodes: nodes.into_boxed_slice(),
            child_edges: child_edges.into_boxed_slice(),
            type_index,
            pattern_index,
            missing_tokens: missing_tokens.into_boxed_slice(),
            diagnostics: diagnostics.into_boxed_slice(),
        })
    }

    pub(crate) fn recovery_status(&self) -> ParseStatus {
        self.nodes.iter().fold(
            ParseStatus::from_recovery(
                !self.missing_tokens.is_empty() || !self.diagnostics.is_empty(),
            ),
            |status, node| {
                let node_status = ParseStatus::from_recovery(
                    node.kind().is_missing_node() || node.kind().is_error_node(),
                );
                let projection_status = match node.semantic() {
                    PendingCandidateSemantic::Expression(projection) => {
                        projection.recovery_status()
                    }
                    PendingCandidateSemantic::Assertion(projection) => {
                        ParseStatus::from_recovery(projection.has_recovery())
                    }
                    PendingCandidateSemantic::KeywordStatement(projection) => {
                        ParseStatus::from_recovery(projection.has_recovery())
                    }
                    PendingCandidateSemantic::Type(projection) => {
                        ParseStatus::from_recovery(matches!(
                            projection.authored().value_at(projection.path()),
                            Some(crate::types::TypeRef::Recovery(_))
                        ))
                    }
                    PendingCandidateSemantic::Pattern(projection) => ParseStatus::from_recovery(
                        projection
                            .authored()
                            .value_at(projection.path())
                            .is_some_and(|node| !node.state().is_valid()),
                    ),
                    PendingCandidateSemantic::Path(_) | PendingCandidateSemantic::KindOnly => {
                        ParseStatus::Clean
                    }
                };
                status.required(node_status).required(projection_status)
            },
        )
    }

    pub(crate) fn diagnostics(&self) -> &[PendingSyntaxDiagnostic] {
        &self.diagnostics
    }

    pub(crate) fn missing_tokens(&self) -> &[(ExpectedToken, usize)] {
        &self.missing_tokens
    }

    pub(crate) const fn roots(&self) -> &[CandidateNodeIndex] {
        &self.roots
    }

    pub(crate) const fn nodes(&self) -> &[PendingCandidateNode] {
        &self.nodes
    }

    pub(crate) fn node(&self, index: CandidateNodeIndex) -> Option<&PendingCandidateNode> {
        self.nodes.get(index.as_usize())
    }

    pub(crate) fn children(&self, index: CandidateNodeIndex) -> Option<&[CandidateNodeIndex]> {
        let node = self.node(index)?;
        self.child_edges.get(node.children.as_range())
    }

    pub(crate) fn type_node(
        &self,
        tree: u64,
        path: &TypeRefNodePath,
    ) -> Option<CandidateNodeIndex> {
        self.type_index.get(&(tree, path.clone())).copied()
    }

    pub(crate) fn pattern_node(
        &self,
        tree: u64,
        path: &PatternNodePath,
    ) -> Option<CandidateNodeIndex> {
        self.pattern_index.get(&(tree, path.clone())).copied()
    }

    fn primary_expression(&self) -> Option<CandidateNodeIndex> {
        self.nodes.iter().enumerate().find_map(|(position, node)| {
            if !matches!(node.semantic(), PendingCandidateSemantic::Expression(_)) {
                return None;
            }
            let mut parent = node.parent();
            while let Some(index) = parent {
                let ancestor = self.node(index)?;
                if matches!(ancestor.semantic(), PendingCandidateSemantic::Expression(_)) {
                    return None;
                }
                parent = ancestor.parent();
            }
            CandidateNodeIndex::try_new(position)
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PendingCandidateGraphError {
    NodeCountExceeded,
    InvalidParent {
        node: CandidateNodeIndex,
        parent: CandidateNodeIndex,
    },
    DuplicateTypeProjection,
    DuplicatePatternProjection,
    InvalidTypeProjection,
    InvalidPatternProjection,
    InvalidKeywordStatementProjection,
    InvalidAssertionProjection,
}

/// The ordinary-index interpretation retained only when both candidates win.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxPostfixIndexCandidate {
    index: CandidateNodeIndex,
    graph: PendingCandidateGraph,
}

impl SyntaxPostfixIndexCandidate {
    pub(crate) fn new(graph: PendingCandidateGraph) -> Self {
        let index = graph
            .primary_expression()
            .expect("viable ordinary-index candidates retain one semantic expression root");
        Self { index, graph }
    }

    pub fn recovery_status(&self) -> ParseStatus {
        self.graph.recovery_status()
    }

    pub(crate) const fn index(&self) -> CandidateNodeIndex {
        self.index
    }

    pub(crate) const fn graph(&self) -> &PendingCandidateGraph {
        &self.graph
    }
}

/// The dialogue-content interpretation retained only when both candidates win.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxPostfixDialogueCandidate {
    content: SyntaxDialogueContentProjection,
    components: Box<[PendingExpressionComponent]>,
    graph: PendingCandidateGraph,
}

impl SyntaxPostfixDialogueCandidate {
    pub(crate) fn new(
        content: SyntaxDialogueContentProjection,
        components: impl Into<Box<[PendingExpressionComponent]>>,
        graph: PendingCandidateGraph,
    ) -> Self {
        Self {
            content,
            components: components.into(),
            graph,
        }
    }

    pub fn recovery_status(&self) -> ParseStatus {
        self.graph
            .recovery_status()
            .required(ParseStatus::from_recovery(self.content.has_recovery()))
    }

    pub const fn content(&self) -> &SyntaxDialogueContentProjection {
        &self.content
    }

    pub(crate) const fn components(&self) -> &[PendingExpressionComponent] {
        &self.components
    }

    pub(crate) const fn graph(&self) -> &PendingCandidateGraph {
        &self.graph
    }
}

/// Exact E34 ambiguity or no-match result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SyntaxPostfixBracketProjection {
    Ambiguous {
        index: Box<SyntaxPostfixIndexCandidate>,
        dialogue: Box<SyntaxPostfixDialogueCandidate>,
    },
    Invalid {
        index: SyntaxPostfixCandidateFailure,
        dialogue: SyntaxPostfixCandidateFailure,
    },
}

impl SyntaxPostfixBracketProjection {
    pub fn recovery_status(&self) -> ParseStatus {
        match self {
            Self::Ambiguous { index, dialogue } => index
                .recovery_status()
                .alternative(dialogue.recovery_status()),
            Self::Invalid { .. } => ParseStatus::Recovered,
        }
    }

    pub fn has_recovery(&self) -> bool {
        self.recovery_status() != ParseStatus::Clean
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxPostfixCandidateFailure {
    kind: SyntaxPostfixCandidateFailureKind,
    site: SyntaxPostfixCandidateFailureSite,
}

impl SyntaxPostfixCandidateFailure {
    pub(crate) const fn new(
        kind: SyntaxPostfixCandidateFailureKind,
        site: SyntaxPostfixCandidateFailureSite,
    ) -> Self {
        Self { kind, site }
    }

    pub const fn kind(&self) -> SyntaxPostfixCandidateFailureKind {
        self.kind
    }

    pub const fn site(&self) -> SyntaxPostfixCandidateFailureSite {
        self.site
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SyntaxPostfixCandidateFailureKind {
    EmptyPayload,
    UnexpectedToken,
    MissingOperand,
    TrailingToken,
    InvalidDialogueAtom,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SyntaxPostfixCandidateFailureSite {
    Span(SourceRange),
    Insertion(usize),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SyntaxDialogueConfigurationArgumentPart {
    Whole,
    Name,
    Value,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SyntaxDialogueNodeSourcePart {
    Whole,
    Text,
    Escape,
    Ruby,
    Interpolation,
    Hash,
    Expression,
    PointAction,
    LineBreak,
    Error,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SyntaxDialoguePointActionSourcePart {
    Whole,
    OpenDelimiter,
    Name,
    Payload,
    CloseDelimiter,
    Marker(crate::id_ref::SyntaxIdRefPart),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SyntaxDialogueActionArgumentSourcePart {
    Whole,
    Name,
    Equals,
    Value,
}
