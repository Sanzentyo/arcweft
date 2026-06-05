//! Rich-text display lowering for runtime-plan sidecars.

use crate::labels::expr_label;
use arcweft_core::plan::RuntimeLineId;
use arcweft_lang_hir::model::{HirDialogue, HirModule, HirTopLevelDecl};
use arcweft_lang_hir::syntax::ast::dialogue::{DialogueTag, DialogueToken, LineArg};
use arcweft_lang_hir::syntax::ast::items::{EntityDeclItem, EntityDeclKind};
use arcweft_lang_hir::syntax::expr::{CallArg, Expr, Literal, parse_expr};
use arcweft_render_text::{
    DialogueHostEvent, FallbackStylePolicy, InlineFailurePolicy, InlineFallback, LineDisplayArg,
    LineDisplaySpec, RichTextColor, RichTextControl, RichTextDocument, RichTextFontFamily,
    RichTextNode, RichTextStyle,
};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct DialogueDisplayDefaults {
    global: DialogueStyleDefaults,
    characters: BTreeMap<String, DialogueStyleDefaults>,
}

#[derive(Clone, Debug, Default, PartialEq)]
struct DialogueStyleDefaults {
    base_styles: Vec<RichTextStyle>,
    default_inline_failure_policy: Option<InlineFailurePolicy>,
}

impl DialogueDisplayDefaults {
    pub(crate) fn from_module(module: &HirModule) -> Self {
        let mut defaults = Self::default();
        for declaration in module.declarations() {
            match declaration {
                HirTopLevelDecl::DialogueDefaults(item) => {
                    defaults.global = style_defaults_from_options(
                        item.options()
                            .iter()
                            .map(|option| (option.name(), option.value())),
                    );
                }
                HirTopLevelDecl::EntityDecl(item) if item.kind() == EntityDeclKind::Character => {
                    let style = character_style_defaults(item);
                    if !style.is_empty() {
                        for key in character_style_keys(item) {
                            defaults.characters.insert(key, style.clone());
                        }
                    }
                }
                _ => {}
            }
        }
        defaults
    }

    fn character_for_callee(&self, callee: &str) -> Option<&DialogueStyleDefaults> {
        self.characters.get(callee).or_else(|| {
            callee
                .split_once('.')
                .and_then(|(speaker, _)| self.characters.get(speaker))
        })
    }
}

pub(crate) fn lower_dialogue_display(
    line: RuntimeLineId,
    dialogue: &HirDialogue,
    defaults: &DialogueDisplayDefaults,
) -> LineDisplaySpec {
    let default_inline_failure_policy = lower_effective_inline_failure_policy(dialogue, defaults);
    LineDisplaySpec {
        line,
        callee: dialogue.callee().to_owned(),
        text_key: dialogue.text_key().map(|id| id.body().to_owned()),
        window: dialogue.window().map(|id| id.body().to_owned()),
        voice: dialogue.voice().map(expr_label),
        look: dialogue.look().map(expr_label),
        style: dialogue.style().map(expr_label),
        base_styles: lower_effective_dialogue_base_styles(dialogue, defaults),
        default_inline_failure_policy: default_inline_failure_policy.clone(),
        args: dialogue
            .args()
            .iter()
            .map(|arg| LineDisplayArg {
                name: arg.name().to_owned(),
                value: expr_label(arg.value()),
            })
            .collect(),
        content: RichTextDocument::new(
            dialogue
                .content()
                .tokens()
                .iter()
                .flat_map(|token| {
                    lower_dialogue_token(token, default_inline_failure_policy.as_ref())
                })
                .collect(),
        ),
    }
}

