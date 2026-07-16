//! Checked Sans-I/O presentation-environment values and revision identity.

use super::{ColorScheme, ContrastPreference};
use core::fmt;
use core::ops::{BitAnd, BitOr, Sub};
use serde::de::{self, MapAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;

const PRESENTATION_ENVIRONMENT_FIELDS: [PresentationEnvironmentField; 4] = [
    PresentationEnvironmentField::ColorScheme,
    PresentationEnvironmentField::Contrast,
    PresentationEnvironmentField::ReducedMotion,
    PresentationEnvironmentField::TextScale,
];

/// Checked text scaling in tenths of one percent.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TextScaleMilli(u16);

/// Failure to construct a checked presentation text scale.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum TextScaleMilliError {
    /// The input is outside the inclusive supported interval.
    #[error("text scale {value} is outside {min}..={max} milli")]
    OutOfRange { value: u64, min: u16, max: u16 },
}

/// One field in the Style-visible presentation environment.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PresentationEnvironmentField {
    ColorScheme,
    Contrast,
    ReducedMotion,
    TextScale,
}

/// A typed value for one presentation-environment field.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PresentationEnvironmentValue {
    ColorScheme(ColorScheme),
    Contrast(ContrastPreference),
    ReducedMotion(bool),
    TextScale(TextScaleMilli),
}

/// The complete effective Style-visible presentation environment.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PresentationEnvironmentValues {
    color_scheme: ColorScheme,
    contrast: ContrastPreference,
    reduced_motion: bool,
    text_scale: TextScaleMilli,
}

/// A checked set of presentation-environment fields.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct PresentationEnvironmentFieldSet(u8);

/// Failure to decode a field set with unknown bits.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum PresentationEnvironmentFieldSetError {
    #[error("presentation environment field set contains unknown bits {bits:#010b}")]
    UnknownBits { bits: u8 },
}

/// Optional values layered over a complete presentation environment.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct PresentationEnvironmentOverrides {
    color_scheme: Option<ColorScheme>,
    contrast: Option<ContrastPreference>,
    reduced_motion: Option<bool>,
    text_scale: Option<TextScaleMilli>,
}

/// Monotonic identity for an effective presentation environment.
#[derive(
    Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct EnvironmentRevision(u64);

/// Per-field revision identity for one presentation environment.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PresentationEnvironmentFieldRevisions {
    color_scheme: EnvironmentRevision,
    contrast: EnvironmentRevision,
    reduced_motion: EnvironmentRevision,
    text_scale: EnvironmentRevision,
}

/// A complete checked environment snapshot and its revision identity.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize)]
pub struct PresentationEnvironment {
    values: PresentationEnvironmentValues,
    revision: EnvironmentRevision,
    field_revisions: PresentationEnvironmentFieldRevisions,
}

/// Failure to construct a snapshot whose field identity is inconsistent.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum PresentationEnvironmentSnapshotError {
    #[error(
        "presentation environment field revision {field_revision:?} for {field:?} exceeds global revision {global_revision:?}"
    )]
    FieldRevisionAheadOfGlobal {
        field: PresentationEnvironmentField,
        field_revision: EnvironmentRevision,
        global_revision: EnvironmentRevision,
    },
}

impl TextScaleMilli {
    pub const MIN_VALUE: u16 = 500;
    pub const MAX_VALUE: u16 = 4_000;
    pub const MIN: Self = Self(Self::MIN_VALUE);
    pub const ONE: Self = Self(1_000);
    pub const MAX: Self = Self(Self::MAX_VALUE);

    pub fn try_new(value: u16) -> Result<Self, TextScaleMilliError> {
        Self::try_from(u64::from(value))
    }

    pub const fn value(self) -> u16 {
        self.0
    }
}

impl Default for TextScaleMilli {
    fn default() -> Self {
        Self::ONE
    }
}

impl TryFrom<u16> for TextScaleMilli {
    type Error = TextScaleMilliError;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        Self::try_new(value)
    }
}

