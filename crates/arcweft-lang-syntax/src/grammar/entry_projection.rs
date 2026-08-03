//! Parser-owned semantic projection for one `entry` declaration.

use arcweft_source::SourceRange;

use crate::name::{SyntaxName, SyntaxNameIssue};

/// Closed built-in entry adapter vocabulary.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum KnownEntryKind {
    Game,
    Editor,
    Cli,
    Server,
    Activity,
    Test,
    Bench,
    Agent,
}

impl KnownEntryKind {
    pub(crate) const fn from_source_name(source: &str) -> Option<Self> {
        match source.as_bytes() {
            b"game" => Some(Self::Game),
            b"editor" => Some(Self::Editor),
            b"cli" => Some(Self::Cli),
            b"server" => Some(Self::Server),
            b"activity" => Some(Self::Activity),
            b"test" => Some(Self::Test),
            b"bench" => Some(Self::Bench),
            b"agent" => Some(Self::Agent),
            _ => None,
        }
    }
}

/// Parser-selected entry kind bound to its exact syntax range.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PendingEntryKind {
    Known {
        value: KnownEntryKind,
        source: SourceRange,
    },
    Custom {
        value: SyntaxName,
        source: SourceRange,
    },
    Missing {
        insertion: SourceRange,
    },
}

impl PendingEntryKind {
    pub(crate) const fn has_recovery(&self) -> bool {
        matches!(self, Self::Missing { .. })
    }

    fn rebased(&self, offset: usize) -> Option<Self> {
        Some(match self {
            Self::Known { value, source } => Self::Known {
                value: *value,
                source: rebase_range(*source, offset)?,
            },
            Self::Custom { value, source } => Self::Custom {
                value: value.clone(),
                source: rebase_range(*source, offset)?,
            },
            Self::Missing { insertion } => Self::Missing {
                insertion: rebase_range(*insertion, offset)?,
            },
        })
    }
}

/// Required entry ID and parser-selected canonical-family status.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PendingEntryId {
    Authored {
        source: SourceRange,
        canonical_entry_family: bool,
    },
    Missing {
        insertion: SourceRange,
    },
}

impl PendingEntryId {
    pub(crate) const fn has_recovery(self) -> bool {
        matches!(
            self,
            Self::Authored {
                canonical_entry_family: false,
                ..
            } | Self::Missing { .. }
        )
    }

    fn rebased(self, offset: usize) -> Option<Self> {
        Some(match self {
            Self::Authored {
                source,
                canonical_entry_family,
            } => Self::Authored {
                source: rebase_range(source, offset)?,
                canonical_entry_family,
            },
            Self::Missing { insertion } => Self::Missing {
                insertion: rebase_range(insertion, offset)?,
            },
        })
    }
}

/// Closed semantic role of one entry role binding.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum EntryRoleSyntaxKind {
    State,
    Initializer,
    Event,
    Reducer,
    Controller,
}

impl EntryRoleSyntaxKind {
    pub(crate) const fn from_source_name(source: &str) -> Option<Self> {
        match source.as_bytes() {
            b"state" => Some(Self::State),
            b"initializer" => Some(Self::Initializer),
            b"event" => Some(Self::Event),
            b"reducer" => Some(Self::Reducer),
            b"controller" => Some(Self::Controller),
            _ => None,
        }
    }

    pub(crate) const fn expects_type(self) -> bool {
        matches!(self, Self::State | Self::Event)
    }
}

/// Closed supported HTTP method vocabulary.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum KnownEntryHttpMethod {
    Get,
    Post,
    Put,
    Patch,
    Delete,
    Head,
    Options,
}

impl KnownEntryHttpMethod {
    pub(crate) const fn from_source_name(source: &str) -> Option<Self> {
        match source.as_bytes() {
            b"GET" => Some(Self::Get),
            b"POST" => Some(Self::Post),
            b"PUT" => Some(Self::Put),
            b"PATCH" => Some(Self::Patch),
            b"DELETE" => Some(Self::Delete),
            b"HEAD" => Some(Self::Head),
            b"OPTIONS" => Some(Self::Options),
            _ => None,
        }
    }
}

/// Parser-selected HTTP method or exact typed recovery.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PendingEntryHttpMethod {
    Known {
        value: KnownEntryHttpMethod,
        source: SourceRange,
    },
    Unsupported {
        value: Result<SyntaxName, SyntaxNameIssue>,
        source: SourceRange,
    },
    Missing {
        insertion: SourceRange,
    },
}

