//! Parser-owned semantic Pattern values.
//!
//! These values are constructed by the active token-cursor transaction. They
//! are not a detached AST and expose no source-text parser.

mod bindings;
mod source;

use crate::ast::symbol_path::ProjectSymbolSegment;
use crate::id_ref::{SyntaxIdRefIssue, SyntaxIdRefSyntax};
use crate::literal::{SyntaxLiteralIssue, SyntaxLiteralSyntax};
use crate::name::{SyntaxName, SyntaxNameIssue};

pub use bindings::{PatternBindingSite, PatternBindingSiteKind};
pub(crate) use bindings::{collect_binding_sites, mark_or_binding_mismatches};
pub(crate) use source::PatternTypeChildSource;
pub use source::{
    AuthoredPattern, PatternComponentRole, PatternComponentSource, PatternFieldPart,
    PatternLiteralPart, PatternRestPart, PatternSourceMap, PatternSourceMapError,
    PatternTypeChildRelation, VariantPatternHeadPart, VariantPatternPayloadPart,
};

/// The closed semantic Pattern family inventory.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PatternSyntaxFamily {
    Binding,
    MutableBinding,
    Literal,
    EntityReference,
    Variant,
    Discard,
    Tuple,
    Record,
    BracketSequence,
    WholeBinding,
    Or,
    TypedBinding,
    Error,
}

/// A binding site whose authored name may be recovered without a fake local.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PatternBindingSyntax {
    Resolved(SyntaxName),
    Recovered(PatternBindingIssue),
}

/// Typed recovery retained for one binding site.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PatternBindingIssue {
    MissingName,
    InvalidName(SyntaxNameIssue),
    ReservedBindingKeyword { spelling: Box<str> },
    UnexpectedTrailingInput { token_count: u32 },
}

/// Structural path from an authored Pattern root to one semantic Pattern node.
#[derive(Clone, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PatternNodePath(Box<[PatternNodeStep]>);

/// One semantic Pattern-child edge.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PatternNodeStep {
    VariantPayload,
    Element(u32),
    RecordField(u32),
    NestedPattern,
}

/// Root behavior retained directly from an authored Pattern path.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PatternPathRoot {
    ImplicitCrate,
    Crate,
    SelfModule,
    Super(usize),
}

/// One path segment, retaining whether the active grammar admitted an
/// external project-symbol spelling rather than an identifier.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PatternPathSegment {
    Identifier(SyntaxName),
    ProjectSymbol(ProjectSymbolSegment),
}

/// A root-preserving path built from tokens consumed by the Pattern grammar.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PatternPath {
    root: PatternPathRoot,
    segments: Box<[PatternPathSegment]>,
}

/// A path component with explicit resolved/recovered/absent state.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PatternPathSyntax {
    Resolved(PatternPath),
    Recovered(PatternPathRecovery),
    Absent,
}

/// Typed recovery for a path that could not become a validated value.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PatternPathRecovery {
    root: Option<PatternPathRoot>,
    segments: Box<[Box<str>]>,
    issue: PatternPathIssue,
}

/// Failure in an authored Pattern path.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PatternPathIssue {
    MissingSegment,
    InvalidSegment {
        ordinal: u32,
        issue: SyntaxNameIssue,
    },
    InvalidRootDepth,
}

/// Variant head selected from shorthand or a qualified path.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PatternVariantHead {
    Qualified(PatternPath),
    Unqualified(PatternUnqualifiedVariantForm),
}

/// Expected-type-relative variant spelling retained for semantic resolution.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PatternUnqualifiedVariantForm {
    DotShorthand,
    BareExpectedType,
}

/// Explicit state of the optional/recoverable variant head.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PatternVariantHeadSyntax {
    Resolved(PatternVariantHead),
    Recovered(PatternPathRecovery),
    Absent,
}

/// Explicit state of an authored required name.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PatternNameSyntax {
    Resolved(SyntaxName),
    Recovered(SyntaxNameIssue),
    Absent,
}

