//! Parser and recovery for leading View-part export declarations.

use crate::ast::common::TextRange;
use crate::ast::view::{ViewPartExportDecl, ViewPartLabelSyntax, ViewPartNameSyntax};

use super::super::headers::simple_error;
use super::super::recovery::ParseError;
use super::ViewSourceLine;

struct ParsedExportNames<'a> {
    local: &'a str,
    local_range: TextRange,
    as_range: TextRange,
    public: &'a str,
    public_range: TextRange,
}

pub(super) fn is_export_candidate(line: &str) -> bool {
    line == "export"
        || line.starts_with("export ")
        || line.starts_with("exportparts")
        || line.starts_with("export_part")
}

pub(super) fn parse_export(
    line: &ViewSourceLine,
    errors: &mut Vec<ParseError>,
) -> Option<ViewPartExportDecl> {
    let tokens = tokens(&line.text, line.start);
    let token = |index: usize| tokens.get(index).copied();

    if token(0).is_none_or(|(text, _)| text != "export")
        || token(1).is_none_or(|(text, _)| text != "part")
    {
        errors.push(export_error(
            TextRange::new(line.start, line.end),
            "view::unsupported_export_spelling",
            "View part exports use `export part local as public`",
            "export part local as public",
        ));
        return None;
    }

    let names = parse_export_names(&tokens, line.end, errors)?;
    if let Some((trailing, trailing_range)) = token(5) {
        let duplicate_as = trailing == "as" || tokens.iter().skip(5).any(|(text, _)| *text == "as");
        errors.push(export_error(
            TextRange::new(trailing_range.start(), line.end),
            if duplicate_as {
                "view::export_part_duplicate_as"
            } else {
                "view::export_part_trailing_syntax"
            },
            if duplicate_as {
                "View part export contains more than one `as`"
            } else {
                "View part export has trailing syntax"
            },
            "end of declaration",
        ));
        return None;
    }

    Some(ViewPartExportDecl::new(
        ViewPartNameSyntax::new(names.local.to_owned(), names.local_range),
        ViewPartNameSyntax::new(names.public.to_owned(), names.public_range),
        token(0).expect("checked above").1,
        token(1).expect("checked above").1,
        names.as_range,
        TextRange::new(line.start, line.end),
    ))
}

fn parse_export_names<'a>(
    tokens: &[(&'a str, TextRange)],
    line_end: usize,
    errors: &mut Vec<ParseError>,
) -> Option<ParsedExportNames<'a>> {
    let token = |index: usize| tokens.get(index).copied();
    let Some((local, local_range)) = token(2) else {
        errors.push(export_error(
            TextRange::new(line_end, line_end),
            "view::export_part_missing_local",
            "View part export needs a private local target name",
            "local part name",
        ));
        return None;
    };
    if local == "as" || !valid_name(local) {
        let missing = local == "as";
        errors.push(export_error(
            local_range,
            if missing {
                "view::export_part_missing_local"
            } else {
                "view::export_part_invalid_local_name"
            },
            if missing {
                "View part export needs a private local target before `as`"
            } else {
                "View part export target must be an unqualified dotted name"
            },
            "local part name",
        ));
        return None;
    }

    let Some((as_keyword, as_range)) = token(3) else {
        errors.push(export_error(
            TextRange::new(line_end, line_end),
            "view::export_part_missing_as",
            "View part export needs `as` before its public name",
            "as public_name",
        ));
        return None;
    };
    if as_keyword != "as" {
        errors.push(export_error(
            as_range,
            "view::export_part_missing_as",
            "View part export needs `as` before its public name",
            "as public_name",
        ));
        return None;
    }

    let Some((public, public_range)) = token(4) else {
        errors.push(export_error(
            TextRange::new(line_end, line_end),
            "view::export_part_missing_public",
            "View part export needs a public capability name",
            "public part name",
        ));
        return None;
    };
    if !valid_name(public) {
        errors.push(export_error(
            public_range,
            "view::export_part_invalid_public_name",
            "View part export public name must be an unqualified dotted name",
            "public part name",
        ));
        return None;
    }

    Some(ParsedExportNames {
        local,
        local_range,
        as_range,
        public,
        public_range,
    })
}

fn export_error(range: TextRange, code: &str, message: &str, expected: &str) -> ParseError {
    simple_error(
        range.start(),
        range.end().saturating_sub(range.start()),
        message,
        expected,
    )
    .with_code(code)
}

pub(super) fn parse_label(
    value: &str,
    line: &str,
    line_range: TextRange,
    errors: &mut Vec<ParseError>,
) -> Option<ViewPartLabelSyntax> {
    let value = value.trim();
    let value_offset = line.find(value).unwrap_or_default();
    let value_range = TextRange::new(
        line_range.start().saturating_add(value_offset),
        line_range
            .start()
            .saturating_add(value_offset + value.len()),
    );
    if value.is_empty() {
        errors.push(
            simple_error(
                value_range.start(),
                0,
                "View `.part(...)` needs one private local name",
                ".part(local_name)",
            )
            .with_code("view::part_missing_name"),
        );
        return None;
    }
    if let Some(trailing_offset) = value.find(char::is_whitespace) {
        let trailing_start = value[trailing_offset..]
            .find(|character: char| !character.is_whitespace())
            .map_or(value.len(), |offset| trailing_offset + offset);
        errors.push(
            simple_error(
                value_range.start().saturating_add(trailing_start),
                value.len().saturating_sub(trailing_start),
                "View `.part(...)` has trailing syntax",
                ".part(local_name)",
            )
            .with_code("view::part_trailing_syntax"),
        );
        return None;
    }
    if !valid_name(value) {
        errors.push(
            simple_error(
                value_range.start(),
                value_range.end().saturating_sub(value_range.start()),
                "View `.part(...)` needs one unqualified dotted name",
                ".part(local_name)",
            )
            .with_code("view::part_invalid_local_name"),
        );
        return None;
    }
    Some(ViewPartLabelSyntax::new(
        ViewPartNameSyntax::new(value.to_owned(), value_range),
        line_range,
    ))
}

fn tokens(line: &str, base: usize) -> Vec<(&str, TextRange)> {
    let mut result = Vec::new();
    let mut start = None;
    for (index, character) in line.char_indices() {
        if character.is_whitespace() {
            if let Some(token_start) = start.take() {
                result.push((
                    &line[token_start..index],
                    TextRange::new(token_start, index),
                ));
            }
        } else if start.is_none() {
            start = Some(index);
        }
    }
    if let Some(token_start) = start {
        result.push((
            &line[token_start..],
            TextRange::new(token_start, line.len()),
        ));
    }
    result
        .into_iter()
        .map(|(token, range)| {
            (
                token,
                TextRange::new(base + range.start(), base + range.end()),
            )
        })
        .collect()
}

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && !matches!(name.chars().next(), Some('@' | '.' | '"' | '\''))
        && name.split('.').all(|segment| {
            let mut characters = segment.chars();
            characters
                .next()
                .is_some_and(|first| first == '_' || first.is_alphabetic())
                && characters.all(|character| character == '_' || character.is_alphanumeric())
        })
}
