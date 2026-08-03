//! Typed semantic projection for lexer-owned literal tokens.

use crate::grammar::kinds::SyntaxKind;
use crate::literal::{
    DurationUnit, FloatSuffix, IntRadix, IntSuffix, SyntaxCharacterIssue,
    SyntaxDecimalComponentIssue, SyntaxDecimalExponent, SyntaxDecimalIssue, SyntaxDecimalLiteral,
    SyntaxDurationIssue, SyntaxIntegerIssue, SyntaxIntegerLiteral, SyntaxLiteralIssue,
    SyntaxLiteralShape, SyntaxLiteralSyntax, SyntaxLiteralValue, SyntaxStringIssue,
    SyntaxStringKind, SyntaxUnitNumberIssue, UnitNumberSuffix,
};

use super::{
    LexToken, LiteralLexemeComponent, LiteralLexemePart, number_body_bounds, token_local_range,
};

/// One lexer-owned literal semantic value and its exact authored components.
pub(in crate::parser) struct LiteralLexemeProjection {
    syntax: SyntaxLiteralSyntax,
    components: Vec<LiteralLexemeComponent>,
}

impl LiteralLexemeProjection {
    pub(in crate::parser) const fn syntax(&self) -> &SyntaxLiteralSyntax {
        &self.syntax
    }

    pub(in crate::parser) fn components(&self) -> &[LiteralLexemeComponent] {
        &self.components
    }

    pub(in crate::parser) fn into_syntax(self) -> SyntaxLiteralSyntax {
        self.syntax
    }
}

/// Projects one already-recognized literal token exactly once.
pub(in crate::parser) fn typed_literal(token: LexToken, spelling: &str) -> LiteralLexemeProjection {
    let components = literal_lexeme_components(token, spelling);
    let syntax = SyntaxLiteralSyntax::new(
        typed_literal_value(token, spelling, &components),
        SyntaxLiteralShape::new(
            literal_component(&components, LiteralLexemePart::Prefix).is_some(),
            literal_component(&components, LiteralLexemePart::Suffix).is_some(),
            literal_component(&components, LiteralLexemePart::Unit).is_some(),
        ),
        numeric_literal_digit_count(token, spelling, &components),
    );
    LiteralLexemeProjection { syntax, components }
}

/// Partitions one already-recognized literal token into authored components.
fn literal_lexeme_components(token: LexToken, spelling: &str) -> Vec<LiteralLexemeComponent> {
    if spelling.len() != token.range().end().saturating_sub(token.range().start()) {
        return Vec::new();
    }

    let mut output = Vec::new();
    match token.kind() {
        SyntaxKind::NumberToken => number_components(token, spelling, &mut output),
        SyntaxKind::RawStringToken => raw_string_components(token, spelling, &mut output),
        SyntaxKind::StringToken | SyntaxKind::CharacterToken => {
            quoted_components(token, spelling, &mut output);
        }
        SyntaxKind::UnterminatedStringToken => {
            unterminated_components(token, spelling, &mut output);
        }
        SyntaxKind::KeywordToken if matches!(spelling, "true" | "false") => {
            push_component(
                &mut output,
                token,
                LiteralLexemePart::Body,
                0,
                spelling.len(),
            );
        }
        _ => {}
    }
    output
}

fn number_components(token: LexToken, spelling: &str, output: &mut Vec<LiteralLexemeComponent>) {
    let (prefix_end, body_end) = number_body_bounds(spelling);
    if prefix_end > 0 {
        push_component(output, token, LiteralLexemePart::Prefix, 0, prefix_end);
    }
    push_component(output, token, LiteralLexemePart::Body, prefix_end, body_end);
    if body_end >= spelling.len() {
        return;
    }
    let suffix = &spelling[body_end..];
    let part = if UnitNumberSuffix::parse(suffix).is_some()
        || matches!(suffix, "ns" | "us" | "ms" | "s" | "min" | "h")
    {
        LiteralLexemePart::Unit
    } else {
        LiteralLexemePart::Suffix
    };
    push_component(output, token, part, body_end, spelling.len());
}

