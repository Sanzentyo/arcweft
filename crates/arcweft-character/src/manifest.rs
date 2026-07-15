//! Validated character manifest model, source-backed registration decoder, and runtime codec.

mod codec;
pub mod diagnostic;
mod fingerprint;
pub mod limits;
mod model;
pub mod registration;

pub use diagnostic::CharacterRuntimeDecodeError;
pub use fingerprint::CharacterManifestFingerprint;
pub use model::{
    CharacterAssetPath, CharacterAssetPathError, CharacterBlendMode, CharacterCanvas,
    CharacterLook, CharacterManifest, CharacterManifestError, CharacterPart,
    CharacterPartSelection, CharacterPoint, CharacterRect, CharacterSource, CharacterSourceKind,
    CharacterSourceLayer, CharacterVariant, ResolvedCharacterLayer,
};

#[cfg(test)]
mod tests;
