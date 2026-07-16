use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use std::path::{Component, Path, PathBuf};
use thiserror::Error;

/// Cargo-like project metadata parsed from `arcw.toml`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProjectManifest {
    package: PackageManifest,
    #[serde(default)]
    build: BuildManifest,
    #[serde(default)]
    resources: ResourceManifest,
}

/// Package identity used in diagnostics, metadata, and target paths.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PackageManifest {
    name: PackageName,
}

/// Filesystem-independent build path configuration.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct BuildManifest {
    #[serde(default = "default_source_dir")]
    source_dir: PathBuf,
    #[serde(default = "default_target_dir")]
    target_dir: PathBuf,
    #[serde(default = "default_incremental")]
    incremental: bool,
}

/// Project-relative authoring input directories.
///
/// These directories contain versionable project inputs. Tool-owned caches and
/// mutable runtime files remain outside this contract.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ResourceManifest {
    #[serde(default = "default_asset_dir")]
    asset_dir: PathBuf,
    #[serde(default = "default_content_dir")]
    content_dir: PathBuf,
}

/// Resolved filesystem roots for authored asset and structured content inputs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthoredResourceRoots {
    asset: PathBuf,
    content: PathBuf,
}

#[derive(Deserialize)]
struct ResourceManifestDocument {
    #[serde(default)]
    resources: ResourceManifest,
}

/// Validated package identifier.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PackageName(String);

/// Package-name validation failure.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum PackageNameError {
    #[error("package name is empty")]
    Empty,
    #[error("package name `{value}` contains unsupported characters")]
    Invalid { value: String },
}

/// Project manifest parse or validation failure.
#[derive(Debug, Error)]
pub enum ProjectManifestError {
    #[error("failed to parse Arcweft project manifest: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("build field `{field}` must be a project-relative path without `..`: `{path}`")]
    InvalidBuildPath { field: &'static str, path: PathBuf },
    #[error(
        "resources field `{field}` must be a non-empty normalized project-relative path: `{path}`"
    )]
    InvalidResourcePath { field: &'static str, path: PathBuf },
    #[error(
        "resources roots must be disjoint, but asset-dir `{asset_dir}` and content-dir `{content_dir}` overlap"
    )]
    OverlappingResourcePaths {
        asset_dir: PathBuf,
        content_dir: PathBuf,
    },
}

impl ProjectManifest {
    /// Parses and validates an `arcw.toml` project section.
    pub fn parse_toml(source: &str) -> Result<Self, ProjectManifestError> {
        let manifest = toml::from_str::<Self>(source)?;
        manifest.build.validate()?;
        manifest.resources.validate()?;
        Ok(manifest)
    }

    pub const fn package(&self) -> &PackageManifest {
        &self.package
    }

    pub const fn build(&self) -> &BuildManifest {
        &self.build
    }

    pub const fn resources(&self) -> &ResourceManifest {
        &self.resources
    }

    /// Absolute or caller-rooted source directory for this project.
    pub fn source_root(&self, project_root: &Path) -> PathBuf {
        project_root.join(self.build.source_dir())
    }

    /// Absolute or caller-rooted artifact directory for this project.
    pub fn target_root(&self, project_root: &Path) -> PathBuf {
        project_root.join(self.build.target_dir())
    }

    /// Resolves authored input roots relative to this project's manifest.
    pub fn authored_resource_roots(&self, project_root: &Path) -> AuthoredResourceRoots {
        self.resources.resolve(project_root)
    }
}

impl PackageManifest {
    pub const fn name(&self) -> &PackageName {
        &self.name
    }
}

impl Default for BuildManifest {
    fn default() -> Self {
        Self {
            source_dir: default_source_dir(),
            target_dir: default_target_dir(),
            incremental: default_incremental(),
        }
    }
}

impl BuildManifest {
    pub fn source_dir(&self) -> &Path {
        &self.source_dir
    }

    pub fn target_dir(&self) -> &Path {
        &self.target_dir
    }

