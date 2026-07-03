use crate::documents::DocumentSnapshot;
use arcweft_compiler::lower::lower_source_runtime_plan_with_stats_and_options;
use arcweft_lang_hir::lower::lower_to_hir;
use arcweft_lang_hir::model::{HirDialogue, HirFlowItem, HirModule};
use arcweft_lang_syntax::ast::common::TextRange;
use arcweft_lang_syntax::ast::dialogue::{DialogueContent, LineOptions};
use arcweft_lang_syntax::ast::flow::{
    AwaitWith, BorrowBlock, FlowItem, ForBlock, IfBlock, IfLetBlock, LoopBlock, ScopeBlock,
    SourceLocaleBlock, Stmt, WhileBlock, WhileLetBlock,
};
use arcweft_lang_syntax::ast::items::Item;
use arcweft_lang_syntax::parser::parse_source;
use arcweft_render_text::{
    LineDisplaySpec, RichTextSettingSource, RichTextSourceRange, RichTextStyleContribution,
};
use arcweft_runtime_plan::flow::RuntimePlanLowerOptions;
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
    document: &DocumentSnapshot,
    offset: usize,
    selected_dialogue_defaults: Option<&str>,
) -> Option<EffectiveDialogueCascade> {
    let parsed = parse_source(document.text());
    if !parsed.errors().is_empty() {
        return None;
    }
    let selected_path = style_path_at(parsed.typed_tree().items(), offset);
    let syntax_ranges = collect_syntax_dialogue_ranges(parsed.typed_tree().items());
    let hir = lower_to_hir(parsed.typed_tree()).ok()?;
    let dialogues = collect_dialogues(&hir);
    let dialogue_index = syntax_ranges
        .iter()
        .position(|range| range_contains(range, offset))
        .or_else(|| {
            dialogues
                .iter()
                .position(|dialogue| dialogue_content_contains_offset(dialogue, offset))
        })?;
    if dialogue_index >= dialogues.len() {
        return None;
    }
    let runtime_options = selected_dialogue_defaults
        .map_or_else(RuntimePlanLowerOptions::default, |id| {
            RuntimePlanLowerOptions::default().with_dialogue_defaults(id)
        });
    let report = lower_source_runtime_plan_with_stats_and_options(&hir, &runtime_options).ok()?;
    let spec = report.line_display_catalog.lines().get(dialogue_index)?;
    Some(EffectiveDialogueCascade {
        spec: spec.clone(),
        selected_path,
    })
}

fn collect_syntax_dialogue_ranges(items: &[Item]) -> Vec<TextRange> {
    let mut ranges = Vec::new();
    for item in items {
        match item {
            Item::Flow(flow) => collect_syntax_dialogue_ranges_from_flow(flow.body(), &mut ranges),
            Item::FlowItem(item) => collect_syntax_dialogue_ranges_from_flow(
                std::slice::from_ref(item.as_ref()),
                &mut ranges,
            ),
            _ => {}
        }
    }
    ranges
}

fn collect_syntax_dialogue_ranges_from_flow(items: &[FlowItem], ranges: &mut Vec<TextRange>) {
    for item in items {
        match item {
            FlowItem::SpeakerLine(line) => ranges.push(*line.range()),
            FlowItem::ContentCall(call) => ranges.push(*call.range()),
            FlowItem::If(block) => collect_syntax_dialogue_ranges_from_flow(block.body(), ranges),
            FlowItem::IfLet(block) => {
                collect_syntax_dialogue_ranges_from_flow(block.body(), ranges);
                collect_syntax_dialogue_ranges_from_flow(block.else_body(), ranges);
            }
            FlowItem::Match(block) => {
                for arm in block.arms() {
                    collect_syntax_dialogue_ranges_from_flow(arm.body(), ranges);
                }
            }
            FlowItem::Loop(block) => collect_syntax_dialogue_ranges_from_flow(block.body(), ranges),
            FlowItem::While(block) => {
                collect_syntax_dialogue_ranges_from_flow(block.body(), ranges);
            }
            FlowItem::WhileLet(block) => {
                collect_syntax_dialogue_ranges_from_flow(block.body(), ranges);
            }
            FlowItem::For(block) => collect_syntax_dialogue_ranges_from_flow(block.body(), ranges),
            FlowItem::Select(block) => {
                for branch in block.branches() {
                    collect_syntax_dialogue_ranges_from_flow(branch.body(), ranges);
                }
            }
            FlowItem::BorrowBlock(block) => {
                collect_syntax_dialogue_ranges_from_flow(block.body(), ranges);
            }
            FlowItem::SourceLocale(block) => {
                collect_syntax_dialogue_ranges_from_flow(block.body(), ranges);
            }
            FlowItem::Scope(block) => {
                collect_syntax_dialogue_ranges_from_flow(block.body(), ranges);
            }
            FlowItem::AwaitWith(await_with) => {
                for branch in await_with.branches() {
                    collect_syntax_dialogue_ranges_from_flow(branch.body(), ranges);
                }
            }
            FlowItem::Stmt(_) | FlowItem::Choice(_) | FlowItem::Include(_) | FlowItem::Raw(_) => {}
        }
    }
}

