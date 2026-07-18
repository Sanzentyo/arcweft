//! Checked presentation-environment guards owned by native Style rules.

use super::ViewStyleSourceId;
use arcweft_presentation::appearance::{
    ColorScheme, ContrastPreference, PresentationEnvironment, PresentationEnvironmentField,
    PresentationEnvironmentFieldSet, TextScaleMilli,
};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;

/// Comparison supported by a text-scale environment clause.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewTextScaleComparison {
    Equal,
    NotEqual,
    Less,
    LessOrEqual,
    Greater,
    GreaterOrEqual,
}

/// The checked typed test performed by one environment clause.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ViewEnvironmentTest {
    ColorScheme(ColorScheme),
    Contrast(ContrastPreference),
    ReducedMotion(bool),
    TextScale {
        comparison: ViewTextScaleComparison,
        value: TextScaleMilli,
    },
}

/// One source-provenanced field test in an environment condition.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ViewEnvironmentClause {
    test: ViewEnvironmentTest,
    wrapper: ViewEnvironmentWrapperIndex,
    source: ViewStyleSourceId,
}

/// Condition-local index of one contributing environment wrapper.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ViewEnvironmentWrapperIndex(u8);

/// Product source provenance for one contributing environment wrapper.
///
/// The repeated suffix is intentional: each field names the exact retained
/// source role in the canonical product wire contract.
#[allow(clippy::struct_field_names)]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ViewEnvironmentWrapperSource {
    predicate_source: ViewStyleSourceId,
    body_source: ViewStyleSourceId,
    scope_source: ViewStyleSourceId,
}

/// A canonical nonempty conjunction guarding one native Style rule.
#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize)]
pub struct ViewEnvironmentCondition {
    wrappers: Box<[ViewEnvironmentWrapperSource]>,
    clauses: Box<[ViewEnvironmentClause]>,
}

/// Failure to construct or decode a checked environment condition.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum ViewEnvironmentConditionError {
    #[error("a Style environment condition must retain at least one wrapper")]
    EmptyWrappers,
    #[error("Style environment condition has {actual} wrappers, exceeding maximum {max}")]
    TooManyWrappers { actual: usize, max: usize },
    #[error("a Style environment condition must contain at least one clause")]
    EmptyClauses,
    #[error("Style environment condition has {actual} clauses, exceeding maximum {max}")]
    TooManyClauses { actual: usize, max: usize },
    #[error(
        "Style environment clause wrapper index {index} is outside the {wrapper_count} retained wrappers"
    )]
    WrapperIndexOutOfBounds { index: u8, wrapper_count: usize },
    #[error("Style environment wrapper index {index} has no clause")]
    UnusedWrapper { index: u8 },
    #[error("Style environment condition repeats field {field:?}")]
    DuplicateField { field: PresentationEnvironmentField },
    #[error("Style environment condition is not canonical: {previous:?} precedes {next:?}")]
    NonCanonicalOrder {
        previous: PresentationEnvironmentField,
        next: PresentationEnvironmentField,
    },
}

/// Match result retaining exactly the fields read before success or failure.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ViewEnvironmentMatch {
    matched: bool,
    usage: PresentationEnvironmentFieldSet,
}

impl ViewEnvironmentClause {
    pub const fn color_scheme(
        value: ColorScheme,
        wrapper: ViewEnvironmentWrapperIndex,
        source: ViewStyleSourceId,
    ) -> Self {
        Self {
            test: ViewEnvironmentTest::ColorScheme(value),
            wrapper,
            source,
        }
    }

    pub const fn contrast(
        value: ContrastPreference,
        wrapper: ViewEnvironmentWrapperIndex,
        source: ViewStyleSourceId,
    ) -> Self {
        Self {
            test: ViewEnvironmentTest::Contrast(value),
            wrapper,
            source,
        }
    }

    pub const fn reduced_motion(
        value: bool,
        wrapper: ViewEnvironmentWrapperIndex,
        source: ViewStyleSourceId,
    ) -> Self {
        Self {
            test: ViewEnvironmentTest::ReducedMotion(value),
            wrapper,
            source,
        }
    }

