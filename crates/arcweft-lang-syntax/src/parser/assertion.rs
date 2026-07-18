//! Reserved assertion-statement recognition and checked argument parsing.

use crate::assertion::{AssertionMode, AssertionStmt};
use crate::ast::common::TextRange;
use crate::cst::{find_matching_punctuation, split_top_level_punctuation};
use crate::expr::{Expr, parse_expr_at};

use super::recovery::ParseErrorKind;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct AssertionParseError {
    kind: ParseErrorKind,
    range: TextRange,
    message: &'static str,
}

impl AssertionParseError {
    pub(super) const fn kind(&self) -> ParseErrorKind {
        self.kind
    }

    pub(super) const fn range(&self) -> TextRange {
        self.range
    }

    pub(super) const fn message(&self) -> &'static str {
        self.message
    }
}

pub(super) fn assertion_statement_candidate(source: &str) -> bool {
    let Some(mut cursor) = assertion_keyword_end(source) else {
        return false;
    };
    let Ok(next) = skip_horizontal_trivia(source, cursor, 0) else {
        return true;
    };
    cursor = next;
    source[cursor..].starts_with('.')
}

pub(super) fn parse_assertion_statement(
    source: &str,
    base: usize,
) -> Result<AssertionStmt, AssertionParseError> {
    let cursor = assertion_keyword_end(source).expect("candidate has exact assert keyword");
    let (mode, mode_range, cursor) = parse_assertion_mode(source, cursor, base)?;
    let (conditions, open, close) = parse_assertion_conditions(source, cursor, base)?;

    Ok(AssertionStmt::new(
        mode,
        conditions,
        TextRange::new(base, base + close + 1),
        TextRange::new(base, mode_range.end()),
        mode_range,
        TextRange::new(base + open, base + close + 1),
    ))
}

fn parse_assertion_mode(
    source: &str,
    cursor: usize,
    base: usize,
) -> Result<(AssertionMode, TextRange, usize), AssertionParseError> {
    let mut cursor = skip_horizontal_trivia(source, cursor, base)?;
    if !source[cursor..].starts_with('.') {
        return Err(error(
            ParseErrorKind::AssertionUnknownMode,
            base + cursor,
            base + cursor,
            "assertion statement requires `.prove`, `.check`, or `.debug`",
        ));
    }
    cursor += 1;
    cursor = skip_horizontal_trivia(source, cursor, base)?;
    let mode_start = cursor;
    while let Some(ch) = source[cursor..].chars().next()
        && (ch == '_' || ch.is_alphanumeric())
    {
        cursor += ch.len_utf8();
    }
    let mode_range = TextRange::new(base + mode_start, base + cursor);
    let Some(mode) = AssertionMode::from_keyword(&source[mode_start..cursor]) else {
        return Err(AssertionParseError {
            kind: ParseErrorKind::AssertionUnknownMode,
            range: mode_range,
            message: "unknown assertion mode",
        });
    };
    cursor = skip_horizontal_trivia(source, cursor, base)?;
    Ok((mode, mode_range, cursor))
}

fn parse_assertion_conditions(
    source: &str,
    cursor: usize,
    base: usize,
) -> Result<(Vec<Expr>, usize, usize), AssertionParseError> {
    if !source[cursor..].starts_with('(') {
        return Err(error(
            ParseErrorKind::AssertionInvalidArgument,
            base + cursor,
            base + cursor,
            "assertion mode must be followed by an argument list on the same logical line",
        ));
    }
    let open = cursor;
    let Some(close) = find_matching_punctuation(source, open, '(', ')') else {
        return Err(error(
            ParseErrorKind::AssertionUnclosedArguments,
            base + source.len(),
            base + source.len(),
            "assertion argument list is missing `)`",
        ));
    };
    if let Some(non_trivia) = source[close + 1..]
        .char_indices()
        .find_map(|(offset, ch)| (!ch.is_whitespace()).then_some(close + 1 + offset))
    {
        return Err(error(
            ParseErrorKind::AssertionInvalidArgument,
            base + non_trivia,
            base + source.len(),
            "unexpected tokens after assertion statement",
        ));
    }

    let interior = &source[open + 1..close];
    let parts = split_top_level_punctuation(interior, ',');
    if parts.is_empty() {
        return Err(error(
            ParseErrorKind::AssertionEmptyConditions,
            base + open + 1,
            base + open + 1,
            "assertion requires at least one condition",
        ));
    }
    if parts.len() > 64 {
        return Err(error(
            ParseErrorKind::AssertionTooManyConditions,
            base + open + 1,
            base + close,
            "assertion accepts at most 64 conditions",
        ));
    }

    let mut conditions = Vec::with_capacity(parts.len());
    let mut search_start = 0;
    for part in parts {
        let Some(relative) = interior[search_start..].find(part) else {
            return Err(error(
                ParseErrorKind::AssertionInvalidArgument,
                base + open + 1 + search_start,
                base + open + 1 + search_start,
                "assertion argument could not be reconciled with source tokens",
            ));
        };
        let start = search_start + relative;
        let end = start + part.len();
        if part.is_empty() {
            return Err(error(
                ParseErrorKind::AssertionInvalidArgument,
                base + open + 1 + start,
                base + open + 1 + start,
                "assertion conditions must be separated by one comma",
            ));
        }
        let condition = parse_expr_at(part, base + open + 1 + start).map_err(|_| {
            error(
                ParseErrorKind::AssertionInvalidArgument,
                base + open + 1 + start,
                base + open + 1 + end,
                "invalid assertion condition",
            )
        })?;
        conditions.push(condition);
        search_start = end;
    }
    Ok((conditions, open, close))
}