/// Explicit state of the optional/recoverable variant payload.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PatternVariantPayloadSyntax {
    Resolved(Box<PatternSyntaxNode>),
    Recovered {
        value: Option<Box<PatternSyntaxNode>>,
        issue: PatternVariantPayloadIssue,
    },
    Absent,
}

/// Typed variant-payload recovery.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PatternVariantPayloadIssue {
    MissingPattern,
    MissingCloseDelimiter,
    InvalidPattern,
}

/// Complete variant syntax payload.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PatternVariantSyntax {
    head: PatternVariantHeadSyntax,
    name: PatternNameSyntax,
    payload: PatternVariantPayloadSyntax,
}

/// One record-pattern field. Only `Explicit` owns a Pattern child path.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PatternRecordFieldSyntax {
    Explicit {
        name: PatternNameSyntax,
        pattern: Box<PatternSyntaxNode>,
    },
    Shorthand(PatternBindingSyntax),
    Rest(Option<PatternBindingSyntax>),
    Invalid(PatternInvalidRecordFieldSyntax),
}

/// Invalid record field retained without constructing a fake child Pattern.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PatternInvalidRecordFieldSyntax {
    name: PatternNameSyntax,
    issue: PatternRecordFieldIssue,
    shape: PatternRecordFieldShape,
}

/// Source component shape of one record field.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PatternRecordFieldShape {
    parts: u8,
}

/// Typed record-field recovery.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PatternRecordFieldIssue {
    MissingName,
    InvalidName(SyntaxNameIssue),
    InvalidBinding(PatternBindingIssue),
    MissingPattern,
    InvalidRestBinding(PatternBindingIssue),
}

/// Record Pattern payload.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PatternRecordSyntax {
    path: PatternPathSyntax,
    fields: Box<[PatternRecordFieldSyntax]>,
}

/// Explicit bracket-sequence rest state. Authored absence, an unbound rest,
/// and a recovered/multiple rest never collapse into one `Option`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PatternSequenceRestSyntax {
    Absent,
    Unbound,
    Binding(PatternBindingSyntax),
    Recovered {
        binding: Option<PatternBindingSyntax>,
        issues: Box<[PatternSequenceRestIssue]>,
    },
}

/// Typed recovery attached to an authored bracket-sequence rest.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PatternSequenceRestIssue {
    InvalidBinding(PatternBindingIssue),
    MultipleRest { ordinal: u32 },
}

/// Bracket-sequence Pattern payload.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PatternSequenceSyntax {
    elements: Box<[PatternSyntaxNode]>,
    rest: PatternSequenceRestSyntax,
}

/// Why an or-pattern alternative cannot reuse the binding positions fixed by
/// the first alternative.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PatternOrBindingIssue {
    CountMismatch {
        alternative: u32,
        expected: u32,
        actual: u32,
    },
    PositionMismatch {
        alternative: u32,
        ordinal: u32,
    },
}

/// Typed local recovery state of one Pattern node.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PatternSyntaxState {
    Valid,
    Recovered(Box<[PatternRecoveryIssue]>),
}

/// Typed source recovery owned by one semantic Pattern node.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PatternRecoveryIssue {
    MissingPattern,
    UnexpectedPattern,
    Binding(PatternBindingIssue),
    Literal(SyntaxLiteralIssue),
    EntityReference(SyntaxIdRefIssue),
    VariantName(SyntaxNameIssue),
    VariantHead(PatternPathIssue),
    VariantPayload(PatternVariantPayloadIssue),
    MissingCloseDelimiter,
    InvalidRecordField {
        ordinal: u32,
        issue: PatternRecordFieldIssue,
    },
    MissingOrAlternative {
        ordinal: u32,
    },
    OrBindings(PatternOrBindingIssue),
    SequenceRest(PatternSequenceRestIssue),
    InvalidType,
}

