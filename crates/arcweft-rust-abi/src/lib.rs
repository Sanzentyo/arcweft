//! Typed, Sans I/O metadata for Rust APIs exported to Arcweft adapters.
//!
//! The root is a deliberate data-model facade. Identity validation, recursive
//! model validation, and presentation formatting live in responsibility
//! modules; adapters own discovery and all filesystem or Cargo interaction.

mod display;
mod identity;
mod model;
mod producer;
mod validation;

pub use identity::{
    ArcweftRustIdentityError, ArcweftRustPackageId, ArcweftRustTypeParameterIndex,
    ArcweftRustTypeParameterName, ArcweftRustTypePath, ArcweftRustTypePathSegment,
};
pub use model::{
    ARCWEFT_RUST_ABI_SCHEMA_VERSION, ArcweftRustField, ArcweftRustFunction, ArcweftRustManifest,
    ArcweftRustManifestBuilder, ArcweftRustPackage, ArcweftRustParam, ArcweftRustPurity,
    ArcweftRustStructShape, ArcweftRustTypeDecl, ArcweftRustTypeKind, ArcweftRustTypeParameter,
    ArcweftRustTypeRef, ArcweftRustVariant, ArcweftRustVariantPayload, ArcweftType,
    ArcweftTypeMetadata,
};
pub use producer::{ArcweftRustOpaqueTypeProducerId, ArcweftRustOpaqueTypeProducerIdError};
pub use validation::{
    ArcweftRustAbiLimits, ArcweftRustManifestError, ArcweftRustTypeSite, ArcweftRustTypeSiteRoot,
    ArcweftRustTypeSiteStep,
};

use thiserror::Error;

impl ArcweftRustManifest {
    /// Creates an empty manifest for one package.
    pub fn new(package: ArcweftRustPackage) -> Self {
        Self {
            schema_version: ARCWEFT_RUST_ABI_SCHEMA_VERSION,
            package,
            types: Vec::new(),
            functions: Vec::new(),
        }
    }

    /// Starts a manifest builder for one package.
    pub fn builder(package: ArcweftRustPackage) -> ArcweftRustManifestBuilder {
        ArcweftRustManifestBuilder::new(package)
    }

    /// Appends one exported type declaration.
    #[must_use]
    pub fn with_type(mut self, ty: ArcweftRustTypeDecl) -> Self {
        self.types.push(ty);
        self
    }

    /// Appends one exported function declaration.
    #[must_use]
    pub fn with_function(mut self, function: ArcweftRustFunction) -> Self {
        self.functions.push(function);
        self
    }

    /// Encodes one validated manifest as deterministic pretty JSON.
    pub fn to_json_pretty(&self) -> Result<String, ArcweftRustAbiError> {
        self.validate(ArcweftRustAbiLimits::PRODUCTION)?;
        Ok(serde_json::to_string_pretty(self)?)
    }

    /// Decodes, version-checks, and validates one final-schema manifest.
    pub fn from_json(source: &str) -> Result<Self, ArcweftRustAbiError> {
        let schema_version = preflight_schema_version(source)?;
        if schema_version != ARCWEFT_RUST_ABI_SCHEMA_VERSION {
            return Err(ArcweftRustAbiError::UnsupportedSchema {
                found: schema_version,
                expected: ARCWEFT_RUST_ABI_SCHEMA_VERSION,
            });
        }
        let value: serde_json::Value = serde_json::from_str(source)?;
        validate_type_producers(&value)?;
        let manifest = serde_json::from_value::<RustManifestDto>(value)?.into_manifest()?;
        manifest.validate(ArcweftRustAbiLimits::PRODUCTION)?;
        Ok(manifest)
    }
}

impl ArcweftRustManifestBuilder {
    /// Creates an empty manifest builder for one package.
    pub fn new(package: ArcweftRustPackage) -> Self {
        Self {
            manifest: ArcweftRustManifest::new(package),
        }
    }

    /// Appends one exported type declaration.
    #[must_use]
    pub fn with_type(mut self, ty: ArcweftRustTypeDecl) -> Self {
        self.manifest.types.push(ty);
        self
    }

    /// Appends metadata for a Rust ADT implementing `ArcweftTypeMetadata`.
    #[must_use]
    pub fn with_type_metadata<T: ArcweftTypeMetadata>(self) -> Self {
        self.with_type(T::arcweft_type_decl())
    }

    /// Appends one exported function declaration.
    #[must_use]
    pub fn with_function(mut self, function: ArcweftRustFunction) -> Self {
        self.manifest.functions.push(function);
        self
    }