impl TryFrom<u32> for TextScaleMilli {
    type Error = TextScaleMilliError;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        Self::try_from(u64::from(value))
    }
}

impl TryFrom<u64> for TextScaleMilli {
    type Error = TextScaleMilliError;

    fn try_from(value: u64) -> Result<Self, Self::Error> {
        match u16::try_from(value) {
            Ok(value) if (Self::MIN_VALUE..=Self::MAX_VALUE).contains(&value) => Ok(Self(value)),
            _ => Err(TextScaleMilliError::OutOfRange {
                value,
                min: Self::MIN_VALUE,
                max: Self::MAX_VALUE,
            }),
        }
    }
}

impl From<TextScaleMilli> for u16 {
    fn from(value: TextScaleMilli) -> Self {
        value.value()
    }
}

impl Serialize for TextScaleMilli {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u16(self.0)
    }
}

impl<'de> Deserialize<'de> for TextScaleMilli {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct TextScaleVisitor;

        impl Visitor<'_> for TextScaleVisitor {
            type Value = TextScaleMilli;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(
                    formatter,
                    "an unsigned integer in {}..={} milli",
                    TextScaleMilli::MIN_VALUE,
                    TextScaleMilli::MAX_VALUE
                )
            }

            fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                TextScaleMilli::try_from(value).map_err(E::custom)
            }
        }

        deserializer.deserialize_u64(TextScaleVisitor)
    }
}

impl PresentationEnvironmentValue {
    pub const fn field(self) -> PresentationEnvironmentField {
        match self {
            Self::ColorScheme(_) => PresentationEnvironmentField::ColorScheme,
            Self::Contrast(_) => PresentationEnvironmentField::Contrast,
            Self::ReducedMotion(_) => PresentationEnvironmentField::ReducedMotion,
            Self::TextScale(_) => PresentationEnvironmentField::TextScale,
        }
    }
}

impl PresentationEnvironmentValues {
    pub const ENGINE_DEFAULT: Self = Self {
        color_scheme: ColorScheme::Dark,
        contrast: ContrastPreference::Standard,
        reduced_motion: false,
        text_scale: TextScaleMilli::ONE,
    };

    pub const fn new(
        color_scheme: ColorScheme,
        contrast: ContrastPreference,
        reduced_motion: bool,
        text_scale: TextScaleMilli,
    ) -> Self {
        Self {
            color_scheme,
            contrast,
            reduced_motion,
            text_scale,
        }
    }

    pub const fn color_scheme(self) -> ColorScheme {
        self.color_scheme
    }

    pub const fn contrast(self) -> ContrastPreference {
        self.contrast
    }

    pub const fn reduced_motion(self) -> bool {
        self.reduced_motion
    }

    pub const fn text_scale(self) -> TextScaleMilli {
        self.text_scale
    }

    pub const fn value(self, field: PresentationEnvironmentField) -> PresentationEnvironmentValue {
        match field {
            PresentationEnvironmentField::ColorScheme => {
                PresentationEnvironmentValue::ColorScheme(self.color_scheme)
            }
            PresentationEnvironmentField::Contrast => {
                PresentationEnvironmentValue::Contrast(self.contrast)
            }
            PresentationEnvironmentField::ReducedMotion => {
                PresentationEnvironmentValue::ReducedMotion(self.reduced_motion)
            }
            PresentationEnvironmentField::TextScale => {
                PresentationEnvironmentValue::TextScale(self.text_scale)
            }
        }
    }
}

impl PresentationEnvironmentFieldSet {
    pub const NONE: Self = Self(0);
    pub const ALL: Self = Self(0b1111);

    pub const fn from_field(field: PresentationEnvironmentField) -> Self {
        Self(field.bit())
    }

    pub const fn try_from_bits(bits: u8) -> Result<Self, PresentationEnvironmentFieldSetError> {
        if bits & !Self::ALL.0 == 0 {
            Ok(Self(bits))
        } else {
            Err(PresentationEnvironmentFieldSetError::UnknownBits { bits })
        }
    }

