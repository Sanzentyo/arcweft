//! Exact data codecs for project-local adapter manifests.

use crate::manifest::{
    AdapterCallableGroupIndex, AdapterCallableModelError, AdapterCallableName,
    AdapterCallableOverloadIndex, AdapterCallableParameterIndex, AdapterCallablePath,
    AdapterEffectCapability, AdapterEnvironmentOwnerId, AdapterFreeCallableKind,
    AdapterFunctionParam, AdapterFunctionSignature, AdapterHostCall, AdapterManifest,
    AdapterManifestModelError, AdapterNominalDeclaration, AdapterNominalOwner, AdapterNominalPath,
    AdapterNominalPathError, AdapterNominalPathPrefix, AdapterNominalPathSegment,
    AdapterNominalTypeRef, AdapterNominalVisibility, AdapterOpaqueTypeProducerId,
    AdapterOpaqueTypeProducerIdError, AdapterParameterGroup, AdapterParameterPassing,
    AdapterParameterPresence, AdapterSymbol, AdapterSymbolPath, AdapterSymbolPathError,
    AdapterSymbolSegment, AdapterToolingDoc, AdapterToolingSubject, AdapterTypeKind,
    AdapterTypeModelError,
};
use arcweft_rust_abi::{ArcweftRustIdentityError, ArcweftRustPackageId};
use serde::{
    Deserialize, Deserializer, Serialize,
    de::{IgnoredAny, MapAccess, Visitor},
};
use std::fmt;
use thiserror::Error;

/// Current stable project-local adapter manifest schema version.
pub const ADAPTER_MANIFEST_SCHEMA_VERSION: u32 = 1;

const MAX_TYPE_DEPTH: usize = 256;
const MAX_TYPE_NODES: usize = 4_096;

/// Serializable final-shape adapter manifest file.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AdapterManifestFile {
    schema_version: u32,
    id: String,
    display_name: String,
    #[serde(default)]
    nominal_types: Vec<AdapterNominalDeclarationFile>,
    #[serde(default)]
    rust_package_mounts: Vec<AdapterRustPackageMountFile>,
    #[serde(default)]
    symbols: Vec<AdapterSymbolFile>,
    #[serde(default)]
    methods: Vec<AdapterMethodFile>,
    #[serde(default)]
    functions: Vec<AdapterFunctionFile>,
    #[serde(default)]
    effects: Vec<String>,
    #[serde(default)]
    host_calls: Vec<AdapterHostCallFile>,
    #[serde(default)]
    tooling_docs: Vec<AdapterToolingDocFile>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
