//! Semantic environment model and canonical base facts.

mod base;
mod effects;
mod enums;
pub mod identity;
pub mod nominal;
pub mod registered;
pub mod rust_metadata;

pub(crate) use base::{EnvironmentEnumSchema, StandardEnvironmentMethodRole};
pub use base::{
    FunctionParam, FunctionParamHigherOrderBinding, FunctionParamSelector,
    FunctionParamSelectorSegment, FunctionSignature, TypeCheckEnv, TypeCheckEnvBuildError,
};
pub use effects::{EffectCapability, EffectCapabilityParts};
pub use enums::EnumVariantPayload;
pub use registered::{RegisteredSemanticWorld, RegisteredTypeCheckEnv};
pub use rust_metadata::{
    AcceptedRustStructShape, AcceptedRustTypeMetadata, AcceptedRustTypeMetadataCatalog,
    AcceptedRustTypeMetadataCatalogError, AcceptedRustTypeMetadataDigest,
    AcceptedRustTypeMetadataKind, InstantiatedRustTypeMetadata, RustMetadataInstantiationError,
    RustStructMetadataInput, RustTypeMetadataPublicationIdentity, RustTypeMetadataPublicationInput,
    RustTypeMetadataPublicationKind, RustTypeParameterPublicationInput, RustVariantMetadataInput,
    RustVariantPayloadInput,
};

#[cfg(test)]
mod nominal_tests;

#[cfg(test)]
mod tests;
