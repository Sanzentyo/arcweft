use super::common::TextRange;

/// Absolute entity reference such as `@flow.opening` or `@<flow.opening@sem:abc>`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EntityRef {
    body: String,
    delimited: bool,
    authored: bool,
    range: TextRange,
    authored_body_range: Option<TextRange>,
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
            authored: false,
            range,
            authored_body_range: None,
        }
    }

    pub(crate) fn authored(body: String, delimited: bool, range: TextRange) -> Self {
        let start = range.start() + if delimited { 2 } else { 1 };
        let authored_body_range = TextRange::new(start, start + body.len());
        Self {
            body,
            delimited,
            authored: true,
            range,
            authored_body_range: Some(authored_body_range),
        }
    }

    /// Creates a normalized declaration identity while retaining that its
    /// source spelling was explicit. Relative declaration IDs cannot expose
    /// an exact authored body range after family normalization, so this
    /// provenance is kept separately from `authored_body_range`.
    pub(crate) fn normalized_authored(body: String, delimited: bool, range: TextRange) -> Self {
        Self {
            body,
            delimited,
            authored: true,
            range,
            authored_body_range: None,
        }
    }

    pub fn module_scoped_declaration(
        family: &str,
        suffix: &str,
        module_path: Option<&str>,
        range: TextRange,
    ) -> Self {
        Self::new(
            Self::module_scoped_declaration_body(family, suffix, module_path),
            false,
            range,
        )
    }

    pub fn module_scoped_declaration_body(
        family: &str,
        suffix: &str,
        module_path: Option<&str>,
    ) -> String {
        module_path
            .map(str::trim)
            .filter(|module| !module.is_empty())
            .map(|module| module.replace("::", "."))
            .map_or_else(
                || format!("{family}.{suffix}"),
                |module| format!("{family}.{module}.{suffix}"),
            )
    }

    pub fn body(&self) -> &str {
        &self.body
    }

    pub const fn is_delimited(&self) -> bool {
        self.delimited
    }

    /// Whether the declaration identity was present in authored source.
    pub const fn is_authored(&self) -> bool {
        self.authored
    }

    pub const fn range(&self) -> &TextRange {
        &self.range
    }

    /// Exact parser-owned byte range of the authored public-ID body.
    ///
    /// Normalized and synthetic references return `None`; their canonical body
    /// is not necessarily present in the retained source range.
    pub const fn authored_body_range(&self) -> Option<TextRange> {
        self.authored_body_range
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

    /// Whether this ID came from an explicit authored identity position.
    pub const fn is_authored(&self) -> bool {
        match self {
            Self::Absolute(entity) => entity.is_authored(),
            Self::Relative(_) | Self::FamilyRelative(_) => true,
        }
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

    pub fn canonical_body(&self) -> String {
        match self {
            Self::Absolute(entity) => entity.body().to_owned(),
            Self::FamilyRelative(relative) => relative.canonical_body(),
        }
    }

    pub fn canonical_entity_ref(&self) -> EntityRef {
        EntityRef::new(self.canonical_body(), self.is_delimited(), *self.range())
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

    pub(crate) fn with_authored_range(&self, range: TextRange) -> Self {
        match self {
            Self::Absolute(entity) => Self::Absolute(EntityRef::authored(
                entity.body().to_owned(),
                entity.is_delimited(),
                range,
            )),
            Self::FamilyRelative(relative) => {
                let delta = range.start().saturating_sub(relative.range().start());
                let relative_range = TextRange::new(
                    relative.relative().range().start() + delta,
                    relative.relative().range().end() + delta,
                );
                Self::FamilyRelative(FamilyRelativeEntityRef::new(
                    relative.family().to_owned(),
                    RelativeId::new(
                        relative.relative().suffix().to_owned(),
                        relative.relative().parent_depth(),
                        relative.relative().spelling(),
                        relative_range,
                    ),
                    range,
                ))
            }
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

    pub fn canonical_body(&self) -> String {
        format!("{}.{}", self.family, self.relative.suffix())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authored_entity_body_ranges_exclude_absolute_and_delimited_markers() {
        let absolute = EntityRef::authored("entry.main".to_owned(), false, TextRange::new(10, 21));
        assert_eq!(absolute.authored_body_range(), Some(TextRange::new(11, 21)));

        let delimited = EntityRef::authored("entry.main".to_owned(), true, TextRange::new(20, 33));
        assert_eq!(
            delimited.authored_body_range(),
            Some(TextRange::new(22, 32))
        );
    }

    #[test]
    fn normalized_and_synthetic_entity_refs_do_not_claim_authored_body_ranges() {
        let synthetic = EntityRef::new("entry.synthetic".to_owned(), false, TextRange::new(4, 9));
        assert_eq!(synthetic.authored_body_range(), None);
        assert_eq!(
            EntityRef::module_scoped_declaration(
                "entry",
                "main",
                Some("crate.feature"),
                TextRange::new(0, 5),
            )
            .authored_body_range(),
            None
        );

        let relative = EntityRefSyntax::family_relative(FamilyRelativeEntityRef::new(
            "entry".to_owned(),
            RelativeId::new(
                "main".to_owned(),
                0,
                RelativeIdSpelling::DotRun,
                TextRange::new(7, 12),
            ),
            TextRange::new(0, 12),
        ));
        assert_eq!(relative.canonical_entity_ref().authored_body_range(), None);
    }
}
