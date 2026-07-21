//! Dedicated parser for top-level and inline Style syntax.

use super::{
    ParseError, Parser,
    headers::{
        DeclEntityId, parse_required_decl_entity_ref_or_marker, parse_visibility_prefix,
        simple_error, slice_offset,
    },
    recovery::{ParseErrorKind, RecoverySuggestion},
};
use crate::{
    ast::{
        common::TextRange,
        ids::EntityRef,
        style::{
            StyleAssignOp, StyleBodyItem, StyleCombinator, StyleDecl, StyleDeclarationDecl,
            StyleEnvironmentBlock, StyleEnvironmentClause, StyleEnvironmentComparisonSyntax,
            StyleEnvironmentFieldSyntax, StyleEnvironmentPercentageLiteral,
            StyleEnvironmentSourceRanges, StyleEnvironmentUnsupportedValue,
            StyleEnvironmentUnsupportedValueKind, StyleEnvironmentValueSyntax, StyleExpr,
            StyleName, StylePatch, StylePredicate, StyleRuleDecl, StyleSelector,
            StyleSelectorSequence, StyleSheet, StyleTokenDecl,
        },
    },
    expr::parse_expr,
    types::parse_type_ref,
};

impl Parser<'_> {
    pub(super) fn parse_style(&mut self) -> Option<StyleDecl> {
        let attrs = self.take_pending_attrs();
        let start_line = self.current().clone();
        let block = self.take_flow_block_event();
        if !block.ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing style declaration",
                ["}"],
                Some(start_line.text.trim()),
                ["insert a closing `}` for the style body"],
            );
            return None;
        }

        let head = block.head.trim();
        let (visibility, rest) = parse_visibility_prefix(head);
        let rest = rest.trim_start().strip_prefix("style")?.trim_start();
        let id_base = start_line.start + slice_offset(head, rest);
        let (id, trailing) = parse_style_decl_head(
            rest,
            id_base,
            self.current_module_path.as_deref(),
            &mut self.errors,
        )?;
        if !trailing.trim().is_empty() {
            self.push_error(
                TextRange::new(id.range().end(), start_line.end),
                "unexpected text after style declaration head",
                ["{"],
                None,
                ["move properties into the native Style body"],
            );
        }

        let body_range = block
            .body_range
            .as_ref()
            .map_or(TextRange::new(start_line.end, start_line.end), |range| {
                TextRange::new(range.start, range.end)
            });
        let sheet = parse_native_sheet(&block.body, body_range, &mut self.errors);
        Some(StyleDecl::new(
            attrs,
            visibility,
            id,
            sheet,
            TextRange::new(start_line.start, block.end),
        ))
    }
}

/// Parses a native inline style body through the same declaration parser used
/// by named sheets.
pub(crate) fn parse_inline_native_style(
    source: &str,
    range: TextRange,
    errors: &mut Vec<ParseError>,
) -> StylePatch {
    let mut parser = NativeStyleParser::new(source, range.start(), errors);
    let declarations = parser.parse_declarations(range.end(), StyleDeclarationContext::InlinePatch);
    StylePatch::new(declarations, range)
}

fn parse_native_sheet(source: &str, range: TextRange, errors: &mut Vec<ParseError>) -> StyleSheet {
    let mut parser = NativeStyleParser::new(source, range.start(), errors);
    let (tokens, body) = parser.parse_sheet(true);
    StyleSheet::new(tokens, body, range)
}

struct NativeStyleParser<'a, 'errors> {
    source: &'a str,
    base: usize,
    cursor: usize,
    errors: &'errors mut Vec<ParseError>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StyleDeclarationContext {
    InlinePatch,
    RuleBody,
}

struct StyleEnvironmentHead {
    when_range: TextRange,
    intrinsic_range: TextRange,
    predicate_range: TextRange,
    condition_closed: bool,
    clauses: Vec<StyleEnvironmentClause>,
    body_open: usize,
}

impl StyleDeclarationContext {
    const fn nested_rule_message(self) -> &'static str {
        match self {
            Self::InlinePatch => "inline native style cannot contain selector rules",
            Self::RuleBody => "native style rules cannot contain nested selector rules",
        }
    }

    const fn nested_rule_kind(self) -> ParseErrorKind {
        match self {
            Self::InlinePatch => ParseErrorKind::StyleInlineSelectorNotSupported,
            Self::RuleBody => ParseErrorKind::StyleMalformedSelector,
        }
    }
}

impl<'a, 'errors> NativeStyleParser<'a, 'errors> {
    const fn new(source: &'a str, base: usize, errors: &'errors mut Vec<ParseError>) -> Self {
        Self {
            source,
            base,
            cursor: 0,
            errors,
        }
    }

    fn parse_sheet(&mut self, allow_tokens: bool) -> (Vec<StyleTokenDecl>, Vec<StyleBodyItem>) {
        let mut tokens = Vec::new();
        let mut body = Vec::new();
        while self.skip_trivia() {
            let item_start = self.cursor;
            if self.starts_keyword("token") {
                if let Some(statement) = self.take_statement()
                    && let Some(token) = self.parse_token(statement, item_start)
                {
                    if allow_tokens {
                        tokens.push(token);
                    } else {
                        self.errors.push(ParseError::new_with_kind(
                            ParseErrorKind::StyleEnvironmentTokenNotAllowed,
                            token.range(),
                            Vec::new(),
                            None,
                            "style tokens are sheet-owned and cannot appear in an environment wrapper"
                                .to_owned(),
                            Vec::new(),
                        ));
                    }
                }
                continue;
            }
            if self.starts_environment_block() {
                if let Some(environment) = self.take_environment_block() {
                    body.push(StyleBodyItem::Environment(environment));
                }
                continue;
            }
            let error_count = self.errors.len();
            match self.take_rule() {
                Some(rule) => body.push(StyleBodyItem::Rule(rule)),
                None if self.errors.len() == error_count => self.recover_line(item_start),
                None => {}
            }
        }
        (tokens, body)
    }

