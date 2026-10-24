//! Accepted producer definitions. Dynamic argument values enter the canonical
//! value visitor before the sole producer-instance issuer constructs a spec.

use super::*;
use crate::value::{RuntimeTupleView, RuntimeValueView};

/// Static call producer seal emitted from the accepted semantic expression.
#[derive(
    Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, serde::Deserialize, serde::Serialize,
)]
#[serde(deny_unknown_fields)]
pub struct HostCallProducerDefinition<Plan = TaskPlanSemanticDigest> {
    pub contract: NeedProducerContractDigest,
    pub plan: Plan,
    pub site: NeedProducerSiteDigest,
}

impl HostCallProducerDefinition<crate::plan::RuntimeTaskPlanBuildCoordinate> {
    pub(crate) fn sealed_definition(
        &self,
    ) -> Result<HostCallProducerDefinition, NeedProducerAdmissionError> {
        let plan = self
            .plan
            .sealed_plan()
            .ok_or(NeedProducerAdmissionError::UnsealedTaskPlan)?
            .key();
        Ok(HostCallProducerDefinition {
            contract: self.contract,
            plan,
            site: self.site,
        })
    }
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
            request: request.into(),
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct NeedProducerDefinition<Plan = crate::plan::RuntimeTaskPlanBuildCoordinate> {
    pub function: crate::plan::RuntimeFunctionDefinitionIdentity,
    pub family: NeedProducerFamily,
    pub contract: NeedProducerContractDigest,
    pub plan: Plan,
    pub producer_site: NeedProducerSiteDigest,
    /// Sole payload declaration, resolved against the original program's types.
    /// Every launch derives its Program outcome from this same identity.
    pub payload_type: RuntimeTypeSemanticDigest,
    pub class: TaskClass,
    pub priority: TaskPriority,
    pub cancel_scope: CancelScopeId,
    pub policy: TaskPolicy,
    pub debug_label: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NeedProducerTemplate<
    Plan = crate::plan::RuntimeTaskPlanBuildCoordinate,
    Request = HostTaskRequestTemplate,
> {
    pub definition: NeedProducerDefinition<Plan>,
    pub request: Request,
}

impl<Plan> NeedProducerDefinition<Plan> {
    #[must_use]
    pub const fn outcome(&self) -> TaskOutcomeContract {
        TaskOutcomeContract::Program {
            payload: crate::pattern::RuntimeSemanticTypeId::from_bytes(
                *self.payload_type.as_bytes(),
            ),
        }
    }
}

impl NeedProducerDefinition {
    pub(crate) fn encode_control_metadata(
        &self,
        encoder: &mut super::semantic::TaskSemanticEncoder<'_>,
    ) {
        encoder.digest(self.function.as_bytes());
        encoder.tag(self.family.semantic_tag());
        encoder.digest(self.contract.as_bytes());
        encoder.digest(self.producer_site.as_bytes());
        encoder.digest(self.payload_type.as_bytes());
        encoder.tag(self.class.semantic_tag());
        encoder.ordinal(self.priority.0.cast_unsigned());
        encoder.string(&self.cancel_scope.0);
        encoder.tag(self.policy.semantic_tag());
    }
}

impl<Plan, Request> NeedProducerTemplate<Plan, Request> {
    #[must_use]
    pub const fn outcome(&self) -> TaskOutcomeContract {
        self.definition.outcome()
    }
}

impl NeedProducerTemplate {
    pub(crate) fn encode_control_metadata(
        &self,
        encoder: &mut super::semantic::TaskSemanticEncoder<'_>,
    ) {
        self.definition.encode_control_metadata(encoder);
        encoder.string(&self.request.capability.0);
        encoder.string(&self.request.operation);
        encoder.count(self.request.captures.len());
        for capture in &self.request.captures {
            if encoder.status().is_err() {
                return;
            }
            encoder.enter_role();
            encoder.ordinal(capture.get().get());
        }
    }
}

impl AwaitManyTarget {
    /// Visits the first occurrence in source order before the union retains
    /// its ID. The caller bounds/charges each slot before either owner grows.
    pub(crate) fn try_visit_captures<E>(
        &self,
        mut visit: impl FnMut(crate::runtime_id::RuntimeLocalDeclarationId) -> Result<(), E>,
    ) -> Result<(), E> {
        let mut seen = std::collections::BTreeSet::new();
        for local in self
            .base
            .request
            .captures()
            .iter()
            .chain(self.child.request.captures())
        {
            if !seen.contains(local) {
                visit(*local)?;
                seen.insert(*local);
            }
        }
        Ok(())
    }

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
}
