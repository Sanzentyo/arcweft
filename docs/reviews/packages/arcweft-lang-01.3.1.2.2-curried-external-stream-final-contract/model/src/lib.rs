#![forbid(unsafe_code)]

mod application;
mod arguments;
mod identity;
mod signature;
mod swap;

pub use application::{
    ApplyError, ApplyOutcome, EvaluatedGroup, ExternalStreamPartial, OpenRequest, RuntimeState,
};
pub use arguments::{
    ArgumentError, ArgumentProduct, ArgumentValue, CheckedValue, LiveGenerations, NamedRestEntry,
    Ownership, OwnershipClass,
};
pub use identity::{
    AWBC_ABI_VERSION, AWBC_CODEC_VERSION, AWBC_CONSTANT_EXTERNAL_STREAM_CALLABLE,
    AWBC_OPCODE_APPLY_EXTERNAL_STREAM_GROUP, AWBC_OPCODE_OPEN_STREAM,
    AWBC_RUNTIME_TYPE_EXTERNAL_STREAM_CALLABLE, AWBC_RUNTIME_TYPE_STREAM_HANDLE, Coordinate,
    DeclarationDigest, DefaultFingerprint, DefinitionId, GenerationId, GroupIndex, ParameterIndex,
    SAVE_SCHEMA_VERSION, SignatureFingerprint, TypeLayoutHash, ValueDigest,
};
pub use signature::{
    Group, GroupKind, MAX_GROUPS, MAX_PARAMETERS, Parameter, Passing, Presence, Signature,
    SignatureError,
};
pub use swap::{SwapCompatibility, classify_swap};

#[cfg(test)]
mod tests;
