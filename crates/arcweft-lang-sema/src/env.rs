//! Semantic environment model and canonical base facts.

mod base;
mod effects;
mod enums;
pub mod identity;
pub mod nominal;
pub mod registered;

pub(crate) use base::NominalRecordLiteralPolicy;
pub use base::{
    AgentActionEnvParam, AgentActionEnvSignature, DebugPathKind, FunctionParam,
    FunctionParamHigherOrderBinding, FunctionParamSelector, FunctionParamSelectorSegment,
    FunctionSignature, MethodSignature, RustPackageExports, TypeCheckEnv, TypeCheckEnvBuildError,
};
pub use effects::{EffectCapability, EffectCapabilityParts};
pub use enums::EnumVariantPayload;
pub use registered::{RegisteredSemanticWorld, RegisteredTypeCheckEnv};

#[cfg(test)]
mod nominal_tests;
