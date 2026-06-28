//! Compiler-private persistent object contracts for `.awbo` cache payloads.
//!
//! `.awbo` objects are not a public compatibility format. They are scoped by
//! cache schema, exact compiler identity where required, query inputs, and
//! dependency digests so corrupt or stale records can be treated as soft cache
//! misses by adapter crates.

mod codec;
mod payload;
mod schema;

pub use codec::AwboEnvelope;
pub use payload::{
    BytecodeUnitObject, CompilerObjectPayload, HirBodyFactsObject, HirBodyObject,
    InterfaceSummaryObject, LineTaskEvidenceObject, LinkPlanObject, ParsedSyntaxEvidenceObject,
    ParsedSyntaxObject, PublicSymbolObject, RuntimePlanUnitObject, StableDiagnosticObject,
    StableDiagnosticSeverity, StableDiagnosticSummaryObject, StableRangeObject,
    StableSourceSpanObject, SyntaxStatsObject,
};
pub use schema::{
    AWBO_MAGIC, AWBO_SCHEMA_VERSION, AwboError, CompilerBuildIdentity,
    CompilerIdentityNamespaceObject, CompilerObjectKey, CompilerObjectKind,
    CompilerObjectStability, CompilerStageInputsObject,
};