fn lower_effective_dialogue_base_styles(
    dialogue: &HirDialogue,
    defaults: &DialogueDisplayDefaults,
) -> Vec<RichTextStyle> {
    let mut styles = defaults.global.base_styles.clone();
    if let Some(character) = defaults.character_for_callee(dialogue.callee()) {
        styles.extend(character.base_styles.clone());
    }
    styles.extend(
        dialogue
            .style()
            .into_iter()
            .flat_map(display_styles_from_expr),
    );
    styles.extend(
        dialogue
            .args()
            .iter()
            .flat_map(|arg| display_styles_from_named_expr(arg.name(), arg.value())),
    );
    styles
}

fn lower_effective_inline_failure_policy(
    dialogue: &HirDialogue,
    defaults: &DialogueDisplayDefaults,
) -> Option<InlineFailurePolicy> {
    lower_default_inline_failure_policy(dialogue.args())
        .or_else(|| {
            defaults
                .character_for_callee(dialogue.callee())
                .and_then(|character| character.default_inline_failure_policy.clone())
        })
        .or_else(|| defaults.global.default_inline_failure_policy.clone())
}

impl DialogueStyleDefaults {
    fn is_empty(&self) -> bool {
        self.base_styles.is_empty() && self.default_inline_failure_policy.is_none()
    }
}

fn character_style_defaults(item: &EntityDeclItem) -> DialogueStyleDefaults {
    item.body()
        .and_then(dialogue_style_block)
        .map(style_defaults_from_body)
        .unwrap_or_default()
}

fn character_style_keys(item: &EntityDeclItem) -> Vec<String> {
    [
        item.surface_alias().map(str::to_owned),
        item.name().map(str::to_owned),
        Some(item.id().body().to_owned()),
        item.id()
            .body()
            .rsplit_once('.')
            .map(|(_, suffix)| suffix.to_owned()),
    ]
    .into_iter()
    .flatten()
    .collect()
}

fn style_defaults_from_body(body: &str) -> DialogueStyleDefaults {
    let mut defaults = DialogueStyleDefaults::default();
    for (name, value) in style_block_assignments(body) {
        if let Ok(expr) = parse_expr(value) {
            if let Some(policy) = inline_default_from_named_expr(name, &expr) {
                defaults.default_inline_failure_policy = Some(policy);
            }
            defaults
                .base_styles
                .extend(display_styles_from_named_expr(name, &expr));
        }
    }
    defaults
}

fn style_defaults_from_options<'a>(
    options: impl IntoIterator<Item = (&'a str, &'a Expr)>,
) -> DialogueStyleDefaults {
    let mut defaults = DialogueStyleDefaults::default();
    for (name, value) in options {
        if let Some(policy) = inline_default_from_named_expr(name, value) {
            defaults.default_inline_failure_policy = Some(policy);
        }
        defaults
            .base_styles
            .extend(display_styles_from_named_expr(name, value));
    }
    defaults
}

fn display_styles_from_expr(expr: &Expr) -> Vec<RichTextStyle> {
    let Expr::Call { callee, args } = expr else {
        return Vec::new();
    };
    let Some(name) = style_call_name(callee) else {
        return Vec::new();
    };
    match name {
        "font" => first_positional_value(args)
            .map(|attrs| RichTextStyle::Font {
                family: RichTextFontFamily::from_attrs(&attrs),
            })
            .into_iter()
            .collect(),
        "color" | "rgb" => first_positional_expr(args)
            .map(color_from_expr)
            .or_else(|| first_positional_value(args).map(|attrs| RichTextColor::from_attrs(&attrs)))
            .map(|value| RichTextStyle::Color { value })
            .into_iter()
            .collect(),
        "size" => first_positional_value(args)
            .map(|attrs| RichTextStyle::from_tag("size", &attrs))
            .into_iter()
            .collect(),
        "text_style" | "dialogue_style" | "style" => args
            .iter()
            .flat_map(display_styles_from_style_arg)
            .collect(),
        _ => Vec::new(),
    }
}

fn display_styles_from_style_arg(arg: &CallArg) -> Vec<RichTextStyle> {
    match arg {
        CallArg::Positional(expr) => display_styles_from_expr(expr),
        CallArg::Named { name, value } => display_styles_from_named_expr(name, value),
        CallArg::Spread { .. } => Vec::new(),
    }
}

