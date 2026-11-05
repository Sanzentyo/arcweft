//! Authored font-family values, shared by checking, policy codecs and layout.

use serde::{Deserialize, Serialize};

/// A generic family or an explicitly requested named family.
/// Quoted names retain their identity even when they spell a generic family.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RichTextFontFamily<S = String> {
    Serif,
    SansSerif,
    Monospace,
    Cursive,
    Fantasy,
    Named { name: S },
}

impl<S> RichTextFontFamily<S> {
    /// Maximum UTF-8 bytes accepted for a requested named family.
    pub const MAX_NAME_BYTES: u16 = 256;
    /// Canonical generic cases in version-one declaration order.
    pub const GENERIC: [Self; 5] = [
        Self::Serif,
        Self::SansSerif,
        Self::Monospace,
        Self::Cursive,
        Self::Fantasy,
    ];

    /// The payload-bearing case follows all generic cases.
    pub const NAMED_ORDINAL: u32 = 5;
    pub const NAMED_VARIANT_NAME: &'static str = "named";

    /// Source identifier of the selected case. The named payload is separate.
    #[must_use]
    pub const fn variant_name(&self) -> &'static str {
        match self {
            Self::Serif => "serif",
            Self::SansSerif => "sans_serif",
            Self::Monospace => "monospace",
            Self::Cursive => "cursive",
            Self::Fantasy => "fantasy",
            Self::Named { .. } => Self::NAMED_VARIANT_NAME,
        }
    }

    #[must_use]
    pub const fn variant_ordinal(&self) -> u32 {
        match self {
            Self::Serif => 0,
            Self::SansSerif => 1,
            Self::Monospace => 2,
            Self::Cursive => 3,
            Self::Fantasy => 4,
            Self::Named { .. } => Self::NAMED_ORDINAL,
        }
    }

    /// Only payload-free generic cases can be constructed from an ordinal alone.
    #[must_use]
    pub fn from_generic_ordinal(ordinal: u32) -> Option<Self> {
        Self::GENERIC
            .into_iter()
            .find(|case| case.variant_ordinal() == ordinal)
    }

    /// Maps the named payload without reclassifying its text.
    #[must_use]
    pub fn map_name<T>(self, map: impl FnOnce(S) -> T) -> RichTextFontFamily<T> {
        match self {
            Self::Serif => RichTextFontFamily::Serif,
            Self::SansSerif => RichTextFontFamily::SansSerif,
            Self::Monospace => RichTextFontFamily::Monospace,
            Self::Cursive => RichTextFontFamily::Cursive,
            Self::Fantasy => RichTextFontFamily::Fantasy,
            Self::Named { name } => RichTextFontFamily::Named { name: map(name) },
        }
    }
}

impl<S: AsRef<str>> RichTextFontFamily<S> {
    #[must_use]
    pub fn as_ref(&self) -> RichTextFontFamily<&str> {
        match self {
            Self::Serif => RichTextFontFamily::Serif,
            Self::SansSerif => RichTextFontFamily::SansSerif,
            Self::Monospace => RichTextFontFamily::Monospace,
            Self::Cursive => RichTextFontFamily::Cursive,
            Self::Fantasy => RichTextFontFamily::Fantasy,
            Self::Named { name } => RichTextFontFamily::Named {
                name: name.as_ref(),
            },
        }
    }
}
