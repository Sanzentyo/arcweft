//! Parser-owned semantic projections for declaration families.
//!
//! These records retain decisions that cannot be reconstructed from CST shape
//! alone, such as a recovered public-ID form or a missing required token.  The
//! immutable attachment validates them against the exact typed descendants
//! before any HIR consumer can observe them.

use arcweft_id::PublicId;
use arcweft_source::SourceRange;

use super::roles::{LayerKindSyntaxValue, LayerMemberSyntaxKind, LayerPolicySyntaxValue};
use crate::id_ref::SyntaxIdRefSyntax;
use crate::name::SyntaxName;

/// Parser-selected public-ID state for one declaration identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PendingDeclarationPublicId {
    Derived,
    Explicit {
        value: PublicId,
        source: SourceRange,
    },
    Recovered {
        issue: PendingDeclarationPublicIdIssue,
        source: SourceRange,
    },
}

impl PendingDeclarationPublicId {
    pub(crate) fn rebased(&self, offset: usize) -> Option<Self> {
        let rebase = |range: SourceRange| {
            Some(SourceRange::new(
                range.start().checked_add(offset)?,
                range.end().checked_add(offset)?,
            ))
        };
        Some(match self {
            Self::Derived => Self::Derived,
            Self::Explicit { value, source } => Self::Explicit {
                value: value.clone(),
                source: rebase(*source)?,
            },
            Self::Recovered { issue, source } => Self::Recovered {
                issue: issue.clone(),
                source: rebase(*source)?,
            },
        })
    }
}

/// Typed recovery for an authored declaration public ID.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PendingDeclarationPublicIdIssue {
    WrongFamily(PublicId),
    Malformed,
    Missing,
}

/// Parser-selected declaration-name state shared by declaration headers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PendingDeclarationName {
    Resolved {
        value: SyntaxName,
        source: SourceRange,
    },
    Missing {
        insertion: SourceRange,
    },
    Invalid {
        insertion: SourceRange,
        recovery: SourceRange,
    },
}

impl PendingDeclarationName {
    fn rebased(&self, offset: usize) -> Option<Self> {
        let rebase = |range: SourceRange| {
            Some(SourceRange::new(
                range.start().checked_add(offset)?,
                range.end().checked_add(offset)?,
            ))
        };
        Some(match self {
            Self::Resolved { value, source } => Self::Resolved {
                value: value.clone(),
                source: rebase(*source)?,
            },
            Self::Missing { insertion } => Self::Missing {
                insertion: rebase(*insertion)?,
            },
            Self::Invalid {
                insertion,
                recovery,
            } => Self::Invalid {
                insertion: rebase(*insertion)?,
                recovery: rebase(*recovery)?,
            },
        })
    }
}

/// Header semantics shared by declaration producers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PendingDeclarationHeaderProjection {
    public_id: PendingDeclarationPublicId,
    name: PendingDeclarationName,
}

impl PendingDeclarationHeaderProjection {
    pub(crate) const fn new(
        public_id: PendingDeclarationPublicId,
        name: PendingDeclarationName,
    ) -> Self {
        Self { public_id, name }
    }

    pub(crate) const fn public_id(&self) -> &PendingDeclarationPublicId {
        &self.public_id
    }

    pub(crate) const fn name(&self) -> &PendingDeclarationName {
        &self.name
    }

    pub(crate) fn has_recovery(&self) -> bool {
        !matches!(
            self.public_id,
            PendingDeclarationPublicId::Derived | PendingDeclarationPublicId::Explicit { .. }
        ) || !matches!(self.name, PendingDeclarationName::Resolved { .. })
    }

    pub(crate) fn rebased(&self, offset: usize) -> Option<Self> {
        Some(Self {
            public_id: self.public_id.rebased(offset)?,
            name: self.name.rebased(offset)?,
        })
    }
}

/// Optional Character surface-alias state selected by the parser.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PendingCharacterSurfaceAlias {
    Absent,
    Resolved {
        value: SyntaxName,
        source: SourceRange,
    },
    Missing {
        insertion: SourceRange,
    },
}

impl PendingCharacterSurfaceAlias {
    fn rebased(&self, offset: usize) -> Option<Self> {
        let rebase = |range: SourceRange| {
            Some(SourceRange::new(
                range.start().checked_add(offset)?,
                range.end().checked_add(offset)?,
            ))
        };
        Some(match self {
            Self::Absent => Self::Absent,
            Self::Resolved { value, source } => Self::Resolved {
                value: value.clone(),
                source: rebase(*source)?,
            },
            Self::Missing { insertion } => Self::Missing {
                insertion: rebase(*insertion)?,
            },
        })
    }
}