fn raw_string_components(
    token: LexToken,
    spelling: &str,
    output: &mut Vec<LiteralLexemeComponent>,
) {
    let quote = spelling
        .as_bytes()
        .iter()
        .position(|byte| *byte == b'"')
        .unwrap_or(spelling.len());
    push_component(output, token, LiteralLexemePart::Prefix, 0, quote);
    let hashes = quote.saturating_sub('r'.len_utf8());
    let body_start = quote.saturating_add('"'.len_utf8()).min(spelling.len());
    let close_width = '"'.len_utf8().saturating_add(hashes);
    let body_end = spelling
        .len()
        .checked_sub(close_width)
        .filter(|close| {
            spelling.as_bytes().get(*close) == Some(&b'"')
                && spelling.as_bytes()[*close + 1..]
                    .iter()
                    .all(|byte| *byte == b'#')
        })
        .unwrap_or(spelling.len());
    push_component(output, token, LiteralLexemePart::Body, body_start, body_end);
}

fn quoted_components(token: LexToken, spelling: &str, output: &mut Vec<LiteralLexemeComponent>) {
    let body_start = '"'.len_utf8().min(spelling.len());
    let suffix_width = usize::from(token.kind() == SyntaxKind::CharacterToken);
    let body_end = spelling.len().saturating_sub('"'.len_utf8() + suffix_width);
    push_component(output, token, LiteralLexemePart::Body, body_start, body_end);
    if token.kind() == SyntaxKind::CharacterToken {
        push_component(
            output,
            token,
            LiteralLexemePart::Suffix,
            spelling.len().saturating_sub('c'.len_utf8()),
            spelling.len(),
        );
    }
}

fn unterminated_components(
    token: LexToken,
    spelling: &str,
    output: &mut Vec<LiteralLexemeComponent>,
) {
    if spelling.starts_with('r') {
        let quote = spelling
            .as_bytes()
            .iter()
            .position(|byte| *byte == b'"')
            .unwrap_or(spelling.len());
        push_component(output, token, LiteralLexemePart::Prefix, 0, quote);
        push_component(
            output,
            token,
            LiteralLexemePart::Body,
            quote.saturating_add(1).min(spelling.len()),
            spelling.len(),
        );
    } else {
        push_component(
            output,
            token,
            LiteralLexemePart::Body,
            '"'.len_utf8().min(spelling.len()),
            spelling.len(),
        );
    }
}

fn push_component(
    output: &mut Vec<LiteralLexemeComponent>,
    token: LexToken,
    part: LiteralLexemePart,
    start: usize,
    end: usize,
) {
    output.push(LiteralLexemeComponent {
        part,
        range: token_local_range(token, start, end),
    });
}

/// Projects one lexer-owned literal token into its typed semantic value.
///
/// This consumes only the token spelling and the component ranges produced by
/// the same lexer. Pattern and expression owners therefore share one decoder
/// without reparsing source substrings or maintaining a second literal grammar.
fn typed_literal_value(
    token: LexToken,
    spelling: &str,
    components: &[LiteralLexemeComponent],
) -> SyntaxLiteralValue {
    match token.kind() {
        SyntaxKind::KeywordToken => match spelling {
            "true" => SyntaxLiteralValue::Bool(true),
            "false" => SyntaxLiteralValue::Bool(false),
            _ => unreachable!("literal projection receives only boolean keywords"),
        },
        SyntaxKind::StringToken => {
            let body = literal_component_text(token, spelling, components, LiteralLexemePart::Body);
            match decode_quoted_literal(body) {
                Ok(value) => SyntaxLiteralValue::String {
                    kind: SyntaxStringKind::Quoted,
                    value: value.into_boxed_str(),
                },
                Err(QuotedLiteralIssue::InvalidEscape { attempted }) => invalid(
                    SyntaxLiteralIssue::String(SyntaxStringIssue::InvalidEscape { attempted }),
                ),
            }
        }
        SyntaxKind::RawStringToken => SyntaxLiteralValue::String {
            kind: SyntaxStringKind::Raw,
            value: literal_component_text(token, spelling, components, LiteralLexemePart::Body)
                .into(),
        },
        SyntaxKind::UnterminatedStringToken => invalid(SyntaxLiteralIssue::String(
            SyntaxStringIssue::Unterminated {
                attempted: spelling.into(),
            },
        )),
        SyntaxKind::CharacterToken => {
            let body = literal_component_text(token, spelling, components, LiteralLexemePart::Body);
            match decode_quoted_literal(body) {
                Ok(value) => {
                    let mut characters = value.chars();
                    match (characters.next(), characters.next()) {
                        (Some(value), None) => SyntaxLiteralValue::Character(value),
                        (None, None) => {
                            invalid(SyntaxLiteralIssue::Character(SyntaxCharacterIssue::Empty {
                                attempted: body.into(),
                            }))
                        }
                        (Some(_), Some(_)) => invalid(SyntaxLiteralIssue::Character(
                            SyntaxCharacterIssue::MultipleScalars {
                                attempted: body.into(),
                            },
                        )),
                        (None, Some(_)) => unreachable!("second scalar requires a first scalar"),
                    }
                }
                Err(QuotedLiteralIssue::InvalidEscape { attempted }) => {
                    invalid(SyntaxLiteralIssue::Character(
                        SyntaxCharacterIssue::InvalidEscape { attempted },
                    ))
                }
            }
        }
        SyntaxKind::NumberToken => numeric_literal_value(token, spelling, components),
        _ => unreachable!("typed literal projection receives only literal token families"),
    }
}