    fn starts_environment_block(&self) -> bool {
        let Some(after_when) = self.source[self.cursor..].strip_prefix("when") else {
            return false;
        };
        if !after_when.starts_with(char::is_whitespace) {
            return false;
        }
        let after_when = after_when.trim_start();
        after_when.strip_prefix("environment").is_some_and(|tail| {
            tail.is_empty() || tail.starts_with(char::is_whitespace) || tail.starts_with('(')
        })
    }

    fn take_environment_block(&mut self) -> Option<StyleEnvironmentBlock> {
        let block_start = self.cursor;
        let head = self.take_environment_head(block_start)?;
        let Some(body_close) = matching_brace(self.source, head.body_open) else {
            self.errors.push(ParseError::new_with_kind(
                ParseErrorKind::StyleEnvironmentExpectedOpenBrace,
                TextRange::new(self.base + head.body_open, self.base + self.source.len()),
                Vec::new(),
                None,
                "unclosed environment style body".to_owned(),
                Vec::new(),
            ));
            self.cursor = self.source.len();
            return Some(StyleEnvironmentBlock::new(
                StyleEnvironmentSourceRanges::new(
                    head.when_range,
                    head.intrinsic_range,
                    head.predicate_range,
                    TextRange::new(
                        self.base + head.body_open + '{'.len_utf8(),
                        self.base + self.cursor,
                    ),
                    TextRange::new(self.base + block_start, self.base + self.cursor),
                ),
                head.condition_closed,
                head.clauses,
                Vec::new(),
            ));
        };
        let nested_start = head.body_open + '{'.len_utf8();
        let mut nested = NativeStyleParser::new(
            &self.source[nested_start..body_close],
            self.base + nested_start,
            self.errors,
        );
        let (_, body) = nested.parse_sheet(false);
        self.cursor = body_close + '}'.len_utf8();
        Some(StyleEnvironmentBlock::new(
            StyleEnvironmentSourceRanges::new(
                head.when_range,
                head.intrinsic_range,
                head.predicate_range,
                TextRange::new(self.base + nested_start, self.base + body_close),
                TextRange::new(self.base + block_start, self.base + self.cursor),
            ),
            head.condition_closed,
            head.clauses,
            body,
        ))
    }

    /// Parses the fixed `when environment(...) {` grammar prefix and leaves
    /// body parsing to `take_environment_block` so recovery remains local to
    /// the grammar component that owns each delimiter.
    fn take_environment_head(&mut self, block_start: usize) -> Option<StyleEnvironmentHead> {
        let when_range = TextRange::new(
            self.base + block_start,
            self.base + block_start + "when".len(),
        );
        self.cursor += "when".len();
        self.cursor +=
            self.source[self.cursor..].len() - self.source[self.cursor..].trim_start().len();
        let environment_start = self.cursor;
        let Some(after_environment) = self.source[self.cursor..].strip_prefix("environment") else {
            self.errors.push(ParseError::new_with_kind(
                ParseErrorKind::StyleEnvironmentExpectedField,
                TextRange::new(self.base + self.cursor, self.base + self.cursor),
                Vec::new(),
                None,
                "expected `environment` after `when`".to_owned(),
                Vec::new(),
            ));
            self.recover_line(block_start);
            return None;
        };
        let intrinsic_range = TextRange::new(
            self.base + environment_start,
            self.base + environment_start + "environment".len(),
        );
        self.cursor += self.source[self.cursor..].len() - after_environment.len();
        self.cursor +=
            self.source[self.cursor..].len() - self.source[self.cursor..].trim_start().len();
        if !self.source[self.cursor..].starts_with('(') {
            self.errors.push(ParseError::new_with_kind(
                ParseErrorKind::StyleEnvironmentExpectedOpenParen,
                TextRange::new(self.base + self.cursor, self.base + self.cursor),
                Vec::new(),
                None,
                "environment guard needs an opening `(`".to_owned(),
                Vec::new(),
            ));
            self.recover_line(block_start);
            return None;
        }
        let condition_open = self.cursor;
        let (condition_close, body_open) =
            self.take_environment_delimiters(block_start, condition_open)?;
        let clauses_end = condition_close;
        let clauses = parse_environment_clauses(
            &self.source[condition_open + 1..clauses_end],
            self.base + condition_open + 1,
            self.errors,
        );
        let predicate_range = TextRange::new(
            self.base + condition_open,
            self.base
                + if condition_close == body_open {
                    condition_close
                } else {
                    condition_close + ')'.len_utf8()
                },
        );
        let condition_closed = condition_close != body_open;
        Some(StyleEnvironmentHead {
            when_range,
            intrinsic_range,
            predicate_range,
            condition_closed,
            clauses,
            body_open,
        })
    }

