//! Sans I/O launch profile model for Arcweft project execution.
//!
//! Launch profiles are the canonical representation of command-specific runtime
//! context. CLI commands may be convenient aliases, but they lower into this
//! data before semantic checking or execution chooses adapter context.

use serde::Deserialize;
use std::{
    collections::BTreeMap,
    fmt,
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

impl LaunchKind {
    /// Stable manifest spelling for diagnostics and logs.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Game => "game",
            Self::Server => "server",
            Self::Cli => "cli",
            Self::Test => "test",
            Self::Bench => "bench",
        }
    }
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

/// Build-mode source payload policy selected by a launch profile.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum LaunchSourcePolicy {
    None,
    Normalized,
    Full,
}

impl LaunchSourcePolicy {
    /// Stable manifest spelling for diagnostics and logs.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Normalized => "normalized",
            Self::Full => "full",
        }
    }
}

impl fmt::Display for LaunchSourcePolicy {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Build-mode debug payload policy selected by a launch profile.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum LaunchDebugPolicy {
    None,
    LineTables,
    Full,
}

impl LaunchDebugPolicy {
    /// Stable manifest spelling for diagnostics and logs.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::LineTables => "line-tables",
            Self::Full => "full",
        }
    }
}

impl fmt::Display for LaunchDebugPolicy {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Content residency policy selected by a launch profile.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum LaunchContentResidency {
    #[default]
    Startup,
    OnDemand,
}

impl LaunchContentResidency {
    /// Stable manifest spelling for diagnostics and logs.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Startup => "startup",
            Self::OnDemand => "on-demand",
        }
    }
}

impl fmt::Display for LaunchContentResidency {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Content placement policy selected by a launch profile.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum LaunchContentPlacement {
    #[default]
    Embedded,
    External,
}

impl LaunchContentPlacement {
    /// Stable manifest spelling for diagnostics and logs.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Embedded => "embedded",
            Self::External => "external",
        }
    }
}

impl fmt::Display for LaunchContentPlacement {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Content compression policy selected by a launch profile.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum LaunchContentCompression {
    #[default]
    None,
    Zstd,
}

impl LaunchContentCompression {
    /// Stable manifest spelling for diagnostics and logs.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Zstd => "zstd",
        }
    }
}

impl fmt::Display for LaunchContentCompression {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Hot-reload policy selected by a launch profile.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum LaunchHotReloadMode {
    Restart,
    Swap,
}

/// Hot-reload fallback selected by a launch profile.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum LaunchHotReloadFallback {
    Error,
    Restart,
}

/// Hot-reload state-compatibility policy selected by a launch profile.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum LaunchHotReloadStatePolicy {
    Strict,
}

/// Player viewport fit selected by a launch profile.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum LaunchPlayerViewportFit {
    /// Use the host surface coordinates directly.
    Raw,
    /// Preserve aspect ratio and fit the whole design viewport.
    #[default]
    Contain,
    /// Preserve aspect ratio and fill the host surface.
    Cover,
    /// Scale width and height independently to the host surface.
    Stretch,
}

/// Project-level profile manifest parsed from `arcw.toml`.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
pub struct LaunchProfileManifest {
    #[serde(default, rename = "default")]
    default_profile: Option<String>,
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
    dialogue_defaults: Option<String>,
    #[serde(default)]
    character_manifests: Vec<PathBuf>,
    #[serde(default)]
    pure: Option<LaunchPureProfileSpec>,
    #[serde(default)]
    build: LaunchBuildProfileSpec,
    #[serde(default)]
    hot_reload: Option<LaunchHotReloadProfileSpec>,
    #[serde(default)]
    content: BTreeMap<String, LaunchContentProfileSpec>,
    #[serde(default)]
    rust_metadata: Vec<PathBuf>,
    #[serde(default)]
    player: LaunchPlayerProfileSpec,
}

/// Optional player-host defaults for one launch profile.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
pub struct LaunchPlayerProfileSpec {
    #[serde(default)]
    viewport: Option<LaunchPlayerViewportSpec>,
}

/// Optional design viewport and host-fit policy for the player.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub struct LaunchPlayerViewportSpec {
    #[serde(default, alias = "design_width")]
    design_width: Option<u32>,
    #[serde(default, alias = "design_height")]
    design_height: Option<u32>,
    #[serde(default)]
    fit: LaunchPlayerViewportFit,
}

