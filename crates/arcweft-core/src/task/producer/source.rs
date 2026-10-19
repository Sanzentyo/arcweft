//! Source-bound selected producer specifications, before registry publication.

use crate::plan::body_semantic::flow::RuntimeBodyTaskSource;
use crate::plan::task_source::{RuntimeTaskSource, RuntimeTaskSourceProgram, RuntimeTaskSourceRef};
use crate::program_types::RuntimeFunctionEffectInstantiation;
use crate::task::{NeedProducerRuntimeArgument, NeedProducerSpec};
use crate::value::{
    AwbcRuntimeValueSnapshotError, RuntimeRetainedValueAdmission, RuntimeStoredValue,
};

#[derive(Debug, thiserror::Error)]
pub(crate) enum RuntimeTaskProducerSourceError {
    #[error("the completed source is not a selected producer request")]
    RequestKind,
    #[error("producer argument admission must select the exact original source program")]
    OriginalProgram,
    #[error("producer arguments have {actual} values; original request requires {expected}")]
    ArgumentCount { actual: usize, expected: usize },
    #[error("producer argument {index} differs from its original binding name")]
    ArgumentName { index: usize },
    #[error(transparent)]
    Admission(#[from] AwbcRuntimeValueSnapshotError),
}

impl RuntimeTaskSourceRef<'_> {
    /// Uses the original issued row, canonical request and existing value bank.
    /// No caller plan digest, copied request definition or fresh bank is accepted.
    pub(crate) fn selected_producer_spec<V: RuntimeStoredValue>(
        &self,
        program: &RuntimeTaskSourceProgram,
        arguments: &[NeedProducerRuntimeArgument<V>],
        binding: Option<&RuntimeFunctionEffectInstantiation>,
        admission: &mut RuntimeRetainedValueAdmission,
    ) -> Result<NeedProducerSpec<RuntimeTaskSource>, RuntimeTaskProducerSourceError> {
        admission.status()?;
        admission.validate_metadata_collection(arguments.len())?;
        let Some(RuntimeBodyTaskSource::Start(target)) = self.request_endpoint().task_source()
        else {
            return Err(RuntimeTaskProducerSourceError::RequestKind);
        };
        let Some(owner) = admission.program_owner() else {
            return Err(RuntimeTaskProducerSourceError::OriginalProgram);
        };
        if !program.owns_source_for(self, owner) {
            return Err(RuntimeTaskProducerSourceError::OriginalProgram);
        }
        let metadata = target.plan();
        metadata.admit_retained_metadata(admission)?;
        if arguments.len() != metadata.argument_types().len() {
            return Err(RuntimeTaskProducerSourceError::ArgumentCount {
                actual: arguments.len(),
                expected: metadata.argument_types().len(),
            });
        }
        for (index, (argument, expected)) in
            arguments.iter().zip(metadata.argument_types()).enumerate()
        {
            admission.validate_metadata(&argument.name)?;
            if metadata.argument_name(index) != Some(argument.name.as_deref()) {
                return Err(RuntimeTaskProducerSourceError::ArgumentName { index });
            }
            admission.validate_typed_borrowed_in_invocation(
                argument.value.borrowed_value(),
                *expected,
                binding,
            )?;
        }
        let arguments = super::need_producer_arguments_digest(arguments).map_err(|error| {
            let error = AwbcRuntimeValueSnapshotError::from(error);
            admission.retain_failure(error.clone());
            RuntimeTaskProducerSourceError::Admission(error)
        })?;
        let source = RuntimeTaskSource::from_borrowed(
            self,
            program,
            admission
                .program_owner()
                .ok_or(RuntimeTaskProducerSourceError::OriginalProgram)?,
        )
        .ok_or(RuntimeTaskProducerSourceError::OriginalProgram)?;
        Ok(NeedProducerSpec {
            family: source.task_plan().definition().family(),
            contract: metadata.contract(),
            plan: source,
            producer_site: metadata.site(),
            payload_type: crate::task::RuntimeTypeSemanticDigest::from_bytes(
                *metadata.payload_type().as_bytes(),
            ),
            arguments,
        })
    }
}