fn display_styles_from_named_expr(name: &str, value: &Expr) -> Vec<RichTextStyle> {
    let attrs = expr_style_value(value);
    match name {
        "style" | "text_style" | "dialogue_style" => display_styles_from_expr(value),
        "font" | "font_family" | "text_font" => vec![RichTextStyle::Font {
            family: RichTextFontFamily::from_attrs(&attrs),
        }],
        "color" | "text_color" | "read_text_color" | "unread_text_color" => {
            vec![RichTextStyle::Color {
                value: color_from_expr(value),
            }]
        }
        "size" | "text_size" => vec![RichTextStyle::from_tag("size", &attrs)],
        _ => Vec::new(),
    }
}

fn style_call_name(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::Path(path) => Some(path.as_str()),
        Expr::Field { field, .. } => Some(field.as_str()),
        _ => None,
    }
}

fn first_positional_expr(args: &[CallArg]) -> Option<&Expr> {
    args.iter().find_map(|arg| match arg {
        CallArg::Positional(expr) => Some(expr),
        CallArg::Named { name, value } if name == "family" || name == "value" => Some(value),
        CallArg::Named { .. } | CallArg::Spread { .. } => None,
    })
}

fn first_positional_value(args: &[CallArg]) -> Option<String> {
    args.iter().find_map(|arg| match arg {
        CallArg::Positional(expr) => Some(expr_style_value(expr)),
        CallArg::Named { name, value } if name == "family" || name == "value" => {
            Some(expr_style_value(value))
        }
        CallArg::Named { .. } | CallArg::Spread { .. } => None,
    })
}

fn color_from_expr(expr: &Expr) -> RichTextColor {
    match expr {
        Expr::Call { callee, args } if matches!(style_call_name(callee), Some("rgb" | "color")) => {
            first_positional_expr(args)
                .map(expr_style_value)
                .map_or_else(
                    || RichTextColor::from_attrs(&expr_label(expr)),
                    |attrs| RichTextColor::from_attrs(&attrs),
                )
        }
        _ => RichTextColor::from_attrs(&expr_style_value(expr)),
    }
}

fn expr_style_value(expr: &Expr) -> String {
    match expr {
        Expr::Literal(Literal::String(value)) | Expr::Path(value) => value.clone(),
        Expr::Literal(literal) => crate::labels::literal_label(literal),
        Expr::EntityRef(entity) => format!("@{}", entity.body()),
        _ => expr_label(expr),
    }
}

fn lower_default_inline_failure_policy(args: &[LineArg]) -> Option<InlineFailurePolicy> {
    args.iter().find_map(|arg| match arg.name() {
        "inline_fallback" => Some(inline_fallback_from_expr(arg.value())),
        "inline_error" | "inline_error_policy" => {
            Some(inline_failure_policy_from_expr(arg.value()))
        }
        _ => None,
    })
}

fn inline_default_from_named_expr(name: &str, value: &Expr) -> Option<InlineFailurePolicy> {
    match name {
        "inline_fallback" => Some(inline_fallback_from_expr(value)),
        "inline_error" | "inline_error_policy" => Some(inline_failure_policy_from_expr(value)),
        _ => None,
    }
}

fn dialogue_style_block(body: &str) -> Option<&str> {
    let start = body.find("dialogue_style")?;
    let open = body[start..].find('{')? + start;
    let close = matching_brace(body, open)?;
    Some(body[open + 1..close].trim())
}

fn matching_brace(source: &str, open: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    for (offset, ch) in source[open..].char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match ch {
            '\\' if in_string => escaped = true,
            '"' => in_string = !in_string,
            '{' if !in_string => depth += 1,
            '}' if !in_string => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(open + offset);
                }
            }
            _ => {}
        }
    }
    None
}