/// Counts radix-valid numeric digits from lexer-owned literal components.
///
/// The body component has already excluded the radix prefix and suffix/unit;
/// separators and decimal punctuation are ignored here. This is the sole
/// syntax-side accounting input for downstream numeric hard-limit preflight.
fn numeric_literal_digit_count(
    token: LexToken,
    spelling: &str,
    components: &[LiteralLexemeComponent],
) -> Option<usize> {
    (token.kind() == SyntaxKind::NumberToken).then(|| {
        let prefix = literal_component(components, LiteralLexemePart::Prefix)
            .map(|component| literal_component_source(token, spelling, component));
        let body = literal_component_text(token, spelling, components, LiteralLexemePart::Body);
        let radix = match prefix {
            Some("0b" | "0B") => 2,
            Some("0o" | "0O") => 8,
            Some("0x" | "0X") => 16,
            Some(_) | None => 10,
        };
        body.bytes()
            .filter(|byte| char::from(*byte).is_digit(radix))
            .count()
    })
}

fn numeric_literal_value(
    token: LexToken,
    spelling: &str,
    components: &[LiteralLexemeComponent],
) -> SyntaxLiteralValue {
    let prefix = literal_component(components, LiteralLexemePart::Prefix)
        .map(|component| literal_component_source(token, spelling, component));
    let body = literal_component_text(token, spelling, components, LiteralLexemePart::Body);
    let suffix = literal_component(components, LiteralLexemePart::Suffix)
        .map(|component| literal_component_source(token, spelling, component));
    let unit = literal_component(components, LiteralLexemePart::Unit)
        .map(|component| literal_component_source(token, spelling, component));

    if prefix.is_some() {
        return integer_literal_value(spelling, prefix, body, suffix, unit);
    }

    if let Some(unit) = unit {
        if let Some(unit) = duration_unit(unit) {
            return decimal_literal_value(body, None).map_or_else(
                |issue| {
                    invalid(SyntaxLiteralIssue::Duration(SyntaxDurationIssue::Decimal(
                        issue,
                    )))
                },
                |value| SyntaxLiteralValue::Duration { value, unit },
            );
        }
        if let Some(unit) = UnitNumberSuffix::parse(unit) {
            return decimal_literal_value(body, None).map_or_else(
                |issue| {
                    invalid(SyntaxLiteralIssue::UnitNumber(
                        SyntaxUnitNumberIssue::Decimal(issue),
                    ))
                },
                |value| SyntaxLiteralValue::Unit { value, unit },
            );
        }
        unreachable!("the lexer emits Unit components only for exact typed units");
    }

    let float_suffix = suffix.and_then(FloatSuffix::parse);
    if body.contains(['.', 'e', 'E']) || float_suffix.is_some() {
        let value = match decimal_literal_value(body, float_suffix) {
            Ok(value) => value,
            Err(issue) => {
                return invalid(SyntaxLiteralIssue::Decimal(SyntaxDecimalIssue::Decimal(
                    issue,
                )));
            }
        };
        if let Some(suffix) = suffix
            && float_suffix.is_none()
        {
            return invalid(SyntaxLiteralIssue::Decimal(
                SyntaxDecimalIssue::InvalidSuffix {
                    suffix: suffix.into(),
                },
            ));
        }
        return SyntaxLiteralValue::Decimal(value);
    }

    integer_literal_value(spelling, prefix, body, suffix, unit)
}

