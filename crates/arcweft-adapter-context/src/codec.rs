//! Data codecs for project-local adapter manifests.

use crate::manifest::{
    AdapterEffectCapability, AdapterHostCall, AdapterManifest, AdapterToolingDoc,
};
use arcweft_lang_sema::env::{FunctionParam, FunctionSignature};
use arcweft_lang_sema::types::TypeKind;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Current stable project-local adapter manifest schema version.
pub const ADAPTER_MANIFEST_SCHEMA_VERSION: u32 = 1;

/// Serializable adapter manifest file.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AdapterManifestFile {
    schema_version: u32,
    id: String,
    display_name: String,
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
struct AdapterSymbolFile {
    name: String,
    ty: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct AdapterMethodFile {
    receiver: String,
    name: String,
    return_type: String,
    #[serde(default)]
    params: Vec<AdapterParamFile>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct AdapterFunctionFile {
    name: String,
    return_type: String,
    #[serde(default)]
    params: Vec<AdapterParamFile>,
    #[serde(default)]
    effects: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct AdapterParamFile {
    name: String,
    ty: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct AdapterHostCallFile {
    id: String,
    return_type: String,
    #[serde(default)]
    params: Vec<AdapterParamFile>,
    #[serde(default)]
    effects: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct AdapterToolingDocFile {
    subject: String,
    docs: String,
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

    /// Converts file data into the typed manifest used by sema and tooling.
    pub fn into_manifest(self) -> AdapterManifest {
        let manifest = AdapterManifest::new(self.id, self.display_name);
        let manifest = self.symbols.into_iter().fold(manifest, |manifest, symbol| {
            manifest.with_symbol(symbol.name, parse_type_kind_label(&symbol.ty))
        });
        let manifest = self.methods.into_iter().fold(manifest, |manifest, method| {
            let signature = FunctionSignature::new(
                parse_type_kind_label(&method.return_type),
                method.params.into_iter().map(function_param_from_file),
            );
            manifest.with_method_signature(
                parse_type_kind_label(&method.receiver),
                method.name,
                signature,
            )
        });
        let manifest = self
            .functions
            .into_iter()
            .fold(manifest, |manifest, function| {
                manifest.with_function_signature(
                    function.name,
                    FunctionSignature::new(
                        parse_type_kind_label(&function.return_type),
                        function.params.into_iter().map(function_param_from_file),
                    ),
                    effect_capabilities(function.effects),
                )
            });
        let manifest = self.effects.into_iter().fold(manifest, |manifest, effect| {
            manifest.with_effect(AdapterEffectCapability::new(effect))
        });
        let manifest = self
            .host_calls
            .into_iter()
            .fold(manifest, |manifest, host_call| {
                manifest.with_host_call(AdapterHostCall::with_signature(
                    host_call.id,
                    FunctionSignature::new(
                        parse_type_kind_label(&host_call.return_type),
                        host_call.params.into_iter().map(function_param_from_file),
                    ),
                    effect_capabilities(host_call.effects),
                ))
            });
        self.tooling_docs
            .into_iter()
            .fold(manifest, |manifest, doc| {
                manifest.with_tooling_doc(AdapterToolingDoc::new(doc.subject, doc.docs))
            })
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

fn function_param_from_file(param: AdapterParamFile) -> FunctionParam {
    FunctionParam::required(param.name, parse_type_kind_label(&param.ty))
}

fn effect_capabilities(
    effects: impl IntoIterator<Item = String>,
) -> impl Iterator<Item = AdapterEffectCapability> {
    effects.into_iter().map(AdapterEffectCapability::new)
}

fn parse_type_kind_label(label: &str) -> TypeKind {
    let label = label.trim();
    if let Some(ty) = TypeKind::primitive_name(label) {
        return ty;
    }
    match label {
        "unit" => TypeKind::Unit,
        "I8" => TypeKind::I8,
        "I16" => TypeKind::I16,
        "I32" => TypeKind::I32,
        "I64" => TypeKind::I64,
        "I128" => TypeKind::I128,
        "ISize" => TypeKind::ISize,
        "U8" => TypeKind::U8,
        "U16" => TypeKind::U16,
        "U32" => TypeKind::U32,
        "U64" => TypeKind::U64,
        "U128" => TypeKind::U128,
        "USize" => TypeKind::USize,
        "F32" => TypeKind::F32,
        "F64" => TypeKind::F64,
        "string" => TypeKind::String,
        other => parse_generic_type_label(other)
            .unwrap_or_else(|| TypeKind::Named(other.trim().to_owned())),
    }
}

fn parse_generic_type_label(label: &str) -> Option<TypeKind> {
    let (head, inner) = split_generic(label)?;
    match head {
        "Vec" => Some(TypeKind::Vec(Box::new(parse_type_kind_label(inner)))),
        "Seq" => Some(TypeKind::Seq(Box::new(parse_type_kind_label(inner)))),
        "Option" => Some(TypeKind::Option(Box::new(parse_type_kind_label(inner)))),
        _ => None,
    }
}

fn split_generic(label: &str) -> Option<(&str, &str)> {
    let (head, rest) = label.split_once('<')?;
    let inner = rest.strip_suffix('>')?;
    Some((head.trim(), inner.trim()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_toml_adapter_manifest_file() {
        let file = AdapterManifestFile::from_toml(
            r#"
schema_version = 1
id = "custom-file"
display_name = "Custom File"
effects = ["custom.read"]

[[symbols]]
name = "custom"
ty = "CustomApi"

[[methods]]
receiver = "CustomApi"
name = "read"
return_type = "String"

[[functions]]
name = "custom.read"
return_type = "String"
effects = ["custom.read"]
params = [{ name = "path", ty = "String" }]

[[host_calls]]
id = "custom.read"
return_type = "String"
effects = ["custom.read"]
params = [{ name = "path", ty = "String" }]

[[tooling_docs]]
subject = "custom.read"
docs = "Read custom content."
"#,
        )
        .expect("adapter manifest parses");
        assert_eq!(file.schema_version(), ADAPTER_MANIFEST_SCHEMA_VERSION);
        let manifest = file.into_manifest();

        assert_eq!(manifest.id().as_str(), "custom-file");
        assert_eq!(manifest.symbols().len(), 1);
        assert_eq!(manifest.methods().len(), 1);
        assert_eq!(manifest.functions().len(), 1);
        assert_eq!(manifest.effects()[0].as_str(), "custom.read");
        assert_eq!(manifest.host_calls()[0].id(), "custom.read");
        assert_eq!(manifest.host_calls()[0].signature().params().len(), 1);
        assert_eq!(
            manifest.host_calls()[0].signature().return_type(),
            &TypeKind::String
        );
        assert_eq!(manifest.tooling_docs()[0].subject(), "custom.read");
    }

    #[test]
    fn parses_json_adapter_manifest_file() {
        let file = AdapterManifestFile::from_json(
            r#"
{
  "schema_version": 1,
  "id": "custom-http",
  "display_name": "Custom HTTP",
  "effects": ["http.respond"],
  "host_calls": [
    {
      "id": "http.respond",
      "return_type": "Unit",
      "effects": ["http.respond"]
    }
  ],
  "tooling_docs": [
    {
      "subject": "http.respond",
      "docs": "Send a server response."
    }
  ]
}
"#,
        )
        .expect("json adapter manifest parses");
        let manifest = file.into_manifest();

        assert_eq!(manifest.id().as_str(), "custom-http");
        assert_eq!(manifest.host_calls()[0].id(), "http.respond");
        assert_eq!(manifest.tooling_docs()[0].subject(), "http.respond");
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

    #[test]
    fn type_label_parser_keeps_named_types_and_common_generics() {
        assert_eq!(
            parse_type_kind_label("Widget"),
            TypeKind::Named("Widget".to_owned())
        );
        assert_eq!(
            parse_type_kind_label("Seq<String>"),
            TypeKind::Seq(Box::new(TypeKind::String))
        );
        assert_eq!(
            parse_type_kind_label("Option<i32>"),
            TypeKind::Option(Box::new(TypeKind::I32))
        );
    }
}
