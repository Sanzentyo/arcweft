//! Parser-owned semantic literal values shared by syntax families.
//!
//! These values are constructed from lexer-owned literal tokens by the active
//! parser transaction. They retain typed semantic payloads and recovery
//! without exposing a source-text reader or a pattern-specific owner.

use std::fmt;

/// Radix used by an integer literal's source spelling.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum IntRadix {
    Binary,
    Octal,
    Decimal,
    Hexadecimal,
}

impl IntRadix {
    /// Detects the radix from a numeric body such as `0xff` or `42`.
    pub fn from_number_source(source: &str) -> Self {
        if source.starts_with("0x") || source.starts_with("0X") {
            Self::Hexadecimal
        } else if source.starts_with("0b") || source.starts_with("0B") {
            Self::Binary
        } else if source.starts_with("0o") || source.starts_with("0O") {
            Self::Octal
        } else {
            Self::Decimal
        }
    }

    /// Numeric base passed to radix-aware integer parsers.
    pub const fn base(self) -> u32 {
        match self {
            Self::Binary => 2,
            Self::Octal => 8,
            Self::Decimal => 10,
            Self::Hexadecimal => 16,
        }
    }

    pub(crate) const fn prefix_len(self) -> usize {
        match self {
            Self::Decimal => 0,
            Self::Binary | Self::Octal | Self::Hexadecimal => 2,
        }
    }
}

/// Explicit integer width suffix accepted by Arcweft source syntax.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum IntSuffix {
    I8,
    I16,
    I32,
    I64,
    I128,
    ISize,
    U8,
    U16,
    U32,
    U64,
    U128,
    USize,
}

impl IntSuffix {
    /// Parses a canonical integer suffix.
    pub fn parse(source: &str) -> Option<Self> {
        Some(match source {
            "i8" => Self::I8,
            "i16" => Self::I16,
            "i32" => Self::I32,
            "i64" => Self::I64,
            "i128" => Self::I128,
            "isize" => Self::ISize,
            "u8" => Self::U8,
            "u16" => Self::U16,
            "u32" => Self::U32,
            "u64" => Self::U64,
            "u128" => Self::U128,
            "usize" => Self::USize,
            _ => return None,
        })
    }

    /// Canonical source spelling of this suffix.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::I8 => "i8",
            Self::I16 => "i16",
            Self::I32 => "i32",
            Self::I64 => "i64",
            Self::I128 => "i128",
            Self::ISize => "isize",
            Self::U8 => "u8",
            Self::U16 => "u16",
            Self::U32 => "u32",
            Self::U64 => "u64",
            Self::U128 => "u128",
            Self::USize => "usize",
        }
    }
}

/// Floating-point literal width suffix.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FloatSuffix {
    F32,
    F64,
}

impl FloatSuffix {
    pub fn parse(source: &str) -> Option<Self> {
        match source {
            "f32" => Some(Self::F32),
            "f64" => Some(Self::F64),
            _ => None,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::F32 => "f32",
            Self::F64 => "f64",
        }
    }
}

impl fmt::Display for FloatSuffix {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Numeric literal suffix that carries presentation or geometry units.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnitNumberSuffix {
    Percent,
    Px,
    Pt,
    Em,
    Rem,
    Vw,
    Vh,
    Deg,
    Rad,
    Turn,
    Db,
    Lufs,
    Bpm,
    Bars,
}

impl UnitNumberSuffix {
    pub fn parse(source: &str) -> Option<Self> {
        match source {
            "%" => Some(Self::Percent),
            "px" => Some(Self::Px),
            "pt" => Some(Self::Pt),
            "em" => Some(Self::Em),
            "rem" => Some(Self::Rem),
            "vw" => Some(Self::Vw),
            "vh" => Some(Self::Vh),
            "deg" => Some(Self::Deg),
            "rad" => Some(Self::Rad),
            "turn" => Some(Self::Turn),
            "db" => Some(Self::Db),
            "lufs" => Some(Self::Lufs),
            "bpm" => Some(Self::Bpm),
            "bars" => Some(Self::Bars),
            _ => None,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Percent => "%",
            Self::Px => "px",
            Self::Pt => "pt",
            Self::Em => "em",
            Self::Rem => "rem",
            Self::Vw => "vw",
            Self::Vh => "vh",
            Self::Deg => "deg",
            Self::Rad => "rad",
            Self::Turn => "turn",
            Self::Db => "db",
            Self::Lufs => "lufs",
            Self::Bpm => "bpm",
            Self::Bars => "bars",
        }
    }
}

impl fmt::Display for UnitNumberSuffix {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Duration suffix recognized by the syntax parser.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DurationUnit {
    Nanos,
    Micros,
    Millis,
    Seconds,
    Minutes,
    Hours,
}

impl DurationUnit {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Nanos => "ns",
            Self::Micros => "us",
            Self::Millis => "ms",
            Self::Seconds => "s",
            Self::Minutes => "min",
            Self::Hours => "h",
        }
    }
}

