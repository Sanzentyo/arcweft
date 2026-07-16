use arcweft_adapter_context::{
    codec::{AdapterManifestCodecError, AdapterManifestFile},
    manifest::AdapterManifest,
};
use arcweft_source::{
    SourceDocument, SourceDocumentError, SourceDocumentId, SourceDocumentIdError, SourceName,
};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
use thiserror::Error;

/// A decoded adapter manifest together with the exact document that supplied it.
#[derive(Clone, Debug)]
pub struct LoadedAdapterManifest {
    document: Arc<SourceDocument>,
    path: PathBuf,
    manifest: AdapterManifest,
}

impl LoadedAdapterManifest {
    /// Exact source document used for decoding.
    pub fn document(&self) -> &Arc<SourceDocument> {
        &self.document
    }

    /// Exact declared path used for format dispatch.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Decoded typed adapter manifest.
    pub const fn manifest(&self) -> &AdapterManifest {
        &self.manifest
    }
}

/// Loads a project-local adapter manifest from TOML or JSON.
pub fn load(path: &Path) -> Result<LoadedAdapterManifest, LoadError> {
    let path = path.to_path_buf();
    let source = std::fs::read_to_string(&path).map_err(LoadError::Read)?;
    let document = Arc::new(SourceDocument::try_new(
        SourceDocumentId::try_new(path.to_string_lossy().replace('\\', "/"))?,
        SourceName::path(path.display().to_string()),
        source,
    )?);
    decode(path, document)
}

/// Decodes one adapter manifest from an already captured source document.
pub fn decode(
    path: PathBuf,
    document: Arc<SourceDocument>,
) -> Result<LoadedAdapterManifest, LoadError> {
    let file = match path.extension().and_then(|extension| extension.to_str()) {
        Some("json") => AdapterManifestFile::from_json(document.text()),
        _ => AdapterManifestFile::from_toml(document.text()),
    }
    .map_err(LoadError::Parse)?;
    Ok(LoadedAdapterManifest {
        document,
        path,
        manifest: file.into_manifest().map_err(LoadError::Parse)?,
    })
}

/// Adapter manifest load failure without host path decoration.
#[derive(Debug, Error)]
pub enum LoadError {
    #[error("failed to read adapter manifest: {0}")]
    Read(std::io::Error),
    #[error("invalid adapter manifest document identity: {0}")]
    DocumentId(#[from] SourceDocumentIdError),
    #[error("failed to construct adapter manifest source document: {0}")]
    Document(#[from] SourceDocumentError),
    #[error("failed to parse adapter manifest: {0}")]
    Parse(AdapterManifestCodecError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{create_dir_all, write};
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn loads_toml_adapter_manifest() {
        let project = TestProject::new("loader-adapter-toml");
        let path = project.path("adapter.toml");
        project.write("adapter.toml", adapter_manifest_toml());

        let loaded = load(&path).expect("adapter manifest loads");

        assert_eq!(loaded.manifest().id().as_str(), "custom-echo");
        assert_eq!(loaded.path(), path);
    }

    #[test]
    fn loads_json_adapter_manifest() {
        let project = TestProject::new("loader-adapter-json");
        let path = project.path("adapter.json");
        project.write(
            "adapter.json",
            r#"{
  "schema_version": 1,
  "id": "json-echo",
  "display_name": "JSON Echo",
  "functions": [],
  "host_calls": []
}"#,
        );

        let loaded = load(&path).expect("adapter manifest loads");

        assert_eq!(loaded.manifest().id().as_str(), "json-echo");
    }

    #[test]
    fn decode_uses_the_supplied_document_without_filesystem_access() {
        let path = PathBuf::from("missing/adapter.toml");
        let document = Arc::new(
            SourceDocument::try_new(
                SourceDocumentId::try_new("arcweft-project://fixture/adapter.toml")
                    .expect("document id"),
                SourceName::path("captured-adapter"),
                adapter_manifest_toml(),
            )
            .expect("source document"),
        );

        let loaded = decode(path.clone(), Arc::clone(&document)).expect("document decodes");

        assert!(Arc::ptr_eq(loaded.document(), &document));
        assert_eq!(loaded.path(), path);
        assert_eq!(loaded.manifest().id().as_str(), "custom-echo");
    }

    #[test]
    fn missing_adapter_manifest_reports_read() {
        let project = TestProject::new("loader-adapter-missing");
        let error = load(&project.path("missing.toml")).expect_err("missing file errors");

        assert!(matches!(error, LoadError::Read(_)));
    }

    #[test]
    fn malformed_toml_adapter_manifest_reports_parse() {
        let project = TestProject::new("loader-adapter-bad-toml");
        let path = project.path("adapter.toml");
        project.write("adapter.toml", "schema_version = ");

        let error = load(&path).expect_err("bad toml errors");

        assert!(matches!(error, LoadError::Parse(_)));
    }

    #[test]
    fn malformed_json_adapter_manifest_reports_parse() {
        let project = TestProject::new("loader-adapter-bad-json");
        let path = project.path("adapter.json");
        project.write("adapter.json", "{ not json");

        let error = load(&path).expect_err("bad json errors");

        assert!(matches!(error, LoadError::Parse(_)));
    }

    fn adapter_manifest_toml() -> &'static str {
        r#"
schema_version = 1
id = "custom-echo"
display_name = "Custom Echo"

[[functions]]
name = "custom.echo"
return_type = "String"
params = [{ name = "value", ty = "String" }]

[[host_calls]]
id = "custom.echo"
return_type = "Unit"
"#
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

    fn _path_type_is_used(_: &Path) {}
}