/// Optional build policy for one launch profile.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
pub struct LaunchBuildProfileSpec {
    #[serde(default)]
    incremental: Option<bool>,
    #[serde(default)]
    tree_shake: Option<bool>,
    #[serde(default)]
    debug: Option<LaunchDebugPolicy>,
    #[serde(default)]
    source: Option<LaunchSourcePolicy>,
    #[serde(default)]
    shared_hoist_threshold_bytes: Option<u64>,
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
    #[serde(default)]
    object_artifacts: Option<bool>,
}

/// Optional hot-reload policy for one launch profile.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
pub struct LaunchHotReloadProfileSpec {
    #[serde(default)]
    mode: Option<LaunchHotReloadMode>,
    #[serde(default)]
    fallback: Option<LaunchHotReloadFallback>,
    #[serde(default)]
    state: Option<LaunchHotReloadStatePolicy>,
}

/// Profile-level policy for one logical content unit.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct LaunchContentProfileSpec {
    #[serde(default)]
    residency: LaunchContentResidency,
    #[serde(default)]
    placement: LaunchContentPlacement,
    #[serde(default)]
    compression: LaunchContentCompression,
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
    dialogue_defaults: Option<String>,
    character_manifests: Vec<PathBuf>,
    pure: Option<LaunchPureProfileSpec>,
    build: LaunchBuildProfileSpec,
    hot_reload: Option<LaunchHotReloadProfileSpec>,
    content: BTreeMap<String, LaunchContentProfileSpec>,
    rust_metadata: Vec<PathBuf>,
    player: LaunchPlayerProfileSpec,
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
    #[error("launch profile `{profile}` player viewport {field} must be greater than zero")]
    InvalidPlayerViewport {
        profile: String,
        field: &'static str,
    },
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
        spec.player.validate(id)?;
        let source = if spec.source.is_absolute() {
            spec.source.clone()
        } else {
            manifest_dir.join(&spec.source)
        };
        let character_manifests = spec
            .character_manifests
            .iter()
            .map(|path| {
                if path.is_absolute() {
                    path.clone()
                } else {
                    manifest_dir.join(path)
                }
            })
            .collect();
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
            dialogue_defaults: spec.dialogue_defaults.clone(),
            character_manifests,
            pure: spec.pure.clone(),
            build: spec.build.clone(),
            hot_reload: spec.hot_reload.clone(),
            content: spec.content.clone(),
            rust_metadata,
            player: spec.player.clone(),
        })
    }

    /// Profiles declared by ID.
    pub fn profiles(&self) -> &BTreeMap<String, LaunchProfileSpec> {
        &self.profiles
    }

    /// Profile IDs whose specs select `kind`.
    pub fn profile_ids_with_kind(&self, kind: LaunchKind) -> Vec<String> {
        self.profiles
            .iter()
            .filter(|(_, profile)| profile.kind() == kind)
            .map(|(profile_id, _)| profile_id.clone())
            .collect()
    }

    /// Default profile selected when a command does not pass `--profile`.
    pub fn default_profile(&self) -> Option<&str> {
        self.default_profile.as_deref()
    }
}

impl LaunchProfileSpec {
    /// Runtime surface selected by this profile.
    pub const fn kind(&self) -> LaunchKind {
        self.kind
    }

    /// Build policy selected by this profile.
    pub const fn build(&self) -> &LaunchBuildProfileSpec {
        &self.build
    }

    /// Hot-reload policy selected by this profile.
    pub const fn hot_reload(&self) -> Option<&LaunchHotReloadProfileSpec> {
        self.hot_reload.as_ref()
    }

    /// Content unit packaging policy selected by this profile.
    pub fn content(&self) -> &BTreeMap<String, LaunchContentProfileSpec> {
        &self.content
    }

    /// Player-host defaults selected by this profile.
    pub const fn player(&self) -> &LaunchPlayerProfileSpec {
        &self.player
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

    /// Optional dialogue defaults profile selected by this launch profile.
    pub fn dialogue_defaults(&self) -> Option<&str> {
        self.dialogue_defaults.as_deref()
    }

    /// Character package manifests selected by this launch profile.
    pub fn character_manifests(&self) -> &[PathBuf] {
        &self.character_manifests
    }

    /// Optional pure-helper execution policy selected by the profile.
    pub const fn pure(&self) -> Option<&LaunchPureProfileSpec> {
        self.pure.as_ref()
    }

    /// Build policy selected by the profile.
    pub const fn build(&self) -> &LaunchBuildProfileSpec {
        &self.build
    }

    /// Hot-reload policy selected by the profile.
    pub const fn hot_reload(&self) -> Option<&LaunchHotReloadProfileSpec> {
        self.hot_reload.as_ref()
    }

    /// Content unit packaging policy selected by the profile.
    pub fn content(&self) -> &BTreeMap<String, LaunchContentProfileSpec> {
        &self.content
    }

    /// Rust ABI metadata files selected by this profile.
    pub fn rust_metadata(&self) -> &[PathBuf] {
        &self.rust_metadata
    }

    /// Player-host defaults selected by the profile.
    pub const fn player(&self) -> &LaunchPlayerProfileSpec {
        &self.player
    }
}

impl LaunchPlayerProfileSpec {
    /// Optional player viewport default.
    pub const fn viewport(&self) -> Option<LaunchPlayerViewportSpec> {
        self.viewport
    }