fn style_block_assignments(body: &str) -> Vec<(&str, &str)> {
    logical_style_items(body)
        .into_iter()
        .filter_map(split_assignment)
        .collect()
}

fn logical_style_items(body: &str) -> Vec<&str> {
    let mut items = Vec::new();
    let mut start = 0usize;
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escaped = false;
    for (offset, ch) in body.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match ch {
            '\\' if in_string => escaped = true,
            '"' => in_string = !in_string,
            '(' | '[' | '{' if !in_string => depth += 1,
            ')' | ']' | '}' if !in_string => depth = depth.saturating_sub(1),
            '\n' if !in_string && depth == 0 => {
                let item = body[start..offset].trim();
                if !item.is_empty() && !item.starts_with("//") {
                    items.push(item);
                }
                start = offset + '\n'.len_utf8();
            }
            _ => {}
        }
    }
    let tail = body[start..].trim();
    if !tail.is_empty() && !tail.starts_with("//") {
        items.push(tail);
    }
    items
}

fn split_assignment(source: &str) -> Option<(&str, &str)> {
    let (name, value) = source.split_once('=')?;
    let name = name.trim();
    let value = value.trim().trim_end_matches(',').trim();
    (!name.is_empty() && !value.is_empty()).then_some((name, value))
}

fn lower_dialogue_token(
    token: &DialogueToken,
    default_inline_failure_policy: Option<&InlineFailurePolicy>,
) -> Vec<RichTextNode> {
    match token {
        DialogueToken::Text(text) => vec![RichTextNode::Text { text: text.clone() }],
        DialogueToken::Raw(text) => {
            vec![RichTextNode::Control(RichTextControl::Raw {
                text: text.clone(),
            })]
        }
        DialogueToken::Tag(tag) => lower_tag(tag),
        DialogueToken::Mark(mark) => {
            vec![RichTextNode::Control(RichTextControl::Mark {
                name: mark.name().to_owned(),
            })]
        }
        DialogueToken::EndTag(name) => {
            vec![RichTextNode::StyleEnd { name: name.clone() }]
        }
        DialogueToken::Expr(expr) => {
            vec![RichTextNode::Interpolation {
                expr: expr_label(expr),
                fallback_source: inline_fallback_source_label(expr),
                on_error: inline_failure_policy(expr, default_inline_failure_policy),
            }]
        }
        DialogueToken::Ruby { base, ruby } => {
            vec![RichTextNode::Ruby {
                base: base.clone(),
                ruby: ruby.clone(),
            }]
        }
        DialogueToken::Escape(ch) => vec![RichTextNode::Text {
            text: ch.to_string(),
        }],
    }
}

