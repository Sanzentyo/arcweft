use crate::{documents::DocumentSnapshot, profiles::LspProfile, uri_key::LspUriKey};
use arcweft_lang_hir::model::{HirDialogue, HirFlowItem, HirModule};
use arcweft_lang_syntax::ast::common::TextRange;
#[cfg(test)]
use arcweft_lang_syntax::ast::dialogue::LineOptions;
use arcweft_lang_syntax::ast::dialogue::{
    DialogueContent, DialogueTag, DialogueTagArg, DialogueToken,
};
#[cfg(test)]
use arcweft_lang_syntax::ast::flow::{
    AwaitWith, FlowItem, ForBlock, IfBlock, IfLetBlock, LoopBlock, ScopeBlock, SourceLocaleBlock,
    Stmt, WhileBlock, WhileLetBlock,
};
#[cfg(test)]
use arcweft_lang_syntax::ast::items::Item;
use arcweft_presentation::rich_text::{RichTextTagFamily, inferred_tag_family};
use arcweft_render_text::{
    LineDisplaySpec, RichTextSettingSource, RichTextSourceRange, RichTextStyleContribution,
};
use std::ops::Range;

/// Effective dialogue display context at a document byte offset.
#[derive(Clone, Debug)]
pub(crate) struct EffectiveDialogueCascade {
    pub(crate) spec: LineDisplaySpec,
    pub(crate) selected_path: Option<String>,
}

impl EffectiveDialogueCascade {
    pub(crate) fn selected_contributions(&self) -> Vec<&RichTextStyleContribution> {
        self.selected_path.as_ref().map_or_else(
            || self.spec.style_contributions.iter().collect(),
            |path| {
                self.spec
                    .style_contributions
                    .iter()
                    .filter(|contribution| {
                        contribution.path == *path
                            || contribution
                                .path
                                .strip_prefix(path)
                                .is_some_and(|tail| tail.starts_with('.'))
                    })
                    .collect()
            },
        )
    }
}

pub(crate) fn effective_dialogue_cascade_at(
    profile: &LspProfile,
    document: &DocumentSnapshot,
    offset: usize,
) -> Option<EffectiveDialogueCascade> {
    let accepted = profile.accepted_environment()?;
    let uri = LspUriKey::from_uri(document.uri());
    let accepted_identity = accepted.project().source_identity_by_uri(&uri)?;
    let overlay = accepted.overlays().get(&uri)?;
    if overlay.version() != document.version()
        || overlay.logical_identity() != accepted_identity
        || accepted
            .project()
            .source(accepted_identity)?
            .document()
            .text()
            != document.text()
    {
        return None;
    }
    let module = accepted.project().module_key(accepted_identity)?;
    // Prove this source/module pair is still present in the accepted HIR before
    // using the compiler-owned linked order that indexes the runtime catalog.
    accepted.project().hir(&module).ok()?;
    let dialogues = collect_dialogues(accepted.compiled().linked_hir());
    let (dialogue_index, dialogue) = dialogues.iter().enumerate().find(|(_, dialogue)| {
        dialogue.source_module() == Some(module.module())
            && dialogue_contains_offset(dialogue, offset)
    })?;
    let report = accepted.compiled().runtime_plan();
    if report.line_display_catalog.dialogue_revision()
        != accepted.compiled().dialogue_profile().revision()
    {
        return None;
    }
    let spec = report.line_display_catalog.lines().get(dialogue_index)?;
    let selected_path = hir_dialogue_style_path(dialogue, offset);
    Some(EffectiveDialogueCascade {
        spec: spec.clone(),
        selected_path,
    })
}

fn dialogue_contains_offset(dialogue: &HirDialogue, offset: usize) -> bool {
    dialogue_content_contains_offset(dialogue, offset)
        || dialogue
            .speaker_surface()
            .is_some_and(|surface| range_contains(&surface.source_line_range(), offset))
        || dialogue
            .style_range()
            .is_some_and(|range| range_contains(range, offset))
        || dialogue
            .rich_text_range()
            .is_some_and(|range| range_contains(range, offset))
        || dialogue
            .args()
            .iter()
            .any(|arg| range_contains(arg.value_range(), offset))
}

