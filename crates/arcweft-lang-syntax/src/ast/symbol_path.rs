//! Typed paths for project symbols whose external segments need not be module identifiers.

use core::{fmt, str::FromStr};

use thiserror::Error;

use super::{
    common::TextRange,
    module_path::{ModulePathError, ModulePathRoot, ModuleSegment},
};

/// Typed project-symbol reference with validated module qualifiers and an exact binding leaf.
///
/// The leaf deliberately permits punctuation used by external domains (for example
/// `character.akane`). Module-like qualifiers remain validated [`ModuleSegment`] values.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SymbolPath {
    root: ModulePathRoot,
    qualifiers: Vec<ModuleSegment>,
    leaf: String,
}

/// Invalid project-symbol binding path.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum SymbolPathError {
    #[error("symbol binding leaf must not be empty")]
    EmptyLeaf,
    #[error("symbol binding leaf contains a control character at byte {byte}")]
    Control { byte: usize },
    #[error("symbol binding leaf must not contain `::`")]
    QualifiedLeaf,
}

impl SymbolPath {
    pub fn try_new(
        root: ModulePathRoot,
        qualifiers: Vec<ModuleSegment>,
        leaf: impl Into<String>,
    ) -> Result<Self, SymbolPathError> {
        let leaf = leaf.into();
        if leaf.is_empty() {
            return Err(SymbolPathError::EmptyLeaf);
        }
        if let Some((byte, _)) = leaf
            .char_indices()
            .find(|(_, character)| character.is_control())
        {
            return Err(SymbolPathError::Control { byte });
        }
        if leaf.contains("::") {
            return Err(SymbolPathError::QualifiedLeaf);
        }
        Ok(Self {
            root,
            qualifiers,
            leaf,
        })
    }

    pub const fn root(&self) -> ModulePathRoot {
        self.root
    }

    pub fn qualifiers(&self) -> &[ModuleSegment] {
        &self.qualifiers
    }

    pub fn leaf(&self) -> &str {
        &self.leaf
    }

    pub fn canonical_string(&self) -> String {
        let mut result = String::new();
        match self.root {
            ModulePathRoot::ImplicitCrate => {}
            ModulePathRoot::Crate => result.push_str("crate."),
            ModulePathRoot::SelfModule => result.push_str("self."),
            ModulePathRoot::Super(levels) => {
                for _ in 0..levels {
                    result.push_str("super.");
                }
            }
        }
        for qualifier in &self.qualifiers {
            result.push_str(qualifier.as_str());
            result.push('.');
        }
        result.push_str(&self.leaf);
        result
    }
}

impl fmt::Display for SymbolPath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.canonical_string())
    }
}

/// One validated project-symbol path segment.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProjectSymbolSegment(String);

/// A source path that may address either a project module or an external symbol.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProjectSymbolPath {
    root: ModulePathRoot,
    segments: Vec<ProjectSymbolSegment>,
}

/// A project-symbol path and the exact source ranges of its segments.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SpannedProjectSymbolPath {
    path: ProjectSymbolPath,
    range: TextRange,
    segment_ranges: Vec<TextRange>,
}

/// An introduced `use` alias with its exact source token range.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UseAlias {
    name: ModuleSegment,
    range: TextRange,
}

/// Invalid project-symbol path.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ProjectSymbolPathError {
    #[error("project symbol path is empty")]
    Empty,
    #[error("project symbol segment is empty")]
    EmptySegment,
    #[error("invalid project symbol segment `{segment}`")]
    InvalidSegment { segment: String },
    #[error("implicit project symbol path has invalid first segment `{segment}`")]
    InvalidImplicitRoot { segment: String },
}

impl ProjectSymbolSegment {
    /// Creates a non-empty segment containing letters, numbers, `_`, or `-`.
    pub fn try_new(value: impl Into<String>) -> Result<Self, ProjectSymbolPathError> {
        let value = value.into();
        if value.is_empty() {
            return Err(ProjectSymbolPathError::EmptySegment);
        }
        if !value
            .chars()
            .all(|character| character.is_alphanumeric() || matches!(character, '_' | '-'))
        {
            return Err(ProjectSymbolPathError::InvalidSegment { segment: value });
        }
        Ok(Self(value))
    }

    /// Exact segment spelling.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Converts this external-capable segment when it is also a module identifier.
    pub fn try_as_module_segment(&self) -> Result<ModuleSegment, ModulePathError> {
        ModuleSegment::new(self.0.clone())
    }
}