/// Semantic value of one Pattern node.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PatternSyntaxKind {
    Binding(PatternBindingSyntax),
    MutableBinding(PatternBindingSyntax),
    Literal(SyntaxLiteralSyntax),
    EntityReference(SyntaxIdRefSyntax),
    Variant(PatternVariantSyntax),
    Discard,
    Tuple(Box<[PatternSyntaxNode]>),
    Record(PatternRecordSyntax),
    BracketSequence(PatternSequenceSyntax),
    WholeBinding {
        binding: PatternBindingSyntax,
        pattern: Box<PatternSyntaxNode>,
    },
    Or(Box<[PatternSyntaxNode]>),
    TypedBinding(PatternBindingSyntax),
    Error,
}

/// One semantic Pattern node and its typed source-recovery state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PatternSyntaxNode {
    kind: PatternSyntaxKind,
    state: PatternSyntaxState,
}

impl PatternBindingSyntax {
    pub const fn name(&self) -> Option<&SyntaxName> {
        match self {
            Self::Resolved(name) => Some(name),
            Self::Recovered(_) => None,
        }
    }

    pub const fn issue(&self) -> Option<&PatternBindingIssue> {
        match self {
            Self::Resolved(_) => None,
            Self::Recovered(issue) => Some(issue),
        }
    }
}

impl PatternNodePath {
    pub fn root() -> Self {
        Self(Box::new([]))
    }

    pub fn steps(&self) -> &[PatternNodeStep] {
        &self.0
    }

    pub(crate) fn child(&self, step: PatternNodeStep) -> Self {
        let mut steps = self.0.to_vec();
        steps.push(step);
        Self(steps.into_boxed_slice())
    }

    pub(crate) fn parent(&self) -> Option<Self> {
        (!self.0.is_empty()).then(|| Self(self.0[..self.0.len() - 1].into()))
    }
}

impl PatternPath {
    pub(crate) fn new(root: PatternPathRoot, segments: Vec<PatternPathSegment>) -> Self {
        Self {
            root,
            segments: segments.into_boxed_slice(),
        }
    }

    pub const fn root(&self) -> PatternPathRoot {
        self.root
    }

    pub fn segments(&self) -> &[PatternPathSegment] {
        &self.segments
    }
}

impl PatternPathSegment {
    pub fn spelling(&self) -> &str {
        match self {
            Self::Identifier(name) => name.as_str(),
            Self::ProjectSymbol(symbol) => symbol.as_str(),
        }
    }
}

impl PatternPathRecovery {
    pub(crate) fn new(
        root: Option<PatternPathRoot>,
        segments: Vec<Box<str>>,
        issue: PatternPathIssue,
    ) -> Self {
        Self {
            root,
            segments: segments.into_boxed_slice(),
            issue,
        }
    }

    pub const fn root(&self) -> Option<PatternPathRoot> {
        self.root
    }

    pub fn segments(&self) -> &[Box<str>] {
        &self.segments
    }

    pub const fn issue(&self) -> &PatternPathIssue {
        &self.issue
    }
}

impl PatternVariantSyntax {
    pub(crate) const fn new(
        head: PatternVariantHeadSyntax,
        name: PatternNameSyntax,
        payload: PatternVariantPayloadSyntax,
    ) -> Self {
        Self {
            head,
            name,
            payload,
        }
    }

    pub const fn head(&self) -> &PatternVariantHeadSyntax {
        &self.head
    }

    pub const fn name(&self) -> &PatternNameSyntax {
        &self.name
    }

    pub const fn payload(&self) -> &PatternVariantPayloadSyntax {
        &self.payload
    }
}

impl PatternInvalidRecordFieldSyntax {
    pub(crate) const fn new(
        name: PatternNameSyntax,
        issue: PatternRecordFieldIssue,
        shape: PatternRecordFieldShape,
    ) -> Self {
        Self { name, issue, shape }
    }

    pub const fn name(&self) -> &PatternNameSyntax {
        &self.name
    }

    pub const fn issue(&self) -> &PatternRecordFieldIssue {
        &self.issue
    }

    pub const fn shape(&self) -> PatternRecordFieldShape {
        self.shape
    }
}

