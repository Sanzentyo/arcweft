use arcweft_rust_abi::{ArcweftRustAbiError, ArcweftRustManifest};
use arcweft_source::{
    SourceDocument, SourceDocumentError, SourceDocumentId, SourceDocumentIdError, SourceName,
};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
use thiserror::Error;

/// Decoded Rust ABI metadata together with the exact document that supplied it.
#[derive(Clone, Debug)]
pub struct LoadedRustMetadata {
    document: Arc<SourceDocument>,
    path: PathBuf,
    manifest: ArcweftRustManifest,
}

impl LoadedRustMetadata {
    /// Exact source document used for decoding.
    pub fn document(&self) -> &Arc<SourceDocument> {
        &self.document
    }

    /// Exact declared metadata path.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Decoded typed Rust ABI metadata.
    pub const fn manifest(&self) -> &ArcweftRustManifest {
        &self.manifest
    }
}

/// Loads Arcweft Rust ABI metadata from JSON.
pub fn load(path: &Path) -> Result<LoadedRustMetadata, LoadError> {
    let path = path.to_path_buf();
    let source = std::fs::read_to_string(&path).map_err(LoadError::Read)?;
    let document = Arc::new(SourceDocument::try_new(
        SourceDocumentId::try_new(path.to_string_lossy().replace('\\', "/"))?,
        SourceName::path(path.display().to_string()),
        source,
    )?);
    decode(path, document)
}

/// Decodes Rust ABI metadata from an already captured source document.
pub fn decode(
    path: PathBuf,
    document: Arc<SourceDocument>,
) -> Result<LoadedRustMetadata, LoadError> {
    let manifest = ArcweftRustManifest::from_json(document.text()).map_err(LoadError::Parse)?;
    Ok(LoadedRustMetadata {
        document,
        path,
        manifest,
    })
}

/// Rust metadata load failure without host path decoration.
#[derive(Debug, Error)]
pub enum LoadError {
    #[error("failed to read Rust ABI metadata: {0}")]
    Read(std::io::Error),
    #[error("invalid Rust ABI metadata document identity: {0}")]
    DocumentId(#[from] SourceDocumentIdError),
    #[error("failed to construct Rust ABI metadata source document: {0}")]
    Document(#[from] SourceDocumentError),
    #[error("failed to parse Rust ABI metadata: {0}")]
    Parse(ArcweftRustAbiError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_rust_abi::{
        ArcweftRustFunction, ArcweftRustPackage, ArcweftRustParam, ArcweftRustPurity,
        ArcweftRustTypeRef,
    };
    use std::fs::{create_dir_all, write};
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn loads_valid_rust_metadata() {
        let project = TestProject::new("loader-rust-valid");
        let path = project.path("metadata.json");
        project.write(
            "metadata.json",
            &rust_manifest().to_json_pretty().expect("metadata json"),
        );

        let loaded = load(&path).expect("rust metadata loads");

        assert_eq!(loaded.manifest().package.name, "custom_adapter");
        assert_eq!(loaded.path(), path);
    }

    #[test]
    fn decode_uses_the_supplied_document_without_filesystem_access() {
        let path = PathBuf::from("missing/metadata.json");
        let source = rust_manifest().to_json_pretty().expect("metadata json");
        let document = Arc::new(
            SourceDocument::try_new(
                SourceDocumentId::try_new("arcweft-project://fixture/metadata.json")
                    .expect("document id"),
                SourceName::path("captured-metadata"),
                source,
            )
            .expect("source document"),
        );

        let loaded = decode(path.clone(), Arc::clone(&document)).expect("document decodes");

        assert!(Arc::ptr_eq(loaded.document(), &document));
        assert_eq!(loaded.path(), path);
        assert_eq!(loaded.manifest().package.name, "custom_adapter");
    }

    #[test]
    fn missing_rust_metadata_reports_read() {
        let project = TestProject::new("loader-rust-missing");
        let error = load(&project.path("missing.json")).expect_err("missing file errors");

        assert!(matches!(error, LoadError::Read(_)));
    }

    #[test]
    fn malformed_rust_metadata_reports_parse() {
        let project = TestProject::new("loader-rust-bad");
        let path = project.path("metadata.json");
        project.write("metadata.json", "{ not json");

        let error = load(&path).expect_err("bad json errors");

        assert!(matches!(error, LoadError::Parse(_)));
    }

    fn rust_manifest() -> ArcweftRustManifest {
        ArcweftRustManifest::new(ArcweftRustPackage {
            name: "custom_adapter".to_owned(),
            version: "0.1.0".to_owned(),
            metadata_hash: None,
        })
        .with_function(ArcweftRustFunction {
            name: "custom.score".to_owned(),
            rust_path: "custom_adapter::score".to_owned(),
            params: vec![ArcweftRustParam {
                name: "value".to_owned(),
                ty: ArcweftRustTypeRef::I32,
            }],
            return_type: ArcweftRustTypeRef::I64,
            purity: ArcweftRustPurity::Pure,
            effects: Vec::new(),
        })
    }

    struct TestProject {
        root: PathBuf,
    }

    impl TestProject {
        fn new(name: &str) -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock")
                .as_nanos();
            let root = std::env::temp_dir().join(format!("{name}-{unique}"));
            create_dir_all(&root).expect("create test project root");
            Self { root }
        }

        fn path(&self, path: &str) -> PathBuf {
            self.root.join(path)
        }

        fn write(&self, path: &str, contents: &str) {
            let path = self.path(path);
            if let Some(parent) = path.parent() {
                create_dir_all(parent).expect("create parent");
            }
            write(path, contents).expect("write fixture");
        }
    }

    impl Drop for TestProject {
        fn drop(&mut self) {
            if self
                .root
                .components()
                .any(|component| matches!(component, std::path::Component::Normal(_)))
            {
                let _ = std::fs::remove_dir_all(&self.root);
            }
        }
    }
}