fn lower_tag(tag: &DialogueTag) -> Vec<RichTextNode> {
    match tag.name() {
        "p" | "page" => vec![RichTextNode::Control(RichTextControl::Page)],
        "l" | "wait" => vec![RichTextNode::Control(RichTextControl::LineWait)],
        "r" | "br" | "nl" => vec![RichTextNode::Control(RichTextControl::HardBreak)],
        "w" => vec![RichTextNode::Control(RichTextControl::TimedWait {
            value: tag.attrs().to_owned(),
        })],
        "clear" | "er" | "cm" => vec![RichTextNode::Control(RichTextControl::Clear)],
        "reset" => vec![RichTextNode::Control(RichTextControl::Reset)],
        "em" | "strong" | "color" | "font" | "size" | "speed" => {
            vec![RichTextNode::StyleStart {
                style: RichTextStyle::from_tag(tag.name(), tag.attrs()),
            }]
        }
        "voice" => host_event(DialogueHostEvent::Voice {
            attrs: tag.attrs().to_owned(),
        }),
        "face" => host_event(DialogueHostEvent::Face {
            attrs: tag.attrs().to_owned(),
        }),
        "pose" => host_event(DialogueHostEvent::Pose {
            attrs: tag.attrs().to_owned(),
        }),
        "show" => host_event(DialogueHostEvent::Show {
            attrs: tag.attrs().to_owned(),
        }),
        "hide" => host_event(DialogueHostEvent::Hide {
            attrs: tag.attrs().to_owned(),
        }),
        "move" => host_event(DialogueHostEvent::Move {
            attrs: tag.attrs().to_owned(),
        }),
        "scale" => host_event(DialogueHostEvent::Scale {
            attrs: tag.attrs().to_owned(),
        }),
        "rotate" => host_event(DialogueHostEvent::Rotate {
            attrs: tag.attrs().to_owned(),
        }),
        "anim" => host_event(DialogueHostEvent::Anim {
            attrs: tag.attrs().to_owned(),
        }),
        "shake" => host_event(DialogueHostEvent::Shake {
            attrs: tag.attrs().to_owned(),
        }),
        "call" => host_event(DialogueHostEvent::Call {
            attrs: tag.attrs().to_owned(),
        }),
        "signal" => host_event(DialogueHostEvent::Signal {
            attrs: tag.attrs().to_owned(),
        }),
        "if" | "else" | "endif" => host_event(DialogueHostEvent::Conditional {
            name: tag.name().to_owned(),
            attrs: tag.attrs().to_owned(),
        }),
        name => vec![RichTextNode::Control(RichTextControl::Unknown {
            name: name.to_owned(),
            attrs: tag.attrs().to_owned(),
        })],
    }
}

fn host_event(event: DialogueHostEvent) -> Vec<RichTextNode> {
    vec![RichTextNode::HostEvent(event)]
}

fn inline_failure_policy(
    expr: &Expr,
    default: Option<&InlineFailurePolicy>,
) -> InlineFailurePolicy {
    match expr {
        Expr::Call { args, .. } | Expr::MethodCall { args, .. } => {
            inline_failure_policy_from_args(args)
                .or_else(|| default.cloned())
                .unwrap_or(InlineFailurePolicy::FailLine)
        }
        _ => default.cloned().unwrap_or(InlineFailurePolicy::FailLine),
    }
}

fn inline_fallback_source_label(expr: &Expr) -> String {
    match expr {
        Expr::Call { args, .. } | Expr::MethodCall { args, .. } => args
            .iter()
            .find_map(|arg| match arg {
                CallArg::Positional(value) => Some(expr_label(value)),
                CallArg::Named { name, value } if name == "value" || name == "input" => {
                    Some(expr_label(value))
                }
                CallArg::Named { .. } | CallArg::Spread { .. } => None,
            })
            .unwrap_or_else(|| expr_label(expr)),
        _ => expr_label(expr),
    }
}

fn inline_failure_policy_from_args(args: &[CallArg]) -> Option<InlineFailurePolicy> {
    args.iter().find_map(|arg| match arg {
        CallArg::Named { name, value } if name == "fallback" || name == "none" => {
            Some(inline_fallback_from_expr(value))
        }
        CallArg::Named { name, value } if name == "discard_error" => {
            is_truthy_policy_value(value).then_some(InlineFailurePolicy::Discard)
        }
        CallArg::Named { name, value } if name == "on_error" => {
            Some(inline_failure_policy_from_expr(value))
        }
        CallArg::Positional(_) | CallArg::Named { .. } | CallArg::Spread { .. } => None,
    })
}

fn is_truthy_policy_value(expr: &Expr) -> bool {
    matches!(expr, Expr::Literal(Literal::Bool(true)))
        || matches!(expr, Expr::Path(path) if path == "true")
}

fn inline_failure_policy_from_expr(expr: &Expr) -> InlineFailurePolicy {
    match enum_variant_name(expr) {
        Some((_, "fail" | "line_error")) => InlineFailurePolicy::FailLine,
        Some((_, "discard")) => InlineFailurePolicy::Discard,
        _ => inline_failure_constructor(expr).unwrap_or(InlineFailurePolicy::FailLine),
    }
}