/// Authored literal component presence.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SyntaxLiteralShape {
    prefix: bool,
    suffix: bool,
    unit: bool,
}

/// Semantic string-literal style.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SyntaxStringKind {
    Quoted,
    Raw,
}

/// Semantic literal family retained for both valid and recovered payloads.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SyntaxLiteralFamily {
    Bool,
    String,
    Character,
    Integer,
    Decimal,
    UnitNumber,
    Duration,
}

/// Arbitrary-width integer syntax without host narrowing.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxIntegerLiteral {
    radix: IntRadix,
    digits: Box<str>,
    suffix: Option<IntSuffix>,
}

/// Decimal exponent retained as digits plus an explicit sign.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SyntaxDecimalExponent {
    negative: bool,
    digits: Box<str>,
}

/// Arbitrary-width decimal syntax retained without float conversion.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxDecimalLiteral {
    integral_digits: Box<str>,
    fractional_digits: Option<Box<str>>,
    exponent: Option<SyntaxDecimalExponent>,
    suffix: Option<FloatSuffix>,
}

/// Typed semantic literal value selected by the active syntax parser.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SyntaxLiteralValue {
    Bool(bool),
    String {
        kind: SyntaxStringKind,
        value: Box<str>,
    },
    Character(char),
    Integer(SyntaxIntegerLiteral),
    Decimal(SyntaxDecimalLiteral),
    Unit {
        value: SyntaxDecimalLiteral,
        unit: UnitNumberSuffix,
    },
    Duration {
        value: SyntaxDecimalLiteral,
        unit: DurationUnit,
    },
    Invalid(SyntaxLiteralIssue),
}

/// Literal value and exact authored component shape.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxLiteralSyntax {
    value: SyntaxLiteralValue,
    shape: SyntaxLiteralShape,
    numeric_digit_count: Option<usize>,
}

/// Typed literal recovery whose variant is the semantic family authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SyntaxLiteralIssue {
    String(SyntaxStringIssue),
    Character(SyntaxCharacterIssue),
    Integer(SyntaxIntegerIssue),
    Decimal(SyntaxDecimalIssue),
    UnitNumber(SyntaxUnitNumberIssue),
    Duration(SyntaxDurationIssue),
}

/// Typed string-literal recovery.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SyntaxStringIssue {
    InvalidEscape { attempted: Box<str> },
    Unterminated { attempted: Box<str> },
}

/// Typed character-literal recovery.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SyntaxCharacterIssue {
    InvalidEscape { attempted: Box<str> },
    Unterminated { attempted: Box<str> },
    Empty { attempted: Box<str> },
    MultipleScalars { attempted: Box<str> },
}

/// Typed integer-literal recovery.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SyntaxIntegerIssue {
    MissingDigits { attempted: Box<str> },
    InvalidDigits { attempted: Box<str> },
    InvalidSeparator { attempted: Box<str> },
}

/// Decimal coefficient/exponent recovery shared by float, unit, and duration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SyntaxDecimalComponentIssue {
    MissingCoefficient { attempted: Box<str> },
    InvalidDigits { attempted: Box<str> },
    InvalidSeparator { attempted: Box<str> },
}

/// Typed decimal-literal recovery.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SyntaxDecimalIssue {
    Decimal(SyntaxDecimalComponentIssue),
    InvalidSuffix { suffix: Box<str> },
}

/// Typed unit-number recovery.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SyntaxUnitNumberIssue {
    Decimal(SyntaxDecimalComponentIssue),
    InvalidUnit { unit: Box<str> },
}

/// Typed duration recovery.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SyntaxDurationIssue {
    Decimal(SyntaxDecimalComponentIssue),
    InvalidUnit { unit: Box<str> },
}

impl SyntaxLiteralShape {
    pub(crate) const fn new(prefix: bool, suffix: bool, unit: bool) -> Self {
        Self {
            prefix,
            suffix,
            unit,
        }
    }

    pub const fn has_prefix(self) -> bool {
        self.prefix
    }

    pub const fn has_suffix(self) -> bool {
        self.suffix
    }

    pub const fn has_unit(self) -> bool {
        self.unit
    }
}

