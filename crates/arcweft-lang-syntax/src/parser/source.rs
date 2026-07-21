use crate::ast::common::TextRange;
use crate::ast::flow::{AuthoredExpr, SourceLocaleBlock, Stmt};
use crate::ast::ids::EntityRef;
use crate::ast::source::{
    SourceBackpressurePolicy, SourceEventPattern, SourceHandler, SourceHeader, SourceItem,
    SourceItemParts, SourceOverflowPolicy, SourcePrivacyPolicy, SourceReplayPolicy,
};
use crate::cst::{
    ArcweftPunctuation, find_top_level_matching_punctuation,
    split_top_level_arcweft_punctuation_once, strip_prefix_arcweft_punctuation,
};
use crate::expr::Expr;
use crate::pattern::parse_pattern;
use crate::types::parse_type_ref;

use super::headers::{
    DeclEntityId, normalize_trailing_colon_id, parse_name_and_tail,
    parse_required_decl_entity_ref_or_marker, parse_visibility_prefix, simple_error, slice_offset,
};
use super::{
    Parser, collect_logical_block_items_with_base, parse_expr_lossy, parse_stmt_with_base,
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
        let block = self.take_flow_block_event();
        if !block.ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing source item",
                ["}"],
                Some(start_line.text.trim()),
                ["insert a closing `}` for the source body"],
            );
            return None;
        }
        let head_trimmed = block.head.trim();
        let head_base = start_line.start + slice_offset(&block.head, head_trimmed);
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

        let signature_tail_source = signature_tail.trim();
        let signature_tail_base = head_base
            + head_trimmed
                .find(signature_tail_source)
                .unwrap_or(head_trimmed.len());
        let source_ty = parse_source_type_from_tail(&signature_tail, signature_tail_base);
        let (header_body, header_body_base) = block
            .body_range
            .as_ref()
            .and_then(|range| {
                self.source
                    .get(range.clone())
                    .map(|body| (body, range.start))
            })
            .unwrap_or((&block.body, start_line.start));
        let headers = parse_source_headers(header_body, header_body_base);
        let handlers = parse_source_handlers(header_body, header_body_base);
        let body_statements = parse_source_stmt_lines(header_body, header_body_base);
        Some(SourceItem::from_parts(SourceItemParts {
            attrs,
            visibility,
            id,
            name,
            signature_tail,
            source_ty,
            headers,
            handlers,
            body: block.body.into_owned(),
            body_statements,
            range: TextRange::new(start_line.start, block.end),
        }))
    }
}

pub(super) fn parse_source_stmt_lines(body: &str, body_base: usize) -> Vec<Stmt> {
    collect_logical_block_items_with_base(body, body_base)
        .into_iter()
        .filter_map(|item| {
            let line = item.source.trim();
            if line.is_empty() {
                return None;
            }
            let base = item.base + slice_offset(&item.source, line);
            parse_source_stmt(line, base)
        })
        .collect()
}

pub(super) fn parse_source_type_from_tail(
    tail: &str,
    base: usize,
) -> Option<crate::types::AuthoredTypeRef> {
    let tail = tail.trim();
    let type_source = tail.strip_prefix(':').map(str::trim).or_else(|| {
        strip_prefix_arcweft_punctuation(tail, ArcweftPunctuation::ThinArrow).map(str::trim)
    })?;
    let mut parsed = parse_type_ref(type_source).ok()?;
    parsed.rebase(base + slice_offset(tail, type_source));
    Some(parsed)
}

pub(super) fn parse_source_headers(body: &str, body_base: usize) -> Vec<SourceHeader> {
    collect_logical_block_items_with_base(body, body_base)
        .into_iter()
        .filter_map(|item| {
            let line = item.source.trim();
            let base = item.base + slice_offset(&item.source, line);
            parse_source_header(line, base)
        })
        .collect()
}

fn parse_source_header(line: &str, base: usize) -> Option<SourceHeader> {
    if let Some(rest) = line.strip_prefix("from ") {
        let source = rest.trim();
        let start = base + slice_offset(line, source);
        return Some(SourceHeader::From(AuthoredExpr::with_source(
            parse_expr_lossy(source),
            source.to_owned(),
            Some(TextRange::new(start, start + source.len())),
        )));
    }
    let (key, value) = split_top_level_binding(line)?;
    let value = value.trim();
    let value_base = base + slice_offset(line, value);
    match key.trim() {
        "backpressure" => Some(SourceHeader::Backpressure {
            policy: parse_source_backpressure(value, value_base),
            range: TextRange::new(value_base, value_base + value.len()),
        }),
        "replay" => Some(SourceHeader::Replay {
            policy: parse_source_replay(value),
            range: TextRange::new(value_base, value_base + value.len()),
        }),
        "privacy" => Some(SourceHeader::Privacy {
            policy: parse_source_privacy(value),
            range: TextRange::new(value_base, value_base + value.len()),
        }),
        _ => None,
    }
}

