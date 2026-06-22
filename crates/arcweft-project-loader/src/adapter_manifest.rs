use arcweft_adapter_context::{
    codec::{AdapterManifestCodecError, AdapterManifestFile},
    manifest::AdapterManifest,
};
use std::path::Path;
use thiserror::Error;

/// Loads a project-local adapter manifest from TOML or JSON.
pub fn load(path: &Path) -> Result<AdapterManifest, LoadError> {
    let source = std::fs::read_to_string(path).map_err(LoadError::Read)?;
    let file = match path.extension().and_then(|extension| extension.to_str()) {
        Some("json") => AdapterManifestFile::from_json(&source),
        _ => AdapterManifestFile::from_toml(&source),
    }
    .map_err(LoadError::Parse)?;
    Ok(file.into_manifest())
}

/// Adapter manifest load failure without host path decoration.
#[derive(Debug, Error)]
pub enum LoadError {
    #[error("failed to read adapter manifest: {0}")]
    Read(std::io::Error),
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

        let manifest = load(&path).expect("adapter manifest loads");

        assert_eq!(manifest.id().as_str(), "custom-echo");
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

        let manifest = load(&path).expect("adapter manifest loads");

        assert_eq!(manifest.id().as_str(), "json-echo");
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