impl SyntaxIntegerLiteral {
    pub(crate) fn new(radix: IntRadix, digits: Box<str>, suffix: Option<IntSuffix>) -> Self {
        Self {
            radix,
            digits,
            suffix,
        }
    }

    pub const fn radix(&self) -> IntRadix {
        self.radix
    }

    pub fn digits(&self) -> &str {
        &self.digits
    }

    pub const fn suffix(&self) -> Option<IntSuffix> {
        self.suffix
    }
}

impl SyntaxDecimalExponent {
    pub(crate) fn new(negative: bool, digits: Box<str>) -> Self {
        Self { negative, digits }
    }

    pub const fn is_negative(&self) -> bool {
        self.negative
    }

    pub fn digits(&self) -> &str {
        &self.digits
    }
}

impl SyntaxDecimalLiteral {
    pub(crate) fn new(
        integral_digits: Box<str>,
        fractional_digits: Option<Box<str>>,
        exponent: Option<SyntaxDecimalExponent>,
        suffix: Option<FloatSuffix>,
    ) -> Self {
        Self {
            integral_digits,
            fractional_digits,
            exponent,
            suffix,
        }
    }

    pub fn integral_digits(&self) -> &str {
        &self.integral_digits
    }

    pub fn fractional_digits(&self) -> Option<&str> {
        self.fractional_digits.as_deref()
    }

    pub const fn exponent(&self) -> Option<&SyntaxDecimalExponent> {
        self.exponent.as_ref()
    }

    pub const fn suffix(&self) -> Option<FloatSuffix> {
        self.suffix
    }
}

impl SyntaxLiteralSyntax {
    pub(crate) const fn new(
        value: SyntaxLiteralValue,
        shape: SyntaxLiteralShape,
        numeric_digit_count: Option<usize>,
    ) -> Self {
        assert!(
            matches!(
                value.family(),
                SyntaxLiteralFamily::Integer
                    | SyntaxLiteralFamily::Decimal
                    | SyntaxLiteralFamily::UnitNumber
                    | SyntaxLiteralFamily::Duration
            ) == numeric_digit_count.is_some(),
            "numeric literal families must own exactly one typed digit count"
        );
        Self {
            value,
            shape,
            numeric_digit_count,
        }
    }

    pub const fn value(&self) -> &SyntaxLiteralValue {
        &self.value
    }

    pub const fn family(&self) -> SyntaxLiteralFamily {
        self.value.family()
    }

    pub const fn shape(&self) -> SyntaxLiteralShape {
        self.shape
    }

    /// Returns the lexer-owned radix-valid digit count for numeric families.
    ///
    /// Radix prefixes, separators, typed suffixes, and units do not contribute
    /// to this count. Non-numeric literal families return `None`.
    pub const fn numeric_digit_count(&self) -> Option<usize> {
        self.numeric_digit_count
    }
}

impl SyntaxLiteralValue {
    pub const fn family(&self) -> SyntaxLiteralFamily {
        match self {
            Self::Bool(_) => SyntaxLiteralFamily::Bool,
            Self::String { .. } => SyntaxLiteralFamily::String,
            Self::Character(_) => SyntaxLiteralFamily::Character,
            Self::Integer(_) => SyntaxLiteralFamily::Integer,
            Self::Decimal(_) => SyntaxLiteralFamily::Decimal,
            Self::Unit { .. } => SyntaxLiteralFamily::UnitNumber,
            Self::Duration { .. } => SyntaxLiteralFamily::Duration,
            Self::Invalid(issue) => issue.family(),
        }
    }

    pub const fn issue(&self) -> Option<&SyntaxLiteralIssue> {
        match self {
            Self::Invalid(issue) => Some(issue),
            Self::Bool(_)
            | Self::String { .. }
            | Self::Character(_)
            | Self::Integer(_)
            | Self::Decimal(_)
            | Self::Unit { .. }
            | Self::Duration { .. } => None,
        }
    }
}

impl SyntaxLiteralIssue {
    pub const fn family(&self) -> SyntaxLiteralFamily {
        match self {
            Self::String(_) => SyntaxLiteralFamily::String,
            Self::Character(_) => SyntaxLiteralFamily::Character,
            Self::Integer(_) => SyntaxLiteralFamily::Integer,
            Self::Decimal(_) => SyntaxLiteralFamily::Decimal,
            Self::UnitNumber(_) => SyntaxLiteralFamily::UnitNumber,
            Self::Duration(_) => SyntaxLiteralFamily::Duration,
        }
    }
}
