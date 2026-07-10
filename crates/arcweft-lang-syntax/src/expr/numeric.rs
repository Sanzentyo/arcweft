//! Exact, raw-preserving integer literal syntax.
//!
//! This module owns source radix and suffix interpretation plus the compact
//! integer-only bracket representation. Expected-type inference and range
//! policy remain semantic-layer responsibilities.

use std::fmt;

use thiserror::Error;

/// Compact representation for integer-only bracket sequence literals.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NumericBracketSeq {
    literals: Vec<IntLiteral>,
}

impl NumericBracketSeq {
    /// Builds a compact integer sequence when every item uses the same suffix.
    pub fn new(literals: Vec<IntLiteral>) -> Result<Self, NumericBracketSeqError> {
        let suffix = literals.first().and_then(IntLiteral::suffix);
        if literals.iter().all(|literal| literal.suffix() == suffix) {
            Ok(Self { literals })
        } else {
            Err(NumericBracketSeqError)
        }
    }

    /// Raw-preserving literals in source order.
    pub fn literals(&self) -> &[IntLiteral] {
        &self.literals
    }

    /// Common explicit suffix shared by every item.
    pub fn suffix(&self) -> Option<IntSuffix> {
        self.literals.first().and_then(IntLiteral::suffix)
    }

    /// Number of integer literals in this sequence.
    pub fn len(&self) -> usize {
        self.literals.len()
    }

    /// Whether the sequence contains no literals.
    pub fn is_empty(&self) -> bool {
        self.literals.is_empty()
    }
}

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

    const fn prefix_len(self) -> usize {
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

impl fmt::Display for IntSuffix {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Raw-preserving integer literal before expected-type inference.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IntLiteral {
    raw: String,
    radix: IntRadix,
    suffix: Option<IntSuffix>,
}

impl IntLiteral {
    /// Builds a literal from parser-owned source components.
    pub(crate) fn new(raw: impl Into<String>, radix: IntRadix, suffix: Option<IntSuffix>) -> Self {
        Self {
            raw: raw.into(),
            radix,
            suffix,
        }
    }

    /// Builds a canonical decimal literal, primarily for typed AST producers.
    pub fn decimal(magnitude: u128, suffix: Option<IntSuffix>) -> Self {
        let suffix_source = suffix.map(IntSuffix::as_str).unwrap_or_default();
        Self::new(
            format!("{magnitude}{suffix_source}"),
            IntRadix::Decimal,
            suffix,
        )
    }

    /// Exact authored spelling, including separators and suffix.
    pub fn raw(&self) -> &str {
        &self.raw
    }

    /// Source radix detected by the parser.
    pub const fn radix(&self) -> IntRadix {
        self.radix
    }

    /// Optional explicit integer width suffix.
    pub const fn suffix(&self) -> Option<IntSuffix> {
        self.suffix
    }

    /// Parses the non-negative mathematical value without narrowing it to a host width.
    pub fn magnitude(&self) -> Result<u128, IntLiteralValueError> {
        let (number, _) = split_number_suffix(&self.raw);
        let digits = number
            .get(self.radix.prefix_len()..)
            .unwrap_or_default()
            .chars()
            .filter(|ch| *ch != '_')
            .collect::<String>();
        if digits.is_empty() {
            return Err(IntLiteralValueError::MissingDigits);
        }
        u128::from_str_radix(&digits, self.radix.base()).map_err(|error| {
            if matches!(error.kind(), std::num::IntErrorKind::PosOverflow) {
                IntLiteralValueError::OutOfRange
            } else {
                IntLiteralValueError::InvalidDigits
            }
        })
    }
}

/// Failure to interpret the mathematical value of an integer literal.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum IntLiteralValueError {
    #[error("integer literal has no digits")]
    MissingDigits,
    #[error("integer literal contains digits that do not match its radix")]
    InvalidDigits,
    #[error("integer literal exceeds the largest representable `u128` magnitude")]
    OutOfRange,
}

/// Compact sequence construction failed because integer suffixes differ.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
#[error("compact integer sequence literals must use one common suffix")]
pub struct NumericBracketSeqError;

pub(super) fn split_number_suffix(source: &str) -> (&str, &str) {
    let split = numeric_body_len(source);
    (&source[..split], &source[split..])
}

fn numeric_body_len(source: &str) -> usize {
    if let Some(rest) = source
        .strip_prefix("0x")
        .or_else(|| source.strip_prefix("0X"))
    {
        return "0x".len() + radix_digits_len(rest, 16);
    }
    if let Some(rest) = source
        .strip_prefix("0b")
        .or_else(|| source.strip_prefix("0B"))
    {
        return "0b".len() + radix_digits_len(rest, 2);
    }
    if let Some(rest) = source
        .strip_prefix("0o")
        .or_else(|| source.strip_prefix("0O"))
    {
        return "0o".len() + radix_digits_len(rest, 8);
    }
    let bytes = source.as_bytes();
    let mut index = decimal_digits_len(source);
    if bytes.get(index) == Some(&b'.') && !source[index..].starts_with("..") {
        index += 1;
        index += decimal_digits_len(&source[index..]);
    }
    if matches!(bytes.get(index), Some(b'e' | b'E')) {
        let exponent_start = index;
        index += 1;
        if matches!(bytes.get(index), Some(b'+' | b'-')) {
            index += 1;
        }
        let digits_start = index;
        index += decimal_digits_len(&source[index..]);
        if source[digits_start..index]
            .chars()
            .filter(|ch| *ch != '_')
            .all(|ch| !ch.is_ascii_digit())
        {
            index = exponent_start;
        }
    }
    index
}

fn decimal_digits_len(source: &str) -> usize {
    source
        .char_indices()
        .take_while(|(_, ch)| ch.is_ascii_digit() || *ch == '_')
        .map(|(index, ch)| index + ch.len_utf8())
        .last()
        .unwrap_or(0)
}

fn radix_digits_len(source: &str, radix: u32) -> usize {
    source
        .char_indices()
        .take_while(|(_, ch)| *ch == '_' || digit_matches_radix(*ch, radix))
        .map(|(index, ch)| index + ch.len_utf8())
        .last()
        .unwrap_or(0)
}

pub(super) fn digit_matches_radix(ch: char, radix: u32) -> bool {
    ch.is_digit(radix)
}
