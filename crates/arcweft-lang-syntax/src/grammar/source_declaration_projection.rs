//! Parser-owned semantic projection for one temporary `source` declaration.
//!
//! This owner exists only for the Proof attached-syntax/HIR switch. Lang-01.3
//! later deletes the complete Source language/runtime surface in favour of
//! ordinary functions returning `Stream`.

use arcweft_source::SourceRange;

use crate::id_ref::{SyntaxIdRefIssue, SyntaxIdRefSyntax};
use crate::name::{SyntaxName, SyntaxNameIssue};

/// Optional Source identity selected by the entity-reference lexer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PendingSourceId {
    Absent,
    Authored {
        value: SyntaxIdRefSyntax,
        source: SourceRange,
        canonical_source_family: bool,
        requires_name: bool,
    },
}

impl PendingSourceId {
    pub(crate) fn has_recovery(&self) -> bool {
        match self {
            Self::Absent => false,
            Self::Authored {
                value,
                canonical_source_family,
                requires_name,
                ..
            } => {
                !canonical_source_family
                    || match value.value() {
                        Ok(_) => false,
                        Err(SyntaxIdRefIssue::MissingSuffix) if *requires_name => false,
                        Err(_) => true,
                    }
            }
        }
    }

    fn rebased(&self, offset: usize) -> Option<Self> {
        Some(match self {
            Self::Absent => Self::Absent,
            Self::Authored {
                value,
                source,
                canonical_source_family,
                requires_name,
            } => Self::Authored {
                value: value.clone(),
                source: rebase_range(*source, offset)?,
                canonical_source_family: *canonical_source_family,
                requires_name: *requires_name,
            },
        })
    }
}

/// Optional or required local Source name selected without attachment-time text reads.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PendingSourceName {
    Absent,
    Authored {
        value: Result<SyntaxName, SyntaxNameIssue>,
        source: SourceRange,
    },
    Missing {
        insertion: SourceRange,
    },
}

impl PendingSourceName {
    pub(crate) const fn has_recovery(&self) -> bool {
        matches!(
            self,
            Self::Missing { .. } | Self::Authored { value: Err(_), .. }
        )
    }

    fn rebased(&self, offset: usize) -> Option<Self> {
        Some(match self {
            Self::Absent => Self::Absent,
            Self::Authored { value, source } => Self::Authored {
                value: value.clone(),
                source: rebase_range(*source, offset)?,
            },
            Self::Missing { insertion } => Self::Missing {
                insertion: rebase_range(*insertion, offset)?,
            },
        })
    }
}

/// Required Source type state. Its semantic value remains owned by the shared type projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PendingSourceTypeState {
    Authored,
    Missing,
}

/// Required punctuation authored in source or inserted by current recovery.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PendingSourcePunctuation {
    Authored(SourceRange),
    Missing(SourceRange),
}

impl PendingSourcePunctuation {
    pub(crate) const fn range(self) -> SourceRange {
        match self {
            Self::Authored(range) | Self::Missing(range) => range,
        }
    }

    pub(crate) const fn has_recovery(self) -> bool {
        matches!(self, Self::Missing(_))
    }

    fn rebased(self, offset: usize) -> Option<Self> {
        let range = rebase_range(self.range(), offset)?;
        Some(match self {
            Self::Authored(_) => Self::Authored(range),
            Self::Missing(_) => Self::Missing(range),
        })
    }
}

/// Shape of one required expression or pattern child.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PendingSourceChildState {
    Authored,
    Missing,
    Invalid,
}

impl PendingSourceChildState {
    pub(crate) const fn has_recovery(self) -> bool {
        !matches!(self, Self::Authored)
    }
}

/// One selected named argument of `bounded(...)`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PendingSourceBoundedArgument {
    Missing,
    Present {
        ordinal: u16,
        value: PendingSourceChildState,
        duplicate: bool,
    },
}

impl PendingSourceBoundedArgument {
    pub(crate) const fn has_recovery(self) -> bool {
        match self {
            Self::Missing => true,
            Self::Present {
                value, duplicate, ..
            } => value.has_recovery() || duplicate,
        }
    }

    /// Whether the selected argument value itself recovered. Duplicate
    /// evidence is orthogonal: the first authored value remains the selected
    /// typed value while the aggregate duplicate bit poisons the policy.
    pub(crate) const fn value_has_recovery(self) -> bool {
        match self {
            Self::Missing => true,
            Self::Present { value, .. } => value.has_recovery(),
        }
    }
}

/// Closed Source overflow policy or exact typed recovery.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PendingSourceOverflowPolicy {
    DropOldest(PendingSourceBoundedArgument),
    DropNewest(PendingSourceBoundedArgument),
    Error(PendingSourceBoundedArgument),
    Coalesce(PendingSourceBoundedArgument),
    Missing,
    Unknown {
        argument: PendingSourceBoundedArgument,
        value: Option<SyntaxName>,
    },
    Invalid {
        argument: PendingSourceBoundedArgument,
    },
}

