//! Transactional Agent REPL overlay/session substrate.
//!
//! This crate owns the compiler-dependent REPL session model for Agent cells:
//! base project snapshots, committed overlay cells, pre-commit validation,
//! immediate VM execution through `arcweft-agent-runner`, and narrow typed
//! projections for later command and tiering packages. Product-player crates do
//! not depend on this REPL crate.

pub mod binding;
pub mod cell;
pub mod command;
mod compile;
pub mod diagnostics;
pub mod error;
pub mod evidence;
mod hash;
pub mod runtime;
pub mod session;
mod source;
pub mod tier;
pub mod tiering;

pub use binding::{
    ReplBindingInvalidation, ReplBindingRecord, ReplBindingSnapshotKind, ReplBindingStatus,
};
pub use cell::{
    ReplBytecodeStats, ReplCellExecutionStatus, ReplCellFilter, ReplCellId, ReplCellInput,
    ReplCellKind, ReplCellList, ReplCellRecord, ReplEvaluateOutcome, ReplResetOptions,
    ReplResetOutcome, ReplUndoOptions, ReplUndoOutcome,
};
pub use diagnostics::{
    AgentDiagnosticProjection, AgentDiagnosticProjector, AgentSyntaxDiagnosticProjection,
    AgentSyntaxDiagnosticProjectionError, RuntimeAssertionDebugContext,
    project_runtime_assertion_debug_diagnostic,
};
pub use error::{ReplParseCoordinateSpace, ReplTransactionError, ReplTransactionPhase};
pub use evidence::{
    ReplBindingEvidence, ReplDebugEventCount, ReplExecutionRecord, ReplGenerationEvidence,
    ReplGenerationId, ReplHostEffectEvidence,
};
pub use runtime::{ReplCapabilityReport, ReplCapabilitySet, ReplEvaluationRuntime};
pub use session::{ReplBaseChangeOutcome, ReplBaseSnapshot, ReplSession, ReplSessionOptions};
pub use tier::{
    ReplExecutableCell, ReplExecutableSnapshot, ReplTierCursor, ReplTierInvalidationReason,
    ReplTierInvalidationToken, ReplTierStatusProjection, ReplTierStatusRecord,
};
pub use tiering::{
    ReplCodegenStatus, ReplPendingWarmJob, ReplTierBackendStatus, ReplTierCommandHandler,
    ReplTierDiagnostic, ReplTierDiagnosticCode, ReplTierDiagnosticSeverity, ReplTierFallback,
    ReplTierManager, ReplTierPolicy, ReplTierStatus, ReplWarmOutcome, ReplWarmRequest,
    ReplWarmUnsupportedReason,
};