fn integer_literal_value(
    spelling: &str,
    prefix: Option<&str>,
    body: &str,
    suffix: Option<&str>,
    unit: Option<&str>,
) -> SyntaxLiteralValue {
    let radix = match prefix {
        Some("0b" | "0B") => IntRadix::Binary,
        Some("0o" | "0O") => IntRadix::Octal,
        Some("0x" | "0X") => IntRadix::Hexadecimal,
        Some(other) => {
            return invalid(SyntaxLiteralIssue::Integer(
                SyntaxIntegerIssue::InvalidDigits {
                    attempted: other.into(),
                },
            ));
        }
        None => IntRadix::Decimal,
    };
    if unit.is_some() {
        return invalid(SyntaxLiteralIssue::Integer(
            SyntaxIntegerIssue::InvalidDigits {
                attempted: spelling.into(),
            },
        ));
    }
    let int_suffix = match suffix {
        Some(suffix) => match IntSuffix::parse(suffix) {
            Some(suffix) => Some(suffix),
            None => {
                return invalid(SyntaxLiteralIssue::Integer(
                    SyntaxIntegerIssue::InvalidDigits {
                        attempted: suffix.into(),
                    },
                ));
            }
        },
        None => None,
    };
    let body = if int_suffix.is_some() {
        body.strip_suffix('_').unwrap_or(body)
    } else {
        body
    };
    match normalize_literal_digits(body, radix.base()) {
        Ok(digits) => {
            SyntaxLiteralValue::Integer(SyntaxIntegerLiteral::new(radix, digits, int_suffix))
        }
        Err(issue) => invalid(SyntaxLiteralIssue::Integer(integer_issue(issue))),
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum LiteralDigitIssue {
    Missing { attempted: Box<str> },
    Invalid { attempted: Box<str> },
    InvalidSeparator { attempted: Box<str> },
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum QuotedLiteralIssue {
    InvalidEscape { attempted: Box<str> },
}

const fn invalid(issue: SyntaxLiteralIssue) -> SyntaxLiteralValue {
    SyntaxLiteralValue::Invalid(issue)
}

fn integer_issue(issue: LiteralDigitIssue) -> SyntaxIntegerIssue {
    match issue {
        LiteralDigitIssue::Missing { attempted } => SyntaxIntegerIssue::MissingDigits { attempted },
        LiteralDigitIssue::Invalid { attempted } => SyntaxIntegerIssue::InvalidDigits { attempted },
        LiteralDigitIssue::InvalidSeparator { attempted } => {
            SyntaxIntegerIssue::InvalidSeparator { attempted }
        }
    }
}

fn decimal_component_issue(
    issue: LiteralDigitIssue,
    coefficient: bool,
) -> SyntaxDecimalComponentIssue {
    match issue {
        LiteralDigitIssue::Missing { attempted } if coefficient => {
            SyntaxDecimalComponentIssue::MissingCoefficient { attempted }
        }
        LiteralDigitIssue::Missing { attempted } | LiteralDigitIssue::Invalid { attempted } => {
            SyntaxDecimalComponentIssue::InvalidDigits { attempted }
        }
        LiteralDigitIssue::InvalidSeparator { attempted } => {
            SyntaxDecimalComponentIssue::InvalidSeparator { attempted }
        }
    }
}

fn decimal_literal_value(
    body: &str,
    suffix: Option<FloatSuffix>,
) -> Result<SyntaxDecimalLiteral, SyntaxDecimalComponentIssue> {
    let (mantissa, exponent) = body.find(['e', 'E']).map_or((body, None), |index| {
        (&body[..index], Some(&body[index + 1..]))
    });
    let (integral, fractional) = mantissa
        .split_once('.')
        .map_or((mantissa, None), |(left, right)| (left, Some(right)));
    let integral = normalize_literal_digits(integral, 10)
        .map_err(|issue| decimal_component_issue(issue, true))?;
    let fractional = fractional
        .map(|digits| {
            normalize_literal_digits(digits, 10)
                .map_err(|issue| decimal_component_issue(issue, false))
        })
        .transpose()?;
    let exponent = exponent
        .map(|value| {
            let (negative, digits) = match value.as_bytes().first() {
                Some(b'-') => (true, &value[1..]),
                Some(b'+') => (false, &value[1..]),
                _ => (false, value),
            };
            Ok(SyntaxDecimalExponent::new(
                negative,
                normalize_literal_digits(digits, 10)
                    .map_err(|issue| decimal_component_issue(issue, false))?,
            ))
        })
        .transpose()?;
    Ok(SyntaxDecimalLiteral::new(
        integral, fractional, exponent, suffix,
    ))
}

fn normalize_literal_digits(source: &str, radix: u32) -> Result<Box<str>, LiteralDigitIssue> {
    if source.is_empty() {
        return Err(LiteralDigitIssue::Missing {
            attempted: source.into(),
        });
    }
    if source.starts_with('_') || source.ends_with('_') || source.contains("__") {
        return Err(LiteralDigitIssue::InvalidSeparator {
            attempted: source.into(),
        });
    }
    if !source
        .chars()
        .all(|character| character == '_' || character.is_digit(radix))
    {
        return Err(LiteralDigitIssue::Invalid {
            attempted: source.into(),
        });
    }
    Ok(source
        .chars()
        .filter(|character| *character != '_')
        .collect::<String>()
        .into_boxed_str())
}

fn decode_quoted_literal(source: &str) -> Result<String, QuotedLiteralIssue> {
    let mut decoded = String::with_capacity(source.len());
    let mut characters = source.chars();
    while let Some(character) = characters.next() {
        if character != '\\' {
            decoded.push(character);
            continue;
        }
        match characters.next() {
            Some('"') => decoded.push('"'),
            Some('\\') => decoded.push('\\'),
            Some('n') => decoded.push('\n'),
            Some('r') => decoded.push('\r'),
            Some('t') => decoded.push('\t'),
            Some('0') => decoded.push('\0'),
            Some('u') => decode_unicode_literal_escape(&mut characters, &mut decoded, source)?,
            _ => {
                return Err(QuotedLiteralIssue::InvalidEscape {
                    attempted: source.into(),
                });
            }
        }
    }
    Ok(decoded)
}

fn decode_unicode_literal_escape(
    characters: &mut core::str::Chars<'_>,
    decoded: &mut String,
    attempted: &str,
) -> Result<(), QuotedLiteralIssue> {
    if characters.next() != Some('{') {
        return Err(QuotedLiteralIssue::InvalidEscape {
            attempted: attempted.into(),
        });
    }
    let mut digits = String::new();
    let mut closed = false;
    for character in characters.by_ref() {
        if character == '}' {
            closed = true;
            break;
        }
        if character != '_' {
            digits.push(character);
        }
    }
    let scalar = closed
        .then(|| u32::from_str_radix(&digits, 16).ok())
        .flatten()
        .and_then(char::from_u32)
        .ok_or_else(|| QuotedLiteralIssue::InvalidEscape {
            attempted: attempted.into(),
        })?;
    decoded.push(scalar);
    Ok(())
}

fn duration_unit(source: &str) -> Option<DurationUnit> {
    Some(match source {
        "ns" => DurationUnit::Nanos,
        "us" => DurationUnit::Micros,
        "ms" => DurationUnit::Millis,
        "s" => DurationUnit::Seconds,
        "min" => DurationUnit::Minutes,
        "h" => DurationUnit::Hours,
        _ => return None,
    })
}

fn literal_component(
    components: &[LiteralLexemeComponent],
    part: LiteralLexemePart,
) -> Option<&LiteralLexemeComponent> {
    components.iter().find(|component| component.part() == part)
}

fn literal_component_text<'a>(
    token: LexToken,
    spelling: &'a str,
    components: &[LiteralLexemeComponent],
    part: LiteralLexemePart,
) -> &'a str {
    literal_component(components, part).map_or("", |component| {
        literal_component_source(token, spelling, component)
    })
}

fn literal_component_source<'a>(
    token: LexToken,
    spelling: &'a str,
    component: &LiteralLexemeComponent,
) -> &'a str {
    let start = component
        .range()
        .start()
        .saturating_sub(token.range().start());
    let end = component
        .range()
        .end()
        .saturating_sub(token.range().start());
    spelling.get(start..end).unwrap_or("")
}