impl PendingSourceOverflowPolicy {
    pub(crate) const fn has_recovery(&self) -> bool {
        match self {
            Self::DropOldest(argument)
            | Self::DropNewest(argument)
            | Self::Error(argument)
            | Self::Coalesce(argument) => argument.has_recovery(),
            Self::Missing | Self::Unknown { .. } | Self::Invalid { .. } => true,
        }
    }
}

/// Closed Source backpressure policy or exact typed recovery.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PendingSourceBackpressurePolicy {
    Latest,
    Bounded {
        capacity: PendingSourceBoundedArgument,
        overflow: PendingSourceOverflowPolicy,
        unexpected_arguments: bool,
        recovered_call: bool,
    },
    BlockingNotAllowed,
    Missing,
    Unknown(Option<SyntaxName>),
    Invalid,
}

impl PendingSourceBackpressurePolicy {
    pub(crate) const fn has_recovery(&self) -> bool {
        match self {
            Self::Latest | Self::BlockingNotAllowed => false,
            Self::Bounded {
                capacity,
                overflow,
                unexpected_arguments,
                recovered_call,
            } => {
                capacity.has_recovery()
                    || overflow.has_recovery()
                    || *unexpected_arguments
                    || *recovered_call
            }
            Self::Missing | Self::Unknown(_) | Self::Invalid => true,
        }
    }
}

/// Closed Source replay vocabulary.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum SourceReplaySyntaxKind {
    Full,
    HashOnly,
    Summary,
    EventOnly,
    None,
}

/// Closed Source privacy vocabulary.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum SourcePrivacySyntaxKind {
    Transient,
    Redacted,
    Recordable,
    Private,
}

/// One closed single-name policy or typed recovery.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PendingSourceNamedPolicy<T> {
    Known(T),
    Missing,
    Unknown(Option<SyntaxName>),
    Invalid,
}

impl<T> PendingSourceNamedPolicy<T> {
    pub(crate) const fn has_recovery(&self) -> bool {
        !matches!(self, Self::Known(_))
    }
}

/// Closed Source handler event selected by the parser.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PendingSourceHandlerEvent {
    Item(PendingSourceChildState),
    Error(PendingSourceChildState),
    Progress(PendingSourceChildState),
    Disconnected(PendingSourceChildState),
    PermissionRevoked(PendingSourceChildState),
    End(PendingSourceChildState),
    Unknown {
        value: Option<SyntaxName>,
        condition: PendingSourceChildState,
    },
}

impl PendingSourceHandlerEvent {
    pub(crate) const fn has_recovery(&self) -> bool {
        match self {
            Self::Item(state)
            | Self::Error(state)
            | Self::Progress(state)
            | Self::Disconnected(state)
            | Self::PermissionRevoked(state)
            | Self::End(state) => state.has_recovery(),
            Self::Unknown { .. } => true,
        }
    }
}

/// Missing, single-statement, or statement-only braced handler body.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PendingSourceHandlerBody {
    Missing,
    Statement,
    Block { closed: bool },
}

impl PendingSourceHandlerBody {
    pub(crate) const fn has_recovery(self) -> bool {
        matches!(self, Self::Missing | Self::Block { closed: false })
    }
}

/// Shared contract family retained as typed but unsupported Source semantics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SourceContractSyntaxKind {
    Requires,
    Ensures,
}

/// One Source body member in exact source order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PendingSourceMemberProjection {
    From {
        source_ordinal: u32,
        statement_ordinal: u32,
        value: PendingSourceChildState,
        duplicate: bool,
    },
    Backpressure {
        source_ordinal: u32,
        statement_ordinal: u32,
        assignment: PendingSourcePunctuation,
        policy: PendingSourceBackpressurePolicy,
        duplicate: bool,
    },
    Replay {
        source_ordinal: u32,
        statement_ordinal: u32,
        assignment: PendingSourcePunctuation,
        policy: PendingSourceNamedPolicy<SourceReplaySyntaxKind>,
        duplicate: bool,
    },
    Privacy {
        source_ordinal: u32,
        statement_ordinal: u32,
        assignment: PendingSourcePunctuation,
        policy: PendingSourceNamedPolicy<SourcePrivacySyntaxKind>,
        duplicate: bool,
    },
    Handler {
        source_ordinal: u32,
        statement_ordinal: u32,
        event: PendingSourceHandlerEvent,
        arrow: PendingSourcePunctuation,
        body: PendingSourceHandlerBody,
    },
    UnsupportedContract {
        source_ordinal: u32,
        contract_ordinal: u16,
        family: SourceContractSyntaxKind,
        family_ordinal: u16,
        condition: PendingSourceChildState,
        out_of_order: bool,
    },
    Recovery {
        source_ordinal: u32,
        statement_ordinal: u32,
    },
}

impl PendingSourceMemberProjection {
    pub(crate) const fn source_ordinal(&self) -> u32 {
        match self {
            Self::From { source_ordinal, .. }
            | Self::Backpressure { source_ordinal, .. }
            | Self::Replay { source_ordinal, .. }
            | Self::Privacy { source_ordinal, .. }
            | Self::Handler { source_ordinal, .. }
            | Self::UnsupportedContract { source_ordinal, .. }
            | Self::Recovery { source_ordinal, .. } => *source_ordinal,
        }
    }

