//! Parser and recovery for leading View-part export declarations.

use crate::ast::common::TextRange;
use crate::ast::view::{
    ViewPartExportDecl, ViewPartLocalNameSyntax, ViewPartModifier, ViewPartNameSyntax,
};
use arcweft_source::{SourceDocument, SourceRange, SourceSpan};

use super::super::recovery::{ParseError, ParseErrorKind, RecoverySuggestion};
use super::ViewSourceLine;

struct ParsedExportNames<'a> {
    local: &'a str,
    local_range: TextRange,
    as_range: TextRange,
    public: &'a str,
    public_range: TextRange,
}

pub(super) fn is_export_candidate(line: &str) -> bool {
    line == "export" || line.starts_with("export ")
}

pub(super) fn parse_export(
    line: &ViewSourceLine,
    document: &SourceDocument,
    errors: &mut Vec<ParseError>,
) -> Option<ViewPartExportDecl> {
    let declaration_text = line
        .text
        .split_once("//")
        .map_or(line.text.as_str(), |(declaration, _)| declaration)
        .trim_end();
    let declaration_end = line.start.saturating_add(declaration_text.len());
    let tokens = tokens(declaration_text, line.start);
    let token = |index: usize| tokens.get(index).copied();

    if token(0).is_none_or(|(text, _)| text != "export")
        || token(1).is_none_or(|(text, _)| text != "part")
    {
        let expected = "part local as public";
        errors.push(ParseError::new_with_kind(
            ParseErrorKind::ViewExportPartMissingPart,
            TextRange::new(line.start, line.end),
            vec![expected.to_owned()],
            None,
            "View part export needs `part` before its private local target".to_owned(),
            vec![RecoverySuggestion::new(format!("use {expected} syntax"))],
        ));
        return None;
    }

    let names = parse_export_names(&tokens, declaration_end, errors)?;
    if let Some((trailing, trailing_range)) = token(5) {
        let duplicate_as = trailing == "as" || tokens.iter().skip(5).any(|(text, _)| *text == "as");
        let expected = "end of declaration";
        errors.push(ParseError::new_with_kind(
            if duplicate_as {
                ParseErrorKind::ViewExportPartDuplicateAs
            } else {
                ParseErrorKind::ViewExportPartTrailingSyntax
            },
            TextRange::new(trailing_range.start(), declaration_end),
            vec![expected.to_owned()],
            None,
            if duplicate_as {
                "View part export contains more than one `as`"
            } else {
                "View part export has trailing syntax"
            }
            .to_owned(),
            vec![RecoverySuggestion::new(format!("use {expected} syntax"))],
        ));
        return None;
    }

    Some(ViewPartExportDecl::new(
        ViewPartLocalNameSyntax::new(
            names.local.to_owned(),
            source_span(document, names.local_range),
        ),
        ViewPartNameSyntax::new(
            names.public.to_owned(),
            source_span(document, names.public_range),
        ),
        source_span(
            document,
            TextRange::new(line.start, names.public_range.end()),
        ),
        source_span(document, token(0).expect("checked above").1),
        source_span(document, token(1).expect("checked above").1),
        source_span(document, names.as_range),
    ))
}

