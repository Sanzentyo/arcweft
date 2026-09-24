//! Source identity retains known substitutions even outside the remaining arrow.

use super::{
    CallableGroupIndex, CheckedProjectFunctionCallableOrigin, CheckedProjectFunctionCallableSource,
    CheckedProjectFunctionInstanceProjectionError, CheckedProjectFunctionRuntimeSelectionError,
    TypeProjectionControl,
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CheckedProjectFunctionCallableSourceDigest([u8; 32]);

impl CheckedProjectFunctionCallableSourceDigest {
    // Only private drafts carry this value; finish always computes the digest.
    pub(super) const UNSEALED: Self = Self([0; 32]);

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl CheckedProjectFunctionCallableSource {
    pub(super) fn compute_digest<C: TypeProjectionControl>(
        &self,
        control: &mut C,
    ) -> Result<
        CheckedProjectFunctionCallableSourceDigest,
        CheckedProjectFunctionInstanceProjectionError<C::Error>,
    > {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"arcweft.lang.project-callable-source.v1\0");
        hasher.update(self.checked.id().semantic_digest().as_bytes());
        hasher.update(self.checked.signature().semantic_digest().as_bytes());
        match self.origin {
            CheckedProjectFunctionCallableOrigin::Root => {
                hasher.update(&[0]);
            }
            CheckedProjectFunctionCallableOrigin::Continuation { lineage } => {
                hasher.update(&[1]);
                hasher.update(lineage.as_bytes());
            }
        }
        super::super::write_group(&mut hasher, self.group);
        // Schema admission requires every declared generic to occur in this
        // complete signature or its predicate. Projecting all groups therefore
        // seals earlier known bindings as well as the remaining scheme. Merely
        // hashing the remaining arrow would lose earlier type arguments.
        let declared = self
            .checked
            .signature()
            .declared_function_type_from_group(CallableGroupIndex::ZERO, self.checked.exposed_row())
            .map_err(CheckedProjectFunctionRuntimeSelectionError::from)?;
        let full = self.project_template(&declared, control)?;
        hasher.update(
            full.view()
                .semantic_identity_digest_with_control(control)?
                .as_bytes(),
        );
        hasher.update(
            self.function_type
                .semantic_identity_digest_in_scope_with_control(
                    &crate::types::GenericScope::default(),
                    control,
                )?
                .as_bytes(),
        );
        for parameter in &self.parameters {
            hasher.update(
                parameter
                    .abi_type()
                    .semantic_identity_digest_with_control(control)?
                    .as_bytes(),
            );
            hasher.update(
                parameter
                    .binding_type()
                    .semantic_identity_digest_with_control(control)?
                    .as_bytes(),
            );
        }
        for parameter in &self.retained_parameters {
            hasher.update(
                parameter
                    .binding_type()
                    .semantic_identity_digest_with_control(control)?
                    .as_bytes(),
            );
        }
        if let Some(attached) = &self.attached {
            hasher.update(&[1]);
            hasher.update(
                attached
                    .abi_type()
                    .semantic_identity_digest_with_control(control)?
                    .as_bytes(),
            );
            hasher.update(
                attached
                    .binding_type()
                    .semantic_identity_digest_with_control(control)?
                    .as_bytes(),
            );
        } else {
            hasher.update(&[0]);
        }
        Ok(CheckedProjectFunctionCallableSourceDigest(
            *hasher.finalize().as_bytes(),
        ))
    }
}
