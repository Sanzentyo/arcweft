use crate::geometry::RenderFontFamily;
use glyphon::Family;

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
        return Family::SansSerif;
    };
    generic_font_family(family).unwrap_or(Family::Name(family))
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
