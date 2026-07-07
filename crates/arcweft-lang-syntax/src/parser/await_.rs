use super::{
    CstLine, FlowItem, ParseError, Parser, RecoverySuggestion, SourceAnchor, SourceName, Stmt,
    SyntaxParseStats, TextRange, find_matching_punctuation, indentation, parse_expr_lossy,
    parse_pattern, parse_stmt, source_line_iter, split_top_level_binding,
    split_top_level_keyword_once,
};
use crate::ast::flow::{AwaitBranch, AwaitBranchKind, AwaitWith};
use crate::cst::{
    ArcweftPunctuation, nonempty_trimmed_source_lines, source_line_count,
    split_top_level_arcweft_punctuation_once,
};
use std::ops::Range;

enum AwaitBody {
    Source(String),
    Lines(Range<usize>),
}

impl Parser<'_> {
    pub(super) fn parse_let_await_with(&mut self) -> Option<Stmt> {
        let start_line = self.current().clone();
        let trimmed = start_line.text.trim();
        let range = TextRange::new(start_line.start, start_line.end);

        let (head, body) = if has_inline_brace_await_with(trimmed) {
            let (head, body, _, ok) = self.take_brace_block();
            if !ok {
                self.push_error(
                    range,
                    "unclosed block while parsing await expression binding",
                    ["}"],
                    Some(trimmed),
                    ["insert a closing `}` for the await wait-view block"],
                );
                return None;
            }
            (
                head.into_owned(),
                Some(AwaitBody::Source(format!("{{ {body} }}"))),
            )
        } else if trimmed.ends_with("with:") {
            self.index += 1;
            let body_range = self.take_indented_line_range(indentation(&start_line.text) + 1);
            let closing = self.take_parenthesized_await_closing();
            (
                format!("{}{}", trimmed, closing.unwrap_or_default()),
                Some(AwaitBody::Lines(body_range)),
            )
        } else {
            self.take_multiline_let_await_head(&start_line)
        };

        let rest = head.trim().strip_prefix("let")?.trim();
        let (pattern, await_head) = split_top_level_binding(rest)?;
        let await_head = normalize_let_await_source(await_head.trim(), None);
        let await_with = match body {
            Some(AwaitBody::Lines(body_range)) => {
                self.parse_await_with_line_range(&await_head, range, body_range)
            }
            Some(AwaitBody::Source(body)) => {
                let await_source = if body.trim_start().starts_with('{') {
                    format!("{await_head} {body}")
                } else {
                    format!("{await_head}\n{body}")
                };
                parse_await_with(&await_source, range, &mut self.errors)
            }
            None => parse_await_with(&await_head, range, &mut self.errors),
        };

        Some(Stmt::LetAwait {
            pattern: parse_pattern(pattern.trim()),
            await_with,
        })
    }

    fn take_multiline_let_await_head(
        &mut self,
        start_line: &CstLine,
    ) -> (String, Option<AwaitBody>) {
        let base_indent = indentation(&start_line.text);
        let mut head = start_line.text.trim().to_owned();
        self.index += 1;

        while self.index < self.events.len() {
            let line = self.current().clone();
            let trimmed = line.text.trim();
            if trimmed.is_empty() {
                self.index += 1;
                continue;
            }
            if trimmed == "with:" {
                self.index += 1;
                let body_range = self.take_indented_line_range(base_indent + 1);
                let closing = self.take_parenthesized_await_closing();
                return (
                    format!("{head} with:{}", closing.unwrap_or_default()),
                    Some(AwaitBody::Lines(body_range)),
                );
            }
            if has_standalone_brace_with(trimmed) {
                let (with_head, body, _, ok) = self.take_brace_block();
                if ok {
                    let closing = self.take_parenthesized_await_closing();
                    return (
                        format!("{head} {with_head}{}", closing.unwrap_or_default()),
                        Some(AwaitBody::Source(format!("{{ {body} }}"))),
                    );
                }
            }
            if indentation(&line.text) > base_indent || trimmed.starts_with('.') {
                append_await_head_continuation(&mut head, trimmed);
                self.index += 1;
                continue;
            }
            break;
        }

        (head, None)
    }

    pub(super) fn parse_await_with_line_range(
        &mut self,
        head: &str,
        range: TextRange,
        body_range: Range<usize>,
    ) -> AwaitWith {
        let source = head.trim();
        let (applies_try, after_keyword) = parse_await_prefix(source);
        let (expr_part, inline_branch_part) = split_await_head(after_keyword);

        push_ambiguous_await_question_error(expr_part, range, &mut self.errors);

        let branches = if inline_branch_part.trim().is_empty() {
            self.parse_await_branches_line_range(body_range)
        } else {
            parse_await_branches(inline_branch_part.trim())
        };
        AwaitWith::new(
            parse_expr_lossy(expr_part.trim_end_matches('?').trim()),
            applies_try,
            branches,
        )
    }

    fn parse_await_branches_line_range(&mut self, range: Range<usize>) -> Vec<AwaitBranch> {
        if range.clone().any(|index| {
            self.events
                .get(index)
                .is_some_and(|line| is_colon_await_branch_head(line.trimmed()))
        }) {
            return self.parse_colon_await_branches_line_range(range);
        }
        range
            .filter_map(|index| self.events.get(index))
            .map(CstLine::trimmed)
            .filter(|line| !line.is_empty())
            .filter_map(parse_await_branch)
            .collect()
    }

    fn parse_colon_await_branches_line_range(&mut self, range: Range<usize>) -> Vec<AwaitBranch> {
        let mut branches = Vec::new();
        let mut current_head = None::<String>;
        let mut body_start = range.start;

        for index in range.clone() {
            let Some(line) = self.events.get(index) else {
                break;
            };
            let trimmed = line.trimmed();
            if trimmed.is_empty() {
                continue;
            }
            if is_colon_await_branch_head(trimmed) {
                if let Some(head) = current_head.replace(trimmed.to_owned())
                    && let Some(branch) =
                        self.parse_colon_await_branch_line_range(&head, body_start..index)
                {
                    branches.push(branch);
                }
                body_start = index + 1;
            }
        }

        if let Some(head) = current_head
            && let Some(branch) =
                self.parse_colon_await_branch_line_range(&head, body_start..range.end)
        {
            branches.push(branch);
        }
        branches
    }

    fn parse_colon_await_branch_line_range(
        &mut self,
        head: &str,
        body_range: Range<usize>,
    ) -> Option<AwaitBranch> {
        let mut parts = head.trim_end_matches(':').split_whitespace();
        let kind = await_branch_kind(parts.next()?)?;
        let pattern = parse_pattern(parts.collect::<Vec<_>>().join(" ").trim());
        let body = self.parse_await_branch_body_line_range(body_range);
        Some(AwaitBranch::new(kind, pattern, body))
    }

    fn parse_await_branch_body_line_range(&mut self, body_range: Range<usize>) -> Vec<FlowItem> {
        let Some(events) = self.events.relative_line_slice(body_range.clone(), 0) else {
            let body = self.collect_line_range_source(body_range);
            return parse_await_branch_body(body.trim());
        };
        let mut nested = Parser::from_line_events("", events, SyntaxParseStats::default());
        let mut items = Vec::new();
        while nested.index < nested.events.len() {
            nested.skip_blank_and_comments();
            if nested.index >= nested.events.len() {
                break;
            }
            let before = nested.index;
            let item = nested.parse_flow_item_until_indent(0).unwrap_or_else(|| {
                let stmt = FlowItem::Stmt(parse_stmt(nested.current().text.trim()));
                nested.index += 1;
                stmt
            });
            items.push(item);
            if nested.index == before {
                nested.index += 1;
            }
        }
        self.errors.extend(nested.errors);
        self.syntax_stats.numeric_seq_summaries += nested.syntax_stats.numeric_seq_summaries;
        if items.is_empty() {
            let body = self.collect_line_range_source(body_range);
            if !body.trim().is_empty() {
                items.push(parse_inline_await_branch_item(body.trim()));
            }
        }
        items
    }

    fn take_parenthesized_await_closing(&mut self) -> Option<String> {
        if self.index >= self.events.len() {
            return None;
        }
        let line = self.current().clone();
        let trimmed = line.text.trim();
        if !trimmed.starts_with(')') {
            return None;
        }
        self.index += 1;
        Some(trimmed.to_owned())
    }

    pub(super) fn has_multiline_await_with(&self, base_indent: usize) -> bool {
        self.events
            .iter()
            .skip(self.index + 1)
            .take_while(|line| {
                let trimmed = line.text.trim();
                trimmed.is_empty()
                    || indentation(&line.text) > base_indent
                    || trimmed.starts_with('.')
                    || trimmed.starts_with("with")
            })
            .any(|line| {
                let trimmed = line.text.trim();
                trimmed == "with:" || has_standalone_brace_with(trimmed)
            })
    }
}

