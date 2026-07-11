//! Canonical finite numbers, unit values, runtime values, and transforms.

use std::{fmt, marker::PhantomData};

use serde::{
    Deserialize, Deserializer, Serialize, Serializer,
    de::{Error as _, Visitor},
};
use thiserror::Error;

/// Golden angle in radians, fixed by its canonical `f32` bits.
pub const FX_GOLDEN_ANGLE_RAD: f32 = f32::from_bits(0x4019_98ff);

/// A finite `f32` stored as canonical IEEE-754 bits.
///
/// Negative zero is stored as positive zero, making equality and hashing exact.
#[derive(Clone, Copy, Default, Eq, Hash, PartialEq)]
pub struct FiniteF32(u32);

/// Failure to convert an external number into the canonical runtime domain.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum FiniteF32Error {
    #[error("number is not finite (IEEE-754 bits {bits:#018x})")]
    NonFinite { bits: u64 },
    #[error("number overflows the finite f32 domain (IEEE-754 bits {bits:#018x})")]
    Overflow { bits: u64 },
    #[error("non-zero number underflows to zero in the f32 domain (IEEE-754 bits {bits:#018x})")]
    Underflow { bits: u64 },
}

/// Logical length in pixels.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(transparent)]
pub struct Length(FiniteF32);

/// Angle in radians.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(transparent)]
pub struct Angle(FiniteF32);

/// Logical duration in seconds.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(transparent)]
pub struct Seconds(FiniteF32);

/// Validated opacity in the closed interval `[0, 1]`.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize)]
#[serde(transparent)]
pub struct Opacity(FiniteF32);

/// Linear RGBA color with every component in the closed interval `[0, 1]`.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct FxColor {
    red: Opacity,
    green: Opacity,
    blue: Opacity,
    alpha: Opacity,
}

/// Dimensionless two-component runtime vector.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct FxVec2 {
    pub x: FiniteF32,
    pub y: FiniteF32,
}

/// Closed runtime value types accepted by executable value programs.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
#[repr(u8)]
pub enum FxRuntimeType {
    Bool = 0,
    I32 = 1,
    F32 = 2,
    Length = 3,
    Angle = 4,
    Seconds = 5,
    Color = 6,
    Vec2 = 7,
    Transform2D = 8,
}

/// Closed runtime value set. Strings, selectors, and resource IDs are static graph data.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum FxRuntimeValue {
    Bool(bool),
    I32(i32),
    F32(FiniteF32),
    Length(Length),
    Angle(Angle),
    Seconds(Seconds),
    Color(FxColor),
    Vec2(FxVec2),
    Transform2D(Transform2D),
}

/// Closed authored transform value.
///
/// Deserialization validates opacity. Runtime resolution additionally rejects
/// any non-finite matrix result caused by transform arithmetic.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize)]
pub struct Transform2D {
    pub translate_x: Length,
    pub translate_y: Length,
    pub scale_x: FiniteF32,
    pub scale_y: FiniteF32,
    pub skew_x: Angle,
    pub skew_y: Angle,
    pub rotation: Angle,
    pub origin_x: Length,
    pub origin_y: Length,
    pub opacity: FiniteF32,
}

/// Resolved affine transform and separately composed opacity.
///
/// The linear fields represent `[[m11, m12], [m21, m22]]` for column vectors.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct ResolvedTransform2D {
    m11: FiniteF32,
    m12: FiniteF32,
    m21: FiniteF32,
    m22: FiniteF32,
    translate_x: Length,
    translate_y: Length,
    opacity: Opacity,
}

/// Transform validation or finite matrix arithmetic failure.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum Transform2DError {
    #[error("transform opacity must be in [0, 1], got bits {bits:#010x}")]
    InvalidOpacity { bits: u32 },
    #[error("transform operation `{operation}` produced a non-finite value")]
    NonFiniteResult { operation: &'static str },
}

impl FiniteF32 {
    pub const ZERO: Self = Self(0.0_f32.to_bits());
    pub const ONE: Self = Self(1.0_f32.to_bits());