fn hir_dialogue_style_path(dialogue: &HirDialogue, offset: usize) -> Option<String> {
    if let Some(range) = dialogue.style_range()
        && range_contains(range, offset)
    {
        return dialogue
            .style_raw()
            .and_then(|raw| style_value_path_at("style", raw, *range, offset))
            .or_else(|| Some("style".to_owned()));
    }
    if let Some(range) = dialogue.rich_text_range()
        && range_contains(range, offset)
    {
        return dialogue
            .rich_text_raw()
            .and_then(|raw| style_value_path_at("rich_text", raw, *range, offset))
            .or_else(|| Some("rich_text".to_owned()));
    }
    dialogue
        .args()
        .iter()
        .find_map(|arg| {
            range_contains(arg.value_range(), offset).then(|| {
                style_value_path_at(arg.name(), arg.raw_value(), *arg.value_range(), offset)
                    .unwrap_or_else(|| arg.name().to_owned())
            })
        })
        .or_else(|| inline_content_style_path(dialogue.content(), offset))
}

fn collect_dialogues(module: &HirModule) -> Vec<&HirDialogue> {
    let mut dialogues = Vec::new();
    for flow in module.flows() {
        collect_flow_item_dialogues(flow.body(), &mut dialogues);
    }
    dialogues
}

fn dialogue_content_contains_offset(dialogue: &HirDialogue, offset: usize) -> bool {
    dialogue.content().content_offset(offset).is_some()
}

pub(crate) fn source_range(source: &RichTextSettingSource) -> Option<RichTextSourceRange> {
    match source {
        RichTextSettingSource::SourceFile { range, .. } => *range,
        RichTextSettingSource::EngineDefault { .. } => None,
    }
}

#[cfg(test)]
pub(crate) fn style_path_at(items: &[Item], offset: usize) -> Option<String> {
    items.iter().find_map(|item| match item {
        Item::Flow(flow) => style_path_from_flow_items(flow.body(), offset),
        _ => None,
    })
}

#[cfg(test)]
fn style_path_from_flow_items(items: &[FlowItem], offset: usize) -> Option<String> {
    items.iter().find_map(|item| match item {
        FlowItem::SpeakerLine(line) => line_options_style_path(line.options(), offset)
            .or_else(|| inline_content_style_path(line.content(), offset)),
        FlowItem::ContentCall(call) => line_options_style_path(call.options(), offset)
            .or_else(|| inline_content_style_path(call.content(), offset)),
        FlowItem::Stmt(stmt) => style_path_from_stmt(stmt, offset),
        FlowItem::If(block) => nested_style_path(block, offset),
        FlowItem::IfLet(block) => nested_style_path(block, offset),
        FlowItem::Match(block) => block
            .arms()
            .iter()
            .find_map(|arm| style_path_from_flow_items(arm.body(), offset)),
        FlowItem::Loop(block) => nested_style_path(block, offset),
        FlowItem::While(block) => nested_style_path(block, offset),
        FlowItem::WhileLet(block) => nested_style_path(block, offset),
        FlowItem::For(block) => nested_style_path(block, offset),
        FlowItem::Select(block) => block
            .branches()
            .iter()
            .find_map(|branch| style_path_from_flow_items(branch.body(), offset)),
        FlowItem::SourceLocale(block) => nested_style_path(block, offset),
        FlowItem::Scope(block) => nested_style_path(block, offset),
        FlowItem::AwaitWith(await_with) => style_path_from_await_with(await_with, offset),
        FlowItem::Choice(_) | FlowItem::Include(_) | FlowItem::Raw(_) => None,
    })
}

#[cfg(test)]
trait HasFlowBody {
    fn body(&self) -> &[FlowItem];
}

#[cfg(test)]
impl HasFlowBody for IfBlock {
    fn body(&self) -> &[FlowItem] {
        self.body()
    }
}

#[cfg(test)]
impl HasFlowBody for IfLetBlock {
    fn body(&self) -> &[FlowItem] {
        self.body()
    }
}

#[cfg(test)]
impl HasFlowBody for LoopBlock {
    fn body(&self) -> &[FlowItem] {
        self.body()
    }
}

#[cfg(test)]
impl HasFlowBody for WhileBlock {
    fn body(&self) -> &[FlowItem] {
        self.body()
    }
}