impl PendingEntryHttpMethod {
    pub(crate) const fn has_recovery(&self) -> bool {
        !matches!(self, Self::Known { .. })
    }

    fn rebased(&self, offset: usize) -> Option<Self> {
        Some(match self {
            Self::Known { value, source } => Self::Known {
                value: *value,
                source: rebase_range(*source, offset)?,
            },
            Self::Unsupported { value, source } => Self::Unsupported {
                value: value.clone(),
                source: rebase_range(*source, offset)?,
            },
            Self::Missing { insertion } => Self::Missing {
                insertion: rebase_range(*insertion, offset)?,
            },
        })
    }
}

/// One parser-owned name or insertion without an attachment-time text read.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PendingEntryName {
    Authored {
        value: Result<SyntaxName, SyntaxNameIssue>,
        source: SourceRange,
    },
    Missing {
        insertion: SourceRange,
    },
}

impl PendingEntryName {
    pub(crate) const fn has_recovery(&self) -> bool {
        matches!(
            self,
            Self::Missing { .. } | Self::Authored { value: Err(_), .. }
        )
    }

    fn rebased(&self, offset: usize) -> Option<Self> {
        Some(match self {
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

/// Required punctuation authored in source or inserted by current recovery.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PendingEntryPunctuation {
    Authored(SourceRange),
    Missing(SourceRange),
}

impl PendingEntryPunctuation {
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

/// Shape of a required semantic child selected by the parser.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PendingEntryValueState {
    Authored,
    Missing,
    Invalid,
}

impl PendingEntryValueState {
    pub(crate) const fn has_recovery(self) -> bool {
        !matches!(self, Self::Authored)
    }
}

/// One explicit route binding in exact source order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PendingEntryRouteBinding {
    pub(crate) source_ordinal: u16,
    pub(crate) parameter: PendingEntryName,
    pub(crate) equals: PendingEntryPunctuation,
    pub(crate) colon: PendingEntryPunctuation,
    pub(crate) capture: PendingEntryName,
    pub(crate) trailing_recovery: bool,
}

impl PendingEntryRouteBinding {
    fn has_recovery(&self) -> bool {
        self.parameter.has_recovery()
            || self.equals.has_recovery()
            || self.colon.has_recovery()
            || self.capture.has_recovery()
            || self.trailing_recovery
    }

    fn rebased(&self, offset: usize) -> Option<Self> {
        Some(Self {
            source_ordinal: self.source_ordinal,
            parameter: self.parameter.rebased(offset)?,
            equals: self.equals.rebased(offset)?,
            colon: self.colon.rebased(offset)?,
            capture: self.capture.rebased(offset)?,
            trailing_recovery: self.trailing_recovery,
        })
    }
}

/// Optional route binding-list owner and delimiter recovery.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PendingEntryRouteBindings {
    Absent,
    Parenthesized {
        bindings: Box<[PendingEntryRouteBinding]>,
        closed: bool,
    },
}

impl PendingEntryRouteBindings {
    fn has_recovery(&self) -> bool {
        match self {
            Self::Absent => false,
            Self::Parenthesized { bindings, closed } => {
                !*closed || bindings.iter().any(PendingEntryRouteBinding::has_recovery)
            }
        }
    }

    fn rebased(&self, offset: usize) -> Option<Self> {
        Some(match self {
            Self::Absent => Self::Absent,
            Self::Parenthesized { bindings, closed } => Self::Parenthesized {
                bindings: bindings
                    .iter()
                    .map(|binding| binding.rebased(offset))
                    .collect::<Option<Vec<_>>>()?
                    .into_boxed_slice(),
                closed: *closed,
            },
        })
    }
}

/// One Entry body member in exact source order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PendingEntryMemberProjection {
    Role {
        source_ordinal: u32,
        role: EntryRoleSyntaxKind,
        assignment: PendingEntryPunctuation,
        value: PendingEntryValueState,
        trailing_recovery: bool,
    },
    Goto {
        source_ordinal: u32,
        target: PendingEntryValueState,
        trailing_recovery: bool,
    },
    Route {
        source_ordinal: u32,
        method: PendingEntryHttpMethod,
        path: PendingEntryValueState,
        arrow: PendingEntryPunctuation,
        target: PendingEntryValueState,
        bindings: PendingEntryRouteBindings,
        trailing_recovery: bool,
    },
    Option {
        source_ordinal: u32,
        name: PendingEntryName,
        assignment: PendingEntryPunctuation,
        value: PendingEntryValueState,
        trailing_recovery: bool,
    },
    Recovery {
        source_ordinal: u32,
    },
}

