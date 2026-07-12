//! Typed diagnostics shared by native, Web, headless, save, and Agent paths.

use serde::{Deserialize, Deserializer, Serialize, de::Error as _};

use super::{
    capability::{FxRendererInterface, FxTarget},
    identity::{FxId, FxInstanceId},
    program::FxEvaluationError,
    state::FxGraphChildPath,
};

/// Stable machine-readable Fx diagnostic code.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FxDiagnosticCode {
    ProgramValidation,
    NumericNonFinite,
    NumericUnderflow,
    DivisionByZero,
    UnitMismatch,
    InvalidOpacity,
    EvaluationBudgetExceeded,
    MissingDefinition,
    AbiMismatch,
    MissingProvider,
    DuplicateProvider,
    ProviderOutputBudgetExceeded,
    ProviderUnavailable,
    UnsupportedCapability,
    NonInvertibleTransform,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FxDiagnosticSeverity {
    Warning,
    Error,
}

impl FxDiagnosticCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ProgramValidation => "program_validation",
            Self::NumericNonFinite => "numeric_non_finite",
            Self::NumericUnderflow => "numeric_underflow",
            Self::DivisionByZero => "division_by_zero",
            Self::UnitMismatch => "unit_mismatch",
            Self::InvalidOpacity => "invalid_opacity",
            Self::EvaluationBudgetExceeded => "evaluation_budget_exceeded",
            Self::MissingDefinition => "missing_definition",
            Self::AbiMismatch => "abi_mismatch",
            Self::MissingProvider => "missing_provider",
            Self::DuplicateProvider => "duplicate_provider",
            Self::ProviderOutputBudgetExceeded => "provider_output_budget_exceeded",
            Self::ProviderUnavailable => "provider_unavailable",
            Self::UnsupportedCapability => "unsupported_capability",
            Self::NonInvertibleTransform => "non_invertible_transform",
        }
    }
}

/// Half-open byte range in an owning source record.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize)]
pub struct FxSourceRange {
    pub start: u32,
    pub end: u32,
}

/// Stable application context attached to every diagnostic where known.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct FxDiagnosticContext {
    pub definition: Option<FxId>,
    pub instance: Option<FxInstanceId>,
    pub child_path: FxGraphChildPath,
    pub target: Option<FxTarget>,
    pub interface: Option<FxRendererInterface>,
    pub source_range: Option<FxSourceRange>,
}

/// Complete renderer-independent diagnostic record.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct FxDiagnostic {
    pub code: FxDiagnosticCode,
    pub severity: FxDiagnosticSeverity,
    pub context: FxDiagnosticContext,
    pub message: String,
}

impl FxSourceRange {
    pub fn try_new(start: u32, end: u32) -> Result<Self, &'static str> {
        if start <= end {
            Ok(Self { start, end })
        } else {
            Err("Fx source range start exceeds end")
        }
    }
}

#[derive(Deserialize)]
struct FxSourceRangeWire {
    start: u32,
    end: u32,
}

impl<'de> Deserialize<'de> for FxSourceRange {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = FxSourceRangeWire::deserialize(deserializer)?;
        Self::try_new(wire.start, wire.end).map_err(D::Error::custom)
    }
}

impl FxDiagnostic {
    pub fn error(
        code: FxDiagnosticCode,
        context: FxDiagnosticContext,
        message: impl Into<String>,
    ) -> Self {
        Self {
            code,
            severity: FxDiagnosticSeverity::Error,
            context,
            message: message.into(),
        }
    }

    pub fn unsupported_capability(
        mut context: FxDiagnosticContext,
        target: FxTarget,
        interface: FxRendererInterface,
    ) -> Self {
        context.target = Some(target);
        context.interface = Some(interface);
        Self::error(
            FxDiagnosticCode::UnsupportedCapability,
            context,
            format!("Fx target {target:?} does not support renderer interface {interface:?}"),
        )
    }

    pub fn from_evaluation(context: FxDiagnosticContext, error: &FxEvaluationError) -> Self {
        let code = match error {
            FxEvaluationError::BudgetExceeded { .. } => FxDiagnosticCode::EvaluationBudgetExceeded,
            FxEvaluationError::DivisionByZero { .. } => FxDiagnosticCode::DivisionByZero,
            FxEvaluationError::Underflow { .. } => FxDiagnosticCode::NumericUnderflow,
            FxEvaluationError::UnitMismatch { .. }
            | FxEvaluationError::InputType { .. }
            | FxEvaluationError::InputCount { .. } => FxDiagnosticCode::UnitMismatch,
            FxEvaluationError::InvalidOpacity { .. } => FxDiagnosticCode::InvalidOpacity,
            FxEvaluationError::NonFiniteResult { .. }
            | FxEvaluationError::IntegerOverflow { .. }
            | FxEvaluationError::IntegerConversion { .. }
            | FxEvaluationError::InvalidClampBounds { .. } => FxDiagnosticCode::NumericNonFinite,
            FxEvaluationError::InvalidProgramState { .. } => FxDiagnosticCode::ProgramValidation,
        };
        Self::error(code, context, error.to_string())
    }
}