#[cfg(test)]
impl HasFlowBody for WhileLetBlock {
    fn body(&self) -> &[FlowItem] {
        self.body()
    }
}

#[cfg(test)]
impl HasFlowBody for ForBlock {
    fn body(&self) -> &[FlowItem] {
        self.body()
    }
}

#[cfg(test)]
impl HasFlowBody for SourceLocaleBlock {
    fn body(&self) -> &[FlowItem] {
        self.body()
    }
}

#[cfg(test)]
impl HasFlowBody for ScopeBlock {
    fn body(&self) -> &[FlowItem] {
        self.body()
    }
}

#[cfg(test)]
fn nested_style_path(block: &impl HasFlowBody, offset: usize) -> Option<String> {
    style_path_from_flow_items(block.body(), offset)
}

#[cfg(test)]
fn style_path_from_await_with(await_with: &AwaitWith, offset: usize) -> Option<String> {
    await_with
        .branches()
        .iter()
        .find_map(|branch| style_path_from_flow_items(branch.body(), offset))
}

#[cfg(test)]
fn style_path_from_stmt(stmt: &Stmt, offset: usize) -> Option<String> {
    match stmt {
        Stmt::Let {
            expr_source,
            expr_range,
            ..
        } => expr_source
            .as_deref()
            .zip(expr_range.as_ref())
            .and_then(|(source, range)| call_option_style_path(source, range, offset)),
        Stmt::LetElse { else_body, .. } => style_path_from_stmts(else_body, offset),
        Stmt::LetScope { scope, .. } => style_path_from_stmts(scope.statements(), offset),
        Stmt::LetLoop { block, .. } => style_path_from_flow_items(block.body(), offset),
        Stmt::LetAwait { await_with, .. } => style_path_from_await_with(await_with, offset),
        Stmt::Thread(thread) => style_path_from_flow_items(thread.body(), offset),
        Stmt::DeferBlock { statements, .. }
        | Stmt::On {
            body: statements, ..
        }
        | Stmt::UnsafeLifetime {
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
        } => style_path_from_stmts(statements, offset),
        Stmt::If {
            body, else_body, ..
        } => {
            style_path_from_stmts(body, offset).or_else(|| style_path_from_stmts(else_body, offset))
        }
        Stmt::Match { arms, .. } => arms
            .iter()
            .find_map(|arm| style_path_from_stmts(arm.body(), offset)),
        Stmt::Assertion(_)
        | Stmt::LetChoice { .. }
        | Stmt::LetActionReceive { .. }
        | Stmt::Assign { .. }
        | Stmt::Return { expr: _, .. }
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
        | Stmt::Expr { expr: _, .. }
        | Stmt::Raw(_) => None,
    }
}

#[cfg(test)]
fn style_path_from_stmts(statements: &[Stmt], offset: usize) -> Option<String> {
    statements
        .iter()
        .find_map(|stmt| style_path_from_stmt(stmt, offset))
}

#[cfg(test)]
fn line_options_style_path(options: &LineOptions, offset: usize) -> Option<String> {
    if let Some(range) = options.style_range()
        && range_contains(&range, offset)
    {
        return options
            .style_raw()
            .and_then(|raw| style_value_path_at("style", raw, range, offset))
            .or_else(|| Some("style".to_owned()));
    }
    if let Some(range) = options.rich_text_range()
        && range_contains(&range, offset)
    {
        return options
            .rich_text_raw()
            .and_then(|raw| style_value_path_at("rich_text", raw, range, offset))
            .or_else(|| Some("rich_text".to_owned()));
    }
    options.args().iter().find_map(|arg| {
        range_contains(arg.value_range(), offset).then(|| {
            style_value_path_at(arg.name(), arg.raw_value(), *arg.value_range(), offset)
                .unwrap_or_else(|| arg.name().to_owned())
        })
    })
}

fn inline_content_style_path(content: &DialogueContent, offset: usize) -> Option<String> {
    let content_offset = content.content_offset(offset)?;
    content.tokens().iter().find_map(|token| match token {
        DialogueToken::Tag(tag) if range_contains(&tag.range(), content_offset) => {
            inline_tag_style_path(tag, false, content_offset)
        }
        DialogueToken::InferredTag(tag) if range_contains(&tag.range(), content_offset) => {
            inline_tag_style_path(tag, true, content_offset)
        }
        _ => None,
    })
}