    pub const fn text_scale(
        comparison: ViewTextScaleComparison,
        value: TextScaleMilli,
        wrapper: ViewEnvironmentWrapperIndex,
        source: ViewStyleSourceId,
    ) -> Self {
        Self {
            test: ViewEnvironmentTest::TextScale { comparison, value },
            wrapper,
            source,
        }
    }

    pub const fn test(self) -> ViewEnvironmentTest {
        self.test
    }

    pub const fn field(self) -> PresentationEnvironmentField {
        match self.test {
            ViewEnvironmentTest::ColorScheme(_) => PresentationEnvironmentField::ColorScheme,
            ViewEnvironmentTest::Contrast(_) => PresentationEnvironmentField::Contrast,
            ViewEnvironmentTest::ReducedMotion(_) => PresentationEnvironmentField::ReducedMotion,
            ViewEnvironmentTest::TextScale { .. } => PresentationEnvironmentField::TextScale,
        }
    }

    pub const fn source(self) -> ViewStyleSourceId {
        self.source
    }

    pub const fn wrapper(self) -> ViewEnvironmentWrapperIndex {
        self.wrapper
    }

    const fn matches(self, environment: PresentationEnvironment) -> bool {
        match self.test {
            ViewEnvironmentTest::ColorScheme(expected) => {
                environment.color_scheme() as u8 == expected as u8
            }
            ViewEnvironmentTest::Contrast(expected) => {
                environment.contrast() as u8 == expected as u8
            }
            ViewEnvironmentTest::ReducedMotion(expected) => {
                environment.reduced_motion() == expected
            }
            ViewEnvironmentTest::TextScale { comparison, value } => {
                comparison.matches(environment.text_scale(), value)
            }
        }
    }
}

impl ViewTextScaleComparison {
    const fn matches(self, actual: TextScaleMilli, expected: TextScaleMilli) -> bool {
        match self {
            Self::Equal => actual.value() == expected.value(),
            Self::NotEqual => actual.value() != expected.value(),
            Self::Less => actual.value() < expected.value(),
            Self::LessOrEqual => actual.value() <= expected.value(),
            Self::Greater => actual.value() > expected.value(),
            Self::GreaterOrEqual => actual.value() >= expected.value(),
        }
    }
}

impl ViewEnvironmentWrapperIndex {
    pub const fn new(value: u8) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u8 {
        self.0
    }

    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

impl ViewEnvironmentWrapperSource {
    pub const fn new(
        predicate_source: ViewStyleSourceId,
        body_source: ViewStyleSourceId,
        scope_source: ViewStyleSourceId,
    ) -> Self {
        Self {
            predicate_source,
            body_source,
            scope_source,
        }
    }

    pub const fn predicate_source(self) -> ViewStyleSourceId {
        self.predicate_source
    }

    pub const fn body_source(self) -> ViewStyleSourceId {
        self.body_source
    }

    pub const fn scope_source(self) -> ViewStyleSourceId {
        self.scope_source
    }
}

impl ViewEnvironmentCondition {
    pub const MAX_WRAPPERS: usize = 4;
    pub const MAX_CLAUSES: usize = 4;

    pub fn try_new(
        wrappers: Vec<ViewEnvironmentWrapperSource>,
        mut clauses: Vec<ViewEnvironmentClause>,
    ) -> Result<Self, ViewEnvironmentConditionError> {
        Self::validate_counts_and_wrapper_use(&wrappers, &clauses)?;
        clauses.sort_by_key(|clause| clause.field());
        Self::from_canonical_parts(wrappers, clauses)
    }

    fn from_canonical_parts(
        wrappers: Vec<ViewEnvironmentWrapperSource>,
        clauses: Vec<ViewEnvironmentClause>,
    ) -> Result<Self, ViewEnvironmentConditionError> {
        Self::validate_counts_and_wrapper_use(&wrappers, &clauses)?;
        for pair in clauses.windows(2) {
            let previous = pair[0].field();
            let next = pair[1].field();
            if previous == next {
                return Err(ViewEnvironmentConditionError::DuplicateField { field: next });
            }
            if previous > next {
                return Err(ViewEnvironmentConditionError::NonCanonicalOrder { previous, next });
            }
        }
        Ok(Self {
            wrappers: wrappers.into_boxed_slice(),
            clauses: clauses.into_boxed_slice(),
        })
    }

