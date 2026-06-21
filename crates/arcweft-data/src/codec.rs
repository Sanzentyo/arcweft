use std::sync::Arc;

use crate::error::{DataError, Result};
use crate::limits::DecodeLimits;
use crate::shape::{BytesFormat, TypeShape};
use crate::value::Value;

/// Built-in data codec format selected by Arcweft source and runtime APIs.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DataFormat {
    Json,
    Toml,
    Yaml,
    MessagePack,
    Cbor,
    Avro,
    Csv,
    ArrowIpc,
    Parquet,
    ArcweftBinary,
}

impl DataFormat {
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::Toml => "toml",
            Self::Yaml => "yaml",
            Self::MessagePack => "msgpack",
            Self::Cbor => "cbor",
            Self::Avro => "avro",
            Self::Csv => "csv",
            Self::ArrowIpc => "arrow-ipc",
            Self::Parquet => "parquet",
            Self::ArcweftBinary => "arcweft-binary",
        }
    }

    #[must_use]
    pub const fn media_type(self) -> &'static str {
        match self {
            Self::Json => "application/json",
            Self::Toml => "application/toml",
            Self::Yaml => "application/yaml",
            Self::MessagePack => "application/msgpack",
            Self::Cbor => "application/cbor",
            Self::Avro => "application/avro",
            Self::Csv => "text/csv",
            Self::ArrowIpc => "application/vnd.apache.arrow.stream",
            Self::Parquet => "application/vnd.apache.parquet",
            Self::ArcweftBinary => "application/vnd.arcweft.binary",
        }
    }

    #[must_use]
    pub const fn variant_name(self) -> &'static str {
        match self {
            Self::Json => "Json",
            Self::Toml => "Toml",
            Self::Yaml => "Yaml",
            Self::MessagePack => "MessagePack",
            Self::Cbor => "Cbor",
            Self::Avro => "Avro",
            Self::Csv => "Csv",
            Self::ArrowIpc => "ArrowIpc",
            Self::Parquet => "Parquet",
            Self::ArcweftBinary => "ArcweftBinary",
        }
    }

    pub fn from_variant_name(value: &str) -> Option<Self> {
        Some(match value {
            "Json" => Self::Json,
            "Toml" => Self::Toml,
            "Yaml" => Self::Yaml,
            "MessagePack" => Self::MessagePack,
            "Cbor" => Self::Cbor,
            "Avro" => Self::Avro,
            "Csv" => Self::Csv,
            "ArrowIpc" => Self::ArrowIpc,
            "Parquet" => Self::Parquet,
            "ArcweftBinary" => Self::ArcweftBinary,
            _ => return None,
        })
    }

    pub fn from_id(value: &str) -> Option<Self> {
        Some(match value {
            "json" => Self::Json,
            "toml" => Self::Toml,
            "yaml" | "yml" => Self::Yaml,
            "msgpack" | "messagepack" => Self::MessagePack,
            "cbor" => Self::Cbor,
            "avro" => Self::Avro,
            "csv" => Self::Csv,
            "arrow-ipc" | "arrow" => Self::ArrowIpc,
            "parquet" => Self::Parquet,
            "arcweft-binary" | "arcweft_binary" => Self::ArcweftBinary,
            _ => return None,
        })
    }
}

/// Stable codec identifier, e.g. `json`, `yaml`, `arrow-ipc`.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FormatId(String);

impl FormatId {
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for FormatId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

/// HTTP-style media type label used by adapters.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MediaType(String);

impl MediaType {
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into().to_ascii_lowercase())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for MediaType {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

/// Encode-time cross-format options.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EncodeOptions {
    pub pretty: bool,
    pub bytes_format: BytesFormat,
}

impl Default for EncodeOptions {
    fn default() -> Self {
        Self {
            pretty: false,
            bytes_format: BytesFormat::Base64,
        }
    }
}

/// Decode-time cross-format options.
#[derive(Clone, Debug, Eq, PartialEq, Default)]
pub struct DecodeOptions {
    pub limits: DecodeLimits,
}

/// Object-safe codec boundary. Concrete formats live outside the builtin crate.
pub trait Codec: Send + Sync {
    fn id(&self) -> FormatId;

    fn media_types(&self) -> &'static [&'static str];

    fn file_extensions(&self) -> &'static [&'static str] {
        &[]
    }

    fn encode_value(
        &self,
        value: &Value,
        shape: &TypeShape,
        options: &EncodeOptions,
    ) -> Result<Vec<u8>>;

    fn decode_value(
        &self,
        input: &[u8],
        shape: &TypeShape,
        options: &DecodeOptions,
    ) -> Result<Value>;
}

/// Runtime codec registry used by save/config/http adapters.
#[derive(Clone, Default)]
pub struct CodecRegistry {
    codecs: Vec<Arc<dyn Codec>>,
}

impl CodecRegistry {
    #[must_use]
    pub const fn new() -> Self {
        Self { codecs: Vec::new() }
    }

    #[must_use]
    pub fn with(mut self, codec: impl Codec + 'static) -> Self {
        self.register(codec);
        self
    }

    pub fn register(&mut self, codec: impl Codec + 'static) {
        self.codecs.push(Arc::new(codec));
    }

    pub fn register_arc(&mut self, codec: Arc<dyn Codec>) {
        self.codecs.push(codec);
    }

    pub fn iter(&self) -> impl Iterator<Item = &Arc<dyn Codec>> {
        self.codecs.iter()
    }

    pub fn by_id(&self, id: &str) -> Result<Arc<dyn Codec>> {
        self.codecs
            .iter()
            .find(|codec| codec.id().as_str() == id)
            .cloned()
            .ok_or_else(|| DataError::unsupported(format!("codec '{id}' is not registered")))
    }

    pub fn by_media_type(&self, media_type: &str) -> Result<Arc<dyn Codec>> {
        let normalized = media_type
            .split(';')
            .next()
            .unwrap_or(media_type)
            .trim()
            .to_ascii_lowercase();
        self.codecs
            .iter()
            .find(|codec| {
                codec
                    .media_types()
                    .iter()
                    .any(|candidate| candidate.eq_ignore_ascii_case(&normalized))
            })
            .cloned()
            .ok_or_else(|| {
                DataError::unsupported(format!("media type '{media_type}' is not registered"))
            })
    }
}