    pub const fn incremental(&self) -> bool {
        self.incremental
    }

    fn validate(&self) -> Result<(), ProjectManifestError> {
        validate_relative_path("source-dir", &self.source_dir)?;
        validate_relative_path("target-dir", &self.target_dir)
    }
}

impl Default for ResourceManifest {
    fn default() -> Self {
        Self {
            asset_dir: default_asset_dir(),
            content_dir: default_content_dir(),
        }
    }
}

impl ResourceManifest {
    /// Parses only the project-level `[resources]` section from `arcw.toml`.
    ///
    /// Launch-only manifests do not need a `[package]` table in order to use
    /// the same authored resource root contract.
    pub fn parse_project_toml(source: &str) -> Result<Self, ProjectManifestError> {
        let document = toml::from_str::<ResourceManifestDocument>(source)?;
        document.resources.validate()?;
        Ok(document.resources)
    }

    pub fn asset_dir(&self) -> &Path {
        &self.asset_dir
    }

    pub fn content_dir(&self) -> &Path {
        &self.content_dir
    }

    /// Resolves project-relative input directories without performing I/O.
    pub fn resolve(&self, project_root: &Path) -> AuthoredResourceRoots {
        AuthoredResourceRoots::new(
            project_root.join(self.asset_dir()),
            project_root.join(self.content_dir()),
        )
    }

    fn validate(&self) -> Result<(), ProjectManifestError> {
        validate_resource_path("asset-dir", &self.asset_dir)?;
        validate_resource_path("content-dir", &self.content_dir)?;
        if resource_roots_overlap(&self.asset_dir, &self.content_dir) {
            return Err(ProjectManifestError::OverlappingResourcePaths {
                asset_dir: self.asset_dir.clone(),
                content_dir: self.content_dir.clone(),
            });
        }
        Ok(())
    }
}

impl AuthoredResourceRoots {
    pub fn new(asset: impl Into<PathBuf>, content: impl Into<PathBuf>) -> Self {
        Self {
            asset: asset.into(),
            content: content.into(),
        }
    }

    pub fn asset(&self) -> &Path {
        &self.asset
    }

    pub fn content(&self) -> &Path {
        &self.content
    }
}

