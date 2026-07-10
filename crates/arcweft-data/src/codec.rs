use std::{collections::BTreeSet, sync::Arc};

use crate::error::{DataError, DataErrorKind, Result};
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
    /// Every built-in format in stable source-facing order.
    ///
    /// Consumers that enumerate formats should use this authoritative list
    /// instead of maintaining a parallel inventory.
    pub const ALL: [Self; 10] = [
        Self::Json,
        Self::Toml,
        Self::Yaml,
        Self::MessagePack,
        Self::Cbor,
        Self::Avro,
        Self::Csv,
        Self::ArrowIpc,
        Self::Parquet,
        Self::ArcweftBinary,
    ];

    /// Stable codec identifier used by codec registries and adapters.
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

    /// Canonical media type for the format.
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

    /// Arcweft source variant name, without the `DataFormat.` prefix.
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

    /// Resolves a canonical Arcweft source variant name.
    #[must_use]
    pub fn from_variant_name(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|format| format.variant_name() == value)
    }

    /// Resolves a canonical codec identifier.
    #[must_use]
    pub fn from_id(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|format| format.id() == value)
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

    pub fn with(mut self, codec: impl Codec + 'static) -> Result<Self> {
        self.register(codec)?;
        Ok(self)
    }

    pub fn register(&mut self, codec: impl Codec + 'static) -> Result<()> {
        self.register_arc(Arc::new(codec))
    }

    pub fn register_arc(&mut self, codec: Arc<dyn Codec>) -> Result<()> {
        self.validate_registration(codec.as_ref())?;
        self.codecs.push(codec);
        Ok(())
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
        let normalized = normalize_media_type(media_type)?;
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

    fn validate_registration(&self, codec: &dyn Codec) -> Result<()> {
        let id = codec.id();
        if id.as_str().trim().is_empty() {
            return Err(DataError::new(
                DataErrorKind::InvalidEncoding,
                "codec id must not be empty",
            ));
        }
        if self
            .codecs
            .iter()
            .any(|existing| existing.id().as_str() == id.as_str())
        {
            return Err(duplicate_registration("codec id", id.as_str()));
        }

        let media_types = codec
            .media_types()
            .iter()
            .map(|media_type| normalize_media_type(media_type))
            .collect::<Result<Vec<_>>>()?;
        reject_duplicate_items("media type", media_types.iter().map(String::as_str))?;
        if let Some(media_type) = media_types
            .iter()
            .find(|media_type| self.has_media_type(media_type))
        {
            return Err(duplicate_registration("media type", media_type));
        }

        let extensions = codec
            .file_extensions()
            .iter()
            .map(|extension| normalize_extension(extension))
            .collect::<Result<Vec<_>>>()?;
        reject_duplicate_items("file extension", extensions.iter().map(String::as_str))?;
        if let Some(extension) = extensions
            .iter()
            .find(|extension| self.has_file_extension(extension))
        {
            return Err(duplicate_registration("file extension", extension));
        }
        Ok(())
    }

    fn has_media_type(&self, media_type: &str) -> bool {
        self.codecs.iter().any(|codec| {
            codec
                .media_types()
                .iter()
                .filter_map(|candidate| normalize_media_type(candidate).ok())
                .any(|candidate| candidate == media_type)
        })
    }

    fn has_file_extension(&self, extension: &str) -> bool {
        self.codecs.iter().any(|codec| {
            codec
                .file_extensions()
                .iter()
                .filter_map(|candidate| normalize_extension(candidate).ok())
                .any(|candidate| candidate == extension)
        })
    }
}

fn normalize_media_type(media_type: &str) -> Result<String> {
    let normalized = media_type
        .split(';')
        .next()
        .unwrap_or(media_type)
        .trim()
        .to_ascii_lowercase();
    let valid = normalized
        .split_once('/')
        .is_some_and(|(type_, subtype)| !type_.is_empty() && !subtype.is_empty());
    if valid {
        Ok(normalized)
    } else {
        Err(DataError::new(
            DataErrorKind::InvalidEncoding,
            format!("invalid media type `{media_type}`"),
        ))
    }
}

fn normalize_extension(extension: &str) -> Result<String> {
    let normalized = extension
        .trim()
        .trim_start_matches('.')
        .to_ascii_lowercase();
    if normalized.is_empty() {
        Err(DataError::new(
            DataErrorKind::InvalidEncoding,
            "file extension must not be empty",
        ))
    } else {
        Ok(normalized)
    }
}

fn reject_duplicate_items<'a>(
    label: &'static str,
    items: impl Iterator<Item = &'a str>,
) -> Result<()> {
    let mut seen = BTreeSet::new();
    items
        .filter(|item| !seen.insert(*item))
        .map(|item| duplicate_registration(label, item))
        .next()
        .map_or(Ok(()), Err)
}

fn duplicate_registration(label: &'static str, value: &str) -> DataError {
    DataError::new(
        DataErrorKind::DuplicateField,
        format!("duplicate codec {label} `{value}`"),
    )
}
