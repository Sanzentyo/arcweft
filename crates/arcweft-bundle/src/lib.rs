//! Sans I/O bundle data model and deterministic codecs.

use apache_avro::types::Value as AvroValue;
use apache_avro::{Reader, Schema, Writer};
use arcweft_agent_protocol::artifact::AgentArtifactManifest;
use arcweft_core::bytecode::BytecodeProgram;
use arcweft_data::{BytesFormat, Codec, DecodeOptions, EncodeOptions, TypeShape};
use arcweft_render_text::LineDisplayCatalog;
use serde::{Deserialize, Serialize};
use std::io::Cursor;
use std::path::Path;
use thiserror::Error;

pub const ARCWEFT_BUNDLE_SCHEMA_VERSION: u32 = 2;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArcweftBundle {
    pub schema_version: u32,
    #[serde(default)]
    pub bundle_kind: BundleKind,
    pub manifest: BundleManifest,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<AgentArtifactManifest>,
    pub source: BundleSource,
    pub bytecode: BundleBytecodeProgram,
    pub display: LineDisplayCatalog,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub adapter_manifests: Vec<BundleAdapterManifest>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub virtual_files: Vec<BundleVirtualFile>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub image_assets: Vec<BundleImageAsset>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BundleManifest {
    pub source_label: String,
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

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BundleSource {
    pub label: String,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BundleBytecodeProgram {
    pub encoding: BundleBytecodeEncoding,
    pub program: BytecodeProgram,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BundleBytecodeEncoding {
    StructuredJson,
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BundleFormat {
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
    Cli,
    Server,
    Test,
    Bench,
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
    #[error("bundle image asset `{asset_id}` references missing virtual file {space}:{path}")]
    MissingImageFile {
        asset_id: String,
        space: BundleVirtualFileSpace,
        path: String,
    },
}

impl ArcweftBundle {
    pub fn new(
        manifest: BundleManifest,
        source: BundleSource,
        bytecode: BytecodeProgram,
        display: LineDisplayCatalog,
    ) -> Self {
        Self {
            schema_version: ARCWEFT_BUNDLE_SCHEMA_VERSION,
            bundle_kind: BundleKind::Game,
            manifest,
            agent: None,
            source,
            bytecode: BundleBytecodeProgram {
                encoding: BundleBytecodeEncoding::StructuredJson,
                program: bytecode,
            },
            display,
            adapter_manifests: Vec::new(),
            virtual_files: Vec::new(),
            image_assets: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_adapter_manifests(
        mut self,
        manifests: impl IntoIterator<Item = BundleAdapterManifest>,
    ) -> Self {
        self.adapter_manifests.extend(manifests);
        self.adapter_manifests
            .sort_by(|left, right| left.id.cmp(&right.id));
        self.adapter_manifests
            .dedup_by(|left, right| left.id == right.id);
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
        self.image_assets
            .dedup_by(|left, right| left.id == right.id);
        self
    }

    #[must_use]
    pub fn with_agent_manifest(mut self, manifest: AgentArtifactManifest) -> Self {
        self.bundle_kind = BundleKind::AgentController;
        self.agent = Some(manifest);
        self
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
            BundleFormat::Json => self.to_json_bytes(),
            BundleFormat::Toml => toml::to_string_pretty(self)
                .map(String::into_bytes)
                .map_err(|error| BundleCodecError::EncodeFormat {
                    format,
                    message: error.to_string(),
                }),
            BundleFormat::Yaml => {
                let value = arcweft_serde_bridge::to_arcweft_value(self).map_err(|error| {
                    BundleCodecError::EncodeFormat {
                        format,
                        message: error.to_string(),
                    }
                })?;
                arcweft_codec_yaml::YamlCodec
                    .encode_value(
                        &value,
                        &TypeShape::Named("ArcweftBundle".to_owned()),
                        &EncodeOptions {
                            pretty: true,
                            bytes_format: BytesFormat::Array,
                        },
                    )
                    .map_err(|error| BundleCodecError::EncodeFormat {
                        format,
                        message: error.to_string(),
                    })
            }
            BundleFormat::MessagePack => {
                rmp_serde::to_vec_named(self).map_err(|error| BundleCodecError::EncodeFormat {
                    format,
                    message: error.to_string(),
                })
            }
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
            BundleFormat::Avro => self.to_avro_envelope_bytes(),
        }
    }

    pub fn from_format_slice(format: BundleFormat, bytes: &[u8]) -> Result<Self, BundleCodecError> {
        let bundle = match format {
            BundleFormat::Json => return Self::from_json_slice(bytes),
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
            BundleFormat::Yaml => {
                let value = arcweft_codec_yaml::YamlCodec
                    .decode_value(
                        bytes,
                        &TypeShape::Named("ArcweftBundle".to_owned()),
                        &DecodeOptions::default(),
                    )
                    .map_err(|error| BundleCodecError::DecodeFormat {
                        format,
                        message: error.to_string(),
                    })?;
                arcweft_serde_bridge::from_arcweft_value(&value).map_err(|error| {
                    BundleCodecError::DecodeFormat {
                        format,
                        message: error.to_string(),
                    }
                })?
            }
            BundleFormat::MessagePack => {
                rmp_serde::from_slice(bytes).map_err(|error| BundleCodecError::DecodeFormat {
                    format,
                    message: error.to_string(),
                })?
            }
            BundleFormat::Cbor => {
                ciborium::de::from_reader(Cursor::new(bytes)).map_err(|error| {
                    BundleCodecError::DecodeFormat {
                        format,
                        message: error.to_string(),
                    }
                })?
            }
            BundleFormat::Avro => Self::from_avro_envelope_slice(bytes)?,
        };
        bundle.validate_schema_and_kind()?;
        Ok(bundle)
    }

    pub fn from_path_slice(path: &Path, bytes: &[u8]) -> Result<Self, BundleCodecError> {
        let Some(format) = BundleFormat::from_path(path) else {
            return Self::from_json_slice(bytes).or_else(|json_error| {
                Self::from_format_slice(BundleFormat::MessagePack, bytes)
                    .or_else(|_| Self::from_format_slice(BundleFormat::Cbor, bytes))
                    .or_else(|_| Self::from_format_slice(BundleFormat::Avro, bytes))
                    .map_err(|_| json_error)
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

    fn validate_kind(&self) -> Result<(), BundleCodecError> {
        match (self.bundle_kind, self.agent.is_some()) {
            (BundleKind::AgentController, false) => Err(BundleCodecError::MissingAgentManifest),
            (BundleKind::AgentController, true) | (BundleKind::Game, false) => Ok(()),
            (BundleKind::Game, true) => Err(BundleCodecError::UnexpectedAgentManifest),
        }
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
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::Toml => "toml",
            Self::Yaml => "yaml",
            Self::MessagePack => "msgpack",
            Self::Cbor => "cbor",
            Self::Avro => "avro",
        }
    }

    pub fn parse(value: &str) -> Result<Self, BundleCodecError> {
        match value.trim().to_ascii_lowercase().as_str() {
            "json" => Ok(Self::Json),
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
    use arcweft_agent_protocol::{
        artifact::{
            AgentArtifactManifest, AgentBudget, AgentBundleKind, EffectCapability, ProjectBinding,
            ProjectBindingMode, RequiredEntity,
        },
        ids::{PublicId, StableHash},
        verified_effects::VerifiedEffectSummary,
    };

    #[test]
    fn bundle_json_round_trips_without_paths() {
        let bundle = ArcweftBundle::new(
            BundleManifest {
                source_label: "main.arcw".to_owned(),
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
            BundleSource {
                label: "main.arcw".to_owned(),
                text: "flow @flow.main main { return \"ok\" }".to_owned(),
            },
            BytecodeProgram::default(),
            LineDisplayCatalog::default(),
        )
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
                id: "asset.ui.logo".to_owned(),
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
            .image_asset("asset.ui.logo")
            .expect("image asset is indexed");
        assert_eq!(asset.animation, BundleImageAnimation::Animated);
        assert_eq!(
            decoded
                .image_asset_bytes("asset.ui.logo")
                .expect("image bytes resolve"),
            Some(b"webp-bytes".as_slice())
        );
        assert_eq!(
            decoded
                .image_asset_bytes("asset.ui.missing")
                .expect("unknown asset is not an error"),
            None
        );
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
                .map(|manifest| manifest.agent_id.as_str()),
            Some("agent.opening_smoke")
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

        for format in [
            BundleFormat::Json,
            BundleFormat::Toml,
            BundleFormat::Yaml,
            BundleFormat::MessagePack,
            BundleFormat::Cbor,
            BundleFormat::Avro,
        ] {
            let bytes = bundle
                .to_format_bytes(format)
                .unwrap_or_else(|error| panic!("{format} encodes: {error}"));
            let decoded = ArcweftBundle::from_format_slice(format, &bytes)
                .unwrap_or_else(|error| panic!("{format} decodes: {error}"));
            assert_eq!(decoded, bundle, "{format} should round-trip bundle data");
        }
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
    }

    fn empty_test_bundle() -> ArcweftBundle {
        ArcweftBundle::new(
            BundleManifest {
                source_label: "main.arcw".to_owned(),
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
            BundleSource {
                label: "main.arcw".to_owned(),
                text: "flow @flow.main main { return \"ok\" }".to_owned(),
            },
            BytecodeProgram::default(),
            LineDisplayCatalog::default(),
        )
    }

    fn test_agent_manifest() -> AgentArtifactManifest {
        AgentArtifactManifest {
            schema_version: 1,
            bundle_kind: AgentBundleKind::AgentController,
            agent_id: public_id("agent.opening_smoke"),
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