fn inline_tag_style_path(tag: &DialogueTag, inferred: bool, offset: usize) -> Option<String> {
    if inferred {
        return inferred_inline_style_path(tag, offset);
    }

    match tag.name() {
        "style" => selected_inline_style_path(tag, style_selector_path, offset),
        "layout" => selected_inline_style_path(tag, layout_selector_path, offset),
        "transform" => selected_inline_style_path(tag, transform_selector_path, offset),
        "effect" | "fx" => selected_inline_style_path(tag, effect_selector_path, offset),
        "color" | "font" | "size" | "em" | "strong" | "i" | "italic" | "oblique" | "slant" => {
            direct_inline_style_path(tag, offset)
        }
        _ => None,
    }
}

fn selected_inline_style_path(
    tag: &DialogueTag,
    path_for: fn(&str, &str, &str) -> Option<String>,
    offset: usize,
) -> Option<String> {
    let selector = tag.arguments().first().and_then(|argument| {
        if argument.name().is_some() {
            return None;
        }
        let value = argument.value()?;
        dot_selector(value.value(), value.range())
    });
    selector_or_attr_path(
        selector.as_ref().map_or("", |(name, _)| *name),
        selector.map(|(_, range)| range),
        tag.arguments(),
        path_for,
        offset,
    )
}

fn inferred_inline_style_path(tag: &DialogueTag, offset: usize) -> Option<String> {
    let (selector, selector_range) = dot_selector(tag.name(), tag.name_range())?;
    let path_for = match inferred_tag_family(selector, tag.attrs())? {
        RichTextTagFamily::Style => style_selector_path,
        RichTextTagFamily::Layout => layout_selector_path,
        RichTextTagFamily::Transform => transform_selector_path,
        RichTextTagFamily::Effect => effect_selector_path,
        RichTextTagFamily::Marker => return None,
    };
    selector_or_attr_path(
        selector,
        Some(selector_range),
        tag.arguments(),
        path_for,
        offset,
    )
}

fn selector_or_attr_path(
    selector: &str,
    selector_range: Option<TextRange>,
    arguments: &[DialogueTagArg],
    path_for: fn(&str, &str, &str) -> Option<String>,
    offset: usize,
) -> Option<String> {
    if selector_range.is_some_and(|range| range_contains(&range, offset)) {
        return path_for(selector, "", "");
    }
    arguments.iter().find_map(|argument| {
        let name = argument.name()?;
        let value = argument.value()?;
        range_contains(&value.range(), offset)
            .then(|| path_for(selector, name, value.value()))
            .flatten()
    })
}

fn dot_selector(source: &str, range: TextRange) -> Option<(&str, TextRange)> {
    let selector = source.strip_prefix('.')?;
    let start = range.end().checked_sub(selector.len())?;
    Some((selector, TextRange::new(start, range.end())))
}

fn direct_inline_style_path(tag: &DialogueTag, offset: usize) -> Option<String> {
    let selected = range_contains(&tag.name_range(), offset)
        || tag
            .arguments()
            .iter()
            .any(|argument| range_contains(&argument.range(), offset));
    if !selected {
        return None;
    }
    match tag.name() {
        "color" => Some("rich_text.text.color".to_owned()),
        "font" => Some("rich_text.text.font".to_owned()),
        "size" => Some("rich_text.text.size".to_owned()),
        "em" | "strong" | "i" | "italic" | "oblique" | "slant" => {
            Some("rich_text.text.style".to_owned())
        }
        _ => None,
    }
}

fn style_selector_path(selector: &str, name: &str, _value: &str) -> Option<String> {
    (!selector.is_empty() || !name.is_empty()).then(|| "rich_text.text.style".to_owned())
}

