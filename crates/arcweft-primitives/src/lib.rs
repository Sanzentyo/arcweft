//! Deterministic scalar quantities shared by semantic and runtime owners.
//!
//! Lengths retain their measurement basis until an owning layout context
//! resolves it. None of these values performs I/O or infers a viewport/font.

/// Signed fixed-point thousandths.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Milli(pub i32);

/// Inclusive zero-to-one ratio encoded as thousandths.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RatioMilli(u16);

impl RatioMilli {
    #[must_use]
    pub const fn try_new(value: u16) -> Option<Self> {
        if value <= 1_000 {
            Some(Self(value))
        } else {
            None
        }
    }

    #[must_use]
    pub const fn get(self) -> u16 {
        self.0
    }
}

/// Measurement basis of an unresolved length.
#[derive(
    Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum LengthUnit {
    Px,
    Pt,
    Ch,
    Em,
    Rem,
    Vw,
    Vh,
    Percent,
}

impl LengthUnit {
    #[must_use]
    pub const fn encoded(self) -> u8 {
        match self {
            Self::Px => 0,
            Self::Pt => 1,
            Self::Ch => 2,
            Self::Em => 3,
            Self::Rem => 4,
            Self::Vw => 5,
            Self::Vh => 6,
            Self::Percent => 7,
        }
    }

    #[must_use]
    pub const fn from_encoded(value: u8) -> Option<Self> {
        Some(match value {
            0 => Self::Px,
            1 => Self::Pt,
            2 => Self::Ch,
            3 => Self::Em,
            4 => Self::Rem,
            5 => Self::Vw,
            6 => Self::Vh,
            7 => Self::Percent,
            _ => return None,
        })
    }

    #[must_use]
    pub const fn suffix(self) -> &'static str {
        match self {
            Self::Px => "px",
            Self::Pt => "pt",
            Self::Ch => "ch",
            Self::Em => "em",
            Self::Rem => "rem",
            Self::Vw => "vw",
            Self::Vh => "vh",
            Self::Percent => "%",
        }
    }
}

/// Fixed-point length; interpretation belongs to the consuming layout context.
#[derive(
    Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, serde::Serialize, serde::Deserialize,
)]
pub struct CheckedLength {
    pub milli: i32,
    pub unit: LengthUnit,
}

/// Angle normalized to milli-degrees.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CheckedAngle {
    pub milli_degrees: i32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ratio_admission_includes_endpoints_and_rejects_overflow() {
        assert_eq!(RatioMilli::try_new(0).map(RatioMilli::get), Some(0));
        assert_eq!(RatioMilli::try_new(1_000).map(RatioMilli::get), Some(1_000));
        assert_eq!(RatioMilli::try_new(1_001), None);
        assert_eq!(RatioMilli::try_new(u16::MAX), None);
    }

    #[test]
    fn length_units_have_distinct_round_trip_tags() {
        let units = [
            LengthUnit::Px,
            LengthUnit::Pt,
            LengthUnit::Ch,
            LengthUnit::Em,
            LengthUnit::Rem,
            LengthUnit::Vw,
            LengthUnit::Vh,
            LengthUnit::Percent,
        ];
        for (tag, unit) in units.into_iter().enumerate() {
            assert_eq!(usize::from(unit.encoded()), tag);
            assert_eq!(LengthUnit::from_encoded(unit.encoded()), Some(unit));
            assert!(!unit.suffix().is_empty());
        }
        assert_eq!(LengthUnit::from_encoded(8), None);
        assert_eq!(LengthUnit::from_encoded(u8::MAX), None);
    }
}
