use crate::geometry::RenderFontFamily;
use glyphon::Family;
use std::collections::HashSet;
use std::fmt;
use std::sync::{Mutex, OnceLock};

const FONT_TRACE_ENV: &str = "ARCWEFT_FONT_TRACE";

pub(crate) fn render_font_family(family: &RenderFontFamily) -> Family<'_> {
    match family {
        RenderFontFamily::Serif => Family::Serif,
        RenderFontFamily::SansSerif => Family::SansSerif,
        RenderFontFamily::Monospace => Family::Monospace,
        RenderFontFamily::Cursive => Family::Cursive,
        RenderFontFamily::Fantasy => Family::Fantasy,
        RenderFontFamily::Named(name) => render_named_font_family(name),
    }
}

fn render_named_font_family(stack: &str) -> Family<'_> {
    let Some(family) = preferred_named_font_family(stack) else {
        trace_font_once(format_args!(
            "font-family stack={stack:?} selected=<none> glyphon_family=SansSerif"
        ));
        return Family::SansSerif;
    };
    if let Some(generic) = generic_font_family(family) {
        trace_font_once(format_args!(
            "font-family stack={stack:?} selected={family:?} glyphon_family=generic"
        ));
        generic
    } else {
        trace_font_once(format_args!(
            "font-family stack={stack:?} selected={family:?} glyphon_family=name"
        ));
        Family::Name(family)
    }
}

fn preferred_named_font_family(stack: &str) -> Option<&str> {
    let mut first = None;
    let mut first_non_generic = None;
    for family in stack.split(',').map(trim_font_family_token) {
        if family.is_empty() {
            continue;
        }
        first.get_or_insert(family);
        if preferred_cjk_font_family(family) {
            return Some(family);
        }
        if first_non_generic.is_none() && generic_font_family(family).is_none() {
            first_non_generic = Some(family);
        }
    }
    first_non_generic.or(first)
}

fn trim_font_family_token(raw: &str) -> &str {
    let trimmed = raw.trim();
    let unquoted = trimmed
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .or_else(|| {
            trimmed
                .strip_prefix('\'')
                .and_then(|value| value.strip_suffix('\''))
        })
        .unwrap_or(trimmed);
    unquoted.trim()
}

fn preferred_cjk_font_family(family: &str) -> bool {
    [
        "Yu Gothic",
        "Yu Gothic UI",
        "Meiryo",
        "Hiragino Sans",
        "Hiragino Kaku Gothic ProN",
        "Noto Sans JP",
        "Noto Sans CJK JP",
        "Source Han Sans JP",
    ]
    .into_iter()
    .any(|candidate| family.eq_ignore_ascii_case(candidate))
}

fn generic_font_family(family: &str) -> Option<Family<'static>> {
    if family.eq_ignore_ascii_case("serif") {
        Some(Family::Serif)
    } else if family.eq_ignore_ascii_case("sans-serif")
        || family.eq_ignore_ascii_case("sans")
        || family.eq_ignore_ascii_case("system-ui")
        || family.eq_ignore_ascii_case("ui-sans-serif")
    {
        Some(Family::SansSerif)
    } else if family.eq_ignore_ascii_case("monospace")
        || family.eq_ignore_ascii_case("ui-monospace")
    {
        Some(Family::Monospace)
    } else if family.eq_ignore_ascii_case("cursive") {
        Some(Family::Cursive)
    } else if family.eq_ignore_ascii_case("fantasy") {
        Some(Family::Fantasy)
    } else {
        None
    }
}

pub(crate) fn font_trace_enabled() -> bool {
    std::env::var_os(FONT_TRACE_ENV).is_some_and(|value| {
        let value = value.to_string_lossy();
        !value.is_empty() && value != "0" && !value.eq_ignore_ascii_case("false")
    })
}

pub(crate) fn trace_font_debug(args: fmt::Arguments<'_>) {
    if font_trace_enabled() {
        eprintln!("[arcweft-font-trace] {args}");
    }
}

fn trace_font_once(args: fmt::Arguments<'_>) {
    if !font_trace_enabled() {
        return;
    }
    let message = args.to_string();
    let seen = FONT_TRACE_ONCE.get_or_init(Default::default);
    let Ok(mut seen) = seen.lock() else {
        eprintln!("[arcweft-font-trace] {message}");
        return;
    };
    if seen.insert(message.clone()) {
        eprintln!("[arcweft-font-trace] {message}");
    }
}

static FONT_TRACE_ONCE: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn font_stack_prefers_japanese_system_family() {
        assert_eq!(
            preferred_named_font_family(
                "\"Arcweft Demo\", Yu Gothic, Hiragino Sans, Noto Sans JP, system-ui"
            ),
            Some("Yu Gothic")
        );
    }

    #[test]
    fn font_stack_falls_back_to_first_non_generic_family() {
        assert_eq!(
            preferred_named_font_family("Arcweft Display, system-ui, sans-serif"),
            Some("Arcweft Display")
        );
    }
}
