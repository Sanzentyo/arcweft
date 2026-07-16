//! Data codecs for project-local adapter manifests.

use crate::manifest::{
    AdapterCallableGroupIndex, AdapterCallableModelError, AdapterCallableName,
    AdapterCallableOverloadIndex, AdapterCallableParameterIndex, AdapterCallablePath,
    AdapterEffectCapability, AdapterFreeCallableKind, AdapterFunctionParam,
    AdapterFunctionSignature, AdapterHostCall, AdapterManifest, AdapterParameterGroup,
    AdapterParameterPassing, AdapterParameterPresence, AdapterToolingDoc, AdapterToolingSubject,
    AdapterTypeKind,
};
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
    #[serde(default)]
    effects: Vec<String>,
    #[serde(default)]
    overload: u16,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct AdapterFunctionFile {
    name: String,
    return_type: String,
    #[serde(default)]
    params: Vec<AdapterParamFile>,
    #[serde(default)]
    effects: Vec<String>,
    #[serde(default)]
    overload: u16,
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
    #[error(transparent)]
    Model(#[from] AdapterCallableModelError),
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
    pub fn into_manifest(self) -> Result<AdapterManifest, AdapterManifestCodecError> {
        let mut manifest = AdapterManifest::new(self.id, self.display_name);
        for symbol in self.symbols {
            manifest = manifest.with_symbol(symbol.name, parse_adapter_type_kind_label(&symbol.ty));
        }
        for method in self.methods {
            let signature = signature_from_file(&method.return_type, method.params)?;
            manifest = manifest.with_method_signature(
                parse_adapter_type_kind_label(&method.receiver),
                AdapterCallableName::try_new(method.name)?,
                AdapterCallableOverloadIndex::try_from_usize(usize::from(method.overload))?,
                signature,
                effect_capabilities(method.effects),
            );
        }
        for function in self.functions {
            manifest = manifest.with_function_signature(
                callable_path_from_file(&function.name)?,
                AdapterCallableOverloadIndex::try_from_usize(usize::from(function.overload))?,
                signature_from_file(&function.return_type, function.params)?,
                effect_capabilities(function.effects),
            );
        }
        for effect in self.effects {
            manifest = manifest.with_effect(AdapterEffectCapability::new(effect));
        }
        for host_call in self.host_calls {
            manifest = manifest.with_host_call(AdapterHostCall::with_signature(
                host_call.id,
                signature_from_file(&host_call.return_type, host_call.params)?,
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
    return_type: &str,
    params: Vec<AdapterParamFile>,
) -> Result<AdapterFunctionSignature, AdapterCallableModelError> {
    let parameters = params
        .into_iter()
        .enumerate()
        .map(|(index, parameter)| {
            AdapterFunctionParam::try_new(
                AdapterCallableParameterIndex::try_from_usize(index)?,
                Some(AdapterCallableName::try_new(parameter.name)?),
                parse_adapter_type_kind_label(&parameter.ty),
                AdapterParameterPassing::PositionalOrNamed,
                AdapterParameterPresence::Required,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    AdapterFunctionSignature::try_new(
        vec![AdapterParameterGroup::try_new(
            AdapterCallableGroupIndex::try_from_usize(0)?,
            parameters,
        )?],
        parse_adapter_type_kind_label(return_type),
    )
}

fn callable_path_from_file(path: &str) -> Result<AdapterCallablePath, AdapterCallableModelError> {
    AdapterCallablePath::try_new(
        path.split('.')
            .map(|segment| AdapterCallableName::try_new(segment.to_owned()))
            .collect::<Result<Vec<_>, _>>()?,
    )
}

fn effect_capabilities(
    effects: impl IntoIterator<Item = String>,
) -> impl Iterator<Item = AdapterEffectCapability> {
    effects.into_iter().map(AdapterEffectCapability::new)
}

fn parse_adapter_type_kind_label(label: &str) -> AdapterTypeKind {
    let label = label.trim();
    if let Some(ty) = AdapterTypeKind::primitive_name(label) {
        return ty;
    }
    parse_generic_type_label(label).unwrap_or_else(|| AdapterTypeKind::Named(label.to_owned()))
}

fn parse_generic_type_label(label: &str) -> Option<AdapterTypeKind> {
    let (head, inner) = split_generic(label)?;
    match head {
        "Vec" => Some(AdapterTypeKind::Vec(Box::new(
            parse_adapter_type_kind_label(inner),
        ))),
        "Seq" => Some(AdapterTypeKind::Seq(Box::new(
            parse_adapter_type_kind_label(inner),
        ))),
        "Option" => Some(AdapterTypeKind::Option(Box::new(
            parse_adapter_type_kind_label(inner),
        ))),
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
        let manifest = file.into_manifest().expect("typed manifest is valid");

        assert_eq!(manifest.id().as_str(), "custom-file");
        assert_eq!(manifest.symbols().len(), 1);
        assert_eq!(manifest.methods().len(), 1);
        assert_eq!(manifest.functions().len(), 1);
        assert_eq!(manifest.effects()[0].as_str(), "custom.read");
        assert_eq!(manifest.host_calls()[0].id(), "custom.read");
        assert_eq!(
            manifest.host_calls()[0].signature().groups()[0]
                .parameters()
                .len(),
            1
        );
        assert_eq!(
            manifest.host_calls()[0].signature().return_type(),
            &AdapterTypeKind::String
        );
        assert!(matches!(
            manifest.tooling_docs()[0].subject(),
            AdapterToolingSubject::Free { path, .. }
                if path.segments().iter().map(AdapterCallableName::as_str).collect::<Vec<_>>()
                    == ["custom", "read"]
        ));
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
        let manifest = file.into_manifest().expect("typed manifest is valid");

        assert_eq!(manifest.id().as_str(), "custom-http");
        assert_eq!(manifest.host_calls()[0].id(), "http.respond");
        assert!(matches!(
            manifest.tooling_docs()[0].subject(),
            AdapterToolingSubject::Free { path, .. }
                if path.segments().iter().map(AdapterCallableName::as_str).collect::<Vec<_>>()
                    == ["http", "respond"]
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

    #[test]
    fn type_label_parser_keeps_named_types_and_common_generics() {
        assert_eq!(
            parse_adapter_type_kind_label("Widget"),
            AdapterTypeKind::Named("Widget".to_owned())
        );
        assert_eq!(
            parse_adapter_type_kind_label("Bool"),
            AdapterTypeKind::Named("Bool".to_owned())
        );
        assert_eq!(
            parse_adapter_type_kind_label("string"),
            AdapterTypeKind::Named("string".to_owned())
        );
        assert_eq!(
            parse_adapter_type_kind_label("Seq<String>"),
            AdapterTypeKind::Seq(Box::new(AdapterTypeKind::String))
        );
        assert_eq!(
            parse_adapter_type_kind_label("Option<i32>"),
            AdapterTypeKind::Option(Box::new(AdapterTypeKind::I32))
        );
    }
}
