//! Typed authored entity-reference syntax shared by source roles.

use arcweft_source::SourceRange;

use crate::name::{SyntaxName, SyntaxNameIssue};

/// One validated non-empty ID suffix segment.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AuthoredIdSegment(Box<str>);

/// Root semantics of an authored entity reference.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AuthoredIdRoot {
    Absolute {
        delimited: bool,
    },
    Relative {
        parent_depth: usize,
    },
    FamilyRelative {
        family: SyntaxName,
        parent_depth: usize,
    },
}

/// Structured entity-reference value retained without a source reader.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AuthoredIdRef {
    root: AuthoredIdRoot,
    segments: Box<[AuthoredIdSegment]>,
}

/// Source shape used to validate an entity reference's exact component map.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SyntaxIdRefShape {
    absolute_marker: bool,
    family: bool,
    parent_depth: usize,
    segment_count: u32,
}

/// Resolved or recovered authored entity-reference payload.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SyntaxIdRefSyntax {
    value: Result<AuthoredIdRef, SyntaxIdRefIssue>,
    shape: SyntaxIdRefShape,
}

/// Typed entity-reference recovery.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SyntaxIdRefIssue {
    MissingSuffix,
    InvalidFamily(SyntaxNameIssue),
    InvalidSegment { ordinal: u32 },
}

/// Source component of a structured entity reference.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SyntaxIdRefPart {
    Whole,
    AbsoluteMarker,
    Family,
    FamilySeparator,
    ParentMarker { ordinal: u32 },
    SuffixSegment { ordinal: u32 },
}

/// One exact source component selected by the entity-reference lexer.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SyntaxIdRefComponent {
    part: SyntaxIdRefPart,
    range: SourceRange,
}

impl SyntaxIdRefComponent {
    pub(crate) const fn new(part: SyntaxIdRefPart, range: SourceRange) -> Self {
        Self { part, range }
    }

    pub const fn part(self) -> SyntaxIdRefPart {
        self.part
    }

    pub const fn range(self) -> SourceRange {
        self.range
    }
}

impl AuthoredIdSegment {
    pub(crate) fn try_new(value: &str) -> Result<Self, ()> {
        (!value.is_empty()).then(|| Self(value.into())).ok_or(())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl AuthoredIdRef {
    pub(crate) fn new(root: AuthoredIdRoot, segments: Vec<AuthoredIdSegment>) -> Self {
        Self {
            root,
            segments: segments.into_boxed_slice(),
        }
    }

    pub const fn root(&self) -> &AuthoredIdRoot {
        &self.root
    }

    pub fn segments(&self) -> &[AuthoredIdSegment] {
        &self.segments
    }
}

impl SyntaxIdRefShape {
    pub(crate) const fn new(
        absolute_marker: bool,
        family: bool,
        parent_depth: usize,
        segment_count: u32,
    ) -> Self {
        Self {
            absolute_marker,
            family,
            parent_depth,
            segment_count,
        }
    }

    pub const fn has_absolute_marker(self) -> bool {
        self.absolute_marker
    }

    pub const fn has_family(self) -> bool {
        self.family
    }

    pub const fn parent_depth(self) -> usize {
        self.parent_depth
    }

    pub const fn segment_count(self) -> u32 {
        self.segment_count
    }
}

impl SyntaxIdRefSyntax {
    pub(crate) const fn new(
        value: Result<AuthoredIdRef, SyntaxIdRefIssue>,
        shape: SyntaxIdRefShape,
    ) -> Self {
        Self { value, shape }
    }

    pub const fn value(&self) -> Result<&AuthoredIdRef, &SyntaxIdRefIssue> {
        self.value.as_ref()
    }

    pub const fn shape(&self) -> SyntaxIdRefShape {
        self.shape
    }

    /// Normalizes a relative reference into one expected declaration family.
    ///
    /// Absolute and already family-relative references retain their authored
    /// roots. The boolean reports whether that root belongs to `family`.
    pub(crate) fn normalized_for_family(&self, family: &SyntaxName) -> (Self, bool) {
        let Ok(reference) = self.value() else {
            return (self.clone(), false);
        };
        let canonical = match reference.root() {
            AuthoredIdRoot::Absolute { .. } => reference
                .segments()
                .first()
                .is_some_and(|segment| segment.as_str() == family.as_str()),
            AuthoredIdRoot::Relative { .. } => true,
            AuthoredIdRoot::FamilyRelative {
                family: authored, ..
            } => authored == family,
        };
        if !canonical {
            return (self.clone(), false);
        }
        let root = match reference.root() {
            AuthoredIdRoot::Relative { parent_depth } => AuthoredIdRoot::FamilyRelative {
                family: family.clone(),
                parent_depth: *parent_depth,
            },
            root => root.clone(),
        };
        (
            Self::new(
                Ok(AuthoredIdRef::new(root, reference.segments().to_vec())),
                self.shape,
            ),
            true,
        )
    }
}
