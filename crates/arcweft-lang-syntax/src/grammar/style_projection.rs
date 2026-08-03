//! Parser-owned semantic projection for the native `style` grammar.

use arcweft_source::SourceRange;

use crate::id_ref::{
    AuthoredIdRef, AuthoredIdRoot, AuthoredIdSegment, SyntaxIdRefIssue, SyntaxIdRefShape,
    SyntaxIdRefSyntax,
};
use crate::name::{is_identifier_continue, is_identifier_start};

/// One validated native Style name.
///
/// Native Style names admit `-` inside a component and `.` between token-name
/// components.  They are intentionally distinct from ordinary Arcweft
/// identifiers so attachment and HIR lowering never need to reinterpret text.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StyleSyntaxName {
    spelling: Box<str>,
    relative_token_id: SyntaxIdRefSyntax,
}

/// Typed recovery for a native Style name.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum StyleSyntaxNameIssue {
    Missing,
    EmptyComponent { ordinal: u16 },
    InvalidComponent { ordinal: u16 },
}

impl StyleSyntaxName {
    pub(crate) fn try_new(spelling: &str) -> Result<Self, StyleSyntaxNameIssue> {
        if spelling.is_empty() {
            return Err(StyleSyntaxNameIssue::Missing);
        }

        let mut authored_segments = Vec::new();
        let mut atom_ordinal = 0_usize;
        for dotted_component in spelling.split('.') {
            for component in dotted_component.split('-') {
                let ordinal = u16::try_from(atom_ordinal).unwrap_or(u16::MAX);
                let mut characters = component.chars();
                let Some(first) = characters.next() else {
                    return Err(StyleSyntaxNameIssue::EmptyComponent { ordinal });
                };
                if !is_identifier_start(first) || !characters.all(is_identifier_continue) {
                    return Err(StyleSyntaxNameIssue::InvalidComponent { ordinal });
                }
                atom_ordinal = atom_ordinal.saturating_add(1);
            }
            authored_segments.push(
                AuthoredIdSegment::try_new(dotted_component)
                    .expect("validated Style token ID components are non-empty"),
            );
        }

        let segment_count = u32::try_from(authored_segments.len()).unwrap_or(u32::MAX);
        Ok(Self {
            spelling: spelling.into(),
            relative_token_id: SyntaxIdRefSyntax::new(
                Ok(AuthoredIdRef::new(
                    AuthoredIdRoot::Relative { parent_depth: 0 },
                    authored_segments,
                )),
                SyntaxIdRefShape::new(false, false, 0, segment_count),
            ),
        })
    }

    pub fn as_str(&self) -> &str {
        &self.spelling
    }

    pub(crate) const fn relative_token_id(&self) -> &SyntaxIdRefSyntax {
        &self.relative_token_id
    }
}

impl StyleSyntaxNameIssue {
    pub(crate) fn invalid_token_id(self, dotted_component_count: u32) -> SyntaxIdRefSyntax {
        let ordinal = match self {
            Self::Missing => 0,
            Self::EmptyComponent { ordinal } | Self::InvalidComponent { ordinal } => {
                u32::from(ordinal)
            }
        };
        SyntaxIdRefSyntax::new(
            Err(SyntaxIdRefIssue::InvalidSegment { ordinal }),
            SyntaxIdRefShape::new(false, false, 0, dotted_component_count),
        )
    }
}

/// Whether a Style declaration ID was authored as an entity reference or a
/// module-relative bare Style name.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum StyleIdForm {
    Explicit,
    Bare,
}

/// Relation of one selector sequence to the preceding sequence.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum StyleSelectorRelation {
    Descendant,
    Child,
}

/// Canonical native Style property operation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum StylePropertyOperation {
    Replace,
    Append,
}

/// Closed presentation-environment field vocabulary.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum StyleEnvironmentField {
    ColorScheme,
    Contrast,
    ReducedMotion,
    TextScale,
}

impl StyleEnvironmentField {
    pub(crate) fn from_source_name(source: &str) -> Option<Self> {
        match source.as_bytes() {
            b"color-scheme" => Some(Self::ColorScheme),
            b"contrast" => Some(Self::Contrast),
            b"reduced-motion" => Some(Self::ReducedMotion),
            b"text-scale" => Some(Self::TextScale),
            _ => None,
        }
    }
}

/// Closed presentation-environment comparison vocabulary.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum StyleEnvironmentComparison {
    Equal,
    NotEqual,
    Less,
    LessOrEqual,
    Greater,
    GreaterOrEqual,
}