    /// Resolves the condition/body delimiter pair while keeping malformed
    /// delimiter recovery at the environment-head grammar boundary.
    fn take_environment_delimiters(
        &mut self,
        block_start: usize,
        condition_open: usize,
    ) -> Option<(usize, usize)> {
        match environment_condition_boundary(self.source, condition_open) {
            EnvironmentConditionBoundary::CloseParen(close) => {
                let after_close = skip_style_trivia(self.source, close + ')'.len_utf8());
                if !self.source[after_close..].starts_with('{') {
                    self.errors.push(ParseError::new_with_kind(
                        ParseErrorKind::StyleEnvironmentExpectedOpenBrace,
                        TextRange::new(self.base + after_close, self.base + after_close),
                        Vec::new(),
                        None,
                        "environment guard needs an opening `{` after its condition".to_owned(),
                        Vec::new(),
                    ));
                    self.cursor = after_close;
                    self.recover_line(block_start);
                    return None;
                }
                Some((close, after_close))
            }
            EnvironmentConditionBoundary::OpenBrace(open) => {
                self.errors.push(ParseError::new_with_kind(
                    ParseErrorKind::StyleEnvironmentUnterminatedCondition,
                    TextRange::new(self.base + condition_open, self.base + open),
                    Vec::new(),
                    None,
                    "environment condition is missing a closing `)`".to_owned(),
                    Vec::new(),
                ));
                Some((open, open))
            }
            EnvironmentConditionBoundary::End => {
                self.errors.push(ParseError::new_with_kind(
                    ParseErrorKind::StyleEnvironmentUnterminatedCondition,
                    TextRange::new(self.base + condition_open, self.base + self.source.len()),
                    Vec::new(),
                    None,
                    "unterminated environment condition".to_owned(),
                    Vec::new(),
                ));
                self.cursor = self.source.len();
                None
            }
        }
    }

    fn parse_declarations(
        &mut self,
        range_end: usize,
        context: StyleDeclarationContext,
    ) -> Vec<StyleDeclarationDecl> {
        let mut declarations = Vec::new();
        while self.skip_trivia() {
            let start = self.cursor;
            if let Some(DeclarationBoundary::NestedRule(open)) =
                first_declaration_boundary(&self.source[start..])
            {
                let open = start + open;
                let end = matching_brace(self.source, open)
                    .map_or(self.source.len(), |close| close + '}'.len_utf8());
                let range = TextRange::new(self.base + start, (self.base + end).min(range_end));
                let expected = "extract the selector into a named `style` declaration";
                self.errors.push(ParseError::new_with_kind(
                    context.nested_rule_kind(),
                    range,
                    vec![expected.to_owned()],
                    None,
                    context.nested_rule_message().to_owned(),
                    vec![RecoverySuggestion::new(format!("use {expected} syntax"))],
                ));
                break;
            }
            let Some(statement) = self.take_statement() else {
                break;
            };
            if let Some(declaration) = self.parse_declaration(statement, start) {
                declarations.push(declaration);
            }
        }
        declarations
    }

    fn parse_token(&mut self, statement: &str, statement_start: usize) -> Option<StyleTokenDecl> {
        let body_offset = statement.find("token")? + "token".len();
        let body = statement[body_offset..].trim_start();
        let body_leading = statement[body_offset..].len() - body.len();
        let body_start = statement_start + body_offset + body_leading;
        let Some(equals) = find_top_level_char(body, '=') else {
            self.errors.push(simple_error(
                self.base + statement_start,
                statement.len(),
                "style token needs `=` before its initializer",
                "token color.text: Color = rgba(255, 255, 255, 255)",
            ));
            return None;
        };
        let head = body[..equals].trim();
        let value_source = body[equals + 1..].trim();
        if head.is_empty() || value_source.is_empty() {
            self.errors.push(simple_error(
                self.base + statement_start,
                statement.len(),
                "style token needs a name and initializer",
                "token color.text: Color = rgba(255, 255, 255, 255)",
            ));
            return None;
        }
        let (name, value_type) = head.split_once(':').map_or((head, None), |(name, ty)| {
            let ty = ty.trim();
            let parsed = parse_type_ref(ty)
                .map(|mut parsed| {
                    parsed.rebase(
                        self.base
                            + body_start
                            + body.find(head).unwrap_or_default()
                            + head.find(ty).unwrap_or_default(),
                    );
                    parsed
                })
                .map_err(|error| {
                    self.errors.push(simple_error(
                        self.base + body_start + head.find(':').unwrap_or_default() + 1,
                        ty.len(),
                        &format!("invalid style token type: {error}"),
                        "Color | Length | ShadowList | FilterList",
                    ));
                });
            (name.trim(), parsed.ok())
        });
        let value_offset = body.find(value_source)?;
        let value_range = TextRange::new(
            self.base + body_start + value_offset,
            self.base + body_start + value_offset + value_source.len(),
        );
        let value = self.parse_value(value_source, value_range)?;
        Some(StyleTokenDecl::new(
            name,
            value_type,
            value,
            TextRange::new(
                self.base + statement_start,
                self.base + statement_start + statement.len(),
            ),
        ))
    }

    fn take_rule(&mut self) -> Option<StyleRuleDecl> {
        let rule_start = self.cursor;
        let open = find_top_level_char(&self.source[rule_start..], '{')? + rule_start;
        let selector_source = self.source[rule_start..open].trim();
        let selector_leading =
            self.source[rule_start..open].len() - self.source[rule_start..open].trim_start().len();
        let selector_start = rule_start + selector_leading;
        let Some(close) = matching_brace(self.source, open) else {
            self.errors.push(simple_error(
                self.base + open,
                self.source.len().saturating_sub(open),
                "unclosed native style rule",
                "insert a closing `}` for the selector rule",
            ));
            self.cursor = self.source.len();
            return None;
        };
        self.cursor = close + '}'.len_utf8();
        let selector = parse_selector(selector_source, self.base + selector_start, self.errors)?;
        let body_start = open + 1;
        let body = &self.source[body_start..close];
        let mut body_parser = NativeStyleParser::new(body, self.base + body_start, self.errors);
        let declarations =
            body_parser.parse_declarations(self.base + close, StyleDeclarationContext::RuleBody);
        Some(StyleRuleDecl::new(
            selector,
            declarations,
            TextRange::new(self.base + rule_start, self.base + self.cursor),
        ))
    }