impl PatternRecordFieldShape {
    const NAME: u8 = 1 << 0;
    const COLON: u8 = 1 << 1;
    const PATTERN: u8 = 1 << 2;
    const REST_MARKER: u8 = 1 << 3;
    const REST_BINDING: u8 = 1 << 4;

    pub(crate) const fn explicit() -> Self {
        Self {
            parts: Self::NAME | Self::COLON | Self::PATTERN,
        }
    }

    pub(crate) const fn shorthand() -> Self {
        Self { parts: Self::NAME }
    }

    pub(crate) const fn rest(has_binding: bool) -> Self {
        Self {
            parts: Self::REST_MARKER | if has_binding { Self::REST_BINDING } else { 0 },
        }
    }

    pub const fn name(self) -> bool {
        self.parts & Self::NAME != 0
    }

    pub const fn colon(self) -> bool {
        self.parts & Self::COLON != 0
    }

    pub const fn pattern(self) -> bool {
        self.parts & Self::PATTERN != 0
    }

    pub const fn rest_marker(self) -> bool {
        self.parts & Self::REST_MARKER != 0
    }

    pub const fn rest_binding(self) -> bool {
        self.parts & Self::REST_BINDING != 0
    }
}

impl PatternRecordSyntax {
    pub(crate) fn new(path: PatternPathSyntax, fields: Vec<PatternRecordFieldSyntax>) -> Self {
        Self {
            path,
            fields: fields.into_boxed_slice(),
        }
    }

    pub const fn path(&self) -> &PatternPathSyntax {
        &self.path
    }

    pub fn fields(&self) -> &[PatternRecordFieldSyntax] {
        &self.fields
    }
}

impl PatternSequenceRestSyntax {
    pub const fn binding(&self) -> Option<&PatternBindingSyntax> {
        match self {
            Self::Binding(binding)
            | Self::Recovered {
                binding: Some(binding),
                ..
            } => Some(binding),
            Self::Absent | Self::Unbound | Self::Recovered { binding: None, .. } => None,
        }
    }

    pub fn issues(&self) -> &[PatternSequenceRestIssue] {
        match self {
            Self::Recovered { issues, .. } => issues,
            Self::Absent | Self::Unbound | Self::Binding(_) => &[],
        }
    }

    pub(crate) fn recover(self, issue: PatternSequenceRestIssue) -> Self {
        match self {
            Self::Absent | Self::Unbound => Self::Recovered {
                binding: None,
                issues: Box::new([issue]),
            },
            Self::Binding(binding) => Self::Recovered {
                binding: Some(binding),
                issues: Box::new([issue]),
            },
            Self::Recovered { binding, issues } => {
                let mut issues = issues.into_vec();
                issues.push(issue);
                Self::Recovered {
                    binding,
                    issues: issues.into_boxed_slice(),
                }
            }
        }
    }
}

impl PatternSequenceSyntax {
    pub(crate) fn new(elements: Vec<PatternSyntaxNode>, rest: PatternSequenceRestSyntax) -> Self {
        Self {
            elements: elements.into_boxed_slice(),
            rest,
        }
    }

    pub fn elements(&self) -> &[PatternSyntaxNode] {
        &self.elements
    }

    pub const fn rest(&self) -> &PatternSequenceRestSyntax {
        &self.rest
    }
}

impl PatternSyntaxState {
    pub(crate) fn from_issues(issues: Vec<PatternRecoveryIssue>) -> Self {
        if issues.is_empty() {
            Self::Valid
        } else {
            Self::Recovered(issues.into_boxed_slice())
        }
    }

    pub const fn is_valid(&self) -> bool {
        matches!(self, Self::Valid)
    }

    pub fn issues(&self) -> &[PatternRecoveryIssue] {
        match self {
            Self::Valid => &[],
            Self::Recovered(issues) => issues,
        }
    }
}

impl PatternSyntaxNode {
    pub(crate) const fn new(kind: PatternSyntaxKind, state: PatternSyntaxState) -> Self {
        Self { kind, state }
    }

