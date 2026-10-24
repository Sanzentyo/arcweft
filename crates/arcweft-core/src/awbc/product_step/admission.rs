//! One immutable executable and its admitted original Task Source.

use std::sync::Arc;

use crate::awbc::schema::AwbcProgram;
use crate::awbc::verify::{AwbcVerifyBudget, AwbcVerifyContext};
use crate::plan::task_source::RuntimeTaskSourceProgram;
use crate::plan::{RuntimeTaskSourceArtifactLimits, ViewTaskPlanAuthority};

use super::AwbcProductStepBuildError;

/// An actual AWBC program admitted with the upper authorities its original
/// Source requires. Clones retain the same immutable executable and Source;
/// invocation, rollback and restore do not reconstruct either owner.
#[derive(Clone, Debug, PartialEq)]
pub struct AwbcProductProgram {
    pub(super) program: Arc<AwbcProgram>,
    pub(super) task_source: Option<RuntimeTaskSourceProgram>,
    plain_text_context_template_proof:
        Option<crate::value::RuntimeDialoguePlainTextContextTemplateProof>,
}

impl AwbcProductProgram {
    pub fn admit(
        program: Arc<AwbcProgram>,
        verify: AwbcVerifyBudget,
        context: AwbcVerifyContext<'_>,
        source: RuntimeTaskSourceArtifactLimits,
        authority: Option<&dyn ViewTaskPlanAuthority>,
    ) -> Result<Self, AwbcProductStepBuildError> {
        let task_source = RuntimeTaskSourceProgram::prepare_awbc(
            Arc::clone(&program),
            verify,
            context,
            source,
            authority,
        )
        .map_err(|error| AwbcProductStepBuildError::InvalidProgram {
            message: error.to_string(),
        })?;
        program
            .validated_plain_text_context_template()
            .map_err(|error| AwbcProductStepBuildError::InvalidProgram {
                message: error.to_string(),
            })?;
        Ok(Self {
            program,
            task_source,
            plain_text_context_template_proof: None,
        })
    }

    /// Retains the upper catalog's admitted fact with this exact executable.
    /// Once attached, invocation, replacement and restore keep that fact with
    /// the same program/Source owner rather than accepting an independent pair.
    pub fn with_plain_text_context_template_proof(
        mut self,
        proof: crate::value::RuntimeDialoguePlainTextContextTemplateProof,
    ) -> Result<Self, AwbcProductStepBuildError> {
        let reference = self
            .program
            .validated_plain_text_context_template()
            .map_err(|error| AwbcProductStepBuildError::InvalidProgram {
                message: error.to_string(),
            })?;
        if reference.is_none_or(|reference| !proof.matches_ref(reference)) {
            return Err(AwbcProductStepBuildError::InvalidProgram {
                message: "plain-text context proof does not match the AWBC program pointer"
                    .to_owned(),
            });
        }
        self.plain_text_context_template_proof = Some(proof);
        Ok(self)
    }

    #[must_use]
    pub const fn plain_text_context_template_proof(
        &self,
    ) -> Option<crate::value::RuntimeDialoguePlainTextContextTemplateProof> {
        self.plain_text_context_template_proof
    }

    /// Retains the actual executable's type authority for host value admission.
    pub fn program_arc(&self) -> Arc<AwbcProgram> {
        Arc::clone(&self.program)
    }

    /// One private fiber rollback group uses the actual enclosing Source and
    /// the same admission for preflight and value materialization.
    pub(crate) fn restore_fiber_snapshot(
        &self,
        snapshot: crate::awbc::fiber::AwbcFiberStateSnapshot,
    ) -> Result<crate::awbc::fiber::FiberState, crate::value::AwbcRuntimeValueSnapshotError> {
        let owner = crate::task::RuntimeProgramOwner::Awbc(self.program_arc());
        let mut admission = crate::value::RuntimeRetainedValueAdmission::for_program(
            owner.clone(),
            crate::entry::RuntimeSchemaLimits::engine_default(),
        );
        snapshot.admit_retained(&mut admission)?;
        let context = crate::value::RuntimeValueRestoreContext::new(
            &owner,
            self.task_source
                .as_ref()
                .map(|program| crate::task::TaskSourceRestore {
                    program,
                    admission: &admission,
                }),
        );
        snapshot.into_admitted_live_for_program(context)
    }
}

impl std::ops::Deref for AwbcProductProgram {
    type Target = AwbcProgram;
    fn deref(&self) -> &Self::Target {
        &self.program
    }
}
