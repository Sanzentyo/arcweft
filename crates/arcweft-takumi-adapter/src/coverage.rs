//! Deterministic CSS layout/cascade coverage data for the retained UI path.
//!
//! This module is intentionally a coverage/evidence layer around Takumi lowering:
//! Takumi remains the CSS cascade/layout/stacking source, and Arcweft still lowers
//! the resulting scene into renderer-owned `UiScene` data. The types here define
//! which CSS forms are considered production-supported, represented for product
//! data only, accepted with diagnostics, or intentionally rejected for the first
//! seq06.12 cut.

use crate::diagnostic::TakumiDiagnostic;
use crate::style::{CssInvalidationClass, CssPropertyClass};
use std::cmp::Ordering;
use std::collections::BTreeSet;

pub const CSS_LAYOUT_CASCADE_EVIDENCE_SCHEMA_VERSION: &str =
    "arcweft.css-layout-cascade-coverage.v1";

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CssCoverageFeature {
    ElementSelector,
    ClassSelector,
    IdSelector,
    PartAttributeSelector,
    DescendantCombinator,
    ChildCombinator,
    InteractionPseudoState,
    PseudoElement,
    StructuralSelector,
    ArcweftLayer,
    CssLayer,
    Specificity,
    SourceOrder,
    Inheritance,
    Important,
    CustomPropertyToken,
    UnresolvedVariable,
    BlockLayout,
    InlineLayout,
    FlexLayout,
    GridLayout,
    Margin,
    Padding,
    Gap,
    WidthHeight,
    MinMaxSize,
    AspectRatio,
    PositionInset,
    ZIndex,
    OverflowClip,
    ColorSchemeQuery,
    ContrastQuery,
    ReducedMotionQuery,
    TextScaleQuery,
    ViewportMediaQuery,
    ContainerQuery,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CssCoverageStatus {
    SupportedNow,
    ProductDataOnly,
    StructuredDiagnostic,
    IntentionallyRejected,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CssCoverageMatrixRow {
    feature: CssCoverageFeature,
    status: CssCoverageStatus,
    detail: &'static str,
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum CssCascadeLayer {
    ArcweftBase,
    ArcweftComponent,
    CssReset,
    #[default]
    CssBase,
    CssComponent,
    CssInline,
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct CssSpecificity {
    ids: u16,
    classes: u16,
    elements: u16,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct CssCascadePriority {
    important: bool,
    layer: CssCascadeLayer,
    specificity: CssSpecificity,
    source_order: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CssMatchedDeclaration {
    selector: String,
    property: String,
    value: String,
    priority: CssCascadePriority,
    invalidation: CssInvalidationClass,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CssSelectorCoverage {
    selector: String,
    specificity: CssSpecificity,
    diagnostics: Vec<TakumiDiagnostic>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CssDeclarationCoverage {
    selector: String,
    property: String,
    value: String,
    invalidation: CssInvalidationClass,
    status: CssCoverageStatus,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CssAtRuleCoverage {
    rule: String,
    prelude: String,
    status: CssCoverageStatus,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CssCoverageReport {
    diagnostics: Vec<TakumiDiagnostic>,
    declarations: Vec<CssDeclarationCoverage>,
    at_rules: Vec<CssAtRuleCoverage>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CssSelectorWinnerEvidence {
    selector: String,
    priority: CssCascadePriority,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CssComputedStyleEvidence {
    node_path: String,
    property: String,
    value: String,
    winner: CssSelectorWinnerEvidence,
    invalidation: CssInvalidationClass,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CssOverflowEvidence {
    Visible,
    Clip,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CssLayoutBoxEvidence {
    node_path: String,
    x_milli: i32,
    y_milli: i32,
    width_milli: i32,
    height_milli: i32,
    overflow: CssOverflowEvidence,
}

pub const CSS_COVERAGE_MATRIX: &[CssCoverageMatrixRow] = &[
    CssCoverageMatrixRow::new(
        CssCoverageFeature::ElementSelector,
        CssCoverageStatus::SupportedNow,
        "Takumi tag names emitted by the adapter (`div`, `span`, `img`) are selector inputs.",
    ),
    CssCoverageMatrixRow::new(
        CssCoverageFeature::ClassSelector,
        CssCoverageStatus::SupportedNow,
        "Adapter emits stable Arcweft class names such as `aw-container` and `aw-text`.",
    ),
    CssCoverageMatrixRow::new(
        CssCoverageFeature::IdSelector,
        CssCoverageStatus::SupportedNow,
        "Adapter emits `aw-node-{NodeId}` ids for retained nodes.",
    ),
    CssCoverageMatrixRow::new(
        CssCoverageFeature::PartAttributeSelector,
        CssCoverageStatus::SupportedNow,
        "Part selection is supported through the Arcweft-owned `data-aw-part` attribute, not CSS pseudo-elements.",
    ),
    CssCoverageMatrixRow::new(
        CssCoverageFeature::DescendantCombinator,
        CssCoverageStatus::SupportedNow,
        "Takumi computes ordinary descendant selector matching before Arcweft lowering.",
    ),
    CssCoverageMatrixRow::new(
        CssCoverageFeature::ChildCombinator,
        CssCoverageStatus::SupportedNow,
        "Takumi computes child selector matching before Arcweft lowering.",
    ),
    CssCoverageMatrixRow::new(
        CssCoverageFeature::InteractionPseudoState,
        CssCoverageStatus::ProductDataOnly,
        "`:hover`, `:focus`, `:active`, and `:disabled` map to retained interaction state; frame-path binding remains gated on seq06.11 state input.",
    ),
    CssCoverageMatrixRow::new(
        CssCoverageFeature::PseudoElement,
        CssCoverageStatus::IntentionallyRejected,
        "`::before`, `::after`, `::part`, and other pseudo-elements would synthesize nodes and are not in this retained-UI cut.",
    ),
    CssCoverageMatrixRow::new(
        CssCoverageFeature::StructuralSelector,
        CssCoverageStatus::StructuredDiagnostic,
        "Structural selectors such as `:nth-child` are rejected with diagnostics instead of approximated.",
    ),
    CssCoverageMatrixRow::new(
        CssCoverageFeature::ArcweftLayer,
        CssCoverageStatus::SupportedNow,
        "Arcweft base/component style layers are ordered before CSS author layers.",
    ),
    CssCoverageMatrixRow::new(
        CssCoverageFeature::CssLayer,
        CssCoverageStatus::SupportedNow,
        "CSS reset/base/component/inline layer order is represented in coverage evidence and tests.",
    ),
    CssCoverageMatrixRow::new(
        CssCoverageFeature::Specificity,
        CssCoverageStatus::SupportedNow,
        "Specificity is stored as `(id, class-or-attribute-or-pseudo, element)` and compared lexicographically.",
    ),
    CssCoverageMatrixRow::new(
        CssCoverageFeature::SourceOrder,
        CssCoverageStatus::SupportedNow,
        "Later source order wins after origin/layer/specificity ties.",
    ),
    CssCoverageMatrixRow::new(
        CssCoverageFeature::Inheritance,
        CssCoverageStatus::ProductDataOnly,
        "Inherited field names are documented for provenance snapshots; full inherited-value serialization is a follow-up.",
    ),
    CssCoverageMatrixRow::new(
        CssCoverageFeature::Important,
        CssCoverageStatus::SupportedNow,
        "`!important` participates in deterministic priority sorting but does not create a browser-only fallback path.",
    ),
    CssCoverageMatrixRow::new(
        CssCoverageFeature::CustomPropertyToken,
        CssCoverageStatus::ProductDataOnly,
        "CSS custom properties lower to Arcweft style token provenance; unsupported references stay diagnostic-driven.",
    ),
    CssCoverageMatrixRow::new(
        CssCoverageFeature::UnresolvedVariable,
        CssCoverageStatus::StructuredDiagnostic,
        "Unresolved `var(--name)` without fallback emits `UnresolvedCssVariable`.",
    ),
    CssCoverageMatrixRow::new(
        CssCoverageFeature::BlockLayout,
        CssCoverageStatus::SupportedNow,
        "Takumi block layout feeds the direct `UiScene` bounds.",
    ),
    CssCoverageMatrixRow::new(
        CssCoverageFeature::InlineLayout,
        CssCoverageStatus::ProductDataOnly,
        "Inline retained nodes are represented, while seq06.10a styled paragraph layout remains the text substrate.",
    ),
    CssCoverageMatrixRow::new(
        CssCoverageFeature::FlexLayout,
        CssCoverageStatus::SupportedNow,
        "Flex row/column, wrapping, alignment, and gap are first-cut layout inputs.",
    ),
    CssCoverageMatrixRow::new(
        CssCoverageFeature::GridLayout,
        CssCoverageStatus::StructuredDiagnostic,
        "Grid declarations are accepted by data ingestion but reported as out of scope for direct retained UI evidence.",
    ),
    CssCoverageMatrixRow::new(
        CssCoverageFeature::Margin,
        CssCoverageStatus::SupportedNow,
        "Margin and side-specific margin properties are layout invalidations.",
    ),
    CssCoverageMatrixRow::new(
        CssCoverageFeature::Padding,
        CssCoverageStatus::SupportedNow,
        "Padding and side-specific padding properties are layout invalidations.",
    ),
    CssCoverageMatrixRow::new(
        CssCoverageFeature::Gap,
        CssCoverageStatus::SupportedNow,
        "`gap`, `row-gap`, and `column-gap` are flex layout inputs.",
    ),
    CssCoverageMatrixRow::new(
        CssCoverageFeature::WidthHeight,
        CssCoverageStatus::SupportedNow,
        "Width and height feed deterministic layout boxes.",
    ),
    CssCoverageMatrixRow::new(
        CssCoverageFeature::MinMaxSize,
        CssCoverageStatus::SupportedNow,
        "Minimum and maximum sizes are layout invalidations and Takumi layout inputs.",
    ),
    CssCoverageMatrixRow::new(
        CssCoverageFeature::AspectRatio,
        CssCoverageStatus::SupportedNow,
        "`aspect-ratio` is included in the first-cut layout subset.",
    ),
    CssCoverageMatrixRow::new(
        CssCoverageFeature::PositionInset,
        CssCoverageStatus::SupportedNow,
        "`position` plus inset edges are layout/stacking inputs.",
    ),
    CssCoverageMatrixRow::new(
        CssCoverageFeature::ZIndex,
        CssCoverageStatus::SupportedNow,
        "`z-index` is a stacking/layout scene invalidation.",
    ),
    CssCoverageMatrixRow::new(
        CssCoverageFeature::OverflowClip,
        CssCoverageStatus::SupportedNow,
        "`overflow: hidden|clip` lowers to clipping evidence where Takumi exposes bounds.",
    ),
    CssCoverageMatrixRow::new(
        CssCoverageFeature::ColorSchemeQuery,
        CssCoverageStatus::ProductDataOnly,
        "Color scheme exists in `PresentationEnvironment`; CSS `@media` binding reports a coverage diagnostic in this cut.",
    ),
    CssCoverageMatrixRow::new(
        CssCoverageFeature::ContrastQuery,
        CssCoverageStatus::ProductDataOnly,
        "Contrast preference exists in `PresentationEnvironment`; CSS `@media` binding reports a coverage diagnostic in this cut.",
    ),
    CssCoverageMatrixRow::new(
        CssCoverageFeature::ReducedMotionQuery,
        CssCoverageStatus::ProductDataOnly,
        "Reduced motion exists in `PresentationEnvironment`; animation features remain seq06.13 non-goals.",
    ),
    CssCoverageMatrixRow::new(
        CssCoverageFeature::TextScaleQuery,
        CssCoverageStatus::ProductDataOnly,
        "Text scale exists in `PresentationEnvironment`; CSS query syntax is not standardized for this cut.",
    ),
    CssCoverageMatrixRow::new(
        CssCoverageFeature::ViewportMediaQuery,
        CssCoverageStatus::StructuredDiagnostic,
        "Viewport media queries are diagnosed until retained UI viewport policy is fixed.",
    ),
    CssCoverageMatrixRow::new(
        CssCoverageFeature::ContainerQuery,
        CssCoverageStatus::IntentionallyRejected,
        "Container queries require query containers and invalidation edges not implemented in this cut.",
    ),
];

impl CssCoverageMatrixRow {
    pub const fn new(
        feature: CssCoverageFeature,
        status: CssCoverageStatus,
        detail: &'static str,
    ) -> Self {
        Self {
            feature,
            status,
            detail,
        }
    }

    pub const fn feature(self) -> CssCoverageFeature {
        self.feature
    }

    pub const fn status(self) -> CssCoverageStatus {
        self.status
    }

    pub const fn detail(self) -> &'static str {
        self.detail
    }
}

impl CssCascadeLayer {
    pub const fn order(self) -> u16 {
        match self {
            Self::ArcweftBase => 10,
            Self::ArcweftComponent => 20,
            Self::CssReset => 30,
            Self::CssBase => 40,
            Self::CssComponent => 50,
            Self::CssInline => 60,
        }
    }
}

impl Ord for CssCascadeLayer {
    fn cmp(&self, other: &Self) -> Ordering {
        self.order().cmp(&other.order())
    }
}

impl PartialOrd for CssCascadeLayer {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl CssSpecificity {
    pub const ZERO: Self = Self::new(0, 0, 0);

    pub const fn new(ids: u16, classes: u16, elements: u16) -> Self {
        Self {
            ids,
            classes,
            elements,
        }
    }

    pub const fn ids(self) -> u16 {
        self.ids
    }

    pub const fn classes(self) -> u16 {
        self.classes
    }

    pub const fn elements(self) -> u16 {
        self.elements
    }

    pub fn add_id(&mut self) {
        self.ids = self.ids.saturating_add(1);
    }

    pub fn add_class_like(&mut self) {
        self.classes = self.classes.saturating_add(1);
    }

    pub fn add_element(&mut self) {
        self.elements = self.elements.saturating_add(1);
    }
}

impl Ord for CssSpecificity {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.ids, self.classes, self.elements).cmp(&(other.ids, other.classes, other.elements))
    }
}

impl PartialOrd for CssSpecificity {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl CssCascadePriority {
    pub const fn new(
        layer: CssCascadeLayer,
        specificity: CssSpecificity,
        source_order: u32,
    ) -> Self {
        Self {
            important: false,
            layer,
            specificity,
            source_order,
        }
    }

    #[must_use]
    pub const fn important(mut self) -> Self {
        self.important = true;
        self
    }

    pub const fn is_important(self) -> bool {
        self.important
    }

    pub const fn layer(self) -> CssCascadeLayer {
        self.layer
    }

    pub const fn specificity(self) -> CssSpecificity {
        self.specificity
    }

    pub const fn source_order(self) -> u32 {
        self.source_order
    }

    pub fn is_stronger_than(self, other: Self) -> bool {
        self > other
    }
}

impl Ord for CssCascadePriority {
    fn cmp(&self, other: &Self) -> Ordering {
        (
            self.important,
            self.layer,
            self.specificity,
            self.source_order,
        )
            .cmp(&(
                other.important,
                other.layer,
                other.specificity,
                other.source_order,
            ))
    }
}

impl PartialOrd for CssCascadePriority {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl CssMatchedDeclaration {
    pub fn new(
        selector: impl Into<String>,
        property: impl Into<String>,
        value: impl Into<String>,
        priority: CssCascadePriority,
    ) -> Self {
        let property = property.into();
        Self {
            selector: selector.into(),
            invalidation: CssPropertyClass::classify(&property).invalidation(),
            property,
            value: value.into(),
            priority,
        }
    }

    pub fn selector(&self) -> &str {
        &self.selector
    }

    pub fn property(&self) -> &str {
        &self.property
    }

    pub fn value(&self) -> &str {
        &self.value
    }

    pub const fn priority(&self) -> CssCascadePriority {
        self.priority
    }

    pub const fn invalidation(&self) -> CssInvalidationClass {
        self.invalidation
    }
}

impl CssSelectorCoverage {
    pub fn analyze(selector: &str) -> Self {
        let mut scanner = SelectorScanner::new(selector);
        scanner.scan();
        Self {
            selector: selector.trim().to_owned(),
            specificity: scanner.specificity,
            diagnostics: scanner.diagnostics,
        }
    }

    pub fn selector(&self) -> &str {
        &self.selector
    }

    pub const fn specificity(&self) -> CssSpecificity {
        self.specificity
    }

    pub fn diagnostics(&self) -> &[TakumiDiagnostic] {
        &self.diagnostics
    }

    pub fn is_supported(&self) -> bool {
        self.diagnostics.is_empty()
    }
}

impl CssDeclarationCoverage {
    pub fn selector(&self) -> &str {
        &self.selector
    }

    pub fn property(&self) -> &str {
        &self.property
    }

    pub fn value(&self) -> &str {
        &self.value
    }

    pub const fn invalidation(&self) -> CssInvalidationClass {
        self.invalidation
    }

    pub const fn status(&self) -> CssCoverageStatus {
        self.status
    }
}

impl CssAtRuleCoverage {
    pub fn rule(&self) -> &str {
        &self.rule
    }

    pub fn prelude(&self) -> &str {
        &self.prelude
    }

    pub const fn status(&self) -> CssCoverageStatus {
        self.status
    }
}

impl CssCoverageReport {
    pub fn analyze_css(css: &str) -> Self {
        let custom_properties = declared_custom_properties(css);
        let mut report = Self::default();
        report.collect_at_rules(css);

        for rule in css_rules(css) {
            let selector_list = rule.selector.trim();
            if selector_list.starts_with('@') {
                continue;
            }
            for selector in selector_list
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
                let selector_coverage = CssSelectorCoverage::analyze(selector);
                report
                    .diagnostics
                    .extend(selector_coverage.diagnostics().iter().cloned());
                for (property, value) in css_declarations(rule.body) {
                    report.record_declaration(selector, property, value, &custom_properties);
                }
            }
        }

        report
    }

    pub fn extend(&mut self, other: Self) {
        self.diagnostics.extend(other.diagnostics);
        self.declarations.extend(other.declarations);
        self.at_rules.extend(other.at_rules);
    }

    pub fn diagnostics(&self) -> &[TakumiDiagnostic] {
        &self.diagnostics
    }

    pub fn declarations(&self) -> &[CssDeclarationCoverage] {
        &self.declarations
    }

    pub fn at_rules(&self) -> &[CssAtRuleCoverage] {
        &self.at_rules
    }

    pub fn is_direct_wgpu_ready(&self) -> bool {
        self.diagnostics.is_empty()
    }

    fn collect_at_rules(&mut self, css: &str) {
        for at_rule in at_rule_preludes(css) {
            let status = at_rule_status(&at_rule.rule, &at_rule.prelude);
            if status != CssCoverageStatus::SupportedNow {
                let feature = format!("@{} {}", at_rule.rule, at_rule.prelude);
                self.diagnostics.push(TakumiDiagnostic::css_coverage_gap(
                    feature.trim().to_owned(),
                    status,
                ));
            }
            self.at_rules.push(CssAtRuleCoverage {
                rule: at_rule.rule,
                prelude: at_rule.prelude,
                status,
            });
        }
    }

    fn record_declaration(
        &mut self,
        selector: &str,
        property: &str,
        value: &str,
        custom_properties: &BTreeSet<String>,
    ) {
        let property = normalize(property);
        let value = value.trim().to_owned();
        let status = declaration_status(&property, &value);
        if property.starts_with("--") {
            self.declarations.push(CssDeclarationCoverage {
                selector: selector.to_owned(),
                property,
                value,
                invalidation: CssInvalidationClass::PaintOnly,
                status: CssCoverageStatus::ProductDataOnly,
            });
            return;
        }

        for variable in unresolved_variables(&value, custom_properties) {
            self.diagnostics
                .push(TakumiDiagnostic::unresolved_css_variable(variable));
        }
        if status != CssCoverageStatus::SupportedNow
            && !matches!(status, CssCoverageStatus::ProductDataOnly)
        {
            self.diagnostics.push(TakumiDiagnostic::css_coverage_gap(
                format!("{property}: {value}"),
                status,
            ));
        }
        self.declarations.push(CssDeclarationCoverage {
            selector: selector.to_owned(),
            invalidation: CssPropertyClass::classify(&property).invalidation(),
            property,
            value,
            status,
        });
    }
}

impl CssSelectorWinnerEvidence {
    pub fn new(selector: impl Into<String>, priority: CssCascadePriority) -> Self {
        Self {
            selector: selector.into(),
            priority,
        }
    }

    pub fn selector(&self) -> &str {
        &self.selector
    }

    pub const fn priority(&self) -> CssCascadePriority {
        self.priority
    }
}

impl CssComputedStyleEvidence {
    pub fn new(
        node_path: impl Into<String>,
        property: impl Into<String>,
        value: impl Into<String>,
        winner: CssSelectorWinnerEvidence,
        invalidation: CssInvalidationClass,
    ) -> Self {
        Self {
            node_path: node_path.into(),
            property: property.into(),
            value: value.into(),
            winner,
            invalidation,
        }
    }

    pub fn node_path(&self) -> &str {
        &self.node_path
    }

    pub fn property(&self) -> &str {
        &self.property
    }

    pub fn value(&self) -> &str {
        &self.value
    }

    pub fn winner(&self) -> &CssSelectorWinnerEvidence {
        &self.winner
    }

    pub const fn invalidation(&self) -> CssInvalidationClass {
        self.invalidation
    }
}

impl CssLayoutBoxEvidence {
    pub fn new(
        node_path: impl Into<String>,
        x_milli: i32,
        y_milli: i32,
        width_milli: i32,
        height_milli: i32,
        overflow: CssOverflowEvidence,
    ) -> Self {
        Self {
            node_path: node_path.into(),
            x_milli,
            y_milli,
            width_milli,
            height_milli,
            overflow,
        }
    }

    pub fn node_path(&self) -> &str {
        &self.node_path
    }

    pub const fn x_milli(&self) -> i32 {
        self.x_milli
    }

    pub const fn y_milli(&self) -> i32 {
        self.y_milli
    }

    pub const fn width_milli(&self) -> i32 {
        self.width_milli
    }

    pub const fn height_milli(&self) -> i32 {
        self.height_milli
    }

    pub const fn overflow(&self) -> CssOverflowEvidence {
        self.overflow
    }
}

pub fn winning_declaration(
    declarations: &[CssMatchedDeclaration],
) -> Option<&CssMatchedDeclaration> {
    declarations
        .iter()
        .max_by_key(|declaration| declaration.priority())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CssRule<'a> {
    selector: &'a str,
    body: &'a str,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct AtRulePrelude {
    rule: String,
    prelude: String,
}

struct SelectorScanner<'a> {
    selector: &'a str,
    index: usize,
    specificity: CssSpecificity,
    diagnostics: Vec<TakumiDiagnostic>,
}

impl<'a> SelectorScanner<'a> {
    fn new(selector: &'a str) -> Self {
        Self {
            selector,
            index: 0,
            specificity: CssSpecificity::ZERO,
            diagnostics: Vec::new(),
        }
    }

    fn scan(&mut self) {
        if self.selector.contains("::") {
            self.diagnostics
                .push(TakumiDiagnostic::unsupported_css_selector(self.selector));
            return;
        }
        while self.index < self.selector.len() {
            self.skip_ascii_whitespace();
            let Some(ch) = self.peek_char() else {
                break;
            };
            match ch {
                '>' => self.index += ch.len_utf8(),
                '+' | '~' | '*' => self.reject_current_selector(),
                '.' => self.scan_class_like(),
                '#' => self.scan_id(),
                '[' => self.scan_attribute(),
                ':' => self.scan_pseudo_class(),
                value if is_identifier_start(value) => self.scan_element(),
                _ => self.reject_current_selector(),
            }
        }
    }

    fn peek_char(&self) -> Option<char> {
        self.selector[self.index..].chars().next()
    }

    fn skip_ascii_whitespace(&mut self) {
        while self
            .peek_char()
            .is_some_and(|value| value.is_ascii_whitespace())
        {
            self.index += 1;
        }
    }

    fn scan_class_like(&mut self) {
        self.index += 1;
        if self.scan_identifier().is_empty() {
            self.reject_current_selector();
        } else {
            self.specificity.add_class_like();
        }
    }

    fn scan_id(&mut self) {
        self.index += 1;
        if self.scan_identifier().is_empty() {
            self.reject_current_selector();
        } else {
            self.specificity.add_id();
        }
    }

    fn scan_attribute(&mut self) {
        let start = self.index;
        let Some(close_relative) = self.selector[start..].find(']') else {
            self.reject_current_selector();
            return;
        };
        self.index = start + close_relative + 1;
        let content = self.selector[start + 1..start + close_relative].trim();
        if is_supported_part_attribute(content) {
            self.specificity.add_class_like();
        } else {
            self.diagnostics
                .push(TakumiDiagnostic::unsupported_css_selector(
                    self.selector[start..self.index].to_owned(),
                ));
        }
    }

    fn scan_pseudo_class(&mut self) {
        self.index += 1;
        let pseudo = self.scan_identifier();
        if matches!(pseudo.as_str(), "hover" | "focus" | "active" | "disabled") {
            self.specificity.add_class_like();
        } else {
            self.diagnostics
                .push(TakumiDiagnostic::unsupported_css_selector(format!(
                    ":{pseudo}"
                )));
        }
    }

    fn scan_element(&mut self) {
        let element = self.scan_identifier();
        if element.is_empty() {
            self.reject_current_selector();
        } else {
            self.specificity.add_element();
        }
    }

    fn scan_identifier(&mut self) -> String {
        let start = self.index;
        while self.peek_char().is_some_and(is_identifier_continue) {
            self.index += self.peek_char().map_or(0, char::len_utf8);
        }
        self.selector[start..self.index].to_owned()
    }

    fn reject_current_selector(&mut self) {
        self.diagnostics
            .push(TakumiDiagnostic::unsupported_css_selector(self.selector));
        self.index = self.selector.len();
    }
}

fn css_rules(css: &str) -> impl Iterator<Item = CssRule<'_>> {
    css.split('}').filter_map(|chunk| {
        let (selector, body) = chunk.split_once('{')?;
        Some(CssRule { selector, body })
    })
}

fn css_declarations(css: &str) -> impl Iterator<Item = (&str, &str)> {
    css.split(';')
        .filter_map(|chunk| chunk.split_once(':'))
        .map(|(name, value)| (name.trim(), value.trim()))
}

fn declared_custom_properties(css: &str) -> BTreeSet<String> {
    css.split(['{', ';', '}'])
        .filter_map(|chunk| chunk.split_once(':'))
        .map(|(name, _)| normalize(name))
        .filter(|name| name.starts_with("--"))
        .collect()
}

fn unresolved_variables(value: &str, custom_properties: &BTreeSet<String>) -> Vec<String> {
    let mut unresolved = Vec::new();
    let mut rest = value;
    while let Some(position) = rest.find("var(") {
        rest = &rest[position + 4..];
        let Some(end) = rest.find(')') else {
            break;
        };
        let body = &rest[..end];
        let has_fallback = body.contains(',');
        let name = body
            .split(',')
            .next()
            .map(str::trim)
            .unwrap_or_default()
            .to_owned();
        if !has_fallback && name.starts_with("--") && !custom_properties.contains(&name) {
            unresolved.push(name);
        }
        rest = &rest[end + 1..];
    }
    unresolved
}

fn at_rule_preludes(css: &str) -> Vec<AtRulePrelude> {
    let mut at_rules = Vec::new();
    let mut rest = css;
    while let Some(position) = rest.find('@') {
        rest = &rest[position + 1..];
        let rule: String = rest.chars().take_while(char::is_ascii_alphabetic).collect();
        if rule.is_empty() {
            continue;
        }
        let prelude_start = rule.len();
        let terminator = rest[prelude_start..]
            .find(['{', ';'])
            .map_or(rest.len(), |index| prelude_start + index);
        let prelude = rest[prelude_start..terminator].trim().to_owned();
        at_rules.push(AtRulePrelude { rule, prelude });
        rest = &rest[terminator..];
    }
    at_rules
}

fn at_rule_status(rule: &str, _prelude: &str) -> CssCoverageStatus {
    match rule {
        "layer" => CssCoverageStatus::ProductDataOnly,
        "container" | "keyframes" => CssCoverageStatus::IntentionallyRejected,
        _ => CssCoverageStatus::StructuredDiagnostic,
    }
}

fn declaration_status(property: &str, value: &str) -> CssCoverageStatus {
    if property.starts_with("--") {
        return CssCoverageStatus::ProductDataOnly;
    }
    if property.starts_with("grid-") || (property == "display" && normalize(value).contains("grid"))
    {
        return CssCoverageStatus::StructuredDiagnostic;
    }
    if property.starts_with("container") {
        return CssCoverageStatus::IntentionallyRejected;
    }
    if property == "transition"
        || property.starts_with("transition-")
        || property == "animation"
        || property.starts_with("animation-")
    {
        return CssCoverageStatus::IntentionallyRejected;
    }
    if is_supported_property(property) {
        CssCoverageStatus::SupportedNow
    } else {
        CssCoverageStatus::StructuredDiagnostic
    }
}

fn is_supported_property(property: &str) -> bool {
    matches!(
        property,
        "display"
            | "flex"
            | "flex-basis"
            | "flex-direction"
            | "flex-flow"
            | "flex-grow"
            | "flex-shrink"
            | "flex-wrap"
            | "justify-content"
            | "align-content"
            | "align-items"
            | "align-self"
            | "order"
            | "gap"
            | "row-gap"
            | "column-gap"
            | "margin"
            | "margin-top"
            | "margin-right"
            | "margin-bottom"
            | "margin-left"
            | "padding"
            | "padding-top"
            | "padding-right"
            | "padding-bottom"
            | "padding-left"
            | "width"
            | "height"
            | "min-width"
            | "min-height"
            | "max-width"
            | "max-height"
            | "aspect-ratio"
            | "position"
            | "inset"
            | "top"
            | "right"
            | "bottom"
            | "left"
            | "z-index"
            | "overflow"
            | "overflow-x"
            | "overflow-y"
            | "background"
            | "background-color"
            | "background-image"
            | "border"
            | "border-color"
            | "border-width"
            | "border-radius"
            | "border-top-color"
            | "border-right-color"
            | "border-bottom-color"
            | "border-left-color"
            | "color"
            | "font-size"
            | "line-height"
            | "white-space"
            | "opacity"
            | "transform"
            | "translate"
            | "rotate"
            | "scale"
            | "filter"
            | "backdrop-filter"
            | "clip-path"
            | "clip-rule"
            | "isolation"
            | "mask"
            | "mask-image"
            | "mask-size"
            | "mask-position"
            | "mask-repeat"
            | "mask-mode"
            | "mask-origin"
            | "mask-clip"
            | "mask-composite"
            | "mix-blend-mode"
            | "src"
    )
}

fn is_supported_part_attribute(content: &str) -> bool {
    let normalized = content.replace(' ', "").to_ascii_lowercase();
    normalized.starts_with("data-aw-part=") || normalized.starts_with("part=")
}

fn normalize(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

fn is_identifier_start(value: char) -> bool {
    value.is_ascii_alphabetic() || value == '_' || value == '-'
}

fn is_identifier_continue(value: char) -> bool {
    value.is_ascii_alphanumeric() || value == '_' || value == '-'
}
