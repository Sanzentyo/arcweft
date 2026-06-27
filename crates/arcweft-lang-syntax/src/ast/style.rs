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

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum StyleVisibility {
    #[default]
    Private,
    Public,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StyleDecl {
    visibility: StyleVisibility,
    name: String,
    body: StyleDeclBody,
    range: TextRange,
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

impl StyleDecl {
    pub fn new(
        visibility: StyleVisibility,
        name: impl Into<String>,
        body: StyleDeclBody,
        range: TextRange,
    ) -> Self {
        Self {
            visibility,
            name: name.into(),
            body,
            range,
        }
    }

    pub fn public(name: impl Into<String>, body: StyleDeclBody, range: TextRange) -> Self {
        Self::new(StyleVisibility::Public, name, body, range)
    }

    pub const fn visibility(&self) -> StyleVisibility {
        self.visibility
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub const fn body(&self) -> &StyleDeclBody {
        &self.body
    }

    pub const fn range(&self) -> TextRange {
        self.range
    }
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

    pub fn arcweft_inline(source: impl Into<String>, range: TextRange) -> Self {
        Self::new(
            StyleSyntax::Arcweft,
            StyleDeclSource::Inline(source.into()),
            range,
        )
    }

    pub fn css_inline(source: impl Into<String>, range: TextRange) -> Self {
        Self::new(
            StyleSyntax::Css,
            StyleDeclSource::Inline(source.into()),
            range,
        )
    }

    pub fn css_file(path: impl Into<String>, range: TextRange) -> Self {
        Self::new(StyleSyntax::Css, StyleDeclSource::file(path, range), range)
    }

    pub fn css_embed(path: impl Into<String>, range: TextRange) -> Self {
        Self::new(StyleSyntax::Css, StyleDeclSource::embed(path, range), range)
    }
}

impl StyleDeclSource {
    pub fn file(path: impl Into<String>, range: TextRange) -> Self {
        Self::Files(vec![StyleFileSource::new(StyleFileMode::File, path, range)])
    }

    pub fn embed(path: impl Into<String>, range: TextRange) -> Self {
        Self::Files(vec![StyleFileSource::new(
            StyleFileMode::Embed,
            path,
            range,
        )])
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

#[cfg(test)]
mod tests {
    use super::{
        StyleDecl, StyleDeclBody, StyleDeclSource, StyleFileMode, StyleSyntax, StyleVisibility,
    };
    use crate::ast::common::TextRange;

    #[test]
    fn public_style_defaults_to_arcweft_syntax() {
        let range = TextRange::new(0, 22);
        let decl = StyleDecl::public(
            "dialogue",
            StyleDeclBody::arcweft_inline("opacity: 1", range),
            range,
        );

        assert_eq!(decl.visibility(), StyleVisibility::Public);
        assert_eq!(decl.body().syntax(), StyleSyntax::Arcweft);
    }

    #[test]
    fn css_style_sources_distinguish_file_and_embed() {
        let range = TextRange::new(0, 32);
        let file = StyleDeclBody::css_file("ui/dialogue.css", range);
        let embed = StyleDeclBody::css_embed("ui/default.css", range);

        assert!(matches!(
            file.source(),
            StyleDeclSource::Files(files) if files[0].mode() == StyleFileMode::File
        ));
        assert!(matches!(
            embed.source(),
            StyleDeclSource::Files(files) if files[0].mode() == StyleFileMode::Embed
        ));
    }
}
