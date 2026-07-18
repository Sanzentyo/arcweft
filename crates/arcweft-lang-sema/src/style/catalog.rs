//! Checked native Style catalog consumed by compiler lowering.

use arcweft_lang_syntax::ast::common::TextRange;
use arcweft_presentation::appearance::{
    ColorScheme, ContrastPreference, PresentationEnvironmentField, TextScaleMilli,
};
use arcweft_view::style::{
    ViewPropertyKind, ViewSpecifiedValue, ViewStylePatchId, ViewStyleSelector, ViewStyleSheetId,
    ViewStyleTokenId, ViewStyleValueKind, ViewTextScaleComparison,
};

/// Complete checked style output for one HIR module.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CheckedViewStyleCatalog {
    sheets: Vec<CheckedViewStyleSheet>,
    inline_patches: Vec<CheckedViewStylePatch>,
}

/// One named checked sheet with typed native inventories.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedViewStyleSheet {
    id: ViewStyleSheetId,
    tokens: Vec<CheckedViewStyleToken>,
    rules: Vec<CheckedViewStyleRule>,
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
    environment: Option<CheckedStyleEnvironmentPath>,
    declarations: Vec<CheckedViewStyleDeclaration>,
    source_order: u32,
    range: TextRange,
}

/// One valid flattened environment path guarding a checked Style rule.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedStyleEnvironmentPath {
    wrappers: Box<[CheckedStyleEnvironmentWrapper]>,
    clauses: Box<[CheckedStyleEnvironmentClause]>,
}

/// Condition-local index of one contributing environment wrapper.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct CheckedStyleEnvironmentWrapperIndex(u8);

/// Exact authored provenance for one checked environment wrapper.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CheckedStyleEnvironmentWrapper {
    predicate: TextRange,
    body: TextRange,
    scope: TextRange,
}

/// One semantically checked environment clause with authored provenance.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CheckedStyleEnvironmentClause {
    ColorScheme {
        value: ColorScheme,
        wrapper: CheckedStyleEnvironmentWrapperIndex,
        range: TextRange,
    },
    Contrast {
        value: ContrastPreference,
        wrapper: CheckedStyleEnvironmentWrapperIndex,
        range: TextRange,
    },
    ReducedMotion {
        value: bool,
        wrapper: CheckedStyleEnvironmentWrapperIndex,
        range: TextRange,
    },
    TextScale {
        comparison: ViewTextScaleComparison,
        value: TextScaleMilli,
        wrapper: CheckedStyleEnvironmentWrapperIndex,
        range: TextRange,
    },
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
    declarations: Vec<CheckedViewStyleDeclaration>,
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
        tokens: Vec<CheckedViewStyleToken>,
        rules: Vec<CheckedViewStyleRule>,
        range: TextRange,
    ) -> Self {
        Self {
            id,
            tokens,
            rules,
            range,
        }
    }

    pub const fn id(&self) -> &ViewStyleSheetId {
        &self.id
    }

    pub fn tokens(&self) -> &[CheckedViewStyleToken] {
        &self.tokens
    }

    pub fn rules(&self) -> &[CheckedViewStyleRule] {
        &self.rules
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
        environment: Option<CheckedStyleEnvironmentPath>,
        declarations: Vec<CheckedViewStyleDeclaration>,
        source_order: u32,
        range: TextRange,
    ) -> Self {
        Self {
            selector,
            environment,
            declarations,
            source_order,
            range,
        }
    }

    pub const fn selector(&self) -> &ViewStyleSelector {
        &self.selector
    }

    pub const fn environment(&self) -> Option<&CheckedStyleEnvironmentPath> {
        self.environment.as_ref()
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

impl CheckedStyleEnvironmentPath {
    pub(crate) const fn new(
        wrappers: Box<[CheckedStyleEnvironmentWrapper]>,
        clauses: Box<[CheckedStyleEnvironmentClause]>,
    ) -> Self {
        Self { wrappers, clauses }
    }

    pub fn wrappers(&self) -> &[CheckedStyleEnvironmentWrapper] {
        &self.wrappers
    }

    pub fn clauses(&self) -> &[CheckedStyleEnvironmentClause] {
        &self.clauses
    }
}

impl CheckedStyleEnvironmentWrapperIndex {
    pub(crate) fn try_from_index(index: usize) -> Option<Self> {
        u8::try_from(index).ok().map(Self)
    }

    pub const fn value(self) -> u8 {
        self.0
    }

    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

impl CheckedStyleEnvironmentWrapper {
    pub(crate) const fn new(
        predicate_range: TextRange,
        body_range: TextRange,
        scope_range: TextRange,
    ) -> Self {
        Self {
            predicate: predicate_range,
            body: body_range,
            scope: scope_range,
        }
    }

    pub const fn predicate_range(self) -> TextRange {
        self.predicate
    }

    pub const fn body_range(self) -> TextRange {
        self.body
    }

    pub const fn scope_range(self) -> TextRange {
        self.scope
    }
}

impl CheckedStyleEnvironmentClause {
    pub const fn field(self) -> PresentationEnvironmentField {
        match self {
            Self::ColorScheme { .. } => PresentationEnvironmentField::ColorScheme,
            Self::Contrast { .. } => PresentationEnvironmentField::Contrast,
            Self::ReducedMotion { .. } => PresentationEnvironmentField::ReducedMotion,
            Self::TextScale { .. } => PresentationEnvironmentField::TextScale,
        }
    }

    pub const fn range(self) -> TextRange {
        match self {
            Self::ColorScheme { range, .. }
            | Self::Contrast { range, .. }
            | Self::ReducedMotion { range, .. }
            | Self::TextScale { range, .. } => range,
        }
    }

    pub const fn wrapper(self) -> CheckedStyleEnvironmentWrapperIndex {
        match self {
            Self::ColorScheme { wrapper, .. }
            | Self::Contrast { wrapper, .. }
            | Self::ReducedMotion { wrapper, .. }
            | Self::TextScale { wrapper, .. } => wrapper,
        }
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
        declarations: Vec<CheckedViewStyleDeclaration>,
        range: TextRange,
    ) -> Self {
        Self {
            id,
            declarations,
            range,
        }
    }

    pub const fn id(&self) -> ViewStylePatchId {
        self.id
    }

    pub fn declarations(&self) -> &[CheckedViewStyleDeclaration] {
        &self.declarations
    }

    pub const fn range(&self) -> TextRange {
        self.range
    }
}
