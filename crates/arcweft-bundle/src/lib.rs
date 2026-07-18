//! Sans I/O bundle data model and deterministic codecs.

pub mod character_package;
pub mod container;
pub mod fx_definitions;
pub mod logical_identity;
pub mod patch;
mod product;
pub mod product_awbc;
pub mod release;
pub mod resource_codec;
pub mod standard_view;

use crate::character_package::BundleCharacterPackage;
use crate::fx_definitions::FxDefinitions;
use crate::resource_codec::view::{
    DialogueViewContractError, ViewStyleContractError, ViewTextSourceKind,
};
use crate::resource_codec::{
    SourceMapBuildError, SourceMapSection, ValidatedViewProduct, ViewInputResource,
    ViewProductValidationError, ViewProductValidationLimits, ViewProgramResource,
    ViewProgramStyleResources, ViewResourceMergeError, ViewStyleResource, ViewTextResource,
    ViewThemeResource,
};
#[cfg(feature = "format-avro")]
use apache_avro::types::Value as AvroValue;
#[cfg(feature = "format-avro")]
use apache_avro::{Reader, Schema, Writer};
use arcweft_agent_protocol::artifact::AgentArtifactManifest;
use arcweft_audio_core::graph::AudioGraph;
use arcweft_core::awbc::schema::AwbcProgram;
use arcweft_core::bytecode::BytecodeProgram;
#[cfg(feature = "format-yaml")]
use arcweft_data::{Number, Value};
use arcweft_layout::stage_placement::StagePlacement;
use arcweft_render_text::LineDisplayCatalog;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
#[cfg(feature = "format-cbor")]
use std::io::Cursor;
use std::path::Path;
use thiserror::Error;
#[cfg(feature = "format-yaml")]
use yaml_rust2::yaml::Hash;
#[cfg(feature = "format-yaml")]
use yaml_rust2::{Yaml, YamlEmitter, YamlLoader};