/// Closed current-grammar recovery for an environment condition list.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum StyleEnvironmentConditionIssue {
    EmptyCondition,
    EmptyClause,
    TrailingComma,
}

impl StyleEnvironmentComparison {
    pub(crate) fn from_source_token(source: &str) -> Option<Self> {
        match source.as_bytes() {
            b"==" => Some(Self::Equal),
            b"!=" => Some(Self::NotEqual),
            b"<" => Some(Self::Less),
            b"<=" => Some(Self::LessOrEqual),
            b">" => Some(Self::Greater),
            b">=" => Some(Self::GreaterOrEqual),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PendingStyleId {
    Authored {
        value: SyntaxIdRefSyntax,
        source: SourceRange,
        form: StyleIdForm,
        canonical_style_family: bool,
    },
    Invalid {
        value: SyntaxIdRefSyntax,
        source: SourceRange,
        authored_name: bool,
    },
    Missing {
        value: SyntaxIdRefSyntax,
        insertion: SourceRange,
    },
}

impl PendingStyleId {
    fn has_recovery(&self) -> bool {
        match self {
            Self::Authored {
                value,
                canonical_style_family,
                ..
            } => value.value().is_err() || !canonical_style_family,
            Self::Invalid { .. } | Self::Missing { .. } => true,
        }
    }

    fn rebased(&self, offset: usize) -> Option<Self> {
        Some(match self {
            Self::Authored {
                value,
                source,
                form,
                canonical_style_family,
            } => Self::Authored {
                value: value.clone(),
                source: rebase_range(*source, offset)?,
                form: *form,
                canonical_style_family: *canonical_style_family,
            },
            Self::Invalid {
                value,
                source,
                authored_name,
            } => Self::Invalid {
                value: value.clone(),
                source: rebase_range(*source, offset)?,
                authored_name: *authored_name,
            },
            Self::Missing { value, insertion } => Self::Missing {
                value: value.clone(),
                insertion: rebase_range(*insertion, offset)?,
            },
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PendingStyleName {
    Authored {
        value: Result<StyleSyntaxName, StyleSyntaxNameIssue>,
        dotted_component_count: u32,
        source: SourceRange,
    },
    Missing {
        insertion: SourceRange,
    },
}

impl PendingStyleName {
    fn has_recovery(&self) -> bool {
        matches!(self, Self::Missing { .. }) || matches!(self, Self::Authored { value: Err(_), .. })
    }

    fn rebased(&self, offset: usize) -> Option<Self> {
        Some(match self {
            Self::Authored {
                value,
                dotted_component_count,
                source,
            } => Self::Authored {
                value: value.clone(),
                dotted_component_count: *dotted_component_count,
                source: rebase_range(*source, offset)?,
            },
            Self::Missing { insertion } => Self::Missing {
                insertion: rebase_range(*insertion, offset)?,
            },
        })
    }

    pub(crate) fn token_id(&self) -> SyntaxIdRefSyntax {
        match self {
            Self::Authored {
                value: Ok(name), ..
            } => name.relative_token_id().clone(),
            Self::Authored {
                value: Err(issue),
                dotted_component_count,
                ..
            } => issue.invalid_token_id(*dotted_component_count),
            Self::Missing { .. } => SyntaxIdRefSyntax::new(
                Err(SyntaxIdRefIssue::MissingSuffix),
                SyntaxIdRefShape::new(false, false, 0, 0),
            ),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PendingStylePunctuation {
    Authored(SourceRange),
    Missing(SourceRange),
    Unsupported(SourceRange),
}

impl PendingStylePunctuation {
    pub(crate) const fn range(self) -> SourceRange {
        match self {
            Self::Authored(range) | Self::Missing(range) | Self::Unsupported(range) => range,
        }
    }

    fn has_recovery(self) -> bool {
        !matches!(self, Self::Authored(_))
    }

    fn rebased(self, offset: usize) -> Option<Self> {
        let range = rebase_range(self.range(), offset)?;
        Some(match self {
            Self::Authored(_) => Self::Authored(range),
            Self::Missing(_) => Self::Missing(range),
            Self::Unsupported(_) => Self::Unsupported(range),
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PendingStyleTypeAnnotation {
    Absent,
    Present { colon: SourceRange },
}

impl PendingStyleTypeAnnotation {
    fn rebased(&self, offset: usize) -> Option<Self> {
        Some(match self {
            Self::Absent => Self::Absent,
            Self::Present { colon } => Self::Present {
                colon: rebase_range(*colon, offset)?,
            },
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PendingStyleTokenProjection {
    pub(crate) source_ordinal: u32,
    pub(crate) name: PendingStyleName,
    pub(crate) id: SyntaxIdRefSyntax,
    pub(crate) type_annotation: PendingStyleTypeAnnotation,
    pub(crate) assignment: PendingStylePunctuation,
    pub(crate) allowed_at_this_depth: bool,
}

impl PendingStyleTokenProjection {
    fn has_recovery(&self) -> bool {
        self.name.has_recovery() || self.assignment.has_recovery() || !self.allowed_at_this_depth
    }

    fn rebased(&self, offset: usize) -> Option<Self> {
        Some(Self {
            source_ordinal: self.source_ordinal,
            name: self.name.rebased(offset)?,
            id: self.id.clone(),
            type_annotation: self.type_annotation.rebased(offset)?,
            assignment: self.assignment.rebased(offset)?,
            allowed_at_this_depth: self.allowed_at_this_depth,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PendingStyleSelectorRelation {
    pub(crate) value: StyleSelectorRelation,
    pub(crate) source: SourceRange,
}

impl PendingStyleSelectorRelation {
    fn rebased(&self, offset: usize) -> Option<Self> {
        Some(Self {
            value: self.value,
            source: rebase_range(self.source, offset)?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PendingStyleSelectorPart {
    pub(crate) separator: SourceRange,
    pub(crate) name: PendingStyleName,
}

impl PendingStyleSelectorPart {
    fn has_recovery(&self) -> bool {
        self.name.has_recovery()
    }

    fn rebased(&self, offset: usize) -> Option<Self> {
        Some(Self {
            separator: rebase_range(self.separator, offset)?,
            name: self.name.rebased(offset)?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PendingStylePredicate {
    pub(crate) source_ordinal: u16,
    pub(crate) colon: SourceRange,
    pub(crate) name: PendingStyleName,
}

impl PendingStylePredicate {
    fn has_recovery(&self) -> bool {
        self.name.has_recovery()
    }

    fn rebased(&self, offset: usize) -> Option<Self> {
        Some(Self {
            source_ordinal: self.source_ordinal,
            colon: rebase_range(self.colon, offset)?,
            name: self.name.rebased(offset)?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PendingStyleSelectorSequence {
    pub(crate) source_ordinal: u32,
    pub(crate) relation: Option<PendingStyleSelectorRelation>,
    pub(crate) element: Option<PendingStyleName>,
    pub(crate) part: Option<PendingStyleSelectorPart>,
    pub(crate) predicates: Box<[PendingStylePredicate]>,
    pub(crate) has_recovery: bool,
}

impl PendingStyleSelectorSequence {
    fn has_recovery(&self) -> bool {
        self.has_recovery
            || self
                .element
                .as_ref()
                .is_some_and(PendingStyleName::has_recovery)
            || self
                .part
                .as_ref()
                .is_some_and(PendingStyleSelectorPart::has_recovery)
            || self
                .predicates
                .iter()
                .any(PendingStylePredicate::has_recovery)
    }

    fn rebased(&self, offset: usize) -> Option<Self> {
        Some(Self {
            source_ordinal: self.source_ordinal,
            relation: match &self.relation {
                Some(relation) => Some(relation.rebased(offset)?),
                None => None,
            },
            element: match &self.element {
                Some(name) => Some(name.rebased(offset)?),
                None => None,
            },
            part: match &self.part {
                Some(part) => Some(part.rebased(offset)?),
                None => None,
            },
            predicates: self
                .predicates
                .iter()
                .map(|predicate| predicate.rebased(offset))
                .collect::<Option<Vec<_>>>()?
                .into_boxed_slice(),
            has_recovery: self.has_recovery,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PendingStyleSelectorProjection {
    pub(crate) sequences: Box<[PendingStyleSelectorSequence]>,
    pub(crate) recovery_count: u32,
    pub(crate) missing: bool,
}

impl PendingStyleSelectorProjection {
    fn has_recovery(&self) -> bool {
        self.missing
            || self.recovery_count != 0
            || self
                .sequences
                .iter()
                .any(PendingStyleSelectorSequence::has_recovery)
    }

    fn rebased(&self, offset: usize) -> Option<Self> {
        Some(Self {
            sequences: self
                .sequences
                .iter()
                .map(|sequence| sequence.rebased(offset))
                .collect::<Option<Vec<_>>>()?
                .into_boxed_slice(),
            recovery_count: self.recovery_count,
            missing: self.missing,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PendingStylePropertyProjection {
    pub(crate) source_ordinal: u32,
    pub(crate) name: PendingStyleName,
    pub(crate) operation: StylePropertyOperation,
    pub(crate) append_keyword: Option<SourceRange>,
    pub(crate) assignment: PendingStylePunctuation,
}

impl PendingStylePropertyProjection {
    fn has_recovery(&self) -> bool {
        self.name.has_recovery() || self.assignment.has_recovery()
    }

    fn rebased(&self, offset: usize) -> Option<Self> {
        Some(Self {
            source_ordinal: self.source_ordinal,
            name: self.name.rebased(offset)?,
            operation: self.operation,
            append_keyword: match self.append_keyword {
                Some(range) => Some(rebase_range(range, offset)?),
                None => None,
            },
            assignment: self.assignment.rebased(offset)?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PendingStyleRuleProjection {
    pub(crate) source_ordinal: u32,
    pub(crate) selector: PendingStyleSelectorProjection,
    pub(crate) declarations: Box<[PendingStylePropertyProjection]>,
    pub(crate) body_closed: bool,
}

impl PendingStyleRuleProjection {
    fn has_recovery(&self) -> bool {
        self.selector.has_recovery()
            || !self.body_closed
            || self
                .declarations
                .iter()
                .any(PendingStylePropertyProjection::has_recovery)
    }

    fn rebased(&self, offset: usize) -> Option<Self> {
        Some(Self {
            source_ordinal: self.source_ordinal,
            selector: self.selector.rebased(offset)?,
            declarations: self
                .declarations
                .iter()
                .map(|declaration| declaration.rebased(offset))
                .collect::<Option<Vec<_>>>()?
                .into_boxed_slice(),
            body_closed: self.body_closed,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PendingStyleEnvironmentField {
    Known {
        value: StyleEnvironmentField,
        name: PendingStyleName,
    },
    Unsupported(PendingStyleName),
    Missing(PendingStyleName),
}

impl PendingStyleEnvironmentField {
    fn has_recovery(&self) -> bool {
        !matches!(self, Self::Known { .. })
    }

    fn rebased(&self, offset: usize) -> Option<Self> {
        Some(match self {
            Self::Known { value, name } => Self::Known {
                value: *value,
                name: name.rebased(offset)?,
            },
            Self::Unsupported(name) => Self::Unsupported(name.rebased(offset)?),
            Self::Missing(name) => Self::Missing(name.rebased(offset)?),
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PendingStyleEnvironmentComparison {
    Known {
        value: StyleEnvironmentComparison,
        source: SourceRange,
    },
    Unsupported {
        source: SourceRange,
    },
    Missing {
        insertion: SourceRange,
    },
}

impl PendingStyleEnvironmentComparison {
    fn has_recovery(&self) -> bool {
        !matches!(self, Self::Known { .. })
    }

    fn rebased(&self, offset: usize) -> Option<Self> {
        Some(match self {
            Self::Known { value, source } => Self::Known {
                value: *value,
                source: rebase_range(*source, offset)?,
            },
            Self::Unsupported { source } => Self::Unsupported {
                source: rebase_range(*source, offset)?,
            },
            Self::Missing { insertion } => Self::Missing {
                insertion: rebase_range(*insertion, offset)?,
            },
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PendingStyleEnvironmentClause {
    pub(crate) source_ordinal: u16,
    pub(crate) field: PendingStyleEnvironmentField,
    pub(crate) comparison: PendingStyleEnvironmentComparison,
}

impl PendingStyleEnvironmentClause {
    fn has_recovery(&self) -> bool {
        self.field.has_recovery() || self.comparison.has_recovery()
    }

    fn rebased(&self, offset: usize) -> Option<Self> {
        Some(Self {
            source_ordinal: self.source_ordinal,
            field: self.field.rebased(offset)?,
            comparison: self.comparison.rebased(offset)?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PendingStyleEnvironmentCondition {
    pub(crate) open: PendingStylePunctuation,
    pub(crate) clauses: Box<[PendingStyleEnvironmentClause]>,
    pub(crate) recoveries: Box<[PendingStyleEnvironmentConditionRecovery]>,
    pub(crate) close: PendingStylePunctuation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PendingStyleEnvironmentConditionRecovery {
    pub(crate) source_ordinal: u32,
    pub(crate) issue: StyleEnvironmentConditionIssue,
    pub(crate) source: SourceRange,
}

impl PendingStyleEnvironmentConditionRecovery {
    fn rebased(self, offset: usize) -> Option<Self> {
        Some(Self {
            source_ordinal: self.source_ordinal,
            issue: self.issue,
            source: rebase_range(self.source, offset)?,
        })
    }
}

impl PendingStyleEnvironmentCondition {
    fn has_recovery(&self) -> bool {
        self.open.has_recovery()
            || self.close.has_recovery()
            || !self.recoveries.is_empty()
            || self
                .clauses
                .iter()
                .any(PendingStyleEnvironmentClause::has_recovery)
    }

    fn rebased(&self, offset: usize) -> Option<Self> {
        Some(Self {
            open: self.open.rebased(offset)?,
            clauses: self
                .clauses
                .iter()
                .map(|clause| clause.rebased(offset))
                .collect::<Option<Vec<_>>>()?
                .into_boxed_slice(),
            recoveries: self
                .recoveries
                .iter()
                .copied()
                .map(|recovery| recovery.rebased(offset))
                .collect::<Option<Vec<_>>>()?
                .into_boxed_slice(),
            close: self.close.rebased(offset)?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PendingStyleEnvironmentProjection {
    pub(crate) source_ordinal: u32,
    pub(crate) intrinsic: PendingStyleName,
    pub(crate) condition: PendingStyleEnvironmentCondition,
    pub(crate) body: Box<PendingStyleBodyProjection>,
}

impl PendingStyleEnvironmentProjection {
    fn has_recovery(&self) -> bool {
        self.intrinsic.has_recovery() || self.condition.has_recovery() || self.body.has_recovery()
    }

    fn rebased(&self, offset: usize) -> Option<Self> {
        Some(Self {
            source_ordinal: self.source_ordinal,
            intrinsic: self.intrinsic.rebased(offset)?,
            condition: self.condition.rebased(offset)?,
            body: Box::new(self.body.rebased(offset)?),
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PendingStyleMemberProjection {
    Token(PendingStyleTokenProjection),
    Rule(PendingStyleRuleProjection),
    Environment(PendingStyleEnvironmentProjection),
    Recovery { source_ordinal: u32 },
}

impl PendingStyleMemberProjection {
    pub(crate) const fn source_ordinal(&self) -> u32 {
        match self {
            Self::Token(token) => token.source_ordinal,
            Self::Rule(rule) => rule.source_ordinal,
            Self::Environment(environment) => environment.source_ordinal,
            Self::Recovery { source_ordinal } => *source_ordinal,
        }
    }

    fn has_recovery(&self) -> bool {
        match self {
            Self::Token(token) => token.has_recovery(),
            Self::Rule(rule) => rule.has_recovery(),
            Self::Environment(environment) => environment.has_recovery(),
            Self::Recovery { .. } => true,
        }
    }

    fn rebased(&self, offset: usize) -> Option<Self> {
        Some(match self {
            Self::Token(token) => Self::Token(token.rebased(offset)?),
            Self::Rule(rule) => Self::Rule(rule.rebased(offset)?),
            Self::Environment(environment) => Self::Environment(environment.rebased(offset)?),
            Self::Recovery { source_ordinal } => Self::Recovery {
                source_ordinal: *source_ordinal,
            },
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PendingStyleBodyProjection {
    Missing,
    Braced {
        members: Box<[PendingStyleMemberProjection]>,
        closed: bool,
    },
}

impl PendingStyleBodyProjection {
    fn has_recovery(&self) -> bool {
        match self {
            Self::Missing => true,
            Self::Braced { members, closed } => {
                !*closed
                    || members
                        .iter()
                        .any(PendingStyleMemberProjection::has_recovery)
            }
        }
    }

    fn rebased(&self, offset: usize) -> Option<Self> {
        Some(match self {
            Self::Missing => Self::Missing,
            Self::Braced { members, closed } => Self::Braced {
                members: members
                    .iter()
                    .map(|member| member.rebased(offset))
                    .collect::<Option<Vec<_>>>()?
                    .into_boxed_slice(),
                closed: *closed,
            },
        })
    }
}

/// Complete parser-selected semantic owner for one native Style declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PendingStyleDeclarationProjection {
    pub(crate) id: PendingStyleId,
    pub(crate) trailing_header_recovery: bool,
    pub(crate) body: PendingStyleBodyProjection,
}

impl PendingStyleDeclarationProjection {
    pub(crate) fn has_recovery(&self) -> bool {
        self.id.has_recovery() || self.trailing_header_recovery || self.body.has_recovery()
    }

    pub(crate) fn rebased(&self, offset: usize) -> Option<Self> {
        Some(Self {
            id: self.id.rebased(offset)?,
            trailing_header_recovery: self.trailing_header_recovery,
            body: self.body.rebased(offset)?,
        })
    }
}

fn rebase_range(range: SourceRange, offset: usize) -> Option<SourceRange> {
    Some(SourceRange::new(
        range.start().checked_add(offset)?,
        range.end().checked_add(offset)?,
    ))
}