pub(super) fn is_await_with_head(trimmed: &str) -> bool {
    (trimmed.starts_with("await ")
        || trimmed.starts_with("try await ")
        || trimmed.starts_with("await? "))
        && (trimmed.contains(" with ") || trimmed.ends_with("with:"))
}

fn has_inline_brace_await_with(trimmed: &str) -> bool {
    (trimmed.contains(" with {") || trimmed.contains(" with{")) && trimmed.contains('{')
}

fn has_standalone_brace_with(trimmed: &str) -> bool {
    trimmed.starts_with("with {") || trimmed.starts_with("with{")
}

fn append_await_head_continuation(head: &mut String, continuation: &str) {
    if continuation.starts_with('.') {
        head.push_str(continuation);
    } else {
        head.push(' ');
        head.push_str(continuation);
    }
}

fn normalize_let_await_source(head: &str, trailing: Option<&str>) -> String {
    let source = trailing.map_or_else(
        || head.trim().to_owned(),
        |trailing| format!("{head}{trailing}"),
    );
    normalize_parenthesized_await_source(&source).unwrap_or(source)
}

fn normalize_parenthesized_await_source(source: &str) -> Option<String> {
    let source = source.trim();
    let (await_part, postfix) = split_parenthesized_await_postfix(source)?;
    let postfix = postfix.trim();
    let applies_try = postfix.ends_with('?');
    let context_call = postfix.trim_end_matches('?').strip_prefix(".context(");
    let await_part = await_part.trim();
    let await_part = await_part.strip_prefix("await ")?;
    let await_head = context_call
        .and_then(|args| args.strip_suffix(')'))
        .map_or_else(
            || await_part.to_owned(),
            |args| insert_context_before_await_with(await_part, args),
        );
    Some(if applies_try {
        format!("try await {await_head}")
    } else {
        format!("await {await_head}")
    })
}

