use core::ops::Range;

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

/// Import realization mode.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UseMode {
    Lazy,
    Eager,
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

    pub fn tree(&self) -> &str {
        &self.tree
    }

    pub const fn range(&self) -> &TextRange {
        &self.range
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
