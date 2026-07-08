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
        RenderFontFamily::Named(name) => render_single_named_font_family(name),
        RenderFontFamily::Stack(stack) => render_font_family_stack(stack),
    }
}

fn render_font_family_stack(stack: &[String]) -> Family<'_> {
    let Some(family) = preferred_stack_font_family(stack) else {
        trace_font_once(format_args!(
            "font-family stack={stack:?} selected=<none> glyphon_family=SansSerif"
        ));
        return Family::SansSerif;
    };
    render_named_font_family_with_trace(family, format_args!("font-family stack={stack:?}"))
}

fn render_single_named_font_family(family: &str) -> Family<'_> {
    let family = trim_font_family_token(family);
    if family.is_empty() {
        trace_font_once(format_args!(
            "font-family name={family:?} selected=<none> glyphon_family=SansSerif"
        ));
        return Family::SansSerif;
    }
    render_named_font_family_with_trace(family, format_args!("font-family name={family:?}"))
}

fn render_named_font_family_with_trace<'a>(
    family: &'a str,
    prefix: fmt::Arguments<'_>,
) -> Family<'a> {
    if let Some(generic) = generic_font_family(family) {
        trace_font_once(format_args!(
            "{prefix} selected={family:?} glyphon_family=generic"
        ));
        generic
    } else {
        trace_font_once(format_args!(
            "{prefix} selected={family:?} glyphon_family=name"
        ));
        Family::Name(family)
    }
}

fn preferred_stack_font_family(stack: &[String]) -> Option<&str> {
    let mut first = None;
    let mut first_non_generic = None;
    for family in stack.iter().map(String::as_str).map(trim_font_family_token) {
        if family.is_empty() {
            continue;
        }
        first.get_or_insert(family);
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

fn generic_font_family(family: &str) -> Option<Family<'static>> {
    if family.eq_ignore_ascii_case("serif") {
        Some(Family::Serif)
    } else if family.eq_ignore_ascii_case("sans-serif")
        || family.eq_ignore_ascii_case("sans")
        || family.eq_ignore_ascii_case("system-view")
        || family.eq_ignore_ascii_case("view-sans-serif")
    {
        Some(Family::SansSerif)
    } else if family.eq_ignore_ascii_case("monospace")
        || family.eq_ignore_ascii_case("view-monospace")
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
    fn font_stack_preserves_authored_non_generic_order() {
        assert_eq!(
            preferred_stack_font_family(&[
                "Arcweft Demo".to_owned(),
                "Yu Gothic View".to_owned(),
                "Yu Gothic".to_owned(),
                "Hiragino Sans".to_owned(),
                "Noto Sans JP".to_owned(),
                "system-view".to_owned(),
            ]),
            Some("Arcweft Demo")
        );
    }

    #[test]
    fn font_stack_falls_back_to_first_non_generic_family() {
        assert_eq!(
            preferred_stack_font_family(&[
                "Arcweft Display".to_owned(),
                "system-view".to_owned(),
                "sans-serif".to_owned(),
            ]),
            Some("Arcweft Display")
        );
    }
}
