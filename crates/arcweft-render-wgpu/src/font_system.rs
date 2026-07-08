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

pub(crate) struct FontRegistrationReport {
    pub before_faces: usize,
    pub after_faces: usize,
    pub primary_sans_family: Option<String>,
}

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

pub(crate) fn load_font_data_and_maybe_set_primary_sans(
    font_system: &mut FontSystem,
    bytes: Vec<u8>,
    set_primary_sans: bool,
) -> FontRegistrationReport {
    let before_faces = font_system.db().faces().count();
    font_system.db_mut().load_font_data(bytes);
    let primary_sans_family = set_primary_sans
        .then(|| first_loaded_family_name(font_system, before_faces))
        .flatten();
    if let Some(family) = primary_sans_family.as_deref() {
        font_system.db_mut().set_sans_serif_family(family);
    }
    FontRegistrationReport {
        before_faces,
        after_faces: font_system.db().faces().count(),
        primary_sans_family,
    }
}

fn first_loaded_family_name(font_system: &FontSystem, before_faces: usize) -> Option<String> {
    font_system
        .db()
        .faces()
        .skip(before_faces)
        .find_map(|face| face.families.first().map(|family| family.0.clone()))
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
