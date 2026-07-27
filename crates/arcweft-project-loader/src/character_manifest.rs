//! Character-package manifest path ownership for the profile topology loader.

use std::path::{Path, PathBuf};

/// File name inside an `.awchar` character package directory.
const CHARACTER_MANIFEST_FILE_NAME: &str = "character.awchar.json";

/// Resolves a manifest file path lexically from a direct path or `.awchar` suffix.
pub(crate) fn manifest_path(path: &Path) -> PathBuf {
    if path.extension().and_then(|extension| extension.to_str()) == Some("awchar") {
        path.join(CHARACTER_MANIFEST_FILE_NAME)
    } else {
        path.to_path_buf()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn awchar_suffix_resolves_without_directory_probe() {
        assert_eq!(
            manifest_path(Path::new("missing/akane.awchar")),
            PathBuf::from("missing/akane.awchar/character.awchar.json")
        );
    }

    #[test]
    fn direct_character_manifest_path_remains_direct() {
        let path = Path::new("assets/akane.character.json");
        assert_eq!(manifest_path(path), path);
    }
}