fn split_parenthesized_await_postfix(source: &str) -> Option<(&str, &str)> {
    source.strip_prefix('(')?;
    let close = find_matching_punctuation(source, 0, '(', ')')?;
    Some((&source[1..close], &source[close + ')'.len_utf8()..]))
}

fn insert_context_before_await_with(await_part: &str, args: &str) -> String {
    let (expr, branches) = split_await_head(await_part);
    if await_part.contains(" with:") {
        format!("{}.context({args}) with:{branches}", expr.trim())
    } else if await_part.contains(" with ") {
        format!("{}.context({args}) with {branches}", expr.trim())
    } else {
        format!("{}.context({args})", expr.trim())
    }
}

pub(super) fn parse_await_with(
    trimmed: &str,
    range: TextRange,
    errors: &mut Vec<ParseError>,
) -> AwaitWith {
    let source = trimmed.trim();
    let (applies_try, after_keyword) = parse_await_prefix(source);
    let (expr_part, branch_part) = split_await_head(after_keyword);

    push_ambiguous_await_question_error(expr_part, range, errors);

    AwaitWith::new(
        parse_expr_lossy(expr_part.trim_end_matches('?').trim()),
        applies_try,
        parse_await_branches(branch_part.trim()),
    )
}

fn parse_await_prefix(source: &str) -> (bool, &str) {
    source
        .strip_prefix("try await")
        .map(|rest| (true, rest.trim()))
        .or_else(|| {
            source
                .strip_prefix("await?")
                .map(|rest| (true, rest.trim()))
        })
        .or_else(|| {
            source
                .strip_prefix("await")
                .map(|rest| (false, rest.trim()))
        })
        .unwrap_or((false, source))
}

fn push_ambiguous_await_question_error(
    expr_part: &str,
    range: TextRange,
    errors: &mut Vec<ParseError>,
) {
    // Postfix `?` remains the ordinary Rust-like propagation operator. The
    // rejected form is only `await expr? with:`, where pending handling must
    // group with the await before propagation is applied.
    if expr_part.trim_end().ends_with('?') {
        errors.push(ParseError::new(
            range,
            vec!["try await expr with:".to_owned()],
            Some(expr_part.trim().to_owned()),
            "`await expr? with` is ambiguous; use `try await expr with`".to_owned(),
            vec![RecoverySuggestion::new(
                "move `?` before `await` as `try await`",
            )],
            SourceAnchor::new(SourceName::path("<memory>"), range.start()..range.end()),
        ));
    }
}

fn split_await_head(source: &str) -> (&str, &str) {
    let (expr, branches) = split_top_level_keyword_once(source, "with");
    if let Some(branches) = branches {
        return (expr, branches.trim_start_matches(':').trim_start());
    }
    (source, "")
}