    pub const fn bits(self) -> u8 {
        self.0
    }

    pub const fn contains(self, field: PresentationEnvironmentField) -> bool {
        self.0 & field.bit() != 0
    }

    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    #[must_use]
    pub const fn intersection(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }

    #[must_use]
    pub const fn difference(self, other: Self) -> Self {
        Self(self.0 & !other.0)
    }

    pub fn iter(self) -> impl Iterator<Item = PresentationEnvironmentField> {
        PRESENTATION_ENVIRONMENT_FIELDS
            .into_iter()
            .filter(move |field| self.contains(*field))
    }
}

impl PresentationEnvironmentField {
    const fn bit(self) -> u8 {
        match self {
            Self::ColorScheme => 0b0001,
            Self::Contrast => 0b0010,
            Self::ReducedMotion => 0b0100,
            Self::TextScale => 0b1000,
        }
    }
}

impl BitOr for PresentationEnvironmentFieldSet {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        self.union(rhs)
    }
}

impl BitAnd for PresentationEnvironmentFieldSet {
    type Output = Self;

    fn bitand(self, rhs: Self) -> Self::Output {
        self.intersection(rhs)
    }
}

impl Sub for PresentationEnvironmentFieldSet {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        self.difference(rhs)
    }
}

impl Serialize for PresentationEnvironmentFieldSet {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u8(self.bits())
    }
}

impl<'de> Deserialize<'de> for PresentationEnvironmentFieldSet {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct FieldSetVisitor;

        impl Visitor<'_> for FieldSetVisitor {
            type Value = PresentationEnvironmentFieldSet;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("an unsigned four-bit presentation environment field set")
            }

            fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                let bits = u8::try_from(value)
                    .map_err(|_| E::invalid_value(de::Unexpected::Unsigned(value), &self))?;
                PresentationEnvironmentFieldSet::try_from_bits(bits).map_err(E::custom)
            }
        }

        deserializer.deserialize_u64(FieldSetVisitor)
    }
}

impl PresentationEnvironmentOverrides {
    pub const fn empty() -> Self {
        Self {
            color_scheme: None,
            contrast: None,
            reduced_motion: None,
            text_scale: None,
        }
    }

    pub const fn fields(self) -> PresentationEnvironmentFieldSet {
        let mut bits = 0;
        if self.color_scheme.is_some() {
            bits |= PresentationEnvironmentField::ColorScheme.bit();
        }
        if self.contrast.is_some() {
            bits |= PresentationEnvironmentField::Contrast.bit();
        }
        if self.reduced_motion.is_some() {
            bits |= PresentationEnvironmentField::ReducedMotion.bit();
        }
        if self.text_scale.is_some() {
            bits |= PresentationEnvironmentField::TextScale.bit();
        }
        PresentationEnvironmentFieldSet(bits)
    }

    pub const fn get(
        self,
        field: PresentationEnvironmentField,
    ) -> Option<PresentationEnvironmentValue> {
        match field {
            PresentationEnvironmentField::ColorScheme => match self.color_scheme {
                Some(value) => Some(PresentationEnvironmentValue::ColorScheme(value)),
                None => None,
            },
            PresentationEnvironmentField::Contrast => match self.contrast {
                Some(value) => Some(PresentationEnvironmentValue::Contrast(value)),
                None => None,
            },
            PresentationEnvironmentField::ReducedMotion => match self.reduced_motion {
                Some(value) => Some(PresentationEnvironmentValue::ReducedMotion(value)),
                None => None,
            },
            PresentationEnvironmentField::TextScale => match self.text_scale {
                Some(value) => Some(PresentationEnvironmentValue::TextScale(value)),
                None => None,
            },
        }
    }