impl PackageName {
    /// Creates a package name accepted by Arcweft project metadata.
    pub fn new(value: impl Into<String>) -> Result<Self, PackageNameError> {
        let value = value.into();
        if value.is_empty() {
            return Err(PackageNameError::Empty);
        }
        if !value
            .chars()
            .all(|character| character.is_alphanumeric() || matches!(character, '-' | '_'))
        {
            return Err(PackageNameError::Invalid { value });
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Serialize for PackageName {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for PackageName {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(de::Error::custom)
    }
}

fn validate_relative_path(field: &'static str, path: &Path) -> Result<(), ProjectManifestError> {
    if path_escapes_project(path) {
        Err(ProjectManifestError::InvalidBuildPath {
            field,
            path: path.to_path_buf(),
        })
    } else {
        Ok(())
    }
}

fn validate_resource_path(field: &'static str, path: &Path) -> Result<(), ProjectManifestError> {
    if path.as_os_str().is_empty()
        || path_escapes_project(path)
        || path_has_current_dir_component(path)
    {
        Err(ProjectManifestError::InvalidResourcePath {
            field,
            path: path.to_path_buf(),
        })
    } else {
        Ok(())
    }
}

fn path_has_current_dir_component(path: &Path) -> bool {
    path.to_string_lossy()
        .split(['/', '\\'])
        .any(|segment| segment == ".")
}

fn resource_roots_overlap(asset_dir: &Path, content_dir: &Path) -> bool {
    portable_path_starts_with(asset_dir, content_dir)
        || portable_path_starts_with(content_dir, asset_dir)
}

fn portable_path_starts_with(path: &Path, base: &Path) -> bool {
    let mut path_components = path.components();
    base.components().all(|base_component| {
        path_components.next().is_some_and(|path_component| {
            path_component
                .as_os_str()
                .to_string_lossy()
                .eq_ignore_ascii_case(&base_component.as_os_str().to_string_lossy())
        })
    })
}

fn path_escapes_project(path: &Path) -> bool {
    path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
}

fn default_source_dir() -> PathBuf {
    PathBuf::from("src")
}

fn default_target_dir() -> PathBuf {
    PathBuf::from("target/arcweft")
}

fn default_asset_dir() -> PathBuf {
    PathBuf::from("assets")
}

fn default_content_dir() -> PathBuf {
    PathBuf::from("content")
}

const fn default_incremental() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::{ProjectManifest, ResourceManifest};
    use std::path::Path;

    #[test]
    fn parses_project_and_ignores_launch_profile_tables() {
        let manifest = ProjectManifest::parse_toml(
            r#"
[package]
name = "opening-game"

[build]
source-dir = "game"

[profiles.dev]
kind = "game"
entry = "entry.game.main"
source = "game/main.arcw"
"#,
        )
        .unwrap();
        assert_eq!(manifest.package().name().as_str(), "opening-game");
        assert_eq!(manifest.build().source_dir(), Path::new("game"));
        assert_eq!(manifest.build().target_dir(), Path::new("target/arcweft"));
        assert!(manifest.build().incremental());
        assert_eq!(manifest.resources().asset_dir(), Path::new("assets"));
        assert_eq!(manifest.resources().content_dir(), Path::new("content"));
    }

    #[test]
    fn rejects_build_paths_that_escape_the_project() {
        let error = ProjectManifest::parse_toml(
            r#"
[package]
name = "opening-game"

[build]
source-dir = "../outside"
"#,
        )
        .unwrap_err();
        assert!(error.to_string().contains("source-dir"));
    }

    #[test]
    fn resolves_custom_authored_resource_roots() {
        let manifest = ProjectManifest::parse_toml(
            r#"
[package]
name = "opening-game"

[resources]
asset-dir = "game-assets"
content-dir = "game-content"
"#,
        )
        .unwrap();

        let roots = manifest.authored_resource_roots(Path::new("project"));
        assert_eq!(roots.asset(), Path::new("project/game-assets"));
        assert_eq!(roots.content(), Path::new("project/game-content"));
    }

    #[test]
    fn launch_only_manifest_can_resolve_default_resource_roots() {
        let resources = ResourceManifest::parse_project_toml(
            r#"
[profiles.game]
kind = "game"
entry = "entry.game.main"
source = "main.arcw"
"#,
        )
        .unwrap();

        let roots = resources.resolve(Path::new("project"));
        assert_eq!(roots.asset(), Path::new("project/assets"));
        assert_eq!(roots.content(), Path::new("project/content"));
    }

    #[test]
    fn rejects_resource_paths_that_escape_the_project() {
        let error = ResourceManifest::parse_project_toml(
            r#"
[resources]
asset-dir = "../shared-assets"
"#,
        )
        .unwrap_err();

        assert!(error.to_string().contains("asset-dir"));
    }

    #[test]
    fn rejects_empty_or_non_normalized_resource_paths() {
        for path in ["", ".", "assets/./images"] {
            let source = format!(
                r#"
[package]
name = "opening-game"

[resources]
asset-dir = "{path}"
"#
            );
            let error = ProjectManifest::parse_toml(&source).unwrap_err();
            assert!(error.to_string().contains("asset-dir"));
        }
    }

    #[test]
    fn rejects_overlapping_resource_roots_portably() {
        for (asset_dir, content_dir) in [
            ("resources", "resources/content"),
            ("game/assets", "GAME/ASSETS"),
        ] {
            let source = format!(
                r#"
[package]
name = "opening-game"

[resources]
asset-dir = "{asset_dir}"
content-dir = "{content_dir}"
"#
            );
            let error = ProjectManifest::parse_toml(&source).unwrap_err();
            assert!(error.to_string().contains("overlap"));
        }
    }
}
