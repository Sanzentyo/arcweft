//! Shared checked compile-time scalar authority.
//!
//! The storage implementation is co-located with text-proxy declarations
//! because those declarations also consume the scalar subset. All other
//! semantic code enters through this module so Object is a consumer rather
//! than the apparent owner of the language-wide scalar algebra.

pub use crate::checked_text_proxy::{
    CheckedCompileTimeScalar, CheckedCompileTimeScalarEnum, CheckedCompileTimeScalarEnumCase,
    CheckedCompileTimeScalarEnumValue, CheckedCompileTimeScalarKind,
    CompileTimeScalarReductionError, reduce_scalar_literal,
};

use crate::{
    registration::{CompileTimeScalarTypeRoleId, RegisteredCompileTimeScalarTypes},
    types::{CompileTimeScalarKind, ProjectNominalType, TypeKind},
};

/// Grammar used to evaluate one authored compile-time scalar before its exact
/// checked value is materialized for the lower constraint transaction.
///
/// This is deliberately separate from [`PreparedCompileTimeScalarAdmission::value_type`]:
/// a raw numeric literal such as `20` may be reduced to an exact Milli value,
/// but it is not itself type-checked as an already-materialized Milli carrier.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CompileTimeScalarSourceMode {
    /// Scalar-owned literal and unit grammar with no contextual `TypeKind`.
    Literal,
    /// Canonical public identity supplied by a string or entity-reference token.
    PublicId,
    /// Ordinary expression evaluated with the exact scalar value type.
    Typed,
}

impl CheckedCompileTimeScalarKind {
    pub(crate) const fn from_registered_role(role: CompileTimeScalarTypeRoleId) -> Self {
        use CompileTimeScalarTypeRoleId as Role;

        match role {
            Role::Bool => Self::Bool,
            Role::Int => Self::Int,
            Role::Milli => Self::Milli,
            Role::Ratio => Self::Ratio,
            Role::Length => Self::Length,
            Role::Angle => Self::Angle,
            Role::Duration => Self::Duration,
            Role::PublicId => Self::PublicId,
            Role::Text => Self::Text,
            Role::Color => Self::Color,
            Role::FontFamily => Self::FontFamily,
        }
    }

    pub(crate) const fn source_mode(&self) -> CompileTimeScalarSourceMode {
        match self {
            Self::Milli | Self::Ratio | Self::Length | Self::Angle => {
                CompileTimeScalarSourceMode::Literal
            }
            Self::PublicId => CompileTimeScalarSourceMode::PublicId,
            Self::Bool
            | Self::Int
            | Self::Duration
            | Self::ClosedEnum(_)
            | Self::Text
            | Self::Color
            | Self::FontFamily => CompileTimeScalarSourceMode::Typed,
        }
    }

    pub(crate) fn value_type(&self, registry: &RegisteredCompileTimeScalarTypes) -> TypeKind {
        use CompileTimeScalarTypeRoleId as Role;

        let role = match self {
            Self::Bool => Role::Bool,
            Self::Int => Role::Int,
            Self::Milli => Role::Milli,
            Self::Ratio => Role::Ratio,
            Self::Length => Role::Length,
            Self::Angle => Role::Angle,
            Self::Duration => Role::Duration,
            Self::PublicId => Role::PublicId,
            Self::Text => Role::Text,
            Self::Color => Role::Color,
            Self::FontFamily => Role::FontFamily,
            Self::ClosedEnum(schema) => {
                return TypeKind::ProjectNominal(ProjectNominalType::new(
                    schema.declaration().clone(),
                    Box::<[TypeKind]>::default(),
                ));
            }
        };
        registry.type_for(role).clone()
    }
}

/// Exact scalar admission carried through one lower source alternative.
///
/// Lower receives `value_type` as the expected and final actual type. The
/// analyzer derives the source grammar from `kind` during the same candidate
/// checkpoint.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PreparedCompileTimeScalarAdmission {
    kind: CheckedCompileTimeScalarKind,
    value_type: TypeKind,
}

impl PreparedCompileTimeScalarAdmission {
    pub(crate) fn for_kind(
        kind: CheckedCompileTimeScalarKind,
        registry: &RegisteredCompileTimeScalarTypes,
    ) -> Self {
        let value_type = kind.value_type(registry);
        Self { kind, value_type }
    }

    /// Recognizes an exact registered nominal scalar in a callable schema.
    /// Builtin values and project enums require an explicit consumer kind.
    pub(crate) fn for_declared_type(
        value_type: &TypeKind,
        registry: &RegisteredCompileTimeScalarTypes,
    ) -> Option<Self> {
        let TypeKind::CompileTimeScalar(scalar) = value_type else {
            return None;
        };
        let kind = match scalar.kind() {
            CompileTimeScalarKind::Milli => CheckedCompileTimeScalarKind::Milli,
            CompileTimeScalarKind::Ratio => CheckedCompileTimeScalarKind::Ratio,
            CompileTimeScalarKind::Length => CheckedCompileTimeScalarKind::Length,
            CompileTimeScalarKind::Angle => CheckedCompileTimeScalarKind::Angle,
            CompileTimeScalarKind::PublicId => CheckedCompileTimeScalarKind::PublicId,
            CompileTimeScalarKind::Color => CheckedCompileTimeScalarKind::Color,
            CompileTimeScalarKind::FontFamily => CheckedCompileTimeScalarKind::FontFamily,
        };
        let admission = Self::for_kind(kind, registry);
        (admission.value_type() == value_type).then_some(admission)
    }

    pub(crate) const fn kind(&self) -> &CheckedCompileTimeScalarKind {
        &self.kind
    }

    pub(crate) const fn value_type(&self) -> &TypeKind {
        &self.value_type
    }
}
