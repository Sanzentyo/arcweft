//! Accepted producer definitions. Dynamic argument values enter the canonical
//! value visitor before the sole producer-instance issuer constructs a spec.

use super::*;
use crate::value::{RuntimeTupleView, RuntimeValueView};

/// Static call producer seal emitted from the accepted semantic expression.
#[derive(
    Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, serde::Deserialize, serde::Serialize,
)]
#[serde(deny_unknown_fields)]
pub struct HostCallProducerDefinition {
    pub contract: NeedProducerContractDigest,
    pub plan: TaskPlanSemanticDigest,
    pub site: NeedProducerSiteDigest,
}

impl HostCallProducerDefinition {
    pub(crate) fn instantiate(
        self,
        generation: GenerationId,
        result: crate::pattern::RuntimeSemanticTypeId,
        request: HostTaskRequest,
    ) -> Result<TaskSpec, NeedProducerAdmissionError> {
        let arguments = request.runtime_values().collect::<Vec<_>>();
        let arguments = crate::entry::schema::canonical_runtime_value_view_digest(
            RuntimeValueView::Tuple(RuntimeTupleView::Borrowed(&arguments)),
            crate::entry::RuntimeSchemaLimits::engine_default().platform_encoded_bytes(),
        )
        .map_err(|_| NeedProducerAdmissionError::InvalidProducerArguments)?;
        let producer = NeedProducerSpec::new(
            NeedProducerFamily::HostAdapterTask,
            self.contract,
            self.plan,
            self.site,
            RuntimeTypeSemanticDigest::from_bytes(*result.as_bytes()),
            arguments,
        );
        Ok(TaskSpec {
            generation,
            producer: NeedProducerInstance::try_from(&producer)?,
            class: request.task_class(),
            priority: TaskPriority(0),
            cancel_scope: CancelScopeId("runtime-host-call".to_owned()),
            policy: TaskPolicy::AlwaysStart,
            outcome: TaskOutcomeContract::program(result),
            debug_label: request.host_call_id(),
            request,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct NeedProducerTemplate {
    pub family: NeedProducerFamily,
    pub contract: NeedProducerContractDigest,
    pub plan: TaskPlanSemanticDigest,
    pub producer_site: NeedProducerSiteDigest,
    pub payload_type: RuntimeTypeSemanticDigest,
    pub class: TaskClass,
    pub priority: TaskPriority,
    pub cancel_scope: CancelScopeId,
    pub policy: TaskPolicy,
    pub outcome: TaskOutcomeContract,
    pub request: HostTaskRequestTemplate,
    pub debug_label: String,
}

impl NeedProducerTemplate {
    pub fn instantiate(
        &self,
        generation: GenerationId,
        arguments: &RuntimeValue,
        request: HostTaskRequest,
        max_encoded_bytes: usize,
    ) -> Result<TaskSpec, NeedProducerAdmissionError> {
        self.instantiate_view(generation, arguments.view(), request, max_encoded_bytes)
    }

    pub(crate) fn instantiate_view(
        &self,
        generation: GenerationId,
        arguments: RuntimeValueView<'_>,
        request: HostTaskRequest,
        max_encoded_bytes: usize,
    ) -> Result<TaskSpec, NeedProducerAdmissionError> {
        let digest =
            crate::entry::schema::canonical_runtime_value_view_digest(arguments, max_encoded_bytes)
                .map_err(|_| NeedProducerAdmissionError::InvalidProducerArguments)?;
        let input = NeedProducerSpec::new(
            self.family,
            self.contract,
            self.plan,
            self.producer_site,
            self.payload_type,
            digest,
        );
        let spec = TaskSpec {
            generation,
            producer: NeedProducerInstance::try_from(&input)?,
            class: self.class.clone(),
            priority: self.priority,
            cancel_scope: self.cancel_scope.clone(),
            policy: self.policy,
            outcome: self.outcome.clone(),
            request,
            debug_label: self.debug_label.clone(),
        };
        spec.validate_outcome()?;
        Ok(spec)
    }
}

impl AwaitManyTarget {
    pub fn captures(
        &self,
    ) -> impl Iterator<Item = crate::runtime_id::RuntimeLocalDeclarationId> + '_ {
        let mut seen = std::collections::BTreeSet::new();
        self.base
            .request
            .captures()
            .iter()
            .chain(self.child.request.captures())
            .copied()
            .filter(move |local| seen.insert(*local))
    }

    pub fn instantiate_base(
        &self,
        generation: GenerationId,
        captured: &[RuntimeValue],
        source: &[RuntimeValue],
        request: HostTaskRequest,
        max_encoded_bytes: usize,
    ) -> Result<TaskSpec, NeedProducerAdmissionError> {
        u32::try_from(source.len())
            .map_err(|_| NeedProducerAdmissionError::InvalidProducerArguments)?;
        let fields = [
            RuntimeValueView::Tuple(RuntimeTupleView::Values(captured)),
            RuntimeValueView::Tuple(RuntimeTupleView::Values(source)),
        ];
        self.base.instantiate_view(
            generation,
            RuntimeValueView::Tuple(RuntimeTupleView::Views(&fields)),
            request,
            max_encoded_bytes,
        )
    }

    pub fn instantiate_child(
        &self,
        generation: GenerationId,
        captured: &[RuntimeValue],
        source_index: u32,
        item: &RuntimeValue,
        request: HostTaskRequest,
        max_encoded_bytes: usize,
    ) -> Result<TaskSpec, NeedProducerAdmissionError> {
        let index = RuntimeValue::u32(source_index);
        let fields = [
            RuntimeValueView::Tuple(RuntimeTupleView::Values(captured)),
            index.view(),
            item.view(),
        ];
        self.child.instantiate_view(
            generation,
            RuntimeValueView::Tuple(RuntimeTupleView::Views(&fields)),
            request,
            max_encoded_bytes,
        )
    }
}