fn inline_failure_constructor(expr: &Expr) -> Option<InlineFailurePolicy> {
    let args = match expr {
        Expr::Call { callee, args } if constructor_name(callee)? == "fallback" => args,
        Expr::MethodCall {
            receiver,
            method,
            args,
        } if matches!(receiver.as_ref(), Expr::Path(namespace) if namespace == "InlineFailure")
            && method == "fallback" =>
        {
            args
        }
        _ => return None,
    };
    let fallback = args
        .iter()
        .find_map(|arg| match arg {
            CallArg::Positional(value) => Some(inline_fallback_value(value)),
            CallArg::Named { name, value } if name == "value" || name == "text" => {
                Some(inline_fallback_value(value))
            }
            CallArg::Named { .. } | CallArg::Spread { .. } => None,
        })
        .unwrap_or_else(|| InlineFallback::Text {
            text: String::new(),
            style: FallbackStylePolicy::Plain,
        });
    Some(InlineFailurePolicy::Fallback { fallback })
}

fn inline_fallback_from_expr(expr: &Expr) -> InlineFailurePolicy {
    InlineFailurePolicy::Fallback {
        fallback: inline_fallback_value(expr),
    }
}

fn inline_fallback_value(expr: &Expr) -> InlineFallback {
    match enum_variant_name(expr) {
        Some((_, "expr_source")) => InlineFallback::ExprSource {
            style: FallbackStylePolicy::Plain,
        },
        Some((_, "call_source")) => InlineFallback::CallSource {
            style: FallbackStylePolicy::Plain,
        },
        Some((_, "value_plain")) => InlineFallback::ValuePlain,
        _ => InlineFallback::Text {
            text: expr_style_value(expr),
            style: FallbackStylePolicy::Plain,
        },
    }
}

fn constructor_name(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::Path(name) if name == "fallback" => Some("fallback"),
        Expr::Field { target, field } if matches!(target.as_ref(), Expr::Path(namespace) if namespace == "InlineFailure") => {
            Some(field)
        }
        _ => None,
    }
}

