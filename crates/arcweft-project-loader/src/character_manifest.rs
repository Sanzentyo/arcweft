//! Filesystem adapter for source-backed character manifests.

use arcweft_character::manifest::{
    diagnostic::CharacterRegistrationDecodeError, registration::SourceBackedCharacterManifest,
};
use arcweft_source::{
    SourceDocument, SourceDocumentError, SourceDocumentId, SourceDocumentIdError, SourceName,
};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};
use thiserror::Error;

use crate::project::{ProjectLoadError, project_document_id};

/// File name inside an `.awchar` character package directory.
pub const CHARACTER_MANIFEST_FILE_NAME: &str = "character.awchar.json";

/// A registration manifest together with the exact bytes that own its spans.
#[derive(Clone, Debug)]
pub struct LoadedCharacterManifest {
    document: Arc<SourceDocument>,
    path: PathBuf,
    manifest: SourceBackedCharacterManifest,
}

/// Character-manifest loading failure.
#[derive(Debug, Error)]
pub enum LoadError {
    #[error("failed to read character manifest: {0}")]
    Read(#[from] std::io::Error),
    #[error("invalid character manifest document identity: {0}")]
    DocumentId(#[from] SourceDocumentIdError),
    #[error("failed to construct character manifest source document: {0}")]
    Document(#[from] SourceDocumentError),
    #[error("failed to assign the project-relative character document identity: {0}")]
    ProjectDocument(#[from] ProjectLoadError),
    #[error("failed to parse character manifest: {0}")]
    Parse(#[from] CharacterRegistrationDecodeError),
}

impl LoadedCharacterManifest {
    pub fn document(&self) -> &Arc<SourceDocument> {
        &self.document
    }

    pub const fn manifest(&self) -> &SourceBackedCharacterManifest {
        &self.manifest
    }

    /// Exact manifest file path used for decoding.
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn into_parts(self) -> (Arc<SourceDocument>, PathBuf, SourceBackedCharacterManifest) {
        (self.document, self.path, self.manifest)
    }
}

/// Resolves a manifest file path lexically from a direct path or `.awchar` suffix.
pub fn manifest_path(path: &Path) -> PathBuf {
    if path.extension().and_then(|extension| extension.to_str()) == Some("awchar") {
        path.join(CHARACTER_MANIFEST_FILE_NAME)
    } else {
        path.to_path_buf()
    }
}

/// Decodes a character manifest from an already captured source document.
pub fn decode(
    path: PathBuf,
    document: Arc<SourceDocument>,
) -> Result<LoadedCharacterManifest, LoadError> {
    let manifest = SourceBackedCharacterManifest::decode_registration_json(&document)?;
    Ok(LoadedCharacterManifest {
        document,
        path,
        manifest,
    })
}

/// Reads and structurally validates one source-backed registration manifest.
pub fn load(path: &Path) -> Result<LoadedCharacterManifest, LoadError> {
    let path = manifest_path(path);
    let source = fs::read_to_string(&path)?;
    let id = SourceDocumentId::try_new(path.to_string_lossy().replace('\\', "/"))?;
    let document = Arc::new(SourceDocument::try_new(
        id,
        SourceName::path(path.display().to_string()),
        source,
    )?);
    decode(path, document)
}

/// Reads one registration manifest with the owning project's canonical document identity.
pub fn load_for_project(
    path: &Path,
    package: &str,
    project_root: &Path,
) -> Result<LoadedCharacterManifest, LoadError> {
    let path = manifest_path(path);
    let source = fs::read_to_string(&path)?;
    let document = Arc::new(SourceDocument::try_new(
        project_document_id(package, project_root, &path)?,
        SourceName::path(path.display().to_string()),
        source,
    )?);
    decode(path, document)
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

    #[test]
    fn decode_uses_the_supplied_document_without_filesystem_access() {
        let path = PathBuf::from("missing/akane.awchar/character.awchar.json");
        let document = Arc::new(
            SourceDocument::try_new(
                SourceDocumentId::try_new("arcweft-project://fixture/akane.awchar.json")
                    .expect("document id"),
                SourceName::path("captured-character-manifest"),
                include_str!(
                    "../../arcweft-character/tests/fixtures/zundamon.awchar/character.awchar.json"
                ),
            )
            .expect("source document"),
        );

        let loaded = decode(path.clone(), Arc::clone(&document)).expect("document decodes");

        assert!(Arc::ptr_eq(loaded.document(), &document));
        assert_eq!(loaded.path(), path);
    }

    #[test]
    fn loaded_directory_manifest_retains_the_resolved_file_path() {
        let root = std::env::temp_dir().join(format!(
            "arcweft-character-path-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock follows epoch")
                .as_nanos()
        ));
        let package = root.join("akane.awchar");
        std::fs::create_dir_all(&package).expect("package directory");
        let path = package.join(CHARACTER_MANIFEST_FILE_NAME);
        std::fs::write(
            &path,
            include_str!(
                "../../arcweft-character/tests/fixtures/zundamon.awchar/character.awchar.json"
            ),
        )
        .expect("manifest fixture");

        let loaded = load(&package).expect("directory manifest loads");
        assert_eq!(loaded.path(), path);

        std::fs::remove_dir_all(root).expect("fixture removes");
    }
}
