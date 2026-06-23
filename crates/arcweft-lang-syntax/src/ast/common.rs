use super::module_path::{ModulePath, ModulePathError};
use core::{ops::Range, str::FromStr};

/// Half-open byte range in the original source.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TextRange {
    start: usize,
    end: usize,
}

/// `mod game::routes::opening`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModuleDecl {
    path: String,
    range: TextRange,
}

/// `use`, `lazy use`, or `eager use` import.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UseItem {
    visibility: Option<Visibility>,
    mode: Option<UseMode>,
    tree: String,
    range: TextRange,
}

/// Markdown documentation comment collected from consecutive `///` lines.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DocBlock {
    text: String,
    range: TextRange,
}

/// Arcweft visibility qualifier.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Visibility {
    Public,
    Crate,
    Super,
}

/// Explicit import realization qualifier written in source.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UseMode {
    Lazy,
    Eager,
}

/// Effective dependency mode after applying the default for plain `use`.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum UseDependencyMode {
    Normal,
    Eager,
    Lazy,
}

impl TextRange {
    /// Builds a half-open byte range.
    pub const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    /// Start byte offset.
    pub const fn start(&self) -> usize {
        self.start
    }

    /// End byte offset.
    pub const fn end(&self) -> usize {
        self.end
    }

    /// Converts to the standard range type.
    pub fn as_range(&self) -> Range<usize> {
        self.start..self.end
    }
}

impl ModuleDecl {
    pub(crate) const fn new(path: String, range: TextRange) -> Self {
        Self { path, range }
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    /// Returns the typed path consumed by project loading and HIR tooling.
    pub fn module_path(&self) -> Result<ModulePath, ModulePathError> {
        ModulePath::from_str(&self.path)
    }

    pub const fn range(&self) -> &TextRange {
        &self.range
    }
}

impl UseItem {
    pub(crate) const fn new(
        visibility: Option<Visibility>,
        mode: Option<UseMode>,
        tree: String,
        range: TextRange,
    ) -> Self {
        Self {
            visibility,
            mode,
            tree,
            range,
        }
    }

    pub const fn visibility(&self) -> Option<Visibility> {
        self.visibility
    }

    pub const fn mode(&self) -> Option<UseMode> {
        self.mode
    }

    /// Effective dependency behavior, including the plain-`use` default.
    pub const fn dependency_mode(&self) -> UseDependencyMode {
        match self.mode {
            Some(mode) => mode.dependency_mode(),
            None => UseDependencyMode::Normal,
        }
    }

    pub fn tree(&self) -> &str {
        &self.tree
    }

    /// Whether the extracted prefix is syntactically known to name a module.
    pub fn module_path_is_exact(&self) -> bool {
        self.tree.contains("::{") || self.tree.ends_with("::*")
    }

    /// Returns the longest syntactic module prefix of this use tree.
    ///
    /// The project loader resolves this prefix against existing module paths,
    /// walking one parent when the final segment can be an imported item.
    pub fn module_path_prefix(&self) -> Result<ModulePath, ModulePathError> {
        let without_alias = self
            .tree
            .split_once(" as ")
            .map_or(self.tree.as_str(), |(path, _)| path);
        let grouped = without_alias
            .find("::{")
            .map_or(without_alias, |index| &without_alias[..index]);
        let globless = grouped.strip_suffix("::*").unwrap_or(grouped);
        ModulePath::from_str(globless.trim())
    }

    pub const fn range(&self) -> &TextRange {
        &self.range
    }
}

impl UseMode {
    /// Effective graph mode represented by this explicit qualifier.
    pub const fn dependency_mode(self) -> UseDependencyMode {
        match self {
            Self::Lazy => UseDependencyMode::Lazy,
            Self::Eager => UseDependencyMode::Eager,
        }
    }

    /// Whether the imported body participates in the initial build.
    pub const fn loads_body_during_initial_build(self) -> bool {
        self.dependency_mode().loads_body_during_initial_build()
    }
}

impl UseDependencyMode {
    /// Whether the dependency body participates in the initial build.
    pub const fn loads_body_during_initial_build(self) -> bool {
        !matches!(self, Self::Lazy)
    }

    /// Merges duplicate imports of the same module without losing eagerness.
    #[must_use]
    pub const fn merge(self, other: Self) -> Self {
        match (self, other) {
            (Self::Eager, _) | (_, Self::Eager) => Self::Eager,
            (Self::Normal, _) | (_, Self::Normal) => Self::Normal,
            (Self::Lazy, Self::Lazy) => Self::Lazy,
        }
    }

    /// Stable spelling for diagnostics, metadata, and cache keys.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Eager => "eager",
            Self::Lazy => "lazy",
        }
    }
}

impl DocBlock {
    pub(crate) const fn new(text: String, range: TextRange) -> Self {
        Self { text, range }
    }

    /// Markdown text without the leading `///` markers.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Source range covered by the whole doc block.
    pub const fn range(&self) -> &TextRange {
        &self.range
    }
}