impl ProjectSymbolPath {
    /// Creates a path from an explicit module root and validated segments.
    pub fn new(
        root: ModulePathRoot,
        segments: impl IntoIterator<Item = ProjectSymbolSegment>,
    ) -> Result<Self, ProjectSymbolPathError> {
        let segments = segments.into_iter().collect::<Vec<_>>();
        let Some(first) = segments.first() else {
            return Err(ProjectSymbolPathError::Empty);
        };
        if matches!(root, ModulePathRoot::ImplicitCrate)
            && !first
                .as_str()
                .chars()
                .next()
                .is_some_and(|character| character == '_' || character.is_alphabetic())
        {
            return Err(ProjectSymbolPathError::InvalidImplicitRoot {
                segment: first.as_str().to_owned(),
            });
        }
        Ok(Self { root, segments })
    }

    /// Root behavior selected by the source spelling.
    pub const fn root(&self) -> ModulePathRoot {
        self.root
    }

    /// Validated segments following the root spelling.
    pub fn segments(&self) -> &[ProjectSymbolSegment] {
        &self.segments
    }

    /// Final symbol segment.
    ///
    /// # Panics
    ///
    /// Panics only if the constructor invariant requiring at least one segment
    /// is violated inside this crate.
    pub fn last_segment(&self) -> &ProjectSymbolSegment {
        self.segments
            .last()
            .expect("project symbol paths contain at least one segment")
    }
}

impl TryFrom<&ProjectSymbolPath> for SymbolPath {
    type Error = ModulePathError;

    fn try_from(path: &ProjectSymbolPath) -> Result<Self, Self::Error> {
        let (leaf, qualifiers) = path
            .segments
            .split_last()
            .expect("project symbol paths contain at least one segment");
        match qualifiers
            .iter()
            .map(ProjectSymbolSegment::try_as_module_segment)
            .collect::<Result<Vec<_>, _>>()
        {
            Ok(qualifiers) => Ok(Self {
                root: path.root,
                qualifiers,
                leaf: leaf.as_str().to_owned(),
            }),
            Err(_) if matches!(path.root, ModulePathRoot::ImplicitCrate) => Ok(Self {
                root: ModulePathRoot::Crate,
                qualifiers: Vec::new(),
                leaf: path.to_string(),
            }),
            Err(error) => Err(error),
        }
    }
}

impl SpannedProjectSymbolPath {
    /// Parses one path and records ranges relative to `base`.
    pub fn parse_at(source: &str, base: usize) -> Result<Self, ProjectSymbolPathError> {
        let leading = source.len() - source.trim_start().len();
        let trimmed = source.trim();
        if trimmed.is_empty() {
            return Err(ProjectSymbolPathError::Empty);
        }
        let path_start = base + leading;
        let raw = trimmed.split('.').collect::<Vec<_>>();
        if raw.iter().any(|segment| segment.is_empty()) {
            return Err(ProjectSymbolPathError::EmptySegment);
        }
        let (root, first_segment) = match raw.as_slice() {
            ["crate", ..] => (ModulePathRoot::Crate, 1),
            ["self", ..] => (ModulePathRoot::SelfModule, 1),
            ["parent", ..] => (ModulePathRoot::Super(1), 1),
            ["super", ..] => {
                let levels = raw
                    .iter()
                    .take_while(|segment| **segment == "super")
                    .count();
                (ModulePathRoot::Super(levels), levels)
            }
            _ => (ModulePathRoot::ImplicitCrate, 0),
        };

        let mut offset = 0;
        let mut segments = Vec::new();
        let mut segment_ranges = Vec::new();
        for (index, raw_segment) in raw.iter().enumerate() {
            let start = path_start + offset;
            let end = start + raw_segment.len();
            if index >= first_segment {
                segments.push(ProjectSymbolSegment::try_new((*raw_segment).to_owned())?);
                segment_ranges.push(TextRange::new(start, end));
            }
            offset += raw_segment.len() + 1;
        }
        let path = ProjectSymbolPath::new(root, segments)?;
        Ok(Self {
            path,
            range: TextRange::new(path_start, path_start + trimmed.len()),
            segment_ranges,
        })
    }

    /// Parsed path.
    pub const fn path(&self) -> &ProjectSymbolPath {
        &self.path
    }

    /// Whole path token range.
    pub const fn range(&self) -> TextRange {
        self.range
    }

    /// Exact ranges for every non-root segment.
    pub fn segment_ranges(&self) -> &[TextRange] {
        &self.segment_ranges
    }
}

impl UseAlias {
    /// Creates an alias from a validated module-style name and token range.
    pub const fn new(name: ModuleSegment, range: TextRange) -> Self {
        Self { name, range }
    }

    pub const fn name(&self) -> &ModuleSegment {
        &self.name
    }

    pub const fn range(&self) -> TextRange {
        self.range
    }
}

impl FromStr for ProjectSymbolPath {
    type Err = ProjectSymbolPathError;

    fn from_str(source: &str) -> Result<Self, Self::Err> {
        Ok(SpannedProjectSymbolPath::parse_at(source, 0)?.path)
    }
}