fn parse_source_backpressure(value: &str, base: usize) -> SourceBackpressurePolicy {
    match value {
        "latest" => SourceBackpressurePolicy::Latest,
        "blocking_not_allowed" => SourceBackpressurePolicy::BlockingNotAllowed,
        value if value.starts_with("bounded") => {
            let options = parse_source_call_options(value, base);
            let capacity = options
                .iter()
                .find_map(|(key, value)| (key == "capacity").then(|| Box::new(value.clone())));
            let overflow = options
                .iter()
                .find_map(|(key, value)| {
                    (key == "overflow").then(|| match value.expr() {
                        Expr::Path(path) => parse_source_overflow(path, value.range()),
                        Expr::Raw(raw) => parse_source_overflow(raw, value.range()),
                        expression => SourceOverflowPolicy::Raw {
                            value: format!("{expression:?}"),
                            range: value.range(),
                        },
                    })
                })
                .unwrap_or(SourceOverflowPolicy::Missing);
            SourceBackpressurePolicy::Bounded { capacity, overflow }
        }
        value => SourceBackpressurePolicy::Raw(value.to_owned()),
    }
}

fn parse_source_call_options(value: &str, base: usize) -> Vec<(String, AuthoredExpr)> {
    let Some((open, close)) = find_top_level_matching_punctuation(value, '(', ')') else {
        return Vec::new();
    };
    let inner = &value[open + 1..close];
    let inner_base = base + open + 1;
    split_comma_args(inner)
        .into_iter()
        .filter_map(|part| {
            let part = part.trim();
            let part_base = inner_base + slice_offset(inner, part);
            split_top_level_binding(part).map(|(key, value)| {
                let source = value.trim();
                let start = part_base + slice_offset(part, source);
                (
                    key.trim().to_owned(),
                    AuthoredExpr::with_source(
                        parse_expr_lossy(source),
                        source.to_owned(),
                        Some(TextRange::new(start, start + source.len())),
                    ),
                )
            })
        })
        .collect()
}

fn parse_source_overflow(value: &str, range: Option<TextRange>) -> SourceOverflowPolicy {
    match value.trim() {
        "drop_oldest" => SourceOverflowPolicy::DropOldest,
        "drop_newest" => SourceOverflowPolicy::DropNewest,
        "error" => SourceOverflowPolicy::Error,
        "coalesce" => SourceOverflowPolicy::Coalesce,
        value => SourceOverflowPolicy::Raw {
            value: value.to_owned(),
            range,
        },
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

pub(super) fn parse_source_handlers(body: &str, body_base: usize) -> Vec<SourceHandler> {
    collect_logical_block_items_with_base(body, body_base)
        .into_iter()
        .filter_map(|item| {
            let line = item.source.trim();
            let base = item.base + slice_offset(&item.source, line);
            parse_source_handler(line, base)
        })
        .collect()
}

fn parse_source_handler(line: &str, base: usize) -> Option<SourceHandler> {
    let rest = line.strip_prefix("on ")?;
    let (head, action) =
        split_top_level_arcweft_punctuation_once(rest, ArcweftPunctuation::FatArrow)?;
    let action = action.trim();
    let action_base = base + slice_offset(line, action);
    let body = action
        .strip_prefix('{')
        .and_then(|action| action.strip_suffix('}'))
        .map_or_else(
            || vec![parse_stmt_with_base(action, action_base)],
            |block| parse_source_handler_body(block, action_base + '{'.len_utf8()),
        );
    Some(SourceHandler::new(
        parse_source_event_pattern(head.trim()),
        body,
    ))
}

fn parse_source_handler_body(body: &str, body_base: usize) -> Vec<Stmt> {
    collect_logical_block_items_with_base(body, body_base)
        .into_iter()
        .filter_map(|item| {
            let statement = item.source.trim();
            (!statement.is_empty()).then(|| {
                let base = item.base + slice_offset(&item.source, statement);
                parse_stmt_with_base(statement, base)
            })
        })
        .collect()
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

fn parse_source_stmt(trimmed: &str, base: usize) -> Option<Stmt> {
    if parse_source_header(trimmed, base).is_some()
        || trimmed.starts_with("from ")
        || trimmed.starts_with("on ")
    {
        return None;
    }
    Some(parse_stmt_with_base(trimmed, base))
}
