//! Shared checked compile-time scalar authority.
//!
//! The storage implementation is co-located with text-proxy declarations
//! because those declarations also consume the scalar subset. All other
//! semantic code enters through this module so Object is a consumer rather
//! than the apparent owner of the language-wide scalar algebra.

pub use crate::checked_text_proxy::{
    CheckedCompileTimeScalar, CheckedCompileTimeScalarEnum, CheckedCompileTimeScalarEnumCase,
    CheckedCompileTimeScalarEnumValue, CheckedCompileTimeScalarKind,
};

use crate::types::TypeKind;

/// Grammar used to evaluate one authored compile-time scalar before its exact
/// checked value is materialized for the lower constraint transaction.
///
/// This is deliberately separate from [`PreparedCompileTimeScalarAdmission::value_type`]:
/// a raw numeric literal such as `20` may be reduced to an exact Milli value,
/// but it is not itself type-checked as an already-materialized Milli carrier.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum CompileTimeScalarSourceMode {
    /// Scalar-owned literal and unit grammar with no contextual `TypeKind`.
    Literal,
    /// Canonical public identity supplied by a string or entity-reference token.
    PublicId,
    /// Ordinary typed expression source.
    Typed(TypeKind),
}

/// Exact scalar admission carried through one lower source alternative.
///
/// Lower owns `value_type` as the expected/final actual type. The analyzer
/// owns `source_mode` only while it evaluates and reduces the raw authored
/// expression inside the same candidate checkpoint.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PreparedCompileTimeScalarAdmission {
    kind: CheckedCompileTimeScalarKind,
    value_type: TypeKind,
    source_mode: CompileTimeScalarSourceMode,
}

impl PreparedCompileTimeScalarAdmission {
    pub(crate) fn try_new(
        kind: CheckedCompileTimeScalarKind,
        value_type: TypeKind,
        source_mode: CompileTimeScalarSourceMode,
    ) -> Option<Self> {
        let valid_mode = matches!(
            (&kind, &source_mode),
            (
                CheckedCompileTimeScalarKind::Milli
                    | CheckedCompileTimeScalarKind::Ratio
                    | CheckedCompileTimeScalarKind::Length
                    | CheckedCompileTimeScalarKind::Angle,
                CompileTimeScalarSourceMode::Literal,
            ) | (
                CheckedCompileTimeScalarKind::PublicId,
                CompileTimeScalarSourceMode::PublicId,
            ) | (
                CheckedCompileTimeScalarKind::Bool
                    | CheckedCompileTimeScalarKind::Int
                    | CheckedCompileTimeScalarKind::Duration
                    | CheckedCompileTimeScalarKind::ClosedEnum(_)
                    | CheckedCompileTimeScalarKind::Text
                    | CheckedCompileTimeScalarKind::Color,
                CompileTimeScalarSourceMode::Typed(_),
            )
        );
        valid_mode.then_some(Self {
            kind,
            value_type,
            source_mode,
        })
    }

    pub(crate) const fn kind(&self) -> &CheckedCompileTimeScalarKind {
        &self.kind
    }

    pub(crate) const fn value_type(&self) -> &TypeKind {
        &self.value_type
    }

    pub(crate) const fn source_mode(&self) -> &CompileTimeScalarSourceMode {
        &self.source_mode
    }
}
