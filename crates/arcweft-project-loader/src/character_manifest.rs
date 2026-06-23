//! Filesystem adapter for Arcweft character manifests.

use arcweft_character::manifest::{CharacterManifest, CharacterManifestCodecError};
use std::{
    fs,
    path::{Path, PathBuf},
};
use thiserror::Error;

/// File name inside an `.awchar` character package directory.
pub const CHARACTER_MANIFEST_FILE_NAME: &str = "character.awchar.json";

/// Character-manifest loading failure.
#[derive(Debug, Error)]
pub enum LoadError {
    #[error("failed to read character manifest: {0}")]
    Read(#[from] std::io::Error),
    #[error("failed to parse character manifest: {0}")]
    Parse(#[from] CharacterManifestCodecError),
}

/// Resolves a manifest file path from a direct JSON path or `.awchar` directory.
pub fn manifest_path(path: &Path) -> PathBuf {
    if path.extension().and_then(|extension| extension.to_str()) == Some("awchar") || path.is_dir()
    {
        path.join(CHARACTER_MANIFEST_FILE_NAME)
    } else {
        path.to_path_buf()
    }
}

/// Reads and validates one character manifest from disk.
pub fn load(path: &Path) -> Result<CharacterManifest, LoadError> {
    let source = fs::read_to_string(manifest_path(path))?;
    CharacterManifest::from_json(&source).map_err(LoadError::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn awchar_directories_resolve_to_the_package_manifest() {
        assert_eq!(
            manifest_path(Path::new("assets/akane.awchar")),
            PathBuf::from("assets/akane.awchar/character.awchar.json")
        );
    }
}
