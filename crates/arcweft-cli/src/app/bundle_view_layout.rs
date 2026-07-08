use arcweft_bundle::resource_codec::{
    ViewLogicalRect, ViewRuntimeButtonBounds, view::ViewInputKind,
};
use arcweft_lang_syntax::{
    ast::view::{ViewArg, ViewButton, ViewModifier, ViewStyleModifier},
    expr::{Expr, Literal, UnitNumberSuffix},
};

use super::bundle_view::{
    inline_style_properties, normalize_property_name, style_layout_length_u32,
};

pub(in crate::app) const VIEW_LAYOUT_GAP_MILLI: i32 = 16_000;
pub(in crate::app) const VIEW_LAYOUT_TEXT_CONTROL_WIDTH_MILLI: u32 = 420_000;
pub(in crate::app) const VIEW_LAYOUT_SCROLL_VIEWPORT_HEIGHT_MILLI: u32 = 180_000;

const VIEW_LAYOUT_ROOT_X_MILLI: i32 = 48_000;
const VIEW_LAYOUT_ROOT_Y_MILLI: i32 = 48_000;
const VIEW_LAYOUT_TEXT_LINE_HEIGHT_MILLI: u32 = 24_000;
const VIEW_LAYOUT_BUTTON_WIDTH_MILLI: u32 = 180_000;
const VIEW_LAYOUT_BUTTON_HEIGHT_MILLI: u32 = 44_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::app) struct ViewLayoutCursor {
    pub(in crate::app) x_milli: i32,
    pub(in crate::app) y_milli: i32,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(in crate::app) struct ViewLayoutFrame {
    pub(in crate::app) width_milli: u32,
    pub(in crate::app) height_milli: u32,
}

impl ViewLayoutCursor {
    pub(in crate::app) const fn root() -> Self {
        Self {
            x_milli: VIEW_LAYOUT_ROOT_X_MILLI,
            y_milli: VIEW_LAYOUT_ROOT_Y_MILLI,
        }
    }

    pub(in crate::app) const fn text_control_rect(self, kind: ViewInputKind) -> ViewLogicalRect {
        ViewLogicalRect::new(
            self.x_milli,
            self.y_milli,
            VIEW_LAYOUT_TEXT_CONTROL_WIDTH_MILLI,
            kind.default_text_control_height_milli(),
        )
    }
}

impl ViewLayoutFrame {
    pub(in crate::app) const fn zero() -> Self {
        Self {
            width_milli: 0,
            height_milli: 0,
        }
    }

    pub(in crate::app) const fn new(width_milli: u32, height_milli: u32) -> Self {
        Self {
            width_milli,
            height_milli,
        }
    }

    pub(in crate::app) const fn text_control(kind: ViewInputKind) -> Self {
        Self::new(
            VIEW_LAYOUT_TEXT_CONTROL_WIDTH_MILLI,
            kind.default_text_control_height_milli(),
        )
    }

    pub(in crate::app) const fn action_button() -> Self {
        Self::new(
            VIEW_LAYOUT_BUTTON_WIDTH_MILLI,
            VIEW_LAYOUT_BUTTON_HEIGHT_MILLI,
        )
    }

    pub(in crate::app) const fn is_empty(self) -> bool {
        self.width_milli == 0 || self.height_milli == 0
    }
}

pub(in crate::app) fn button_bounds(
    button: &ViewButton,
    layout: ViewLayoutCursor,
) -> ViewRuntimeButtonBounds {
    ViewRuntimeButtonBounds::new(
        named_layout_length_i32(button.args(), &["x"]).unwrap_or(layout.x_milli),
        named_layout_length_i32(button.args(), &["y"]).unwrap_or(layout.y_milli),
        named_layout_length_u32(button.args(), &["width", "w"])
            .unwrap_or(VIEW_LAYOUT_BUTTON_WIDTH_MILLI),
        named_layout_length_u32(button.args(), &["height", "h"])
            .unwrap_or(VIEW_LAYOUT_BUTTON_HEIGHT_MILLI),
    )
}

pub(in crate::app) fn named_layout_length_u32(args: &[ViewArg], names: &[&str]) -> Option<u32> {
    named_layout_length_i32(args, names).and_then(|value| u32::try_from(value.max(0)).ok())
}

pub(in crate::app) fn modifier_layout_length_u32(
    modifiers: &[ViewModifier],
    names: &[&str],
) -> Option<u32> {
    modifiers.iter().find_map(|modifier| match modifier {
        ViewModifier::Property { name, value }
            if names.iter().any(|candidate| name == candidate) =>
        {
            expr_px_milli(value).and_then(|value| u32::try_from(value.max(0)).ok())
        }
        _ => None,
    })
}

pub(in crate::app) fn text_block_frame(text: &str, modifiers: &[ViewModifier]) -> ViewLayoutFrame {
    let width_milli = modifier_style_or_property_length_u32(
        modifiers,
        &["width", "w", "inline-size", "inline_size"],
    )
    .unwrap_or(VIEW_LAYOUT_TEXT_CONTROL_WIDTH_MILLI)
    .max(1);
    let font_size_milli =
        modifier_style_or_property_length_u32(modifiers, &["font-size"]).unwrap_or(20_000);
    let fallback_line_height = font_size_milli.saturating_mul(6).saturating_add(4) / 5;
    let line_height_milli = modifier_style_or_property_length_u32(
        modifiers,
        &["line-height", "line-height-milli", "line_height_milli"],
    )
    .unwrap_or(fallback_line_height)
    .max(VIEW_LAYOUT_TEXT_LINE_HEIGHT_MILLI);
    let line_count = estimated_wrapped_text_lines(text, width_milli, font_size_milli);
    let inferred_height_milli = line_height_milli.saturating_mul(line_count);
    let height_milli = modifier_style_or_property_length_u32(
        modifiers,
        &["height", "h", "block-size", "block_size"],
    )
    .unwrap_or(inferred_height_milli)
    .max(1);
    ViewLayoutFrame::new(width_milli, height_milli)
}

pub(in crate::app) fn u32_to_i32_saturating(value: u32) -> i32 {
    i32::try_from(value).unwrap_or(i32::MAX)
}

fn named_layout_length_i32(args: &[ViewArg], names: &[&str]) -> Option<i32> {
    names
        .iter()
        .find_map(|name| named_arg(args, name))
        .and_then(expr_px_milli)
}

pub(in crate::app) fn named_arg<'a>(args: &'a [ViewArg], name: &str) -> Option<&'a Expr> {
    args.iter().find_map(|arg| match arg {
        ViewArg::Named {
            name: actual,
            value,
        } if actual == name => Some(value),
        _ => None,
    })
}

