//! Syntax-level style declaration data.
//!
//! `style` declarations are entity declarations whose body is parsed enough for
//! HIR lowering, formatting, diagnostics, and source/file dependency tracking.
//! CSS is a style language variant, not a separate top-level declaration family.

use super::common::TextRange;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum StyleSyntax {
    #[default]
    Arcweft,
    Css,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StyleDeclBody {
    syntax: StyleSyntax,
    source: StyleDeclSource,
    range: TextRange,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StyleDeclSource {
    Inline(String),
    Files(Vec<StyleFileSource>),
    FilesWithInline {
        files: Vec<StyleFileSource>,
        inline: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StyleFileSource {
    mode: StyleFileMode,
    path: String,
    range: TextRange,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StyleFileMode {
    File,
    Embed,
}

impl StyleDeclBody {
    pub const fn new(syntax: StyleSyntax, source: StyleDeclSource, range: TextRange) -> Self {
        Self {
            syntax,
            source,
            range,
        }
    }

    pub const fn syntax(&self) -> StyleSyntax {
        self.syntax
    }

    pub const fn source(&self) -> &StyleDeclSource {
        &self.source
    }

    pub const fn range(&self) -> TextRange {
        self.range
    }
}

impl StyleFileSource {
    pub fn new(mode: StyleFileMode, path: impl Into<String>, range: TextRange) -> Self {
        Self {
            mode,
            path: path.into(),
            range,
        }
    }

    pub const fn mode(&self) -> StyleFileMode {
        self.mode
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub const fn range(&self) -> TextRange {
        self.range
    }
}