fn layout_selector_path(selector: &str, name: &str, _value: &str) -> Option<String> {
    match name {
        "" if matches!(
            selector,
            "vertical_rl" | "vertical" | "vertical_lr" | "horizontal_tb"
        ) =>
        {
            Some("rich_text.layout.writing_mode".to_owned())
        }
        "" if matches!(
            selector,
            "ruby_over" | "ruby_under" | "ruby_inter_character"
        ) =>
        {
            Some("rich_text.ruby.position".to_owned())
        }
        "ruby_size" | "size" if selector.starts_with("ruby_") => {
            Some("rich_text.ruby.size".to_owned())
        }
        "ruby_gap" | "gap" if selector.starts_with("ruby_") => {
            Some("rich_text.ruby.gap".to_owned())
        }
        "ruby_overhang" | "overhang" => Some("rich_text.ruby.overhang".to_owned()),
        "ruby_collision_gap" | "collision_gap" => Some("rich_text.ruby.collision_gap".to_owned()),
        "jlreq" | "strictness" | "kinsoku" => Some("rich_text.layout.jlreq".to_owned()),
        "latin" | "vertical_latin" => Some("rich_text.layout.vertical_latin".to_owned()),
        "dir" | "direction" => Some("rich_text.layout.direction".to_owned()),
        "column_gap" | "gap" => Some("rich_text.layout.column_gap".to_owned()),
        _ => None,
    }
}

fn transform_selector_path(selector: &str, name: &str, _value: &str) -> Option<String> {
    if name.is_empty() {
        Some("rich_text.transform.kind".to_owned())
    } else {
        Some(format!("rich_text.transform.{name}"))
    }
    .filter(|_| !selector.is_empty())
}

fn effect_selector_path(selector: &str, name: &str, _value: &str) -> Option<String> {
    if selector.is_empty() {
        return None;
    }
    if name.is_empty() {
        Some("rich_text.effect".to_owned())
    } else {
        Some(format!("rich_text.effect.{selector}.{name}"))
    }
}

#[cfg(test)]
fn call_option_style_path(source: &str, range: &TextRange, offset: usize) -> Option<String> {
    if !range_contains(range, offset) {
        return None;
    }
    let relative = offset.saturating_sub(range.start());
    let open = find_top_level_char(source, '(')?;
    let close = source.rfind(')')?;
    if relative <= open || relative > close {
        return None;
    }
    split_top_level_ranges(&source[open + '('.len_utf8()..close], ',')
        .into_iter()
        .find_map(|(arg_start, raw)| {
            let leading = raw.len() - raw.trim_start().len();
            let trimmed = raw.trim();
            let (name, value) = trimmed.split_once('=')?;
            let value_start =
                open + '('.len_utf8() + arg_start + leading + name.len() + '='.len_utf8();
            let value_end = value_start + value.trim().len();
            let path = name.trim();
            let absolute_value_range = TextRange::new(
                range.start() + value_start,
                range.start() + value_start + value.trim().len(),
            );
            (value_start <= relative && relative <= value_end).then(|| {
                style_value_path_at(path, value.trim(), absolute_value_range, offset)
                    .unwrap_or_else(|| path.to_owned())
            })
        })
}