    fn parse_declaration(
        &mut self,
        statement: &str,
        statement_start: usize,
    ) -> Option<StyleDeclarationDecl> {
        let trimmed = statement.trim();
        let leading = statement.len() - statement.trim_start().len();
        let (op, body, body_offset) = trimmed.strip_prefix("append ").map_or(
            (StyleAssignOp::Replace, trimmed, leading),
            |body| {
                let body = body.trim_start();
                (
                    StyleAssignOp::Append,
                    body,
                    leading + "append ".len() + (trimmed["append ".len()..].len() - body.len()),
                )
            },
        );
        let Some(equals) = find_top_level_char(body, '=') else {
            self.errors.push(simple_error(
                self.base + statement_start,
                statement.len(),
                "style declaration needs `=`",
                "property-name = value",
            ));
            return None;
        };
        let property = body[..equals].trim();
        let value_source = body[equals + 1..].trim();
        if property.is_empty() || value_source.is_empty() {
            self.errors.push(simple_error(
                self.base + statement_start,
                statement.len(),
                "style declaration needs a property and value",
                "property-name = value",
            ));
            return None;
        }
        let property_offset = body_offset + body.find(property)?;
        let value_offset = body_offset + body.find(value_source)?;
        let property_range = TextRange::new(
            self.base + statement_start + property_offset,
            self.base + statement_start + property_offset + property.len(),
        );
        let value_range = TextRange::new(
            self.base + statement_start + value_offset,
            self.base + statement_start + value_offset + value_source.len(),
        );
        let value = self.parse_value(value_source, value_range)?;
        Some(StyleDeclarationDecl::new(
            StyleName::new(property, property_range),
            value,
            op,
            TextRange::new(
                self.base + statement_start,
                self.base + statement_start + statement.len(),
            ),
        ))
    }

    fn parse_value(&mut self, source: &str, range: TextRange) -> Option<StyleExpr> {
        match parse_expr(source) {
            Ok(expr) => Some(StyleExpr::new(expr, source, range)),
            Err(error) => {
                self.errors.push(simple_error(
                    range.start(),
                    range.end().saturating_sub(range.start()),
                    &format!("invalid style value expression: {error}"),
                    "a valid Arcweft expression",
                ));
                None
            }
        }
    }

