//! Sans I/O launch profile model for Arcweft project execution.
//!
//! Launch profiles are the canonical representation of command-specific runtime
//! context. CLI commands may be convenient aliases, but they lower into this
//! data before semantic checking or execution chooses adapter context.

use serde::Deserialize;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};
use thiserror::Error;

/// Stable identifier for a launch profile.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct ProfileId(String);

/// The runtime surface selected by a launch profile.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum LaunchKind {
    Game,
    Server,
    Cli,
    Test,
    Bench,
}

/// Pure helper execution backend selected by a launch profile.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum LaunchPureBackend {
    Auto,
    Vm,
    Aot,
    Jit,
}

/// Matrix/tensor backend selected by a launch profile.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum LaunchMathBackend {
    Auto,
    Scalar,
    Glam,
    Ndarray,
    Wgpu,
}

/// Project-level profile manifest parsed from `arcw.toml`.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
pub struct LaunchProfileManifest {
    #[serde(default)]
    profiles: BTreeMap<String, LaunchProfileSpec>,
}

/// One launch profile entry in the manifest.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct LaunchProfileSpec {
    kind: LaunchKind,
    source: PathBuf,
    #[serde(default)]
    entry: Option<String>,
    #[serde(default)]
    adapter: Option<String>,
    #[serde(default)]
    adapter_manifests: Vec<PathBuf>,
    #[serde(default)]
    listen: Option<String>,
    #[serde(default)]
    pure: Option<LaunchPureProfileSpec>,
    #[serde(default)]
    rust_metadata: Vec<PathBuf>,
}

/// Optional pure-helper execution policy for one launch profile.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
pub struct LaunchPureProfileSpec {
    #[serde(default)]
    backend: Option<LaunchPureBackend>,
    #[serde(default)]
    math_backend: Option<LaunchMathBackend>,
    #[serde(default)]
    math_wgpu_min_elements: Option<usize>,
    #[serde(default)]
    workers: Option<String>,
    #[serde(default)]
    batch_min_len: Option<usize>,
}

/// Fully resolved launch profile ready for CLI/runtime use.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedLaunchProfile {
    id: ProfileId,
    kind: LaunchKind,
    source: PathBuf,
    entry: Option<String>,
    adapter: Option<String>,
    adapter_manifests: Vec<PathBuf>,
    listen: Option<String>,
    pure: Option<LaunchPureProfileSpec>,
    rust_metadata: Vec<PathBuf>,
}

/// Errors from parsing or resolving launch profile data.
#[derive(Debug, Error)]
pub enum LaunchProfileError {
    #[error("failed to parse launch manifest: {0}")]
    Parse(#[from] toml::de::Error),
    #[error("launch profile `{0}` was not found")]
    MissingProfile(String),
    #[error("launch profile `{0}` must declare a source path")]
    MissingSource(String),
    #[error("launch profile `{profile}` uses unknown adapter `{adapter}`")]
    UnknownAdapter { profile: String, adapter: String },
}

impl ProfileId {
    /// Creates a profile ID.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// String form used in manifests and diagnostics.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl LaunchProfileManifest {
    /// Parses an `arcw.toml` manifest body.
    pub fn parse_toml(source: &str) -> Result<Self, LaunchProfileError> {
        Ok(toml::from_str(source)?)
    }

    /// Resolves one profile relative to the manifest directory.
    pub fn resolve_profile(
        &self,
        id: &str,
        manifest_dir: &Path,
    ) -> Result<ResolvedLaunchProfile, LaunchProfileError> {
        self.resolve_profile_with_adapters(id, manifest_dir, &[])
    }

    /// Resolves one profile and rejects adapters outside `known_adapters`.
    pub fn resolve_profile_with_adapters(
        &self,
        id: &str,
        manifest_dir: &Path,
        known_adapters: &[&str],
    ) -> Result<ResolvedLaunchProfile, LaunchProfileError> {
        let spec = self
            .profiles
            .get(id)
            .ok_or_else(|| LaunchProfileError::MissingProfile(id.to_owned()))?;
        if spec.source.as_os_str().is_empty() {
            return Err(LaunchProfileError::MissingSource(id.to_owned()));
        }
        if let Some(adapter) = spec.adapter.as_deref()
            && !known_adapters.is_empty()
            && !known_adapters.contains(&adapter)
            && spec.adapter_manifests.is_empty()
        {
            return Err(LaunchProfileError::UnknownAdapter {
                profile: id.to_owned(),
                adapter: adapter.to_owned(),
            });
        }
        let source = if spec.source.is_absolute() {
            spec.source.clone()
        } else {
            manifest_dir.join(&spec.source)
        };
        let rust_metadata = spec
            .rust_metadata
            .iter()
            .map(|path| {
                if path.is_absolute() {
                    path.clone()
                } else {
                    manifest_dir.join(path)
                }
            })
            .collect();
        let adapter_manifests = spec
            .adapter_manifests
            .iter()
            .map(|path| {
                if path.is_absolute() {
                    path.clone()
                } else {
                    manifest_dir.join(path)
                }
            })
            .collect();
        Ok(ResolvedLaunchProfile {
            id: ProfileId::new(id),
            kind: spec.kind,
            source,
            entry: spec.entry.clone(),
            adapter: spec.adapter.clone(),
            adapter_manifests,
            listen: spec.listen.clone(),
            pure: spec.pure.clone(),
            rust_metadata,
        })
    }