    /// Validates and canonicalizes an already-rounded runtime value.
    pub fn try_new(value: f32) -> Result<Self, FiniteF32Error> {
        if !value.is_finite() {
            return Err(FiniteF32Error::NonFinite {
                bits: value.to_bits().into(),
            });
        }
        Ok(if value == 0.0 {
            Self::ZERO
        } else {
            Self(value.to_bits())
        })
    }

    /// Converts a source-decimal intermediate while detecting f32 overflow and underflow.
    pub fn try_from_f64(value: f64) -> Result<Self, FiniteF32Error> {
        if !value.is_finite() {
            return Err(FiniteF32Error::NonFinite {
                bits: value.to_bits(),
            });
        }
        // This is the single intentional source-decimal narrowing boundary.
        #[expect(
            clippy::cast_possible_truncation,
            reason = "overflow and non-zero underflow are checked immediately after narrowing"
        )]
        let rounded = value as f32;
        if !rounded.is_finite() {
            return Err(FiniteF32Error::Overflow {
                bits: value.to_bits(),
            });
        }
        if value != 0.0 && rounded == 0.0 {
            return Err(FiniteF32Error::Underflow {
                bits: value.to_bits(),
            });
        }
        Self::try_new(rounded)
    }

    pub fn try_from_bits(bits: u32) -> Result<Self, FiniteF32Error> {
        Self::try_new(f32::from_bits(bits))
    }

    pub const fn get(self) -> f32 {
        f32::from_bits(self.0)
    }

    pub const fn to_bits(self) -> u32 {
        self.0
    }
}

impl fmt::Debug for FiniteF32 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("FiniteF32")
            .field(&self.get())
            .finish()
    }
}

impl fmt::Display for FiniteF32 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.get().fmt(formatter)
    }
}

impl TryFrom<f32> for FiniteF32 {
    type Error = FiniteF32Error;

    fn try_from(value: f32) -> Result<Self, Self::Error> {
        Self::try_new(value)
    }
}

impl Serialize for FiniteF32 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_f32(self.get())
    }
}

struct FiniteF32Visitor(PhantomData<FiniteF32>);

impl Visitor<'_> for FiniteF32Visitor {
    type Value = FiniteF32;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a finite number representable as a non-underflowing f32")
    }

    fn visit_f32<E>(self, value: f32) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        FiniteF32::try_new(value).map_err(E::custom)
    }

    fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        FiniteF32::try_from_f64(value).map_err(E::custom)
    }

    fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        let value = value.to_string().parse::<f64>().map_err(E::custom)?;
        self.visit_f64(value)
    }

    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        let value = value.to_string().parse::<f64>().map_err(E::custom)?;
        self.visit_f64(value)
    }
}

impl<'de> Deserialize<'de> for FiniteF32 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(FiniteF32Visitor(PhantomData))
    }
}

impl Length {
    pub const ZERO: Self = Self(FiniteF32::ZERO);

    pub fn try_pixels(value: f32) -> Result<Self, FiniteF32Error> {
        FiniteF32::try_new(value).map(Self)
    }

    pub fn try_pixels_f64(value: f64) -> Result<Self, FiniteF32Error> {
        FiniteF32::try_from_f64(value).map(Self)
    }

    pub const fn value(self) -> FiniteF32 {
        self.0
    }

    pub const fn pixels(self) -> f32 {
        self.0.get()
    }
}

impl Angle {
    pub const ZERO: Self = Self(FiniteF32::ZERO);

    pub fn try_radians(value: f32) -> Result<Self, FiniteF32Error> {
        FiniteF32::try_new(value).map(Self)
    }

    pub fn try_degrees(value: f64) -> Result<Self, FiniteF32Error> {
        FiniteF32::try_from_f64(value.to_radians()).map(Self)
    }

    pub fn try_turns(value: f64) -> Result<Self, FiniteF32Error> {
        FiniteF32::try_from_f64(value * std::f64::consts::TAU).map(Self)
    }

    pub const fn value(self) -> FiniteF32 {
        self.0
    }

