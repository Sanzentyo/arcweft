//! Sole compact codec for accepted Character presentation `LocaleCatalog` data.
//!
//! This family is closed at schema 1. It has no JSON, TOML, source, legacy,
//! or alternate bundle reader.

mod decode;
mod encode;
mod error;
mod wire;

pub use error::CharacterPresentationCatalogCodecError;

use arcweft_character::presentation_name::CharacterPresentationCatalogData;

/// Canonical compact `LocaleCatalog` family for Character presentation metadata.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CharacterPresentationCatalogSection;

impl CharacterPresentationCatalogSection {
    pub fn encode_canonical(
        catalog: &CharacterPresentationCatalogData,
    ) -> Result<Vec<u8>, CharacterPresentationCatalogCodecError> {
        encode::encode(catalog)
    }

    pub fn decode_canonical(
        bytes: &[u8],
    ) -> Result<CharacterPresentationCatalogData, CharacterPresentationCatalogCodecError> {
        decode::decode(bytes)
    }
}
