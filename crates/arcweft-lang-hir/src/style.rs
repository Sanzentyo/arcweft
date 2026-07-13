//! Source-oriented HIR for named and inline View styles.

use arcweft_lang_syntax::{
    ast::{
        common::{TextRange, Visibility},
        ids::EntityRef,
        items::Attribute,
        style::{
            StyleAssignOp, StyleCombinator, StyleDecl, StyleDeclarationDecl, StylePatch,
            StyleSelector, StyleSelectorSequence,
        },
    },
    expr::Expr,
    types::TypeRef,
};

/// One lowered top-level style declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirStyleDecl {
    attrs: Vec<Attribute>,
    visibility: Option<Visibility>,
    id: EntityRef,
    sheet: HirStyleSheet,
    range: TextRange,
}

/// One lowered named native sheet.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirStyleSheet {
    tokens: Vec<HirStyleTokenDecl>,
    rules: Vec<HirStyleRuleDecl>,
    range: TextRange,
}

/// One lowered native token declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirStyleTokenDecl {
    public_id: String,
    value_type: Option<TypeRef>,
    value: HirStyleExpr,
    range: TextRange,
}

/// One lowered native selector rule.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirStyleRuleDecl {
    selector: HirStyleSelector,
    declarations: Vec<HirStyleDeclaration>,
    range: TextRange,
}

/// Structurally validated selector HIR.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirStyleSelector {
    sequences: Vec<HirStyleSelectorSequence>,
    range: TextRange,
}

/// One compound selector HIR sequence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirStyleSelectorSequence {
    relation_to_previous: Option<HirStyleCombinator>,
    element: Option<HirStyleName>,
    part: Option<HirStyleName>,
    predicates: Vec<HirStyleName>,
    range: TextRange,
}

/// Supported native selector combinator in HIR.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirStyleCombinator {
    Descendant,
    Child,
}

/// Name plus source range retained for semantic lookup.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirStyleName {
    text: String,
    range: TextRange,
}

/// One property assignment after syntax lowering.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirStyleDeclaration {
    property: HirStyleName,
    value: HirStyleExpr,
    op: HirStyleAssignOp,
    range: TextRange,
}

/// Assignment operation in HIR.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirStyleAssignOp {
    Replace,
    Append,
}

/// Ordinary expression HIR plus authored provenance.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirStyleExpr {
    expr: Expr,
    source: String,
    range: TextRange,
}

/// One stable inline patch extracted while lowering a View declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirStylePatch {
    ordinal: u32,
    declarations: Vec<HirStyleDeclaration>,
    range: TextRange,
}

impl From<&StyleDecl> for HirStyleDecl {
    fn from(value: &StyleDecl) -> Self {
        let sheet = value.sheet();
        Self {
            attrs: value.attrs().to_vec(),
            visibility: value.visibility(),
            id: value.id().clone(),
            sheet: HirStyleSheet {
                tokens: sheet.tokens().iter().map(HirStyleTokenDecl::from).collect(),
                rules: sheet.rules().iter().map(HirStyleRuleDecl::from).collect(),
                range: sheet.range(),
            },
            range: *value.range(),
        }
    }
}

impl From<&arcweft_lang_syntax::ast::style::StyleTokenDecl> for HirStyleTokenDecl {
    fn from(value: &arcweft_lang_syntax::ast::style::StyleTokenDecl) -> Self {
        Self {
            public_id: value.public_id().to_owned(),
            value_type: value.value_type().cloned(),
            value: HirStyleExpr::from(value.value()),
            range: value.range(),
        }
    }
}

impl From<&arcweft_lang_syntax::ast::style::StyleRuleDecl> for HirStyleRuleDecl {
    fn from(value: &arcweft_lang_syntax::ast::style::StyleRuleDecl) -> Self {
        Self {
            selector: HirStyleSelector::from(value.selector()),
            declarations: value
                .declarations()
                .iter()
                .map(HirStyleDeclaration::from)
                .collect(),
            range: value.range(),
        }
    }
}

impl From<&StyleSelector> for HirStyleSelector {
    fn from(value: &StyleSelector) -> Self {
        Self {
            sequences: value
                .sequences()
                .iter()
                .map(HirStyleSelectorSequence::from)
                .collect(),
            range: value.range(),
        }
    }
}

impl From<&StyleSelectorSequence> for HirStyleSelectorSequence {
    fn from(value: &StyleSelectorSequence) -> Self {
        Self {
            relation_to_previous: value.relation_to_previous().map(HirStyleCombinator::from),
            element: value.element().map(HirStyleName::from),
            part: value.part().map(HirStyleName::from),
            predicates: value
                .predicates()
                .iter()
                .map(|predicate| HirStyleName {
                    text: predicate.name().to_owned(),
                    range: predicate.range(),
                })
                .collect(),
            range: value.range(),
        }
    }
}

impl From<StyleCombinator> for HirStyleCombinator {
    fn from(value: StyleCombinator) -> Self {
        match value {
            StyleCombinator::Descendant => Self::Descendant,
            StyleCombinator::Child => Self::Child,
        }
    }
}

impl From<&arcweft_lang_syntax::ast::style::StyleName> for HirStyleName {
    fn from(value: &arcweft_lang_syntax::ast::style::StyleName) -> Self {
        Self {
            text: value.text().to_owned(),
            range: value.range(),
        }
    }
}