    pub const fn radians(self) -> f32 {
        self.0.get()
    }
}

impl Seconds {
    pub const ZERO: Self = Self(FiniteF32::ZERO);

    pub fn try_seconds(value: f32) -> Result<Self, FiniteF32Error> {
        FiniteF32::try_new(value).map(Self)
    }

    pub fn try_seconds_f64(value: f64) -> Result<Self, FiniteF32Error> {
        FiniteF32::try_from_f64(value).map(Self)
    }

    pub fn try_milliseconds(value: f64) -> Result<Self, FiniteF32Error> {
        FiniteF32::try_from_f64(value / 1_000.0).map(Self)
    }

    pub const fn value(self) -> FiniteF32 {
        self.0
    }

    pub const fn seconds(self) -> f32 {
        self.0.get()
    }
}

impl Opacity {
    pub const TRANSPARENT: Self = Self(FiniteF32::ZERO);
    pub const OPAQUE: Self = Self(FiniteF32::ONE);

    pub fn try_new(value: FiniteF32) -> Result<Self, Transform2DError> {
        if (0.0..=1.0).contains(&value.get()) {
            Ok(Self(value))
        } else {
            Err(Transform2DError::InvalidOpacity {
                bits: value.to_bits(),
            })
        }
    }

    pub const fn value(self) -> FiniteF32 {
        self.0
    }
}

impl Default for Opacity {
    fn default() -> Self {
        Self::OPAQUE
    }
}

impl<'de> Deserialize<'de> for Opacity {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::try_new(FiniteF32::deserialize(deserializer)?).map_err(D::Error::custom)
    }
}

impl FxColor {
    pub const TRANSPARENT: Self = Self {
        red: Opacity::TRANSPARENT,
        green: Opacity::TRANSPARENT,
        blue: Opacity::TRANSPARENT,
        alpha: Opacity::TRANSPARENT,
    };

    pub const WHITE: Self = Self {
        red: Opacity::OPAQUE,
        green: Opacity::OPAQUE,
        blue: Opacity::OPAQUE,
        alpha: Opacity::OPAQUE,
    };

    pub const fn new(red: Opacity, green: Opacity, blue: Opacity, alpha: Opacity) -> Self {
        Self {
            red,
            green,
            blue,
            alpha,
        }
    }

    pub const fn red(self) -> Opacity {
        self.red
    }

    pub const fn green(self) -> Opacity {
        self.green
    }

    pub const fn blue(self) -> Opacity {
        self.blue
    }

    pub const fn alpha(self) -> Opacity {
        self.alpha
    }
}

impl FxRuntimeValue {
    pub const fn value_type(&self) -> FxRuntimeType {
        match self {
            Self::Bool(_) => FxRuntimeType::Bool,
            Self::I32(_) => FxRuntimeType::I32,
            Self::F32(_) => FxRuntimeType::F32,
            Self::Length(_) => FxRuntimeType::Length,
            Self::Angle(_) => FxRuntimeType::Angle,
            Self::Seconds(_) => FxRuntimeType::Seconds,
            Self::Color(_) => FxRuntimeType::Color,
            Self::Vec2(_) => FxRuntimeType::Vec2,
            Self::Transform2D(_) => FxRuntimeType::Transform2D,
        }
    }
}

impl Default for Transform2D {
    fn default() -> Self {
        Self {
            translate_x: Length::ZERO,
            translate_y: Length::ZERO,
            scale_x: FiniteF32::ONE,
            scale_y: FiniteF32::ONE,
            skew_x: Angle::ZERO,
            skew_y: Angle::ZERO,
            rotation: Angle::ZERO,
            origin_x: Length::ZERO,
            origin_y: Length::ZERO,
            opacity: FiniteF32::ONE,
        }
    }
}

impl Transform2D {
    /// Validates fields whose domain is narrower than finite `f32`.
    pub fn validate(&self) -> Result<(), Transform2DError> {
        Opacity::try_new(self.opacity).map(|_| ())
    }

