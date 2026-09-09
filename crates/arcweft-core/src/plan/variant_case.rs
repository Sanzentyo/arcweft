//! Borrowed case selection from the admitted plan type and nominal domain.

use thiserror::Error;

use super::{
    RuntimePlan, RuntimePlanTypeDeclaration, RuntimePlanTypeProjection, RuntimeVariantDomain,
};
use crate::pattern::{
    RuntimeBuiltinVariantCaseIdentity, RuntimeBuiltinVariantIdentity, RuntimeVariantIdentity,
};
use crate::runtime_id::RuntimePlanTypeId;

/// One case selected from the plan's canonical type/domain authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimePlanVariantCase<'a> {
    owner: RuntimeVariantIdentity,
    name: &'a str,
    payload: Option<RuntimePlanTypeId>,
}

impl<'a> RuntimePlanVariantCase<'a> {
    #[must_use]
    pub const fn owner(&self) -> &RuntimeVariantIdentity {
        &self.owner
    }

    #[must_use]
    pub const fn name(&self) -> &'a str {
        self.name
    }

    #[must_use]
    pub const fn payload(&self) -> Option<RuntimePlanTypeId> {
        self.payload
    }
}

/// Failure to resolve a case through its admitted plan type.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimePlanVariantCaseError {
    #[error("runtime plan has no type {ty}")]
    UnknownType { ty: RuntimePlanTypeId },
    #[error("runtime plan type {ty} is not a variant")]
    NotVariant { ty: RuntimePlanTypeId },
    #[error("runtime plan type {ty} has no nominal variant domain")]
    MissingDomain { ty: RuntimePlanTypeId },
    #[error("runtime plan type {ty} has no variant case {ordinal}")]
    UnknownCase { ty: RuntimePlanTypeId, ordinal: u32 },
}

impl RuntimePlan {
    /// Resolves names, value identity and payload type for every variant family.
    /// Callers retain plan-local payload IDs instead of reconstructing checked types.
    pub fn variant_case(
        &self,
        ty: RuntimePlanTypeId,
        ordinal: u32,
    ) -> Result<RuntimePlanVariantCase<'_>, RuntimePlanVariantCaseError> {
        self.type_table()
            .get(ty)
            .ok_or(RuntimePlanVariantCaseError::UnknownType { ty })?
            .select_variant_case(ty, self.variant_domains().get(ty), ordinal)
    }
}

impl RuntimePlanTypeDeclaration {
    pub(crate) fn select_variant_case<'a>(
        &'a self,
        ty: RuntimePlanTypeId,
        nominal_domain: Option<&'a RuntimeVariantDomain>,
        ordinal: u32,
    ) -> Result<RuntimePlanVariantCase<'a>, RuntimePlanVariantCaseError> {
        let unknown_case = || RuntimePlanVariantCaseError::UnknownCase { ty, ordinal };
        let (builtin, payload) = match self.projection() {
            RuntimePlanTypeProjection::Option { some_payload, .. } => {
                let builtin = RuntimeBuiltinVariantIdentity::Option;
                let payload = match builtin
                    .case_at(ordinal)
                    .ok_or_else(unknown_case)?
                    .identity()
                {
                    RuntimeBuiltinVariantCaseIdentity::OptionSome => Some(*some_payload),
                    RuntimeBuiltinVariantCaseIdentity::OptionNone => None,
                    _ => return Err(unknown_case()),
                };
                (builtin, payload)
            }
            RuntimePlanTypeProjection::Result {
                value_payload,
                error_payload,
                ..
            } => {
                let builtin = RuntimeBuiltinVariantIdentity::Result;
                let payload = match builtin
                    .case_at(ordinal)
                    .ok_or_else(unknown_case)?
                    .identity()
                {
                    RuntimeBuiltinVariantCaseIdentity::ResultOk => Some(*value_payload),
                    RuntimeBuiltinVariantCaseIdentity::ResultErr => Some(*error_payload),
                    _ => return Err(unknown_case()),
                };
                (builtin, payload)
            }
            RuntimePlanTypeProjection::BuiltinVariant { owner, cases } => {
                let payload = usize::try_from(ordinal)
                    .ok()
                    .and_then(|index| cases.get(index))
                    .copied()
                    .ok_or_else(unknown_case)?;
                (*owner, payload)
            }
            RuntimePlanTypeProjection::ProjectNominal { .. }
            | RuntimePlanTypeProjection::Opaque { .. } => {
                let domain =
                    nominal_domain.ok_or(RuntimePlanVariantCaseError::MissingDomain { ty })?;
                let case = domain.case(ordinal).ok_or_else(unknown_case)?;
                return Ok(RuntimePlanVariantCase {
                    owner: RuntimeVariantIdentity::Nominal {
                        nominal: domain.nominal().clone(),
                        semantic_identity: self.semantic_identity(),
                    },
                    name: case.name(),
                    payload: case.payload(),
                });
            }
            _ => return Err(RuntimePlanVariantCaseError::NotVariant { ty }),
        };
        let schema = builtin.case_at(ordinal).ok_or_else(unknown_case)?;
        Ok(RuntimePlanVariantCase {
            owner: RuntimeVariantIdentity::Builtin(builtin),
            name: schema.name(),
            payload,
        })
    }
}
