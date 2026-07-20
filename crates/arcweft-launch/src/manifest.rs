//! Final schema-1 manifest records, private until the one-reader cut switches consumers.

use arcweft_dialogue::InlineFailurePolicy;
use arcweft_manifest_model::{
    ActivityBindingSpec, ActivityImplementationId, ActivityImplementationSpec, AdapterProfileId,
    BuildSpec, ContentUnitId, ContentUnitSpec, EntityIdRef, ExternalModuleImportId,
    ExternalModuleImportSpec, LaunchKind, ManifestSchemaVersion, NormalizedProjectPath,
    PackageSpec, ProfileContentSpec, ProfileId, ProfileLocalizationSpec,
};
use arcweft_view::{ViewId, ViewStyleSheetId};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use std::{collections::BTreeMap, net::SocketAddr, num::NonZeroU32, str::FromStr};

/// The only accepted authored manifest shape after the atomic reader switch.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct ArcweftManifestDocument {
    pub(crate) schema: ManifestSchemaVersion,
    pub(crate) package: PackageSpec,
    #[serde(default)]
    pub(crate) build: BuildSpec,
    #[serde(default)]
    pub(crate) content_units: BTreeMap<ContentUnitId, ContentUnitSpec>,
    #[serde(default)]
    pub(crate) external_modules: BTreeMap<ExternalModuleImportId, ExternalModuleImportSpec>,
    #[serde(default)]
    pub(crate) activity_implementations:
        BTreeMap<ActivityImplementationId, ActivityImplementationSpec>,
    #[serde(default)]
    pub(crate) default_profile: Option<ProfileId>,
    #[serde(default)]
    pub(crate) profiles: BTreeMap<ProfileId, ProfileSpec>,
}

impl ArcweftManifestDocument {
    pub const fn schema(&self) -> ManifestSchemaVersion {
        self.schema
    }

    pub const fn package(&self) -> &PackageSpec {
        &self.package
    }

    pub const fn build(&self) -> &BuildSpec {
        &self.build
    }
}

/// Strict authored launch facts for one selected profile.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub(crate) struct ProfileSpec {
    pub(crate) kind: LaunchKind,
    pub(crate) source: NormalizedProjectPath,
    #[serde(default)]
    pub(crate) entry: Option<EntityIdRef>,
    #[serde(default)]
    pub(crate) adapter: Option<AdapterProfileId>,
    #[serde(default)]
    pub(crate) external_modules: Vec<ExternalModuleImportId>,
    #[serde(default)]
    pub(crate) activity_bindings: Vec<ActivityBindingSpec>,
    #[serde(default)]
    pub(crate) dialogue: DialogueProfileSpec,
    #[serde(default)]
    pub(crate) localization: ProfileLocalizationSpec,
    #[serde(default)]
    pub(crate) listen: Option<LaunchListenAddress>,
    #[serde(default)]
    pub(crate) pure: Option<LaunchPureProfileSpec>,
    #[serde(default)]
    pub(crate) content: BTreeMap<ContentUnitId, ProfileContentSpec>,
    #[serde(default)]
    pub(crate) player: LaunchPlayerProfileSpec,
}

/// Dialogue runtime policy selected by one profile.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub(crate) struct DialogueProfileSpec {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) view: Option<ViewId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) style: Option<ViewStyleSheetId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) inline_failure: Option<InlineFailurePolicy>,
}

/// Numeric host address used by server profiles.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct LaunchListenAddress(SocketAddr);

impl Serialize for LaunchListenAddress {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0.to_string())
    }
}

impl<'de> Deserialize<'de> for LaunchListenAddress {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        SocketAddr::from_str(&String::deserialize(deserializer)?)
            .map(Self)
            .map_err(de::Error::custom)
    }
}

impl LaunchListenAddress {
    pub(crate) fn parse(value: &str) -> Result<Self, std::net::AddrParseError> {
        SocketAddr::from_str(value).map(Self)
    }

    pub const fn socket_addr(self) -> SocketAddr {
        self.0
    }
}

/// Strict authored worker-count syntax for pure-helper execution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LaunchPureWorkers {
    Auto,
    Count(NonZeroU32),
}

impl Serialize for LaunchPureWorkers {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::Auto => serializer.serialize_str("auto"),
            Self::Count(count) => serializer.serialize_u32(count.get()),
        }
    }
}

impl<'de> Deserialize<'de> for LaunchPureWorkers {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Value {
            Auto(String),
            Count(NonZeroU32),
        }

        match Value::deserialize(deserializer)? {
            Value::Auto(value) if value == "auto" => Ok(Self::Auto),
            Value::Auto(value) => Err(de::Error::custom(format!(
                "pure workers must be `auto` or a positive integer, not `{value}`"
            ))),
            Value::Count(count) => Ok(Self::Count(count)),
        }
    }
}

/// Pure-helper policy retained in the final launch document.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct LaunchPureProfileSpec {
    #[serde(default)]
    pub(crate) backend: Option<super::LaunchPureBackend>,
    #[serde(default)]
    pub(crate) math_backend: Option<super::LaunchMathBackend>,
    #[serde(default)]
    pub(crate) math_wgpu_min_elements: Option<NonZeroU32>,
    #[serde(default)]
    pub(crate) workers: Option<LaunchPureWorkers>,
    #[serde(default)]
    pub(crate) batch_min_len: Option<NonZeroU32>,
    #[serde(default)]
    pub(crate) object_artifacts: Option<bool>,
}

/// Player profile values retained by the final manifest.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct LaunchPlayerProfileSpec {
    #[serde(default)]
    pub(crate) viewport: Option<LaunchPlayerViewportSpec>,
}

/// Optional authored design viewport and fit behavior.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct LaunchPlayerViewportSpec {
    #[serde(default)]
    pub(crate) design_width: Option<NonZeroU32>,
    #[serde(default)]
    pub(crate) design_height: Option<NonZeroU32>,
    #[serde(default)]
    pub(crate) fit: super::LaunchPlayerViewportFit,
}

impl LaunchPureProfileSpec {
    pub const fn backend(&self) -> Option<super::LaunchPureBackend> {
        self.backend
    }

    pub const fn math_backend(&self) -> Option<super::LaunchMathBackend> {
        self.math_backend
    }

    pub const fn math_wgpu_min_elements(&self) -> Option<NonZeroU32> {
        self.math_wgpu_min_elements
    }

    pub const fn workers(&self) -> Option<LaunchPureWorkers> {
        self.workers
    }

    pub const fn batch_min_len(&self) -> Option<NonZeroU32> {
        self.batch_min_len
    }

    pub const fn object_artifacts(&self) -> Option<bool> {
        self.object_artifacts
    }
}

impl LaunchPlayerProfileSpec {
    pub const fn viewport(&self) -> Option<LaunchPlayerViewportSpec> {
        self.viewport
    }
}

impl LaunchPlayerViewportSpec {
    pub const fn design_width(self) -> Option<NonZeroU32> {
        match (self.fit, self.design_width) {
            (super::LaunchPlayerViewportFit::Raw, _) => None,
            (_, Some(width)) => Some(width),
            (_, None) => NonZeroU32::new(1280),
        }
    }

    pub const fn design_height(self) -> Option<NonZeroU32> {
        match (self.fit, self.design_height) {
            (super::LaunchPlayerViewportFit::Raw, _) => None,
            (_, Some(height)) => Some(height),
            (_, None) => NonZeroU32::new(720),
        }
    }

    pub const fn fit(self) -> super::LaunchPlayerViewportFit {
        self.fit
    }
}
