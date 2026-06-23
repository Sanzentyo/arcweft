use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use std::path::{Component, Path, PathBuf};
use thiserror::Error;

/// Cargo-like project metadata parsed from `arcw.toml`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProjectManifest {
    package: PackageManifest,
    #[serde(default)]
    build: BuildManifest,
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
}

impl ProjectManifest {
    /// Parses and validates an `arcw.toml` project section.
    pub fn parse_toml(source: &str) -> Result<Self, ProjectManifestError> {
        let manifest = toml::from_str::<Self>(source)?;
        manifest.build.validate()?;
        Ok(manifest)
    }

    pub const fn package(&self) -> &PackageManifest {
        &self.package
    }

    pub const fn build(&self) -> &BuildManifest {
        &self.build
    }

    /// Absolute or caller-rooted source directory for this project.
    pub fn source_root(&self, project_root: &Path) -> PathBuf {
        project_root.join(self.build.source_dir())
    }

    /// Absolute or caller-rooted artifact directory for this project.
    pub fn target_root(&self, project_root: &Path) -> PathBuf {
        project_root.join(self.build.target_dir())
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
    let invalid = path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        });
    if invalid {
        Err(ProjectManifestError::InvalidBuildPath {
            field,
            path: path.to_path_buf(),
        })
    } else {
        Ok(())
    }
}

fn default_source_dir() -> PathBuf {
    PathBuf::from("src")
}

fn default_target_dir() -> PathBuf {
    PathBuf::from("target/arcweft")
}

const fn default_incremental() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::ProjectManifest;
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
source = "game/main.arcw"
"#,
        )
        .unwrap();
        assert_eq!(manifest.package().name().as_str(), "opening-game");
        assert_eq!(manifest.build().source_dir(), Path::new("game"));
        assert_eq!(manifest.build().target_dir(), Path::new("target/arcweft"));
        assert!(manifest.build().incremental());
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
}
