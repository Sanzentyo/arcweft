//! Production limits for accepted Character presentation metadata.

use core::fmt;

pub const MAX_CATALOG_CHARACTERS: usize = 65_536;
pub const MAX_LOCALIZED_NAMES_PER_CHARACTER: usize = 64;
pub const MAX_CATALOG_LOCALIZED_ENTRIES: usize = 262_144;
pub const MAX_FALLBACK_LOCALES: usize = 16;
pub const MAX_CHARACTER_DISPLAY_NAME_BYTES: usize = 1_024;
pub const MAX_CHARACTER_DISPLAY_NAME_SCALARS: usize = 256;
pub const MAX_CHARACTER_ID_BYTES: usize = 4_096;
pub const MAX_GENERATED_DISPLAY_NAME_KEY_BYTES: usize = 8_512;

/// Count or byte budget enforced by the accepted presentation catalog.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CharacterPresentationLimitKind {
    Characters,
    LocalizedEntriesPerCharacter,
    TotalLocalizedEntries,
    CharacterIdBytes,
    GeneratedDisplayNameKeyBytes,
}

impl fmt::Display for CharacterPresentationLimitKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Characters => "characters",
            Self::LocalizedEntriesPerCharacter => "localized entries per Character",
            Self::TotalLocalizedEntries => "total localized entries",
            Self::CharacterIdBytes => "Character ID bytes",
            Self::GeneratedDisplayNameKeyBytes => "generated display-name key bytes",
        })
    }
}
