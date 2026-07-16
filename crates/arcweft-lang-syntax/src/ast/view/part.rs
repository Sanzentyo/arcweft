//! Source-bound typed nodes for authored View-part modifiers and exports.

use arcweft_source::SourceSpan;

/// Private local View-part name with its exact source span.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewPartLocalNameSyntax {
    text: String,
    span: SourceSpan,
}

/// Public View-part name with its exact source span.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewPartNameSyntax {
    text: String,
    span: SourceSpan,
}

/// Private `.part(name)` modifier attached to one node-producing View expression.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewPartModifier {
    local_name: ViewPartLocalNameSyntax,
    modifier_span: SourceSpan,
}

/// Leading `export part local as public` declaration in one View body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewPartExportDecl {
    local_name: ViewPartLocalNameSyntax,
    public_name: ViewPartNameSyntax,
    declaration_span: SourceSpan,
    export_keyword_span: SourceSpan,
    part_keyword_span: SourceSpan,
    as_keyword_span: SourceSpan,
}

impl ViewPartLocalNameSyntax {
    pub(crate) const fn new(text: String, span: SourceSpan) -> Self {
        Self { text, span }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub const fn span(&self) -> &SourceSpan {
        &self.span
    }
}

impl ViewPartNameSyntax {
    pub(crate) const fn new(text: String, span: SourceSpan) -> Self {
        Self { text, span }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub const fn span(&self) -> &SourceSpan {
        &self.span
    }
}

impl ViewPartModifier {
    pub(crate) const fn new(
        local_name: ViewPartLocalNameSyntax,
        modifier_span: SourceSpan,
    ) -> Self {
        Self {
            local_name,
            modifier_span,
        }
    }

    pub const fn local_name(&self) -> &ViewPartLocalNameSyntax {
        &self.local_name
    }

    pub const fn modifier_span(&self) -> &SourceSpan {
        &self.modifier_span
    }

    pub const fn operand_span(&self) -> &SourceSpan {
        self.local_name.span()
    }
}

impl ViewPartExportDecl {
    pub(crate) const fn new(
        local_name: ViewPartLocalNameSyntax,
        public_name: ViewPartNameSyntax,
        declaration_span: SourceSpan,
        export_keyword_span: SourceSpan,
        part_keyword_span: SourceSpan,
        as_keyword_span: SourceSpan,
    ) -> Self {
        Self {
            local_name,
            public_name,
            declaration_span,
            export_keyword_span,
            part_keyword_span,
            as_keyword_span,
        }
    }

    pub const fn local_name(&self) -> &ViewPartLocalNameSyntax {
        &self.local_name
    }

    pub const fn public_name(&self) -> &ViewPartNameSyntax {
        &self.public_name
    }

    pub const fn declaration_span(&self) -> &SourceSpan {
        &self.declaration_span
    }

    pub const fn export_keyword_span(&self) -> &SourceSpan {
        &self.export_keyword_span
    }

    pub const fn part_keyword_span(&self) -> &SourceSpan {
        &self.part_keyword_span
    }

    pub const fn local_operand_span(&self) -> &SourceSpan {
        self.local_name.span()
    }

    pub const fn as_keyword_span(&self) -> &SourceSpan {
        &self.as_keyword_span
    }

    pub const fn public_operand_span(&self) -> &SourceSpan {
        self.public_name.span()
    }
}
