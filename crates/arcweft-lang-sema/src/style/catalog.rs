//! Checked native/CSS Style catalog consumed by compiler lowering.

use arcweft_lang_syntax::ast::common::TextRange;
use arcweft_view::style::{
    ViewPropertyKind, ViewSpecifiedValue, ViewStylePatchId, ViewStyleSelector, ViewStyleSheetId,
    ViewStyleTokenId, ViewStyleValueKind,
};

/// Checked style language variant.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CheckedViewStyleSyntax {
    Arcweft,
    Css,
}

/// Complete checked style output for one HIR module.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CheckedViewStyleCatalog {
    sheets: Vec<CheckedViewStyleSheet>,
    inline_patches: Vec<CheckedViewStylePatch>,
}

/// One named checked sheet. CSS source is opaque and native inventories are typed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedViewStyleSheet {
    id: ViewStyleSheetId,
    syntax: CheckedViewStyleSyntax,
    tokens: Vec<CheckedViewStyleToken>,
    rules: Vec<CheckedViewStyleRule>,
    css_source: Option<String>,
    css_source_range: Option<TextRange>,
    range: TextRange,
}

/// One sheet-owned checked token.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedViewStyleToken {
    id: ViewStyleTokenId,
    value_kind: ViewStyleValueKind,
    value: ViewSpecifiedValue,
    range: TextRange,
}

/// One checked native selector rule.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedViewStyleRule {
    selector: ViewStyleSelector,
    declarations: Vec<CheckedViewStyleDeclaration>,
    source_order: u32,
    range: TextRange,
}

/// One checked native property assignment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedViewStyleDeclaration {
    property: ViewPropertyKind,
    value: ViewSpecifiedValue,
    append: bool,
    range: TextRange,
}

/// One stable checked inline patch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedViewStylePatch {
    id: ViewStylePatchId,
    syntax: CheckedViewStyleSyntax,
    declarations: Vec<CheckedViewStyleDeclaration>,
    css_source: Option<String>,
    range: TextRange,
}

impl CheckedViewStyleCatalog {
    pub(crate) const fn new(
        sheets: Vec<CheckedViewStyleSheet>,
        inline_patches: Vec<CheckedViewStylePatch>,
    ) -> Self {
        Self {
            sheets,
            inline_patches,
        }
    }

    pub fn sheets(&self) -> &[CheckedViewStyleSheet] {
        &self.sheets
    }

    pub fn inline_patches(&self) -> &[CheckedViewStylePatch] {
        &self.inline_patches
    }

    pub fn sheet(&self, id: &ViewStyleSheetId) -> Option<&CheckedViewStyleSheet> {
        self.sheets.iter().find(|sheet| sheet.id() == id)
    }
}

impl CheckedViewStyleSheet {
    pub(crate) fn new(
        id: ViewStyleSheetId,
        syntax: CheckedViewStyleSyntax,
        tokens: Vec<CheckedViewStyleToken>,
        rules: Vec<CheckedViewStyleRule>,
        css_source: Option<(String, TextRange)>,
        range: TextRange,
    ) -> Self {
        let (css_source, css_source_range) = match css_source {
            Some((source, range)) => (Some(source), Some(range)),
            None => (None, None),
        };
        Self {
            id,
            syntax,
            tokens,
            rules,
            css_source,
            css_source_range,
            range,
        }
    }

    pub const fn id(&self) -> &ViewStyleSheetId {
        &self.id
    }

    pub const fn syntax(&self) -> CheckedViewStyleSyntax {
        self.syntax
    }

    pub fn tokens(&self) -> &[CheckedViewStyleToken] {
        &self.tokens
    }

    pub fn rules(&self) -> &[CheckedViewStyleRule] {
        &self.rules
    }

    pub fn css_source(&self) -> Option<&str> {
        self.css_source.as_deref()
    }

    /// Exact authored body range for an opaque CSS sheet.
    pub const fn css_source_range(&self) -> Option<TextRange> {
        self.css_source_range
    }

    pub const fn range(&self) -> TextRange {
        self.range
    }
}

impl CheckedViewStyleToken {
    pub(crate) const fn new(
        id: ViewStyleTokenId,
        value_kind: ViewStyleValueKind,
        value: ViewSpecifiedValue,
        range: TextRange,
    ) -> Self {
        Self {
            id,
            value_kind,
            value,
            range,
        }
    }

    pub const fn id(&self) -> &ViewStyleTokenId {
        &self.id
    }

    pub const fn value_kind(&self) -> ViewStyleValueKind {
        self.value_kind
    }

    pub const fn value(&self) -> &ViewSpecifiedValue {
        &self.value
    }

    pub const fn range(&self) -> TextRange {
        self.range
    }
}

impl CheckedViewStyleRule {
    pub(crate) const fn new(
        selector: ViewStyleSelector,
        declarations: Vec<CheckedViewStyleDeclaration>,
        source_order: u32,
        range: TextRange,
    ) -> Self {
        Self {
            selector,
            declarations,
            source_order,
            range,
        }
    }

    pub const fn selector(&self) -> &ViewStyleSelector {
        &self.selector
    }

    pub fn declarations(&self) -> &[CheckedViewStyleDeclaration] {
        &self.declarations
    }

    pub const fn source_order(&self) -> u32 {
        self.source_order
    }

    pub const fn range(&self) -> TextRange {
        self.range
    }
}

impl CheckedViewStyleDeclaration {
    pub(crate) const fn new(
        property: ViewPropertyKind,
        value: ViewSpecifiedValue,
        append: bool,
        range: TextRange,
    ) -> Self {
        Self {
            property,
            value,
            append,
            range,
        }
    }

    pub const fn property(&self) -> ViewPropertyKind {
        self.property
    }

    pub const fn value(&self) -> &ViewSpecifiedValue {
        &self.value
    }

    pub const fn is_append(&self) -> bool {
        self.append
    }

    pub const fn range(&self) -> TextRange {
        self.range
    }
}

impl CheckedViewStylePatch {
    pub(crate) const fn new(
        id: ViewStylePatchId,
        syntax: CheckedViewStyleSyntax,
        declarations: Vec<CheckedViewStyleDeclaration>,
        css_source: Option<String>,
        range: TextRange,
    ) -> Self {
        Self {
            id,
            syntax,
            declarations,
            css_source,
            range,
        }
    }

    pub const fn id(&self) -> ViewStylePatchId {
        self.id
    }

    pub const fn syntax(&self) -> CheckedViewStyleSyntax {
        self.syntax
    }

    pub fn declarations(&self) -> &[CheckedViewStyleDeclaration] {
        &self.declarations
    }

    pub fn css_source(&self) -> Option<&str> {
        self.css_source.as_deref()
    }

    pub const fn range(&self) -> TextRange {
        self.range
    }
}
