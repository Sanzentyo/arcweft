//! Data codecs for project-local adapter manifests.

use crate::manifest::{
    AdapterEffectCapability, AdapterHostCall, AdapterManifest, AdapterToolingDoc,
};
use arcweft_lang_sema::env::{FunctionParam, FunctionSignature};
use arcweft_lang_sema::types::TypeKind;
use serde::Deserialize;
use thiserror::Error;

/// Serializable adapter manifest file.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct AdapterManifestFile {
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

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
struct AdapterSymbolFile {
    name: String,
    ty: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
struct AdapterMethodFile {
    receiver: String,
    name: String,
    return_type: String,
    #[serde(default)]
    params: Vec<AdapterParamFile>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
struct AdapterFunctionFile {
    name: String,
    return_type: String,
    #[serde(default)]
    params: Vec<AdapterParamFile>,
    #[serde(default)]
    effects: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
struct AdapterParamFile {
    name: String,
    ty: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
struct AdapterHostCallFile {
    id: String,
    #[serde(default)]
    effects: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
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
}

impl AdapterManifestFile {
    /// Parses a JSON adapter manifest.
    pub fn from_json(source: &str) -> Result<Self, AdapterManifestCodecError> {
        Ok(serde_json::from_str(source)?)
    }

    /// Parses a TOML adapter manifest.
    pub fn from_toml(source: &str) -> Result<Self, AdapterManifestCodecError> {
        Ok(toml::from_str(source)?)
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
                manifest.with_host_call(AdapterHostCall::new(
                    host_call.id,
                    effect_capabilities(host_call.effects),
                ))
            });
        self.tooling_docs
            .into_iter()
            .fold(manifest, |manifest, doc| {
                manifest.with_tooling_doc(AdapterToolingDoc::new(doc.subject, doc.docs))
            })
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
    match label {
        "()" | "Unit" | "unit" => TypeKind::Unit,
        "Bool" | "bool" => TypeKind::Bool,
        "i8" | "I8" => TypeKind::I8,
        "i16" | "I16" => TypeKind::I16,
        "i32" | "I32" => TypeKind::I32,
        "i64" | "I64" => TypeKind::I64,
        "i128" | "I128" => TypeKind::I128,
        "isize" | "ISize" => TypeKind::ISize,
        "u8" | "U8" => TypeKind::U8,
        "u16" | "U16" => TypeKind::U16,
        "u32" | "U32" => TypeKind::U32,
        "u64" | "U64" => TypeKind::U64,
        "u128" | "U128" => TypeKind::U128,
        "usize" | "USize" => TypeKind::USize,
        "f32" | "F32" => TypeKind::F32,
        "f64" | "F64" => TypeKind::F64,
        "String" | "string" => TypeKind::String,
        "Char" | "char" => TypeKind::Char,
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
effects = ["custom.read"]

[[tooling_docs]]
subject = "custom.read"
docs = "Read custom content."
"#,
        )
        .expect("adapter manifest parses");
        let manifest = file.into_manifest();

        assert_eq!(manifest.id().as_str(), "custom-file");
        assert_eq!(manifest.symbols().len(), 1);
        assert_eq!(manifest.methods().len(), 1);
        assert_eq!(manifest.functions().len(), 1);
        assert_eq!(manifest.effects()[0].as_str(), "custom.read");
        assert_eq!(manifest.host_calls()[0].id(), "custom.read");
        assert_eq!(manifest.tooling_docs()[0].subject(), "custom.read");
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