    fn take_statement(&mut self) -> Option<&'a str> {
        let start = self.cursor;
        let end = statement_end(self.source, start);
        self.cursor = end;
        let statement = self.source[start..end]
            .trim_end_matches(['\r', '\n'])
            .trim_end();
        let statement = top_level_line_comment_start(statement)
            .map_or(statement, |comment| &statement[..comment])
            .trim_end();
        (!statement.trim().is_empty()).then_some(statement)
    }

    fn skip_trivia(&mut self) -> bool {
        loop {
            self.cursor += self.source[self.cursor..]
                .find(|ch: char| !ch.is_whitespace())
                .unwrap_or(self.source.len() - self.cursor);
            if self.cursor >= self.source.len() {
                return false;
            }
            if self.source[self.cursor..].starts_with("//") {
                self.cursor = self.source[self.cursor..]
                    .find('\n')
                    .map_or(self.source.len(), |offset| self.cursor + offset + 1);
                continue;
            }
            return true;
        }
    }

    fn starts_keyword(&self, keyword: &str) -> bool {
        self.source[self.cursor..]
            .strip_prefix(keyword)
            .is_some_and(|tail| tail.starts_with(char::is_whitespace))
    }

    fn recover_line(&mut self, start: usize) {
        let end = self.source[start..]
            .find('\n')
            .map_or(self.source.len(), |offset| start + offset + 1);
        self.errors.push(simple_error(
            self.base + start,
            end.saturating_sub(start),
            "invalid native style item",
            "token name: Type = value | Element:state { property = value }",
        ));
        self.cursor = end;
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EnvironmentConditionBoundary {
    CloseParen(usize),
    OpenBrace(usize),
    End,
}

fn environment_condition_boundary(source: &str, open: usize) -> EnvironmentConditionBoundary {
    let mut state = StyleDelimiterState::default();
    let mut chars = source[open..].char_indices().peekable();
    while let Some((offset, ch)) = chars.next() {
        let index = open + offset;
        if !state.observe(ch, chars.peek().map(|(_, next)| *next)) {
            continue;
        }
        if ch == '{' && state.delimiters == ['('] {
            return EnvironmentConditionBoundary::OpenBrace(index);
        }
        if ch == ')' && state.delimiters.last() == Some(&'(') {
            state.update_delimiters(ch);
            if state.is_top_level() {
                return EnvironmentConditionBoundary::CloseParen(index);
            }
            continue;
        }
        state.update_delimiters(ch);
    }
    EnvironmentConditionBoundary::End
}

fn skip_style_trivia(source: &str, mut cursor: usize) -> usize {
    loop {
        cursor += source[cursor..]
            .find(|ch: char| !ch.is_whitespace())
            .unwrap_or(source.len() - cursor);
        if cursor >= source.len() || !source[cursor..].starts_with("//") {
            return cursor;
        }
        cursor = source[cursor..]
            .find('\n')
            .map_or(source.len(), |offset| cursor + offset + 1);
    }
}

fn parse_environment_clauses(
    source: &str,
    base: usize,
    errors: &mut Vec<ParseError>,
) -> Vec<StyleEnvironmentClause> {
    let mut clauses = Vec::new();
    let mut state = StyleDelimiterState::default();
    let mut segment_start = 0;
    let mut chars = source.char_indices().peekable();
    while let Some((index, ch)) = chars.next() {
        if !state.observe(ch, chars.peek().map(|(_, next)| *next)) {
            continue;
        }
        if ch == ',' && state.is_top_level() {
            if let Some(clause) = parse_environment_clause(
                &source[segment_start..index],
                base + segment_start,
                errors,
            ) {
                clauses.push(clause);
            } else if !source[segment_start..index].trim().is_empty() {
                // The clause parser already emitted a precise diagnostic.
            } else if source[..index].trim().is_empty()
                || source[index + ','.len_utf8()..].trim().is_empty()
            {
                // Empty condition and a canonical trailing comma are handled below.
            } else {
                errors.push(ParseError::new_with_kind(
                    ParseErrorKind::StyleEnvironmentExpectedField,
                    TextRange::new(base + index, base + index),
                    Vec::new(),
                    None,
                    "environment condition contains an empty clause".to_owned(),
                    Vec::new(),
                ));
            }
            segment_start = index + ','.len_utf8();
            continue;
        }
        state.update_delimiters(ch);
    }
    if let Some(clause) =
        parse_environment_clause(&source[segment_start..], base + segment_start, errors)
    {
        clauses.push(clause);
    }
    if source.trim().is_empty() {
        errors.push(ParseError::new_with_kind(
            ParseErrorKind::StyleEnvironmentExpectedField,
            TextRange::new(base, base),
            Vec::new(),
            None,
            "environment condition needs at least one field".to_owned(),
            Vec::new(),
        ));
    }
    clauses
}

fn parse_environment_clause(
    source: &str,
    base: usize,
    errors: &mut Vec<ParseError>,
) -> Option<StyleEnvironmentClause> {
    let leading = source.len() - source.trim_start().len();
    let trimmed = source.trim();
    if trimmed.is_empty() {
        return None;
    }
    let clause_base = base + leading;
    let field_len = trimmed
        .find(|ch: char| ch.is_whitespace() || matches!(ch, '=' | '!' | '<' | '>'))
        .unwrap_or(trimmed.len());
    let field_source = &trimmed[..field_len];
    let field_range = TextRange::new(clause_base, clause_base + field_len);
    let field = match field_source {
        "color-scheme" => StyleEnvironmentFieldSyntax::ColorScheme,
        "contrast" => StyleEnvironmentFieldSyntax::Contrast,
        "reduced-motion" => StyleEnvironmentFieldSyntax::ReducedMotion,
        "text-scale" => StyleEnvironmentFieldSyntax::TextScale,
        _ => {
            errors.push(ParseError::new_with_kind(
                ParseErrorKind::StyleEnvironmentExpectedField,
                field_range,
                Vec::new(),
                None,
                "unknown presentation-environment field".to_owned(),
                Vec::new(),
            ));
            StyleEnvironmentFieldSyntax::Unknown
        }
    };

    let mut cursor = field_len;
    cursor += trimmed[cursor..].len() - trimmed[cursor..].trim_start().len();
    let comparison_start = cursor;
    cursor += trimmed[cursor..]
        .find(|ch: char| !matches!(ch, '=' | '!' | '<' | '>'))
        .unwrap_or(trimmed.len() - cursor);
    let comparison_source = &trimmed[comparison_start..cursor];
    let comparison_range = TextRange::new(clause_base + comparison_start, clause_base + cursor);
    let comparison = match comparison_source {
        "==" => StyleEnvironmentComparisonSyntax::Equal,
        "!=" => StyleEnvironmentComparisonSyntax::NotEqual,
        "<" => StyleEnvironmentComparisonSyntax::Less,
        "<=" => StyleEnvironmentComparisonSyntax::LessOrEqual,
        ">" => StyleEnvironmentComparisonSyntax::Greater,
        ">=" => StyleEnvironmentComparisonSyntax::GreaterOrEqual,
        _ => {
            errors.push(ParseError::new_with_kind(
                ParseErrorKind::StyleEnvironmentExpectedComparison,
                comparison_range,
                Vec::new(),
                None,
                "environment clause needs one comparison token".to_owned(),
                Vec::new(),
            ));
            StyleEnvironmentComparisonSyntax::Unsupported
        }
    };

    cursor += trimmed[cursor..].len() - trimmed[cursor..].trim_start().len();
    let value_source = trimmed[cursor..].trim_end();
    let value_range = TextRange::new(
        clause_base + cursor,
        clause_base + cursor + value_source.len(),
    );
    let value = parse_environment_value(value_source, value_range);
    if let StyleEnvironmentValueSyntax::Unsupported(unsupported) = &value {
        let kind = match unsupported.kind() {
            StyleEnvironmentUnsupportedValueKind::Missing => {
                ParseErrorKind::StyleEnvironmentExpectedValue
            }
            StyleEnvironmentUnsupportedValueKind::TrailingTokens => {
                ParseErrorKind::StyleEnvironmentExpectedCommaOrCloseParen
            }
            _ => ParseErrorKind::StyleEnvironmentUnsupportedValue,
        };
        errors.push(ParseError::new_with_kind(
            kind,
            unsupported.range(),
            Vec::new(),
            None,
            environment_unsupported_value_message(unsupported.kind()).to_owned(),
            Vec::new(),
        ));
    }
    Some(StyleEnvironmentClause::new(
        field,
        comparison,
        value,
        field_range,
        comparison_range,
        value_range,
        TextRange::new(clause_base, clause_base + trimmed.len()),
    ))
}

fn parse_environment_value(source: &str, range: TextRange) -> StyleEnvironmentValueSyntax {
    if source.is_empty() {
        return unsupported_environment_value(StyleEnvironmentUnsupportedValueKind::Missing, range);
    }
    if source
        .chars()
        .any(|ch| matches!(ch, '(' | ')' | '[' | ']' | '{' | '}'))
    {
        return unsupported_environment_value(
            StyleEnvironmentUnsupportedValueKind::NestedDelimiter,
            range,
        );
    }
    if source.starts_with('"') || source.starts_with('\'') {
        return unsupported_environment_value(
            StyleEnvironmentUnsupportedValueKind::StringLiteral,
            range,
        );
    }
    if source.chars().any(char::is_whitespace) {
        return unsupported_environment_value(
            StyleEnvironmentUnsupportedValueKind::TrailingTokens,
            range,
        );
    }
    if matches!(source, "true" | "false") {
        return StyleEnvironmentValueSyntax::Boolean {
            value: source == "true",
            range,
        };
    }
    if (source.starts_with('+') || source.starts_with('-')) && source.ends_with('%') {
        return unsupported_environment_value(
            StyleEnvironmentUnsupportedValueKind::SignedPercentage,
            range,
        );
    }
    if source.ends_with('%') && source.bytes().any(|byte| matches!(byte, b'e' | b'E')) {
        return unsupported_environment_value(
            StyleEnvironmentUnsupportedValueKind::ExponentPercentage,
            range,
        );
    }
    if let Some(number) = source.strip_suffix('%') {
        let mut parts = number.split('.');
        let integer = parts.next().unwrap_or_default();
        let fractional = parts.next();
        if parts.next().is_some()
            || integer.is_empty()
            || !integer.bytes().all(|byte| byte.is_ascii_digit())
            || fractional.is_some_and(|digits| {
                digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit())
            })
        {
            return unsupported_environment_value(
                StyleEnvironmentUnsupportedValueKind::MalformedPercentage,
                range,
            );
        }
        if fractional.is_some_and(|digits| digits.len() > 1) {
            return unsupported_environment_value(
                StyleEnvironmentUnsupportedValueKind::FractionalPrecision,
                range,
            );
        }
        let integer_range = TextRange::new(range.start(), range.start() + integer.len());
        let fractional_range = fractional.map(|digits| {
            let start = integer_range.end() + '.'.len_utf8();
            TextRange::new(start, start + digits.len())
        });
        let percent_start = range.end() - '%'.len_utf8();
        return StyleEnvironmentValueSyntax::Percentage(StyleEnvironmentPercentageLiteral::new(
            integer_range,
            fractional_range,
            TextRange::new(percent_start, range.end()),
            range,
        ));
    }
    if is_unsigned_decimal(source) {
        return unsupported_environment_value(
            StyleEnvironmentUnsupportedValueKind::IntegerWithoutPercent,
            range,
        );
    }
    if is_environment_identifier(source) {
        return StyleEnvironmentValueSyntax::Identifier { range };
    }
    unsupported_environment_value(
        StyleEnvironmentUnsupportedValueKind::UnknownIdentifier,
        range,
    )
}

