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
    source: ViewStyleSourceId,
}

/// A canonical nonempty conjunction guarding one native Style rule.
#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize)]
pub struct ViewEnvironmentCondition {
    source: ViewStyleSourceId,
    clauses: Box<[ViewEnvironmentClause]>,
}

/// Failure to construct or decode a checked environment condition.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum ViewEnvironmentConditionError {
    #[error("a Style environment condition must contain at least one clause")]
    Empty,
    #[error("Style environment condition has {actual} clauses, exceeding maximum {max}")]
    TooMany { actual: usize, max: usize },
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
    pub const fn color_scheme(value: ColorScheme, source: ViewStyleSourceId) -> Self {
        Self {
            test: ViewEnvironmentTest::ColorScheme(value),
            source,
        }
    }

    pub const fn contrast(value: ContrastPreference, source: ViewStyleSourceId) -> Self {
        Self {
            test: ViewEnvironmentTest::Contrast(value),
            source,
        }
    }

    pub const fn reduced_motion(value: bool, source: ViewStyleSourceId) -> Self {
        Self {
            test: ViewEnvironmentTest::ReducedMotion(value),
            source,
        }
    }

    pub const fn text_scale(
        comparison: ViewTextScaleComparison,
        value: TextScaleMilli,
        source: ViewStyleSourceId,
    ) -> Self {
        Self {
            test: ViewEnvironmentTest::TextScale { comparison, value },
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

impl ViewEnvironmentCondition {
    pub const MAX_CLAUSES: usize = 4;

    pub fn try_new(
        source: ViewStyleSourceId,
        mut clauses: Vec<ViewEnvironmentClause>,
    ) -> Result<Self, ViewEnvironmentConditionError> {
        if clauses.is_empty() {
            return Err(ViewEnvironmentConditionError::Empty);
        }
        if clauses.len() > Self::MAX_CLAUSES {
            return Err(ViewEnvironmentConditionError::TooMany {
                actual: clauses.len(),
                max: Self::MAX_CLAUSES,
            });
        }
        clauses.sort_by_key(|clause| clause.field());
        Self::from_canonical_parts(source, clauses)
    }

    fn from_canonical_parts(
        source: ViewStyleSourceId,
        clauses: Vec<ViewEnvironmentClause>,
    ) -> Result<Self, ViewEnvironmentConditionError> {
        if clauses.is_empty() {
            return Err(ViewEnvironmentConditionError::Empty);
        }
        if clauses.len() > Self::MAX_CLAUSES {
            return Err(ViewEnvironmentConditionError::TooMany {
                actual: clauses.len(),
                max: Self::MAX_CLAUSES,
            });
        }
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
            source,
            clauses: clauses.into_boxed_slice(),
        })
    }

    pub const fn source(&self) -> ViewStyleSourceId {
        self.source
    }

    pub fn clauses(&self) -> &[ViewEnvironmentClause] {
        &self.clauses
    }

    /// Projects every source identity while preserving the checked condition shape.
    pub fn try_map_sources<E>(
        &self,
        mut map: impl FnMut(ViewStyleSourceId) -> Result<ViewStyleSourceId, E>,
    ) -> Result<Self, E> {
        let source = map(self.source)?;
        let clauses = self
            .clauses
            .iter()
            .map(|clause| {
                Ok(ViewEnvironmentClause {
                    test: clause.test,
                    source: map(clause.source)?,
                })
            })
            .collect::<Result<Vec<_>, E>>()?;
        Ok(Self {
            source,
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
        source: ViewStyleSourceId,
    },
    Contrast {
        comparison: EqualityComparison,
        value: ContrastPreference,
        source: ViewStyleSourceId,
    },
    ReducedMotion {
        comparison: EqualityComparison,
        value: bool,
        source: ViewStyleSourceId,
    },
    TextScale {
        comparison: ViewTextScaleComparison,
        value: TextScaleMilli,
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
                source: self.source,
            }
            .serialize(serializer),
            ViewEnvironmentTest::Contrast(value) => EncodedClause::Contrast {
                comparison: EqualityComparison::Equal,
                value,
                source: self.source,
            }
            .serialize(serializer),
            ViewEnvironmentTest::ReducedMotion(value) => EncodedClause::ReducedMotion {
                comparison: EqualityComparison::Equal,
                value,
                source: self.source,
            }
            .serialize(serializer),
            ViewEnvironmentTest::TextScale { comparison, value } => EncodedClause::TextScale {
                comparison,
                value,
                source: self.source,
            }
            .serialize(serializer),
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawCondition {
    source: ViewStyleSourceId,
    clauses: Vec<RawClause>,
}

#[derive(Deserialize)]
#[serde(tag = "field", rename_all = "snake_case", deny_unknown_fields)]
enum RawClause {
    ColorScheme {
        comparison: RawEqualityComparison,
        value: ColorScheme,
        source: ViewStyleSourceId,
    },
    Contrast {
        comparison: RawEqualityComparison,
        value: ContrastPreference,
        source: ViewStyleSourceId,
    },
    ReducedMotion {
        comparison: RawEqualityComparison,
        value: bool,
        source: ViewStyleSourceId,
    },
    TextScale {
        comparison: ViewTextScaleComparison,
        value: TextScaleMilli,
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
                source,
            } => Self::color_scheme(value, source),
            RawClause::Contrast {
                comparison: RawEqualityComparison::Equal,
                value,
                source,
            } => Self::contrast(value, source),
            RawClause::ReducedMotion {
                comparison: RawEqualityComparison::Equal,
                value,
                source,
            } => Self::reduced_motion(value, source),
            RawClause::TextScale {
                comparison,
                value,
                source,
            } => Self::text_scale(comparison, value, source),
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
            raw.source,
            raw.clauses.into_iter().map(Into::into).collect(),
        )
        .map_err(serde::de::Error::custom)
    }
}