fn modifier_style_or_property_length_u32(
    modifiers: &[ViewModifier],
    names: &[&str],
) -> Option<u32> {
    modifier_layout_length_u32(modifiers, names)
        .or_else(|| modifier_inline_style_length_u32(modifiers, names))
}

fn estimated_wrapped_text_lines(text: &str, width_milli: u32, font_size_milli: u32) -> u32 {
    let max_width = width_milli.max(1);
    let mut line_count = 1_u32;
    let mut line_width = 0_u32;
    for ch in text.chars() {
        if ch == '\r' {
            continue;
        }
        if ch == '\n' {
            line_count = line_count.saturating_add(1);
            line_width = 0;
            continue;
        }
        let advance = estimated_text_advance_milli(ch, font_size_milli);
        if line_width > 0 && line_width.saturating_add(advance) > max_width {
            line_count = line_count.saturating_add(1);
            line_width = advance.min(max_width);
        } else {
            line_width = line_width.saturating_add(advance).min(max_width);
        }
    }
    line_count
}

fn estimated_text_advance_milli(ch: char, font_size_milli: u32) -> u32 {
    let ratio_milli = if ch.is_ascii_whitespace() {
        330
    } else if ch.is_ascii_alphanumeric() {
        560
    } else if ch.is_ascii_punctuation() {
        440
    } else {
        1_000
    };
    font_size_milli
        .saturating_mul(ratio_milli)
        .saturating_add(999)
        / 1_000
}