fn is_unsigned_decimal(source: &str) -> bool {
    let mut parts = source.split('.');
    let integer = parts.next().unwrap_or_default();
    let fractional = parts.next();
    parts.next().is_none()
        && !integer.is_empty()
        && integer.bytes().all(|byte| byte.is_ascii_digit())
        && fractional.is_none_or(|digits| {
            !digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit())
        })
}

fn is_environment_identifier(source: &str) -> bool {
    source
        .bytes()
        .next()
        .is_some_and(|byte| byte.is_ascii_alphabetic() || byte == b'_')
        && source
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

fn unsupported_environment_value(
    kind: StyleEnvironmentUnsupportedValueKind,
    range: TextRange,
) -> StyleEnvironmentValueSyntax {
    StyleEnvironmentValueSyntax::Unsupported(StyleEnvironmentUnsupportedValue::new(kind, range))
}

const fn environment_unsupported_value_message(
    kind: StyleEnvironmentUnsupportedValueKind,
) -> &'static str {
    match kind {
        StyleEnvironmentUnsupportedValueKind::Missing => "environment clause needs a value",
        StyleEnvironmentUnsupportedValueKind::UnknownIdentifier => {
            "environment value is not a supported identifier"
        }
        StyleEnvironmentUnsupportedValueKind::StringLiteral => {
            "environment values cannot be string literals"
        }
        StyleEnvironmentUnsupportedValueKind::IntegerWithoutPercent => {
            "text-scale values need a trailing `%`"
        }
        StyleEnvironmentUnsupportedValueKind::SignedPercentage => {
            "text-scale percentages cannot have a sign"
        }
        StyleEnvironmentUnsupportedValueKind::ExponentPercentage => {
            "text-scale percentages cannot use exponent notation"
        }
        StyleEnvironmentUnsupportedValueKind::FractionalPrecision => {
            "text-scale percentages allow at most one fractional digit"
        }
        StyleEnvironmentUnsupportedValueKind::MalformedPercentage => {
            "malformed text-scale percentage"
        }
        StyleEnvironmentUnsupportedValueKind::NestedDelimiter => {
            "environment values cannot contain nested delimiters"
        }
        StyleEnvironmentUnsupportedValueKind::TrailingTokens => {
            "environment values cannot contain trailing tokens"
        }
    }
}

