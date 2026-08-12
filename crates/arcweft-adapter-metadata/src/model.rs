use arcweft_manifest_model::{
    ActivityId, AdapterExportId, AdapterOpaqueTypeProducerId, AdapterTypeName, CapabilityId,
    DependencyDemand, ExternalModuleId, FieldName, FunctionName, GeneratorName, ManifestVisibility,
    NormalizedProjectPath, PackageId, PackageVersion, RawDigest, SemanticDigest, TargetTriple,
    TypeReference, WitWorldId,
};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use std::fmt;

/// Exact metadata format marker.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdapterMetadataFormat;

/// Exact generated metadata schema marker.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdapterMetadataSchema;

macro_rules! exact_string {
    ($name:ident, $value:literal) => {
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        pub struct $name;

        impl Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                serializer.serialize_str($value)
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                let value = String::deserialize(deserializer)?;
                if value == $value {
                    Ok(Self)
                } else {
                    Err(de::Error::custom(concat!("expected `", $value, "`")))
                }
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str($value)
            }
        }
    };
}

exact_string!(RustAbi, "arcweft-rust-v1");
exact_string!(WasmAbi, "arcweft-wasm-component-v1");
exact_string!(ProcessAbi, "arcweft-process-v1");
exact_string!(ProcessTransport, "stdio-framed-v1");

impl Serialize for AdapterMetadataFormat {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str("arcweft.adapter-metadata")
    }
}

impl<'de> Deserialize<'de> for AdapterMetadataFormat {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        if value == "arcweft.adapter-metadata" {
            Ok(Self)
        } else {
            Err(de::Error::custom(
                "expected metadata format `arcweft.adapter-metadata`",
            ))
        }
    }
}

impl Serialize for AdapterMetadataSchema {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u32(1)
    }
}

impl<'de> Deserialize<'de> for AdapterMetadataSchema {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = u32::deserialize(deserializer)?;
        if value == 1 {
            Ok(Self)
        } else {
            Err(de::Error::custom("expected metadata schema 1"))
        }
    }
}

/// Final neutral metadata envelope.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdapterMetadata {
    pub format: AdapterMetadataFormat,
    pub schema: AdapterMetadataSchema,
    pub generator: GeneratorProvenance,
    pub target: AdapterTarget,
    pub package: AdapterPackage,
    pub module: AdapterModule,
    pub artifact: AdapterArtifact,
    pub requirements: Vec<AdapterRequirement>,
    pub exports: AdapterExports,
    pub abi_hash: SemanticDigest,
    pub payload_hash: SemanticDigest,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeneratorProvenance {
    pub name: GeneratorName,
    pub version: PackageVersion,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "family", rename_all = "kebab-case", deny_unknown_fields)]
pub enum AdapterTarget {
    Rust(RustTarget),
    Wasm(WasmTarget),
    Process(ProcessTarget),
}

/// Rust target payload used by programmatic generators.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RustTarget {
    pub abi: RustAbi,
    pub target_triple: TargetTriple,
}

/// WASM component target payload used by programmatic generators.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WasmTarget {
    pub abi: WasmAbi,
    pub world: WitWorldId,
}

/// Process protocol target payload used by programmatic generators.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProcessTarget {
    pub abi: ProcessAbi,
    pub transport: ProcessTransport,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdapterPackage {
    pub id: PackageId,
    pub version: PackageVersion,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdapterModule {
    pub id: ExternalModuleId,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdapterArtifact {
    pub path: NormalizedProjectPath,
    pub size: u64,
    pub hash: RawDigest,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum AdapterRequirement {
    Capability {
        id: CapabilityId,
        demand: DependencyDemand,
        interface_hash: SemanticDigest,
    },
    Module {
        package: PackageId,
        version: PackageVersion,
        module: ExternalModuleId,
        demand: DependencyDemand,
        abi_hash: SemanticDigest,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdapterExports {
    pub types: Vec<AdapterTypeExport>,
    pub functions: Vec<AdapterFunctionExport>,
    pub activities: Vec<AdapterActivityExport>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdapterTypeExport {
    pub name: AdapterTypeName,
    pub visibility: ManifestVisibility,
    pub opaque_producer: AdapterOpaqueTypeProducerId,
    pub shape: AdapterTypeShape,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum AdapterTypeShape {
    Record { fields: Vec<AdapterTypeField> },
    Opaque,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdapterTypeField {
    pub name: FieldName,
    #[serde(rename = "type")]
    pub ty: TypeReference,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdapterFunctionExport {
    pub name: FunctionName,
    pub visibility: ManifestVisibility,
    pub params: Vec<AdapterParameter>,
    #[serde(rename = "return")]
    pub return_type: TypeReference,
    pub purity: FunctionPurity,
    pub effects: Vec<CapabilityId>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdapterParameter {
    pub name: FieldName,
    #[serde(rename = "type")]
    pub ty: TypeReference,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FunctionPurity {
    Pure,
    Effectful,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdapterActivityExport {
    pub export: AdapterExportId,
    pub visibility: ManifestVisibility,
    pub activity_id: ActivityId,
    pub interface_hash: SemanticDigest,
    pub state_hash: SemanticDigest,
}
