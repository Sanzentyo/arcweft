use arcweft_lang_syntax::incremental::SyntaxDiagnostic;
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

/// Coordinate space retained with parser diagnostics at the REPL boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReplParseCoordinateSpace {
    /// UTF-8 byte offsets in the generated compilation source.
    SyntheticSourceUtf8Bytes,
}

/// Structured cell transaction failure. Pre-commit variants leave committed
/// session state unchanged.
#[derive(Debug, Error)]
pub enum ReplTransactionError {
    #[error("REPL command input is owned by seq05.2: `{command}`")]
    CommandInputDelegated { command: String },
    #[error("REPL cell is incomplete or invalid: {message}")]
    IncompleteOrInvalid { message: String },
    #[error("REPL attached source parsing failed")]
    AttachedParse {
        diagnostics: Vec<SyntaxDiagnostic>,
        coordinate_space: ReplParseCoordinateSpace,
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
            | Self::AttachedParse { .. } => ReplTransactionPhase::ClassifyParse,
            Self::Compile { phase, .. } => *phase,
            Self::EffectPolicy { .. } => ReplTransactionPhase::SemanticEffectChecks,
            Self::Verifier { .. } => ReplTransactionPhase::VerifierGate,
            Self::ProjectBinding { .. } => ReplTransactionPhase::RuntimeSessionPreflight,
            Self::Commit { .. } => ReplTransactionPhase::CommitRecordConstruction,
        }
    }

    /// Revision-bound attached-source diagnostics retained by project compilation.
    #[must_use]
    pub fn attached_parse_diagnostics(&self) -> Option<&[SyntaxDiagnostic]> {
        match self {
            Self::AttachedParse { diagnostics, .. } => Some(diagnostics),
            _ => None,
        }
    }

    /// Coordinate space shared by the retained parser diagnostics.
    #[must_use]
    pub const fn parse_coordinate_space(&self) -> Option<ReplParseCoordinateSpace> {
        match self {
            Self::AttachedParse {
                coordinate_space, ..
            } => Some(*coordinate_space),
            _ => None,
        }
    }
}

impl ReplParseCoordinateSpace {
    /// Stable protocol spelling for this coordinate space.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SyntheticSourceUtf8Bytes => "synthetic_source",
        }
    }
}