fn collect_dialogues(module: &HirModule) -> Vec<&HirDialogue> {
    let mut dialogues = Vec::new();
    for flow in module.flows() {
        collect_flow_item_dialogues(flow.body(), &mut dialogues);
    }
    collect_flow_item_dialogues(module.top_level_items(), &mut dialogues);
    dialogues
}

fn dialogue_content_contains_offset(dialogue: &HirDialogue, offset: usize) -> bool {
    let range = dialogue.content().range();
    range.start() <= offset && offset <= range.end()
}

pub(crate) fn source_range(source: &RichTextSettingSource) -> Option<RichTextSourceRange> {
    match source {
        RichTextSettingSource::SourceFile { range, .. } => *range,
        RichTextSettingSource::EngineDefault { .. } => None,
    }
}

pub(crate) fn style_path_at(items: &[Item], offset: usize) -> Option<String> {
    items.iter().find_map(|item| match item {
        Item::DialogueDefaults(defaults) => defaults
            .assignments()
            .iter()
            .find(|assignment| range_contains(assignment.range(), offset))
            .map(|assignment| assignment.path().dotted()),
        Item::Flow(flow) => style_path_from_flow_items(flow.body(), offset),
        Item::FlowItem(item) => {
            style_path_from_flow_items(std::slice::from_ref(item.as_ref()), offset)
        }
        _ => None,
    })
}

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
        FlowItem::BorrowBlock(block) => nested_style_path(block, offset),
        FlowItem::SourceLocale(block) => nested_style_path(block, offset),
        FlowItem::Scope(block) => nested_style_path(block, offset),
        FlowItem::AwaitWith(await_with) => style_path_from_await_with(await_with, offset),
        FlowItem::Choice(_) | FlowItem::Include(_) | FlowItem::Raw(_) => None,
    })
}

trait HasFlowBody {
    fn body(&self) -> &[FlowItem];
}

impl HasFlowBody for IfBlock {
    fn body(&self) -> &[FlowItem] {
        self.body()
    }
}

impl HasFlowBody for IfLetBlock {
    fn body(&self) -> &[FlowItem] {
        self.body()
    }
}

impl HasFlowBody for LoopBlock {
    fn body(&self) -> &[FlowItem] {
        self.body()
    }
}

impl HasFlowBody for WhileBlock {
    fn body(&self) -> &[FlowItem] {
        self.body()
    }
}

impl HasFlowBody for WhileLetBlock {
    fn body(&self) -> &[FlowItem] {
        self.body()
    }
}

impl HasFlowBody for ForBlock {
    fn body(&self) -> &[FlowItem] {
        self.body()
    }
}

impl HasFlowBody for BorrowBlock {
    fn body(&self) -> &[FlowItem] {
        self.body()
    }
}

impl HasFlowBody for SourceLocaleBlock {
    fn body(&self) -> &[FlowItem] {
        self.body()
    }
}

impl HasFlowBody for ScopeBlock {
    fn body(&self) -> &[FlowItem] {
        self.body()
    }
}

