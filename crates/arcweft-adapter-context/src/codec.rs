//! Exact data codecs for project-local adapter manifests.

use crate::manifest::{
    AdapterCallableGroupIndex, AdapterCallableModelError, AdapterCallableName,
    AdapterCallableOverloadIndex, AdapterCallableParameterIndex, AdapterCallablePath,
    AdapterEffectCapability, AdapterEnvironmentOwnerId, AdapterFreeCallableKind,
    AdapterFunctionParam, AdapterFunctionSignature, AdapterHostCall, AdapterManifest,
    AdapterManifestModelError, AdapterNominalDeclaration, AdapterNominalOwner, AdapterNominalPath,
    AdapterNominalPathError, AdapterNominalPathPrefix, AdapterNominalPathSegment,
    AdapterNominalTypeRef, AdapterNominalVisibility, AdapterParameterGroup,
    AdapterParameterPassing, AdapterParameterPresence, AdapterSymbol, AdapterSymbolPath,
    AdapterSymbolPathError, AdapterSymbolSegment, AdapterToolingDoc, AdapterToolingSubject,
    AdapterTypeKind, AdapterTypeModelError,
};
use arcweft_rust_abi::{ArcweftRustIdentityError, ArcweftRustPackageId};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Current stable project-local adapter manifest schema version.
pub const ADAPTER_MANIFEST_SCHEMA_VERSION: u32 = 1;

const MAX_TYPE_DEPTH: usize = 256;
const MAX_TYPE_NODES: usize = 4_096;

/// Serializable final-shape adapter manifest file.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
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

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct AdapterNominalDeclarationFile {
    path: Vec<String>,
    arity: u16,
    visibility: AdapterNominalVisibility,
    source_label: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct AdapterRustPackageMountFile {
    package: String,
    prefix: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct AdapterSymbolFile {
    name: String,
    #[serde(rename = "type")]
    ty: AdapterTypeKindFile,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
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
struct AdapterFunctionFile {
    name: String,
    signature: AdapterFunctionSignatureFile,
    #[serde(default)]
    effects: Vec<String>,
    #[serde(default)]
    overload: u16,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct AdapterFunctionSignatureFile {
    groups: Vec<AdapterParameterGroupFile>,
    result: AdapterTypeKindFile,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct AdapterParameterGroupFile {
    index: u16,
    #[serde(default)]
    parameters: Vec<AdapterParamFile>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
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
struct AdapterHostCallFile {
    id: String,
    signature: AdapterFunctionSignatureFile,
    #[serde(default)]
    effects: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct AdapterToolingDocFile {
    subject: String,
    docs: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum AdapterTypeKindFile {
    Unit,
    Bool,
    I8,
    I16,
    I32,
    I64,
    I128,
    ISize,
    U8,
    U16,
    U32,
    U64,
    U128,
    USize,
    F32,
    F64,
    String,
    Char,
    Vec { item: Box<Self> },
    Seq { item: Box<Self> },
    Option { item: Box<Self> },
    Result { ok: Box<Self>, error: Box<Self> },
    Tuple { items: Vec<Self> },
    Need { ready: Box<Self>, error: Box<Self> },
    Nominal { nominal: AdapterNominalTypeRefFile },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct AdapterNominalTypeRefFile {
    owner: AdapterNominalOwnerFile,
    path: Vec<String>,
    #[serde(default)]
    arguments: Vec<AdapterTypeKindFile>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum AdapterNominalOwnerFile {
    Environment { owner: String },
    RustPackage { package: String },
}

/// Errors while parsing project-local adapter manifests.
#[derive(Debug, Error)]
pub enum AdapterManifestCodecError {
    #[error("failed to parse adapter manifest JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("failed to parse adapter manifest TOML: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("unsupported adapter manifest schema {found}, expected {expected}")]
    UnsupportedSchema { found: u32, expected: u32 },
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
        let file = serde_json::from_str::<Self>(source)?;
        file.validate_schema_version()?;
        Ok(file)
    }

    /// Parses a TOML adapter manifest.
    pub fn from_toml(source: &str) -> Result<Self, AdapterManifestCodecError> {
        let file = toml::from_str::<Self>(source)?;
        file.validate_schema_version()?;
        Ok(file)
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
            manifest = manifest.with_host_call(AdapterHostCall::with_signature(
                host_call.id,
                signature_from_file(host_call.signature, &environment_owner)?,
                effect_capabilities(host_call.effects),
            ));
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

    fn validate_schema_version(&self) -> Result<(), AdapterManifestCodecError> {
        if self.schema_version == ADAPTER_MANIFEST_SCHEMA_VERSION {
            Ok(())
        } else {
            Err(AdapterManifestCodecError::UnsupportedSchema {
                found: self.schema_version,
                expected: ADAPTER_MANIFEST_SCHEMA_VERSION,
            })
        }
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
            AdapterTypeKindFile::Unit => AdapterTypeKind::Unit,
            AdapterTypeKindFile::Bool => AdapterTypeKind::Bool,
            AdapterTypeKindFile::I8 => AdapterTypeKind::I8,
            AdapterTypeKindFile::I16 => AdapterTypeKind::I16,
            AdapterTypeKindFile::I32 => AdapterTypeKind::I32,
            AdapterTypeKindFile::I64 => AdapterTypeKind::I64,
            AdapterTypeKindFile::I128 => AdapterTypeKind::I128,
            AdapterTypeKindFile::ISize => AdapterTypeKind::ISize,
            AdapterTypeKindFile::U8 => AdapterTypeKind::U8,
            AdapterTypeKindFile::U16 => AdapterTypeKind::U16,
            AdapterTypeKindFile::U32 => AdapterTypeKind::U32,
            AdapterTypeKindFile::U64 => AdapterTypeKind::U64,
            AdapterTypeKindFile::U128 => AdapterTypeKind::U128,
            AdapterTypeKindFile::USize => AdapterTypeKind::USize,
            AdapterTypeKindFile::F32 => AdapterTypeKind::F32,
            AdapterTypeKindFile::F64 => AdapterTypeKind::F64,
            AdapterTypeKindFile::String => AdapterTypeKind::String,
            AdapterTypeKindFile::Char => AdapterTypeKind::Char,
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
            AdapterTypeKindFile::Need { ready, error } => AdapterTypeKind::Need {
                ready: Box::new(self.convert(*ready, environment_owner, child_depth)?),
                error: Box::new(self.convert(*error, environment_owner, child_depth)?),
            },
            AdapterTypeKindFile::Nominal { nominal } => {
                let owner = match nominal.owner {
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
                found: 2,
                expected: ADAPTER_MANIFEST_SCHEMA_VERSION
            }
        ));
    }
}