fn parse_export_names<'a>(
    tokens: &[(&'a str, TextRange)],
    line_end: usize,
    errors: &mut Vec<ParseError>,
) -> Option<ParsedExportNames<'a>> {
    let token = |index: usize| tokens.get(index).copied();
    let Some((local, local_range)) = token(2) else {
        let expected = "local part name";
        errors.push(ParseError::new_with_kind(
            ParseErrorKind::ViewExportPartMissingLocal,
            TextRange::new(line_end, line_end),
            vec![expected.to_owned()],
            None,
            "View part export needs a private local target name".to_owned(),
            vec![RecoverySuggestion::new(format!("use {expected} syntax"))],
        ));
        return None;
    };
    if local == "as" || !valid_name(local) {
        let missing = local == "as";
        let expected = "local part name";
        errors.push(ParseError::new_with_kind(
            if missing {
                ParseErrorKind::ViewExportPartMissingLocal
            } else {
                ParseErrorKind::ViewExportPartInvalidLocalName
            },
            local_range,
            vec![expected.to_owned()],
            None,
            if missing {
                "View part export needs a private local target before `as`"
            } else {
                "View part export target must be an unqualified dotted name"
            }
            .to_owned(),
            vec![RecoverySuggestion::new(format!("use {expected} syntax"))],
        ));
        return None;
    }

    let Some((as_keyword, as_range)) = token(3) else {
        let expected = "as public_name";
        errors.push(ParseError::new_with_kind(
            ParseErrorKind::ViewExportPartMissingAs,
            TextRange::new(line_end, line_end),
            vec![expected.to_owned()],
            None,
            "View part export needs `as` before its public name".to_owned(),
            vec![RecoverySuggestion::new(format!("use {expected} syntax"))],
        ));
        return None;
    };
    if as_keyword != "as" {
        let expected = "as public_name";
        errors.push(ParseError::new_with_kind(
            ParseErrorKind::ViewExportPartMissingAs,
            as_range,
            vec![expected.to_owned()],
            None,
            "View part export needs `as` before its public name".to_owned(),
            vec![RecoverySuggestion::new(format!("use {expected} syntax"))],
        ));
        return None;
    }

    let Some((public, public_range)) = token(4) else {
        let expected = "public part name";
        errors.push(ParseError::new_with_kind(
            ParseErrorKind::ViewExportPartMissingPublic,
            TextRange::new(line_end, line_end),
            vec![expected.to_owned()],
            None,
            "View part export needs a public capability name".to_owned(),
            vec![RecoverySuggestion::new(format!("use {expected} syntax"))],
        ));
        return None;
    };
    if !valid_name(public) {
        let expected = "public part name";
        errors.push(ParseError::new_with_kind(
            ParseErrorKind::ViewExportPartInvalidPublicName,
            public_range,
            vec![expected.to_owned()],
            None,
            "View part export public name must be an unqualified dotted name".to_owned(),
            vec![RecoverySuggestion::new(format!("use {expected} syntax"))],
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

pub(super) fn parse_label(
    value: &str,
    line: &str,
    line_range: TextRange,
    document: &SourceDocument,
    errors: &mut Vec<ParseError>,
) -> Option<ViewPartModifier> {
    let value = value.trim();
    let operand_start = line
        .find('(')
        .map_or(0, |open_paren| open_paren.saturating_add(1));
    let value_offset = operand_start + line[operand_start..].find(value).unwrap_or_default();
    let value_range = TextRange::new(
        line_range.start().saturating_add(value_offset),
        line_range
            .start()
            .saturating_add(value_offset + value.len()),
    );
    if value.is_empty() {
        let expected = ".part(local_name)";
        errors.push(ParseError::new_with_kind(
            ParseErrorKind::ViewPartMissingName,
            TextRange::new(value_range.start(), value_range.start()),
            vec![expected.to_owned()],
            None,
            "View `.part(...)` needs one private local name".to_owned(),
            vec![RecoverySuggestion::new(format!("use {expected} syntax"))],
        ));
        return None;
    }
    if let Some(trailing_offset) = value.find(char::is_whitespace) {
        let trailing_start = value[trailing_offset..]
            .find(|character: char| !character.is_whitespace())
            .map_or(value.len(), |offset| trailing_offset + offset);
        let expected = ".part(local_name)";
        errors.push(ParseError::new_with_kind(
            ParseErrorKind::ViewPartTrailingSyntax,
            TextRange::new(
                value_range.start().saturating_add(trailing_start),
                value_range.end(),
            ),
            vec![expected.to_owned()],
            None,
            "View `.part(...)` has trailing syntax".to_owned(),
            vec![RecoverySuggestion::new(format!("use {expected} syntax"))],
        ));
        return None;
    }
    if !valid_name(value) {
        let expected = ".part(local_name)";
        errors.push(ParseError::new_with_kind(
            ParseErrorKind::ViewPartInvalidLocalName,
            value_range,
            vec![expected.to_owned()],
            None,
            "View `.part(...)` needs one unqualified dotted name".to_owned(),
            vec![RecoverySuggestion::new(format!("use {expected} syntax"))],
        ));
        return None;
    }
    Some(ViewPartModifier::new(
        ViewPartLocalNameSyntax::new(value.to_owned(), source_span(document, value_range)),
        source_span(document, line_range),
    ))
}

fn source_span(document: &SourceDocument, range: TextRange) -> SourceSpan {
    document
        .span(SourceRange::new(range.start(), range.end()))
        .expect("parser ranges are UTF-8 boundaries within the owning source document")
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
        && name.len() <= 255
        && !name.contains("::")
        && name.split('.').count() <= 32
        && name.split('.').all(|segment| {
            let mut characters = segment.chars();
            characters
                .next()
                .is_some_and(|first| first == '_' || first.is_alphabetic())
                && characters.all(|character| {
                    character == '_' || character == '-' || character.is_alphanumeric()
                })
        })
}
