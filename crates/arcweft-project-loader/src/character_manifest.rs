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

    pub fn into_parts(self) -> (Arc<SourceDocument>, SourceBackedCharacterManifest) {
        (self.document, self.manifest)
    }
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
    let manifest = SourceBackedCharacterManifest::decode_registration_json(&document)?;
    Ok(LoadedCharacterManifest { document, manifest })
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
    let manifest = SourceBackedCharacterManifest::decode_registration_json(&document)?;
    Ok(LoadedCharacterManifest { document, manifest })
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
