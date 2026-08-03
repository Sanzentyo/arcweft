//! Exact, raw-preserving integer literal syntax.
//!
//! Shared literal syntax owns source radix and suffix interpretation. This
//! expression module owns the raw integer payload and compact integer-only
//! bracket representation. Expected-type inference and range policy remain
//! semantic-layer responsibilities.

use thiserror::Error;

use crate::ast::common::TextRange;
use crate::literal::{IntRadix, IntSuffix};

/// Compact representation for integer-only bracket sequence literals.
#[derive(Clone, Debug)]
pub struct NumericBracketSeq {
    literals: Vec<IntLiteral>,
    source: NumericBracketSource,
}

#[derive(Clone, Debug)]
enum NumericBracketSource {
    Synthetic,
    Authored(AuthoredNumericBracketSource),
}

#[derive(Clone, Debug)]
struct AuthoredNumericBracketSource {
    literal_ranges: Box<[TextRange]>,
}

impl NumericBracketSeq {
    /// Builds a compact integer sequence when every item uses the same suffix.
    pub fn new(literals: Vec<IntLiteral>) -> Result<Self, NumericBracketSeqError> {
        let suffix = literals.first().and_then(IntLiteral::suffix);
        if literals.iter().all(|literal| literal.suffix() == suffix) {
            Ok(Self {
                literals,
                source: NumericBracketSource::Synthetic,
            })
        } else {
            Err(NumericBracketSeqError)
        }
    }

    /// Builds a parser-owned compact sequence with one exact range per literal.
    pub(super) fn authored(
        literals: Vec<IntLiteral>,
        literal_ranges: Vec<TextRange>,
    ) -> Result<Self, AuthoredNumericBracketSeqError> {
        let source = AuthoredNumericBracketSource::try_new(literals.len(), literal_ranges)?;
        let mut sequence = Self::new(literals)?;
        sequence.source = NumericBracketSource::Authored(source);
        Ok(sequence)
    }

    /// Exact authored byte range for one literal, absent for synthetic AST values.
    pub fn literal_range(&self, index: usize) -> Option<TextRange> {
        match &self.source {
            NumericBracketSource::Synthetic => None,
            NumericBracketSource::Authored(source) => source.literal_range(index),
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

impl AuthoredNumericBracketSource {
    fn try_new(
        literal_count: usize,
        literal_ranges: Vec<TextRange>,
    ) -> Result<Self, AuthoredNumericBracketSeqError> {
        let ranges_are_valid = literal_ranges.len() == literal_count
            && literal_ranges
                .iter()
                .all(|range| range.start() < range.end())
            && literal_ranges
                .windows(2)
                .all(|ranges| ranges[0].end() <= ranges[1].start());
        if !ranges_are_valid {
            return Err(AuthoredNumericBracketSeqError::InvalidLiteralRanges);
        }
        Ok(Self {
            literal_ranges: literal_ranges.into_boxed_slice(),
        })
    }

    fn literal_range(&self, index: usize) -> Option<TextRange> {
        self.literal_ranges.get(index).copied()
    }
}

impl PartialEq for NumericBracketSeq {
    fn eq(&self, other: &Self) -> bool {
        self.literals == other.literals
    }
}

impl Eq for NumericBracketSeq {}

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

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub(super) enum AuthoredNumericBracketSeqError {
    #[error(transparent)]
    Numeric(#[from] NumericBracketSeqError),
    #[error("compact integer sequence literal ranges must be non-empty, ordered, and complete")]
    InvalidLiteralRanges,
}

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
