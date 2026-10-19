//! Source-bound producer specifications, before registry publication.

use crate::plan::body_semantic::flow::RuntimeBodyTaskSource;
use crate::plan::task_source::{RuntimeTaskSource, RuntimeTaskSourceProgram, RuntimeTaskSourceRef};
use crate::program_types::RuntimeFunctionEffectInstantiation;
use crate::task::{NeedProducerRuntimeArgument, NeedProducerSpec, RuntimeHostArgumentTemplate};
use crate::value::{
    AwbcRuntimeValueSnapshotError, RuntimeRetainedValueAdmission, RuntimeStoredValue,
};

#[derive(Debug, thiserror::Error)]
pub(crate) enum RuntimeTaskProducerSourceError {
    #[error("the completed source has a different producer request kind")]
    RequestKind,
    #[error("producer argument admission must select the exact original source program")]
    OriginalProgram,
    #[error("producer arguments have {actual} values; original request requires {expected}")]
    ArgumentCount { actual: usize, expected: usize },
    #[error("producer argument {index} differs from its original binding name")]
    ArgumentName { index: usize },
    #[error("producer argument {index} differs from its original role or spread")]
    ArgumentRole { index: usize },
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
        let arguments =
            super::need_producer_arguments_digest(arguments.iter().map(|argument| &argument.value))
                .map_err(|error| {
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

    /// Admits source operands before host argument expansion or request
    /// projection. The original operand grammar and the actual backend's value
    /// authority remain distinct, including for a compiled AWBC execution.
    pub(crate) fn host_producer_spec<V: RuntimeStoredValue>(
        &self,
        program: &RuntimeTaskSourceProgram,
        arguments: &[RuntimeHostArgumentTemplate<V>],
        binding: Option<&RuntimeFunctionEffectInstantiation>,
        admission: &mut RuntimeRetainedValueAdmission,
    ) -> Result<NeedProducerSpec<RuntimeTaskSource>, RuntimeTaskProducerSourceError> {
        admission.status()?;
        admission.validate_metadata_collection(arguments.len())?;
        let Some(RuntimeBodyTaskSource::Host(target)) = self.request_endpoint().task_source()
        else {
            return Err(RuntimeTaskProducerSourceError::RequestKind);
        };
        let Some(owner) = admission.program_owner() else {
            return Err(RuntimeTaskProducerSourceError::OriginalProgram);
        };
        if !program.owns_source_for(self, owner) {
            return Err(RuntimeTaskProducerSourceError::OriginalProgram);
        }
        admission.validate_metadata(&(
            target.producer.contract,
            target.producer.site,
            &target.public_id,
            &target.capability,
            &target.operation,
            target.contract,
            target.deterministic,
        ))?;
        if arguments.len() != target.args.len() {
            return Err(RuntimeTaskProducerSourceError::ArgumentCount {
                actual: arguments.len(),
                expected: target.args.len(),
            });
        }
        // Check the complete operand shape before the first value projection.
        for (index, (argument, expected)) in arguments.iter().zip(&target.args).enumerate() {
            admission.validate_metadata(&argument.name())?;
            if argument.name() != expected.name() {
                return Err(RuntimeTaskProducerSourceError::ArgumentName { index });
            }
            if argument.identity() != expected.identity()
                || argument.semantic_tag() != expected.semantic_tag()
            {
                return Err(RuntimeTaskProducerSourceError::ArgumentRole { index });
            }
        }
        for (argument, expected) in arguments.iter().zip(&target.args) {
            let ty = program
                .original()
                .type_table()
                .get(expected.value().ty())
                .expect("the common seal admitted every original host operand type")
                .semantic_identity();
            admission.validate_typed_borrowed_in_invocation(
                argument.value().borrowed_value(),
                ty,
                binding,
            )?;
        }
        let arguments = super::need_producer_arguments_digest(
            arguments.iter().map(RuntimeHostArgumentTemplate::value),
        )
        .map_err(|error| {
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
        let payload = program
            .original()
            .type_table()
            .get(target.result)
            .expect("the common seal admitted the original host result type")
            .semantic_identity();
        Ok(NeedProducerSpec {
            family: source.task_plan().definition().family(),
            contract: target.producer.contract,
            plan: source,
            producer_site: target.producer.site,
            payload_type: crate::task::RuntimeTypeSemanticDigest::from_bytes(*payload.as_bytes()),
            arguments,
        })
    }
}
