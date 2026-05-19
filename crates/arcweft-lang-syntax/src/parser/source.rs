use crate::ast::common::TextRange;
use crate::ast::dialogue::ScenarioCommand;
use crate::ast::flow::Stmt;
use crate::ast::source::{
    SourceBackpressurePolicy, SourceEventPattern, SourceHandler, SourceHeader,
    SourceOverflowPolicy, SourcePrivacyPolicy, SourceReplayPolicy,
};
use crate::cst::{
    find_matching_punctuation, find_top_level_punctuation,
    split_top_level_punctuation_sequence_once,
};
use crate::expr::Expr;
use crate::pattern::parse_pattern;
use crate::types::parse_type_ref;

use super::{
    collect_logical_block_items, parse_expr_lossy, parse_stmt, parse_stmt_lines, split_comma_args,
    split_top_level_binding,
};
pub(super) fn parse_source_stmt_lines(body: &str) -> Vec<Stmt> {
    collect_logical_block_items(body)
        .into_iter()
        .map(|line| line.trim().to_owned())
        .filter(|line| !line.is_empty())
        .map(|line| parse_source_stmt(&line))
        .collect()
}

pub(super) fn parse_source_type_from_tail(tail: &str) -> Option<crate::types::TypeRef> {
    let tail = tail.trim();
    let type_source = tail
        .strip_prefix(':')
        .map(str::trim)
        .or_else(|| tail.strip_prefix("->").map(str::trim))?;
    parse_type_ref(type_source).ok()
}

pub(super) fn parse_source_headers(body: &str) -> Vec<SourceHeader> {
    collect_logical_block_items(body)
        .into_iter()
        .map(|line| line.trim().to_owned())
        .filter_map(|line| parse_source_header(&line))
        .collect()
}

fn parse_source_header(line: &str) -> Option<SourceHeader> {
    if let Some(rest) = line.strip_prefix("from ") {
        return Some(SourceHeader::From(parse_expr_lossy(rest.trim())));
    }
    let (key, value) = split_top_level_binding(line)?;
    let value = value.trim();
    match key.trim() {
        "backpressure" => Some(SourceHeader::Backpressure(parse_source_backpressure(value))),
        "replay" => Some(SourceHeader::Replay(parse_source_replay(value))),
        "privacy" => Some(SourceHeader::Privacy(parse_source_privacy(value))),
        _ => None,
    }
}

fn parse_source_backpressure(value: &str) -> SourceBackpressurePolicy {
    match value {
        "latest" => SourceBackpressurePolicy::Latest,
        "blocking_not_allowed" => SourceBackpressurePolicy::BlockingNotAllowed,
        value if value.starts_with("bounded") => {
            let options = parse_source_call_options(value);
            let capacity = options
                .iter()
                .find_map(|(key, value)| (key == "capacity").then_some(value.clone()))
                .unwrap_or_else(|| Expr::Raw("missing_capacity".to_owned()));
            let overflow = options
                .iter()
                .find_map(|(key, value)| {
                    (key == "overflow").then(|| match value {
                        Expr::Path(path) => parse_source_overflow(path),
                        Expr::Raw(raw) => parse_source_overflow(raw),
                        _ => SourceOverflowPolicy::Raw(format!("{value:?}")),
                    })
                })
                .unwrap_or(SourceOverflowPolicy::Error);
            SourceBackpressurePolicy::Bounded { capacity, overflow }
        }
        value => SourceBackpressurePolicy::Raw(value.to_owned()),
    }
}

fn parse_source_call_options(value: &str) -> Vec<(String, Expr)> {
    let Some(open) = find_top_level_punctuation(value, '(') else {
        return Vec::new();
    };
    let Some(close) = find_matching_punctuation(value, open, '(', ')') else {
        return Vec::new();
    };
    split_comma_args(&value[open + 1..close])
        .into_iter()
        .filter_map(|part| {
            split_top_level_binding(part.trim())
                .map(|(key, value)| (key.trim().to_owned(), parse_expr_lossy(value.trim())))
        })
        .collect()
}

fn parse_source_overflow(value: &str) -> SourceOverflowPolicy {
    match value.trim() {
        "drop_oldest" => SourceOverflowPolicy::DropOldest,
        "drop_newest" => SourceOverflowPolicy::DropNewest,
        "error" => SourceOverflowPolicy::Error,
        "coalesce" => SourceOverflowPolicy::Coalesce,
        value => SourceOverflowPolicy::Raw(value.to_owned()),
    }
}

fn parse_source_replay(value: &str) -> SourceReplayPolicy {
    match value.trim() {
        "full" => SourceReplayPolicy::Full,
        "hash_only" => SourceReplayPolicy::HashOnly,
        "summary" => SourceReplayPolicy::Summary,
        "event_only" => SourceReplayPolicy::EventOnly,
        "none" => SourceReplayPolicy::None,
        value => SourceReplayPolicy::Raw(value.to_owned()),
    }
}

fn parse_source_privacy(value: &str) -> SourcePrivacyPolicy {
    match value.trim() {
        "transient" => SourcePrivacyPolicy::Transient,
        "redacted" => SourcePrivacyPolicy::Redacted,
        "recordable" => SourcePrivacyPolicy::Recordable,
        "private" => SourcePrivacyPolicy::Private,
        value => SourcePrivacyPolicy::Raw(value.to_owned()),
    }
}

pub(super) fn parse_source_handlers(body: &str) -> Vec<SourceHandler> {
    collect_logical_block_items(body)
        .into_iter()
        .map(|line| line.trim().to_owned())
        .filter_map(|line| parse_source_handler(&line))
        .collect()
}

fn parse_source_handler(line: &str) -> Option<SourceHandler> {
    let rest = line.strip_prefix("on ")?;
    let (head, action) = split_top_level_punctuation_sequence_once(rest, &["=", ">"])?;
    let action = action.trim();
    let body = action
        .strip_prefix('{')
        .and_then(|action| action.strip_suffix('}'))
        .map_or_else(
            || vec![parse_stmt(action)],
            |block| parse_stmt_lines(block.trim()),
        );
    Some(SourceHandler::new(
        parse_source_event_pattern(head.trim()),
        body,
    ))
}

fn parse_source_event_pattern(source: &str) -> SourceEventPattern {
    if let Some(rest) = source.strip_prefix("item ") {
        return SourceEventPattern::Item(parse_pattern(rest.trim()));
    }
    if let Some(rest) = source.strip_prefix("error ") {
        return SourceEventPattern::Error(parse_pattern(rest.trim()));
    }
    if let Some(rest) = source.strip_prefix("progress ") {
        return SourceEventPattern::Progress(parse_pattern(rest.trim()));
    }
    match source {
        "disconnected" => SourceEventPattern::Disconnected,
        "permission_revoked" => SourceEventPattern::PermissionRevoked,
        "end" => SourceEventPattern::End,
        source => SourceEventPattern::Raw(source.to_owned()),
    }
}

fn parse_source_stmt(trimmed: &str) -> Stmt {
    if let Some(rest) = trimmed.strip_prefix("from ") {
        return Stmt::Command(ScenarioCommand::new(
            "from".to_owned(),
            vec![parse_expr_lossy(rest.trim())],
            TextRange::new(0, trimmed.len()),
        ));
    }
    if trimmed.starts_with("on ") {
        // Source handlers are preserved structurally on SourceItem::handlers.
        // Keep the legacy body-statement view typecheck-ready without
        // duplicating handler effects into the ordinary statement stream.
        return Stmt::Expr(Expr::Tuple(Vec::new()));
    }
    parse_stmt(trimmed)
}
