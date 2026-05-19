use super::TextRange;

/// Absolute entity reference such as `@flow.opening` or `@<flow.opening@sem:abc>`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EntityRef {
    body: String,
    delimited: bool,
    range: TextRange,
}

/// ID-bearing reference accepted by declaration-like ID positions.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IdRef {
    Absolute(EntityRef),
    Relative(RelativeId),
    FamilyRelative(FamilyRelativeEntityRef),
}

/// Relative ID suffix such as `@.greeting`, `@..shared`, or `@super.shared`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RelativeId {
    suffix: String,
    parent_depth: usize,
    spelling: RelativeIdSpelling,
    range: TextRange,
}

/// Source spelling used for a relative ID marker.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RelativeIdSpelling {
    DotRun,
    SuperChain,
}

/// Entity reference syntax before family-relative references are normalized.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EntityRefSyntax {
    Absolute(EntityRef),
    FamilyRelative(FamilyRelativeEntityRef),
}

/// Family-qualified relative entity reference such as `@flow:.next`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FamilyRelativeEntityRef {
    family: String,
    relative: RelativeId,
    range: TextRange,
}

/// Documentation/RAG link written as `[[...]]`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WikiLink {
    body: String,
    range: TextRange,
}

impl EntityRef {
    pub const fn new(body: String, delimited: bool, range: TextRange) -> Self {
        Self {
            body,
            delimited,
            range,
        }
    }

    pub fn body(&self) -> &str {
        &self.body
    }

    pub const fn is_delimited(&self) -> bool {
        self.delimited
    }

    pub const fn range(&self) -> &TextRange {
        &self.range
    }
}

impl IdRef {
    pub const fn absolute(entity: EntityRef) -> Self {
        Self::Absolute(entity)
    }

    pub(crate) const fn relative(relative: RelativeId) -> Self {
        Self::Relative(relative)
    }

    pub(crate) const fn family_relative(relative: FamilyRelativeEntityRef) -> Self {
        Self::FamilyRelative(relative)
    }

    pub fn body(&self) -> &str {
        match self {
            Self::Absolute(entity) => entity.body(),
            Self::Relative(relative) => relative.suffix(),
            Self::FamilyRelative(relative) => relative.relative().suffix(),
        }
    }

    pub const fn is_relative(&self) -> bool {
        matches!(self, Self::Relative(_) | Self::FamilyRelative(_))
    }

    pub const fn relative_id(&self) -> Option<&RelativeId> {
        match self {
            Self::Absolute(_) => None,
            Self::Relative(relative) => Some(relative),
            Self::FamilyRelative(relative) => Some(relative.relative()),
        }
    }

    pub const fn as_absolute(&self) -> Option<&EntityRef> {
        match self {
            Self::Absolute(entity) => Some(entity),
            Self::Relative(_) | Self::FamilyRelative(_) => None,
        }
    }

    pub const fn family_relative_ref(&self) -> Option<&FamilyRelativeEntityRef> {
        match self {
            Self::Absolute(_) | Self::Relative(_) => None,
            Self::FamilyRelative(relative) => Some(relative),
        }
    }

    pub const fn range(&self) -> &TextRange {
        match self {
            Self::Absolute(entity) => entity.range(),
            Self::Relative(relative) => relative.range(),
            Self::FamilyRelative(relative) => relative.range(),
        }
    }
}

impl RelativeId {
    pub(crate) const fn new(
        suffix: String,
        parent_depth: usize,
        spelling: RelativeIdSpelling,
        range: TextRange,
    ) -> Self {
        Self {
            suffix,
            parent_depth,
            spelling,
            range,
        }
    }

    pub fn suffix(&self) -> &str {
        &self.suffix
    }

    pub const fn parent_depth(&self) -> usize {
        self.parent_depth
    }

    pub const fn spelling(&self) -> RelativeIdSpelling {
        self.spelling
    }

    pub const fn range(&self) -> &TextRange {
        &self.range
    }
}

impl EntityRefSyntax {
    pub const fn absolute(entity: EntityRef) -> Self {
        Self::Absolute(entity)
    }

    pub(crate) const fn family_relative(relative: FamilyRelativeEntityRef) -> Self {
        Self::FamilyRelative(relative)
    }

    pub fn body(&self) -> &str {
        match self {
            Self::Absolute(entity) => entity.body(),
            Self::FamilyRelative(relative) => relative.relative().suffix(),
        }
    }

    pub const fn as_absolute(&self) -> Option<&EntityRef> {
        match self {
            Self::Absolute(entity) => Some(entity),
            Self::FamilyRelative(_) => None,
        }
    }

    pub const fn family_relative_ref(&self) -> Option<&FamilyRelativeEntityRef> {
        match self {
            Self::Absolute(_) => None,
            Self::FamilyRelative(relative) => Some(relative),
        }
    }

    pub const fn is_delimited(&self) -> bool {
        match self {
            Self::Absolute(entity) => entity.is_delimited(),
            Self::FamilyRelative(_) => false,
        }
    }

    pub const fn range(&self) -> &TextRange {
        match self {
            Self::Absolute(entity) => entity.range(),
            Self::FamilyRelative(relative) => relative.range(),
        }
    }
}

impl FamilyRelativeEntityRef {
    pub(crate) const fn new(family: String, relative: RelativeId, range: TextRange) -> Self {
        Self {
            family,
            relative,
            range,
        }
    }

    pub fn family(&self) -> &str {
        &self.family
    }

    pub const fn relative(&self) -> &RelativeId {
        &self.relative
    }

    pub const fn range(&self) -> &TextRange {
        &self.range
    }
}

impl WikiLink {
    pub(crate) const fn new(body: String, range: TextRange) -> Self {
        Self { body, range }
    }

    pub fn body(&self) -> &str {
        &self.body
    }

    pub const fn range(&self) -> &TextRange {
        &self.range
    }
}