    pub(crate) const fn has_recovery(&self) -> bool {
        match self {
            Self::From {
                value, duplicate, ..
            } => value.has_recovery() || *duplicate,
            Self::Backpressure {
                assignment,
                policy,
                duplicate,
                ..
            } => assignment.has_recovery() || policy.has_recovery() || *duplicate,
            Self::Replay {
                assignment,
                policy,
                duplicate,
                ..
            } => assignment.has_recovery() || policy.has_recovery() || *duplicate,
            Self::Privacy {
                assignment,
                policy,
                duplicate,
                ..
            } => assignment.has_recovery() || policy.has_recovery() || *duplicate,
            Self::Handler {
                event, arrow, body, ..
            } => event.has_recovery() || arrow.has_recovery() || body.has_recovery(),
            Self::UnsupportedContract { .. } | Self::Recovery { .. } => true,
        }
    }

    fn rebased(&self, offset: usize) -> Option<Self> {
        Some(match self {
            Self::From {
                source_ordinal,
                statement_ordinal,
                value,
                duplicate,
            } => Self::From {
                source_ordinal: *source_ordinal,
                statement_ordinal: *statement_ordinal,
                value: *value,
                duplicate: *duplicate,
            },
            Self::Backpressure {
                source_ordinal,
                statement_ordinal,
                assignment,
                policy,
                duplicate,
            } => Self::Backpressure {
                source_ordinal: *source_ordinal,
                statement_ordinal: *statement_ordinal,
                assignment: assignment.rebased(offset)?,
                policy: policy.clone(),
                duplicate: *duplicate,
            },
            Self::Replay {
                source_ordinal,
                statement_ordinal,
                assignment,
                policy,
                duplicate,
            } => Self::Replay {
                source_ordinal: *source_ordinal,
                statement_ordinal: *statement_ordinal,
                assignment: assignment.rebased(offset)?,
                policy: policy.clone(),
                duplicate: *duplicate,
            },
            Self::Privacy {
                source_ordinal,
                statement_ordinal,
                assignment,
                policy,
                duplicate,
            } => Self::Privacy {
                source_ordinal: *source_ordinal,
                statement_ordinal: *statement_ordinal,
                assignment: assignment.rebased(offset)?,
                policy: policy.clone(),
                duplicate: *duplicate,
            },
            Self::Handler {
                source_ordinal,
                statement_ordinal,
                event,
                arrow,
                body,
            } => Self::Handler {
                source_ordinal: *source_ordinal,
                statement_ordinal: *statement_ordinal,
                event: event.clone(),
                arrow: arrow.rebased(offset)?,
                body: *body,
            },
            Self::UnsupportedContract {
                source_ordinal,
                contract_ordinal,
                family,
                family_ordinal,
                condition,
                out_of_order,
            } => Self::UnsupportedContract {
                source_ordinal: *source_ordinal,
                contract_ordinal: *contract_ordinal,
                family: *family,
                family_ordinal: *family_ordinal,
                condition: *condition,
                out_of_order: *out_of_order,
            },
            Self::Recovery {
                source_ordinal,
                statement_ordinal,
            } => Self::Recovery {
                source_ordinal: *source_ordinal,
                statement_ordinal: *statement_ordinal,
            },
        })
    }
}

/// Missing or authored Source body and its exact member projection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PendingSourceBodyProjection {
    Missing,
    Braced {
        members: Box<[PendingSourceMemberProjection]>,
        closed: bool,
    },
}

impl PendingSourceBodyProjection {
    pub(crate) fn has_recovery(&self) -> bool {
        match self {
            Self::Missing => true,
            Self::Braced { members, closed } => {
                !*closed
                    || members
                        .iter()
                        .any(PendingSourceMemberProjection::has_recovery)
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

/// Complete parser-selected semantic owner for one Source declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PendingSourceDeclarationProjection {
    pub(crate) id: PendingSourceId,
    pub(crate) name: PendingSourceName,
    pub(crate) source_type: PendingSourceTypeState,
    pub(crate) missing_type_colon: bool,
    pub(crate) body: PendingSourceBodyProjection,
}

impl PendingSourceDeclarationProjection {
    pub(crate) const fn new(
        id: PendingSourceId,
        name: PendingSourceName,
        source_type: PendingSourceTypeState,
        missing_type_colon: bool,
        body: PendingSourceBodyProjection,
    ) -> Self {
        Self {
            id,
            name,
            source_type,
            missing_type_colon,
            body,
        }
    }

    pub(crate) fn has_recovery(&self) -> bool {
        self.id.has_recovery()
            || self.name.has_recovery()
            || matches!(self.source_type, PendingSourceTypeState::Missing)
            || self.missing_type_colon
            || self.body.has_recovery()
    }

    pub(crate) fn rebased(&self, offset: usize) -> Option<Self> {
        Some(Self {
            id: self.id.rebased(offset)?,
            name: self.name.rebased(offset)?,
            source_type: self.source_type,
            missing_type_colon: self.missing_type_colon,
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
