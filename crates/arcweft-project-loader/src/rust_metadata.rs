use arcweft_rust_abi::{ArcweftRustAbiError, ArcweftRustManifest};
use std::path::Path;
use thiserror::Error;

/// Loads Arcweft Rust ABI metadata from JSON.
pub fn load(path: &Path) -> Result<ArcweftRustManifest, LoadError> {
    let source = std::fs::read_to_string(path).map_err(LoadError::Read)?;
    ArcweftRustManifest::from_json(&source).map_err(LoadError::Parse)
}

/// Rust metadata load failure without host path decoration.
#[derive(Debug, Error)]
pub enum LoadError {
    #[error("failed to read Rust ABI metadata: {0}")]
    Read(std::io::Error),
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

        let manifest = load(&path).expect("rust metadata loads");

        assert_eq!(manifest.package.name, "custom_adapter");
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