fn parse_selector(
    source: &str,
    base: usize,
    errors: &mut Vec<ParseError>,
) -> Option<StyleSelector> {
    if source.is_empty() {
        errors.push(simple_error(
            base,
            0,
            "style selector cannot be empty",
            "Button:hover",
        ));
        return None;
    }
    let mut sequences = Vec::new();
    let mut cursor = 0;
    let mut relation = None;
    while cursor < source.len() {
        let whitespace = source[cursor..]
            .find(|ch: char| !ch.is_whitespace())
            .unwrap_or(source.len() - cursor);
        if whitespace > 0 && !sequences.is_empty() && relation.is_none() {
            relation = Some(StyleCombinator::Descendant);
        }
        cursor += whitespace;
        if cursor >= source.len() {
            break;
        }
        if source[cursor..].starts_with('>') {
            if sequences.is_empty() || relation == Some(StyleCombinator::Child) {
                errors.push(simple_error(
                    base + cursor,
                    1,
                    "style selector has a leading or repeated child combinator",
                    "Parent > Child",
                ));
                return None;
            }
            relation = Some(StyleCombinator::Child);
            cursor += 1;
            continue;
        }
        let end = source[cursor..]
            .find(|ch: char| ch.is_whitespace() || ch == '>')
            .map_or(source.len(), |offset| cursor + offset);
        let compound = &source[cursor..end];
        let sequence = parse_selector_compound(compound, base + cursor, relation, errors)?;
        sequences.push(sequence);
        relation = None;
        cursor = end;
    }
    if relation.is_some() {
        errors.push(simple_error(
            base + source.len().saturating_sub(1),
            1,
            "style selector cannot end with a combinator",
            "Parent > Child",
        ));
        return None;
    }
    Some(StyleSelector::new(
        sequences,
        TextRange::new(base, base + source.len()),
    ))
}

fn parse_selector_compound(
    source: &str,
    base: usize,
    relation: Option<StyleCombinator>,
    errors: &mut Vec<ParseError>,
) -> Option<StyleSelectorSequence> {
    let predicate_start = source.find(':').unwrap_or(source.len());
    let head = &source[..predicate_start];
    let mut element = None;
    let mut part = None;
    if let Some(part_name) = head.strip_prefix('.') {
        if part_name.is_empty() {
            errors.push(simple_error(
                base,
                head.len(),
                "style part name cannot be empty",
                ".label",
            ));
            return None;
        }
        part = Some(StyleName::new(
            part_name,
            TextRange::new(base + 1, base + head.len()),
        ));
    } else if let Some(dot) = head.find('.') {
        let element_name = &head[..dot];
        let part_name = &head[dot + 1..];
        if element_name.is_empty() || part_name.is_empty() {
            errors.push(simple_error(
                base,
                head.len(),
                "malformed style element/part selector",
                "Button.label",
            ));
            return None;
        }
        element = Some(StyleName::new(
            element_name,
            TextRange::new(base, base + dot),
        ));
        part = Some(StyleName::new(
            part_name,
            TextRange::new(base + dot + 1, base + head.len()),
        ));
    } else if !head.is_empty() {
        element = Some(StyleName::new(
            head,
            TextRange::new(base, base + head.len()),
        ));
    }

    let mut predicates = Vec::new();
    let mut offset = predicate_start;
    while offset < source.len() {
        let Some(rest) = source[offset..].strip_prefix(':') else {
            break;
        };
        let name_start = offset + 1;
        let next = rest
            .find(':')
            .map_or(source.len(), |next| name_start + next);
        let name = &source[name_start..next];
        if name.is_empty() {
            errors.push(simple_error(
                base + offset,
                1,
                "style predicate name cannot be empty",
                ":hover",
            ));
            return None;
        }
        predicates.push(StylePredicate::new(
            name,
            TextRange::new(base + name_start, base + next),
        ));
        offset = next;
    }
    if element.is_none() && part.is_none() {
        errors.push(simple_error(
            base,
            source.len(),
            "style selector sequence needs an element or part",
            "Button:hover | .label",
        ));
        return None;
    }
    Some(StyleSelectorSequence::new(
        relation,
        element,
        part,
        predicates,
        TextRange::new(base, base + source.len()),
    ))
}

fn parse_style_decl_head(
    input: &str,
    base: usize,
    module_path: Option<&str>,
    errors: &mut Vec<ParseError>,
) -> Option<(EntityRef, String)> {
    let input = input.trim_start();
    let (id, tail) = if input.starts_with('@') {
        let (parsed, rest) =
            parse_required_decl_entity_ref_or_marker(input, "style", base, errors)?;
        match parsed {
            DeclEntityId::Entity(entity) => {
                let (entity, rest) = normalize_style_decl_colon(entity, rest);
                (
                    rebase_relative_style_decl_entity(entity, input, module_path),
                    rest,
                )
            }
            DeclEntityId::NameMarker(marker) => {
                let rest = rest.trim_start();
                let (name, tail) = parse_style_name_and_tail(rest);
                let Some(name) = name else {
                    errors.push(simple_error(
                        marker.range.start(),
                        marker.range.end() - marker.range.start(),
                        "relative style declaration marker needs a following style name",
                        "@style:. primary_button",
                    ));
                    return None;
                };
                (
                    EntityRef::new(
                        style_decl_body(&name, module_path),
                        false,
                        TextRange::new(marker.range.end(), marker.range.end() + name.len()),
                    ),
                    tail,
                )
            }
        }
    } else {
        let (name, tail) = parse_style_name_and_tail(input);
        let Some(name) = name else {
            errors.push(simple_error(
                base,
                input.len(),
                "style declaration needs a canonical style name or declaration id",
                "style primary_button",
            ));
            return None;
        };
        let start = input
            .find(&name)
            .map_or(base, |offset| base.saturating_add(offset));
        (
            EntityRef::new(
                style_decl_body(&name, module_path),
                false,
                TextRange::new(start, start + name.len()),
            ),
            tail,
        )
    };
    Some((id, tail))
}