    /// Resolves `T(origin) * T(translation) * R * K * S * T(-origin)`.
    pub fn resolve(self) -> Result<ResolvedTransform2D, Transform2DError> {
        let opacity = Opacity::try_new(self.opacity)?;
        let scale_x = self.scale_x.get();
        let scale_y = self.scale_y.get();
        let skew_x = self.skew_x.radians().tan();
        let skew_y = self.skew_y.radians().tan();
        let (sin, cos) = self.rotation.radians().sin_cos();

        // R * K * S, using f32 at every operation to match evaluator values.
        let m11 = checked_finite("matrix.m11", (cos - sin * skew_y) * scale_x)?;
        let m12 = checked_finite("matrix.m12", (cos * skew_x - sin) * scale_y)?;
        let m21 = checked_finite("matrix.m21", (sin + cos * skew_y) * scale_x)?;
        let m22 = checked_finite("matrix.m22", (sin * skew_x + cos) * scale_y)?;

        let origin_x = self.origin_x.pixels();
        let origin_y = self.origin_y.pixels();
        let translate_x = checked_length(
            "matrix.translate_x",
            origin_x + self.translate_x.pixels() - (m11.get() * origin_x + m12.get() * origin_y),
        )?;
        let translate_y = checked_length(
            "matrix.translate_y",
            origin_y + self.translate_y.pixels() - (m21.get() * origin_x + m22.get() * origin_y),
        )?;

        Ok(ResolvedTransform2D {
            m11,
            m12,
            m21,
            m22,
            translate_x,
            translate_y,
            opacity,
        })
    }
}

#[derive(Deserialize)]
struct Transform2DWire {
    translate_x: Length,
    translate_y: Length,
    scale_x: FiniteF32,
    scale_y: FiniteF32,
    skew_x: Angle,
    skew_y: Angle,
    rotation: Angle,
    origin_x: Length,
    origin_y: Length,
    opacity: FiniteF32,
}

impl<'de> Deserialize<'de> for Transform2D {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = Transform2DWire::deserialize(deserializer)?;
        let value = Self {
            translate_x: wire.translate_x,
            translate_y: wire.translate_y,
            scale_x: wire.scale_x,
            scale_y: wire.scale_y,
            skew_x: wire.skew_x,
            skew_y: wire.skew_y,
            rotation: wire.rotation,
            origin_x: wire.origin_x,
            origin_y: wire.origin_y,
            opacity: wire.opacity,
        };
        value.validate().map_err(D::Error::custom)?;
        Ok(value)
    }
}

impl ResolvedTransform2D {
    pub const fn identity() -> Self {
        Self {
            m11: FiniteF32::ONE,
            m12: FiniteF32::ZERO,
            m21: FiniteF32::ZERO,
            m22: FiniteF32::ONE,
            translate_x: Length::ZERO,
            translate_y: Length::ZERO,
            opacity: Opacity::OPAQUE,
        }
    }

    pub const fn matrix(self) -> [FiniteF32; 4] {
        [self.m11, self.m12, self.m21, self.m22]
    }

    pub const fn translation(self) -> [Length; 2] {
        [self.translate_x, self.translate_y]
    }

    pub const fn opacity(self) -> Opacity {
        self.opacity
    }

    /// Composes `next` after `self`, matching authored Fx stack order.
    pub fn then(self, next: Self) -> Result<Self, Transform2DError> {
        let m11 = checked_finite(
            "compose.m11",
            next.m11.get() * self.m11.get() + next.m12.get() * self.m21.get(),
        )?;
        let m12 = checked_finite(
            "compose.m12",
            next.m11.get() * self.m12.get() + next.m12.get() * self.m22.get(),
        )?;
        let m21 = checked_finite(
            "compose.m21",
            next.m21.get() * self.m11.get() + next.m22.get() * self.m21.get(),
        )?;
        let m22 = checked_finite(
            "compose.m22",
            next.m21.get() * self.m12.get() + next.m22.get() * self.m22.get(),
        )?;
        let translate_x = checked_length(
            "compose.translate_x",
            next.m11.get() * self.translate_x.pixels()
                + next.m12.get() * self.translate_y.pixels()
                + next.translate_x.pixels(),
        )?;
        let translate_y = checked_length(
            "compose.translate_y",
            next.m21.get() * self.translate_x.pixels()
                + next.m22.get() * self.translate_y.pixels()
                + next.translate_y.pixels(),
        )?;
        let opacity = Opacity::try_new(checked_finite(
            "compose.opacity",
            self.opacity.value().get() * next.opacity.value().get(),
        )?)?;
        Ok(Self {
            m11,
            m12,
            m21,
            m22,
            translate_x,
            translate_y,
            opacity,
        })
    }

