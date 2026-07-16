//! Strict, deterministic, Sans-I/O codec for generated Arcweft adapter
//! metadata shared by Rust, WASM component, and process adapters.

mod codec;
mod model;
mod strict_json;

pub use codec::{AdapterMetadataCodecError, SourceBackedAdapterMetadata};
pub use model::{
    AdapterActivityExport, AdapterArtifact, AdapterExports, AdapterFunctionExport, AdapterMetadata,
    AdapterMetadataFormat, AdapterMetadataSchema, AdapterModule, AdapterPackage, AdapterParameter,
    AdapterRequirement, AdapterTarget, AdapterTypeExport, AdapterTypeField, AdapterTypeShape,
    FunctionPurity, GeneratorProvenance, ProcessAbi, ProcessTarget, ProcessTransport, RustAbi,
    RustTarget, WasmAbi, WasmTarget,
};
pub use strict_json::{
    AdapterMetadataSourceMap, JsonPath, JsonPathSegment, JsonToken, StrictJsonError,
};