impl fmt::Display for ProjectSymbolSegment {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl fmt::Display for ProjectSymbolPath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.root {
            ModulePathRoot::ImplicitCrate => {}
            ModulePathRoot::Crate => formatter.write_str("crate")?,
            ModulePathRoot::SelfModule => formatter.write_str("self")?,
            ModulePathRoot::Super(levels) => {
                for index in 0..levels {
                    if index > 0 {
                        formatter.write_str(".")?;
                    }
                    formatter.write_str("super")?;
                }
            }
        }
        for (index, segment) in self.segments.iter().enumerate() {
            if index > 0 || !matches!(self.root, ModulePathRoot::ImplicitCrate) {
                formatter.write_str(".")?;
            }
            fmt::Display::fmt(segment, formatter)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ProjectSymbolPath, ProjectSymbolPathError, ProjectSymbolSegment, SpannedProjectSymbolPath,
        SymbolPath, TextRange,
    };

    #[test]
    fn project_symbol_segment_accepts_external_hyphen_without_becoming_a_module_segment() {
        let external = ProjectSymbolSegment::try_new("hero-pack").expect("external segment");
        let path = ProjectSymbolPath::new(
            super::ModulePathRoot::ImplicitCrate,
            [
                ProjectSymbolSegment::try_new("character").expect("namespace segment"),
                external.clone(),
            ],
        )
        .expect("qualified external path");

        assert_eq!(
            path.segments()
                .iter()
                .map(ProjectSymbolSegment::as_str)
                .collect::<Vec<_>>(),
            ["character", "hero-pack"]
        );
        assert!(external.try_as_module_segment().is_err());
    }

    #[test]
    fn project_symbol_segment_rejects_empty_control_and_separators() {
        assert_eq!(
            ProjectSymbolSegment::try_new(""),
            Err(ProjectSymbolPathError::EmptySegment)
        );
        for value in ["a.b", "a:b", "a/b", "a\\b", "\u{7}"] {
            assert!(matches!(
                ProjectSymbolSegment::try_new(value),
                Err(ProjectSymbolPathError::InvalidSegment { .. })
            ));
        }
    }

    #[test]
    fn project_symbol_path_rejects_empty_and_invalid_implicit_root() {
        assert_eq!(
            ProjectSymbolPath::new(super::ModulePathRoot::ImplicitCrate, []),
            Err(ProjectSymbolPathError::Empty)
        );
        let numeric = ProjectSymbolSegment::try_new("2d").expect("numeric segment");
        assert!(matches!(
            ProjectSymbolPath::new(super::ModulePathRoot::ImplicitCrate, [numeric.clone()]),
            Err(ProjectSymbolPathError::InvalidImplicitRoot { segment }) if segment == "2d"
        ));
        assert!(
            ProjectSymbolPath::new(
                super::ModulePathRoot::ImplicitCrate,
                [
                    ProjectSymbolSegment::try_new("character").expect("namespace segment"),
                    numeric,
                ],
            )
            .is_ok()
        );
    }

    #[test]
    fn accepts_external_segments_and_records_exact_ranges() {
        let path = SpannedProjectSymbolPath::parse_at("character.hero-pack.2d", 10).unwrap();
        assert_eq!(path.path().to_string(), "character.hero-pack.2d");
        assert_eq!(path.range().as_range(), 10..32);
        assert_eq!(
            path.segment_ranges()
                .iter()
                .map(TextRange::as_range)
                .collect::<Vec<_>>(),
            vec![10..19, 20..29, 30..32]
        );
    }

    #[test]
    fn enforces_implicit_root_without_restricting_later_segments() {
        assert!(matches!(
            "-hero.face".parse::<ProjectSymbolPath>(),
            Err(ProjectSymbolPathError::InvalidImplicitRoot { .. })
        ));
        assert!("hero.-face".parse::<ProjectSymbolPath>().is_ok());
    }

    #[test]
    fn project_paths_convert_to_module_qualified_or_external_root_symbols() {
        let callable = "helpers.neutral_name"
            .parse::<ProjectSymbolPath>()
            .expect("callable path");
        let callable = SymbolPath::try_from(&callable).expect("qualified symbol");
        assert_eq!(callable.canonical_string(), "helpers.neutral_name");
        assert_eq!(callable.qualifiers()[0].as_str(), "helpers");
        assert_eq!(callable.leaf(), "neutral_name");

        let external = "character.hero-pack.v2"
            .parse::<ProjectSymbolPath>()
            .expect("external path");
        let external = SymbolPath::try_from(&external).expect("external root symbol");
        assert_eq!(external.root(), super::ModulePathRoot::Crate);
        assert!(external.qualifiers().is_empty());
        assert_eq!(external.leaf(), "character.hero-pack.v2");
    }
}
