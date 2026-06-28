use crate::cell::ReplCellKind;
use thiserror::Error;

/// Transaction phase used for deterministic diagnostics and rollback audits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReplTransactionPhase {
    ClassifyParse,
    HirLowering,
    SemanticEffectChecks,
    VerifierGate,
    CommitRecordConstruction,
    ImmediateVmExecution,
    EvidencePublication,
    RuntimeSessionPreflight,
}

/// Structured cell transaction failure. Pre-commit variants leave committed
/// session state unchanged.
#[derive(Debug, Error)]
pub enum ReplTransactionError {
    #[error("REPL command input is owned by seq05.2: `{command}`")]
    CommandInputDelegated { command: String },
    #[error("REPL cell is incomplete or invalid: {message}")]
    IncompleteOrInvalid { message: String },
    #[error("REPL cell kind mismatch: expected {expected:?}, got {actual:?}")]
    UnexpectedCellKind {
        expected: ReplCellKind,
        actual: ReplCellKind,
    },
    #[error("{phase:?} failed: {message}")]
    Compile {
        phase: ReplTransactionPhase,
        message: String,
    },
    #[error("semantic/effect policy rejected the REPL cell: {message}")]
    EffectPolicy { message: String },
    #[error("bytecode verifier rejected the REPL cell: {message}")]
    Verifier { message: String },
    #[error("runtime project binding rejected the REPL cell before commit: {message}")]
    ProjectBinding { message: String },
    #[error("failed to construct committed REPL cell record: {message}")]
    Commit { message: String },
}

impl ReplTransactionError {
    #[must_use]
    pub const fn phase(&self) -> ReplTransactionPhase {
        match self {
            Self::CommandInputDelegated { .. }
            | Self::IncompleteOrInvalid { .. }
            | Self::UnexpectedCellKind { .. } => ReplTransactionPhase::ClassifyParse,
            Self::Compile { phase, .. } => *phase,
            Self::EffectPolicy { .. } => ReplTransactionPhase::SemanticEffectChecks,
            Self::Verifier { .. } => ReplTransactionPhase::VerifierGate,
            Self::ProjectBinding { .. } => ReplTransactionPhase::RuntimeSessionPreflight,
            Self::Commit { .. } => ReplTransactionPhase::CommitRecordConstruction,
        }
    }
}
