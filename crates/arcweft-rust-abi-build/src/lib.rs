//! Build-script helpers for writing Arcweft Rust ABI metadata.
//!
//! This crate owns filesystem and Cargo build-script integration so
//! `arcweft-rust-abi` can remain data and codec only.

use arcweft_rust_abi::{ArcweftRustAbiError, ArcweftRustManifest};
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Options used when emitting Rust ABI metadata from a `build.rs`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MetadataBuildOptions {
    out_dir: PathBuf,
    file_stem: String,
    rerun_if_changed: Vec<PathBuf>,
    rerun_if_env_changed: Vec<String>,
}

/// Result of writing one metadata JSON file.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MetadataBuildOutput {
    path: PathBuf,
    changed: bool,
    content_hash: String,
}

/// Error produced by build-script metadata emission.
#[derive(Debug, Error)]
pub enum MetadataBuildError {
    #[error("OUT_DIR is not available for Arcweft Rust ABI metadata build output")]
    MissingOutDir(#[from] std::env::VarError),
    #[error("failed to encode Rust ABI metadata")]
    Abi(#[from] ArcweftRustAbiError),
    #[error("failed to create metadata directory `{path}`: {source}")]
    CreateDir {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to read existing metadata file `{path}`: {source}")]
    ReadExisting {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to write metadata file `{path}`: {source}")]
    Write {
        path: PathBuf,
        source: std::io::Error,
    },
}

impl MetadataBuildOptions {
    /// Creates metadata emission options rooted at Cargo's `OUT_DIR`.
    pub fn new(out_dir: impl Into<PathBuf>, file_stem: impl Into<String>) -> Self {
        Self {
            out_dir: out_dir.into(),
            file_stem: file_stem.into(),
            rerun_if_changed: Vec::new(),
            rerun_if_env_changed: Vec::new(),
        }
    }

    /// Creates metadata emission options from Cargo's `OUT_DIR` environment.
    ///
    /// This is the normal `build.rs` entrypoint. Tests and adapter tooling that
    /// already have an output directory should use [`Self::new`] directly.
    pub fn from_out_dir_env(file_stem: impl Into<String>) -> Result<Self, MetadataBuildError> {
        Ok(Self::new(
            std::env::var_os("OUT_DIR").ok_or(std::env::VarError::NotPresent)?,
            file_stem,
        ))
    }

    /// Adds a `cargo:rerun-if-changed=` input path.
    #[must_use]
    pub fn with_rerun_if_changed(mut self, path: impl Into<PathBuf>) -> Self {
        self.rerun_if_changed.push(path.into());
        self
    }

    /// Adds a `cargo:rerun-if-env-changed=` input variable.
    #[must_use]
    pub fn with_rerun_if_env_changed(mut self, variable: impl Into<String>) -> Self {
        self.rerun_if_env_changed.push(variable.into());
        self
    }

    /// Cargo `OUT_DIR` where metadata will be written under `arcweft/`.
    pub fn out_dir(&self) -> &Path {
        &self.out_dir
    }

    /// File stem used for the generated JSON file.
    pub fn file_stem(&self) -> &str {
        &self.file_stem
    }

    /// Paths emitted as `cargo:rerun-if-changed=`.
    pub fn rerun_if_changed(&self) -> &[PathBuf] {
        &self.rerun_if_changed
    }

    /// Variables emitted as `cargo:rerun-if-env-changed=`.
    pub fn rerun_if_env_changed(&self) -> &[String] {
        &self.rerun_if_env_changed
    }

    fn output_path(&self) -> PathBuf {
        self.out_dir
            .join("arcweft")
            .join(format!("{}.json", self.file_stem))
    }
}

impl MetadataBuildOutput {
    /// Path to the generated JSON file.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Whether this call changed the on-disk file contents.
    pub const fn changed(&self) -> bool {
        self.changed
    }

    /// BLAKE3 hash of the emitted JSON bytes.
    pub fn content_hash(&self) -> &str {
        &self.content_hash
    }
}

/// Writes deterministic JSON metadata and avoids rewriting unchanged output.
pub fn write_manifest(
    manifest: &ArcweftRustManifest,
    options: &MetadataBuildOptions,
) -> Result<MetadataBuildOutput, MetadataBuildError> {
    let json = manifest.to_json_pretty()?;
    let bytes = json.as_bytes();
    let path = options.output_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| MetadataBuildError::CreateDir {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let changed = match fs::read(&path) {
        Ok(existing) if existing == bytes => false,
        Ok(_) => {
            fs::write(&path, bytes).map_err(|source| MetadataBuildError::Write {
                path: path.clone(),
                source,
            })?;
            true
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fs::write(&path, bytes).map_err(|source| MetadataBuildError::Write {
                path: path.clone(),
                source,
            })?;
            true
        }
        Err(source) => {
            return Err(MetadataBuildError::ReadExisting {
                path: path.clone(),
                source,
            });
        }
    };
    Ok(MetadataBuildOutput {
        path,
        changed,
        content_hash: blake3::hash(bytes).to_hex().to_string(),
    })
}

/// Emits Cargo rerun hints from build metadata options.
pub fn emit_cargo_rerun_hints(options: &MetadataBuildOptions) {
    for path in options.rerun_if_changed() {
        println!("cargo:rerun-if-changed={}", path.display());
    }
    for variable in options.rerun_if_env_changed() {
        println!("cargo:rerun-if-env-changed={variable}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_rust_abi::{
        ArcweftRustFunction, ArcweftRustPackage, ArcweftRustPackageId, ArcweftRustParam,
        ArcweftRustPurity, ArcweftRustTypeRef,
    };

    #[test]
    fn writes_manifest_without_absolute_paths_in_json() {
        let dir =
            std::env::temp_dir().join(format!("arcweft-rust-abi-build-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let manifest = ArcweftRustManifest::new(ArcweftRustPackage {
            id: ArcweftRustPackageId::try_new("truck_game").expect("valid package ID"),
            version: "0.1.0".to_owned(),
            metadata_hash: None,
        })
        .with_function(ArcweftRustFunction {
            name: "mini_games.truck.score_to_rank".to_owned(),
            rust_path: "truck_game::score_to_rank".to_owned(),
            params: vec![ArcweftRustParam {
                name: "score".to_owned(),
                ty: ArcweftRustTypeRef::I32,
            }],
            return_type: ArcweftRustTypeRef::I64,
            purity: ArcweftRustPurity::Pure,
            effects: Vec::new(),
        });
        let options = MetadataBuildOptions::new(&dir, "truck_game")
            .with_rerun_if_changed("src/lib.rs")
            .with_rerun_if_env_changed("CARGO_PKG_VERSION");

        let first = write_manifest(&manifest, &options).expect("metadata writes");
        let second = write_manifest(&manifest, &options).expect("unchanged metadata is detected");
        let json = fs::read_to_string(first.path()).expect("metadata is readable");

        assert!(first.changed());
        assert!(!second.changed());
        assert_eq!(first.content_hash(), second.content_hash());
        assert!(!json.contains(&dir.display().to_string()));
        assert!(options.output_path().ends_with("arcweft/truck_game.json"));
        let _ = fs::remove_dir_all(&dir);
    }
}