/// Required assignment token owned by a Character display-name member.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PendingCharacterAssignment {
    Authored(SourceRange),
    Missing(SourceRange),
}

impl PendingCharacterAssignment {
    pub(crate) const fn range(self) -> SourceRange {
        match self {
            Self::Authored(range) | Self::Missing(range) => range,
        }
    }

    pub(crate) fn rebased(self, offset: usize) -> Option<Self> {
        let range = SourceRange::new(
            self.range().start().checked_add(offset)?,
            self.range().end().checked_add(offset)?,
        );
        Some(match self {
            Self::Authored(_) => Self::Authored(range),
            Self::Missing(_) => Self::Missing(range),
        })
    }
}

/// Whether the member owns an authored expression or typed missing-value node.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PendingCharacterInitializer {
    Authored,
    Missing,
}

/// One Character body member in exact source order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PendingCharacterMemberProjection {
    DisplayName {
        source_ordinal: u16,
        name: SourceRange,
        duplicate: bool,
        assignment: PendingCharacterAssignment,
        initializer: PendingCharacterInitializer,
    },
    Recovery {
        source_ordinal: u16,
    },
}

impl PendingCharacterMemberProjection {
    pub(crate) const fn source_ordinal(&self) -> u16 {
        match self {
            Self::DisplayName { source_ordinal, .. } | Self::Recovery { source_ordinal } => {
                *source_ordinal
            }
        }
    }

    fn rebased(&self, offset: usize) -> Option<Self> {
        Some(match self {
            Self::DisplayName {
                source_ordinal,
                name,
                duplicate,
                assignment,
                initializer,
            } => Self::DisplayName {
                source_ordinal: *source_ordinal,
                name: SourceRange::new(
                    name.start().checked_add(offset)?,
                    name.end().checked_add(offset)?,
                ),
                duplicate: *duplicate,
                assignment: assignment.rebased(offset)?,
                initializer: *initializer,
            },
            Self::Recovery { source_ordinal } => Self::Recovery {
                source_ordinal: *source_ordinal,
            },
        })
    }
}

/// Parser-selected Character body shape.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PendingCharacterBodyProjection {
    Missing,
    Braced {
        closed: bool,
        members: Box<[PendingCharacterMemberProjection]>,
    },
}

impl PendingCharacterBodyProjection {
    fn rebased(&self, offset: usize) -> Option<Self> {
        Some(match self {
            Self::Missing => Self::Missing,
            Self::Braced { closed, members } => Self::Braced {
                closed: *closed,
                members: members
                    .iter()
                    .map(|member| member.rebased(offset))
                    .collect::<Option<Vec<_>>>()?
                    .into_boxed_slice(),
            },
        })
    }
}

/// Sole parser-owned semantic projection for one Character declaration item.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PendingCharacterDeclarationProjection {
    surface_alias: PendingCharacterSurfaceAlias,
    body: PendingCharacterBodyProjection,
    unexpected_header: bool,
    trailing_syntax: bool,
}

impl PendingCharacterDeclarationProjection {
    pub(crate) const fn new(
        surface_alias: PendingCharacterSurfaceAlias,
        body: PendingCharacterBodyProjection,
        unexpected_header: bool,
        trailing_syntax: bool,
    ) -> Self {
        Self {
            surface_alias,
            body,
            unexpected_header,
            trailing_syntax,
        }
    }

    pub(crate) const fn surface_alias(&self) -> &PendingCharacterSurfaceAlias {
        &self.surface_alias
    }

    pub(crate) const fn body(&self) -> &PendingCharacterBodyProjection {
        &self.body
    }

    pub(crate) const fn has_unexpected_header(&self) -> bool {
        self.unexpected_header
    }

    pub(crate) const fn has_trailing_syntax(&self) -> bool {
        self.trailing_syntax
    }

    pub(crate) fn has_recovery(&self) -> bool {
        matches!(
            self.surface_alias,
            PendingCharacterSurfaceAlias::Missing { .. }
        ) || match &self.body {
            PendingCharacterBodyProjection::Missing => true,
            PendingCharacterBodyProjection::Braced { closed, members } => {
                !closed
                    || members.iter().any(|member| match member {
                        PendingCharacterMemberProjection::Recovery { .. } => true,
                        PendingCharacterMemberProjection::DisplayName {
                            duplicate,
                            assignment,
                            initializer,
                            ..
                        } => {
                            *duplicate
                                || matches!(assignment, PendingCharacterAssignment::Missing(_))
                                || matches!(initializer, PendingCharacterInitializer::Missing)
                        }
                    })
            }
        } || self.unexpected_header
            || self.trailing_syntax
    }