impl PendingEntryMemberProjection {
    pub(crate) const fn source_ordinal(&self) -> u32 {
        match self {
            Self::Role { source_ordinal, .. }
            | Self::Goto { source_ordinal, .. }
            | Self::Route { source_ordinal, .. }
            | Self::Option { source_ordinal, .. }
            | Self::Recovery { source_ordinal } => *source_ordinal,
        }
    }

    fn has_recovery(&self) -> bool {
        match self {
            Self::Role {
                assignment,
                value,
                trailing_recovery,
                ..
            } => assignment.has_recovery() || value.has_recovery() || *trailing_recovery,
            Self::Goto {
                target,
                trailing_recovery,
                ..
            } => target.has_recovery() || *trailing_recovery,
            Self::Route {
                method,
                path,
                arrow,
                target,
                bindings,
                trailing_recovery,
                ..
            } => {
                method.has_recovery()
                    || path.has_recovery()
                    || arrow.has_recovery()
                    || target.has_recovery()
                    || bindings.has_recovery()
                    || *trailing_recovery
            }
            Self::Option {
                name,
                assignment,
                value,
                trailing_recovery,
                ..
            } => {
                name.has_recovery()
                    || assignment.has_recovery()
                    || value.has_recovery()
                    || *trailing_recovery
            }
            Self::Recovery { .. } => true,
        }
    }

    fn rebased(&self, offset: usize) -> Option<Self> {
        Some(match self {
            Self::Role {
                source_ordinal,
                role,
                assignment,
                value,
                trailing_recovery,
            } => Self::Role {
                source_ordinal: *source_ordinal,
                role: *role,
                assignment: assignment.rebased(offset)?,
                value: *value,
                trailing_recovery: *trailing_recovery,
            },
            Self::Goto {
                source_ordinal,
                target,
                trailing_recovery,
            } => Self::Goto {
                source_ordinal: *source_ordinal,
                target: *target,
                trailing_recovery: *trailing_recovery,
            },
            Self::Route {
                source_ordinal,
                method,
                path,
                arrow,
                target,
                bindings,
                trailing_recovery,
            } => Self::Route {
                source_ordinal: *source_ordinal,
                method: method.rebased(offset)?,
                path: *path,
                arrow: arrow.rebased(offset)?,
                target: *target,
                bindings: bindings.rebased(offset)?,
                trailing_recovery: *trailing_recovery,
            },
            Self::Option {
                source_ordinal,
                name,
                assignment,
                value,
                trailing_recovery,
            } => Self::Option {
                source_ordinal: *source_ordinal,
                name: name.rebased(offset)?,
                assignment: assignment.rebased(offset)?,
                value: *value,
                trailing_recovery: *trailing_recovery,
            },
            Self::Recovery { source_ordinal } => Self::Recovery {
                source_ordinal: *source_ordinal,
            },
        })
    }
}

/// Missing or authored Entry body and its exact member projection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PendingEntryBodyProjection {
    Missing,
    Braced {
        members: Box<[PendingEntryMemberProjection]>,
        closed: bool,
    },
}

impl PendingEntryBodyProjection {
    fn has_recovery(&self) -> bool {
        match self {
            Self::Missing => true,
            Self::Braced { members, closed } => {
                !*closed
                    || members
                        .iter()
                        .any(PendingEntryMemberProjection::has_recovery)
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

/// Complete parser-selected semantic owner for one Entry declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PendingEntryDeclarationProjection {
    pub(crate) kind: PendingEntryKind,
    pub(crate) id: PendingEntryId,
    pub(crate) trailing_header_recovery: bool,
    pub(crate) body: PendingEntryBodyProjection,
}

impl PendingEntryDeclarationProjection {
    pub(crate) fn has_recovery(&self) -> bool {
        self.kind.has_recovery()
            || self.id.has_recovery()
            || self.trailing_header_recovery
            || self.body.has_recovery()
    }

    pub(crate) fn rebased(&self, offset: usize) -> Option<Self> {
        Some(Self {
            kind: self.kind.rebased(offset)?,
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