    /// Finishes the typed model. Call `validate` before registration.
    pub fn build(self) -> ArcweftRustManifest {
        self.manifest
    }
}

/// Rust ABI metadata codec and validation errors.
#[derive(Debug, Error)]
pub enum ArcweftRustAbiError {
    #[error("failed to encode or decode Rust ABI metadata: {0}")]
    Json(#[from] serde_json::Error),
    #[error("missing Rust ABI schema_version")]
    MissingSchemaVersion,
    #[error("malformed Rust ABI schema_version: {problem}")]
    MalformedSchemaVersion {
        problem: ArcweftRustSchemaHeaderProblem,
    },
    #[error("unsupported Rust ABI schema {found}; expected {expected}")]
    UnsupportedSchema { found: u32, expected: u32 },
    #[error("Rust type declaration {site:?} is missing opaque_producer")]
    MissingOpaqueProducer { site: ArcweftRustTypeFieldSite },
    #[error("Rust type declaration {site:?} has malformed opaque_producer {found:?}")]
    MalformedOpaqueProducer {
        site: ArcweftRustTypeFieldSite,
        found: ArcweftRustJsonValueKind,
    },
    #[error("Rust type declaration {site:?} has invalid opaque producer: {error}")]
    InvalidOpaqueProducer {
        site: ArcweftRustTypeFieldSite,
        error: ArcweftRustOpaqueTypeProducerIdError,
    },
    #[error(transparent)]
    Manifest(#[from] ArcweftRustManifestError),
}

/// JSON value category used by Rust ABI header and producer diagnostics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArcweftRustJsonValueKind {
    Null,
    Boolean,
    Integer,
    IntegerOutOfRange,
    Float,
    String,
    Array,
    Object,
}

/// Header-specific failure detected before Rust ABI body decoding.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ArcweftRustSchemaHeaderProblem {
    #[error("manifest root must be an object")]
    RootNotObject,
    #[error("schema_version appears more than once")]
    DuplicateSchemaVersion,
    #[error("schema_version has wrong value kind {found:?}")]
    WrongType { found: ArcweftRustJsonValueKind },
    #[error("schema_version integer is outside u32")]
    IntegerOutOfRange,
}

/// Stable authored location of a Rust type declaration producer field.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArcweftRustTypeFieldSite {
    declaration: usize,
}

impl ArcweftRustTypeFieldSite {
    pub const fn declaration(self) -> usize {
        self.declaration
    }
}

#[derive(serde::Deserialize)]
struct RustManifestDto {
    schema_version: u32,
    package: ArcweftRustPackage,
    #[serde(default)]
    types: Vec<RustTypeDeclDto>,
    #[serde(default)]
    functions: Vec<ArcweftRustFunction>,
}

#[derive(serde::Deserialize)]
struct RustTypeDeclDto {
    path: ArcweftRustTypePath,
    rust_path: String,
    opaque_producer: String,
    #[serde(default)]
    parameters: Vec<ArcweftRustTypeParameter>,
    kind: ArcweftRustTypeKind,
}

impl RustManifestDto {
    fn into_manifest(self) -> Result<ArcweftRustManifest, ArcweftRustAbiError> {
        let types = self
            .types
            .into_iter()
            .enumerate()
            .map(|(declaration, declaration_dto)| {
                let opaque_producer =
                    ArcweftRustOpaqueTypeProducerId::try_new(declaration_dto.opaque_producer)
                        .map_err(|error| ArcweftRustAbiError::InvalidOpaqueProducer {
                            site: ArcweftRustTypeFieldSite { declaration },
                            error,
                        })?;
                Ok(ArcweftRustTypeDecl {
                    path: declaration_dto.path,
                    rust_path: declaration_dto.rust_path,
                    opaque_producer,
                    parameters: declaration_dto.parameters,
                    kind: declaration_dto.kind,
                })
            })
            .collect::<Result<Vec<_>, ArcweftRustAbiError>>()?;
        Ok(ArcweftRustManifest {
            schema_version: self.schema_version,
            package: self.package,
            types,
            functions: self.functions,
        })
    }
}

fn preflight_schema_version(source: &str) -> Result<u32, ArcweftRustAbiError> {
    let mut deserializer = serde_json::Deserializer::from_str(source);
    let header =
        serde::de::Deserializer::deserialize_any(&mut deserializer, RustSchemaHeaderVisitor)
            .map_err(ArcweftRustAbiError::Json)?;
    deserializer.end().map_err(ArcweftRustAbiError::Json)?;
    header
}

