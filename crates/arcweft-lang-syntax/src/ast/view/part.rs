//! Typed source nodes for authored View-part labels and exports.

use crate::ast::common::TextRange;

/// One unqualified dotted part name with its exact source range.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewPartNameSyntax {
    text: String,
    range: TextRange,
}

/// Private `.part(name)` label attached to one View expression.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewPartLabelSyntax {
    name: ViewPartNameSyntax,
    range: TextRange,
}

/// Leading `export part local as public` declaration in one View body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewPartExportDecl {
    local: ViewPartNameSyntax,
    public: ViewPartNameSyntax,
    export_keyword_range: TextRange,
    part_keyword_range: TextRange,
    as_keyword_range: TextRange,
    range: TextRange,
}

impl ViewPartNameSyntax {
    pub(crate) const fn new(text: String, range: TextRange) -> Self {
        Self { text, range }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub const fn range(&self) -> TextRange {
        self.range
    }
}

impl ViewPartLabelSyntax {
    pub(crate) const fn new(name: ViewPartNameSyntax, range: TextRange) -> Self {
        Self { name, range }
    }

    pub const fn name(&self) -> &ViewPartNameSyntax {
        &self.name
    }

    pub const fn range(&self) -> TextRange {
        self.range
    }
}

impl ViewPartExportDecl {
    pub(crate) const fn new(
        local: ViewPartNameSyntax,
        public: ViewPartNameSyntax,
        export_keyword_range: TextRange,
        part_keyword_range: TextRange,
        as_keyword_range: TextRange,
        range: TextRange,
    ) -> Self {
        Self {
            local,
            public,
            export_keyword_range,
            part_keyword_range,
            as_keyword_range,
            range,
        }
    }

    pub const fn local(&self) -> &ViewPartNameSyntax {
        &self.local
    }

    pub const fn public(&self) -> &ViewPartNameSyntax {
        &self.public
    }

    pub const fn export_keyword_range(&self) -> TextRange {
        self.export_keyword_range
    }

    pub const fn part_keyword_range(&self) -> TextRange {
        self.part_keyword_range
    }

    pub const fn as_keyword_range(&self) -> TextRange {
        self.as_keyword_range
    }

    pub const fn range(&self) -> TextRange {
        self.range
    }
}