    fn validate(&self, profile: &str) -> Result<(), LaunchProfileError> {
        let Some(viewport) = self.viewport else {
            return Ok(());
        };
        if viewport.fit == LaunchPlayerViewportFit::Raw {
            return Ok(());
        }
        if viewport.design_width.unwrap_or(1280) == 0 {
            return Err(LaunchProfileError::InvalidPlayerViewport {
                profile: profile.to_owned(),
                field: "design-width",
            });
        }
        if viewport.design_height.unwrap_or(720) == 0 {
            return Err(LaunchProfileError::InvalidPlayerViewport {
                profile: profile.to_owned(),
                field: "design-height",
            });
        }
        Ok(())
    }
}

impl LaunchPlayerViewportSpec {
    /// Design viewport width in logical pixels.
    pub const fn design_width(self) -> u32 {
        match self.design_width {
            Some(width) => width,
            None => 1280,
        }
    }

    /// Design viewport height in logical pixels.
    pub const fn design_height(self) -> u32 {
        match self.design_height {
            Some(height) => height,
            None => 720,
        }
    }

    /// Host fit policy for the design viewport.
    pub const fn fit(self) -> LaunchPlayerViewportFit {
        self.fit
    }
}

impl LaunchBuildProfileSpec {
    /// Optional incremental compilation policy.
    pub const fn incremental(&self) -> Option<bool> {
        self.incremental
    }

    /// Optional tree-shaking policy.
    pub const fn tree_shake(&self) -> Option<bool> {
        self.tree_shake
    }

    /// Optional debug payload policy.
    pub const fn debug(&self) -> Option<LaunchDebugPolicy> {
        self.debug
    }

    /// Optional source payload policy.
    pub const fn source(&self) -> Option<LaunchSourcePolicy> {
        self.source
    }

    /// Optional shared-pack hoisting threshold in decoded bytes.
    pub const fn shared_hoist_threshold_bytes(&self) -> Option<u64> {
        self.shared_hoist_threshold_bytes
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

    /// Optional build-time AOT object artifact emission policy.
    pub const fn object_artifacts(&self) -> Option<bool> {
        self.object_artifacts
    }
}

impl LaunchHotReloadProfileSpec {
    /// Optional hot-reload mode policy.
    pub const fn mode(&self) -> Option<LaunchHotReloadMode> {
        self.mode
    }

    /// Optional hot-reload fallback policy.
    pub const fn fallback(&self) -> Option<LaunchHotReloadFallback> {
        self.fallback
    }

    /// Optional state compatibility policy.
    pub const fn state(&self) -> Option<LaunchHotReloadStatePolicy> {
        self.state
    }
}

impl LaunchContentProfileSpec {
    /// Desired content residency for this logical content unit.
    pub const fn residency(&self) -> LaunchContentResidency {
        self.residency
    }

    /// Desired bundle placement for this logical content unit.
    pub const fn placement(&self) -> LaunchContentPlacement {
        self.placement
    }

    /// Desired compression for this logical content unit.
    pub const fn compression(&self) -> LaunchContentCompression {
        self.compression
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_resolves_profiles_relative_to_manifest_dir() {
        let manifest = LaunchProfileManifest::parse_toml(
            r#"
default = "server.dev"

[profiles."server.dev"]
kind = "server"
source = "src/server.arcw"
entry = "http"
adapter = "native-http"
adapter_manifests = ["adapters/http.toml"]
listen = "127.0.0.1:8787"
dialogue_defaults = "dialogue.defaults.mobile"
rust_metadata = ["target/arcweft/truck_game.json"]

[profiles."server.dev".pure]
backend = "jit"
math_backend = "ndarray"
math_wgpu_min_elements = 1024
workers = "auto"
batch_min_len = 2048
object_artifacts = true
"#,
        )
        .expect("manifest parses");

        assert_eq!(manifest.default_profile(), Some("server.dev"));
        assert_eq!(LaunchKind::Server.as_str(), "server");
        assert_eq!(
            manifest
                .profiles()
                .get("server.dev")
                .map(LaunchProfileSpec::kind),
            Some(LaunchKind::Server)
        );
        assert_eq!(
            manifest.profile_ids_with_kind(LaunchKind::Server),
            vec!["server.dev".to_owned()]
        );

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
            resolved.dialogue_defaults(),
            Some("dialogue.defaults.mobile")
        );
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
        assert_eq!(pure.object_artifacts(), Some(true));
    }

