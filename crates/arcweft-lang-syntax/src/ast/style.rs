//! Syntax tree for Arcweft `style` declarations and inline style patches.
//!
//! Style values use the ordinary expression AST. Arcweft has one native Style
//! language, so the syntax tree does not carry a language discriminator or raw
//! foreign source.

use crate::{expr::Expr, types::TypeRef};

use super::{
    common::{TextRange, Visibility},
    ids::EntityRef,
    items::Attribute,
};

/// One top-level `style` declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StyleDecl {
    attrs: Vec<Attribute>,
    visibility: Option<Visibility>,
    id: EntityRef,
    sheet: StyleSheet,
    range: TextRange,
}

/// Native declarations owned by one named style sheet.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StyleSheet {
    tokens: Vec<StyleTokenDecl>,
    rules: Vec<StyleRuleDecl>,
    range: TextRange,
}

/// A named native style token.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StyleTokenDecl {
    public_id: String,
    value_type: Option<TypeRef>,
    value: StyleExpr,
    range: TextRange,
}

/// One selector rule inside a native style sheet.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StyleRuleDecl {
    selector: StyleSelector,
    declarations: Vec<StyleDeclarationDecl>,
    range: TextRange,
}

/// A structurally valid native selector.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StyleSelector {
    sequences: Vec<StyleSelectorSequence>,
    range: TextRange,
}

/// One compound selector and its relation to the preceding sequence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StyleSelectorSequence {
    relation_to_previous: Option<StyleCombinator>,
    element: Option<StyleName>,
    part: Option<StyleName>,
    predicates: Vec<StylePredicate>,
    range: TextRange,
}

/// Supported native selector combinator.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StyleCombinator {
    Descendant,
    Child,
}

/// A selector predicate whose spelling is resolved by semantic analysis.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StylePredicate {
    name: String,
    range: TextRange,
}

/// A source name and its exact source range.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StyleName {
    text: String,
    range: TextRange,
}

/// One property assignment in a native selector rule or inline patch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StyleDeclarationDecl {
    property: StyleName,
    value: StyleExpr,
    op: StyleAssignOp,
    range: TextRange,
}

/// Assignment operation for a native style declaration.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum StyleAssignOp {
    #[default]
    Replace,
    Append,
}

/// Ordinary Arcweft expression together with authored source provenance.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StyleExpr {
    expr: Expr,
    source: String,
    range: TextRange,
}

/// Inline `.style {}` patch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StylePatch {
    declarations: Vec<StyleDeclarationDecl>,
    range: TextRange,
}

impl StyleDecl {
    pub(crate) const fn new(
        attrs: Vec<Attribute>,
        visibility: Option<Visibility>,
        id: EntityRef,
        sheet: StyleSheet,
        range: TextRange,
    ) -> Self {
        Self {
            attrs,
            visibility,
            id,
            sheet,
            range,
        }
    }

    pub fn attrs(&self) -> &[Attribute] {
        &self.attrs
    }

    pub const fn visibility(&self) -> Option<Visibility> {
        self.visibility
    }

    pub const fn id(&self) -> &EntityRef {
        &self.id
    }

    pub const fn sheet(&self) -> &StyleSheet {
        &self.sheet
    }

    pub const fn range(&self) -> &TextRange {
        &self.range
    }
}

impl StyleSheet {
    pub(crate) const fn new(
        tokens: Vec<StyleTokenDecl>,
        rules: Vec<StyleRuleDecl>,
        range: TextRange,
    ) -> Self {
        Self {
            tokens,
            rules,
            range,
        }
    }

    pub fn tokens(&self) -> &[StyleTokenDecl] {
        &self.tokens
    }

    pub fn rules(&self) -> &[StyleRuleDecl] {
        &self.rules
    }

    pub const fn range(&self) -> TextRange {
        self.range
    }
}