    pub(crate) fn rebased(&self, offset: usize) -> Option<Self> {
        Some(Self {
            surface_alias: self.surface_alias.rebased(offset)?,
            body: self.body.rebased(offset)?,
            unexpected_header: self.unexpected_header,
            trailing_syntax: self.trailing_syntax,
        })
    }
}

/// Required `:` token owned by one Layer header.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PendingLayerColon {
    Authored(SourceRange),
    Missing(SourceRange),
}

impl PendingLayerColon {
    pub(crate) const fn range(self) -> SourceRange {
        match self {
            Self::Authored(range) | Self::Missing(range) => range,
        }
    }

    fn rebased(self, offset: usize) -> Option<Self> {
        let range = rebase_range(self.range(), offset)?;
        Some(match self {
            Self::Authored(_) => Self::Authored(range),
            Self::Missing(_) => Self::Missing(range),
        })
    }
}

/// Closed Layer kind or parser-owned recovery.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PendingLayerKind {
    Authored(LayerKindSyntaxValue),
    Missing,
    Unknown,
}

/// Required assignment token owned by one Layer member.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PendingLayerAssignment {
    Authored(SourceRange),
    Missing(SourceRange),
}

impl PendingLayerAssignment {
    pub(crate) const fn range(self) -> SourceRange {
        match self {
            Self::Authored(range) | Self::Missing(range) => range,
        }
    }

    fn rebased(self, offset: usize) -> Option<Self> {
        let range = rebase_range(self.range(), offset)?;
        Some(match self {
            Self::Authored(_) => Self::Authored(range),
            Self::Missing(_) => Self::Missing(range),
        })
    }
}

/// Parser-selected closed policy value or exact unknown-value recovery.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PendingLayerPolicy {
    Authored(LayerPolicySyntaxValue),
    Unknown,
}

/// Lexer-owned entity-reference payload and its syntax-time family result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PendingLayerReference {
    syntax: SyntaxIdRefSyntax,
    wrong_absolute_family: bool,
}

impl PendingLayerReference {
    pub(crate) const fn new(syntax: SyntaxIdRefSyntax, wrong_absolute_family: bool) -> Self {
        Self {
            syntax,
            wrong_absolute_family,
        }
    }

    pub(crate) const fn syntax(&self) -> &SyntaxIdRefSyntax {
        &self.syntax
    }

    pub(crate) const fn is_wrong_absolute_family(&self) -> bool {
        self.wrong_absolute_family
    }

    pub(crate) fn has_recovery(&self) -> bool {
        self.wrong_absolute_family || self.syntax.value().is_err()
    }
}

/// One known Layer member's parser-selected value family.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PendingLayerMemberValue {
    Reference(PendingLayerReference),
    Policy(PendingLayerPolicy),
    Expression,
    Missing,
}

impl PendingLayerMemberValue {
    fn has_recovery(&self) -> bool {
        match self {
            Self::Reference(reference) => reference.has_recovery(),
            Self::Policy(PendingLayerPolicy::Unknown) | Self::Missing => true,
            Self::Policy(PendingLayerPolicy::Authored(_)) | Self::Expression => false,
        }
    }
}

/// One Layer body entry in exact source order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PendingLayerMemberProjection {
    Member {
        source_ordinal: u16,
        kind: LayerMemberSyntaxKind,
        duplicate: bool,
        assignment: PendingLayerAssignment,
        value: PendingLayerMemberValue,
        trailing_recovery: bool,
    },
    Recovery {
        source_ordinal: u16,
    },
}

impl PendingLayerMemberProjection {
    pub(crate) const fn source_ordinal(&self) -> u16 {
        match self {
            Self::Member { source_ordinal, .. } | Self::Recovery { source_ordinal } => {
                *source_ordinal
            }
        }
    }

    pub(crate) const fn kind(&self) -> Option<LayerMemberSyntaxKind> {
        match self {
            Self::Member { kind, .. } => Some(*kind),
            Self::Recovery { .. } => None,
        }
    }

