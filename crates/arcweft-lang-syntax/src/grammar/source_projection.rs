//! Parser-owned semantic projections for source-file paths and import trees.

use arcweft_source::SourceRange;

/// Root semantics fixed while the path grammar owns the token cursor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PendingPathRoot {
    ImplicitCrate,
    Crate(SourceRange),
    SelfModule(SourceRange),
    Super(Box<[SourceRange]>),
}

/// Parser-validated token family of one ID-less path segment.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum PendingPathSegmentKind {
    Identifier,
    Keyword,
    ProjectSymbol,
    Lifetime,
}

/// Parser-selected visibility semantics retained without source-text lookup.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum PendingVisibilityKind {
    Public,
    Crate,
    Super,
    Recovery,
}

/// One ID-less path component owned by the complete `Path` identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PendingPathSegment {
    kind: PendingPathSegmentKind,
    source: SourceRange,
}

impl PendingPathSegment {
    pub(crate) const fn new(kind: PendingPathSegmentKind, source: SourceRange) -> Self {
        Self { kind, source }
    }

    pub(crate) const fn kind(&self) -> PendingPathSegmentKind {
        self.kind
    }

    pub(crate) const fn source(&self) -> SourceRange {
        self.source
    }

    fn rebased(&self, offset: usize) -> Option<Self> {
        Some(Self::new(self.kind, rebase_range(self.source, offset)?))
    }
}

/// Semantic root and ordered ID-less segments attached to one `Path` node.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PendingPathProjection {
    root: PendingPathRoot,
    segments: Box<[PendingPathSegment]>,
}

impl PendingPathProjection {
    pub(crate) fn new(
        root: PendingPathRoot,
        segments: impl Into<Box<[PendingPathSegment]>>,
    ) -> Self {
        Self {
            root,
            segments: segments.into(),
        }
    }

    pub(crate) const fn root(&self) -> &PendingPathRoot {
        &self.root
    }

    pub(crate) const fn segments(&self) -> &[PendingPathSegment] {
        &self.segments
    }

    pub(crate) fn rebased(&self, offset: usize) -> Option<Self> {
        let root = match &self.root {
            PendingPathRoot::ImplicitCrate => PendingPathRoot::ImplicitCrate,
            PendingPathRoot::Crate(source) => {
                PendingPathRoot::Crate(rebase_range(*source, offset)?)
            }
            PendingPathRoot::SelfModule(source) => {
                PendingPathRoot::SelfModule(rebase_range(*source, offset)?)
            }
            PendingPathRoot::Super(levels) => PendingPathRoot::Super(
                levels
                    .iter()
                    .map(|source| rebase_range(*source, offset))
                    .collect::<Option<Vec<_>>>()?
                    .into_boxed_slice(),
            ),
        };
        Some(Self::new(
            root,
            self.segments
                .iter()
                .map(|segment| segment.rebased(offset))
                .collect::<Option<Vec<_>>>()?,
        ))
    }
}

/// Parser-owned structural alias whose name remains an identity-bearing child.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PendingUseAlias {
    source: SourceRange,
}

impl PendingUseAlias {
    pub(crate) const fn new(source: SourceRange) -> Self {
        Self { source }
    }

    pub(crate) const fn source(&self) -> SourceRange {
        self.source
    }

    fn rebased(&self, offset: usize) -> Option<Self> {
        Some(Self::new(rebase_range(self.source, offset)?))
    }
}

/// One authored grouped-import member in source order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PendingUseGroupMember {
    Binding {
        source: SourceRange,
        name_ordinal: u32,
        name_kind: PendingPathSegmentKind,
        alias_ordinal: Option<u16>,
        recovery_ordinal: Option<u32>,
    },
    Recovery {
        source: SourceRange,
        recovery_ordinal: u32,
    },
}

impl PendingUseGroupMember {
    pub(crate) const fn source(&self) -> SourceRange {
        match self {
            Self::Binding { source, .. } | Self::Recovery { source, .. } => *source,
        }
    }

    pub(crate) const fn name_ordinal(&self) -> Option<u32> {
        match self {
            Self::Binding { name_ordinal, .. } => Some(*name_ordinal),
            Self::Recovery { .. } => None,
        }
    }

    pub(crate) const fn alias_ordinal(&self) -> Option<u16> {
        match self {
            Self::Binding { alias_ordinal, .. } => *alias_ordinal,
            Self::Recovery { .. } => None,
        }
    }

    pub(crate) const fn name_kind(&self) -> Option<PendingPathSegmentKind> {
        match self {
            Self::Binding { name_kind, .. } => Some(*name_kind),
            Self::Recovery { .. } => None,
        }
    }

    pub(crate) const fn recovery_ordinal(&self) -> Option<u32> {
        match self {
            Self::Binding {
                recovery_ordinal, ..
            } => *recovery_ordinal,
            Self::Recovery {
                recovery_ordinal, ..
            } => Some(*recovery_ordinal),
        }
    }

    fn rebased(&self, offset: usize) -> Option<Self> {
        Some(match self {
            Self::Binding {
                source,
                name_ordinal,
                name_kind,
                alias_ordinal,
                recovery_ordinal,
            } => Self::Binding {
                source: rebase_range(*source, offset)?,
                name_ordinal: *name_ordinal,
                name_kind: *name_kind,
                alias_ordinal: *alias_ordinal,
                recovery_ordinal: *recovery_ordinal,
            },
            Self::Recovery {
                source,
                recovery_ordinal,
            } => Self::Recovery {
                source: rebase_range(*source, offset)?,
                recovery_ordinal: *recovery_ordinal,
            },
        })
    }
}

/// Structural form selected by one `UseDeclaration` grammar transaction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PendingUseTreeKind {
    Path,
    Glob { marker: SourceRange },
    Group(Box<[PendingUseGroupMember]>),
}

/// Typed import-tree projection attached to the `UseDeclaration` identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PendingUseProjection {
    kind: PendingUseTreeKind,
    aliases: Box<[PendingUseAlias]>,
}

impl PendingUseProjection {
    pub(crate) fn new(
        kind: PendingUseTreeKind,
        aliases: impl Into<Box<[PendingUseAlias]>>,
    ) -> Self {
        Self {
            kind,
            aliases: aliases.into(),
        }
    }

    pub(crate) const fn kind(&self) -> &PendingUseTreeKind {
        &self.kind
    }

    pub(crate) const fn aliases(&self) -> &[PendingUseAlias] {
        &self.aliases
    }

    pub(crate) fn rebased(&self, offset: usize) -> Option<Self> {
        let kind = match &self.kind {
            PendingUseTreeKind::Path => PendingUseTreeKind::Path,
            PendingUseTreeKind::Glob { marker } => PendingUseTreeKind::Glob {
                marker: rebase_range(*marker, offset)?,
            },
            PendingUseTreeKind::Group(members) => PendingUseTreeKind::Group(
                members
                    .iter()
                    .map(|member| member.rebased(offset))
                    .collect::<Option<Vec<_>>>()?
                    .into_boxed_slice(),
            ),
        };
        Some(Self::new(
            kind,
            self.aliases
                .iter()
                .map(|alias| alias.rebased(offset))
                .collect::<Option<Vec<_>>>()?,
        ))
    }
}

fn rebase_range(range: SourceRange, offset: usize) -> Option<SourceRange> {
    Some(SourceRange::new(
        range.start().checked_add(offset)?,
        range.end().checked_add(offset)?,
    ))
}