fn nested_style_path(block: &impl HasFlowBody, offset: usize) -> Option<String> {
    style_path_from_flow_items(block.body(), offset)
}

fn style_path_from_await_with(await_with: &AwaitWith, offset: usize) -> Option<String> {
    await_with
        .branches()
        .iter()
        .find_map(|branch| style_path_from_flow_items(branch.body(), offset))
}

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
        Stmt::LetChoice { .. }
        | Stmt::LetTextSubmit { .. }
        | Stmt::Assign { .. }
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
        | Stmt::Raw(_) => None,
    }
}

fn style_path_from_stmts(statements: &[Stmt], offset: usize) -> Option<String> {
    statements
        .iter()
        .find_map(|stmt| style_path_from_stmt(stmt, offset))
}

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
    if !range_contains(content.range(), offset) {
        return None;
    }
    let raw = content.raw();
    let raw_start = content.range().start();
    inline_tag_ranges(raw).into_iter().find_map(|tag_range| {
        let absolute = TextRange::new(raw_start + tag_range.start, raw_start + tag_range.end);
        if !range_contains(&absolute, offset) {
            return None;
        }
        let inside = &raw[tag_range.start + '['.len_utf8()..tag_range.end - ']'.len_utf8()];
        inline_tag_style_path(inside, raw_start + tag_range.start + '['.len_utf8(), offset)
    })
}

fn inline_tag_ranges(raw: &str) -> Vec<Range<usize>> {
    let mut ranges = Vec::new();
    let mut cursor = 0usize;
    while let Some(open_relative) = raw[cursor..].find('[') {
        let open = cursor + open_relative;
        let Some(close_relative) = raw[open + '['.len_utf8()..].find(']') else {
            break;
        };
        let close = open + '['.len_utf8() + close_relative + ']'.len_utf8();
        ranges.push(open..close);
        cursor = close;
    }
    ranges
}

fn inline_tag_style_path(inside: &str, inside_start: usize, offset: usize) -> Option<String> {
    let leading = inside.len() - inside.trim_start().len();
    let trimmed = inside.trim();
    if trimmed.is_empty() || trimmed.starts_with('/') || trimmed.starts_with('!') {
        return None;
    }
    let trimmed_start = inside_start + leading;
    if trimmed.starts_with('.') {
        let (selector, attrs) = split_inline_name_attrs(trimmed);
        let attrs_start = inline_attrs_start(trimmed, selector, trimmed_start);
        return inferred_inline_style_path(
            selector.trim_start_matches('.'),
            attrs,
            trimmed_start,
            attrs_start,
            offset,
        );
    }
    let (name, attrs) = split_inline_name_attrs(trimmed);
    let attrs_start = inline_attrs_start(trimmed, name, trimmed_start);
    match name {
        "style" => selected_inline_style_path(attrs, attrs_start, style_selector_path, offset),
        "layout" => selected_inline_style_path(attrs, attrs_start, layout_selector_path, offset),
        "transform" => {
            selected_inline_style_path(attrs, attrs_start, transform_selector_path, offset)
        }
        "effect" | "fx" => {
            selected_inline_style_path(attrs, attrs_start, effect_selector_path, offset)
        }
        "color" | "font" | "size" | "em" | "strong" | "i" | "italic" | "oblique" | "slant" => {
            direct_inline_style_path(name, attrs, trimmed_start, attrs_start, offset)
        }
        _ => None,
    }
}

fn split_inline_name_attrs(source: &str) -> (&str, &str) {
    let mut parts = source.splitn(2, char::is_whitespace);
    let name = parts.next().unwrap_or_default();
    let attrs = parts.next().unwrap_or_default().trim();
    (name, attrs)
}

fn inline_attrs_start(trimmed: &str, name: &str, trimmed_start: usize) -> usize {
    trimmed_start + name.len() + trimmed[name.len()..].len()
        - trimmed[name.len()..].trim_start().len()
}

