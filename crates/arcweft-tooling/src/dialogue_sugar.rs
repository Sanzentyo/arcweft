use arcweft_lang_syntax::{
    ast::{
        dialogue::DialogueContent,
        flow::{FlowItem, Stmt},
        items::{Attribute, Item},
    },
    expr::Expr,
    source::ParsedSource,
};
use std::collections::BTreeSet;

use crate::model::TextEdit;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DialogueSugarMode {
    All,
    RichTextOnly,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct DialogueSugarContext {
    text_proxy_types: BTreeSet<String>,
}

impl DialogueSugarContext {
    pub(crate) fn from_parsed(parsed: &ParsedSource) -> Self {
        Self {
            text_proxy_types: collect_text_proxy_type_names(parsed),
        }
    }
}

pub(crate) fn dialogue_text_sugar_edits(
    source: &str,
    parsed: &ParsedSource,
    mode: DialogueSugarMode,
    context: &DialogueSugarContext,
) -> Vec<TextEdit> {
    let mut edits = Vec::new();
    for item in parsed.typed_tree().items() {
        collect_dialogue_text_sugar_edits_from_item(source, item, &mut edits, mode, context);
    }
    edits
}

fn collect_dialogue_text_sugar_edits_from_item(
    source: &str,
    item: &Item,
    edits: &mut Vec<TextEdit>,
    mode: DialogueSugarMode,
    context: &DialogueSugarContext,
) {
    match item {
        Item::Flow(flow) => {
            for item in flow.body() {
                collect_dialogue_text_sugar_edits_from_flow_item(
                    source, item, edits, mode, context,
                );
            }
        }
        Item::FlowItem(item) => {
            collect_dialogue_text_sugar_edits_from_flow_item(source, item, edits, mode, context);
        }
        _ => {}
    }
}

fn collect_dialogue_text_sugar_edits_from_flow_item(
    source: &str,
    item: &FlowItem,
    edits: &mut Vec<TextEdit>,
    mode: DialogueSugarMode,
    context: &DialogueSugarContext,
) {
    match item {
        FlowItem::SpeakerLine(line) => {
            collect_dialogue_content_sugar_edits(source, line.content(), edits, mode, context);
        }
        FlowItem::ContentCall(call) => {
            collect_dialogue_content_sugar_edits(source, call.content(), edits, mode, context);
        }
        FlowItem::Stmt(stmt) => {
            collect_dialogue_text_sugar_edits_from_stmt(source, stmt, edits, mode, context);
        }
        FlowItem::Choice(_) | FlowItem::Include(_) | FlowItem::Raw(_) => {}
        _ => collect_dialogue_text_sugar_edits_from_flow_item_children(
            source, item, edits, mode, context,
        ),
    }
}

fn collect_dialogue_text_sugar_edits_from_flow_item_children(
    source: &str,
    item: &FlowItem,
    edits: &mut Vec<TextEdit>,
    mode: DialogueSugarMode,
    context: &DialogueSugarContext,
) {
    if let Some(body) = flow_item_body(item) {
        collect_dialogue_text_sugar_edits_from_flow_items(source, body, edits, mode, context);
        return;
    }
    match item {
        FlowItem::Match(block) => {
            for arm in block.arms() {
                collect_dialogue_text_sugar_edits_from_flow_items(
                    source,
                    arm.body(),
                    edits,
                    mode,
                    context,
                );
            }
        }
        FlowItem::Select(block) => {
            for branch in block.branches() {
                collect_dialogue_text_sugar_edits_from_flow_items(
                    source,
                    branch.body(),
                    edits,
                    mode,
                    context,
                );
            }
        }
        FlowItem::AwaitWith(await_with) => {
            for branch in await_with.branches() {
                collect_dialogue_text_sugar_edits_from_flow_items(
                    source,
                    branch.body(),
                    edits,
                    mode,
                    context,
                );
            }
        }
        FlowItem::Scope(_)
        | FlowItem::If(_)
        | FlowItem::IfLet(_)
        | FlowItem::Loop(_)
        | FlowItem::While(_)
        | FlowItem::WhileLet(_)
        | FlowItem::For(_)
        | FlowItem::BorrowBlock(_)
        | FlowItem::SourceLocale(_)
        | FlowItem::SpeakerLine(_)
        | FlowItem::ContentCall(_)
        | FlowItem::Stmt(_)
        | FlowItem::Choice(_)
        | FlowItem::Include(_)
        | FlowItem::Raw(_) => {}
    }
}

fn flow_item_body(item: &FlowItem) -> Option<&[FlowItem]> {
    match item {
        FlowItem::Scope(block) => Some(block.body()),
        FlowItem::If(block) => Some(block.body()),
        FlowItem::IfLet(block) => Some(block.body()),
        FlowItem::Loop(block) => Some(block.body()),
        FlowItem::While(block) => Some(block.body()),
        FlowItem::WhileLet(block) => Some(block.body()),
        FlowItem::For(block) => Some(block.body()),
        FlowItem::BorrowBlock(block) => Some(block.body()),
        FlowItem::SourceLocale(block) => Some(block.body()),
        _ => None,
    }
}

fn collect_dialogue_text_sugar_edits_from_flow_items(
    source: &str,
    items: &[FlowItem],
    edits: &mut Vec<TextEdit>,
    mode: DialogueSugarMode,
    context: &DialogueSugarContext,
) {
    for item in items {
        collect_dialogue_text_sugar_edits_from_flow_item(source, item, edits, mode, context);
    }
}

fn collect_dialogue_text_sugar_edits_from_stmt(
    source: &str,
    stmt: &Stmt,
    edits: &mut Vec<TextEdit>,
    mode: DialogueSugarMode,
    context: &DialogueSugarContext,
) {
    match stmt {
        Stmt::Let {
            expr,
            expr_source,
            expr_range,
            ..
        } => collect_dialogue_text_sugar_edits_from_expr(
            expr,
            expr_source.as_deref(),
            expr_range.as_ref(),
            edits,
            mode,
            context,
        ),
        Stmt::LetElse { else_body, .. } => {
            for stmt in else_body {
                collect_dialogue_text_sugar_edits_from_stmt(source, stmt, edits, mode, context);
            }
        }
        Stmt::LetScope { scope, .. } => {
            for stmt in scope.statements() {
                collect_dialogue_text_sugar_edits_from_stmt(source, stmt, edits, mode, context);
            }
        }
        Stmt::LetLoop { block, .. } => {
            for item in block.body() {
                collect_dialogue_text_sugar_edits_from_flow_item(
                    source, item, edits, mode, context,
                );
            }
        }
        Stmt::LetAwait { await_with, .. } => {
            for branch in await_with.branches() {
                for item in branch.body() {
                    collect_dialogue_text_sugar_edits_from_flow_item(
                        source, item, edits, mode, context,
                    );
                }
            }
        }
        Stmt::Thread(thread) => {
            for item in thread.body() {
                collect_dialogue_text_sugar_edits_from_flow_item(
                    source, item, edits, mode, context,
                );
            }
        }
        Stmt::DeferBlock { statements, .. }
        | Stmt::On {
            body: statements, ..
        }
        | Stmt::UnsafeLifetime {
            body: statements, ..
        }
        | Stmt::If {
            body: statements, ..
        }
        | Stmt::Loop {
            body: statements, ..
        }
        | Stmt::While {
            body: statements, ..
        }
        | Stmt::WhileLet {
            body: statements, ..
        }
        | Stmt::For {
            body: statements, ..
        } => {
            for stmt in statements {
                collect_dialogue_text_sugar_edits_from_stmt(source, stmt, edits, mode, context);
            }
        }
        Stmt::Match { arms, .. } => {
            for arm in arms {
                for stmt in arm.body() {
                    collect_dialogue_text_sugar_edits_from_stmt(source, stmt, edits, mode, context);
                }
            }
        }
        Stmt::LetChoice { .. }
        | Stmt::Return(_)
        | Stmt::Out { .. }
        | Stmt::Goto(_)
        | Stmt::Defer { .. }
        | Stmt::Yield(_)
        | Stmt::Signal { .. }
        | Stmt::LifetimeSet { .. }
        | Stmt::Wait(_)
        | Stmt::Close(_)
        | Stmt::Select(_)
        | Stmt::Break { .. }
        | Stmt::Continue { .. }
        | Stmt::Expr(_)
        | Stmt::Raw(_) => {}
    }
}

fn collect_dialogue_text_sugar_edits_from_expr(
    expr: &Expr,
    expr_source: Option<&str>,
    expr_range: Option<&arcweft_lang_syntax::ast::common::TextRange>,
    edits: &mut Vec<TextEdit>,
    mode: DialogueSugarMode,
    context: &DialogueSugarContext,
) {
    match expr {
        Expr::DialogueCall { content, .. } => {
            let (Some(expr_source), Some(expr_range)) = (expr_source, expr_range) else {
                return;
            };
            let Some(content_start) = expr_source.find(content) else {
                return;
            };
            edits.extend(dialogue_text_canonical_edits(
                content,
                expr_range.start() + content_start,
                mode,
                context,
            ));
        }
        Expr::Try { expr } => {
            collect_dialogue_text_sugar_edits_from_expr(
                expr,
                expr_source,
                expr_range,
                edits,
                mode,
                context,
            );
        }
        _ => {}
    }
}

fn collect_dialogue_content_sugar_edits(
    source: &str,
    content: &DialogueContent,
    edits: &mut Vec<TextEdit>,
    mode: DialogueSugarMode,
    sugar_context: &DialogueSugarContext,
) {
    let Some(base) = dialogue_content_source_base(source, content) else {
        return;
    };
    edits.extend(dialogue_text_canonical_edits(
        content.raw(),
        base,
        mode,
        sugar_context,
    ));
}

pub(crate) fn dialogue_content_source_base(
    source: &str,
    content: &DialogueContent,
) -> Option<usize> {
    let range = content.range();
    if let Some(slice) = source.get(range.start()..range.end()) {
        if slice == content.raw() {
            return Some(range.start());
        }
        if let Some(relative) = slice.find(content.raw()) {
            return Some(range.start() + relative);
        }
    }
    // Nested parser ranges are currently local to their flow body. Use a
    // source search fallback so formatter sugar expansion can still operate
    // until syntax ranges are fully rebased in the typed tree.
    source.find(content.raw())
}

pub(crate) fn dialogue_text_canonical_edits(
    raw: &str,
    base: usize,
    mode: DialogueSugarMode,
    context: &DialogueSugarContext,
) -> Vec<TextEdit> {
    let mut edits = Vec::new();
    let mut cursor = 0;
    let mut inferred_span_stack = Vec::new();
    while cursor < raw.len() {
        let Some(ch) = raw[cursor..].chars().next() else {
            break;
        };
        match ch {
            '\\' => {
                cursor += ch.len_utf8();
                if let Some(escaped) = raw[cursor..].chars().next() {
                    cursor += escaped.len_utf8();
                }
            }
            '｜' if mode == DialogueSugarMode::All => {
                if let Some((end, replacement)) = natural_ruby_edit(raw, cursor) {
                    edits.push(TextEdit {
                        start: base + cursor,
                        end: base + end,
                        replacement,
                    });
                    cursor = end;
                } else {
                    cursor += ch.len_utf8();
                }
            }
            '|' if mode == DialogueSugarMode::All => {
                if let Some((end, replacement)) = compact_ruby_edit(raw, cursor) {
                    edits.push(TextEdit {
                        start: base + cursor,
                        end: base + end,
                        replacement,
                    });
                    cursor = end;
                } else {
                    cursor += ch.len_utf8();
                }
            }
            '$' if mode == DialogueSugarMode::All => {
                if let Some((end, replacement)) = dollar_expr_edit(raw, cursor) {
                    edits.push(TextEdit {
                        start: base + cursor,
                        end: base + end,
                        replacement,
                    });
                    cursor = end;
                } else {
                    cursor += ch.len_utf8();
                }
            }
            '[' => {
                if let Some((end, replacement)) =
                    bracket_dialogue_edit(raw, cursor, &mut inferred_span_stack, mode, context)
                {
                    edits.push(TextEdit {
                        start: base + cursor,
                        end: base + end,
                        replacement,
                    });
                    cursor = end;
                } else if let Some(end) = raw_span_end(raw, cursor) {
                    cursor = end;
                } else {
                    cursor += ch.len_utf8();
                }
            }
            _ => cursor += ch.len_utf8(),
        }
    }
    edits
}

fn natural_ruby_edit(raw: &str, start: usize) -> Option<(usize, String)> {
    let after_marker = start + '｜'.len_utf8();
    let tail = raw.get(after_marker..)?;
    let open = tail.find('《')?;
    let base_text = &tail[..open];
    let ruby_start = after_marker + open + '《'.len_utf8();
    let ruby_tail = raw.get(ruby_start..)?;
    let close = ruby_tail.find('》')?;
    let ruby = &ruby_tail[..close];
    if base_text.is_empty() || ruby.is_empty() {
        return None;
    }
    Some((
        ruby_start + close + '》'.len_utf8(),
        format!("|[{base_text}]({ruby})"),
    ))
}

fn compact_ruby_edit(raw: &str, start: usize) -> Option<(usize, String)> {
    let after_marker = start + '|'.len_utf8();
    let tail = raw.get(after_marker..)?;
    if tail.starts_with('[') {
        return None;
    }
    let open = tail.find('{')?;
    let base_text = &tail[..open];
    if base_text.is_empty()
        || base_text
            .chars()
            .any(|ch| ch.is_whitespace() || matches!(ch, '[' | ']' | '{' | '}' | '#' | '|'))
    {
        return None;
    }
    let ruby_start = after_marker + open + '{'.len_utf8();
    let ruby_tail = raw.get(ruby_start..)?;
    let close = ruby_tail.find('}')?;
    let ruby = &ruby_tail[..close];
    if ruby.is_empty() {
        return None;
    }
    Some((
        ruby_start + close + '}'.len_utf8(),
        format!("|[{base_text}]({ruby})"),
    ))
}

fn dollar_expr_edit(raw: &str, start: usize) -> Option<(usize, String)> {
    let expr_start = start + "$(".len();
    let end = balanced_close(raw, expr_start, '(', ')')?;
    let expr = raw.get(expr_start..end - ')'.len_utf8())?;
    Some((end, format!("#[{expr}]")))
}

fn bracket_dialogue_edit(
    raw: &str,
    start: usize,
    inferred_span_stack: &mut Vec<Option<&'static str>>,
    mode: DialogueSugarMode,
    context: &DialogueSugarContext,
) -> Option<(usize, String)> {
    if mode == DialogueSugarMode::All
        && let Some(body) = raw.get(start..)?.strip_prefix("[raw:")
    {
        let close_relative = raw_colon_close(body)?;
        let raw_body = body[..close_relative].trim_start();
        return Some((
            start + "[raw:".len() + close_relative + ']'.len_utf8(),
            format!("[raw]{raw_body}[/raw]"),
        ));
    }
    let close = raw.get(start + '['.len_utf8()..)?.find(']')? + start + '['.len_utf8();
    let inside = raw.get(start + '['.len_utf8()..close)?.trim();
    let end = close + ']'.len_utf8();
    if inside == "/" {
        return inferred_span_stack
            .pop()
            .map(|family| family.map_or_else(String::new, |family| format!("[/{family}]")))
            .map(|replacement| (end, replacement));
    }
    if mode == DialogueSugarMode::All && inside == "page" {
        return Some((end, "[p]".to_owned()));
    }
    if mode == DialogueSugarMode::All && inside == "wait" {
        return Some((end, "[l]".to_owned()));
    }
    if mode == DialogueSugarMode::All && inside == "nl" {
        return Some((end, "[r]".to_owned()));
    }
    if mode == DialogueSugarMode::All
        && let Some(rest) = inside.strip_prefix("! ")
    {
        return Some((end, format!("[call {rest}]")));
    }
    if inside.starts_with('.') && inside.len() > 1 {
        let (selector, attrs) = split_dialogue_tag_head(inside);
        let selector_name = selector.trim_start_matches('.');
        let family = inferred_rich_text_family(selector_name, attrs, context);
        if let Some(family) = family {
            inferred_span_stack.push(Some(family));
            let attrs = canonical_object_attrs(selector_name, attrs, context)
                .filter(|_| family == "object")
                .unwrap_or_else(|| attrs.to_owned());
            let replacement = if attrs.is_empty() {
                format!("[{family} {selector}]")
            } else {
                format!("[{family} {selector} {attrs}]")
            };
            return Some((end, replacement));
        }
        inferred_span_stack.push(None);
        return Some((end, format!("[mark {selector}]")));
    }
    if mode == DialogueSugarMode::All
        && let Some(time) = inside.strip_prefix("w ")
        && !time.contains('=')
    {
        return Some((end, format!("[w time={}]", time.trim())));
    }
    if mode == DialogueSugarMode::All
        && let Some((tag, body)) = inside.split_once(':')
    {
        let body = body.trim_start();
        if tag == "em" || tag == "strong" {
            return Some((end, format!("[{tag}]{body}[/{tag}]")));
        }
        if let Some(color) = tag.strip_prefix("color ") {
            return Some((
                end,
                format!("[color value=\"{}\"]{body}[/color]", color.trim()),
            ));
        }
    }
    if mode == DialogueSugarMode::All {
        rb_tag_edit(raw, start, inside, end)
    } else {
        None
    }
}

fn raw_colon_close(body: &str) -> Option<usize> {
    let mut depth = 0usize;
    let mut cursor = 0usize;
    while cursor < body.len() {
        let ch = body[cursor..].chars().next()?;
        match ch {
            '\\' => {
                cursor += ch.len_utf8();
                if let Some(escaped) = body[cursor..].chars().next() {
                    cursor += escaped.len_utf8();
                }
            }
            '[' => {
                depth += 1;
                cursor += ch.len_utf8();
            }
            ']' if depth == 0 => return Some(cursor),
            ']' => {
                depth = depth.saturating_sub(1);
                cursor += ch.len_utf8();
            }
            _ => cursor += ch.len_utf8(),
        }
    }
    None
}

fn split_dialogue_tag_head(source: &str) -> (&str, &str) {
    let mut parts = source.splitn(2, char::is_whitespace);
    (
        parts.next().unwrap_or_default(),
        parts.next().unwrap_or_default().trim(),
    )
}

fn inferred_rich_text_family(
    selector: &str,
    attrs: &str,
    context: &DialogueSugarContext,
) -> Option<&'static str> {
    match selector {
        "italic" | "oblique" | "opacity" | "alpha" | "layer" | "object_layer" | "meta"
        | "metadata" | "data" | "z" | "z_index" => Some("style"),
        "horizontal_tb"
        | "vertical_rl"
        | "vertical_lr"
        | "dir"
        | "ruby_over"
        | "ruby_under"
        | "ruby_inter_character" => Some("layout"),
        "offset" | "pos" | "rotate" | "scale" | "skew" => Some("transform"),
        "wave" | "shake" | "arc" | "spin" | "pulse" | "motion" | "typewriter" | "jitter"
        | "shader" | "host" => Some("effect"),
        _ if inferred_text_proxy_type(selector, attrs, context).is_some() => Some("object"),
        _ if !attrs.trim().is_empty() => Some("effect"),
        _ => None,
    }
}

