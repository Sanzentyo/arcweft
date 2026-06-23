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

/// `use` import.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UseItem {
    visibility: Option<Visibility>,
    tree: UseTree,
    range: TextRange,
}

/// Typed `use` tree syntax with the module prefix pre-parsed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UseTree {
    source: String,
    module_path_prefix: ModulePath,
    exact_module_prefix: bool,
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
        tree: UseTree,
        range: TextRange,
    ) -> Self {
        Self {
            visibility,
            tree,
            range,
        }
    }

    pub const fn visibility(&self) -> Option<Visibility> {
        self.visibility
    }

    pub const fn tree(&self) -> &UseTree {
        &self.tree
    }

    pub const fn range(&self) -> &TextRange {
        &self.range
    }
}

impl UseTree {
    /// Parses a normalized import tree and extracts its module prefix.
    pub fn parse(source: impl Into<String>) -> Result<Self, ModulePathError> {
        let source = normalize_parent_module_root(&source.into());
        let module_prefix_source = use_tree_module_prefix_source(&source);
        let module_path_prefix = ModulePath::from_str(module_prefix_source)?;
        let exact_module_prefix = source.contains("::{") || source.ends_with("::*");
        Ok(Self {
            source,
            module_path_prefix,
            exact_module_prefix,
        })
    }

    /// Normalized source spelling of the use tree.
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Whether the extracted prefix is syntactically known to name a module.
    pub const fn module_path_is_exact(&self) -> bool {
        self.exact_module_prefix
    }

    /// Returns the longest syntactic module prefix of this use tree.
    ///
    /// The project loader resolves this prefix against existing module paths,
    /// walking one parent when the final segment can be an imported item.
    pub fn module_path_prefix(&self) -> &ModulePath {
        &self.module_path_prefix
    }
}

fn normalize_parent_module_root(path: &str) -> String {
    path.strip_prefix("parent::")
        .map_or_else(|| path.to_owned(), |tail| format!("super::{tail}"))
}

fn use_tree_module_prefix_source(source: &str) -> &str {
    let without_alias = source.split_once(" as ").map_or(source, |(path, _)| path);
    let grouped = without_alias
        .find("::{")
        .map_or(without_alias, |index| &without_alias[..index]);
    grouped.strip_suffix("::*").unwrap_or(grouped).trim()
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
