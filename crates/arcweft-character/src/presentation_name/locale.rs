//! Nominal Character-name locale roles and borrowed resolution input.

use arcweft_id::LocaleTag;
use core::fmt;

/// Locale identity used by Character display-name metadata.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CharacterNameLocale(LocaleTag);

/// Character-declared locale used after project fallback locales.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CharacterNameSourceLocale(CharacterNameLocale);

/// One lookup's session locale and accepted project fallback chain.
///
/// This borrows the project policy rather than retaining a second Character
/// policy. A record's explicit source locale supersedes `project_source`.
#[derive(Clone, Copy, Debug)]
pub struct CharacterNameResolutionLocales<'a> {
    active: &'a LocaleTag,
    project_source: &'a LocaleTag,
    project_fallbacks: &'a [LocaleTag],
}

impl CharacterNameLocale {
    #[must_use]
    pub const fn new(value: LocaleTag) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn locale_tag(&self) -> &LocaleTag {
        &self.0
    }
}

impl fmt::Display for CharacterNameLocale {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.locale_tag().as_str())
    }
}

impl fmt::Display for CharacterNameSourceLocale {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.locale().fmt(formatter)
    }
}

impl CharacterNameSourceLocale {
    #[must_use]
    pub const fn new(value: CharacterNameLocale) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn locale(&self) -> &CharacterNameLocale {
        &self.0
    }
}

impl<'a> CharacterNameResolutionLocales<'a> {
    #[must_use]
    pub const fn new(
        active: &'a LocaleTag,
        project_source: &'a LocaleTag,
        project_fallbacks: &'a [LocaleTag],
    ) -> Self {
        Self {
            active,
            project_source,
            project_fallbacks,
        }
    }

    #[must_use]
    pub const fn active(&self) -> &LocaleTag {
        self.active
    }

    #[must_use]
    pub const fn project_source(&self) -> &LocaleTag {
        self.project_source
    }

    #[must_use]
    pub const fn project_fallbacks(&self) -> &[LocaleTag] {
        self.project_fallbacks
    }
}