    /// Profiles declared by ID.
    pub fn profiles(&self) -> &BTreeMap<String, LaunchProfileSpec> {
        &self.profiles
    }
}

impl ResolvedLaunchProfile {
    /// Profile ID selected by the user or command alias.
    pub const fn id(&self) -> &ProfileId {
        &self.id
    }

    /// Runtime surface selected by the profile.
    pub const fn kind(&self) -> LaunchKind {
        self.kind
    }

    /// Resolved `.arcw` source path.
    pub fn source(&self) -> &Path {
        &self.source
    }

    /// Optional source entry selector.
    pub fn entry(&self) -> Option<&str> {
        self.entry.as_deref()
    }

    /// Optional adapter selected by the profile.
    pub fn adapter(&self) -> Option<&str> {
        self.adapter.as_deref()
    }

    /// Adapter manifest files selected by this profile.
    pub fn adapter_manifests(&self) -> &[PathBuf] {
        &self.adapter_manifests
    }

    /// Optional listen address selected by the profile.
    pub fn listen(&self) -> Option<&str> {
        self.listen.as_deref()
    }

    /// Optional pure-helper execution policy selected by the profile.
    pub const fn pure(&self) -> Option<&LaunchPureProfileSpec> {
        self.pure.as_ref()
    }

    /// Rust ABI metadata files selected by this profile.
    pub fn rust_metadata(&self) -> &[PathBuf] {
        &self.rust_metadata
    }
}

impl LaunchPureProfileSpec {
    /// Optional pure backend selected by the profile.
    pub const fn backend(&self) -> Option<LaunchPureBackend> {
        self.backend
    }

    /// Optional math backend selected for built-in matrix/tensor operations.
    pub const fn math_backend(&self) -> Option<LaunchMathBackend> {
        self.math_backend
    }

    /// Optional minimum element count before auto math dispatch considers GPU.
    pub const fn math_wgpu_min_elements(&self) -> Option<usize> {
        self.math_wgpu_min_elements
    }

    /// Optional worker-count policy, either `auto` or a positive integer.
    pub fn workers(&self) -> Option<&str> {
        self.workers.as_deref()
    }

    /// Optional minimum item count before batch parallelism is considered.
    pub const fn batch_min_len(&self) -> Option<usize> {
        self.batch_min_len
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_resolves_profiles_relative_to_manifest_dir() {
        let manifest = LaunchProfileManifest::parse_toml(
            r#"
[profiles."server.dev"]
kind = "server"
source = "src/server.arcw"
entry = "http"
adapter = "native-http"
adapter_manifests = ["adapters/http.toml"]
listen = "127.0.0.1:8787"
rust_metadata = ["target/arcweft/truck_game.json"]

[profiles."server.dev".pure]
backend = "jit"
math_backend = "ndarray"
math_wgpu_min_elements = 1024
workers = "auto"
batch_min_len = 2048
"#,
        )
        .expect("manifest parses");

        let resolved = manifest
            .resolve_profile_with_adapters("server.dev", Path::new("game"), &["native-http"])
            .expect("profile resolves");

        assert_eq!(resolved.kind(), LaunchKind::Server);
        assert_eq!(resolved.source(), Path::new("game/src/server.arcw"));
        assert_eq!(resolved.entry(), Some("http"));
        assert_eq!(resolved.adapter(), Some("native-http"));
        assert_eq!(
            resolved.adapter_manifests(),
            &[PathBuf::from("game/adapters/http.toml")]
        );
        assert_eq!(resolved.listen(), Some("127.0.0.1:8787"));
        assert_eq!(
            resolved.rust_metadata(),
            &[PathBuf::from("game/target/arcweft/truck_game.json")]
        );
        let pure = resolved.pure().expect("pure profile resolves");
        assert_eq!(pure.backend(), Some(LaunchPureBackend::Jit));
        assert_eq!(pure.math_backend(), Some(LaunchMathBackend::Ndarray));
        assert_eq!(pure.math_wgpu_min_elements(), Some(1024));
        assert_eq!(pure.workers(), Some("auto"));
        assert_eq!(pure.batch_min_len(), Some(2048));
    }

    #[test]
    fn rejects_missing_profiles_and_unknown_adapters() {
        let manifest = LaunchProfileManifest::parse_toml(
            r#"
[profiles.bad]
kind = "server"
source = "server.arcw"
adapter = "custom-http"

[profiles.custom]
kind = "server"
source = "server.arcw"
adapter = "custom-http"
adapter_manifests = ["adapters/custom-http.toml"]
"#,
        )
        .expect("manifest parses");

        assert!(matches!(
            manifest.resolve_profile("missing", Path::new(".")),
            Err(LaunchProfileError::MissingProfile(id)) if id == "missing"
        ));
        assert!(matches!(
            manifest.resolve_profile_with_adapters("bad", Path::new("."), &["native-http"]),
            Err(LaunchProfileError::UnknownAdapter { profile, adapter })
                if profile == "bad" && adapter == "custom-http"
        ));
        let custom = manifest
            .resolve_profile_with_adapters("custom", Path::new("game"), &["native-http"])
            .expect("profile with custom adapter manifest resolves");
        assert_eq!(custom.adapter(), Some("custom-http"));
        assert_eq!(
            custom.adapter_manifests(),
            &[PathBuf::from("game/adapters/custom-http.toml")]
        );
    }
}
