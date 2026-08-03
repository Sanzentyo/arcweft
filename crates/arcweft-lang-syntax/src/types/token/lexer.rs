//! Standalone-source adapter for the shared type-token grammar.

use super::{TypeToken, TypeTokenKind};
use crate::ast::common::TextRange;
use crate::name::{is_identifier_continue, is_identifier_start};
use crate::types::TypeParseError;

pub(super) fn lex_source(source: &str, base: usize) -> Result<Vec<TypeToken<'_>>, TypeParseError> {
    let mut tokens = Vec::new();
    let mut cursor = 0usize;
    while cursor < source.len() {
        if let Some(next) = trivia_end(source, base, cursor)? {
            cursor = next;
            continue;
        }
        let start = cursor;
        let kind = lex_token(source, base, &mut cursor)?;
        tokens.push(TypeToken::from_parser(
            kind,
            TextRange::new(base + start, base + cursor),
        ));
    }
    Ok(tokens)
}

/// Returns the first byte after trivia, or `None` when the cursor starts on a
/// semantic token.
fn trivia_end(source: &str, base: usize, cursor: usize) -> Result<Option<usize>, TypeParseError> {
    let ch = source[cursor..]
        .chars()
        .next()
        .expect("cursor remains at a UTF-8 boundary");
    if ch.is_whitespace() {
        return Ok(Some(cursor + ch.len_utf8()));
    }
    if source[cursor..].starts_with("//") {
        return Ok(Some(
            source[cursor..]
                .find('\n')
                .map_or(source.len(), |newline| cursor + newline + 1),
        ));
    }
    if source[cursor..].starts_with("/*") {
        let Some(close) = source[cursor + 2..].find("*/") else {
            return Err(TypeParseError::at(
                "syntax.type.invalid",
                "unclosed comment in type",
                TextRange::new(base + cursor, base + source.len()),
            ));
        };
        return Ok(Some(cursor + 2 + close + 2));
    }
    Ok(None)
}

fn lex_token<'source>(
    source: &'source str,
    base: usize,
    cursor: &mut usize,
) -> Result<TypeTokenKind<'source>, TypeParseError> {
    let start = *cursor;
    let ch = source[start..]
        .chars()
        .next()
        .expect("cursor remains at a UTF-8 boundary");
    if source[start..].starts_with("->") {
        *cursor += 2;
        return Ok(TypeTokenKind::ThinArrow);
    }
    if source[start..].starts_with("::") {
        *cursor += 2;
        return Ok(TypeTokenKind::PathSeparator);
    }
    let kind = match ch {
        '!' => single(cursor, ch, TypeTokenKind::Bang),
        '&' => single(cursor, ch, TypeTokenKind::Ampersand),
        '(' => single(cursor, ch, TypeTokenKind::OpenParen),
        ')' => single(cursor, ch, TypeTokenKind::CloseParen),
        '[' => single(cursor, ch, TypeTokenKind::OpenBracket),
        ']' => single(cursor, ch, TypeTokenKind::CloseBracket),
        '{' => single(cursor, ch, TypeTokenKind::OpenBrace),
        '}' => single(cursor, ch, TypeTokenKind::CloseBrace),
        '<' => single(cursor, ch, TypeTokenKind::OpenAngle),
        '>' => single(cursor, ch, TypeTokenKind::CloseAngle),
        ',' => single(cursor, ch, TypeTokenKind::Comma),
        '.' => single(cursor, ch, TypeTokenKind::Dot),
        ':' => single(cursor, ch, TypeTokenKind::Colon),
        '=' => single(cursor, ch, TypeTokenKind::Equals),
        '|' => single(cursor, ch, TypeTokenKind::Pipe),
        '\'' => {
            *cursor += ch.len_utf8();
            let name_start = *cursor;
            take_while(source, cursor, is_identifier_continue);
            if name_start == *cursor {
                return Err(TypeParseError::at(
                    "syntax.type.invalid",
                    "expected lifetime name after apostrophe",
                    TextRange::new(base + start, base + *cursor),
                ));
            }
            TypeTokenKind::Lifetime(&source[start..*cursor])
        }
        '0'..='9' => {
            *cursor += ch.len_utf8();
            take_while(source, cursor, |next| next.is_ascii_digit());
            TypeTokenKind::Integer(&source[start..*cursor])
        }
        _ if is_identifier_start(ch) => {
            *cursor += ch.len_utf8();
            take_while(source, cursor, is_identifier_continue);
            TypeTokenKind::Identifier(&source[start..*cursor])
        }
        _ => {
            *cursor += ch.len_utf8();
            return Err(TypeParseError::at(
                "syntax.type.invalid",
                "invalid token in type",
                TextRange::new(base + start, base + *cursor),
            ));
        }
    };
    Ok(kind)
}

fn take_while(source: &str, cursor: &mut usize, predicate: impl Fn(char) -> bool) {
    while let Some(next) = source[*cursor..].chars().next()
        && predicate(next)
    {
        *cursor += next.len_utf8();
    }
}

fn single<'source>(
    cursor: &mut usize,
    ch: char,
    kind: TypeTokenKind<'source>,
) -> TypeTokenKind<'source> {
    *cursor += ch.len_utf8();
    kind
}
