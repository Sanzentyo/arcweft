//! Accepted Character display-name metadata and deterministic locale lookup.
//!
//! This module is Sans I/O. Source parsing, manifest decoding, bundle bytes,
//! runtime publication, and presentation remain in their respective owners.

mod catalog;
mod digest;
mod limits;
mod locale;
mod transcript;
mod value;

pub use catalog::{
    AcceptedCharacterPresentationCatalog, CharacterDisplayNameLookupError,
    CharacterDisplayNameRecord, CharacterDisplayNameRecordInput,
    CharacterDisplayNameResolutionSource, CharacterPresentationCatalogData,
    CharacterPresentationCatalogError, CharacterPresentationCatalogGeneration,
    CharacterPresentationCatalogInput, CharacterPresentationCatalogPublicationError,
    CharacterPresentationCatalogRevision, CharacterPresentationCatalogRevisionError,
    CharacterPresentationRole, ResolvedCharacterDisplayName,
};
pub use digest::{
    CharacterPresentationLocalePolicyDigest, CharacterPresentationSemanticDigest, DigestParseError,
};
pub use limits::{
    CharacterPresentationLimitKind, MAX_CATALOG_CHARACTERS, MAX_CATALOG_LOCALIZED_ENTRIES,
    MAX_CHARACTER_DISPLAY_NAME_BYTES, MAX_CHARACTER_DISPLAY_NAME_SCALARS, MAX_CHARACTER_ID_BYTES,
    MAX_FALLBACK_LOCALES, MAX_GENERATED_DISPLAY_NAME_KEY_BYTES, MAX_LOCALIZED_NAMES_PER_CHARACTER,
};
pub use locale::{
    CharacterNameFallbackLocale, CharacterNameLocale, CharacterNameLocalePolicy,
    CharacterNameLocalePolicyError, CharacterNameSourceLocale,
};
pub use value::{
    CharacterDeclarationNameFallback, CharacterDisplayNameEntry, CharacterDisplayNameInput,
    CharacterDisplayNameKey, CharacterDisplayNameKeyError, CharacterDisplayNameValue,
    CharacterDisplayNameValueError, LocalizedCharacterDisplayName,
    LocalizedCharacterDisplayNameInput,
};

#[cfg(test)]
mod tests;