impl From<&StyleDeclarationDecl> for HirStyleDeclaration {
    fn from(value: &StyleDeclarationDecl) -> Self {
        Self {
            property: HirStyleName::from(value.property()),
            value: HirStyleExpr::from(value.value()),
            op: HirStyleAssignOp::from(value.op()),
            range: value.range(),
        }
    }
}

impl From<StyleAssignOp> for HirStyleAssignOp {
    fn from(value: StyleAssignOp) -> Self {
        match value {
            StyleAssignOp::Replace => Self::Replace,
            StyleAssignOp::Append => Self::Append,
        }
    }
}

impl From<&arcweft_lang_syntax::ast::style::StyleExpr> for HirStyleExpr {
    fn from(value: &arcweft_lang_syntax::ast::style::StyleExpr) -> Self {
        Self {
            expr: value.expr().clone(),
            source: value.source().to_owned(),
            range: value.range(),
        }
    }
}

impl HirStylePatch {
    /// Lowers an inline patch with its stable source-order ordinal.
    pub fn from_syntax(ordinal: u32, patch: &StylePatch) -> Self {
        Self {
            ordinal,
            declarations: patch
                .declarations()
                .iter()
                .map(HirStyleDeclaration::from)
                .collect(),
            range: patch.range(),
        }
    }

    pub const fn ordinal(&self) -> u32 {
        self.ordinal
    }

    /// Rebases this module-local ordinal into a linked-module ordinal space.
    ///
    /// Inline patch ordinals are the deterministic identity seed consumed by
    /// later semantic and compiler layers. Rebasing changes only that identity;
    /// authored declarations and source ranges remain untouched.
    pub(crate) fn rebase_ordinal(&mut self, base: u32) {
        self.ordinal = self
            .ordinal
            .checked_add(base)
            .expect("linked HIR contains more inline style patches than u32 can identify");
    }

    pub fn declarations(&self) -> &[HirStyleDeclaration] {
        &self.declarations
    }

    pub const fn range(&self) -> TextRange {
        self.range
    }
}

impl HirStyleDecl {
    pub fn attrs(&self) -> &[Attribute] {
        &self.attrs
    }

    pub const fn visibility(&self) -> Option<Visibility> {
        self.visibility
    }

    pub const fn id(&self) -> &EntityRef {
        &self.id
    }

    pub const fn sheet(&self) -> &HirStyleSheet {
        &self.sheet
    }

    pub const fn range(&self) -> TextRange {
        self.range
    }
}

impl HirStyleSheet {
    pub fn tokens(&self) -> &[HirStyleTokenDecl] {
        &self.tokens
    }

    pub fn rules(&self) -> &[HirStyleRuleDecl] {
        &self.rules
    }

    pub const fn range(&self) -> TextRange {
        self.range
    }
}

impl HirStyleTokenDecl {
    pub fn public_id(&self) -> &str {
        &self.public_id
    }

    pub const fn value_type(&self) -> Option<&TypeRef> {
        self.value_type.as_ref()
    }

    pub const fn value(&self) -> &HirStyleExpr {
        &self.value
    }

    pub const fn range(&self) -> TextRange {
        self.range
    }
}

impl HirStyleRuleDecl {
    pub const fn selector(&self) -> &HirStyleSelector {
        &self.selector
    }

    pub fn declarations(&self) -> &[HirStyleDeclaration] {
        &self.declarations
    }

    pub const fn range(&self) -> TextRange {
        self.range
    }
}

impl HirStyleSelector {
    pub fn sequences(&self) -> &[HirStyleSelectorSequence] {
        &self.sequences
    }

    pub const fn range(&self) -> TextRange {
        self.range
    }
}

impl HirStyleSelectorSequence {
    pub const fn relation_to_previous(&self) -> Option<HirStyleCombinator> {
        self.relation_to_previous
    }

    pub const fn element(&self) -> Option<&HirStyleName> {
        self.element.as_ref()
    }

    pub const fn part(&self) -> Option<&HirStyleName> {
        self.part.as_ref()
    }

    pub fn predicates(&self) -> &[HirStyleName] {
        &self.predicates
    }

    pub const fn range(&self) -> TextRange {
        self.range
    }
}

impl HirStyleName {
    pub fn text(&self) -> &str {
        &self.text
    }

    pub const fn range(&self) -> TextRange {
        self.range
    }
}

impl HirStyleDeclaration {
    pub const fn property(&self) -> &HirStyleName {
        &self.property
    }

    pub const fn value(&self) -> &HirStyleExpr {
        &self.value
    }

    pub const fn op(&self) -> HirStyleAssignOp {
        self.op
    }

    pub const fn range(&self) -> TextRange {
        self.range
    }
}

impl HirStyleExpr {
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

#[cfg(test)]
mod tests {
    use crate::lower::lower_to_hir;
    use arcweft_lang_syntax::parser::parse_source;

    #[test]
    fn patch_rebase_changes_only_ordinal_identity() {
        let hir = lower_to_hir(
            &parse_source(
                r#"pub view Example() {
    Button("OK").style { outline-width = 2px }
}
"#,
            )
            .into_typed_tree(),
        )
        .unwrap();
        let mut patch = hir.style_patches()[0].clone();
        let original_declarations = patch.declarations().to_vec();
        let original_range = patch.range();

        patch.rebase_ordinal(7);

        assert_eq!(patch.ordinal(), 7);
        assert_eq!(patch.declarations(), original_declarations);
        assert_eq!(patch.range(), original_range);
    }
}