impl StyleTokenDecl {
    pub(crate) fn new(
        public_id: impl Into<String>,
        value_type: Option<TypeRef>,
        value: StyleExpr,
        range: TextRange,
    ) -> Self {
        Self {
            public_id: public_id.into(),
            value_type,
            value,
            range,
        }
    }

    pub fn public_id(&self) -> &str {
        &self.public_id
    }

    pub const fn value_type(&self) -> Option<&TypeRef> {
        self.value_type.as_ref()
    }

    pub const fn value(&self) -> &StyleExpr {
        &self.value
    }

    pub const fn range(&self) -> TextRange {
        self.range
    }
}

impl StyleRuleDecl {
    pub(crate) const fn new(
        selector: StyleSelector,
        declarations: Vec<StyleDeclarationDecl>,
        range: TextRange,
    ) -> Self {
        Self {
            selector,
            declarations,
            range,
        }
    }

    pub const fn selector(&self) -> &StyleSelector {
        &self.selector
    }

    pub fn declarations(&self) -> &[StyleDeclarationDecl] {
        &self.declarations
    }

    pub const fn range(&self) -> TextRange {
        self.range
    }
}

impl StyleSelector {
    pub(crate) const fn new(sequences: Vec<StyleSelectorSequence>, range: TextRange) -> Self {
        Self { sequences, range }
    }

    pub fn sequences(&self) -> &[StyleSelectorSequence] {
        &self.sequences
    }

    pub const fn range(&self) -> TextRange {
        self.range
    }
}

impl StyleSelectorSequence {
    pub(crate) const fn new(
        relation_to_previous: Option<StyleCombinator>,
        element: Option<StyleName>,
        part: Option<StyleName>,
        predicates: Vec<StylePredicate>,
        range: TextRange,
    ) -> Self {
        Self {
            relation_to_previous,
            element,
            part,
            predicates,
            range,
        }
    }

    pub const fn relation_to_previous(&self) -> Option<StyleCombinator> {
        self.relation_to_previous
    }

    pub const fn element(&self) -> Option<&StyleName> {
        self.element.as_ref()
    }

    pub const fn part(&self) -> Option<&StyleName> {
        self.part.as_ref()
    }

    pub fn predicates(&self) -> &[StylePredicate] {
        &self.predicates
    }

    pub const fn range(&self) -> TextRange {
        self.range
    }
}

impl StyleName {
    pub(crate) fn new(text: impl Into<String>, range: TextRange) -> Self {
        Self {
            text: text.into(),
            range,
        }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub const fn range(&self) -> TextRange {
        self.range
    }
}

impl StylePredicate {
    pub(crate) fn new(name: impl Into<String>, range: TextRange) -> Self {
        Self {
            name: name.into(),
            range,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub const fn range(&self) -> TextRange {
        self.range
    }
}

impl StyleDeclarationDecl {
    pub(crate) const fn new(
        property: StyleName,
        value: StyleExpr,
        op: StyleAssignOp,
        range: TextRange,
    ) -> Self {
        Self {
            property,
            value,
            op,
            range,
        }
    }

    pub const fn property(&self) -> &StyleName {
        &self.property
    }

    pub const fn value(&self) -> &StyleExpr {
        &self.value
    }

    pub const fn op(&self) -> StyleAssignOp {
        self.op
    }

    pub const fn range(&self) -> TextRange {
        self.range
    }
}

impl StyleExpr {
    pub(crate) fn new(expr: Expr, source: impl Into<String>, range: TextRange) -> Self {
        Self {
            expr,
            source: source.into(),
            range,
        }
    }

    pub const fn expr(&self) -> &Expr {
        &self.expr
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    pub const fn range(&self) -> TextRange {
        self.range
    }
}

impl StylePatch {
    pub(crate) const fn new(declarations: Vec<StyleDeclarationDecl>, range: TextRange) -> Self {
        Self {
            declarations,
            range,
        }
    }

    pub fn declarations(&self) -> &[StyleDeclarationDecl] {
        &self.declarations
    }

    pub const fn range(&self) -> TextRange {
        self.range
    }
}