fn inferred_text_proxy_type<'a>(
    selector: &'a str,
    attrs: &'a str,
    context: &'a DialogueSugarContext,
) -> Option<&'a str> {
    text_proxy_type_attr(attrs)
        .filter(|name| context.text_proxy_types.contains(*name))
        .or_else(|| {
            context
                .text_proxy_types
                .contains(selector)
                .then_some(selector)
        })
}

fn canonical_object_attrs(
    selector: &str,
    attrs: &str,
    context: &DialogueSugarContext,
) -> Option<String> {
    let proxy_type = inferred_text_proxy_type(selector, attrs, context)?;
    if text_proxy_type_attr(attrs).is_some() {
        Some(attrs.to_owned())
    } else if attrs.trim().is_empty() {
        Some(format!("type={proxy_type}"))
    } else {
        Some(format!("type={proxy_type} {}", attrs.trim()))
    }
}

fn text_proxy_type_attr(attrs: &str) -> Option<&str> {
    find_tag_attr(attrs, "type")
        .or_else(|| find_tag_attr(attrs, "struct"))
        .or_else(|| find_tag_attr(attrs, "proxy"))
}

fn find_tag_attr<'a>(attrs: &'a str, key: &str) -> Option<&'a str> {
    attrs.split_whitespace().find_map(|part| {
        let (name, value) = part.split_once('=')?;
        (name == key).then(|| value.trim_matches('"'))
    })
}