fn normalize_style_decl_colon(entity: EntityRef, rest: &str) -> (EntityRef, String) {
    if entity.is_delimited() || !entity.body().ends_with(':') {
        return (entity, rest.to_owned());
    }
    let body = entity.body().trim_end_matches(':').to_owned();
    let range = TextRange::new(entity.range().start(), entity.range().end() - 1);
    (
        EntityRef::new(body, false, range),
        format!(": {}", rest.trim_start()),
    )
}

fn style_decl_body(name: &str, module_path: Option<&str>) -> String {
    EntityRef::module_scoped_declaration_body("style", name, module_path)
}

fn rebase_relative_style_decl_entity(
    entity: EntityRef,
    source: &str,
    module_path: Option<&str>,
) -> EntityRef {
    if !(source.starts_with("@.") || source.starts_with("@style:.")) {
        return entity;
    }
    let Some(suffix) = entity.body().strip_prefix("style.") else {
        return entity;
    };
    EntityRef::new(style_decl_body(suffix, module_path), false, *entity.range())
}

fn parse_style_name_and_tail(input: &str) -> (Option<String>, String) {
    let trimmed = input.trim_start();
    let Some((first, mut tail)) = crate::cst::split_leading_ident(trimmed) else {
        return (None, trimmed.to_owned());
    };
    let mut name = first.to_owned();
    while let Some(after_dot) = tail.strip_prefix('.') {
        let Some((segment, next_tail)) = crate::cst::split_leading_ident(after_dot) else {
            break;
        };
        name.push('.');
        name.push_str(segment);
        tail = next_tail;
    }
    (Some(name), tail.trim().to_owned())
}

fn statement_end(source: &str, start: usize) -> usize {
    let mut state = StyleDelimiterState::default();
    let mut chars = source[start..].char_indices().peekable();
    while let Some((offset, ch)) = chars.next() {
        let index = start + offset;
        if !state.observe(ch, chars.peek().map(|(_, next)| *next)) {
            continue;
        }
        if ch == '\n' && state.is_top_level() {
            return index + '\n'.len_utf8();
        }
        state.update_delimiters(ch);
    }
    source.len()
}

fn find_top_level_char(source: &str, needle: char) -> Option<usize> {
    let mut state = StyleDelimiterState::default();
    let mut chars = source.char_indices().peekable();
    while let Some((index, ch)) = chars.next() {
        if !state.observe(ch, chars.peek().map(|(_, next)| *next)) {
            continue;
        }
        if ch == needle && state.is_top_level() {
            return Some(index);
        }
        state.update_delimiters(ch);
    }
    None
}

fn matching_brace(source: &str, open: usize) -> Option<usize> {
    let mut state = StyleDelimiterState::default();
    let mut chars = source[open..].char_indices().peekable();
    while let Some((offset, ch)) = chars.next() {
        let index = open + offset;
        if !state.observe(ch, chars.peek().map(|(_, next)| *next)) {
            continue;
        }
        if ch == '}' && state.delimiters.last() == Some(&'{') {
            state.update_delimiters(ch);
            if state.is_top_level() {
                return Some(index);
            }
            continue;
        }
        state.update_delimiters(ch);
    }
    None
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DeclarationBoundary {
    Assignment,
    NestedRule(usize),
}

fn first_declaration_boundary(source: &str) -> Option<DeclarationBoundary> {
    let mut state = StyleDelimiterState::default();
    let mut chars = source.char_indices().peekable();
    while let Some((index, ch)) = chars.next() {
        if !state.observe(ch, chars.peek().map(|(_, next)| *next)) {
            continue;
        }
        if state.is_top_level() {
            match ch {
                '=' => return Some(DeclarationBoundary::Assignment),
                '{' => return Some(DeclarationBoundary::NestedRule(index)),
                '\n' => return None,
                _ => {}
            }
        }
        state.update_delimiters(ch);
    }
    None
}

fn top_level_line_comment_start(source: &str) -> Option<usize> {
    let mut state = StyleDelimiterState::default();
    let mut chars = source.char_indices().peekable();
    while let Some((index, ch)) = chars.next() {
        let next = chars.peek().map(|(_, next)| *next);
        if ch == '/' && next == Some('/') && state.quote.is_none() && state.is_top_level() {
            return Some(index);
        }
        if state.observe(ch, next) {
            state.update_delimiters(ch);
        }
    }
    None
}

/// Tracks the punctuation that separates native Style items without treating
/// braces inside expressions, strings, or comments as selector boundaries.
#[derive(Default)]
struct StyleDelimiterState {
    delimiters: Vec<char>,
    quote: Option<char>,
    escaped: bool,
    line_comment: bool,
}

impl StyleDelimiterState {
    fn observe(&mut self, ch: char, next: Option<char>) -> bool {
        if self.line_comment {
            if ch == '\n' {
                self.line_comment = false;
                return true;
            }
            return false;
        }
        if let Some(active) = self.quote {
            if self.escaped {
                self.escaped = false;
            } else if ch == '\\' {
                self.escaped = true;
            } else if ch == active {
                self.quote = None;
            }
            return false;
        }
        if ch == '/' && next == Some('/') {
            self.line_comment = true;
            return false;
        }
        if matches!(ch, '"' | '\'') {
            self.quote = Some(ch);
            return false;
        }
        true
    }

    fn update_delimiters(&mut self, ch: char) {
        match ch {
            '(' | '[' | '{' => self.delimiters.push(ch),
            ')' if self.delimiters.last() == Some(&'(') => {
                self.delimiters.pop();
            }
            ']' if self.delimiters.last() == Some(&'[') => {
                self.delimiters.pop();
            }
            '}' if self.delimiters.last() == Some(&'{') => {
                self.delimiters.pop();
            }
            _ => {}
        }
    }

    fn is_top_level(&self) -> bool {
        self.delimiters.is_empty()
    }
}

#[cfg(test)]
mod tests;
