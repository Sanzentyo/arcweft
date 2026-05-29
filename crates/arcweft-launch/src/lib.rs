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
    listen: Option<String>,
    #[serde(default)]
    pure: Option<LaunchPureProfileSpec>,
}

/// Optional pure-helper execution policy for one launch profile.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
pub struct LaunchPureProfileSpec {
    #[serde(default)]
    backend: Option<LaunchPureBackend>,
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
    listen: Option<String>,
    pure: Option<LaunchPureProfileSpec>,
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
        Ok(ResolvedLaunchProfile {
            id: ProfileId::new(id),
            kind: spec.kind,
            source,
            entry: spec.entry.clone(),
            adapter: spec.adapter.clone(),
            listen: spec.listen.clone(),
            pure: spec.pure.clone(),
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

    /// Optional listen address selected by the profile.
    pub fn listen(&self) -> Option<&str> {
        self.listen.as_deref()
    }

    /// Optional pure-helper execution policy selected by the profile.
    pub const fn pure(&self) -> Option<&LaunchPureProfileSpec> {
        self.pure.as_ref()
    }
}

impl LaunchPureProfileSpec {
    /// Optional pure backend selected by the profile.
    pub const fn backend(&self) -> Option<LaunchPureBackend> {
        self.backend
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
listen = "127.0.0.1:8787"

[profiles."server.dev".pure]
backend = "jit"
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
        assert_eq!(resolved.listen(), Some("127.0.0.1:8787"));
        let pure = resolved.pure().expect("pure profile resolves");
        assert_eq!(pure.backend(), Some(LaunchPureBackend::Jit));
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
    }
}