    pub fn insert(
        &mut self,
        value: PresentationEnvironmentValue,
    ) -> Option<PresentationEnvironmentValue> {
        match value {
            PresentationEnvironmentValue::ColorScheme(value) => self
                .color_scheme
                .replace(value)
                .map(PresentationEnvironmentValue::ColorScheme),
            PresentationEnvironmentValue::Contrast(value) => self
                .contrast
                .replace(value)
                .map(PresentationEnvironmentValue::Contrast),
            PresentationEnvironmentValue::ReducedMotion(value) => self
                .reduced_motion
                .replace(value)
                .map(PresentationEnvironmentValue::ReducedMotion),
            PresentationEnvironmentValue::TextScale(value) => self
                .text_scale
                .replace(value)
                .map(PresentationEnvironmentValue::TextScale),
        }
    }

    pub fn remove(
        &mut self,
        field: PresentationEnvironmentField,
    ) -> Option<PresentationEnvironmentValue> {
        match field {
            PresentationEnvironmentField::ColorScheme => self
                .color_scheme
                .take()
                .map(PresentationEnvironmentValue::ColorScheme),
            PresentationEnvironmentField::Contrast => self
                .contrast
                .take()
                .map(PresentationEnvironmentValue::Contrast),
            PresentationEnvironmentField::ReducedMotion => self
                .reduced_motion
                .take()
                .map(PresentationEnvironmentValue::ReducedMotion),
            PresentationEnvironmentField::TextScale => self
                .text_scale
                .take()
                .map(PresentationEnvironmentValue::TextScale),
        }
    }

    pub const fn apply_to(
        self,
        base: PresentationEnvironmentValues,
    ) -> PresentationEnvironmentValues {
        PresentationEnvironmentValues::new(
            match self.color_scheme {
                Some(value) => value,
                None => base.color_scheme(),
            },
            match self.contrast {
                Some(value) => value,
                None => base.contrast(),
            },
            match self.reduced_motion {
                Some(value) => value,
                None => base.reduced_motion(),
            },
            match self.text_scale {
                Some(value) => value,
                None => base.text_scale(),
            },
        )
    }
}

impl Serialize for PresentationEnvironmentOverrides {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(Some(self.fields().iter().count()))?;
        if let Some(value) = self.color_scheme {
            map.serialize_entry("color_scheme", &value)?;
        }
        if let Some(value) = self.contrast {
            map.serialize_entry("contrast", &value)?;
        }
        if let Some(value) = self.reduced_motion {
            map.serialize_entry("reduced_motion", &value)?;
        }
        if let Some(value) = self.text_scale {
            map.serialize_entry("text_scale", &value)?;
        }
        map.end()
    }
}

impl<'de> Deserialize<'de> for PresentationEnvironmentOverrides {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(field_identifier, rename_all = "snake_case")]
        enum Field {
            ColorScheme,
            Contrast,
            ReducedMotion,
            TextScale,
        }

        struct OverridesVisitor;

        impl<'de> Visitor<'de> for OverridesVisitor {
            type Value = PresentationEnvironmentOverrides;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a presentation environment override object")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut result = PresentationEnvironmentOverrides::empty();
                while let Some(field) = map.next_key::<Field>()? {
                    match field {
                        Field::ColorScheme => {
                            if result.color_scheme.is_some() {
                                return Err(de::Error::duplicate_field("color_scheme"));
                            }
                            result.color_scheme = Some(map.next_value()?);
                        }
                        Field::Contrast => {
                            if result.contrast.is_some() {
                                return Err(de::Error::duplicate_field("contrast"));
                            }
                            result.contrast = Some(map.next_value()?);
                        }
                        Field::ReducedMotion => {
                            if result.reduced_motion.is_some() {
                                return Err(de::Error::duplicate_field("reduced_motion"));
                            }
                            result.reduced_motion = Some(map.next_value()?);
                        }
                        Field::TextScale => {
                            if result.text_scale.is_some() {
                                return Err(de::Error::duplicate_field("text_scale"));
                            }
                            result.text_scale = Some(map.next_value()?);
                        }
                    }
                }
                Ok(result)
            }
        }

        deserializer.deserialize_map(OverridesVisitor)
    }
}