fn enum_variant_name(expr: &Expr) -> Option<(&str, &str)> {
    match expr {
        Expr::Path(value) => value.strip_prefix('.').map(|variant| ("", variant)),
        Expr::MethodCall {
            receiver,
            method,
            args,
        } if args.is_empty() => match receiver.as_ref() {
            Expr::Path(namespace) => Some((namespace.as_str(), method.as_str())),
            _ => None,
        },
        Expr::Field { target, field } => match target.as_ref() {
            Expr::Path(namespace) => Some((namespace.as_str(), field.as_str())),
            _ => None,
        },
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_lang_hir::lower::lower_to_hir;
    use arcweft_lang_syntax::parser::parse_source;
    use arcweft_render_text::{RichTextColor, RichTextFontFamily};

    #[test]
    fn lowers_full_tag_families_to_render_text_nodes() {
        let parsed = parse_source(
            r##"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice(style=text_style(font=serif, color="#f7e8ff"), inline_error=InlineFailure.fallback("?")): Hello #[player] |[夢](ゆめ)[r][font monospace][em:quiet][voice auto][face smile][signal .seen][p]
}
"##,
        );
        let hir = lower_to_hir(parsed.typed_tree()).expect("fixture lowers");
        let dialogue = hir
            .flows()
            .first()
            .and_then(|flow| flow.body().first())
            .and_then(|item| match item {
                arcweft_lang_hir::model::HirFlowItem::Dialogue(dialogue) => Some(dialogue),
                _ => None,
            })
            .expect("dialogue item");

        let defaults = DialogueDisplayDefaults::from_module(&hir);
        let spec = lower_dialogue_display(
            RuntimeLineId("say.opening.001".to_owned()),
            dialogue,
            &defaults,
        );

        assert_eq!(
            spec.base_styles,
            vec![
                RichTextStyle::Font {
                    family: RichTextFontFamily::Serif
                },
                RichTextStyle::Color {
                    value: RichTextColor::Rgb {
                        red: 247,
                        green: 232,
                        blue: 255
                    }
                }
            ]
        );
        assert_eq!(
            spec.default_inline_failure_policy,
            Some(InlineFailurePolicy::Fallback {
                fallback: InlineFallback::Text {
                    text: "?".to_owned(),
                    style: FallbackStylePolicy::Plain
                }
            })
        );
        assert!(spec.content.nodes.iter().any(|node| {
            matches!(
                node,
                RichTextNode::Interpolation {
                    expr,
                    fallback_source,
                    on_error: InlineFailurePolicy::Fallback {
                        fallback: InlineFallback::Text { text, .. }
                    },
                } if expr == "player"
                    && fallback_source == "player"
                    && text == "?"
            )
        }));
        assert!(spec.content.nodes.iter().any(|node| {
            matches!(
                node,
                RichTextNode::Ruby { base, ruby } if base == "夢" && ruby == "ゆめ"
            )
        }));
        assert!(spec.content.nodes.iter().any(|node| {
            matches!(
                node,
                RichTextNode::HostEvent(DialogueHostEvent::Voice { .. })
            )
        }));
        assert!(spec.content.nodes.iter().any(|node| {
            matches!(
                node,
                RichTextNode::HostEvent(DialogueHostEvent::Signal { .. })
            )
        }));
        assert!(spec.content.nodes.iter().any(|node| {
            matches!(
                node,
                RichTextNode::StyleStart {
                    style: RichTextStyle::Font {
                        family: RichTextFontFamily::Monospace
                    }
                }
            )
        }));
    }

    #[test]
    fn dialogue_display_inherits_global_and_character_style_defaults() {
        let parsed = parse_source(
            r##"
pub dialogue defaults @dialogue.defaults {
    font = serif
    text_color = rgb("#101112")
    inline_error = InlineFailure.fallback("global")
}

character @character.alice Alice as alice {
    dialogue_style {
        text_color = rgb("#202122")
        inline_error = InlineFailure.discard
    }
}

flow @flow.main main {
    alice(color=rgb("#303132")): Hello #[missing][p]
}
"##,
        );
        let hir = lower_to_hir(parsed.typed_tree()).expect("fixture lowers");
        let defaults = DialogueDisplayDefaults::from_module(&hir);
        let dialogue = hir
            .flows()
            .first()
            .and_then(|flow| flow.body().first())
            .and_then(|item| match item {
                arcweft_lang_hir::model::HirFlowItem::Dialogue(dialogue) => Some(dialogue),
                _ => None,
            })
            .expect("dialogue item");

        let spec = lower_dialogue_display(
            RuntimeLineId("say.opening.002".to_owned()),
            dialogue,
            &defaults,
        );

        assert_eq!(
            spec.base_styles,
            vec![
                RichTextStyle::Font {
                    family: RichTextFontFamily::Serif
                },
                RichTextStyle::Color {
                    value: RichTextColor::Rgb {
                        red: 16,
                        green: 17,
                        blue: 18
                    }
                },
                RichTextStyle::Color {
                    value: RichTextColor::Rgb {
                        red: 32,
                        green: 33,
                        blue: 34
                    }
                },
                RichTextStyle::Color {
                    value: RichTextColor::Rgb {
                        red: 48,
                        green: 49,
                        blue: 50
                    }
                }
            ]
        );
        assert_eq!(
            spec.default_inline_failure_policy,
            Some(InlineFailurePolicy::Discard)
        );
        assert!(spec.content.nodes.iter().any(|node| {
            matches!(
                node,
                RichTextNode::Interpolation {
                    expr,
                    on_error: InlineFailurePolicy::Discard,
                    ..
                } if expr == "missing"
            )
        }));
    }
}