    fn validate_counts_and_wrapper_use(
        wrappers: &[ViewEnvironmentWrapperSource],
        clauses: &[ViewEnvironmentClause],
    ) -> Result<(), ViewEnvironmentConditionError> {
        if wrappers.is_empty() {
            return Err(ViewEnvironmentConditionError::EmptyWrappers);
        }
        if wrappers.len() > Self::MAX_WRAPPERS {
            return Err(ViewEnvironmentConditionError::TooManyWrappers {
                actual: wrappers.len(),
                max: Self::MAX_WRAPPERS,
            });
        }
        if clauses.is_empty() {
            return Err(ViewEnvironmentConditionError::EmptyClauses);
        }
        if clauses.len() > Self::MAX_CLAUSES {
            return Err(ViewEnvironmentConditionError::TooManyClauses {
                actual: clauses.len(),
                max: Self::MAX_CLAUSES,
            });
        }

        let mut used = [false; Self::MAX_WRAPPERS];
        for clause in clauses {
            let index = clause.wrapper().index();
            if index >= wrappers.len() {
                return Err(ViewEnvironmentConditionError::WrapperIndexOutOfBounds {
                    index: clause.wrapper().value(),
                    wrapper_count: wrappers.len(),
                });
            }
            used[index] = true;
        }
        if let Some(index) = used[..wrappers.len()]
            .iter()
            .position(|referenced| !referenced)
        {
            return Err(ViewEnvironmentConditionError::UnusedWrapper {
                index: u8::try_from(index).expect("wrapper count is bounded by four"),
            });
        }
        Ok(())
    }

    pub fn wrappers(&self) -> &[ViewEnvironmentWrapperSource] {
        &self.wrappers
    }

    pub fn clauses(&self) -> &[ViewEnvironmentClause] {
        &self.clauses
    }

    /// Projects every source identity while preserving the checked condition shape.
    pub fn try_map_sources<E>(
        &self,
        mut map: impl FnMut(ViewStyleSourceId) -> Result<ViewStyleSourceId, E>,
    ) -> Result<Self, E> {
        let wrappers = self
            .wrappers
            .iter()
            .map(|wrapper| {
                Ok(ViewEnvironmentWrapperSource::new(
                    map(wrapper.predicate_source())?,
                    map(wrapper.body_source())?,
                    map(wrapper.scope_source())?,
                ))
            })
            .collect::<Result<Vec<_>, E>>()?;
        let clauses = self
            .clauses
            .iter()
            .map(|clause| {
                Ok(ViewEnvironmentClause {
                    test: clause.test,
                    wrapper: clause.wrapper,
                    source: map(clause.source)?,
                })
            })
            .collect::<Result<Vec<_>, E>>()?;
        Ok(Self {
            wrappers: wrappers.into_boxed_slice(),
            clauses: clauses.into_boxed_slice(),
        })
    }

    pub fn matches(&self, environment: PresentationEnvironment) -> ViewEnvironmentMatch {
        let mut usage = PresentationEnvironmentFieldSet::NONE;
        for clause in &self.clauses {
            usage = usage.union(PresentationEnvironmentFieldSet::from_field(clause.field()));
            if !clause.matches(environment) {
                return ViewEnvironmentMatch {
                    matched: false,
                    usage,
                };
            }
        }
        ViewEnvironmentMatch {
            matched: true,
            usage,
        }
    }
}

impl ViewEnvironmentMatch {
    pub const fn matched(self) -> bool {
        self.matched
    }