pub const ARCWEFT_BUNDLE_SCHEMA_VERSION: u32 = 5;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArcweftBundle {
    pub schema_version: u32,
    #[serde(default)]
    pub bundle_kind: BundleKind,
    pub manifest: BundleManifest,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<AgentArtifactManifest>,
    pub source_map: SourceMapSection,
    pub bytecode: BundleBytecodeProgram,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub product_awbc: Option<BundleAwbcProgram>,
    pub display: LineDisplayCatalog,
    #[serde(default, skip_serializing_if = "FxDefinitions::is_empty")]
    pub fx_definitions: FxDefinitions,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub adapter_manifests: Vec<BundleAdapterManifest>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub virtual_files: Vec<BundleVirtualFile>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub image_assets: Vec<BundleImageAsset>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub character_packages: Vec<BundleCharacterPackage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio: Option<AudioGraph>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub image_objects: Vec<BundleImageObject>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view_program: Option<ViewProgramResource>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view_style: Option<ViewStyleResource>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view_text: Option<ViewTextResource>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view_input: Option<ViewInputResource>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view_theme: Option<ViewThemeResource>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BundleManifest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile_kind: Option<BundleLaunchKind>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entry: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub adapter: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub adapter_manifest_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_host_calls: Vec<String>,
    pub runtime: BundleRuntimeSummary,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BundleBytecodeProgram {
    pub encoding: BundleBytecodeEncoding,
    pub program: BytecodeProgram,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BundleAwbcProgram {
    pub encoding: BundleAwbcEncoding,
    pub program: AwbcProgram,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BundleBytecodeEncoding {
    StructuredJson,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BundleAwbcEncoding {
    AwbcV1,
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BundleFormat {
    Awfb,
    #[default]
    Json,
    Toml,
    Yaml,
    MessagePack,
    Cbor,
    Avro,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BundleKind {
    #[default]
    Game,
    AgentController,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BundleAdapterManifest {
    pub id: String,
    pub display_name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub effects: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub host_calls: Vec<BundleAdapterHostCall>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BundleAdapterHostCall {
    pub id: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub effects: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BundleRuntimeSummary {
    pub entry_flow: Option<String>,
    pub flows: usize,
    pub bytecode_instructions: usize,
    pub line_task_groups: usize,
    pub stream_plans: usize,
    pub source_plans: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BundleVirtualFile {
    pub space: BundleVirtualFileSpace,
    pub path: String,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BundleVirtualFileRef {
    pub space: BundleVirtualFileSpace,
    pub path: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BundleImageAsset {
    pub id: String,
    pub file: BundleVirtualFileRef,
    pub format: BundleImageFormat,
    pub animation: BundleImageAnimation,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dimensions: Option<BundleImageDimensions>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BundleImageObject {
    pub id: String,
    pub asset: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layer: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub containing_scroll_region: Option<String>,
    pub bounds: BundleImageObjectBounds,
    /// Authored placement contract. When absent, `bounds` is explicit absolute
    /// placement for image object data that has no responsive stage placement.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placement: Option<StagePlacement>,
    #[serde(default)]
    pub fit: BundleImageObjectFit,
    #[serde(default)]
    pub alignment: BundleImageObjectAlignment,
    #[serde(default)]
    pub playback: BundleImageObjectPlayback,
    #[serde(default)]
    pub transform: BundleImageObjectTransform,
    #[serde(default)]
    pub depth_milli: i32,
    #[serde(default = "default_opacity_milli")]
    pub opacity_milli: u16,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub actions: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub params: BTreeMap<String, BundleImageObjectParam>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub proxies: Vec<BundleImageObjectProxy>,
    #[serde(default = "default_true")]
    pub visible: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BundleImageObjectParam {
    Bool { value: bool },
    Integer { value: i64 },
    Milli { value: i32 },
    Text { value: String },
    Id { value: String },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BundleImageObjectProxy {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub type_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layer: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub depth_milli: Option<i32>,
    #[serde(default)]
    pub hit_test: bool,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub params: BTreeMap<String, BundleImageObjectParam>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BundleImageObjectBounds {
    pub x_milli: i32,
    pub y_milli: i32,
    pub width_milli: u32,
    pub height_milli: u32,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BundleImageObjectFit {
    #[default]
    Contain,
    Cover,
    Stretch,
    Intrinsic,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BundleImageObjectAlignment {
    pub x_milli: i32,
    pub y_milli: i32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BundleImageObjectPlayback {
    pub start_time_millis: u64,
    pub rate_milli: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paused_at_millis: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pinned_local_time_millis: Option<u64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BundleImageObjectTransform {
    pub m11_milli: i32,
    pub m12_milli: i32,
    pub m21_milli: i32,
    pub m22_milli: i32,
    pub tx_milli: i32,
    pub ty_milli: i32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BundleImageFormat {
    Png,
    Jpeg,
    Gif,
    #[serde(rename = "webp")]
    WebP,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BundleImageAnimation {
    Static,
    Animated,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BundleImageDimensions {
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BundleVirtualFileSpace {
    Asset,
    Save,
    Temp,
    Export,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BundleLaunchKind {
    Game,
    Editor,
    Cli,
    Server,
    Test,
    Bench,
    Agent,
}

#[derive(Debug, Error)]
pub enum BundleCodecError {
    #[error("unsupported Arcweft bundle schema version {actual}; expected {expected}")]
    UnsupportedSchema { actual: u32, expected: u32 },
    #[error("agent controller bundle is missing its Agent artifact manifest")]
    MissingAgentManifest,
    #[error("non-agent bundle must not carry an Agent artifact manifest")]
    UnexpectedAgentManifest,
    #[error("failed to encode Arcweft bundle JSON: {0}")]
    Encode(#[source] serde_json::Error),
    #[error("failed to decode Arcweft bundle JSON: {0}")]
    Decode(#[source] serde_json::Error),
    #[error("failed to encode Arcweft bundle {format}: {message}")]
    EncodeFormat {
        format: BundleFormat,
        message: String,
    },
    #[error("failed to decode Arcweft bundle {format}: {message}")]
    DecodeFormat {
        format: BundleFormat,
        message: String,
    },
    #[error("unsupported Arcweft bundle format `{format}`")]
    UnsupportedFormat { format: String },
    #[error("Arcweft product bundle path must use `.awfb`: {path}")]
    ExpectedProductAwfbPath { path: String },
    #[error("failed to encode Arcweft AWFB bundle: {message}")]
    EncodeAwfb { message: String },
    #[error("failed to decode Arcweft AWFB bundle: {message}")]
    DecodeAwfb { message: String },
    #[error("product AWFB is missing its canonical AWBC executable payload")]
    MissingProductAwbcExecutable,
    #[error("product AWFB manifest is missing its required executable payload discriminator")]
    MissingProductExecutablePayload,
    #[error("product AWFB contains malformed AWBC executable payload: {message}")]
    MalformedProductAwbcExecutable { message: String },
    #[error("product AWFB structured bytecode payload tag {encoding_tag} is no longer executable")]
    StructuredProductBytecodeUnsupported { encoding_tag: u32 },
    #[error("unsupported product AWFB executable payload `{actual}`")]
    UnsupportedProductExecutablePayload { actual: String },
    #[error("product AWBC executable verification failed: {message}")]
    ProductAwbcVerification { message: String },
    #[error("Arcweft bundle format `{format}` requires Cargo feature `{feature}`")]
    DisabledFormat {
        format: BundleFormat,
        feature: &'static str,
    },
    #[error("bundle image asset `{asset_id}` references missing virtual file {space}:{path}")]
    MissingImageFile {
        asset_id: String,
        space: BundleVirtualFileSpace,
        path: String,
    },
    #[error("bundle audio asset `{asset_id}` references missing asset file {path}")]
    MissingAudioFile { asset_id: String, path: String },
    #[error("bundle contains duplicate adapter manifest id `{id}`")]
    DuplicateAdapterManifest { id: String },
    #[error("bundle contains duplicate virtual file {space}:{path}")]
    DuplicateVirtualFile {
        space: BundleVirtualFileSpace,
        path: String,
    },
    #[error("bundle contains duplicate image asset id `{id}`")]
    DuplicateImageAsset { id: String },
    #[error("bundle contains duplicate image object id `{id}`")]
    DuplicateImageObject { id: String },
    #[error("bundle contains duplicate character package `{id}`")]
    DuplicateCharacterPackage { id: String },
    #[error(
        "bundle character package `{character_id}` references missing virtual file asset:{path}"
    )]
    MissingCharacterPackageFile { character_id: String, path: String },
    #[error(
        "bundle character package `{character_id}` is missing layer payload `{path}` for `{part}.{variant}`"
    )]
    MissingCharacterLayerPayload {
        character_id: String,
        path: String,
        part: String,
        variant: String,
    },
    #[error(transparent)]
    InvalidDialogueViewContract(#[from] DialogueViewContractError),
    #[error(transparent)]
    InvalidViewStyleContract(#[from] ViewStyleContractError),
    #[error(transparent)]
    InvalidViewProduct(#[from] ViewProductValidationError),
}

#[cfg(feature = "format-yaml")]
fn bundle_value_to_yaml(value: &Value) -> Result<Yaml, String> {
    match value {
        Value::Unit => Ok(Yaml::Null),
        Value::Bool(value) => Ok(Yaml::Boolean(*value)),
        Value::Number(Number::I(value)) => i64::try_from(*value)
            .map(Yaml::Integer)
            .map_err(|_| "YAML bundle integer is out of i64 range".to_owned()),
        Value::Number(Number::U(value)) => i64::try_from(*value)
            .map(Yaml::Integer)
            .map_err(|_| "YAML bundle unsigned integer is out of i64 range".to_owned()),
        Value::Number(Number::F32(value)) if value.is_finite() => Ok(Yaml::Real(value.to_string())),
        Value::Number(Number::F64(value)) if value.is_finite() => Ok(Yaml::Real(value.to_string())),
        Value::Number(Number::F32(_) | Number::F64(_)) => {
            Err("YAML bundle floats must be finite".to_owned())
        }
        Value::String(value) => Ok(Yaml::String(value.clone())),
        Value::Char(value) => Ok(Yaml::String(value.to_string())),
        Value::Bytes(bytes) => Ok(Yaml::Array(
            bytes
                .as_slice()
                .iter()
                .copied()
                .map(i64::from)
                .map(Yaml::Integer)
                .collect(),
        )),
        Value::Seq(values) => values
            .iter()
            .map(bundle_value_to_yaml)
            .collect::<Result<Vec<_>, _>>()
            .map(Yaml::Array),
        Value::Map(entries) | Value::Record(entries) => {
            let mut hash = Hash::new();
            for (key, value) in entries {
                hash.insert(Yaml::String(key.clone()), bundle_value_to_yaml(value)?);
            }
            Ok(Yaml::Hash(hash))
        }
        Value::Enum { variant, payload } => match payload {
            Some(payload) => {
                let mut hash = Hash::new();
                hash.insert(
                    Yaml::String(variant.clone()),
                    bundle_value_to_yaml(payload)?,
                );
                Ok(Yaml::Hash(hash))
            }
            None => Ok(Yaml::String(variant.clone())),
        },
    }
}

#[cfg(feature = "format-yaml")]
fn bundle_yaml_to_value(yaml: &Yaml) -> Result<Value, String> {
    match yaml {
        Yaml::Null => Ok(Value::Unit),
        Yaml::Boolean(value) => Ok(Value::Bool(*value)),
        Yaml::Integer(value) => Ok(Value::Number(Number::I(i128::from(*value)))),
        Yaml::Real(value) => value
            .parse::<f64>()
            .map_err(|error| error.to_string())
            .and_then(|value| {
                if value.is_finite() {
                    Ok(Value::Number(Number::F64(value)))
                } else {
                    Err("YAML bundle floats must be finite".to_owned())
                }
            }),
        Yaml::String(value) => Ok(Value::String(value.clone())),
        Yaml::Array(values) => values
            .iter()
            .map(bundle_yaml_to_value)
            .collect::<Result<Vec<_>, _>>()
            .map(Value::Seq),
        Yaml::Hash(entries) => entries
            .iter()
            .map(|(key, value)| {
                let Yaml::String(key) = key else {
                    return Err("YAML bundle map keys must be strings".to_owned());
                };
                bundle_yaml_to_value(value).map(|value| (key.clone(), value))
            })
            .collect::<Result<BTreeMap<_, _>, _>>()
            .map(Value::Record),
        Yaml::Alias(_) => Err("YAML bundle aliases are not supported".to_owned()),
        Yaml::BadValue => Err("YAML bundle contains an invalid value".to_owned()),
    }
}

impl ArcweftBundle {
    /// Constructs a game bundle with the engine-owned standard dialogue resources.
    ///
    /// # Errors
    ///
    /// Returns a source-map build error when the supplied map conflicts with,
    /// or has no capacity for, the reserved standard dialogue Style source.
    pub fn try_new(
        manifest: BundleManifest,
        source_map: SourceMapSection,
        bytecode: BytecodeProgram,
        display: LineDisplayCatalog,
    ) -> Result<Self, SourceMapBuildError> {
        let standard_style_source = standard_view::dialogue_style_source_document();
        let source_map = source_map.try_with_document(&standard_style_source)?;
        Ok(Self {
            schema_version: ARCWEFT_BUNDLE_SCHEMA_VERSION,
            bundle_kind: BundleKind::Game,
            manifest,
            agent: None,
            source_map,
            bytecode: BundleBytecodeProgram {
                encoding: BundleBytecodeEncoding::StructuredJson,
                program: bytecode,
            },
            product_awbc: None,
            display,
            fx_definitions: FxDefinitions::default(),
            adapter_manifests: Vec::new(),
            virtual_files: Vec::new(),
            image_assets: Vec::new(),
            character_packages: Vec::new(),
            audio: None,
            image_objects: Vec::new(),
            view_program: Some(standard_view::dialogue_program()),
            view_style: Some(standard_view::dialogue_style()),
            view_text: Some(standard_view::dialogue_text()),
            view_input: None,
            view_theme: None,
        })
    }

    /// Human-readable label projected from the canonical source map.
    pub fn source_display_name(&self) -> &str {
        self.source_map
            .documents()
            .find(|source| source.document_id().as_str() != standard_view::DIALOGUE_STYLE_SOURCE_ID)
            .or_else(|| self.source_map.documents().next())
            .map_or("<no source>", |source| source.display_name().display_name())
    }

    /// Primary non-engine source document used by adapters that need a workspace anchor.
    pub fn primary_source_document(&self) -> Option<&crate::resource_codec::SourceMapDocument> {
        self.source_map
            .documents()
            .find(|source| source.document_id().as_str() != standard_view::DIALOGUE_STYLE_SOURCE_ID)
            .or_else(|| self.source_map.documents().next())
    }

    #[must_use]
    pub fn with_adapter_manifests(
        mut self,
        manifests: impl IntoIterator<Item = BundleAdapterManifest>,
    ) -> Self {
        self.adapter_manifests.extend(manifests);
        self.adapter_manifests
            .sort_by(|left, right| left.id.cmp(&right.id));
        self
    }

    #[must_use]
    pub fn with_virtual_files(
        mut self,
        files: impl IntoIterator<Item = BundleVirtualFile>,
    ) -> Self {
        self.virtual_files.extend(files);
        self.virtual_files.sort_by(|left, right| {
            left.space
                .as_str()
                .cmp(right.space.as_str())
                .then_with(|| left.path.cmp(&right.path))
        });
        self
    }

    #[must_use]
    pub fn with_image_assets(mut self, assets: impl IntoIterator<Item = BundleImageAsset>) -> Self {
        self.image_assets.extend(assets);
        self.image_assets
            .sort_by(|left, right| left.id.cmp(&right.id));
        self
    }

    #[must_use]
    pub fn with_character_packages(
        mut self,
        packages: impl IntoIterator<Item = BundleCharacterPackage>,
    ) -> Self {
        self.character_packages.extend(packages);
        self.character_packages
            .sort_by(|left, right| left.character.cmp(&right.character));
        self
    }

    pub fn character_package(&self, id: &str) -> Option<&BundleCharacterPackage> {
        self.character_packages
            .iter()
            .find(|package| package.character == id)
    }

    #[must_use]
    pub fn with_image_objects(
        mut self,
        objects: impl IntoIterator<Item = BundleImageObject>,
    ) -> Self {
        self.image_objects.extend(objects);
        self.image_objects
            .sort_by(|left, right| left.id.cmp(&right.id));
        self
    }

    #[must_use]
    pub fn with_audio_graph(mut self, graph: AudioGraph) -> Self {
        self.audio = Some(graph);
        self
    }

    #[must_use]
    pub fn with_fx_definitions(mut self, definitions: FxDefinitions) -> Self {
        self.fx_definitions = definitions;
        self
    }

    pub fn with_view_resources(
        mut self,
        program: Option<ViewProgramResource>,
        style: Option<ViewStyleResource>,
    ) -> Result<Self, ViewResourceMergeError> {
        // The authored resource is the product program; the built-in dialogue
        // resource is linked into it as a library. Keeping the authored side
        // as the merge owner preserves its program identity while still
        // validating reserved standard IDs in the same atomic transaction.
        let merged = ViewProgramStyleResources::new(program, style).merge(
            ViewProgramStyleResources::new(self.view_program.take(), self.view_style.take()),
        )?;
        self.view_program = merged.program;
        self.view_style = merged.style;
        Ok(self)
    }

    #[must_use]
    pub fn with_view_text(mut self, resource: ViewTextResource) -> Self {
        self.view_text = Some(standard_view::merge_text(
            self.view_text
                .take()
                .unwrap_or_else(standard_view::dialogue_text),
            resource,
        ));
        self
    }

    #[must_use]
    pub fn with_view_input(mut self, resource: ViewInputResource) -> Self {
        self.view_input = Some(resource);
        self
    }

    #[must_use]
    pub fn with_view_theme(mut self, resource: ViewThemeResource) -> Self {
        self.view_theme = Some(resource);
        self
    }

    #[must_use]
    pub fn with_agent_manifest(mut self, manifest: AgentArtifactManifest) -> Self {
        self.bundle_kind = BundleKind::AgentController;
        self.agent = Some(manifest);
        self
    }

    #[must_use]
    pub fn with_product_awbc(mut self, program: AwbcProgram) -> Self {
        self.product_awbc = Some(BundleAwbcProgram::new(program));
        self
    }

    pub const fn product_awbc(&self) -> Option<&BundleAwbcProgram> {
        self.product_awbc.as_ref()
    }

    pub fn product_awbc_program(&self) -> Result<&AwbcProgram, BundleCodecError> {
        self.product_awbc
            .as_ref()
            .map(BundleAwbcProgram::program)
            .ok_or(BundleCodecError::MissingProductAwbcExecutable)
    }

    pub fn to_json_bytes(&self) -> Result<Vec<u8>, BundleCodecError> {
        self.validate_kind()?;
        serde_json::to_vec_pretty(self).map_err(BundleCodecError::Encode)
    }

    pub fn from_json_slice(bytes: &[u8]) -> Result<Self, BundleCodecError> {
        let bundle: Self = serde_json::from_slice(bytes).map_err(BundleCodecError::Decode)?;
        bundle.validate_schema_and_kind()?;
        Ok(bundle)
    }

    pub fn to_format_bytes(&self, format: BundleFormat) -> Result<Vec<u8>, BundleCodecError> {
        self.validate_kind()?;
        match format {
            BundleFormat::Awfb => product::to_awfb_bytes(self),
            BundleFormat::Json => self.to_json_bytes(),
            #[cfg(feature = "format-toml")]
            BundleFormat::Toml => toml::to_string_pretty(self)
                .map(String::into_bytes)
                .map_err(|error| BundleCodecError::EncodeFormat {
                    format,
                    message: error.to_string(),
                }),
            #[cfg(not(feature = "format-toml"))]
            BundleFormat::Toml => Err(format.disabled_error()),
            #[cfg(feature = "format-yaml")]
            BundleFormat::Yaml => {
                let value = arcweft_serde_bridge::to_arcweft_value(self).map_err(|error| {
                    BundleCodecError::EncodeFormat {
                        format,
                        message: error.to_string(),
                    }
                })?;
                let yaml = bundle_value_to_yaml(&value)
                    .map_err(|message| BundleCodecError::EncodeFormat { format, message })?;
                let mut out = String::new();
                YamlEmitter::new(&mut out).dump(&yaml).map_err(|error| {
                    BundleCodecError::EncodeFormat {
                        format,
                        message: error.to_string(),
                    }
                })?;
                Ok(out.into_bytes())
            }
            #[cfg(not(feature = "format-yaml"))]
            BundleFormat::Yaml => Err(format.disabled_error()),
            #[cfg(feature = "format-messagepack")]
            BundleFormat::MessagePack => {
                rmp_serde::to_vec_named(self).map_err(|error| BundleCodecError::EncodeFormat {
                    format,
                    message: error.to_string(),
                })
            }
            #[cfg(not(feature = "format-messagepack"))]
            BundleFormat::MessagePack => Err(format.disabled_error()),
            #[cfg(feature = "format-cbor")]
            BundleFormat::Cbor => {
                let mut bytes = Vec::new();
                ciborium::ser::into_writer(self, &mut bytes).map_err(|error| {
                    BundleCodecError::EncodeFormat {
                        format,
                        message: error.to_string(),
                    }
                })?;
                Ok(bytes)
            }
            #[cfg(not(feature = "format-cbor"))]
            BundleFormat::Cbor => Err(format.disabled_error()),
            #[cfg(feature = "format-avro")]
            BundleFormat::Avro => self.to_avro_envelope_bytes(),
            #[cfg(not(feature = "format-avro"))]
            BundleFormat::Avro => Err(format.disabled_error()),
        }
    }

    pub fn from_format_slice(format: BundleFormat, bytes: &[u8]) -> Result<Self, BundleCodecError> {
        match format {
            BundleFormat::Awfb => return product::from_awfb_slice(bytes),
            BundleFormat::Json => return Self::from_json_slice(bytes),
            BundleFormat::Toml
            | BundleFormat::Yaml
            | BundleFormat::MessagePack
            | BundleFormat::Cbor
            | BundleFormat::Avro => {}
        }
        let bundle = Self::from_non_json_format_slice(format, bytes)?;
        bundle.validate_schema_and_kind()?;
        Ok(bundle)
    }

    fn from_non_json_format_slice(
        format: BundleFormat,
        bytes: &[u8],
    ) -> Result<Self, BundleCodecError> {
        #[cfg(not(any(
            feature = "format-avro",
            feature = "format-cbor",
            feature = "format-messagepack",
            feature = "format-toml",
            feature = "format-yaml"
        )))]
        {
            let _ = bytes;
            Err(format.disabled_error())
        }

        #[cfg(any(
            feature = "format-avro",
            feature = "format-cbor",
            feature = "format-messagepack",
            feature = "format-toml",
            feature = "format-yaml"
        ))]
        let bundle = match format {
            BundleFormat::Awfb => return product::from_awfb_slice(bytes),
            BundleFormat::Json => return Self::from_json_slice(bytes),
            #[cfg(feature = "format-toml")]
            BundleFormat::Toml => {
                let source =
                    std::str::from_utf8(bytes).map_err(|error| BundleCodecError::DecodeFormat {
                        format,
                        message: error.to_string(),
                    })?;
                toml::from_str(source).map_err(|error| BundleCodecError::DecodeFormat {
                    format,
                    message: error.to_string(),
                })?
            }
            #[cfg(not(feature = "format-toml"))]
            BundleFormat::Toml => return Err(format.disabled_error()),
            #[cfg(feature = "format-yaml")]
            BundleFormat::Yaml => {
                let source =
                    std::str::from_utf8(bytes).map_err(|error| BundleCodecError::DecodeFormat {
                        format,
                        message: error.to_string(),
                    })?;
                let documents = YamlLoader::load_from_str(source).map_err(|error| {
                    BundleCodecError::DecodeFormat {
                        format,
                        message: error.to_string(),
                    }
                })?;
                let [document] = documents.as_slice() else {
                    let message = match documents.len() {
                        0 => "YAML bundle document is empty".to_owned(),
                        _ => "YAML bundle accepts exactly one document".to_owned(),
                    };
                    return Err(BundleCodecError::DecodeFormat { format, message });
                };
                let value = bundle_yaml_to_value(document)
                    .map_err(|message| BundleCodecError::DecodeFormat { format, message })?;
                arcweft_serde_bridge::from_arcweft_value(&value).map_err(|error| {
                    BundleCodecError::DecodeFormat {
                        format,
                        message: error.to_string(),
                    }
                })?
            }
            #[cfg(not(feature = "format-yaml"))]
            BundleFormat::Yaml => return Err(format.disabled_error()),
            #[cfg(feature = "format-messagepack")]
            BundleFormat::MessagePack => {
                rmp_serde::from_slice(bytes).map_err(|error| BundleCodecError::DecodeFormat {
                    format,
                    message: error.to_string(),
                })?
            }
            #[cfg(not(feature = "format-messagepack"))]
            BundleFormat::MessagePack => return Err(format.disabled_error()),
            #[cfg(feature = "format-cbor")]
            BundleFormat::Cbor => {
                ciborium::de::from_reader(Cursor::new(bytes)).map_err(|error| {
                    BundleCodecError::DecodeFormat {
                        format,
                        message: error.to_string(),
                    }
                })?
            }
            #[cfg(not(feature = "format-cbor"))]
            BundleFormat::Cbor => return Err(format.disabled_error()),
            #[cfg(feature = "format-avro")]
            BundleFormat::Avro => Self::from_avro_envelope_slice(bytes)?,
            #[cfg(not(feature = "format-avro"))]
            BundleFormat::Avro => return Err(format.disabled_error()),
        };
        #[cfg(any(
            feature = "format-avro",
            feature = "format-cbor",
            feature = "format-messagepack",
            feature = "format-toml",
            feature = "format-yaml"
        ))]
        Ok(bundle)
    }

    pub fn from_product_path_slice(path: &Path, bytes: &[u8]) -> Result<Self, BundleCodecError> {
        if BundleFormat::from_path(path) != Some(BundleFormat::Awfb) {
            return Err(BundleCodecError::ExpectedProductAwfbPath {
                path: path.display().to_string(),
            });
        }
        product::from_awfb_slice(bytes)
    }

    pub fn from_product_path_slice_with_external_sections(
        path: &Path,
        bytes: &[u8],
        external_sections: &[container::ExternalSectionPayload],
    ) -> Result<Self, BundleCodecError> {
        if BundleFormat::from_path(path) != Some(BundleFormat::Awfb) {
            return Err(BundleCodecError::ExpectedProductAwfbPath {
                path: path.display().to_string(),
            });
        }
        product::from_awfb_slice_with_external_sections(bytes, external_sections)
    }

    pub fn from_awfb_slice_with_external_sections(
        bytes: &[u8],
        external_sections: &[container::ExternalSectionPayload],
    ) -> Result<Self, BundleCodecError> {
        product::from_awfb_slice_with_external_sections(bytes, external_sections)
    }

    pub fn from_inspection_path_slice(path: &Path, bytes: &[u8]) -> Result<Self, BundleCodecError> {
        let Some(format) = BundleFormat::from_path(path) else {
            return Err(BundleCodecError::UnsupportedFormat {
                format: path
                    .extension()
                    .and_then(std::ffi::OsStr::to_str)
                    .unwrap_or("<missing>")
                    .to_owned(),
            });
        };
        Self::from_format_slice(format, bytes)
    }

    pub fn virtual_file(&self, file: &BundleVirtualFileRef) -> Option<&BundleVirtualFile> {
        self.virtual_files
            .iter()
            .find(|candidate| candidate.space == file.space && candidate.path == file.path)
    }

    pub fn image_asset(&self, id: &str) -> Option<&BundleImageAsset> {
        self.image_assets.iter().find(|asset| asset.id == id)
    }

    pub fn image_object(&self, id: &str) -> Option<&BundleImageObject> {
        self.image_objects.iter().find(|object| object.id == id)
    }

    pub fn image_asset_bytes(&self, id: &str) -> Result<Option<&[u8]>, BundleCodecError> {
        let Some(asset) = self.image_asset(id) else {
            return Ok(None);
        };
        let Some(file) = self.virtual_file(&asset.file) else {
            return Err(BundleCodecError::MissingImageFile {
                asset_id: asset.id.clone(),
                space: asset.file.space,
                path: asset.file.path.clone(),
            });
        };
        Ok(Some(file.bytes.as_slice()))
    }

    pub fn audio_asset_bytes(&self, id: &str) -> Result<Option<&[u8]>, BundleCodecError> {
        let Some(asset) = self
            .audio
            .as_ref()
            .and_then(|graph| graph.assets.iter().find(|asset| asset.id.as_str() == id))
        else {
            return Ok(None);
        };
        let Some(file) = self
            .virtual_files
            .iter()
            .find(|file| file.space == BundleVirtualFileSpace::Asset && file.path == asset.path)
        else {
            return Err(BundleCodecError::MissingAudioFile {
                asset_id: asset.id.as_str().to_owned(),
                path: asset.path.clone(),
            });
        };
        Ok(Some(file.bytes.as_slice()))
    }

    fn validate_kind(&self) -> Result<(), BundleCodecError> {
        match (self.bundle_kind, self.agent.is_some()) {
            (BundleKind::AgentController, false) => Err(BundleCodecError::MissingAgentManifest),
            (BundleKind::AgentController, true) | (BundleKind::Game, false) => {
                self.validate_unique_items()
            }
            (BundleKind::Game, true) => Err(BundleCodecError::UnexpectedAgentManifest),
        }
    }

    fn validate_unique_items(&self) -> Result<(), BundleCodecError> {
        let mut adapter_ids = BTreeSet::new();
        for manifest in &self.adapter_manifests {
            if !adapter_ids.insert(manifest.id.as_str()) {
                return Err(BundleCodecError::DuplicateAdapterManifest {
                    id: manifest.id.clone(),
                });
            }
        }

        let mut virtual_files = BTreeSet::new();
        for file in &self.virtual_files {
            if !virtual_files.insert((file.space.as_str(), file.path.as_str())) {
                return Err(BundleCodecError::DuplicateVirtualFile {
                    space: file.space,
                    path: file.path.clone(),
                });
            }
        }

        let mut image_asset_ids = BTreeSet::new();
        for asset in &self.image_assets {
            if !image_asset_ids.insert(asset.id.as_str()) {
                return Err(BundleCodecError::DuplicateImageAsset {
                    id: asset.id.clone(),
                });
            }
        }

        let mut character_package_ids = BTreeSet::new();
        for package in &self.character_packages {
            if !character_package_ids.insert(package.character.as_str()) {
                return Err(BundleCodecError::DuplicateCharacterPackage {
                    id: package.character.clone(),
                });
            }
            package.validate_files(&self.virtual_files)?;
        }

        let mut image_object_ids = BTreeSet::new();
        for object in &self.image_objects {
            if !image_object_ids.insert(object.id.as_str()) {
                return Err(BundleCodecError::DuplicateImageObject {
                    id: object.id.clone(),
                });
            }
        }
        let dialogue_contract = match &self.view_program {
            Some(program) => program
                .validate_dialogue_contract(self.view_text.as_ref())
                .map_err(BundleCodecError::from),
            None => self
                .view_text
                .as_ref()
                .and_then(|text| {
                    text.sources
                        .iter()
                        .find(|source| matches!(source.kind, ViewTextSourceKind::Dialogue { .. }))
                })
                .map_or(Ok(()), |source| {
                    Err(BundleCodecError::from(
                        DialogueViewContractError::MissingProgram {
                            text_source: source.public_id.clone(),
                        },
                    ))
                }),
        };
        dialogue_contract?;
        if let Some(program) = &self.view_program {
            program.validate_style_contract(self.view_style.as_ref())?;
        } else if let Some(style) = &self.view_style {
            style
                .encode_canonical_section()
                .map_err(ViewStyleContractError::InvalidResource)?;
        }
        ValidatedViewProduct::try_new(
            Some(self.source_map.clone()),
            self.view_program.clone(),
            self.view_style.clone(),
            ViewProductValidationLimits::default(),
        )?;
        Ok(())
    }

    fn validate_schema_and_kind(&self) -> Result<(), BundleCodecError> {
        if self.schema_version != ARCWEFT_BUNDLE_SCHEMA_VERSION {
            return Err(BundleCodecError::UnsupportedSchema {
                actual: self.schema_version,
                expected: ARCWEFT_BUNDLE_SCHEMA_VERSION,
            });
        }
        self.validate_kind()
    }

    #[cfg(feature = "format-avro")]
    fn to_avro_envelope_bytes(&self) -> Result<Vec<u8>, BundleCodecError> {
        let schema = bundle_avro_envelope_schema()?;
        let payload_json = serde_json::to_string(self).map_err(BundleCodecError::Encode)?;
        let mut writer = Writer::new(&schema, Vec::new());
        writer
            .append(AvroValue::Record(vec![
                (
                    "schema_version".to_owned(),
                    AvroValue::Int(
                        i32::try_from(ARCWEFT_BUNDLE_SCHEMA_VERSION)
                            .expect("Arcweft bundle schema version fits in Avro int"),
                    ),
                ),
                (
                    "payload_format".to_owned(),
                    AvroValue::String(BundleFormat::Json.as_str().to_owned()),
                ),
                ("payload_json".to_owned(), AvroValue::String(payload_json)),
            ]))
            .map_err(|error| BundleCodecError::EncodeFormat {
                format: BundleFormat::Avro,
                message: error.to_string(),
            })?;
        writer
            .into_inner()
            .map_err(|error| BundleCodecError::EncodeFormat {
                format: BundleFormat::Avro,
                message: error.to_string(),
            })
    }

    #[cfg(feature = "format-avro")]
    fn from_avro_envelope_slice(bytes: &[u8]) -> Result<Self, BundleCodecError> {
        let reader = Reader::new(bytes).map_err(|error| BundleCodecError::DecodeFormat {
            format: BundleFormat::Avro,
            message: error.to_string(),
        })?;
        let mut values = reader.into_iter();
        let Some(value) = values.next() else {
            return Err(BundleCodecError::DecodeFormat {
                format: BundleFormat::Avro,
                message: "Avro bundle envelope is empty".to_owned(),
            });
        };
        let value = value.map_err(|error| BundleCodecError::DecodeFormat {
            format: BundleFormat::Avro,
            message: error.to_string(),
        })?;
        let fields = match value {
            AvroValue::Record(fields) => fields,
            other => {
                return Err(BundleCodecError::DecodeFormat {
                    format: BundleFormat::Avro,
                    message: format!("expected Avro record envelope, found {other:?}"),
                });
            }
        };
        let payload_format = avro_string_field(&fields, "payload_format")?;
        if payload_format != BundleFormat::Json.as_str() {
            return Err(BundleCodecError::DecodeFormat {
                format: BundleFormat::Avro,
                message: format!("unsupported Avro payload format `{payload_format}`"),
            });
        }
        Self::from_json_slice(avro_string_field(&fields, "payload_json")?.as_bytes())
    }
}

impl BundleFormat {
    pub const ALL: [Self; 7] = [
        Self::Awfb,
        Self::Json,
        Self::Toml,
        Self::Yaml,
        Self::MessagePack,
        Self::Cbor,
        Self::Avro,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Awfb => "awfb",
            Self::Json => "json",
            Self::Toml => "toml",
            Self::Yaml => "yaml",
            Self::MessagePack => "msgpack",
            Self::Cbor => "cbor",
            Self::Avro => "avro",
        }
    }

    pub const fn required_feature(self) -> Option<&'static str> {
        match self {
            Self::Awfb | Self::Json => None,
            Self::Toml => Some("format-toml"),
            Self::Yaml => Some("format-yaml"),
            Self::MessagePack => Some("format-messagepack"),
            Self::Cbor => Some("format-cbor"),
            Self::Avro => Some("format-avro"),
        }
    }

    pub const fn is_codec_enabled(self) -> bool {
        match self {
            Self::Awfb | Self::Json => true,
            Self::Toml => cfg!(feature = "format-toml"),
            Self::Yaml => cfg!(feature = "format-yaml"),
            Self::MessagePack => cfg!(feature = "format-messagepack"),
            Self::Cbor => cfg!(feature = "format-cbor"),
            Self::Avro => cfg!(feature = "format-avro"),
        }
    }

    pub fn enabled_formats() -> impl Iterator<Item = Self> {
        Self::ALL
            .into_iter()
            .filter(|format| format.is_codec_enabled())
    }

    pub fn probe_order() -> Vec<Self> {
        [
            Self::Awfb,
            Self::Json,
            Self::MessagePack,
            Self::Cbor,
            Self::Avro,
            Self::Toml,
            Self::Yaml,
        ]
        .into_iter()
        .filter(|format| format.is_codec_enabled())
        .collect()
    }

    #[cfg(any(
        not(feature = "format-avro"),
        not(feature = "format-cbor"),
        not(feature = "format-messagepack"),
        not(feature = "format-toml"),
        not(feature = "format-yaml")
    ))]
    fn disabled_error(self) -> BundleCodecError {
        BundleCodecError::DisabledFormat {
            format: self,
            feature: self
                .required_feature()
                .expect("only non-JSON bundle formats can be disabled"),
        }
    }

    pub fn parse(value: &str) -> Result<Self, BundleCodecError> {
        match value.trim().to_ascii_lowercase().as_str() {
            "json" => Ok(Self::Json),
            "awfb" => Ok(Self::Awfb),
            "toml" => Ok(Self::Toml),
            "yaml" | "yml" => Ok(Self::Yaml),
            "messagepack" | "msgpack" | "mpk" => Ok(Self::MessagePack),
            "cbor" => Ok(Self::Cbor),
            "avro" => Ok(Self::Avro),
            other => Err(BundleCodecError::UnsupportedFormat {
                format: other.to_owned(),
            }),
        }
    }

    #[must_use]
    pub fn from_path(path: &Path) -> Option<Self> {
        let extension = path.extension()?.to_str()?;
        Self::parse(extension).ok()
    }
}

impl std::fmt::Display for BundleFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for BundleFormat {
    type Err = BundleCodecError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

#[cfg(feature = "format-avro")]
fn bundle_avro_envelope_schema() -> Result<Schema, BundleCodecError> {
    Schema::parse_str(
        r#"{
          "type": "record",
          "name": "ArcweftBundleEnvelope",
          "namespace": "org.arcweft.bundle",
          "fields": [
            {"name": "schema_version", "type": "int"},
            {"name": "payload_format", "type": "string"},
            {"name": "payload_json", "type": "string"}
          ]
        }"#,
    )
    .map_err(|error| BundleCodecError::EncodeFormat {
        format: BundleFormat::Avro,
        message: error.to_string(),
    })
}

#[cfg(feature = "format-avro")]
fn avro_string_field(
    fields: &[(String, AvroValue)],
    name: &str,
) -> Result<String, BundleCodecError> {
    fields
        .iter()
        .find_map(|(field_name, value)| {
            (field_name == name).then(|| match value {
                AvroValue::String(value) => Ok(value.clone()),
                other => Err(BundleCodecError::DecodeFormat {
                    format: BundleFormat::Avro,
                    message: format!(
                        "Avro envelope field `{name}` must be string, found {other:?}"
                    ),
                }),
            })
        })
        .unwrap_or_else(|| {
            Err(BundleCodecError::DecodeFormat {
                format: BundleFormat::Avro,
                message: format!("Avro envelope missing field `{name}`"),
            })
        })
}

impl BundleVirtualFile {
    pub fn file_ref(&self) -> BundleVirtualFileRef {
        BundleVirtualFileRef {
            space: self.space,
            path: self.path.clone(),
        }
    }
}

impl BundleImageDimensions {
    pub const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }
}

impl BundleImageObjectBounds {
    pub const fn from_px(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            x_milli: x.saturating_mul(1_000),
            y_milli: y.saturating_mul(1_000),
            width_milli: width.saturating_mul(1_000),
            height_milli: height.saturating_mul(1_000),
        }
    }
}

impl Default for BundleImageObjectAlignment {
    fn default() -> Self {
        Self {
            x_milli: 500,
            y_milli: 500,
        }
    }
}

impl Default for BundleImageObjectPlayback {
    fn default() -> Self {
        Self {
            start_time_millis: 0,
            rate_milli: 1_000,
            paused_at_millis: None,
            pinned_local_time_millis: None,
        }
    }
}

impl BundleImageObjectPlayback {
    pub fn local_time_millis(self, visual_time_millis: u64) -> u64 {
        if let Some(pinned) = self.pinned_local_time_millis {
            return pinned;
        }
        let sample_time = self.paused_at_millis.unwrap_or(visual_time_millis);
        let elapsed = sample_time.saturating_sub(self.start_time_millis);
        if self.rate_milli == 0 {
            return 0;
        }
        elapsed.saturating_mul(u64::from(self.rate_milli)) / 1_000
    }
}

impl Default for BundleImageObjectTransform {
    fn default() -> Self {
        Self {
            m11_milli: 1_000,
            m12_milli: 0,
            m21_milli: 0,
            m22_milli: 1_000,
            tx_milli: 0,
            ty_milli: 0,
        }
    }
}

const fn default_opacity_milli() -> u16 {
    1_000
}

const fn default_true() -> bool {
    true
}

impl BundleVirtualFileSpace {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Asset => "asset",
            Self::Save => "save",
            Self::Temp => "temp",
            Self::Export => "export",
        }
    }
}

impl BundleKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Game => "game",
            Self::AgentController => "agent_controller",
        }
    }
}

impl BundleAdapterManifest {
    pub fn host_call_ids(&self) -> impl Iterator<Item = &str> {
        self.host_calls
            .iter()
            .map(|host_call| host_call.id.as_str())
    }
}

impl std::fmt::Display for BundleVirtualFileSpace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::fmt::Display for BundleKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::container::{
        BundleSectionKind as ContainerSectionKind, BundleView, ReadBudget, SectionInput,
        encode_bundle,
    };
    use crate::resource_codec::runtime::RuntimeTypesSection;
    use arcweft_agent_protocol::{
        artifact::{
            AgentArtifactManifest, AgentBundleKind, EffectCapability, ProjectBinding,
            ProjectBindingMode, RequiredEntity,
        },
        ids::{CallableId, PublicId, StableHash},
        verified_effects::VerifiedEffectSummary,
    };
    use arcweft_audio_core::graph::{
        AudioAsset, AudioBusDef, AudioDecodeStrategy, AudioFormat, AudioGraph,
    };
    use arcweft_core::awbc::schema::{
        AwbcBlock, AwbcBlockId, AwbcEffectSetId, AwbcEntry, AwbcEntryKind, AwbcEntryTarget,
        AwbcFrameLayout, AwbcFrameLayoutId, AwbcFunction, AwbcFunctionFlags, AwbcFunctionId,
        AwbcFunctionKind, AwbcSafePointKind, AwbcSignature, AwbcSignatureId, AwbcStringId,
        AwbcTableRange, AwbcTerminator,
    };
    use arcweft_core::entry::AgentBudget;
    use arcweft_interaction_model::audio::{
        AudioBusId, AudioLoopMode, AudioResourceId, GainDbMilli,
    };
    use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};

    #[test]
    fn bundle_json_round_trips_without_paths() {
        let bundle = ArcweftBundle::try_new(
            BundleManifest {
                profile_id: None,
                profile_kind: None,
                entry: Some("main".to_owned()),
                adapter: Some("native-file".to_owned()),
                adapter_manifest_ids: vec!["native-file".to_owned()],
                required_host_calls: vec!["fs.read_text".to_owned()],
                runtime: BundleRuntimeSummary {
                    entry_flow: Some("flow.main".to_owned()),
                    flows: 1,
                    bytecode_instructions: 3,
                    line_task_groups: 0,
                    stream_plans: 0,
                    source_plans: 0,
                },
            },
            source_map("main.arcw", "flow @flow.main main { return \"ok\" }"),
            BytecodeProgram::default(),
            LineDisplayCatalog::default(),
        )
        .expect("standard dialogue source joins source map")
        .with_adapter_manifests([BundleAdapterManifest {
            id: "native-file".to_owned(),
            display_name: "Native File".to_owned(),
            effects: vec!["fs.read".to_owned()],
            host_calls: vec![BundleAdapterHostCall {
                id: "fs.read_text".to_owned(),
                effects: vec!["fs.read".to_owned()],
            }],
        }])
        .with_virtual_files([BundleVirtualFile {
            space: BundleVirtualFileSpace::Asset,
            path: "dialogue/opening.txt".to_owned(),
            bytes: b"hello".to_vec(),
        }]);

        let bytes = bundle.to_json_bytes().expect("bundle encodes");
        let json = String::from_utf8(bytes.clone()).expect("json is utf8");
        assert!(!json.contains("D:\\"));
        assert!(!json.contains("/tmp/"));
        assert_eq!(
            ArcweftBundle::from_json_slice(&bytes).expect("bundle decodes"),
            bundle
        );
    }

    #[test]
    fn bundle_image_assets_resolve_encoded_virtual_file_bytes() {
        let image_file = BundleVirtualFile {
            space: BundleVirtualFileSpace::Asset,
            path: "images/logo.webp".to_owned(),
            bytes: b"webp-bytes".to_vec(),
        };
        let bundle = empty_test_bundle()
            .with_virtual_files([image_file.clone()])
            .with_image_assets([BundleImageAsset {
                id: "asset.view.logo".to_owned(),
                file: image_file.file_ref(),
                format: BundleImageFormat::WebP,
                animation: BundleImageAnimation::Animated,
                dimensions: Some(BundleImageDimensions::new(320, 180)),
            }]);

        let bytes = bundle.to_json_bytes().expect("bundle encodes");
        let json = String::from_utf8(bytes.clone()).expect("json is utf8");
        assert!(json.contains("\"image_assets\""));
        assert!(json.contains("\"format\": \"webp\""));
        let decoded = ArcweftBundle::from_json_slice(&bytes).expect("bundle decodes");

        let asset = decoded
            .image_asset("asset.view.logo")
            .expect("image asset is indexed");
        assert_eq!(asset.animation, BundleImageAnimation::Animated);
        assert_eq!(
            decoded
                .image_asset_bytes("asset.view.logo")
                .expect("image bytes resolve"),
            Some(b"webp-bytes".as_slice())
        );
        assert_eq!(
            decoded
                .image_asset_bytes("asset.view.missing")
                .expect("unknown asset is not an error"),
            None
        );
    }

    #[test]
    fn bundle_rejects_duplicate_image_asset_ids_instead_of_deduplicating() {
        let image_file = BundleVirtualFile {
            space: BundleVirtualFileSpace::Asset,
            path: "images/logo.webp".to_owned(),
            bytes: b"webp-bytes".to_vec(),
        };
        let asset = BundleImageAsset {
            id: "asset.view.logo".to_owned(),
            file: image_file.file_ref(),
            format: BundleImageFormat::WebP,
            animation: BundleImageAnimation::Animated,
            dimensions: Some(BundleImageDimensions::new(320, 180)),
        };
        let bundle = empty_test_bundle()
            .with_virtual_files([image_file])
            .with_image_assets([asset.clone(), asset]);

        let error = bundle
            .to_format_bytes(BundleFormat::Json)
            .expect_err("duplicate image assets reject");

        assert!(
            matches!(error, BundleCodecError::DuplicateImageAsset { id } if id == "asset.view.logo")
        );
    }

    #[test]
    fn bundle_rejects_duplicate_adapter_manifest_ids_instead_of_deduplicating() {
        let manifest = BundleAdapterManifest {
            id: "native-file".to_owned(),
            display_name: "Native File".to_owned(),
            effects: vec!["fs.read".to_owned()],
            host_calls: Vec::new(),
        };
        let bundle = empty_test_bundle().with_adapter_manifests([manifest.clone(), manifest]);

        let error = bundle
            .to_format_bytes(BundleFormat::Json)
            .expect_err("duplicate adapter manifests reject");

        assert!(
            matches!(error, BundleCodecError::DuplicateAdapterManifest { id } if id == "native-file")
        );
    }

    #[test]
    fn bundle_image_objects_round_trip_as_typed_metadata() {
        let expected = BundleImageObject {
            id: "image.hero.logo".to_owned(),
            asset: "asset.view.logo".to_owned(),
            target: Some("target.hero.logo".to_owned()),
            layer: Some("layer.foreground".to_owned()),
            view: None,
            containing_scroll_region: None,
            bounds: BundleImageObjectBounds::from_px(10, 20, 320, 180),
            placement: None,
            fit: BundleImageObjectFit::Cover,
            alignment: BundleImageObjectAlignment {
                x_milli: 250,
                y_milli: 750,
            },
            playback: BundleImageObjectPlayback {
                start_time_millis: 40,
                rate_milli: 500,
                paused_at_millis: None,
                pinned_local_time_millis: Some(160),
            },
            transform: BundleImageObjectTransform {
                m11_milli: 1_000,
                m12_milli: 0,
                m21_milli: 0,
                m22_milli: 1_000,
                tx_milli: 12_000,
                ty_milli: -3_000,
            },
            depth_milli: 2400,
            opacity_milli: 900,
            actions: vec!["action.inspect.logo".to_owned()],
            params: [(
                "param.role".to_owned(),
                BundleImageObjectParam::Text {
                    value: "hero".to_owned(),
                },
            )]
            .into(),
            proxies: vec![BundleImageObjectProxy {
                id: "proxy.logo.hotspot".to_owned(),
                type_name: Some("LogoHotspot".to_owned()),
                role: Some("inspect".to_owned()),
                layer: Some("layer.hit".to_owned()),
                depth_milli: Some(2600),
                hit_test: true,
                params: [(
                    "channel".to_owned(),
                    BundleImageObjectParam::Text {
                        value: "preview".to_owned(),
                    },
                )]
                .into(),
            }],
            visible: true,
        };
        let bundle = empty_test_bundle().with_image_objects([expected.clone()]);

        let bytes = bundle.to_json_bytes().expect("bundle encodes");
        let decoded = ArcweftBundle::from_json_slice(&bytes).expect("bundle decodes");

        assert_eq!(decoded.image_object("image.hero.logo"), Some(&expected));
    }

    #[test]
    fn bundle_image_asset_reports_missing_virtual_file() {
        let bundle = empty_test_bundle().with_image_assets([BundleImageAsset {
            id: "asset.bg.room".to_owned(),
            file: BundleVirtualFileRef {
                space: BundleVirtualFileSpace::Asset,
                path: "bg/room.png".to_owned(),
            },
            format: BundleImageFormat::Png,
            animation: BundleImageAnimation::Static,
            dimensions: None,
        }]);

        let error = bundle
            .image_asset_bytes("asset.bg.room")
            .expect_err("missing file is a structural bundle error");

        assert!(matches!(
            error,
            BundleCodecError::MissingImageFile {
                asset_id,
                space: BundleVirtualFileSpace::Asset,
                path,
            } if asset_id == "asset.bg.room" && path == "bg/room.png"
        ));
    }

    #[test]
    fn bundle_audio_graph_round_trips_and_resolves_asset_bytes() {
        let audio_file = BundleVirtualFile {
            space: BundleVirtualFileSpace::Asset,
            path: "audio/opening.wav".to_owned(),
            bytes: b"wav-bytes".to_vec(),
        };
        let master_bus = AudioBusId::new("bus.master").expect("bus id");
        let bundle = empty_test_bundle()
            .with_virtual_files([audio_file])
            .with_audio_graph(AudioGraph {
                master_bus: master_bus.clone(),
                assets: vec![AudioAsset {
                    id: AudioResourceId::new("asset.voice.opening").expect("audio asset id"),
                    path: "audio/opening.wav".to_owned(),
                    format: AudioFormat::Wav,
                    strategy: AudioDecodeStrategy::Preload,
                    default_loop: AudioLoopMode::None,
                }],
                buses: vec![AudioBusDef {
                    id: master_bus,
                    parent: None,
                    gain: GainDbMilli::UNITY,
                    muted: false,
                    effects: Vec::new(),
                }],
                snapshots: Vec::new(),
            });

        let bytes = bundle.to_json_bytes().expect("bundle encodes");
        let json = String::from_utf8(bytes.clone()).expect("json is utf8");
        assert!(json.contains("\"audio\""));
        assert!(json.contains("\"format\": \"wav\""));
        let decoded = ArcweftBundle::from_json_slice(&bytes).expect("bundle decodes");

        assert_eq!(
            decoded
                .audio_asset_bytes("asset.voice.opening")
                .expect("audio bytes resolve"),
            Some(b"wav-bytes".as_slice())
        );
        assert_eq!(
            decoded
                .audio_asset_bytes("asset.voice.missing")
                .expect("unknown audio asset is not an error"),
            None
        );
    }

    #[test]
    fn bundle_agent_manifest_marks_agent_controller_and_round_trips() {
        let bundle = empty_test_bundle().with_agent_manifest(test_agent_manifest());

        let bytes = bundle.to_json_bytes().expect("agent bundle encodes");
        let json = String::from_utf8(bytes.clone()).expect("json is utf8");
        assert!(json.contains("\"bundle_kind\": \"agent_controller\""));
        assert!(json.contains("\"agent\""));
        assert!(json.contains("\"declared_effects\""));
        assert!(json.contains("\"verified_effects\""));
        assert!(json.contains("\"semantic_hash\""));
        assert!(!json.contains("\"type_fingerprint\""));

        let decoded = ArcweftBundle::from_json_slice(&bytes).expect("agent bundle decodes");
        assert_eq!(decoded.bundle_kind, BundleKind::AgentController);
        assert_eq!(
            decoded
                .agent
                .as_ref()
                .map(|manifest| manifest.entry_id.as_str()),
            Some("entry.agent.opening_smoke")
        );
        assert_eq!(decoded, bundle);
    }

    #[test]
    fn bundle_agent_kind_requires_agent_manifest() {
        let mut bundle = empty_test_bundle();
        bundle.bundle_kind = BundleKind::AgentController;

        let error = bundle
            .to_json_bytes()
            .expect_err("agent controller bundle requires manifest");

        assert!(matches!(error, BundleCodecError::MissingAgentManifest));
    }

    #[test]
    fn bundle_game_kind_rejects_agent_manifest() {
        let mut bundle = empty_test_bundle().with_agent_manifest(test_agent_manifest());
        bundle.bundle_kind = BundleKind::Game;

        let error = bundle
            .to_json_bytes()
            .expect_err("game bundle cannot carry agent manifest");

        assert!(matches!(error, BundleCodecError::UnexpectedAgentManifest));
    }

    #[test]
    fn bundle_format_codecs_round_trip_supported_formats() {
        let bundle = empty_test_bundle().with_virtual_files([BundleVirtualFile {
            space: BundleVirtualFileSpace::Asset,
            path: "dialogue/opening.txt".to_owned(),
            bytes: b"hello".to_vec(),
        }]);

        for format in BundleFormat::enabled_formats() {
            let bytes = bundle
                .to_format_bytes(format)
                .unwrap_or_else(|error| panic!("{format} encodes: {error}"));
            let decoded = ArcweftBundle::from_format_slice(format, &bytes)
                .unwrap_or_else(|error| panic!("{format} decodes: {error}"));
            assert_eq!(decoded, bundle, "{format} should round-trip bundle data");
        }
    }

    #[test]
    fn awfb_decodes_runtime_types_layout_independent_from_awbc_payload() {
        let bytes = awfb_with_runtime_types_layout_signature(
            &empty_test_bundle(),
            "arcweft.bytecode.runtime-layout.v0.test",
        );

        let decoded = ArcweftBundle::from_format_slice(BundleFormat::Awfb, &bytes)
            .expect("AWBC product AWFB decodes");

        assert_eq!(
            decoded.bytecode.program.runtime_layout.signature,
            "arcweft.bytecode.runtime-layout.v0.test"
        );
        assert!(decoded.product_awbc().is_some());
    }

    #[test]
    fn bundle_format_can_be_inferred_from_common_extensions() {
        assert_eq!(
            BundleFormat::from_path(Path::new("game.toml")),
            Some(BundleFormat::Toml)
        );
        assert_eq!(
            BundleFormat::from_path(Path::new("game.yaml")),
            Some(BundleFormat::Yaml)
        );
        assert_eq!(
            BundleFormat::from_path(Path::new("game.msgpack")),
            Some(BundleFormat::MessagePack)
        );
        assert_eq!(
            BundleFormat::from_path(Path::new("game.avro")),
            Some(BundleFormat::Avro)
        );
        assert_eq!(
            BundleFormat::from_path(Path::new("game.awfb")),
            Some(BundleFormat::Awfb)
        );
        assert_eq!(
            BundleFormat::from_path(Path::new("game.awfb.json")),
            Some(BundleFormat::Json)
        );
    }

    #[test]
    fn awfb_path_does_not_fall_back_to_json_decoder() {
        let bytes = empty_test_bundle()
            .to_json_bytes()
            .expect("legacy JSON bundle encodes");
        let error = ArcweftBundle::from_inspection_path_slice(Path::new("game.awfb"), &bytes)
            .expect_err("AWFB product path must require AWFB bytes");

        assert!(
            matches!(error, BundleCodecError::DecodeAwfb { message } if message.contains("magic"))
        );
    }

    #[test]
    fn product_path_requires_awfb_extension() {
        let bytes = empty_test_bundle()
            .to_json_bytes()
            .expect("inspection JSON bundle encodes");
        let error = ArcweftBundle::from_product_path_slice(Path::new("game.awfb.json"), &bytes)
            .expect_err("product decode must not accept inspection JSON paths");

        assert!(matches!(
            error,
            BundleCodecError::ExpectedProductAwfbPath { .. }
        ));
    }

    #[test]
    fn inspection_path_requires_explicit_json_format_extension() {
        let bytes = empty_test_bundle()
            .to_json_bytes()
            .expect("inspection JSON bundle encodes");
        let error = ArcweftBundle::from_inspection_path_slice(Path::new("game"), &bytes)
            .expect_err("inspection decode must not probe legacy JSON for unknown paths");

        assert!(matches!(error, BundleCodecError::UnsupportedFormat { .. }));
    }

    #[test]
    fn inspection_json_path_decodes_bundle_export() {
        let bundle = empty_test_bundle();
        let bytes = bundle
            .to_json_bytes()
            .expect("inspection JSON bundle encodes");
        let decoded =
            ArcweftBundle::from_inspection_path_slice(Path::new("game.awfb.json"), &bytes)
                .expect("explicit inspection JSON path decodes");

        assert_eq!(decoded, bundle);
    }

    #[test]
    fn tampered_dialogue_text_parameter_role_is_rejected() {
        let mut bundle = empty_test_bundle();
        let program = bundle.view_program.as_mut().expect("standard View program");
        program.action_buttons.clear();
        program.definitions[0].parameters[0].role =
            crate::resource_codec::view::ViewParameterRole::Value;

        let error = ArcweftBundle::from_json_slice(
            &serde_json::to_vec(&bundle).expect("tampered bundle serializes"),
        )
        .expect_err("incorrect dialogue parameter role must be rejected");

        assert!(matches!(
            error,
            BundleCodecError::InvalidDialogueViewContract(
                DialogueViewContractError::InvalidTextParameterRole { .. }
            )
        ));
    }

    #[test]
    fn serialized_dialogue_parameter_role_is_required() {
        let bundle = empty_test_bundle();
        let mut value = serde_json::to_value(bundle).expect("bundle serializes");
        value["view_program"]["definitions"][0]["parameters"][0]
            .as_object_mut()
            .expect("parameter is an object")
            .remove("role");

        let error = ArcweftBundle::from_json_slice(
            &serde_json::to_vec(&value).expect("tampered bundle serializes"),
        )
        .expect_err("missing dialogue parameter role must be rejected");

        assert!(matches!(error, BundleCodecError::Decode(_)));
    }

    #[test]
    fn tampered_dialogue_projection_surface_is_rejected() {
        let mut bundle = empty_test_bundle();
        let content_source = bundle
            .view_text
            .as_ref()
            .expect("standard View text")
            .sources
            .iter()
            .find_map(|source| {
                matches!(
                    source.kind,
                    ViewTextSourceKind::Dialogue {
                        projection: crate::resource_codec::view::DialogueTextProjection::Content,
                        ..
                    }
                )
                .then(|| source.public_id.clone())
            })
            .expect("content projection");
        let block = bundle
            .view_program
            .as_mut()
            .expect("standard View program")
            .text_blocks
            .iter_mut()
            .find(|block| block.text_source == content_source)
            .expect("content text block");
        block.surface = crate::resource_codec::view::ViewTextSurface::Text;

        let error = ArcweftBundle::from_json_slice(
            &serde_json::to_vec(&bundle).expect("tampered bundle serializes"),
        )
        .expect_err("incorrect dialogue projection surface must be rejected");

        assert!(matches!(
            error,
            BundleCodecError::InvalidDialogueViewContract(
                DialogueViewContractError::TextSurfaceMismatch { .. }
            )
        ));
    }

    #[test]
    fn tampered_dialogue_primary_action_parameter_is_rejected() {
        let mut bundle = empty_test_bundle();
        let action = &mut bundle
            .view_program
            .as_mut()
            .expect("standard View program")
            .action_buttons[0]
            .action;
        *action =
            crate::resource_codec::view::ViewActionButtonActionResource::DialoguePrimaryAction {
                parameter: "not_dialogue".to_owned(),
            };

        let error = ArcweftBundle::from_json_slice(
            &serde_json::to_vec(&bundle).expect("tampered bundle serializes"),
        )
        .expect_err("incorrect primary action parameter must be rejected");

        assert!(matches!(
            error,
            BundleCodecError::InvalidDialogueViewContract(
                DialogueViewContractError::InvalidActionParameterRole { .. }
            )
        ));
    }

    fn empty_test_bundle() -> ArcweftBundle {
        ArcweftBundle::try_new(
            BundleManifest {
                profile_id: None,
                profile_kind: None,
                entry: Some("main".to_owned()),
                adapter: None,
                adapter_manifest_ids: Vec::new(),
                required_host_calls: Vec::new(),
                runtime: BundleRuntimeSummary {
                    entry_flow: Some("flow.main".to_owned()),
                    flows: 1,
                    bytecode_instructions: 0,
                    line_task_groups: 0,
                    stream_plans: 0,
                    source_plans: 0,
                },
            },
            source_map("main.arcw", "flow @flow.main main { return \"ok\" }"),
            BytecodeProgram::default(),
            LineDisplayCatalog::default(),
        )
        .expect("standard dialogue source joins source map")
        .with_product_awbc(minimal_awbc_program())
    }

    fn source_map(label: &str, text: &str) -> SourceMapSection {
        let document = SourceDocument::try_new(
            SourceDocumentId::try_new(label).expect("source ID"),
            SourceName::path(label),
            text,
        )
        .expect("source document");
        SourceMapSection::try_from_documents(&[&document]).expect("source map")
    }

    fn minimal_awbc_program() -> AwbcProgram {
        AwbcProgram {
            strings: vec!["entry.main".to_owned()],
            signatures: vec![AwbcSignature {
                params: Vec::new(),
                result: None,
                effects: AwbcEffectSetId(0),
            }],
            frame_layouts: vec![AwbcFrameLayout {
                slots: Vec::new(),
                max_scope_depth: 0,
            }],
            functions: vec![AwbcFunction {
                public_id: Some(AwbcStringId(0)),
                kind: AwbcFunctionKind::Flow,
                signature: AwbcSignatureId(0),
                frame_layout: AwbcFrameLayoutId(0),
                blocks: AwbcTableRange::new(0, 1),
                entry_block: AwbcBlockId(0),
                flags: AwbcFunctionFlags(AwbcFunctionFlags::DETERMINISTIC),
            }],
            blocks: vec![AwbcBlock {
                owner: AwbcFunctionId(0),
                instructions: AwbcTableRange::new(0, 0),
                terminator: AwbcTerminator::Return { value: None },
                safe_point: AwbcSafePointKind::FlowEntry,
                source_map: None,
            }],
            entries: vec![AwbcEntry {
                runtime_id: arcweft_core::plan::EntryRuntimeId::from_source_entity_body(
                    "entry.main",
                )
                .expect("test entry ID is valid"),
                binding: arcweft_core::entry::EntryBindingIdentity::from_bytes([1; 32]),
                public_id: AwbcStringId(0),
                kind: AwbcEntryKind::Cli,
                signature: AwbcSignatureId(0),
                target: AwbcEntryTarget::Function(AwbcFunctionId(0)),
                roles: arcweft_core::entry::RuntimeEntryRoles::None,
            }],
            ..AwbcProgram::default()
        }
    }

    fn awfb_with_runtime_types_layout_signature(
        bundle: &ArcweftBundle,
        signature: &str,
    ) -> Vec<u8> {
        let bytes = bundle
            .to_format_bytes(BundleFormat::Awfb)
            .expect("AWFB bundle encodes");
        let view = BundleView::parse(&bytes, ReadBudget::default()).expect("AWFB parses");
        let sections = view
            .sections()
            .iter()
            .map(|descriptor| {
                let decoded = if descriptor.kind() == ContainerSectionKind::RuntimeTypes {
                    let mut section = RuntimeTypesSection::from_bundle(bundle)
                        .expect("runtime types section builds");
                    section.runtime_layout.signature = signature.to_owned();
                    section
                        .encode_canonical_section()
                        .expect("runtime types section encodes")
                } else {
                    view.decoded_section(descriptor.id())
                        .expect("section decodes")
                        .expect("product AWFB section is embedded")
                };
                SectionInput::embedded(
                    descriptor.id(),
                    descriptor.kind(),
                    descriptor.schema_version(),
                    descriptor.residency(),
                    descriptor.required(),
                    decoded,
                )
            })
            .collect::<Vec<_>>();

        encode_bundle(view.kind(), view.manifest(), sections).expect("AWFB re-encodes")
    }

    fn test_agent_manifest() -> AgentArtifactManifest {
        AgentArtifactManifest {
            schema_version: 1,
            bundle_kind: AgentBundleKind::AgentController,
            entry_id: public_id("entry.agent.opening_smoke"),
            controller_id: CallableId::new("test::crate.opening_smoke")
                .expect("test callable id is nonempty"),
            entry_binding_hash: stable_hash("blake3:entry-binding"),
            controller_contract_hash: stable_hash("blake3:controller-contract"),
            policy_hash: stable_hash("blake3:agent-policy"),
            source_hash: stable_hash("sha256:agent-source"),
            compiler_version: "arcweft-test".to_owned(),
            project_binding: ProjectBinding {
                program_hash: stable_hash("sha256:program"),
                mode: ProjectBindingMode::Strict,
                required_entities: vec![RequiredEntity {
                    public_id: public_id("choice.opening.listen"),
                    kind: "choice_option".to_owned(),
                    semantic_hash: stable_hash("type:none"),
                    source_anchor: None,
                }],
            },
            declared_effects: vec![
                EffectCapability::new("agent.observe"),
                EffectCapability::new("agent.act.semantic"),
            ],
            verified_effects: VerifiedEffectSummary::new(
                1,
                vec![
                    EffectCapability::new("agent.observe"),
                    EffectCapability::new("agent.act.semantic"),
                ],
                vec![EffectCapability::new("agent.observe")],
                stable_hash("blake3:agent-effects"),
            ),
            budget: AgentBudget::default(),
            debug_map_hash: Some(stable_hash("sha256:debug-map")),
        }
    }

    fn public_id(value: &str) -> PublicId {
        PublicId::new(value).expect("test public id is nonempty")
    }

    fn stable_hash(value: &str) -> StableHash {
        StableHash::new(value).expect("test hash is nonempty")
    }
}
