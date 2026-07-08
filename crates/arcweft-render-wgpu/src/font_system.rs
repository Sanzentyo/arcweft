//! Renderer-owned `glyphon::FontSystem` construction.
//!
//! `cosmic-text` uses the FontSystem locale when choosing platform fallback
//! faces for Han text. Some platform locale APIs report BCP-47 region tags such
//! as `ja-JP`, while the Windows fallback table currently keys Japanese and
//! Korean Han fallback by primary language labels (`ja`, `ko`). Normalize the
//! locale label at FontSystem construction so fallback remains locale-driven
//! without hard-coding Arcweft's own family priority list.

use glyphon::FontSystem;
use std::borrow::Cow;

pub(crate) fn new_font_system() -> FontSystem {
    let system = FontSystem::new();
    let original_locale = system.locale().to_owned();
    let locale = fallback_locale_label(&original_locale).into_owned();
    if locale == original_locale {
        return system;
    }
    let (_, db) = system.into_locale_and_db();
    FontSystem::new_with_locale_and_db(locale, db)
}

fn fallback_locale_label(locale: &str) -> Cow<'_, str> {
    let locale = locale.trim();
    if locale.is_empty() {
        return Cow::Borrowed(locale);
    }
    let normalized_separators = locale.replace('_', "-");
    let mut parts = normalized_separators.split('-');
    let language = parts.next().unwrap_or_default();
    if language.eq_ignore_ascii_case("ja") {
        return Cow::Borrowed("ja");
    }
    if language.eq_ignore_ascii_case("ko") {
        return Cow::Borrowed("ko");
    }
    if !language.eq_ignore_ascii_case("zh") {
        return if normalized_separators == locale {
            Cow::Borrowed(locale)
        } else {
            Cow::Owned(normalized_separators)
        };
    }

    let subtags = normalized_separators
        .split('-')
        .skip(1)
        .map(|part| part.to_ascii_uppercase())
        .collect::<Vec<_>>();
    if subtags.iter().any(|part| part == "HK") {
        Cow::Borrowed("zh-HK")
    } else if subtags.iter().any(|part| part == "TW") {
        Cow::Borrowed("zh-TW")
    } else if normalized_separators == locale {
        Cow::Borrowed(locale)
    } else {
        Cow::Owned(normalized_separators)
    }
}

#[cfg(test)]
mod tests {
    use super::fallback_locale_label;

    #[test]
    fn fallback_locale_collapses_japanese_region_tag() {
        assert_eq!(fallback_locale_label("ja-JP"), "ja");
        assert_eq!(fallback_locale_label("ja_JP"), "ja");
    }

    #[test]
    fn fallback_locale_keeps_chinese_region_that_cosmic_text_distinguishes() {
        assert_eq!(fallback_locale_label("zh-Hant-TW"), "zh-TW");
        assert_eq!(fallback_locale_label("zh_HK"), "zh-HK");
    }

    #[test]
    fn fallback_locale_preserves_unrelated_locale() {
        assert_eq!(fallback_locale_label("en-US"), "en-US");
    }
}
