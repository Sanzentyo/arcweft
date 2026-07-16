use arcweft_manifest_model::{
    ContentCompression, ContentPlacement, ContentResidency, EntrySelectionId, LaunchKind, ProfileId,
};
use serde::Deserialize;
use std::{
    collections::BTreeMap,
    fmt,
    path::{Path, PathBuf},
};
use thiserror::Error;

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

/// Policy for selecting one profile ID from a launch manifest.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LaunchProfileSelection<'a> {
    /// Select exactly the requested ID without fallback.
    Explicit(&'a str),
    /// Apply manifest-default, previous-profile, then lexical-first precedence.
    Automatic { previous: Option<&'a str> },
}

/// One launch profile entry in the manifest.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct LaunchProfileSpec {
    kind: LaunchKind,
    source: PathBuf,
    entry: EntrySelectionId,
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
    residency: ContentResidency,
    #[serde(default)]
    placement: ContentPlacement,
    #[serde(default)]
    compression: ContentCompression,
}

/// Fully resolved launch profile ready for CLI/runtime use.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedLaunchProfile {
    id: ProfileId,
    kind: LaunchKind,
    source: PathBuf,
    entry: EntrySelectionId,
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
    #[error("launch manifest does not declare any profile")]
    NoProfiles,
    #[error("launch manifest default profile `{profile}` was not found")]
    InvalidDefaultProfile { profile: String },
    #[error("launch profile `{0}` must declare a source path")]
    MissingSource(String),
    #[error("launch profile `{profile}` has an invalid canonical profile ID")]
    InvalidProfileId { profile: String },
    #[error("launch profile `{profile}` uses unknown adapter `{adapter}`")]
    UnknownAdapter { profile: String, adapter: String },
    #[error("launch profile `{profile}` player viewport {field} must be greater than zero")]
    InvalidPlayerViewport {
        profile: String,
        field: &'static str,
    },
}

impl LaunchProfileManifest {
    /// Parses an `arcw.toml` manifest body.
    pub fn parse_toml(source: &str) -> Result<Self, LaunchProfileError> {
        Ok(toml::from_str(source)?)
    }

    /// Selects one declared profile according to an explicit or automatic policy.
    pub fn select_profile_id<'manifest>(
        &'manifest self,
        selection: LaunchProfileSelection<'_>,
    ) -> Result<&'manifest str, LaunchProfileError> {
        match selection {
            LaunchProfileSelection::Explicit(profile) => self
                .profiles
                .get_key_value(profile)
                .map(|(profile, _)| profile.as_str())
                .ok_or_else(|| LaunchProfileError::MissingProfile(profile.to_owned())),
            LaunchProfileSelection::Automatic { previous } => {
                if let Some(profile) = self.default_profile.as_deref() {
                    return self
                        .profiles
                        .get_key_value(profile)
                        .map(|(profile, _)| profile.as_str())
                        .ok_or_else(|| LaunchProfileError::InvalidDefaultProfile {
                            profile: profile.to_owned(),
                        });
                }
                if let Some(profile) = previous
                    && let Some((profile, _)) = self.profiles.get_key_value(profile)
                {
                    return Ok(profile.as_str());
                }
                self.profiles
                    .first_key_value()
                    .map(|(profile, _)| profile.as_str())
                    .ok_or(LaunchProfileError::NoProfiles)
            }
        }
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
            id: ProfileId::new(id).map_err(|_| LaunchProfileError::InvalidProfileId {
                profile: id.to_owned(),
            })?,
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

    /// Exact canonical source entry selected by this profile.
    pub const fn entry(&self) -> &EntrySelectionId {
        &self.entry
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

    /// Exact canonical source entry selected by this profile.
    pub const fn entry(&self) -> &EntrySelectionId {
        &self.entry
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
    pub const fn residency(&self) -> ContentResidency {
        self.residency
    }

    /// Desired bundle placement for this logical content unit.
    pub const fn placement(&self) -> ContentPlacement {
        self.placement
    }

    /// Desired compression for this logical content unit.
    pub const fn compression(&self) -> ContentCompression {
        self.compression
    }
}