struct RustSchemaHeaderVisitor;

impl<'de> serde::de::Visitor<'de> for RustSchemaHeaderVisitor {
    type Value = Result<u32, ArcweftRustAbiError>;

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("a Rust ABI manifest object")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: serde::de::MapAccess<'de>,
    {
        let mut version = None;
        while let Some(key) = map.next_key::<String>()? {
            if key == "schema_version" {
                if version.is_some() {
                    return Ok(Err(ArcweftRustAbiError::MalformedSchemaVersion {
                        problem: ArcweftRustSchemaHeaderProblem::DuplicateSchemaVersion,
                    }));
                }
                let value = map.next_value::<serde_json::Value>()?;
                version = Some(match json_u32(&value) {
                    Ok(value) => Ok(value),
                    Err(problem) => Err(ArcweftRustAbiError::MalformedSchemaVersion { problem }),
                });
            } else {
                map.next_value::<serde::de::IgnoredAny>()?;
            }
        }
        Ok(version.unwrap_or(Err(ArcweftRustAbiError::MissingSchemaVersion)))
    }

    fn visit_bool<E>(self, _: bool) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(Err(ArcweftRustAbiError::MalformedSchemaVersion {
            problem: ArcweftRustSchemaHeaderProblem::RootNotObject,
        }))
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        self.visit_bool(false)
    }

    fn visit_i64<E>(self, _: i64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        self.visit_bool(false)
    }

    fn visit_u64<E>(self, _: u64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        self.visit_bool(false)
    }

    fn visit_f64<E>(self, _: f64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        self.visit_bool(false)
    }

    fn visit_str<E>(self, _: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        self.visit_bool(false)
    }

    fn visit_seq<A>(self, _: A) -> Result<Self::Value, A::Error>
    where
        A: serde::de::SeqAccess<'de>,
    {
        Ok(Err(ArcweftRustAbiError::MalformedSchemaVersion {
            problem: ArcweftRustSchemaHeaderProblem::RootNotObject,
        }))
    }
}

fn json_u32(value: &serde_json::Value) -> Result<u32, ArcweftRustSchemaHeaderProblem> {
    match value {
        serde_json::Value::Number(number) => number
            .as_u64()
            .and_then(|value| u32::try_from(value).ok())
            .ok_or(ArcweftRustSchemaHeaderProblem::IntegerOutOfRange),
        value => Err(ArcweftRustSchemaHeaderProblem::WrongType {
            found: json_value_kind(value),
        }),
    }
}

fn json_value_kind(value: &serde_json::Value) -> ArcweftRustJsonValueKind {
    match value {
        serde_json::Value::Null => ArcweftRustJsonValueKind::Null,
        serde_json::Value::Bool(_) => ArcweftRustJsonValueKind::Boolean,
        serde_json::Value::Number(number) if number.is_i64() || number.is_u64() => {
            if number
                .as_u64()
                .is_some_and(|value| value > u64::from(u32::MAX))
                || number.as_i64().is_some_and(|value| value < 0)
            {
                ArcweftRustJsonValueKind::IntegerOutOfRange
            } else {
                ArcweftRustJsonValueKind::Integer
            }
        }
        serde_json::Value::Number(_) => ArcweftRustJsonValueKind::Float,
        serde_json::Value::String(_) => ArcweftRustJsonValueKind::String,
        serde_json::Value::Array(_) => ArcweftRustJsonValueKind::Array,
        serde_json::Value::Object(_) => ArcweftRustJsonValueKind::Object,
    }
}

fn validate_type_producers(value: &serde_json::Value) -> Result<(), ArcweftRustAbiError> {
    let Some(types) = value.as_object().and_then(|object| object.get("types")) else {
        return Ok(());
    };
    let Some(types) = types.as_array() else {
        return Ok(());
    };
    for (declaration, type_value) in types.iter().enumerate() {
        let Some(type_object) = type_value.as_object() else {
            continue;
        };
        let site = ArcweftRustTypeFieldSite { declaration };
        let Some(producer) = type_object.get("opaque_producer") else {
            return Err(ArcweftRustAbiError::MissingOpaqueProducer { site });
        };
        let Some(producer) = producer.as_str() else {
            return Err(ArcweftRustAbiError::MalformedOpaqueProducer {
                site,
                found: json_value_kind(producer),
            });
        };
        ArcweftRustOpaqueTypeProducerId::try_new(producer)
            .map_err(|error| ArcweftRustAbiError::InvalidOpaqueProducer { site, error })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
