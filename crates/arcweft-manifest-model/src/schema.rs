use crate::{
    ActivityId, ActivityImplementationId, AdapterExportId, ExternalModuleId,
    ExternalModuleImportId, ModuleMountPath, NormalizedProjectPath, PackageId, PackageVersion,
    RawDigest, SemanticDigest,
};
use serde::{Deserialize, Deserializer, Serialize, de};
use std::fmt;
use thiserror::Error;

/// Manifest schema number. Only the final schema 1 is accepted.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct ManifestSchemaVersion(u32);

#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("unsupported Arcweft manifest schema {found}; expected 1")]
pub struct ManifestSchemaVersionError {
    found: u32,
}

impl ManifestSchemaVersion {
    pub const V1: Self = Self(1);

    pub fn new(found: u32) -> Result<Self, ManifestSchemaVersionError> {
        if found == 1 {
            Ok(Self::V1)
        } else {
            Err(ManifestSchemaVersionError { found })
        }
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

impl<'de> Deserialize<'de> for ManifestSchemaVersion {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::new(u32::deserialize(deserializer)?).map_err(de::Error::custom)
    }
}

/// Required package identity and exact version.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackageSpec {
    pub id: PackageId,
    pub version: PackageVersion,
}

/// Filesystem-independent build defaults.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct BuildSpec {
    #[serde(default = "default_source_dir")]
    pub source_dir: NormalizedProjectPath,
    #[serde(default = "default_target_dir")]
    pub target_dir: NormalizedProjectPath,
    #[serde(default = "default_incremental")]
    pub incremental: bool,
}

impl Default for BuildSpec {
    fn default() -> Self {
        Self {
            source_dir: default_source_dir(),
            target_dir: default_target_dir(),
            incremental: default_incremental(),
        }
    }
}

/// Non-empty ordered collection used for fields whose absence is invalid.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct NonEmptyVec<T>(Vec<T>);

impl<T> NonEmptyVec<T> {
    pub fn new(values: Vec<T>) -> Option<Self> {
        (!values.is_empty()).then_some(Self(values))
    }

    pub fn as_slice(&self) -> &[T] {
        &self.0
    }

    pub fn into_vec(self) -> Vec<T> {
        self.0
    }
}

impl<'de, T> Deserialize<'de> for NonEmptyVec<T>
where
    T: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let values = Vec::deserialize(deserializer)?;
        Self::new(values).ok_or_else(|| de::Error::custom("array must not be empty"))
    }
}

/// Syntactically valid, unresolved Arcweft entity reference.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct EntityIdRef(Box<str>);

impl EntityIdRef {
    pub fn new(value: impl Into<Box<str>>) -> Result<Self, &'static str> {
        let value = value.into();
        if value.starts_with('@') && value.len() > 1 && !value.chars().any(char::is_whitespace) {
            Ok(Self(value))
        } else {
            Err("entity reference must be a non-empty `@` reference without whitespace")
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for EntityIdRef {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::new(String::deserialize(deserializer)?).map_err(de::Error::custom)
    }
}

/// Content root reference. Symbol family resolution belongs to project sema.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ContentRootRef(pub EntityIdRef);

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContentUnitSpec {
    pub roots: NonEmptyVec<ContentRootRef>,
    pub visibility: ManifestVisibility,
    pub demand: DependencyDemand,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct ExternalModuleImportSpec {
    pub mount: ModuleMountPath,
    pub metadata: NormalizedProjectPath,
    pub metadata_hash: RawDigest,
    pub expected_package: PackageId,
    pub expected_version: PackageVersion,
    pub expected_module: ExternalModuleId,
    pub expected_family: AdapterFamily,
    pub expected_abi_hash: SemanticDigest,
    pub visibility: ManifestVisibility,
    pub demand: DependencyDemand,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActivityImplementationSpec {
    pub module: ExternalModuleImportId,
    pub export: AdapterExportId,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActivityBindingSpec {
    pub activity: ActivityId,
    pub implementation: ActivityImplementationId,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfileContentSpec {
    pub residency: ContentResidency,
    pub placement: ContentPlacement,
    pub compression: ContentCompression,
}

macro_rules! kebab_enum {
    ($name:ident { $($variant:ident),+ $(,)? }) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
        #[serde(rename_all = "kebab-case")]
        pub enum $name { $($variant),+ }
    };
}

kebab_enum!(ManifestVisibility {
    Private,
    Package,
    Public
});
kebab_enum!(DependencyDemand { Required, Optional });
kebab_enum!(AdapterFamily {
    Rust,
    Wasm,
    Process
});
#[derive(
    Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize,
)]
#[serde(rename_all = "kebab-case")]
pub enum ContentResidency {
    #[default]
    Startup,
    OnDemand,
}

#[derive(
    Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize,
)]
#[serde(rename_all = "kebab-case")]
pub enum ContentPlacement {
    #[default]
    Embedded,
    External,
}

#[derive(
    Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize,
)]
#[serde(rename_all = "kebab-case")]
pub enum ContentCompression {
    #[default]
    None,
    Zstd,
}
kebab_enum!(LaunchKind {
    Game,
    Server,
    Cli,
    Test,
    Bench
});

impl LaunchKind {
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

impl ContentResidency {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Startup => "startup",
            Self::OnDemand => "on-demand",
        }
    }
}

impl fmt::Display for ContentResidency {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl ContentPlacement {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Embedded => "embedded",
            Self::External => "external",
        }
    }
}

impl fmt::Display for ContentPlacement {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl ContentCompression {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Zstd => "zstd",
        }
    }
}

impl fmt::Display for ContentCompression {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

fn default_source_dir() -> NormalizedProjectPath {
    NormalizedProjectPath::new("src").expect("static source directory is valid")
}

fn default_target_dir() -> NormalizedProjectPath {
    NormalizedProjectPath::new("target/arcweft").expect("static target directory is valid")
}

const fn default_incremental() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::{BuildSpec, ManifestSchemaVersion};

    #[test]
    fn schema_one_is_the_only_supported_schema() {
        assert_eq!(ManifestSchemaVersion::new(1).unwrap().get(), 1);
        assert!(ManifestSchemaVersion::new(0).is_err());
        assert!(ManifestSchemaVersion::new(2).is_err());
    }

    #[test]
    fn build_defaults_are_final_normalized_paths() {
        let build = BuildSpec::default();
        assert_eq!(build.source_dir.as_str(), "src");
        assert_eq!(build.target_dir.as_str(), "target/arcweft");
        assert!(build.incremental);
    }
}