impl EnvironmentRevision {
    pub const ZERO: Self = Self(0);

    pub const fn from_value(value: u64) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u64 {
        self.0
    }

    pub const fn checked_next(self) -> Option<Self> {
        match self.0.checked_add(1) {
            Some(value) => Some(Self(value)),
            None => None,
        }
    }
}

impl PresentationEnvironmentFieldRevisions {
    pub const ZERO: Self = Self {
        color_scheme: EnvironmentRevision::ZERO,
        contrast: EnvironmentRevision::ZERO,
        reduced_motion: EnvironmentRevision::ZERO,
        text_scale: EnvironmentRevision::ZERO,
    };

    pub const fn new(
        color_scheme: EnvironmentRevision,
        contrast: EnvironmentRevision,
        reduced_motion: EnvironmentRevision,
        text_scale: EnvironmentRevision,
    ) -> Self {
        Self {
            color_scheme,
            contrast,
            reduced_motion,
            text_scale,
        }
    }

    pub const fn field_revision(self, field: PresentationEnvironmentField) -> EnvironmentRevision {
        match field {
            PresentationEnvironmentField::ColorScheme => self.color_scheme,
            PresentationEnvironmentField::Contrast => self.contrast,
            PresentationEnvironmentField::ReducedMotion => self.reduced_motion,
            PresentationEnvironmentField::TextScale => self.text_scale,
        }
    }
}

impl PresentationEnvironment {
    pub const ENGINE_DEFAULT: Self = Self::initial(PresentationEnvironmentValues::ENGINE_DEFAULT);

    pub const fn initial(values: PresentationEnvironmentValues) -> Self {
        Self {
            values,
            revision: EnvironmentRevision::ZERO,
            field_revisions: PresentationEnvironmentFieldRevisions::ZERO,
        }
    }

    pub fn try_from_parts(
        values: PresentationEnvironmentValues,
        revision: EnvironmentRevision,
        field_revisions: PresentationEnvironmentFieldRevisions,
    ) -> Result<Self, PresentationEnvironmentSnapshotError> {
        for field in PRESENTATION_ENVIRONMENT_FIELDS {
            let field_revision = field_revisions.field_revision(field);
            if field_revision > revision {
                return Err(
                    PresentationEnvironmentSnapshotError::FieldRevisionAheadOfGlobal {
                        field,
                        field_revision,
                        global_revision: revision,
                    },
                );
            }
        }
        Ok(Self {
            values,
            revision,
            field_revisions,
        })
    }

    pub const fn values(self) -> PresentationEnvironmentValues {
        self.values
    }

    pub const fn value(self, field: PresentationEnvironmentField) -> PresentationEnvironmentValue {
        self.values.value(field)
    }

    pub const fn color_scheme(self) -> ColorScheme {
        self.values.color_scheme()
    }

    pub const fn contrast(self) -> ContrastPreference {
        self.values.contrast()
    }

    pub const fn reduced_motion(self) -> bool {
        self.values.reduced_motion()
    }

    pub const fn text_scale(self) -> TextScaleMilli {
        self.values.text_scale()
    }

    pub const fn revision(self) -> EnvironmentRevision {
        self.revision
    }

    pub const fn field_revisions(self) -> PresentationEnvironmentFieldRevisions {
        self.field_revisions
    }

    pub const fn field_revision(self, field: PresentationEnvironmentField) -> EnvironmentRevision {
        self.field_revisions.field_revision(field)
    }
}

impl<'de> Deserialize<'de> for PresentationEnvironment {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct RawPresentationEnvironment {
            values: PresentationEnvironmentValues,
            revision: EnvironmentRevision,
            field_revisions: PresentationEnvironmentFieldRevisions,
        }

        let raw = RawPresentationEnvironment::deserialize(deserializer)?;
        Self::try_from_parts(raw.values, raw.revision, raw.field_revisions)
            .map_err(de::Error::custom)
    }
}
