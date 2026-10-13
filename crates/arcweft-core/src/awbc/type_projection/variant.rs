//! Variant row invariants shared by verification and value/type projection.

use crate::pattern::RuntimeBuiltinVariantCaseIdentity;
use std::collections::BTreeSet;

use super::{
    AwbcProgram, AwbcRuntimeTypeShape, AwbcTypeId, AwbcTypeProjectionError as Error,
    AwbcVariantCase, AwbcVariantIdentity,
};

impl AwbcProgram {
    /// Resolves the declared item of a registry-owned unary payload container.
    pub(crate) fn builtin_variant_payload_item(
        &self,
        ty: AwbcTypeId,
        case: RuntimeBuiltinVariantCaseIdentity,
    ) -> Option<AwbcTypeId> {
        let payload = self.builtin_variant_case_payload(ty, case).ok()??;
        let AwbcRuntimeTypeShape::Tuple(fields) = self.runtime_types.get(payload.index())?.shape()
        else {
            return None;
        };
        let [item] = fields.as_slice() else {
            return None;
        };
        Some(*item)
    }

    /// Borrows the exact payload row, including unit cases, from the canonical
    /// original builtin variant inventory. It never copies or unfolds a body.
    pub(crate) fn builtin_variant_case_payload(
        &self,
        ty: AwbcTypeId,
        case: RuntimeBuiltinVariantCaseIdentity,
    ) -> Result<Option<AwbcTypeId>, Error> {
        let AwbcRuntimeTypeShape::Variant {
            owner: owner @ AwbcVariantIdentity::Builtin(builtin),
            arguments,
            cases,
        } = self
            .runtime_types
            .get(ty.index())
            .ok_or(Error::RuntimeTypeOutOfBounds { index: ty.0 })?
            .shape()
        else {
            return Err(Error::InvalidBuiltinVariant { index: ty.0 });
        };
        let invalid = || Error::InvalidBuiltinVariant { index: ty.0 };
        let (ordinal, _) = builtin.resolve_case(case).ok_or_else(invalid)?;
        self.validate_variant_fields(ty, owner, arguments, cases)?;
        Ok(cases
            .get(usize::try_from(ordinal).map_err(|_| invalid())?)
            .ok_or_else(invalid)?
            .payload)
    }

    /// Checks the complete row without unfolding any nominal payload body.
    pub(crate) fn validate_variant_fields(
        &self,
        ty: AwbcTypeId,
        owner: &AwbcVariantIdentity,
        arguments: &[AwbcTypeId],
        cases: &[AwbcVariantCase],
    ) -> Result<(), Error> {
        let builtin = match owner {
            AwbcVariantIdentity::Nominal { public_id, .. } => {
                self.nominal_identity(*public_id)?;
                None
            }
            AwbcVariantIdentity::Builtin(owner) => {
                if !arguments.is_empty() || cases.len() != owner.cases().len() {
                    return Err(Error::InvalidBuiltinVariant { index: ty.0 });
                }
                Some(*owner)
            }
        };
        for argument in arguments {
            self.runtime_types
                .get(argument.index())
                .ok_or(Error::RuntimeTypeOutOfBounds { index: argument.0 })?;
        }
        let mut names = BTreeSet::new();
        for (ordinal, case) in cases.iter().enumerate() {
            let name = self
                .strings
                .get(case.name.index())
                .ok_or(Error::StringOutOfBounds {
                    index: case.name.0,
                    role: "variant case name",
                })?;
            if name.is_empty() || !names.insert(name) {
                return Err(Error::InvalidVariantCaseName {
                    index: ty.0,
                    ordinal,
                });
            }
            let payload = case
                .payload
                .map(|payload| {
                    self.runtime_types
                        .get(payload.index())
                        .ok_or(Error::RuntimeTypeOutOfBounds { index: payload.0 })
                })
                .transpose()?;
            if let Some(owner) = builtin {
                let expected = &owner.cases()[ordinal];
                if name != expected.name() || payload.is_some() != expected.has_payload() {
                    return Err(Error::InvalidBuiltinVariant { index: ty.0 });
                }
                if let Some(payload) = payload {
                    let AwbcRuntimeTypeShape::Tuple(fields) = payload.shape() else {
                        return Err(Error::InvalidBuiltinVariant { index: ty.0 });
                    };
                    if expected.payload_arity() != Some(fields.len()) {
                        return Err(Error::InvalidBuiltinVariant { index: ty.0 });
                    }
                    for item in fields {
                        self.runtime_types
                            .get(item.index())
                            .ok_or(Error::RuntimeTypeOutOfBounds { index: item.0 })?;
                    }
                }
            }
        }
        Ok(())
    }
}