    pub(crate) fn valid(kind: PatternSyntaxKind) -> Self {
        Self::new(kind, PatternSyntaxState::Valid)
    }

    pub const fn kind(&self) -> &PatternSyntaxKind {
        &self.kind
    }

    pub const fn state(&self) -> &PatternSyntaxState {
        &self.state
    }

    pub const fn family(&self) -> PatternSyntaxFamily {
        match self.kind {
            PatternSyntaxKind::Binding(_) => PatternSyntaxFamily::Binding,
            PatternSyntaxKind::MutableBinding(_) => PatternSyntaxFamily::MutableBinding,
            PatternSyntaxKind::Literal(_) => PatternSyntaxFamily::Literal,
            PatternSyntaxKind::EntityReference(_) => PatternSyntaxFamily::EntityReference,
            PatternSyntaxKind::Variant(_) => PatternSyntaxFamily::Variant,
            PatternSyntaxKind::Discard => PatternSyntaxFamily::Discard,
            PatternSyntaxKind::Tuple(_) => PatternSyntaxFamily::Tuple,
            PatternSyntaxKind::Record(_) => PatternSyntaxFamily::Record,
            PatternSyntaxKind::BracketSequence(_) => PatternSyntaxFamily::BracketSequence,
            PatternSyntaxKind::WholeBinding { .. } => PatternSyntaxFamily::WholeBinding,
            PatternSyntaxKind::Or(_) => PatternSyntaxFamily::Or,
            PatternSyntaxKind::TypedBinding(_) => PatternSyntaxFamily::TypedBinding,
            PatternSyntaxKind::Error => PatternSyntaxFamily::Error,
        }
    }

    /// Immediate semantic Pattern-child edges in deterministic source order.
    ///
    /// This stays on the parser-owned Pattern value so attached roots and
    /// retained candidate graphs cannot drift into separate traversal rules.
    pub(crate) fn immediate_child_steps(&self) -> Vec<PatternNodeStep> {
        match self.kind() {
            PatternSyntaxKind::Variant(variant) => match variant.payload() {
                PatternVariantPayloadSyntax::Resolved(_)
                | PatternVariantPayloadSyntax::Recovered { value: Some(_), .. } => {
                    vec![PatternNodeStep::VariantPayload]
                }
                PatternVariantPayloadSyntax::Recovered { value: None, .. }
                | PatternVariantPayloadSyntax::Absent => Vec::new(),
            },
            PatternSyntaxKind::Tuple(elements) | PatternSyntaxKind::Or(elements) => {
                indexed_pattern_steps(elements.len(), PatternNodeStep::Element)
            }
            PatternSyntaxKind::Record(record) => record
                .fields()
                .iter()
                .enumerate()
                .filter(|(_, field)| matches!(field, PatternRecordFieldSyntax::Explicit { .. }))
                .map(|(index, _)| {
                    PatternNodeStep::RecordField(
                        u32::try_from(index)
                            .expect("validated Pattern limits fit structural ordinals"),
                    )
                })
                .collect(),
            PatternSyntaxKind::BracketSequence(sequence) => {
                indexed_pattern_steps(sequence.elements().len(), PatternNodeStep::Element)
            }
            PatternSyntaxKind::WholeBinding { .. } => vec![PatternNodeStep::NestedPattern],
            PatternSyntaxKind::Binding(_)
            | PatternSyntaxKind::MutableBinding(_)
            | PatternSyntaxKind::Literal(_)
            | PatternSyntaxKind::EntityReference(_)
            | PatternSyntaxKind::Discard
            | PatternSyntaxKind::TypedBinding(_)
            | PatternSyntaxKind::Error => Vec::new(),
        }
    }
}

fn indexed_pattern_steps(len: usize, step: fn(u32) -> PatternNodeStep) -> Vec<PatternNodeStep> {
    (0..len)
        .map(|index| {
            step(u32::try_from(index).expect("validated Pattern limits fit structural ordinals"))
        })
        .collect()
}
