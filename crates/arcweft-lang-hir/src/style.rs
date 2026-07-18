//! Source-oriented HIR for named and inline View styles.

use arcweft_lang_syntax::{
    ast::{
        common::{TextRange, Visibility},
        ids::EntityRef,
        items::Attribute,
        style::{
            StyleAssignOp, StyleBodyItem, StyleCombinator, StyleDecl, StyleDeclarationDecl,
            StyleEnvironmentBlock, StyleEnvironmentClause, StyleEnvironmentComparisonSyntax,
            StyleEnvironmentFieldSyntax, StyleEnvironmentUnsupportedValueKind,
            StyleEnvironmentValueSyntax, StylePatch, StyleSelector, StyleSelectorSequence,
        },
    },
    expr::Expr,
    types::TypeRef,
};

/// Stable caller-assigned identity for one lowered Style declaration.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirStyleId(u32);

/// Stable caller-assigned identity for one environment wrapper in a Style tree.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirStyleEnvironmentId(u32);

impl HirStyleId {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u32 {
        self.0
    }
}

impl HirStyleEnvironmentId {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u32 {
        self.0
    }
}

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
    body: Vec<HirStyleBodyItem>,
    range: TextRange,
}

/// One ordered rule or environment wrapper in Style HIR.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirStyleBodyItem {
    Rule(HirStyleRuleDecl),
    Environment(HirStyleEnvironmentBlock),
}

/// One retained environment wrapper in Style HIR.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirStyleEnvironmentBlock {
    clauses: Vec<HirStyleEnvironmentClause>,
    body: Vec<HirStyleBodyItem>,
    predicate_range: TextRange,
    body_range: TextRange,
    scope_range: TextRange,
}

/// One retained environment operand triple in Style HIR.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirStyleEnvironmentClause {
    field: HirStyleEnvironmentField,
    comparison: HirStyleEnvironmentComparison,
    value: HirStyleEnvironmentValue,
    ranges: HirStyleEnvironmentClauseRanges,
}

/// Closed environment field, including a typed unknown-field recovery.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirStyleEnvironmentField {
    ColorScheme,
    Contrast,
    ReducedMotion,
    TextScale,
    Recovered { spelling: Box<str> },
}

/// Comparison token retained until field-specific semantic checking.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum HirStyleEnvironmentComparison {
    Equal,
    NotEqual,
    Less,
    LessOrEqual,
    Greater,
    GreaterOrEqual,
    Recovered,
}

/// One source-backed environment operand.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirStyleEnvironmentValue {
    Identifier { spelling: Box<str> },
    Boolean(bool),
    Percentage(HirStyleEnvironmentPercentage),
    Recovered(HirStyleEnvironmentRecovery),
}

/// Authored percentage digits retained without integer accumulation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirStyleEnvironmentPercentage {
    integer_digits: Box<str>,
    fractional_digits: Option<Box<str>>,
}

/// Typed recovery states that can never enter executable semantic output.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirStyleEnvironmentRecovery {
    MissingValue,
    UnsupportedValue(StyleEnvironmentUnsupportedValueKind),
    UnknownField { spelling: Box<str> },
    InvalidComparison,
    InvalidEnumValue { spelling: Box<str> },
    TextScaleOutOfRange,
    DuplicateField,
    DuplicateFieldOnEffectivePath,
}

/// Exact source ranges for all parts of one environment clause.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HirStyleEnvironmentClauseRanges {
    field: TextRange,
    comparison: TextRange,
    value: TextRange,
    clause: TextRange,
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

impl HirStyleDecl {
    /// Lowers one parser-owned Style declaration against its exact source.
    pub fn from_syntax(value: &StyleDecl, source: &str) -> Self {
        let sheet = value.sheet();
        Self {
            attrs: value.attrs().to_vec(),
            visibility: value.visibility(),
            id: value.id().clone(),
            sheet: HirStyleSheet {
                tokens: sheet.tokens().iter().map(HirStyleTokenDecl::from).collect(),
                body: sheet
                    .body()
                    .iter()
                    .map(|item| HirStyleBodyItem::from_syntax(item, source))
                    .collect(),
                range: sheet.range(),
            },
            range: *value.range(),
        }
    }
}

impl HirStyleBodyItem {
    fn from_syntax(value: &StyleBodyItem, source: &str) -> Self {
        match value {
            StyleBodyItem::Rule(rule) => Self::Rule(HirStyleRuleDecl::from(rule)),
            StyleBodyItem::Environment(environment) => {
                Self::Environment(HirStyleEnvironmentBlock::from_syntax(environment, source))
            }
        }
    }
}

impl HirStyleEnvironmentBlock {
    fn from_syntax(value: &StyleEnvironmentBlock, source: &str) -> Self {
        Self {
            clauses: value
                .clauses()
                .iter()
                .map(|clause| HirStyleEnvironmentClause::from_syntax(clause, source))
                .collect(),
            body: value
                .body()
                .iter()
                .map(|item| HirStyleBodyItem::from_syntax(item, source))
                .collect(),
            predicate_range: value.predicate_range(),
            body_range: value.body_range(),
            scope_range: value.scope_range(),
        }
    }
}