    pub(crate) const fn duplicate(&self) -> bool {
        match self {
            Self::Member { duplicate, .. } => *duplicate,
            Self::Recovery { .. } => false,
        }
    }

    pub(crate) const fn assignment(&self) -> Option<PendingLayerAssignment> {
        match self {
            Self::Member { assignment, .. } => Some(*assignment),
            Self::Recovery { .. } => None,
        }
    }

    pub(crate) const fn value(&self) -> Option<&PendingLayerMemberValue> {
        match self {
            Self::Member { value, .. } => Some(value),
            Self::Recovery { .. } => None,
        }
    }

    pub(crate) const fn has_trailing_recovery(&self) -> bool {
        match self {
            Self::Member {
                trailing_recovery, ..
            } => *trailing_recovery,
            Self::Recovery { .. } => false,
        }
    }

    fn has_recovery(&self) -> bool {
        match self {
            Self::Member {
                duplicate,
                assignment,
                value,
                trailing_recovery,
                ..
            } => {
                *duplicate
                    || matches!(assignment, PendingLayerAssignment::Missing(_))
                    || value.has_recovery()
                    || *trailing_recovery
            }
            Self::Recovery { .. } => true,
        }
    }

    fn rebased(&self, offset: usize) -> Option<Self> {
        Some(match self {
            Self::Member {
                source_ordinal,
                kind,
                duplicate,
                assignment,
                value,
                trailing_recovery,
            } => Self::Member {
                source_ordinal: *source_ordinal,
                kind: *kind,
                duplicate: *duplicate,
                assignment: assignment.rebased(offset)?,
                value: value.clone(),
                trailing_recovery: *trailing_recovery,
            },
            Self::Recovery { source_ordinal } => Self::Recovery {
                source_ordinal: *source_ordinal,
            },
        })
    }
}

/// Parser-selected Layer body shape.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PendingLayerBodyProjection {
    Missing,
    Braced {
        closed: bool,
        members: Box<[PendingLayerMemberProjection]>,
    },
}

impl PendingLayerBodyProjection {
    fn has_recovery(&self) -> bool {
        match self {
            Self::Missing => true,
            Self::Braced { closed, members } => {
                !closed
                    || members
                        .iter()
                        .any(PendingLayerMemberProjection::has_recovery)
            }
        }
    }

    fn rebased(&self, offset: usize) -> Option<Self> {
        Some(match self {
            Self::Missing => Self::Missing,
            Self::Braced { closed, members } => Self::Braced {
                closed: *closed,
                members: members
                    .iter()
                    .map(|member| member.rebased(offset))
                    .collect::<Option<Vec<_>>>()?
                    .into_boxed_slice(),
            },
        })
    }
}

/// Sole parser-owned semantic projection for one Layer declaration item.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PendingLayerDeclarationProjection {
    colon: PendingLayerColon,
    kind: PendingLayerKind,
    body: PendingLayerBodyProjection,
    trailing_syntax: bool,
}

impl PendingLayerDeclarationProjection {
    pub(crate) const fn new(
        colon: PendingLayerColon,
        kind: PendingLayerKind,
        body: PendingLayerBodyProjection,
        trailing_syntax: bool,
    ) -> Self {
        Self {
            colon,
            kind,
            body,
            trailing_syntax,
        }
    }

    pub(crate) const fn colon(&self) -> PendingLayerColon {
        self.colon
    }

    pub(crate) const fn kind(&self) -> PendingLayerKind {
        self.kind
    }

    pub(crate) const fn body(&self) -> &PendingLayerBodyProjection {
        &self.body
    }

    pub(crate) const fn has_trailing_syntax(&self) -> bool {
        self.trailing_syntax
    }

    pub(crate) fn has_recovery(&self) -> bool {
        matches!(self.colon, PendingLayerColon::Missing(_))
            || matches!(
                self.kind,
                PendingLayerKind::Missing | PendingLayerKind::Unknown
            )
            || self.body.has_recovery()
            || self.trailing_syntax
    }

    pub(crate) fn rebased(&self, offset: usize) -> Option<Self> {
        Some(Self {
            colon: self.colon.rebased(offset)?,
            kind: self.kind,
            body: self.body.rebased(offset)?,
            trailing_syntax: self.trailing_syntax,
        })
    }
}

fn rebase_range(range: SourceRange, offset: usize) -> Option<SourceRange> {
    Some(SourceRange::new(
        range.start().checked_add(offset)?,
        range.end().checked_add(offset)?,
    ))
}