fn assertion_keyword_end(source: &str) -> Option<usize> {
    source
        .strip_prefix("assert")
        .filter(|tail| {
            tail.chars()
                .next()
                .is_none_or(|ch| ch == '.' || ch.is_whitespace() || ch == '/')
        })
        .map(|_| "assert".len())
}

fn skip_horizontal_trivia(
    source: &str,
    mut cursor: usize,
    base: usize,
) -> Result<usize, AssertionParseError> {
    loop {
        while matches!(source[cursor..].chars().next(), Some(' ' | '\t')) {
            cursor += 1;
        }
        if source[cursor..].starts_with("/*") {
            let start = cursor;
            let Some(close) = source[cursor + 2..].find("*/") else {
                return Err(error(
                    ParseErrorKind::AssertionInvalidArgument,
                    base + start,
                    base + source.len(),
                    "unclosed comment in assertion callee",
                ));
            };
            let end = cursor + 2 + close + 2;
            if source[cursor..end].contains('\n') {
                return Err(error(
                    ParseErrorKind::AssertionInvalidArgument,
                    base + start,
                    base + end,
                    "assertion callee cannot contain a physical newline",
                ));
            }
            cursor = end;
            continue;
        }
        return Ok(cursor);
    }
}

const fn error(
    kind: ParseErrorKind,
    start: usize,
    end: usize,
    message: &'static str,
) -> AssertionParseError {
    AssertionParseError {
        kind,
        range: TextRange::new(start, end),
        message,
    }
}

#[cfg(test)]
mod tests {
    use super::parse_assertion_statement;
    use crate::assertion::AssertionMode;
    use crate::ast::common::TextRange;
    use crate::expr::Expr;
    use crate::parser::recovery::ParseErrorKind;
    use crate::reference::BorrowKind;

    #[test]
    fn parses_modes_conditions_trailing_comma_and_ranges() {
        let parsed =
            parse_assertion_statement("assert.check(first, second,)", 4).expect("assertion parses");
        assert_eq!(parsed.mode(), AssertionMode::Check);
        assert_eq!(parsed.conditions().len(), 2);
        assert_eq!(parsed.range(), TextRange::new(4, 32));
        assert_eq!(parsed.callee_range(), TextRange::new(4, 16));
        assert_eq!(parsed.mode_range(), TextRange::new(11, 16));
        assert_eq!(parsed.arguments_range(), TextRange::new(16, 32));
    }

    #[test]
    fn empty_unknown_unclosed_and_over_limit_are_typed() {
        let empty = parse_assertion_statement("assert.prove()", 0).expect_err("empty fails");
        assert_eq!(empty.kind(), ParseErrorKind::AssertionEmptyConditions);
        assert_eq!(empty.kind().code(), "syntax.assert.empty_conditions");
        assert_eq!(empty.range(), TextRange::new(13, 13));
        let unknown =
            parse_assertion_statement("assert.assume(value)", 0).expect_err("unknown fails");
        assert_eq!(unknown.kind(), ParseErrorKind::AssertionUnknownMode);
        assert_eq!(unknown.kind().code(), "syntax.assert.unknown_mode");
        assert_eq!(unknown.range(), TextRange::new(7, 13));
        let unclosed =
            parse_assertion_statement("assert.debug(value", 0).expect_err("unclosed fails");
        assert_eq!(unclosed.kind(), ParseErrorKind::AssertionUnclosedArguments);
        assert_eq!(unclosed.kind().code(), "syntax.assert.unclosed_arguments");
        assert_eq!(unclosed.range(), TextRange::new(18, 18));
        let invalid =
            parse_assertion_statement("assert.debug", 0).expect_err("missing arguments fail");
        assert_eq!(invalid.kind(), ParseErrorKind::AssertionInvalidArgument);
        assert_eq!(invalid.kind().code(), "syntax.assert.invalid_argument");
        assert_eq!(invalid.range(), TextRange::new(12, 12));
        let at_limit = core::iter::repeat_n("true", 64)
            .collect::<Vec<_>>()
            .join(",");
        assert_eq!(
            parse_assertion_statement(&format!("assert.check({at_limit})"), 0)
                .expect("the inclusive maximum succeeds")
                .conditions()
                .len(),
            64
        );
        let over = parse_assertion_statement(&format!("assert.check({at_limit},true)"), 0)
            .expect_err("over limit fails");
        assert_eq!(over.kind(), ParseErrorKind::AssertionTooManyConditions);
        assert_eq!(over.kind().code(), "syntax.assert.too_many_conditions");
        assert_eq!(
            over.range(),
            TextRange::new(13, 13 + at_limit.len() + ",true".len())
        );
    }

    #[test]
    fn condition_prefix_ranges_remain_document_absolute() {
        let parsed = parse_assertion_statement("assert.check(&mut value)", 9)
            .expect("assertion with borrow condition parses");
        let [Expr::Borrow(borrow)] = parsed.conditions() else {
            panic!("expected one borrow condition");
        };
        assert_eq!(borrow.kind(), BorrowKind::Mutable);
        assert_eq!(borrow.operator_range(), TextRange::new(22, 26));
    }
}
