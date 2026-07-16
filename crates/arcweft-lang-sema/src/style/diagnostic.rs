//! Structured semantic diagnostics for native Style.

use arcweft_lang_syntax::ast::common::TextRange;

/// Stable symbolic Style diagnostic code.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum StyleDiagnosticCode {
    UnknownProperty,
    InvalidValueType,
    InvalidUnit,
    NonFiniteValue,
    UnknownElement,
    UnknownState,
    MalformedSelector,
    DuplicateToken,
    UnresolvedToken,
    TokenCycle,
    TokenTypeMismatch,
    InvalidAppend,
    LogicalTranslationNotSignReversible,
    PropertyNotApplicable,
    InteractiveOverflowRequiresScroll,
    InlineSelectorNotSupported,
    ScopeReferenceNotFound,
    EnvironmentExpectedField,
    EnvironmentExpectedComparison,
    EnvironmentInvalidComparison,
    EnvironmentExpectedValue,
    EnvironmentUnsupportedValue,
    EnvironmentInvalidValue,
    EnvironmentTextScalePrecision,
    EnvironmentTextScaleRange,
    EnvironmentDuplicateField,
    EnvironmentDuplicateFieldOnPath,
    EnvironmentConditionLimit,
    EnvironmentInvalidPath,
    EnvironmentEmptyCondition,
}

/// Source-addressed Style diagnostic with typed comparison context.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StyleDiagnostic {
    code: StyleDiagnosticCode,
    range: TextRange,
    details: Box<StyleDiagnosticDetails>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct StyleDiagnosticDetails {
    message: String,
    subject: Option<String>,
    expected: Option<String>,
    actual: Option<String>,
    nearest_names: Vec<String>,
    valid_inventory: Vec<String>,
    accepted_units: Vec<String>,
    owner_sheet: Option<String>,
    ordered_subjects: Vec<String>,
    related_ranges: Vec<TextRange>,
}

impl StyleDiagnosticCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UnknownProperty => "style::unknown_property",
            Self::InvalidValueType => "style::invalid_value_type",
            Self::InvalidUnit => "style::invalid_unit",
            Self::NonFiniteValue => "style::non_finite_value",
            Self::UnknownElement => "style::unknown_element",
            Self::UnknownState => "style::unknown_state",
            Self::MalformedSelector => "style::malformed_selector",
            Self::DuplicateToken => "style::duplicate_token",
            Self::UnresolvedToken => "style::unresolved_token",
            Self::TokenCycle => "style::token_cycle",
            Self::TokenTypeMismatch => "style::token_type_mismatch",
            Self::InvalidAppend => "style::invalid_append",
            Self::LogicalTranslationNotSignReversible => {
                "style::logical_translation_not_sign_reversible"
            }
            Self::PropertyNotApplicable => "style::property_not_applicable",
            Self::InteractiveOverflowRequiresScroll => "view::interactive_overflow_requires_scroll",
            Self::InlineSelectorNotSupported => "style::inline_selector_not_supported",
            Self::ScopeReferenceNotFound => "style::scope_reference_not_found",
            Self::EnvironmentExpectedField => "style.environment.expected_field",
            Self::EnvironmentExpectedComparison => "style.environment.expected_comparison",
            Self::EnvironmentInvalidComparison => "style.environment.invalid_comparison",
            Self::EnvironmentExpectedValue => "style.environment.expected_value",
            Self::EnvironmentUnsupportedValue => "style.environment.unsupported_value",
            Self::EnvironmentInvalidValue => "style.environment.invalid_value",
            Self::EnvironmentTextScalePrecision => "style.environment.text_scale_precision",
            Self::EnvironmentTextScaleRange => "style.environment.text_scale_range",
            Self::EnvironmentDuplicateField => "style.environment.duplicate_field",
            Self::EnvironmentDuplicateFieldOnPath => "style.environment.duplicate_field_on_path",
            Self::EnvironmentConditionLimit => "style.environment.condition_limit",
            Self::EnvironmentInvalidPath => "style.environment.invalid_path",
            Self::EnvironmentEmptyCondition => "style.environment.empty_condition",
        }
    }
}

impl StyleDiagnostic {
    pub fn new(code: StyleDiagnosticCode, message: impl Into<String>, range: TextRange) -> Self {
        Self {
            code,
            range,
            details: Box::new(StyleDiagnosticDetails {
                message: message.into(),
                subject: None,
                expected: None,
                actual: None,
                nearest_names: Vec::new(),
                valid_inventory: Vec::new(),
                accepted_units: Vec::new(),
                owner_sheet: None,
                ordered_subjects: Vec::new(),
                related_ranges: Vec::new(),
            }),
        }
    }

    #[must_use]
    pub fn with_subject(mut self, subject: impl Into<String>) -> Self {
        self.details.subject = Some(subject.into());
        self
    }

    #[must_use]
    pub fn with_types(mut self, expected: impl Into<String>, actual: impl Into<String>) -> Self {
        self.details.expected = Some(expected.into());
        self.details.actual = Some(actual.into());
        self
    }

    #[must_use]
    pub fn with_nearest_names(mut self, names: Vec<String>) -> Self {
        self.details.nearest_names = names;
        self
    }

    #[must_use]
    pub fn with_valid_inventory(mut self, names: Vec<String>) -> Self {
        self.details.valid_inventory = names;
        self
    }

    #[must_use]
    pub fn with_accepted_units(mut self, units: Vec<String>) -> Self {
        self.details.accepted_units = units;
        self
    }

    #[must_use]
    pub fn with_owner_sheet(mut self, sheet: impl Into<String>) -> Self {
        self.details.owner_sheet = Some(sheet.into());
        self
    }

    #[must_use]
    pub fn with_ordered_subjects(mut self, subjects: Vec<String>) -> Self {
        self.details.ordered_subjects = subjects;
        self
    }

    #[must_use]
    pub fn with_related_range(mut self, range: TextRange) -> Self {
        self.details.related_ranges.push(range);
        self
    }

    pub const fn code(&self) -> StyleDiagnosticCode {
        self.code
    }

    pub fn message(&self) -> &str {
        &self.details.message
    }

    pub const fn range(&self) -> TextRange {
        self.range
    }

    pub fn subject(&self) -> Option<&str> {
        self.details.subject.as_deref()
    }

    pub fn expected(&self) -> Option<&str> {
        self.details.expected.as_deref()
    }

    pub fn actual(&self) -> Option<&str> {
        self.details.actual.as_deref()
    }

    pub fn nearest_names(&self) -> &[String] {
        &self.details.nearest_names
    }

    pub fn valid_inventory(&self) -> &[String] {
        &self.details.valid_inventory
    }

    pub fn accepted_units(&self) -> &[String] {
        &self.details.accepted_units
    }

    pub fn owner_sheet(&self) -> Option<&str> {
        self.details.owner_sheet.as_deref()
    }

    pub fn ordered_subjects(&self) -> &[String] {
        &self.details.ordered_subjects
    }

    pub fn related_ranges(&self) -> &[TextRange] {
        &self.details.related_ranges
    }
}
