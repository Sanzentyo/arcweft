//! Source-bound producer specifications, before registry publication.

use crate::plan::body_semantic::flow::RuntimeBodyTaskSource;
use crate::plan::task_source::{RuntimeTaskSource, RuntimeTaskSourceProgram, RuntimeTaskSourceRef};
use crate::program_types::RuntimeFunctionEffectInstantiation;
use crate::task::{NeedProducerRuntimeArgument, NeedProducerSpec, RuntimeHostArgumentTemplate};
use crate::value::{
    AwbcRuntimeValueSnapshotError, RuntimeRetainedValueAdmission, RuntimeStoredValue,
};

mod template;
pub(crate) use template::RuntimeTaskTemplateArguments;

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
    #[error("producer capture count is {actual}; original source requires {expected}")]
    CaptureCount { actual: usize, expected: usize },
    #[error("AwaitMany source is not its original sequence or homogeneous tuple")]
    SourceShape,
    #[error("AwaitMany source length {actual} exceeds the u32 child-index domain")]
    SourceCount { actual: usize },
    #[error(transparent)]
    Admission(#[from] AwbcRuntimeValueSnapshotError),
    #[error(transparent)]
    Request(#[from] crate::task::NeedProducerRequestError),
    #[error(transparent)]
    Identity(#[from] crate::task::TaskIdentityError),
    #[error("stored task specification differs from its admitted original producer")]
    Specification,
}

impl From<RuntimeTaskProducerSourceError> for AwbcRuntimeValueSnapshotError {
    fn from(error: RuntimeTaskProducerSourceError) -> Self {
        match error {
            RuntimeTaskProducerSourceError::Admission(error) => error,
            error => Self::Message {
                message: error.to_string(),
            },
        }
    }
}

pub(crate) struct PreparedRuntimeSelectedProducer<'source, 'bank> {
    pub(super) source: &'source RuntimeTaskSourceRef<'source>,
    pub(super) program: &'source RuntimeTaskSourceProgram,
    pub(super) metadata: &'source crate::task::NeedProducerTaskPlan,
    arguments: &'source [RuntimeHostArgumentTemplate],
    pub(super) admission: &'bank mut RuntimeRetainedValueAdmission,
}

impl PreparedRuntimeSelectedProducer<'_, '_> {
    pub(crate) fn arguments(&self) -> &[RuntimeHostArgumentTemplate] {
        self.arguments
    }

    pub(crate) fn argument_types(&self) -> &[crate::pattern::RuntimeSemanticTypeId] {
        self.metadata.argument_types()
    }

    pub(super) fn producer_spec<V: RuntimeStoredValue>(
        &mut self,
        arguments: &[NeedProducerRuntimeArgument<V>],
        binding: Option<&RuntimeFunctionEffectInstantiation>,
    ) -> Result<NeedProducerSpec<RuntimeTaskSource>, RuntimeTaskProducerSourceError> {
        let admission = &mut self.admission;
        admission.status()?;
        admission.validate_metadata_collection(arguments.len())?;
        if arguments.len() != self.metadata.argument_types().len() {
            return Err(RuntimeTaskProducerSourceError::ArgumentCount {
                actual: arguments.len(),
                expected: self.metadata.argument_types().len(),
            });
        }
        for (index, (argument, expected)) in arguments
            .iter()
            .zip(self.metadata.argument_types())
            .enumerate()
        {
            admission.validate_metadata(&argument.name)?;
            if self.metadata.argument_name(index) != Some(argument.name.as_deref()) {
                return Err(RuntimeTaskProducerSourceError::ArgumentName { index });
            }
            admission.validate_typed_borrowed_in_invocation(
                argument.value.borrowed_value(),
                *expected,
                binding,
            )?;
        }
        let arguments = super::need_producer_arguments_digest(
            arguments.iter().map(|argument| &argument.value),
            admission.work().limits().platform_encoded_bytes(),
        )
        .map_err(|error| {
            let error = AwbcRuntimeValueSnapshotError::from(error);
            admission.retain_failure(error.clone());
            RuntimeTaskProducerSourceError::Admission(error)
        })?;
        let source = RuntimeTaskSource::from_borrowed(
            self.source,
            self.program,
            admission
                .program_owner()
                .ok_or(RuntimeTaskProducerSourceError::OriginalProgram)?,
        )
        .ok_or(RuntimeTaskProducerSourceError::OriginalProgram)?;
        Ok(NeedProducerSpec {
            family: source.task_plan().definition().family(),
            contract: self.metadata.contract(),
            plan: source,
            producer_site: self.metadata.site(),
            payload_type: crate::task::RuntimeTypeSemanticDigest::from_bytes(
                *self.metadata.payload_type().as_bytes(),
            ),
            arguments,
        })
    }
}

impl RuntimeTaskSourceRef<'_> {
    /// Bounds the original operand grammar before evaluation allocates argument
    /// storage or copies a binding name. This token borrows the same value bank.
    pub(crate) fn prepare_selected_producer<'source, 'bank>(
        &'source self,
        program: &'source RuntimeTaskSourceProgram,
        admission: &'bank mut RuntimeRetainedValueAdmission,
    ) -> Result<PreparedRuntimeSelectedProducer<'source, 'bank>, RuntimeTaskProducerSourceError>
    {
        admission.status()?;
        let Some(RuntimeBodyTaskSource::Start(target)) = self.request_endpoint().task_source()
        else {
            return Err(RuntimeTaskProducerSourceError::RequestKind);
        };
        if !admission
            .program_owner()
            .is_some_and(|owner| program.owns_source_for(self, owner))
        {
            return Err(RuntimeTaskProducerSourceError::OriginalProgram);
        }
        let metadata = target.plan();
        metadata.admit_retained_metadata(admission)?;
        admission.validate_metadata_collection(target.arguments().len())?;
        if target.arguments().len() != metadata.argument_types().len() {
            return Err(RuntimeTaskProducerSourceError::ArgumentCount {
                actual: target.arguments().len(),
                expected: metadata.argument_types().len(),
            });
        }
        for (index, argument) in target.arguments().iter().enumerate() {
            admission.validate_metadata(&argument.name())?;
            if metadata.argument_name(index) != Some(argument.name()) {
                return Err(RuntimeTaskProducerSourceError::ArgumentName { index });
            }
            if argument.is_spread() {
                return Err(RuntimeTaskProducerSourceError::ArgumentRole { index });
            }
        }
        Ok(PreparedRuntimeSelectedProducer {
            source: self,
            program,
            metadata,
            arguments: target.arguments(),
            admission,
        })
    }

    /// Uses the original issued row, canonical request and existing value bank.
    pub(crate) fn selected_producer_spec<V: RuntimeStoredValue>(
        &self,
        program: &RuntimeTaskSourceProgram,
        arguments: &[NeedProducerRuntimeArgument<V>],
        binding: Option<&RuntimeFunctionEffectInstantiation>,
        admission: &mut RuntimeRetainedValueAdmission,
    ) -> Result<NeedProducerSpec<RuntimeTaskSource>, RuntimeTaskProducerSourceError> {
        self.prepare_selected_producer(program, admission)?
            .producer_spec(arguments, binding)
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
            admission.work().limits().platform_encoded_bytes(),
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