fn selected_inline_style_path(
    attrs: &str,
    attrs_start: usize,
    path_for: fn(&str, &str, &str) -> Option<String>,
    offset: usize,
) -> Option<String> {
    let (selector, selector_attrs) = split_selector_attrs_for_inline(attrs);
    let selector_offset = attrs.find(selector).unwrap_or(0);
    let selector_start = attrs_start + selector_offset;
    let selector_attrs_start =
        inline_attrs_start(&attrs[selector_offset..], selector, selector_start);
    selector_or_attr_path(
        selector.trim_start_matches('.'),
        selector_attrs,
        selector_start,
        selector_attrs_start,
        path_for,
        offset,
    )
}

fn inferred_inline_style_path(
    selector: &str,
    attrs: &str,
    selector_start: usize,
    attrs_start: usize,
    offset: usize,
) -> Option<String> {
    let path_for = match selector {
        "italic" | "oblique" => style_selector_path,
        "horizontal_tb"
        | "vertical_rl"
        | "vertical_lr"
        | "dir"
        | "ruby_over"
        | "ruby_under"
        | "ruby_inter_character" => layout_selector_path,
        "offset" | "pos" | "rotate" | "scale" | "skew" => transform_selector_path,
        "wave" | "shake" | "arc" | "typewriter" | "jitter" | "shader" | "host" => {
            effect_selector_path
        }
        _ => return None,
    };
    selector_or_attr_path(
        selector,
        attrs,
        selector_start,
        attrs_start,
        path_for,
        offset,
    )
}

fn selector_or_attr_path(
    selector: &str,
    attrs: &str,
    selector_start: usize,
    attrs_start: usize,
    path_for: fn(&str, &str, &str) -> Option<String>,
    offset: usize,
) -> Option<String> {
    if selector_start <= offset && offset <= selector_start + selector.len() {
        return path_for(selector, "", "");
    }
    inline_attr_ranges(attrs, attrs_start)
        .into_iter()
        .find_map(|attr| {
            (attr.value_range.start <= offset && offset <= attr.value_range.end)
                .then(|| path_for(selector, &attr.name, &attr.value))
                .flatten()
        })
}

fn direct_inline_style_path(
    name: &str,
    attrs: &str,
    name_start: usize,
    attrs_start: usize,
    offset: usize,
) -> Option<String> {
    let value_range = if attrs.is_empty() {
        name_start..name_start + name.len()
    } else {
        attrs_start..attrs_start + attrs.len()
    };
    if offset < value_range.start || offset > value_range.end {
        return None;
    }
    match name {
        "color" => Some("rich_text.text.color".to_owned()),
        "font" => Some("rich_text.text.font".to_owned()),
        "size" => Some("rich_text.text.size".to_owned()),
        "em" | "strong" | "i" | "italic" | "oblique" | "slant" => {
            Some("rich_text.text.style".to_owned())
        }
        _ => None,
    }
}

fn split_selector_attrs_for_inline(attrs: &str) -> (&str, &str) {
    let attrs = attrs.trim();
    let mut parts = attrs.splitn(2, char::is_whitespace);
    let first = parts.next().unwrap_or_default();
    if first.starts_with('.') {
        (first, parts.next().unwrap_or_default().trim())
    } else {
        ("", attrs)
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

#[derive(Clone, Debug, Eq, PartialEq)]
struct InlineAttrRange {
    name: String,
    value: String,
    value_range: Range<usize>,
}

fn inline_attr_ranges(attrs: &str, attrs_start: usize) -> Vec<InlineAttrRange> {
    let mut ranges = Vec::new();
    let mut cursor = 0usize;
    for part in attrs.split_whitespace() {
        let part_start = attrs[cursor..]
            .find(part)
            .map_or(cursor, |relative| cursor + relative);
        cursor = part_start + part.len();
        let Some((name, value)) = part.split_once('=') else {
            continue;
        };
        let value_start = attrs_start + part_start + name.len() + '='.len_utf8();
        ranges.push(InlineAttrRange {
            name: name.to_owned(),
            value: value.to_owned(),
            value_range: value_start..value_start + value.len(),
        });
    }
    ranges
}

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
            HirFlowItem::Borrow(block) => collect_flow_item_dialogues(block.body(), dialogues),
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
