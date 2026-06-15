use crate::ast::common::TextRange;
use crate::ast::flow::{SourceLocaleBlock, Stmt};
use crate::ast::ids::EntityRef;
use crate::ast::source::{
    SourceBackpressurePolicy, SourceEventPattern, SourceHandler, SourceHeader, SourceItem,
    SourceItemParts, SourceOverflowPolicy, SourcePrivacyPolicy, SourceReplayPolicy,
};
use crate::cst::{find_top_level_matching_punctuation, split_top_level_punctuation_sequence_once};
use crate::expr::{CallArg, Expr};
use crate::pattern::parse_pattern;
use crate::types::parse_type_ref;

use super::headers::{
    DeclEntityId, normalize_trailing_colon_id, parse_name_and_tail,
    parse_required_decl_entity_ref_or_marker, parse_visibility_prefix, simple_error, slice_offset,
};
use super::{
    Parser, collect_logical_block_items, parse_expr_lossy, parse_stmt, parse_stmt_lines,
    split_comma_args, split_top_level_binding,
};

impl Parser<'_> {
    pub(super) fn parse_source_locale_block(&mut self) -> Option<SourceLocaleBlock> {
        let start_line = self.current().clone();
        let block = self.take_brace_block_event();
        if !block.ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing source locale",
                ["}"],
                Some(start_line.text.trim()),
                ["insert a closing `}` for the source locale block"],
            );
            return None;
        }
        let head = &block.head;
        let locale = head.trim().strip_prefix("source locale")?.trim().to_owned();
        let body = self.parse_flow_body_from_block(&block, start_line.start + head.len());
        Some(SourceLocaleBlock::new(
            locale,
            body,
            TextRange::new(start_line.start, block.end),
        ))
    }

    pub(super) fn parse_source_item(&mut self) -> Option<SourceItem> {
        let attrs = self.take_pending_attrs();
        let start_line = self.current().clone();
        let (head, body, end, ok) = self.take_flow_block();
        if !ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing source item",
                ["}"],
                Some(start_line.text.trim()),
                ["insert a closing `}` for the source body"],
            );
            return None;
        }
        let head_trimmed = head.trim();
        let head_base = start_line.start + slice_offset(&head, head_trimmed);
        let (visibility, after_visibility) = parse_visibility_prefix(head_trimmed);
        let after_source = after_visibility
            .trim_start()
            .strip_prefix("source")?
            .trim_start();
        let (id, name, signature_tail) = if after_source.starts_with('@') {
            let id_base = head_base + slice_offset(head_trimmed, after_source);
            match parse_required_decl_entity_ref_or_marker(
                after_source,
                "source",
                id_base,
                &mut self.errors,
            )? {
                (DeclEntityId::Entity(id), rest) => {
                    let (id, tail) = normalize_trailing_colon_id(id, rest);
                    let (name, tail) = parse_name_and_tail(&tail);
                    (Some(id), name, tail.trim().to_owned())
                }
                (DeclEntityId::NameMarker(marker), rest) => {
                    let (name, tail) = parse_name_and_tail(rest);
                    let Some(name_value) = name.as_deref() else {
                        self.errors.push(simple_error(
                            marker.range.start(),
                            marker.range.end() - marker.range.start(),
                            "source declaration marker needs a following source name",
                            "@source:. name()",
                        ));
                        return None;
                    };
                    (
                        Some(EntityRef::new(
                            format!("source.{name_value}"),
                            false,
                            marker.range,
                        )),
                        name,
                        tail,
                    )
                }
            }
        } else {
            let (name, tail) = parse_name_and_tail(after_source);
            (None, name, tail)
        };

        let source_ty = parse_source_type_from_tail(&signature_tail);
        let headers = parse_source_headers(&body);
        let handlers = parse_source_handlers(&body);
        let body_statements = parse_source_stmt_lines(&body);
        Some(SourceItem::from_parts(SourceItemParts {
            attrs,
            visibility,
            id,
            name,
            signature_tail,
            source_ty,
            headers,
            handlers,
            body: body.into_owned(),
            body_statements,
            range: TextRange::new(start_line.start, end),
        }))
    }
}

pub(super) fn parse_source_stmt_lines(body: &str) -> Vec<Stmt> {
    collect_logical_block_items(body)
        .into_iter()
        .map(|line| line.trim().to_owned())
        .filter(|line| !line.is_empty())
        .filter_map(|line| parse_source_stmt(&line))
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
    let Some((open, close)) = find_top_level_matching_punctuation(value, '(', ')') else {
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

fn parse_source_stmt(trimmed: &str) -> Option<Stmt> {
    if parse_source_header(trimmed).is_some() {
        return None;
    }
    if let Some(rest) = trimmed.strip_prefix("from ") {
        return Some(Stmt::Expr(Expr::Call {
            callee: Box::new(Expr::Path("from".to_owned())),
            args: vec![CallArg::Positional(parse_expr_lossy(rest.trim()))],
        }));
    }
    if trimmed.starts_with("on ") {
        // Source handlers are preserved structurally on SourceItem::handlers.
        // Keep the body-statement view typecheck-ready without duplicating
        // handler effects into the ordinary statement stream.
        return Some(Stmt::Expr(Expr::Tuple(Vec::new())));
    }
    Some(parse_stmt(trimmed))
}