fn modifier_inline_style_length_u32(modifiers: &[ViewModifier], names: &[&str]) -> Option<u32> {
    modifiers.iter().find_map(|modifier| {
        let ViewModifier::Style(
            ViewStyleModifier::InlineArcweft(source) | ViewStyleModifier::InlineCss(source),
        ) = modifier
        else {
            return None;
        };
        inline_style_properties(source).find_map(|(name, value)| {
            let name = normalize_property_name(&name);
            names
                .iter()
                .any(|candidate| name == normalize_property_name(candidate))
                .then(|| style_layout_length_u32(&value))
                .flatten()
        })
    })
}

fn expr_px_milli(expr: &Expr) -> Option<i32> {
    match expr {
        Expr::Literal(Literal::UnitNumber {
            raw,
            suffix: UnitNumberSuffix::Px,
        }) => {
            let raw = raw.trim();
            let raw = raw.strip_suffix("px").map_or(raw, str::trim);
            parse_px_milli(raw)
        }
        Expr::Literal(Literal::UnitNumber {
            raw,
            suffix: UnitNumberSuffix::Milli,
        }) => {
            let raw = raw.trim();
            raw.strip_suffix("milli")
                .map(str::trim)
                .and_then(|raw| raw.parse::<i32>().ok())
        }
        Expr::Literal(Literal::Int { value, .. }) => {
            i32::try_from(value.saturating_mul(1_000)).ok()
        }
        Expr::Raw(value) => value
            .trim()
            .strip_suffix("px")
            .map(str::trim)
            .and_then(parse_px_milli),
        Expr::Path(value) => value
            .as_label()
            .trim()
            .strip_suffix("px")
            .map(str::trim)
            .and_then(parse_px_milli),
        _ => None,
    }
}

pub(in crate::app) fn parse_px_milli(raw: &str) -> Option<i32> {
    let source = raw.trim().replace('_', "");
    let (negative, unsigned) = source
        .strip_prefix('-')
        .map_or((false, source.as_str()), |rest| (true, rest));
    let unsigned = unsigned.strip_prefix('+').unwrap_or(unsigned);
    let (whole, fraction) = unsigned.split_once('.').unwrap_or((unsigned, ""));
    if whole.is_empty() && fraction.is_empty() {
        return None;
    }
    if !whole.chars().all(|ch| ch.is_ascii_digit())
        || !fraction.chars().all(|ch| ch.is_ascii_digit())
    {
        return None;
    }

    let whole_milli = if whole.is_empty() {
        0
    } else {
        whole.parse::<i64>().ok()?.checked_mul(1_000)?
    };
    let (fraction_milli, round_up) = fractional_px_milli(fraction)?;
    let magnitude = whole_milli
        .checked_add(fraction_milli)?
        .checked_add(i64::from(round_up))?;
    let signed = if negative { -magnitude } else { magnitude };
    i32::try_from(signed).ok()
}

fn fractional_px_milli(fraction: &str) -> Option<(i64, bool)> {
    let mut milli = 0_i64;
    let mut scale = 100_i64;
    for digit in fraction.chars().take(3) {
        let value = i64::from(digit.to_digit(10)?);
        milli = milli.checked_add(value.checked_mul(scale)?)?;
        scale /= 10;
    }
    let round_up = fraction
        .chars()
        .nth(3)
        .and_then(|digit| digit.to_digit(10))
        .is_some_and(|digit| digit >= 5);
    Some((milli, round_up))
}