    #[test]
    fn parses_build_hot_reload_and_content_profile_policy() {
        let manifest = LaunchProfileManifest::parse_toml(
            r#"
[profiles.release]
kind = "game"
source = "src/main.arcw"
entry = "entry.main"

[profiles.release.build]
incremental = true
tree_shake = true
debug = "line-tables"
source = "none"
shared_hoist_threshold_bytes = 65536

[profiles.release.hot_reload]
mode = "swap"
fallback = "restart"
state = "strict"

[profiles.release.player.viewport]
design-width = 1280
design-height = 720
fit = "contain"

[profiles.release.content."content.chapter_two"]
residency = "on-demand"
placement = "external"
compression = "zstd"

[profiles.desktop]
kind = "game"
source = "src/main.arcw"

[profiles.desktop.content."content.chapter_two"]
residency = "on-demand"
placement = "embedded"
"#,
        )
        .expect("manifest parses");

        let release = manifest
            .resolve_profile("release", Path::new("game"))
            .expect("release profile resolves");
        let build = release.build();
        assert_eq!(build.incremental(), Some(true));
        assert_eq!(build.tree_shake(), Some(true));
        assert_eq!(build.debug(), Some(LaunchDebugPolicy::LineTables));
        assert_eq!(build.source(), Some(LaunchSourcePolicy::None));
        assert_eq!(build.shared_hoist_threshold_bytes(), Some(65_536));
        assert_eq!(LaunchDebugPolicy::LineTables.to_string(), "line-tables");
        assert_eq!(LaunchSourcePolicy::None.to_string(), "none");

        let hot_reload = release.hot_reload().expect("hot reload policy");
        assert_eq!(hot_reload.mode(), Some(LaunchHotReloadMode::Swap));
        assert_eq!(
            hot_reload.fallback(),
            Some(LaunchHotReloadFallback::Restart)
        );
        assert_eq!(hot_reload.state(), Some(LaunchHotReloadStatePolicy::Strict));
        let viewport = release.player().viewport().expect("player viewport policy");
        assert_eq!(viewport.design_width(), 1280);
        assert_eq!(viewport.design_height(), 720);
        assert_eq!(viewport.fit(), LaunchPlayerViewportFit::Contain);

        let content = release
            .content()
            .get("content.chapter_two")
            .expect("content policy");
        assert_eq!(content.residency(), LaunchContentResidency::OnDemand);
        assert_eq!(content.placement(), LaunchContentPlacement::External);
        assert_eq!(content.compression(), LaunchContentCompression::Zstd);
        assert_eq!(LaunchContentResidency::OnDemand.to_string(), "on-demand");
        assert_eq!(LaunchContentPlacement::External.to_string(), "external");
        assert_eq!(LaunchContentCompression::Zstd.to_string(), "zstd");

        let desktop = manifest
            .resolve_profile("desktop", Path::new("game"))
            .expect("desktop profile resolves");
        let desktop_content = desktop
            .content()
            .get("content.chapter_two")
            .expect("desktop content policy");
        assert_eq!(
            desktop_content.residency(),
            LaunchContentResidency::OnDemand
        );
        assert_eq!(
            desktop_content.placement(),
            LaunchContentPlacement::Embedded
        );
        assert_eq!(
            desktop_content.compression(),
            LaunchContentCompression::None
        );
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

    #[test]
    fn rejects_zero_sized_player_design_viewport() {
        let manifest = LaunchProfileManifest::parse_toml(
            r#"
[profiles.bad]
kind = "game"
source = "game.arcw"

[profiles.bad.player.viewport]
design-width = 0
design-height = 720
fit = "contain"
"#,
        )
        .expect("manifest parses");

        assert!(matches!(
            manifest.resolve_profile("bad", Path::new(".")),
            Err(LaunchProfileError::InvalidPlayerViewport { profile, field })
                if profile == "bad" && field == "design-width"
        ));
    }
}