fn parse_await_branches(source: &str) -> Vec<AwaitBranch> {
    let body = source
        .strip_prefix('{')
        .and_then(|value| value.strip_suffix('}'))
        .unwrap_or(source)
        .trim();
    if source_line_iter(body).any(|line| is_colon_await_branch_head(line.trim())) {
        return parse_colon_await_branches(body);
    }
    split_await_branch_lines(body)
        .into_iter()
        .filter_map(parse_await_branch)
        .collect()
}

fn parse_colon_await_branches(source: &str) -> Vec<AwaitBranch> {
    let mut branches = Vec::new();
    let mut current_head = None::<String>;
    let mut current_body = String::new();

    for line in source_line_iter(source) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if is_colon_await_branch_head(trimmed) {
            if let Some(head) = current_head.replace(trimmed.to_owned()) {
                if let Some(branch) = parse_colon_await_branch(&head, &current_body) {
                    branches.push(branch);
                }
                current_body.clear();
            }
        } else {
            if !current_body.is_empty() {
                current_body.push('\n');
            }
            current_body.push_str(line);
        }
    }

    if let Some(head) = current_head
        && let Some(branch) = parse_colon_await_branch(&head, &current_body)
    {
        branches.push(branch);
    }
    branches
}

fn is_colon_await_branch_head(trimmed: &str) -> bool {
    trimmed.ends_with(':')
        && matches!(
            trimmed.trim_end_matches(':').split_whitespace().next(),
            Some("pending" | "ready" | "error" | "denied")
        )
}

fn parse_colon_await_branch(head: &str, body: &str) -> Option<AwaitBranch> {
    let mut parts = head.trim_end_matches(':').split_whitespace();
    let kind = await_branch_kind(parts.next()?)?;
    let pattern = parse_pattern(parts.collect::<Vec<_>>().join(" ").trim());
    Some(AwaitBranch::new(
        kind,
        pattern,
        parse_await_branch_body(body),
    ))
}

fn split_await_branch_lines(source: &str) -> Vec<&str> {
    if source_line_count(source) > 1 {
        return nonempty_trimmed_source_lines(source);
    }
    let mut lines = Vec::new();
    let mut start = 0;
    for keyword in [" pending ", " ready ", " error ", " denied "] {
        for (index, _) in source.match_indices(keyword) {
            let line = source[start..index].trim();
            if !line.is_empty() {
                lines.push(line);
            }
            start = index + 1;
        }
    }
    let tail = source[start..].trim();
    if !tail.is_empty() {
        lines.push(tail);
    }
    lines
}

fn parse_await_branch(line: &str) -> Option<AwaitBranch> {
    let (head, body) =
        split_top_level_arcweft_punctuation_once(line, ArcweftPunctuation::FatArrow)?;
    let mut parts = head.split_whitespace();
    let kind = await_branch_kind(parts.next()?)?;
    let pattern = parse_pattern(parts.collect::<Vec<_>>().join(" ").trim());
    Some(AwaitBranch::new(
        kind,
        pattern,
        parse_await_branch_body(body.trim()),
    ))
}

fn await_branch_kind(source: &str) -> Option<AwaitBranchKind> {
    match source {
        "pending" => Some(AwaitBranchKind::Pending),
        "ready" => Some(AwaitBranchKind::Ready),
        "error" => Some(AwaitBranchKind::Error),
        "denied" => Some(AwaitBranchKind::Denied),
        _ => None,
    }
}

fn parse_await_branch_body(body: &str) -> Vec<FlowItem> {
    let mut nested = Parser::new(body);
    let mut items = Vec::new();
    while nested.index < nested.events.len() {
        nested.skip_blank_and_comments();
        if nested.index >= nested.events.len() {
            break;
        }
        let before = nested.index;
        let item = nested.parse_flow_item_until_indent(0).unwrap_or_else(|| {
            let stmt = FlowItem::Stmt(parse_stmt(nested.current().text.trim()));
            nested.index += 1;
            stmt
        });
        items.push(item);
        if nested.index == before {
            nested.index += 1;
        }
    }
    if items.is_empty() && !body.trim().is_empty() {
        items.push(parse_inline_await_branch_item(body.trim()));
    }
    items
}

fn parse_inline_await_branch_item(body: &str) -> FlowItem {
    let mut nested = Parser::new(body);
    nested
        .parse_flow_item_until_indent(0)
        .unwrap_or_else(|| FlowItem::Stmt(parse_stmt(body)))
}