    pub const fn usage(self) -> PresentationEnvironmentFieldSet {
        self.usage
    }
}

#[derive(Serialize)]
#[serde(tag = "field", rename_all = "snake_case")]
enum EncodedClause {
    ColorScheme {
        comparison: EqualityComparison,
        value: ColorScheme,
        wrapper: ViewEnvironmentWrapperIndex,
        source: ViewStyleSourceId,
    },
    Contrast {
        comparison: EqualityComparison,
        value: ContrastPreference,
        wrapper: ViewEnvironmentWrapperIndex,
        source: ViewStyleSourceId,
    },
    ReducedMotion {
        comparison: EqualityComparison,
        value: bool,
        wrapper: ViewEnvironmentWrapperIndex,
        source: ViewStyleSourceId,
    },
    TextScale {
        comparison: ViewTextScaleComparison,
        value: TextScaleMilli,
        wrapper: ViewEnvironmentWrapperIndex,
        source: ViewStyleSourceId,
    },
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
enum EqualityComparison {
    Equal,
}

impl Serialize for ViewEnvironmentClause {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self.test {
            ViewEnvironmentTest::ColorScheme(value) => EncodedClause::ColorScheme {
                comparison: EqualityComparison::Equal,
                value,
                wrapper: self.wrapper,
                source: self.source,
            }
            .serialize(serializer),
            ViewEnvironmentTest::Contrast(value) => EncodedClause::Contrast {
                comparison: EqualityComparison::Equal,
                value,
                wrapper: self.wrapper,
                source: self.source,
            }
            .serialize(serializer),
            ViewEnvironmentTest::ReducedMotion(value) => EncodedClause::ReducedMotion {
                comparison: EqualityComparison::Equal,
                value,
                wrapper: self.wrapper,
                source: self.source,
            }
            .serialize(serializer),
            ViewEnvironmentTest::TextScale { comparison, value } => EncodedClause::TextScale {
                comparison,
                value,
                wrapper: self.wrapper,
                source: self.source,
            }
            .serialize(serializer),
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawCondition {
    wrappers: Vec<ViewEnvironmentWrapperSource>,
    clauses: Vec<RawClause>,
}

#[derive(Deserialize)]
#[serde(tag = "field", rename_all = "snake_case", deny_unknown_fields)]
enum RawClause {
    ColorScheme {
        comparison: RawEqualityComparison,
        value: ColorScheme,
        wrapper: ViewEnvironmentWrapperIndex,
        source: ViewStyleSourceId,
    },
    Contrast {
        comparison: RawEqualityComparison,
        value: ContrastPreference,
        wrapper: ViewEnvironmentWrapperIndex,
        source: ViewStyleSourceId,
    },
    ReducedMotion {
        comparison: RawEqualityComparison,
        value: bool,
        wrapper: ViewEnvironmentWrapperIndex,
        source: ViewStyleSourceId,
    },
    TextScale {
        comparison: ViewTextScaleComparison,
        value: TextScaleMilli,
        wrapper: ViewEnvironmentWrapperIndex,
        source: ViewStyleSourceId,
    },
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum RawEqualityComparison {
    Equal,
}

impl From<RawClause> for ViewEnvironmentClause {
    fn from(value: RawClause) -> Self {
        match value {
            RawClause::ColorScheme {
                comparison: RawEqualityComparison::Equal,
                value,
                wrapper,
                source,
            } => Self::color_scheme(value, wrapper, source),
            RawClause::Contrast {
                comparison: RawEqualityComparison::Equal,
                value,
                wrapper,
                source,
            } => Self::contrast(value, wrapper, source),
            RawClause::ReducedMotion {
                comparison: RawEqualityComparison::Equal,
                value,
                wrapper,
                source,
            } => Self::reduced_motion(value, wrapper, source),
            RawClause::TextScale {
                comparison,
                value,
                wrapper,
                source,
            } => Self::text_scale(comparison, value, wrapper, source),
        }
    }
}

impl<'de> Deserialize<'de> for ViewEnvironmentCondition {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = RawCondition::deserialize(deserializer)?;
        Self::from_canonical_parts(
            raw.wrappers,
            raw.clauses.into_iter().map(Into::into).collect(),
        )
        .map_err(serde::de::Error::custom)
    }
}