struct AdapterManifestWire {
    schema_version: u32,
    id: String,
    display_name: String,
    #[serde(default)]
    nominal_types: Vec<AdapterNominalDeclarationWire>,
    #[serde(default)]
    rust_package_mounts: Vec<AdapterRustPackageMountFile>,
    #[serde(default)]
    symbols: Vec<AdapterSymbolFile>,
    #[serde(default)]
    methods: Vec<AdapterMethodFile>,
    #[serde(default)]
    functions: Vec<AdapterFunctionFile>,
    #[serde(default)]
    effects: Vec<String>,
    #[serde(default)]
    host_calls: Vec<AdapterHostCallFile>,
    #[serde(default)]
    tooling_docs: Vec<AdapterToolingDocFile>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct AdapterNominalDeclarationFile {
    path: Vec<String>,
    arity: u16,
    opaque_producer: AdapterOpaqueTypeProducerId,
    visibility: AdapterNominalVisibility,
    source_label: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
struct AdapterNominalDeclarationWire {
    path: Vec<String>,
    arity: u16,
    opaque_producer: String,
    visibility: AdapterNominalVisibility,
    source_label: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct AdapterRustPackageMountFile {
    package: String,
    prefix: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct AdapterSymbolFile {
    name: String,
    #[serde(rename = "type")]
    ty: AdapterTypeKindFile,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct AdapterMethodFile {
    receiver: AdapterTypeKindFile,
    name: String,
    signature: AdapterFunctionSignatureFile,
    #[serde(default)]
    effects: Vec<String>,
    #[serde(default)]
    overload: u16,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct AdapterFunctionFile {
    name: String,
    signature: AdapterFunctionSignatureFile,
    #[serde(default)]
    effects: Vec<String>,
    #[serde(default)]
    overload: u16,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct AdapterFunctionSignatureFile {
    groups: Vec<AdapterParameterGroupFile>,
    result: AdapterTypeKindFile,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct AdapterParameterGroupFile {
    index: u16,
    #[serde(default)]
    parameters: Vec<AdapterParamFile>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct AdapterParamFile {
    index: u16,
    #[serde(default)]
    name: Option<String>,
    #[serde(rename = "type")]
    ty: AdapterTypeKindFile,
    passing: AdapterParameterPassingFile,
    presence: AdapterParameterPresenceFile,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum AdapterParameterPassingFile {
    PositionalOrNamed,
    PositionalOnly,
    NamedOnly,
    RestPositional,
    RestNamed,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum AdapterParameterPresenceFile {
    Required,
    Defaulted,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct AdapterHostCallFile {
    id: String,
    signature: AdapterFunctionSignatureFile,
    #[serde(default)]
    domain_error: Option<AdapterTypeKindFile>,
    #[serde(default)]
    effects: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct AdapterToolingDocFile {
    subject: String,
    docs: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum AdapterTypeKindFile {
    Unit {},
    Bool {},
    I8 {},
    I16 {},
    I32 {},
    I64 {},
    I128 {},
    ISize {},
    U8 {},
    U16 {},
    U32 {},
    U64 {},
    U128 {},
    USize {},
    F32 {},
    F64 {},
    String {},
    Char {},
    Vec { item: Box<Self> },
    Seq { item: Box<Self> },
    Option { item: Box<Self> },
    Result { ok: Box<Self>, error: Box<Self> },
    Tuple { items: Vec<Self> },
    Need { item: Box<Self> },
    Nominal { nominal: AdapterNominalTypeRefFile },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct AdapterNominalTypeRefFile {
    owner: AdapterNominalOwnerFile,
    path: Vec<String>,
    #[serde(default)]
    arguments: Vec<AdapterTypeKindFile>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum AdapterNominalOwnerFile {
    Standard {},
    Environment { owner: String },
    RustPackage { package: String },
}

/// Serialized source syntax used for one adapter manifest.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdapterManifestSourceFormat {
    Json,
    Toml,
}

/// Stable classification of an adapter manifest value at an invalid field.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdapterManifestValueKind {
    Null,
    Boolean,
    Integer,
    IntegerOutOfRange,
    Float,
    String,
    Array,
    Object,
}

/// Exact structural defect in the schema header.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum AdapterManifestSchemaHeaderProblem {
    #[error("manifest root must be an object/table")]
    RootNotObject,
    #[error("schema_version appears more than once")]
    DuplicateSchemaVersion,
    #[error("schema_version has wrong value kind {found:?}")]
    WrongType { found: AdapterManifestValueKind },
    #[error("schema_version integer is outside u32")]
    IntegerOutOfRange,
}

/// Stable authored location of one adapter-native producer field.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdapterManifestFieldSite {
    format: AdapterManifestSourceFormat,
    nominal_index: usize,
}

impl AdapterManifestFieldSite {
    pub const fn format(self) -> AdapterManifestSourceFormat {
        self.format
    }

    pub const fn nominal_index(self) -> usize {
        self.nominal_index
    }
}

/// Errors while parsing project-local adapter manifests.
#[derive(Debug, Error)]
pub enum AdapterManifestCodecError {
    #[error("failed to parse adapter manifest JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("failed to parse adapter manifest TOML: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("adapter manifest {format:?} is missing schema_version")]
    MissingSchemaVersion { format: AdapterManifestSourceFormat },
    #[error("adapter manifest {format:?} has malformed schema_version: {problem}")]
    MalformedSchemaVersion {
        format: AdapterManifestSourceFormat,
        problem: AdapterManifestSchemaHeaderProblem,
    },
    #[error("unsupported adapter manifest {format:?} schema {found}, expected {expected}")]
    UnsupportedSchema {
        format: AdapterManifestSourceFormat,
        found: u32,
        expected: u32,
    },
    #[error("adapter manifest nominal row {site:?} is missing opaque_producer")]
    MissingOpaqueProducer { site: AdapterManifestFieldSite },
    #[error("adapter manifest nominal row {site:?} has malformed opaque_producer kind {found:?}")]
    MalformedOpaqueProducer {
        site: AdapterManifestFieldSite,
        found: AdapterManifestValueKind,
    },
    #[error("adapter manifest nominal row {site:?} has invalid opaque_producer: {error}")]
    InvalidOpaqueProducer {
        site: AdapterManifestFieldSite,
        #[source]
        error: AdapterOpaqueTypeProducerIdError,
    },
    #[error(transparent)]
    Model(#[from] AdapterCallableModelError),
    #[error(transparent)]
    Manifest(#[from] AdapterManifestModelError),
    #[error(transparent)]
    NominalPath(#[from] AdapterNominalPathError),
    #[error(transparent)]
    TypeModel(#[from] AdapterTypeModelError),
    #[error(transparent)]
    RustIdentity(#[from] ArcweftRustIdentityError),
    #[error(transparent)]
    SymbolPath(#[from] AdapterSymbolPathError),
    #[error("adapter nominal owner `{actual}` does not match manifest owner `{expected}`")]
    EnvironmentOwnerMismatch { expected: String, actual: String },
}

impl AdapterManifestFile {
    /// Parses a JSON adapter manifest.
    pub fn from_json(source: &str) -> Result<Self, AdapterManifestCodecError> {
        let value = serde_json::from_str::<serde_json::Value>(source)?;
        let format = AdapterManifestSourceFormat::Json;
        let version = json_schema_version(source, &value)?;
        validate_schema_version(format, version)?;
        validate_json_opaque_producers(&value)?;
        let file = serde_json::from_value::<AdapterManifestWire>(value)?;
        Self::from_wire(file, format)
    }

    /// Parses a TOML adapter manifest.
    pub fn from_toml(source: &str) -> Result<Self, AdapterManifestCodecError> {
        let value = toml::from_str::<toml::Value>(source)?;
        let format = AdapterManifestSourceFormat::Toml;
        let version = toml_schema_version(&value)?;
        validate_schema_version(format, version)?;
        validate_toml_opaque_producers(&value)?;
        let file = value.try_into::<AdapterManifestWire>()?;
        Self::from_wire(file, format)
    }

    /// Manifest schema version parsed from the source file.
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// Converts final file data into the validated manifest used by registration.
    pub fn into_manifest(self) -> Result<AdapterManifest, AdapterManifestCodecError> {
        let mut manifest = AdapterManifest::new(self.id, self.display_name);
        let environment_owner = AdapterEnvironmentOwnerId::for_adapter(manifest.id());

        for mount in self.rust_package_mounts {
            manifest = manifest.try_with_rust_package_mount(
                ArcweftRustPackageId::try_new(mount.package)?,
                nominal_prefix(mount.prefix)?,
            )?;
        }
        for declaration in self.nominal_types {
            manifest =
                manifest.try_with_nominal_declaration(AdapterNominalDeclaration::try_new(
                    nominal_path(declaration.path)?,
                    declaration.arity,
                    declaration.opaque_producer,
                    declaration.visibility,
                    declaration.source_label,
                )?)?;
        }
        for symbol in self.symbols {
            manifest = manifest.with_symbol(AdapterSymbol::new(
                symbol_path_from_file(&symbol.name)?,
                adapter_type_from_file(symbol.ty, &environment_owner)?,
            ));
        }
        for method in self.methods {
            manifest = manifest.with_method_signature(
                adapter_type_from_file(method.receiver, &environment_owner)?,
                AdapterCallableName::try_new(method.name)?,
                AdapterCallableOverloadIndex::try_from_usize(usize::from(method.overload))?,
                signature_from_file(method.signature, &environment_owner)?,
                effect_capabilities(method.effects),
            );
        }
        for function in self.functions {
            manifest = manifest.with_function_signature(
                callable_path_from_file(&function.name)?,
                AdapterCallableOverloadIndex::try_from_usize(usize::from(function.overload))?,
                signature_from_file(function.signature, &environment_owner)?,
                effect_capabilities(function.effects),
            );
        }
        for effect in self.effects {
            manifest = manifest.with_effect(AdapterEffectCapability::new(effect));
        }
        for host_call in self.host_calls {
            let AdapterHostCallFile {
                id,
                signature,
                domain_error,
                effects,
            } = host_call;
            let host_call = AdapterHostCall::with_signature(
                id,
                signature_from_file(signature, &environment_owner)?,
                effect_capabilities(effects),
            );
            let host_call = match domain_error {
                Some(domain_error) => host_call
                    .with_domain_error(adapter_type_from_file(domain_error, &environment_owner)?),
                None => host_call,
            };
            manifest = manifest.with_host_call(host_call);
        }
        for doc in self.tooling_docs {
            manifest = manifest.with_tooling_doc(AdapterToolingDoc::try_new(
                AdapterToolingSubject::Free {
                    kind: AdapterFreeCallableKind::Function,
                    path: callable_path_from_file(&doc.subject)?,
                    overload: AdapterCallableOverloadIndex::try_from_usize(0)?,
                },
                Some(doc.docs),
                None,
                Vec::new(),
            )?);
        }
        Ok(manifest)
    }

    fn from_wire(
        file: AdapterManifestWire,
        format: AdapterManifestSourceFormat,
    ) -> Result<Self, AdapterManifestCodecError> {
        let nominal_types = file
            .nominal_types
            .into_iter()
            .enumerate()
            .map(|(nominal_index, declaration)| {
                let site = AdapterManifestFieldSite {
                    format,
                    nominal_index,
                };
                Ok(AdapterNominalDeclarationFile {
                    path: declaration.path,
                    arity: declaration.arity,
                    opaque_producer: AdapterOpaqueTypeProducerId::try_new(
                        declaration.opaque_producer,
                    )
                    .map_err(|error| {
                        AdapterManifestCodecError::InvalidOpaqueProducer { site, error }
                    })?,
                    visibility: declaration.visibility,
                    source_label: declaration.source_label,
                })
            })
            .collect::<Result<Vec<_>, AdapterManifestCodecError>>()?;
        Ok(Self {
            schema_version: file.schema_version,
            id: file.id,
            display_name: file.display_name,
            nominal_types,
            rust_package_mounts: file.rust_package_mounts,
            symbols: file.symbols,
            methods: file.methods,
            functions: file.functions,
            effects: file.effects,
            host_calls: file.host_calls,
            tooling_docs: file.tooling_docs,
        })
    }
}

fn validate_schema_version(
    format: AdapterManifestSourceFormat,
    found: u32,
) -> Result<(), AdapterManifestCodecError> {
    if found == ADAPTER_MANIFEST_SCHEMA_VERSION {
        Ok(())
    } else {
        Err(AdapterManifestCodecError::UnsupportedSchema {
            format,
            found,
            expected: ADAPTER_MANIFEST_SCHEMA_VERSION,
        })
    }
}

fn json_schema_version(
    source: &str,
    value: &serde_json::Value,
) -> Result<u32, AdapterManifestCodecError> {
    if !value.is_object() {
        return Err(AdapterManifestCodecError::MalformedSchemaVersion {
            format: AdapterManifestSourceFormat::Json,
            problem: AdapterManifestSchemaHeaderProblem::RootNotObject,
        });
    }
    let mut deserializer = serde_json::Deserializer::from_str(source);
    deserializer
        .deserialize_map(JsonSchemaHeaderVisitor)
        .map_err(AdapterManifestCodecError::Json)?
}

struct JsonSchemaHeaderVisitor;

impl<'de> Visitor<'de> for JsonSchemaHeaderVisitor {
    type Value = Result<u32, AdapterManifestCodecError>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("an adapter manifest JSON object")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut schema = None;
        let mut duplicate = false;
        while let Some(key) = map.next_key::<String>()? {
            if key == "schema_version" {
                let value = map.next_value::<serde_json::Value>()?;
                if schema.is_some() {
                    duplicate = true;
                } else {
                    schema = Some(json_u32_header(&value));
                }
            } else {
                map.next_value::<IgnoredAny>()?;
            }
        }
        if duplicate {
            return Ok(Err(AdapterManifestCodecError::MalformedSchemaVersion {
                format: AdapterManifestSourceFormat::Json,
                problem: AdapterManifestSchemaHeaderProblem::DuplicateSchemaVersion,
            }));
        }
        Ok(
            schema.unwrap_or(Err(AdapterManifestCodecError::MissingSchemaVersion {
                format: AdapterManifestSourceFormat::Json,
            })),
        )
    }
}

fn json_u32_header(value: &serde_json::Value) -> Result<u32, AdapterManifestCodecError> {
    let problem = match value {
        serde_json::Value::Number(number) => {
            if let Some(value) = number.as_u64() {
                return u32::try_from(value).map_err(|_| {
                    AdapterManifestCodecError::MalformedSchemaVersion {
                        format: AdapterManifestSourceFormat::Json,
                        problem: AdapterManifestSchemaHeaderProblem::IntegerOutOfRange,
                    }
                });
            }
            if number.as_i64().is_some() {
                AdapterManifestSchemaHeaderProblem::IntegerOutOfRange
            } else {
                AdapterManifestSchemaHeaderProblem::WrongType {
                    found: AdapterManifestValueKind::Float,
                }
            }
        }
        other => AdapterManifestSchemaHeaderProblem::WrongType {
            found: json_value_kind(other),
        },
    };
    Err(AdapterManifestCodecError::MalformedSchemaVersion {
        format: AdapterManifestSourceFormat::Json,
        problem,
    })
}

fn toml_schema_version(value: &toml::Value) -> Result<u32, AdapterManifestCodecError> {
    let Some(table) = value.as_table() else {
        return Err(AdapterManifestCodecError::MalformedSchemaVersion {
            format: AdapterManifestSourceFormat::Toml,
            problem: AdapterManifestSchemaHeaderProblem::RootNotObject,
        });
    };
    let Some(value) = table.get("schema_version") else {
        return Err(AdapterManifestCodecError::MissingSchemaVersion {
            format: AdapterManifestSourceFormat::Toml,
        });
    };
    let Some(integer) = value.as_integer() else {
        return Err(AdapterManifestCodecError::MalformedSchemaVersion {
            format: AdapterManifestSourceFormat::Toml,
            problem: AdapterManifestSchemaHeaderProblem::WrongType {
                found: toml_value_kind(value),
            },
        });
    };
    u32::try_from(integer).map_err(|_| AdapterManifestCodecError::MalformedSchemaVersion {
        format: AdapterManifestSourceFormat::Toml,
        problem: AdapterManifestSchemaHeaderProblem::IntegerOutOfRange,
    })
}

fn validate_json_opaque_producers(
    root: &serde_json::Value,
) -> Result<(), AdapterManifestCodecError> {
    let Some(rows) = root
        .as_object()
        .and_then(|object| object.get("nominal_types"))
        .and_then(serde_json::Value::as_array)
    else {
        return Ok(());
    };
    let producers = rows
        .iter()
        .enumerate()
        .filter_map(|(nominal_index, row)| {
            row.as_object().map(|row| {
                let site = AdapterManifestFieldSite {
                    format: AdapterManifestSourceFormat::Json,
                    nominal_index,
                };
                let producer = row
                    .get("opaque_producer")
                    .ok_or(AdapterManifestCodecError::MissingOpaqueProducer { site })?;
                let producer = producer.as_str().ok_or(
                    AdapterManifestCodecError::MalformedOpaqueProducer {
                        site,
                        found: json_value_kind(producer),
                    },
                )?;
                Ok((site, producer))
            })
        })
        .collect::<Result<Vec<_>, AdapterManifestCodecError>>()?;
    validate_opaque_producer_spellings(&producers)
}

fn validate_toml_opaque_producers(root: &toml::Value) -> Result<(), AdapterManifestCodecError> {
    let Some(rows) = root
        .as_table()
        .and_then(|table| table.get("nominal_types"))
        .and_then(toml::Value::as_array)
    else {
        return Ok(());
    };
    let producers = rows
        .iter()
        .enumerate()
        .filter_map(|(nominal_index, row)| {
            row.as_table().map(|row| {
                let site = AdapterManifestFieldSite {
                    format: AdapterManifestSourceFormat::Toml,
                    nominal_index,
                };
                let producer = row
                    .get("opaque_producer")
                    .ok_or(AdapterManifestCodecError::MissingOpaqueProducer { site })?;
                let producer = producer.as_str().ok_or(
                    AdapterManifestCodecError::MalformedOpaqueProducer {
                        site,
                        found: toml_value_kind(producer),
                    },
                )?;
                Ok((site, producer))
            })
        })
        .collect::<Result<Vec<_>, AdapterManifestCodecError>>()?;
    validate_opaque_producer_spellings(&producers)
}

fn validate_opaque_producer_spellings(
    producers: &[(AdapterManifestFieldSite, &str)],
) -> Result<(), AdapterManifestCodecError> {
    for &(site, producer) in producers {
        match AdapterOpaqueTypeProducerId::try_new(producer) {
            Err(
                error @ (AdapterOpaqueTypeProducerIdError::Empty
                | AdapterOpaqueTypeProducerIdError::ControlCharacter { .. }),
            ) => {
                return Err(AdapterManifestCodecError::InvalidOpaqueProducer { site, error });
            }
            Ok(_) | Err(AdapterOpaqueTypeProducerIdError::ReservedStandardNamespace { .. }) => {}
        }
    }
    for &(site, producer) in producers {
        if let Err(error @ AdapterOpaqueTypeProducerIdError::ReservedStandardNamespace { .. }) =
            AdapterOpaqueTypeProducerId::try_new(producer)
        {
            return Err(AdapterManifestCodecError::InvalidOpaqueProducer { site, error });
        }
    }
    Ok(())
}

fn json_value_kind(value: &serde_json::Value) -> AdapterManifestValueKind {
    match value {
        serde_json::Value::Null => AdapterManifestValueKind::Null,
        serde_json::Value::Bool(_) => AdapterManifestValueKind::Boolean,
        serde_json::Value::Number(number) if number.is_i64() || number.is_u64() => {
            AdapterManifestValueKind::Integer
        }
        serde_json::Value::Number(_) => AdapterManifestValueKind::Float,
        serde_json::Value::String(_) => AdapterManifestValueKind::String,
        serde_json::Value::Array(_) => AdapterManifestValueKind::Array,
        serde_json::Value::Object(_) => AdapterManifestValueKind::Object,
    }
}

fn toml_value_kind(value: &toml::Value) -> AdapterManifestValueKind {
    match value {
        toml::Value::String(_) | toml::Value::Datetime(_) => AdapterManifestValueKind::String,
        toml::Value::Integer(_) => AdapterManifestValueKind::Integer,
        toml::Value::Float(_) => AdapterManifestValueKind::Float,
        toml::Value::Boolean(_) => AdapterManifestValueKind::Boolean,
        toml::Value::Array(_) => AdapterManifestValueKind::Array,
        toml::Value::Table(_) => AdapterManifestValueKind::Object,
    }
}

fn signature_from_file(
    signature: AdapterFunctionSignatureFile,
    environment_owner: &AdapterEnvironmentOwnerId,
) -> Result<AdapterFunctionSignature, AdapterManifestCodecError> {
    let groups = signature
        .groups
        .into_iter()
        .map(|group| {
            let parameters = group
                .parameters
                .into_iter()
                .map(|parameter| {
                    AdapterFunctionParam::try_new(
                        AdapterCallableParameterIndex::try_from_usize(usize::from(
                            parameter.index,
                        ))?,
                        parameter
                            .name
                            .map(AdapterCallableName::try_new)
                            .transpose()?,
                        adapter_type_from_file(parameter.ty, environment_owner)?,
                        parameter.passing.into(),
                        parameter.presence.into(),
                    )
                    .map_err(AdapterManifestCodecError::from)
                })
                .collect::<Result<Vec<_>, _>>()?;
            AdapterParameterGroup::try_new(
                AdapterCallableGroupIndex::try_from_usize(usize::from(group.index))?,
                parameters,
            )
            .map_err(AdapterManifestCodecError::from)
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(AdapterFunctionSignature::try_new(
        groups,
        adapter_type_from_file(signature.result, environment_owner)?,
    )?)
}

impl From<AdapterParameterPassingFile> for AdapterParameterPassing {
    fn from(value: AdapterParameterPassingFile) -> Self {
        match value {
            AdapterParameterPassingFile::PositionalOrNamed => Self::PositionalOrNamed,
            AdapterParameterPassingFile::PositionalOnly => Self::PositionalOnly,
            AdapterParameterPassingFile::NamedOnly => Self::NamedOnly,
            AdapterParameterPassingFile::RestPositional => Self::RestPositional,
            AdapterParameterPassingFile::RestNamed => Self::RestNamed,
        }
    }
}

impl From<AdapterParameterPresenceFile> for AdapterParameterPresence {
    fn from(value: AdapterParameterPresenceFile) -> Self {
        match value {
            AdapterParameterPresenceFile::Required => Self::Required,
            AdapterParameterPresenceFile::Defaulted => Self::Defaulted,
        }
    }
}

fn adapter_type_from_file(
    ty: AdapterTypeKindFile,
    environment_owner: &AdapterEnvironmentOwnerId,
) -> Result<AdapterTypeKind, AdapterManifestCodecError> {
    let mut budget = TypeConversionBudget { nodes: 0 };
    budget.convert(ty, environment_owner, 1)
}

struct TypeConversionBudget {
    nodes: usize,
}

impl TypeConversionBudget {
    fn convert(
        &mut self,
        ty: AdapterTypeKindFile,
        environment_owner: &AdapterEnvironmentOwnerId,
        depth: usize,
    ) -> Result<AdapterTypeKind, AdapterManifestCodecError> {
        if depth > MAX_TYPE_DEPTH {
            return Err(AdapterTypeModelError::DepthLimit {
                observed: depth,
                maximum: MAX_TYPE_DEPTH,
            }
            .into());
        }
        self.nodes = self
            .nodes
            .checked_add(1)
            .ok_or(AdapterTypeModelError::NodeLimit {
                observed: usize::MAX,
                maximum: MAX_TYPE_NODES,
            })?;
        if self.nodes > MAX_TYPE_NODES {
            return Err(AdapterTypeModelError::NodeLimit {
                observed: self.nodes,
                maximum: MAX_TYPE_NODES,
            }
            .into());
        }

        let child_depth = depth.saturating_add(1);
        Ok(match ty {
            AdapterTypeKindFile::Unit {} => AdapterTypeKind::Unit,
            AdapterTypeKindFile::Bool {} => AdapterTypeKind::Bool,
            AdapterTypeKindFile::I8 {} => AdapterTypeKind::I8,
            AdapterTypeKindFile::I16 {} => AdapterTypeKind::I16,
            AdapterTypeKindFile::I32 {} => AdapterTypeKind::I32,
            AdapterTypeKindFile::I64 {} => AdapterTypeKind::I64,
            AdapterTypeKindFile::I128 {} => AdapterTypeKind::I128,
            AdapterTypeKindFile::ISize {} => AdapterTypeKind::ISize,
            AdapterTypeKindFile::U8 {} => AdapterTypeKind::U8,
            AdapterTypeKindFile::U16 {} => AdapterTypeKind::U16,
            AdapterTypeKindFile::U32 {} => AdapterTypeKind::U32,
            AdapterTypeKindFile::U64 {} => AdapterTypeKind::U64,
            AdapterTypeKindFile::U128 {} => AdapterTypeKind::U128,
            AdapterTypeKindFile::USize {} => AdapterTypeKind::USize,
            AdapterTypeKindFile::F32 {} => AdapterTypeKind::F32,
            AdapterTypeKindFile::F64 {} => AdapterTypeKind::F64,
            AdapterTypeKindFile::String {} => AdapterTypeKind::String,
            AdapterTypeKindFile::Char {} => AdapterTypeKind::Char,
            AdapterTypeKindFile::Vec { item } => AdapterTypeKind::Vec {
                item: Box::new(self.convert(*item, environment_owner, child_depth)?),
            },
            AdapterTypeKindFile::Seq { item } => AdapterTypeKind::Seq {
                item: Box::new(self.convert(*item, environment_owner, child_depth)?),
            },
            AdapterTypeKindFile::Option { item } => AdapterTypeKind::Option {
                item: Box::new(self.convert(*item, environment_owner, child_depth)?),
            },
            AdapterTypeKindFile::Result { ok, error } => AdapterTypeKind::Result {
                ok: Box::new(self.convert(*ok, environment_owner, child_depth)?),
                error: Box::new(self.convert(*error, environment_owner, child_depth)?),
            },
            AdapterTypeKindFile::Tuple { items } => AdapterTypeKind::Tuple {
                items: items
                    .into_iter()
                    .map(|item| self.convert(item, environment_owner, child_depth))
                    .collect::<Result<Box<[_]>, _>>()?,
            },
            AdapterTypeKindFile::Need { item } => AdapterTypeKind::Need {
                item: Box::new(self.convert(*item, environment_owner, child_depth)?),
            },
            AdapterTypeKindFile::Nominal { nominal } => {
                let owner = match nominal.owner {
                    AdapterNominalOwnerFile::Standard {} => AdapterNominalOwner::Standard,
                    AdapterNominalOwnerFile::Environment { owner } => {
                        if owner != environment_owner.as_str() {
                            return Err(AdapterManifestCodecError::EnvironmentOwnerMismatch {
                                expected: environment_owner.as_str().to_owned(),
                                actual: owner,
                            });
                        }
                        AdapterNominalOwner::Environment {
                            owner: environment_owner.clone(),
                        }
                    }
                    AdapterNominalOwnerFile::RustPackage { package } => {
                        AdapterNominalOwner::RustPackage {
                            package: ArcweftRustPackageId::try_new(package)?,
                        }
                    }
                };
                let path = nominal_path(nominal.path)?;
                let arguments = nominal
                    .arguments
                    .into_iter()
                    .map(|argument| self.convert(argument, environment_owner, child_depth))
                    .collect::<Result<Vec<_>, _>>()?;
                AdapterTypeKind::Nominal {
                    nominal: AdapterNominalTypeRef::try_new(owner, path, arguments)?,
                }
            }
        })
    }
}

fn nominal_path(segments: Vec<String>) -> Result<AdapterNominalPath, AdapterNominalPathError> {
    AdapterNominalPath::try_new(
        segments
            .into_iter()
            .map(AdapterNominalPathSegment::try_new)
            .collect::<Result<Vec<_>, _>>()?,
    )
}

fn nominal_prefix(
    segments: Vec<String>,
) -> Result<AdapterNominalPathPrefix, AdapterNominalPathError> {
    AdapterNominalPathPrefix::try_new(
        segments
            .into_iter()
            .map(AdapterNominalPathSegment::try_new)
            .collect::<Result<Vec<_>, _>>()?,
    )
}

fn callable_path_from_file(path: &str) -> Result<AdapterCallablePath, AdapterCallableModelError> {
    AdapterCallablePath::try_new(
        path.split('.')
            .map(|segment| AdapterCallableName::try_new(segment.to_owned()))
            .collect::<Result<Vec<_>, _>>()?,
    )
}

fn symbol_path_from_file(path: &str) -> Result<AdapterSymbolPath, AdapterSymbolPathError> {
    if path.is_empty() {
        return AdapterSymbolPath::try_new([]);
    }
    AdapterSymbolPath::try_new(
        path.split('.')
            .map(|segment| AdapterSymbolSegment::try_new(segment.to_owned()))
            .collect::<Result<Vec<_>, _>>()?,
    )
}

fn effect_capabilities(
    effects: impl IntoIterator<Item = String>,
) -> impl Iterator<Item = AdapterEffectCapability> {
    effects.into_iter().map(AdapterEffectCapability::new)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_toml_recursive_typed_adapter_manifest_file() {
        let file = AdapterManifestFile::from_toml(
            r#"
schema_version = 1
id = "custom-file"
display_name = "Custom File"
effects = ["custom.read"]

[[nominal_types]]
path = ["CustomApi"]
arity = 0
opaque_producer = "fixture.adapter-codec.shared"
visibility = "public"
source_label = "CustomApi"

[[symbols]]
name = "adapter.viewport"
type = { kind = "nominal", nominal = { owner = { kind = "environment", owner = "adapter:custom-file" }, path = ["CustomApi"], arguments = [] } }

[[methods]]
name = "read"
overload = 0
receiver = { kind = "nominal", nominal = { owner = { kind = "environment", owner = "adapter:custom-file" }, path = ["CustomApi"], arguments = [] } }
signature = { groups = [{ index = 0, parameters = [] }], result = { kind = "string" } }

[[functions]]
name = "custom.read"
overload = 0
effects = ["custom.read"]
signature = { groups = [{ index = 0, parameters = [{ index = 0, name = "path", passing = "positional_or_named", presence = "required", type = { kind = "string" } }] }], result = { kind = "option", item = { kind = "string" } } }

[[host_calls]]
id = "custom.read"
effects = ["custom.read"]
signature = { groups = [{ index = 0, parameters = [{ index = 0, name = "path", passing = "positional_or_named", presence = "required", type = { kind = "string" } }] }], result = { kind = "string" } }

[[tooling_docs]]
subject = "custom.read"
docs = "Read custom content."
"#,
        )
        .expect("adapter manifest parses");
        assert_eq!(file.schema_version(), ADAPTER_MANIFEST_SCHEMA_VERSION);
        let manifest = file.into_manifest().expect("typed manifest is valid");

        assert_eq!(manifest.id().as_str(), "custom-file");
        assert_eq!(manifest.nominal_declarations().len(), 1);
        assert_eq!(manifest.symbols().len(), 1);
        assert_eq!(manifest.methods().len(), 1);
        assert_eq!(manifest.functions().len(), 1);
        assert_eq!(manifest.effects()[0].as_str(), "custom.read");
        assert_eq!(manifest.host_calls()[0].id(), "custom.read");
        assert!(matches!(
            manifest.functions()[0].signature().return_type(),
            AdapterTypeKind::Option { item } if item.as_ref() == &AdapterTypeKind::String
        ));
    }

    #[test]
    fn parses_json_adapter_manifest_file() {
        let file = AdapterManifestFile::from_json(
            r#"{
  "schema_version": 1,
  "id": "custom-http",
  "display_name": "Custom HTTP",
  "effects": ["http.respond"],
  "host_calls": [{
    "id": "http.respond",
    "effects": ["http.respond"],
    "signature": {
      "groups": [{"index": 0, "parameters": []}],
      "result": {"kind": "unit"}
    }
  }]
}"#,
        )
        .expect("json adapter manifest parses");
        let manifest = file.into_manifest().expect("typed manifest is valid");

        assert_eq!(manifest.id().as_str(), "custom-http");
        assert_eq!(manifest.host_calls()[0].id(), "http.respond");
    }

    #[test]
    fn rejects_unknown_string_type_carriers() {
        let error = AdapterManifestFile::from_json(
            r#"{
  "schema_version": 1,
  "id": "fixture",
  "display_name": "Fixture",
  "symbols": [{"name": "value", "type": "Widget"}]
}"#,
        )
        .expect_err("string type carriers are not part of schema v1");

        assert!(matches!(error, AdapterManifestCodecError::Json(_)));
    }

    #[test]
    fn rejects_unknown_fields_at_json_manifest_and_nested_levels() {
        let top_level = AdapterManifestFile::from_json(
            r#"{
  "schema_version": 1,
  "id": "fixture",
  "display_name": "Fixture",
  "unexpected": true
}"#,
        )
        .expect_err("unknown manifest fields are rejected");
        assert!(matches!(
            top_level,
            AdapterManifestCodecError::Json(error)
                if error.to_string().contains("unknown field")
        ));

        let nested = AdapterManifestFile::from_json(
            r#"{
  "schema_version": 1,
  "id": "fixture",
  "display_name": "Fixture",
  "symbols": [{
    "name": "value",
    "type": {"kind": "string", "unexpected": true}
  }]
}"#,
        )
        .expect_err("unknown nested fields are rejected");
        assert!(matches!(
            nested,
            AdapterManifestCodecError::Json(error)
                if error.to_string().contains("unknown field")
        ));
    }

    #[test]
    fn rejects_unknown_fields_at_toml_manifest_and_nested_levels() {
        let top_level = AdapterManifestFile::from_toml(
            r#"
schema_version = 1
id = "fixture"
display_name = "Fixture"
unexpected = true
"#,
        )
        .expect_err("unknown manifest fields are rejected");
        assert!(matches!(
            top_level,
            AdapterManifestCodecError::Toml(error)
                if error.to_string().contains("unknown field")
        ));

        let nested = AdapterManifestFile::from_toml(
            r#"
schema_version = 1
id = "fixture"
display_name = "Fixture"

[[host_calls]]
id = "fixture.call"
unexpected = true
signature = { groups = [{ index = 0, parameters = [] }], result = { kind = "unit" } }
"#,
        )
        .expect_err("unknown nested fields are rejected");
        assert!(matches!(
            nested,
            AdapterManifestCodecError::Toml(error)
                if error.to_string().contains("unknown field")
        ));
    }

    #[test]
    fn rejects_environment_owner_mismatch() {
        let file = AdapterManifestFile::from_json(
            r#"{
  "schema_version": 1,
  "id": "fixture",
  "display_name": "Fixture",
  "symbols": [{
    "name": "value",
    "type": {
      "kind": "nominal",
      "nominal": {
        "owner": {"kind": "environment", "owner": "adapter:other"},
        "path": ["Widget"],
        "arguments": []
      }
    }
  }]
}"#,
        )
        .expect("file shape parses");
        let error = file
            .into_manifest()
            .expect_err("mismatched owner is rejected");

        assert!(matches!(
            error,
            AdapterManifestCodecError::EnvironmentOwnerMismatch { .. }
        ));
    }

    #[test]
    fn rejects_unsupported_adapter_manifest_schema() {
        let error = AdapterManifestFile::from_toml(
            r#"
schema_version = 2
id = "custom-file"
display_name = "Custom File"
"#,
        )
        .expect_err("unsupported schema is rejected");

        assert!(matches!(
            error,
            AdapterManifestCodecError::UnsupportedSchema {
                format: AdapterManifestSourceFormat::Toml,
                found: 2,
                expected: ADAPTER_MANIFEST_SCHEMA_VERSION
            }
        ));
    }

    #[test]
    fn unsupported_schema_is_rejected_before_missing_producer() {
        let error = AdapterManifestFile::from_json(
            r#"{
  "schema_version": 2,
  "id": "fixture",
  "display_name": "Fixture",
  "nominal_types": [{
    "path": ["Widget"],
    "arity": 0,
    "visibility": "public",
    "source_label": "Widget"
  }]
}"#,
        )
        .expect_err("unsupported schema is rejected before its body is interpreted");
        assert!(matches!(
            error,
            AdapterManifestCodecError::UnsupportedSchema {
                format: AdapterManifestSourceFormat::Json,
                found: 2,
                expected: ADAPTER_MANIFEST_SCHEMA_VERSION
            }
        ));
    }

    #[test]
    fn current_schema_requires_and_validates_producer_in_authored_order() {
        let missing = AdapterManifestFile::from_json(
            r#"{
  "schema_version": 1,
  "id": "fixture",
  "display_name": "Fixture",
  "nominal_types": [{
    "path": ["Widget"],
    "arity": 0,
    "visibility": "public",
    "source_label": "Widget"
  }]
}"#,
        )
        .expect_err("producer is mandatory");
        assert!(matches!(
            missing,
            AdapterManifestCodecError::MissingOpaqueProducer { site }
                if site.format() == AdapterManifestSourceFormat::Json
                    && site.nominal_index() == 0
        ));

        let reserved = AdapterManifestFile::from_toml(
            r#"
schema_version = 1
id = "fixture"
display_name = "Fixture"

[[nominal_types]]
path = ["Widget"]
arity = 0
opaque_producer = "std.claimed"
visibility = "public"
source_label = "Widget"
"#,
        )
        .expect_err("reserved producer is rejected");
        assert!(matches!(
            reserved,
            AdapterManifestCodecError::InvalidOpaqueProducer {
                site,
                error: AdapterOpaqueTypeProducerIdError::ReservedStandardNamespace { .. }
            } if site.format() == AdapterManifestSourceFormat::Toml
                && site.nominal_index() == 0
        ));
    }

    #[test]
    fn json_header_preflight_rejects_duplicate_and_wrong_root() {
        let duplicate = AdapterManifestFile::from_json(
            r#"{"schema_version":1,"schema_version":1,"id":"x","display_name":"X"}"#,
        )
        .expect_err("duplicate schema header is rejected");
        assert!(matches!(
            duplicate,
            AdapterManifestCodecError::MalformedSchemaVersion {
                format: AdapterManifestSourceFormat::Json,
                problem: AdapterManifestSchemaHeaderProblem::DuplicateSchemaVersion
            }
        ));

        let root = AdapterManifestFile::from_json("[]")
            .expect_err("non-object root is rejected as a malformed header");
        assert!(matches!(
            root,
            AdapterManifestCodecError::MalformedSchemaVersion {
                format: AdapterManifestSourceFormat::Json,
                problem: AdapterManifestSchemaHeaderProblem::RootNotObject
            }
        ));
    }
}