fn collect_text_proxy_type_names(parsed: &ParsedSource) -> BTreeSet<String> {
    parsed
        .typed_tree()
        .items()
        .iter()
        .filter_map(|item| match item {
            Item::Struct(item)
                if item.attrs().iter().any(is_text_proxy_attribute) && !item.name().is_empty() =>
            {
                Some(item.name().to_owned())
            }
            _ => None,
        })
        .collect()
}

fn is_text_proxy_attribute(attr: &Attribute) -> bool {
    matches!(attr.name(), "text_proxy" | "rich_text_proxy")
}

fn rb_tag_edit(raw: &str, _start: usize, inside: &str, open_end: usize) -> Option<(usize, String)> {
    let attrs = inside.strip_prefix("rb")?.trim();
    let ruby = ruby_attr_value(attrs)?;
    let tail = raw.get(open_end..)?;
    let close = tail.find("[/rb]")?;
    let body = raw.get(open_end..open_end + close)?;
    Some((
        open_end + close + "[/rb]".len(),
        format!("[ruby rt=\"{ruby}\"]{body}[/ruby]"),
    ))
}

fn ruby_attr_value(attrs: &str) -> Option<&str> {
    let value = attrs.trim().strip_prefix("rt")?.trim_start();
    let value = value.strip_prefix('=')?.trim_start();
    if let Some(quoted) = value.strip_prefix('"') {
        return quoted.find('"').map(|end| &quoted[..end]);
    }
    let end = value.find(char::is_whitespace).unwrap_or(value.len());
    (end > 0).then_some(&value[..end])
}

fn raw_span_end(raw: &str, start: usize) -> Option<usize> {
    let body_start = start + "[raw]".len();
    raw.get(start..)?.starts_with("[raw]").then_some(())?;
    let close = raw.get(body_start..)?.find("[/raw]")?;
    Some(body_start + close + "[/raw]".len())
}

fn balanced_close(raw: &str, start: usize, open: char, close: char) -> Option<usize> {
    let mut depth = 1_u32;
    for (relative, ch) in raw.get(start..)?.char_indices() {
        if ch == open {
            depth += 1;
        } else if ch == close {
            depth -= 1;
            if depth == 0 {
                return Some(start + relative + close.len_utf8());
            }
        }
    }
    None
}
