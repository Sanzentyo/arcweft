use arcweft_lang_hir::syntax::expr::{CallArg, Expr};
use arcweft_render_text::{
    RichTextColor, RichTextFontFamily, RichTextInlineDirection, RichTextJlreqStrictness,
    RichTextLayout, RichTextRubyPosition, RichTextStyle, RichTextVerticalLatinMode,
    RichTextWritingMode, parse_milli_token,
};

use crate::labels::expr_label;

use super::helpers::{expr_style_value, style_call_name};

pub(crate) fn display_styles_from_expr(expr: &Expr) -> Vec<RichTextStyle> {
    let Expr::Call(call) = expr else {
        return Vec::new();
    };
    let Some(name) = style_call_name(call.callee()) else {
        return Vec::new();
    };
    let args = call.args();
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
        "text_style" | "dialogue_style" | "style" | "rich_text_style" => args
            .iter()
            .flat_map(display_styles_from_style_arg)
            .collect(),
        "ruby_style" => ruby_layout_from_args(args)
            .map(|layout| RichTextStyle::Layout { layout })
            .into_iter()
            .collect(),
        "layout_style" => text_layout_from_args(args)
            .map(|layout| RichTextStyle::Layout { layout })
            .into_iter()
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

pub(crate) fn display_styles_from_named_expr(name: &str, value: &Expr) -> Vec<RichTextStyle> {
    if let Some(field) = name.strip_prefix("rich_text.text.") {
        return display_styles_from_named_expr(field, value);
    }
    if let Some(field) = name.strip_prefix("rich_text.ruby.") {
        return ruby_layout_from_field(field, value)
            .map(|layout| RichTextStyle::Layout { layout })
            .into_iter()
            .collect();
    }
    if let Some(field) = name.strip_prefix("rich_text.layout.") {
        return text_layout_from_field(field, value)
            .map(|layout| RichTextStyle::Layout { layout })
            .into_iter()
            .collect();
    }
    let attrs = expr_style_value(value);
    match name {
        "style" | "text_style" | "dialogue_style" | "rich_text" | "text" | "layout" | "ruby" => {
            display_styles_from_expr(value)
        }
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

fn ruby_layout_from_args(args: &[CallArg]) -> Option<RichTextLayout> {
    let mut layout = RichTextLayout::default();
    let mut changed = false;
    for arg in args {
        if let CallArg::Named { name, value } = arg
            && apply_ruby_layout_field(&mut layout, name, value)
        {
            changed = true;
        }
    }
    changed.then_some(layout)
}

fn ruby_layout_from_field(field: &str, value: &Expr) -> Option<RichTextLayout> {
    let mut layout = RichTextLayout::default();
    apply_ruby_layout_field(&mut layout, field, value).then_some(layout)
}

fn apply_ruby_layout_field(layout: &mut RichTextLayout, field: &str, value: &Expr) -> bool {
    match field {
        "position" => {
            layout.ruby_position = ruby_position_from_value(&expr_style_value(value));
            true
        }
        "size" | "font_size" => {
            layout.ruby_font_size = Some(parse_milli_token(&expr_style_value(value)));
            true
        }
        "gap" => {
            layout.ruby_gap = Some(parse_milli_token(&expr_style_value(value)));
            true
        }
        "overhang" => {
            layout.ruby_overhang = Some(parse_milli_token(&expr_style_value(value)));
            true
        }
        "collision_gap" => {
            layout.ruby_collision_gap = Some(parse_milli_token(&expr_style_value(value)));
            true
        }
        _ => false,
    }
}

fn text_layout_from_args(args: &[CallArg]) -> Option<RichTextLayout> {
    let mut layout = RichTextLayout::default();
    let mut changed = false;
    for arg in args {
        if let CallArg::Named { name, value } = arg
            && apply_text_layout_field(&mut layout, name, value)
        {
            changed = true;
        }
    }
    changed.then_some(layout)
}

fn text_layout_from_field(field: &str, value: &Expr) -> Option<RichTextLayout> {
    let mut layout = RichTextLayout::default();
    apply_text_layout_field(&mut layout, field, value).then_some(layout)
}

fn apply_text_layout_field(layout: &mut RichTextLayout, field: &str, value: &Expr) -> bool {
    match field {
        "writing_mode" => {
            layout.writing_mode = writing_mode_from_value(&expr_style_value(value));
            true
        }
        "direction" | "dir" => {
            layout.direction = direction_from_value(&expr_style_value(value));
            true
        }
        "vertical_latin" | "latin" => {
            layout.vertical_latin = vertical_latin_from_value(&expr_style_value(value));
            true
        }
        "jlreq" | "jlreq_strictness" => {
            layout.jlreq_strictness = jlreq_from_value(&expr_style_value(value));
            true
        }
        "column_gap" => {
            layout.column_gap = parse_milli_token(&expr_style_value(value));
            true
        }
        _ => false,
    }
}

fn ruby_position_from_value(value: &str) -> RichTextRubyPosition {
    match value {
        "over" => RichTextRubyPosition::Over,
        "under" => RichTextRubyPosition::Under,
        "inter_character" => RichTextRubyPosition::InterCharacter,
        _ => RichTextRubyPosition::Auto,
    }
}

fn writing_mode_from_value(value: &str) -> RichTextWritingMode {
    match value {
        "vertical_rl" | "vertical" | "rl" => RichTextWritingMode::VerticalRl,
        "vertical_lr" | "lr" => RichTextWritingMode::VerticalLr,
        _ => RichTextWritingMode::HorizontalTb,
    }
}

fn direction_from_value(value: &str) -> RichTextInlineDirection {
    match value {
        "ltr" => RichTextInlineDirection::Ltr,
        "rtl" => RichTextInlineDirection::Rtl,
        _ => RichTextInlineDirection::Auto,
    }
}

fn vertical_latin_from_value(value: &str) -> RichTextVerticalLatinMode {
    match value {
        "upright" => RichTextVerticalLatinMode::Upright,
        "sideways" => RichTextVerticalLatinMode::Sideways,
        _ => RichTextVerticalLatinMode::Mixed,
    }
}

fn jlreq_from_value(value: &str) -> RichTextJlreqStrictness {
    match value {
        "loose" => RichTextJlreqStrictness::Loose,
        "normal" => RichTextJlreqStrictness::Normal,
        "strict" => RichTextJlreqStrictness::Strict,
        _ => RichTextJlreqStrictness::Auto,
    }
}

fn first_positional_expr(args: &[CallArg]) -> Option<&Expr> {
    args.iter().find_map(|arg| match arg {
        CallArg::Positional(expr) => Some(expr.as_ref()),
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
        Expr::Call(call) if matches!(style_call_name(call.callee()), Some("rgb" | "color")) => {
            first_positional_expr(call.args())
                .map(expr_style_value)
                .map_or_else(
                    || RichTextColor::from_attrs(&expr_label(expr)),
                    |attrs| RichTextColor::from_attrs(&attrs),
                )
        }
        _ => RichTextColor::from_attrs(&expr_style_value(expr)),
    }
}
