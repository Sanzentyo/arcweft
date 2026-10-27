//! Original Program body and owned fanout ingress admission.

use super::*;
use crate::plan::{
    RuntimeProgramFanout, RuntimeProgramTaskBody, RuntimeProgramTaskCapture,
    RuntimeProgramTaskTemplate,
};
use crate::task::{NeedProducerDefinition, TaskHandleType, TaskPolicy};

impl RuntimePlanBodyConstruction<'_> {
    pub(super) fn lower_program_captures(
        &self,
        captures: Vec<RuntimeProgramTaskCapture<RuntimeLocalSeedId>>,
    ) -> Result<Vec<RuntimeProgramTaskCapture>, RuntimePlanBuildError> {
        let mut selected = BTreeSet::new();
        captures
            .into_iter()
            .map(|capture| {
                let (local, _) = self.resolve_local(&capture.local)?;
                if !selected.insert(local) {
                    return Err(RuntimePlanBuildError::DuplicateProgramTaskCapture { local });
                }
                Ok(RuntimeProgramTaskCapture {
                    local,
                    mode: capture.mode,
                })
            })
            .collect()
    }

    pub(super) fn lower_program_body(
        &self,
        producer: &NeedProducerDefinition,
        captures: &[RuntimeProgramTaskCapture],
        body: RuntimeProgramTaskBody<RuntimeLocalSeedId, RuntimeFlowOpSeed>,
    ) -> Result<RuntimeProgramTaskBody, RuntimePlanBuildError> {
        match body {
            RuntimeProgramTaskBody::Flow(body) => {
                Ok(RuntimeProgramTaskBody::Flow(self.lower_flow_ops(body)?))
            }
            RuntimeProgramTaskBody::Fanout(fanout) => {
                let (source, source_type) = self.resolve_local(&fanout.source)?;
                let (limit, limit_type) = self.resolve_local(&fanout.limit)?;
                let (item, item_type) = self.resolve_local(&fanout.item)?;
                require_same(
                    "Program fanout item",
                    self.await_many_item_type(source_type)?,
                    item_type,
                )?;
                self.require_projection("Program fanout limit", limit_type, |projection| {
                    matches!(
                        projection,
                        RuntimePlanTypeProjection::Unsigned(RuntimeUnsignedIntWidth::U32)
                    )
                })?;
                let worker = fanout.worker;
                if !matches!(producer.handle_type, TaskHandleType::Need { .. })
                    || !matches!(worker.producer.handle_type, TaskHandleType::Need { .. })
                    || worker.producer.policy != TaskPolicy::AlwaysStart
                    || worker
                        .captures
                        .iter()
                        .any(|capture| capture.mode != crate::value::RuntimeLocalReadMode::Move)
                {
                    return Err(RuntimePlanBuildError::InvalidProgramFanoutWorker);
                }
                for (definition, binding, context) in [
                    (
                        producer,
                        crate::plan::RuntimeTaskSemanticBinding::<
                            super::super::RuntimeLineTaskGroupSeedId,
                        >::AwaitManyBase,
                        "Program fanout aggregate",
                    ),
                    (
                        &worker.producer,
                        crate::plan::RuntimeTaskSemanticBinding::AwaitManyChild,
                        "Program fanout worker",
                    ),
                ] {
                    definition
                        .family
                        .validate_runtime_task_binding(&binding)
                        .map_err(
                            |source| RuntimePlanBuildError::InvalidTaskProducerDefinition {
                                context,
                                source,
                            },
                        )?;
                }
                let worker_captures = self.lower_program_captures(worker.captures)?;
                for local in [source, limit] {
                    if !captures.iter().any(|capture| capture.local == local) {
                        return Err(RuntimePlanBuildError::InvalidProgramFanoutIngress { local });
                    }
                }
                if captures.iter().any(|capture| capture.local == item)
                    || !worker_captures.iter().any(|capture| capture.local == item)
                {
                    return Err(RuntimePlanBuildError::InvalidProgramFanoutIngress { local: item });
                }
                for capture in &worker_captures {
                    if capture.local != item
                        && (capture.local == source
                            || capture.local == limit
                            || !captures.iter().any(|parent| parent.local == capture.local))
                    {
                        return Err(RuntimePlanBuildError::InvalidProgramFanoutIngress {
                            local: capture.local,
                        });
                    }
                }
                Ok(RuntimeProgramTaskBody::Fanout(Box::new(
                    RuntimeProgramFanout {
                        source,
                        limit,
                        item,
                        worker: RuntimeProgramTaskTemplate {
                            producer: self.lower_producer_definition(worker.producer)?,
                            delivery: worker.delivery,
                            captures: worker_captures,
                            body: self.lower_flow_ops(worker.body)?,
                        },
                    },
                )))
            }
        }
    }
}