impl HirStyleEnvironmentClause {
    fn from_syntax(value: &StyleEnvironmentClause, source: &str) -> Self {
        let ranges = HirStyleEnvironmentClauseRanges {
            field: value.field_range(),
            comparison: value.comparison_range(),
            value: value.value_range(),
            clause: value.range(),
        };
        let field_spelling = source_slice(source, value.field_range());
        let field = match value.field() {
            StyleEnvironmentFieldSyntax::ColorScheme => HirStyleEnvironmentField::ColorScheme,
            StyleEnvironmentFieldSyntax::Contrast => HirStyleEnvironmentField::Contrast,
            StyleEnvironmentFieldSyntax::ReducedMotion => HirStyleEnvironmentField::ReducedMotion,
            StyleEnvironmentFieldSyntax::TextScale => HirStyleEnvironmentField::TextScale,
            StyleEnvironmentFieldSyntax::Unknown => HirStyleEnvironmentField::Recovered {
                spelling: field_spelling.clone(),
            },
        };
        let comparison = match value.comparison() {
            StyleEnvironmentComparisonSyntax::Equal => HirStyleEnvironmentComparison::Equal,
            StyleEnvironmentComparisonSyntax::NotEqual => HirStyleEnvironmentComparison::NotEqual,
            StyleEnvironmentComparisonSyntax::Less => HirStyleEnvironmentComparison::Less,
            StyleEnvironmentComparisonSyntax::LessOrEqual => {
                HirStyleEnvironmentComparison::LessOrEqual
            }
            StyleEnvironmentComparisonSyntax::Greater => HirStyleEnvironmentComparison::Greater,
            StyleEnvironmentComparisonSyntax::GreaterOrEqual => {
                HirStyleEnvironmentComparison::GreaterOrEqual
            }
            StyleEnvironmentComparisonSyntax::Unsupported => {
                HirStyleEnvironmentComparison::Recovered
            }
        };
        let lowered_value = if matches!(value.field(), StyleEnvironmentFieldSyntax::Unknown) {
            HirStyleEnvironmentValue::Recovered(HirStyleEnvironmentRecovery::UnknownField {
                spelling: field_spelling,
            })
        } else if matches!(
            value.comparison(),
            StyleEnvironmentComparisonSyntax::Unsupported
        ) {
            HirStyleEnvironmentValue::Recovered(HirStyleEnvironmentRecovery::InvalidComparison)
        } else {
            match value.value() {
                StyleEnvironmentValueSyntax::Identifier { range } => {
                    HirStyleEnvironmentValue::Identifier {
                        spelling: source_slice(source, *range),
                    }
                }
                StyleEnvironmentValueSyntax::Boolean { value, .. } => {
                    HirStyleEnvironmentValue::Boolean(*value)
                }
                StyleEnvironmentValueSyntax::Percentage(percentage) => {
                    HirStyleEnvironmentValue::Percentage(HirStyleEnvironmentPercentage {
                        integer_digits: source_slice(source, percentage.integer_range()),
                        fractional_digits: percentage
                            .fractional_range()
                            .map(|range| source_slice(source, range)),
                    })
                }
                StyleEnvironmentValueSyntax::Unsupported(unsupported) => {
                    HirStyleEnvironmentValue::Recovered(match unsupported.kind() {
                        StyleEnvironmentUnsupportedValueKind::Missing => {
                            HirStyleEnvironmentRecovery::MissingValue
                        }
                        kind => HirStyleEnvironmentRecovery::UnsupportedValue(kind),
                    })
                }
            }
        };
        Self {
            field,
            comparison,
            value: lowered_value,
            ranges,
        }
    }
}

fn source_slice(source: &str, range: TextRange) -> Box<str> {
    source
        .get(range.as_range())
        .expect("parser-owned Style range belongs to the exact source")
        .into()
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

    pub fn body(&self) -> &[HirStyleBodyItem] {
        &self.body
    }

    pub const fn range(&self) -> TextRange {
        self.range
    }
}

impl HirStyleBodyItem {
    pub const fn as_rule(&self) -> Option<&HirStyleRuleDecl> {
        match self {
            Self::Rule(rule) => Some(rule),
            Self::Environment(_) => None,
        }
    }

    pub const fn as_environment(&self) -> Option<&HirStyleEnvironmentBlock> {
        match self {
            Self::Rule(_) => None,
            Self::Environment(environment) => Some(environment),
        }
    }

    pub const fn range(&self) -> TextRange {
        match self {
            Self::Rule(rule) => rule.range(),
            Self::Environment(environment) => environment.scope_range(),
        }
    }
}

impl HirStyleEnvironmentBlock {
    pub fn clauses(&self) -> &[HirStyleEnvironmentClause] {
        &self.clauses
    }

    pub fn body(&self) -> &[HirStyleBodyItem] {
        &self.body
    }

    /// Exact parenthesized predicate copied from syntax.
    pub const fn predicate_range(&self) -> TextRange {
        self.predicate_range
    }

    /// Exact bytes between this wrapper's braces.
    pub const fn body_range(&self) -> TextRange {
        self.body_range
    }

    /// Complete lexical wrapper range copied from syntax.
    pub const fn scope_range(&self) -> TextRange {
        self.scope_range
    }
}

impl HirStyleEnvironmentClause {
    pub const fn field(&self) -> &HirStyleEnvironmentField {
        &self.field
    }

    pub const fn comparison(&self) -> HirStyleEnvironmentComparison {
        self.comparison
    }

    pub const fn value(&self) -> &HirStyleEnvironmentValue {
        &self.value
    }

    pub const fn ranges(&self) -> HirStyleEnvironmentClauseRanges {
        self.ranges
    }
}

impl HirStyleEnvironmentPercentage {
    pub fn integer_digits(&self) -> &str {
        &self.integer_digits
    }

    pub fn fractional_digits(&self) -> Option<&str> {
        self.fractional_digits.as_deref()
    }
}

impl HirStyleEnvironmentClauseRanges {
    pub const fn field(self) -> TextRange {
        self.field
    }

    pub const fn comparison(self) -> TextRange {
        self.comparison
    }

    pub const fn value(self) -> TextRange {
        self.value
    }

    pub const fn clause(self) -> TextRange {
        self.clause
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