fn style_value_path_at(
    root_path: &str,
    raw: &str,
    value_range: TextRange,
    offset: usize,
) -> Option<String> {
    if !range_contains(&value_range, offset) {
        return None;
    }
    let (callee, args_range) = raw_call_parts_with_args_range(raw)?;
    match style_call_path_mode(callee)? {
        StyleCallPathMode::NestedRecord | StyleCallPathMode::LeafRecord => {
            style_arg_path_at(root_path, raw, args_range, value_range.start(), offset)
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StyleCallPathMode {
    NestedRecord,
    LeafRecord,
}

fn style_call_path_mode(callee: &str) -> Option<StyleCallPathMode> {
    match callee.rsplit('.').next().unwrap_or(callee) {
        "text_style" | "dialogue_style" | "style" | "rich_text_style" => {
            Some(StyleCallPathMode::NestedRecord)
        }
        "ruby_style" | "layout_style" => Some(StyleCallPathMode::LeafRecord),
        _ => None,
    }
}

fn style_arg_path_at(
    root_path: &str,
    raw: &str,
    args_range: Range<usize>,
    raw_absolute_start: usize,
    offset: usize,
) -> Option<String> {
    split_top_level_range_indices(&raw[args_range.clone()], ',')
        .into_iter()
        .find_map(|arg_range| {
            let arg_source =
                &raw[args_range.start + arg_range.start..args_range.start + arg_range.end];
            let leading = arg_source.len() - arg_source.trim_start().len();
            let trailing = arg_source.len() - arg_source.trim_end().len();
            let trimmed_start = args_range.start + arg_range.start + leading;
            let trimmed_end = args_range.start + arg_range.end - trailing;
            let absolute_arg = TextRange::new(
                raw_absolute_start + trimmed_start,
                raw_absolute_start + trimmed_end,
            );
            if !range_contains(&absolute_arg, offset) {
                return None;
            }
            let trimmed = arg_source.trim();
            let Some(equals) = find_top_level_char(trimmed, '=') else {
                return style_value_path_at(root_path, trimmed, absolute_arg, offset)
                    .or_else(|| Some(root_path.to_owned()));
            };
            let name = trimmed[..equals].trim();
            if name.is_empty() {
                return Some(root_path.to_owned());
            }
            let value = &trimmed[equals + '='.len_utf8()..];
            let value_leading = value.len() - value.trim_start().len();
            let value = value.trim();
            let child_path = format!("{root_path}.{name}");
            if value.is_empty() {
                return Some(child_path);
            }
            let value_start =
                raw_absolute_start + trimmed_start + equals + '='.len_utf8() + value_leading;
            let value_range = TextRange::new(value_start, value_start + value.len());
            style_value_path_at(&child_path, value, value_range, offset).or(Some(child_path))
        })
}

fn raw_call_parts_with_args_range(raw: &str) -> Option<(&str, Range<usize>)> {
    let open = find_top_level_char(raw, '(')?;
    let close = raw.rfind(')')?;
    (close > open && raw[close + ')'.len_utf8()..].trim().is_empty())
        .then(|| (raw[..open].trim(), open + '('.len_utf8()..close))
}

fn split_top_level_range_indices(source: &str, delimiter: char) -> Vec<Range<usize>> {
    let mut ranges = Vec::new();
    let mut start = 0usize;
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escaped = false;
    for (offset, ch) in source.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match ch {
            '\\' if in_string => escaped = true,
            '"' => in_string = !in_string,
            '(' | '[' | '{' if !in_string => depth += 1,
            ')' | ']' | '}' if !in_string => depth = depth.saturating_sub(1),
            _ if ch == delimiter && !in_string && depth == 0 => {
                ranges.push(start..offset);
                start = offset + ch.len_utf8();
            }
            _ => {}
        }
    }
    ranges.push(start..source.len());
    ranges
}

#[cfg(test)]
fn split_top_level_ranges(source: &str, delimiter: char) -> Vec<(usize, &str)> {
    let mut ranges = Vec::new();
    let mut start = 0usize;
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escaped = false;
    for (offset, ch) in source.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match ch {
            '\\' if in_string => escaped = true,
            '"' => in_string = !in_string,
            '(' | '[' | '{' if !in_string => depth += 1,
            ')' | ']' | '}' if !in_string => depth = depth.saturating_sub(1),
            _ if ch == delimiter && !in_string && depth == 0 => {
                ranges.push((start, &source[start..offset]));
                start = offset + ch.len_utf8();
            }
            _ => {}
        }
    }
    ranges.push((start, &source[start..]));
    ranges
}

fn find_top_level_char(source: &str, needle: char) -> Option<usize> {
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escaped = false;
    for (offset, ch) in source.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match ch {
            '\\' if in_string => escaped = true,
            '"' => in_string = !in_string,
            _ if ch == needle && !in_string && depth == 0 => return Some(offset),
            '(' | '[' | '{' if !in_string => depth += 1,
            ')' | ']' | '}' if !in_string => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    None
}

fn range_contains(range: &TextRange, offset: usize) -> bool {
    range.start() <= offset && offset <= range.end()
}

fn collect_flow_item_dialogues<'a>(items: &'a [HirFlowItem], dialogues: &mut Vec<&'a HirDialogue>) {
    for item in items {
        match item {
            HirFlowItem::Dialogue(dialogue) => dialogues.push(dialogue),
            HirFlowItem::LetLoop { block, .. } | HirFlowItem::Loop(block) => {
                collect_flow_item_dialogues(block.body(), dialogues);
            }
            HirFlowItem::LetAwait { await_with, .. } | HirFlowItem::Await(await_with) => {
                for branch in await_with.branches() {
                    collect_flow_item_dialogues(branch.body(), dialogues);
                }
            }
            HirFlowItem::Thread(thread) => collect_flow_item_dialogues(thread.body(), dialogues),
            HirFlowItem::If(block) => {
                collect_flow_item_dialogues(block.body(), dialogues);
                collect_flow_item_dialogues(block.else_body(), dialogues);
            }
            HirFlowItem::IfLet(block) => {
                collect_flow_item_dialogues(block.body(), dialogues);
                collect_flow_item_dialogues(block.else_body(), dialogues);
            }
            HirFlowItem::Match(block) => {
                for arm in block.arms() {
                    collect_flow_item_dialogues(arm.body(), dialogues);
                }
            }
            HirFlowItem::While(block) => collect_flow_item_dialogues(block.body(), dialogues),
            HirFlowItem::WhileLet(block) => collect_flow_item_dialogues(block.body(), dialogues),
            HirFlowItem::For(block) => collect_flow_item_dialogues(block.body(), dialogues),
            HirFlowItem::Select(block) => {
                for branch in block.branches() {
                    collect_flow_item_dialogues(branch.body(), dialogues);
                }
            }
            HirFlowItem::SourceLocale(block) => {
                collect_flow_item_dialogues(block.body(), dialogues);
            }
            HirFlowItem::Scope(block) => collect_flow_item_dialogues(block.body(), dialogues),
            HirFlowItem::Stmt(_)
            | HirFlowItem::Choice(_)
            | HirFlowItem::LetChoice { .. }
            | HirFlowItem::LetScope { .. }
            | HirFlowItem::Include(_) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::style_path_at;
    use arcweft_lang_syntax::parser::parse_source;

    #[test]
    fn inline_style_paths_project_multiline_lf_and_crlf_offsets() {
        let source_lf = "flow opening {\n    narrator:\n        Intro\n        [.ruby_over ruby_size=11px]text[/]\n}\n";
        for source in [source_lf.to_owned(), source_lf.replace('\n', "\r\n")] {
            let offset = source.find("11px").expect("ruby size value") + 1;
            let parsed = parse_source(&source);
            assert!(
                parsed.errors().is_empty(),
                "unexpected parser errors: {:?}",
                parsed.errors()
            );
            assert_eq!(
                style_path_at(parsed.typed_tree().items(), offset).as_deref(),
                Some("rich_text.ruby.size")
            );
        }
    }

    #[test]
    fn inline_effect_paths_use_typed_ranges_for_quoted_closing_brackets() {
        let source_lf = "flow opening {\n    narrator:\n        [.sparkle note=\"contains ] safely\" amp=2px]text[/]\n}\n";
        for source in [source_lf.to_owned(), source_lf.replace('\n', "\r\n")] {
            let parsed = parse_source(&source);
            assert!(
                parsed.errors().is_empty(),
                "unexpected parser errors: {:?}",
                parsed.errors()
            );

            let selector_offset = source.find("sparkle").expect("effect selector") + 2;
            assert_eq!(
                style_path_at(parsed.typed_tree().items(), selector_offset).as_deref(),
                Some("rich_text.effect")
            );
            let note_offset = source.find("safely").expect("quoted note") + 2;
            assert_eq!(
                style_path_at(parsed.typed_tree().items(), note_offset).as_deref(),
                Some("rich_text.effect.sparkle.note")
            );
            let amp_offset = source.find("2px").expect("effect amplitude") + 1;
            assert_eq!(
                style_path_at(parsed.typed_tree().items(), amp_offset).as_deref(),
                Some("rich_text.effect.sparkle.amp")
            );
        }
    }

    #[test]
    fn inline_direct_style_paths_cover_named_argument_keys_and_values() {
        let source_lf =
            "flow opening {\n    narrator:\n        [color value=\"#ff4050\"]text[/color]\n}\n";
        for source in [source_lf.to_owned(), source_lf.replace('\n', "\r\n")] {
            let parsed = parse_source(&source);
            assert!(
                parsed.errors().is_empty(),
                "unexpected parser errors: {:?}",
                parsed.errors()
            );

            for selected in ["value", "=", "#ff4050"] {
                let offset = source.find(selected).expect("direct color argument")
                    + selected.len().saturating_sub(1);
                assert_eq!(
                    style_path_at(parsed.typed_tree().items(), offset).as_deref(),
                    Some("rich_text.text.color"),
                    "selection `{selected}`"
                );
            }
        }
    }
}