    /// Resolves and composes transforms so each later authored item applies last.
    pub fn compose_authored(
        transforms: impl IntoIterator<Item = Transform2D>,
    ) -> Result<Self, Transform2DError> {
        transforms
            .into_iter()
            .try_fold(Self::identity(), |total, transform| {
                total.then(transform.resolve()?)
            })
    }

    pub fn apply_point(self, x: Length, y: Length) -> Result<[Length; 2], Transform2DError> {
        Ok([
            checked_length(
                "apply.x",
                self.m11.get() * x.pixels()
                    + self.m12.get() * y.pixels()
                    + self.translate_x.pixels(),
            )?,
            checked_length(
                "apply.y",
                self.m21.get() * x.pixels()
                    + self.m22.get() * y.pixels()
                    + self.translate_y.pixels(),
            )?,
        ])
    }

    pub fn determinant(self) -> Result<FiniteF32, Transform2DError> {
        checked_finite(
            "determinant",
            self.m11.get() * self.m22.get() - self.m12.get() * self.m21.get(),
        )
    }

    pub fn is_invertible(self) -> Result<bool, Transform2DError> {
        self.determinant()
            .map(|determinant| determinant.get() != 0.0)
    }
}

fn checked_finite(operation: &'static str, value: f32) -> Result<FiniteF32, Transform2DError> {
    FiniteF32::try_new(value).map_err(|_| Transform2DError::NonFiniteResult { operation })
}

fn checked_length(operation: &'static str, value: f32) -> Result<Length, Transform2DError> {
    checked_finite(operation, value).map(Length)
}

pub(crate) fn hash_runtime_value(hasher: &mut blake3::Hasher, value: &FxRuntimeValue) {
    hasher.update(&[value.value_type() as u8]);
    match value {
        FxRuntimeValue::Bool(value) => {
            hasher.update(&[u8::from(*value)]);
        }
        FxRuntimeValue::I32(value) => {
            hasher.update(&value.to_le_bytes());
        }
        FxRuntimeValue::F32(value) => {
            hasher.update(&value.to_bits().to_le_bytes());
        }
        FxRuntimeValue::Length(value) => {
            hasher.update(&value.value().to_bits().to_le_bytes());
        }
        FxRuntimeValue::Angle(value) => {
            hasher.update(&value.value().to_bits().to_le_bytes());
        }
        FxRuntimeValue::Seconds(value) => {
            hasher.update(&value.value().to_bits().to_le_bytes());
        }
        FxRuntimeValue::Color(value) => {
            for channel in [value.red(), value.green(), value.blue(), value.alpha()] {
                hasher.update(&channel.value().to_bits().to_le_bytes());
            }
        }
        FxRuntimeValue::Vec2(value) => {
            hasher.update(&value.x.to_bits().to_le_bytes());
            hasher.update(&value.y.to_bits().to_le_bytes());
        }
        FxRuntimeValue::Transform2D(value) => {
            for bits in [
                value.translate_x.value().to_bits(),
                value.translate_y.value().to_bits(),
                value.scale_x.to_bits(),
                value.scale_y.to_bits(),
                value.skew_x.value().to_bits(),
                value.skew_y.value().to_bits(),
                value.rotation.value().to_bits(),
                value.origin_x.value().to_bits(),
                value.origin_y.value().to_bits(),
                value.opacity.to_bits(),
            ] {
                hasher.update(&bits.to_le_bytes());
            }
        }
    }
}
