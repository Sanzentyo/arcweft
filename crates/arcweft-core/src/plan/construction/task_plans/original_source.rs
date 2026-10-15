//! Realize explicitly source-owned reservations from original typed endpoints.

use super::{NormalizedTaskPlanSeed, RuntimeTaskPlanDefinition};
use crate::plan::body_semantic::flow::RuntimeBodyTaskSource;
use crate::plan::body_semantic::function::{OriginalFunctionSource, OriginalRequestEndpoint};
use crate::plan::{
    RuntimeBodySemanticError, RuntimeControlEffectCancellation, RuntimeControlEffectCardinality,
    RuntimeControlEffectContractError, RuntimeControlEffectContractSeed,
    RuntimeControlEffectContractSeedId, RuntimeControlEffectKind, RuntimeControlEffectOrdering,
    RuntimeControlEffectRow, RuntimeControlEffectTerminalBehavior, RuntimeLineTaskGroupSeedId,
    RuntimePlanBuildError, RuntimePlanBuilder, RuntimeTaskControlMode, RuntimeTaskPlan,
    RuntimeTaskPlanSealError, RuntimeTaskPlanSealLimits, RuntimeTaskPlanSeed,
    RuntimeTaskRequestTemplateSeed,
};
use crate::task::semantic::{TaskSemanticEncodingError, TaskSemanticMeter};
use crate::task::{NeedProducerFamily, TaskClass};

impl RuntimeTaskPlanDefinition {
    fn is_original(&self) -> bool {
        matches!(
            self,
            Self::OriginalSource | Self::OriginalControl(_) | Self::OriginalComplete(_)
        )
    }

    fn control(&self) -> Option<&RuntimeControlEffectContractSeedId> {
        match self {
            Self::Explicit(Some(seed)) => Some(&seed.control_effect),
            Self::OriginalComplete(seed) => Some(&seed.control_effect),
            Self::OriginalControl(control) => Some(control),
            Self::FunctionRoot(seed) => Some(&seed.control_effect),
            Self::Explicit(None) | Self::OriginalSource => None,
        }
    }

    fn into_seed(
        self,
        builder: &RuntimePlanBuilder,
    ) -> Result<Option<NormalizedTaskPlanSeed>, RuntimeTaskPlanSealError> {
        Ok(match self {
            Self::Explicit(Some(seed)) => {
                let (producer_function, ..) = seed
                    .producer_function
                    .resolve(&builder.issuer)
                    .ok_or(RuntimePlanBuildError::ForeignFunctionSiteSeed)?;
                Some(RuntimeTaskPlan {
                    producer_function,
                    family: seed.family,
                    class: seed.class,
                    request_template: seed.request_template,
                    control_effect: seed.control_effect,
                    binding: seed.binding,
                })
            }
            Self::OriginalComplete(seed) => Some(seed),
            Self::Explicit(None)
            | Self::OriginalSource
            | Self::OriginalControl(_)
            | Self::FunctionRoot(_) => None,
        })
    }

    fn complete_root(
        self,
        builder: &RuntimePlanBuilder,
        meter: &mut TaskSemanticMeter,
        limits: RuntimeTaskPlanSealLimits,
    ) -> Result<Self, RuntimeTaskPlanSealError> {
        let Self::FunctionRoot(seed) = self else {
            return Ok(self);
        };
        let source = builder.original_function_source(&seed.producer_function)?;
        let endpoint =
            OriginalRequestEndpoint::from_construction_root(builder, source, meter, limits)?;
        let request = builder.task_request_seed_for_endpoint(endpoint, meter, limits)?;
        Ok(Self::Explicit(Some(RuntimeTaskPlanSeed::new(
            seed.producer_function,
            seed.family,
            seed.class,
            request,
            seed.control_effect,
            seed.binding,
        ))))
    }
}

impl RuntimePlanBuilder {
    fn prepare_source_definitions(
        &mut self,
        definitions: &mut [RuntimeTaskPlanDefinition],
        meter: &mut TaskSemanticMeter,
        limits: RuntimeTaskPlanSealLimits,
    ) -> Result<bool, RuntimeTaskPlanSealError> {
        for definition in definitions.iter_mut() {
            let pending = std::mem::replace(definition, RuntimeTaskPlanDefinition::Explicit(None));
            *definition = pending.complete_root(self, meter, limits)?;
        }
        let mut source_rows = 0;
        for definition in definitions.iter() {
            meter.charge_work(1)?;
            if definition.is_original() {
                source_rows = meter.checked_count_sum(source_rows, 1)?;
            }
        }
        if source_rows == 0 {
            return Ok(false);
        }
        let controls = self
            .control_effect_contracts
            .iter()
            .flatten()
            .try_fold(source_rows, |total, control| {
                meter.checked_count_sum(total, control.effects().len())
            })?;
        if controls > limits.max_control_effect_rows as usize {
            return Err(RuntimeControlEffectContractError::EffectRowsLimit {
                actual: meter.checked_ordinal(controls)?,
                maximum: limits.max_control_effect_rows,
            }
            .into());
        }
        for definition in definitions.iter_mut() {
            if matches!(definition, RuntimeTaskPlanDefinition::OriginalSource) {
                *definition = RuntimeTaskPlanDefinition::OriginalControl(
                    self.reserve_control_effect_contract()?,
                );
            }
        }
        Ok(true)
    }

    pub(super) fn complete_original_source_tasks(
        &mut self,
        mut definitions: Vec<RuntimeTaskPlanDefinition>,
        meter: &mut TaskSemanticMeter,
        limits: RuntimeTaskPlanSealLimits,
    ) -> Result<Vec<Option<NormalizedTaskPlanSeed>>, RuntimeTaskPlanSealError> {
        let owner = self.task_coordinate_owner(meter.checked_ordinal(definitions.len())?);
        if !self.prepare_source_definitions(&mut definitions, meter, limits)? {
            return definitions
                .into_iter()
                .map(|definition| definition.into_seed(self))
                .collect();
        }
        let mut claimed = std::collections::BTreeSet::new();
        for index in 0..self.function_sites.len() {
            let defined = &self.function_sites[index];
            let Some(body) = defined.body.as_ref() else {
                continue;
            };
            let function = crate::runtime_id::RuntimeFunctionSiteId::from_accepted_ordinal(
                std::num::NonZeroU32::new(meter.checked_ordinal(index + 1)?)
                    .ok_or(TaskSemanticEncodingError::ArithmeticOverflow)?,
            );
            let mut pending: Vec<(usize, OriginalSourceTaskDefinition)> = Vec::new();
            OriginalRequestEndpoint::visit_construction_tasks(
                self,
                OriginalFunctionSource {
                    function,
                    body,
                    inputs: &defined.inputs,
                },
                meter,
                limits,
                &mut |endpoint, meter| {
                    let source = endpoint
                        .task_source()
                        .expect("visitor supplies task sources");
                    if !owner.contains(source.coordinate()) {
                        return Err(RuntimeBodySemanticError::ForeignTaskCoordinate);
                    }
                    let ordinal = source.coordinate().ordinal() as usize;
                    if !definitions[ordinal].is_original() {
                        return Ok(());
                    }
                    if !claimed.insert(ordinal) {
                        return Err(RuntimeBodySemanticError::AmbiguousOriginalRequestEndpoint {
                            function,
                        });
                    }
                    let child = endpoint
                        .control_child_coordinate()
                        .map(|coordinate| {
                            if !owner.contains(coordinate) {
                                return Err(RuntimeBodySemanticError::ForeignTaskCoordinate);
                            }
                            let ordinal = coordinate.ordinal() as usize;
                            definitions[ordinal].control().cloned().ok_or(
                                RuntimeBodySemanticError::MissingRow {
                                    table: "task plans",
                                    ordinal,
                                },
                            )
                        })
                        .transpose()?;
                    pending.push((
                        ordinal,
                        source.original_definition(self, endpoint, child, meter, limits)?,
                    ));
                    Ok(())
                },
            )?;
            for (ordinal, draft) in pending {
                let control_effect = definitions[ordinal]
                    .control()
                    .expect("the original source reserved its control row")
                    .clone();
                self.define_control_effect_contract(&control_effect, draft.control_effect)?;
                definitions[ordinal] =
                    RuntimeTaskPlanDefinition::OriginalComplete(RuntimeTaskPlan {
                        producer_function: draft.producer_function,
                        family: draft.family,
                        class: draft.class,
                        request_template: draft.request_template,
                        control_effect,
                        binding: draft.binding,
                    });
            }
        }
        definitions
            .into_iter()
            .map(|definition| definition.into_seed(self))
            .collect()
    }
}

type OriginalSourceTaskDefinition = RuntimeTaskPlan<
    RuntimeLineTaskGroupSeedId,
    crate::runtime_id::RuntimeFunctionSiteId,
    RuntimeTaskRequestTemplateSeed,
    RuntimeControlEffectContractSeed,
>;

impl RuntimeBodyTaskSource<'_> {
    fn original_definition(
        &self,
        builder: &RuntimePlanBuilder,
        endpoint: OriginalRequestEndpoint<'_>,
        child: Option<RuntimeControlEffectContractSeedId>,
        meter: &mut TaskSemanticMeter,
        limits: RuntimeTaskPlanSealLimits,
    ) -> Result<OriginalSourceTaskDefinition, RuntimeBodySemanticError> {
        let request = endpoint.definition(meter, limits)?;
        let identity = self.control_identity(meter)?;
        let (family, class, mode, kind, output, cancellation) = match self {
            RuntimeBodyTaskSource::Host(target) => (
                NeedProducerFamily::HostAdapterTask,
                TaskClass::Io,
                match target.mode {
                    crate::step::RuntimeHostCallMode::Immediate => {
                        RuntimeTaskControlMode::StraightLine
                    }
                    crate::step::RuntimeHostCallMode::Suspend => {
                        RuntimeTaskControlMode::MustSuspend
                    }
                },
                RuntimeControlEffectKind::HostOperation,
                builder
                    .types
                    .get(target.result)
                    .ok_or(RuntimeBodySemanticError::UnknownType { ty: target.result })?
                    .semantic_identity(),
                match target.mode {
                    crate::step::RuntimeHostCallMode::Immediate => {
                        RuntimeControlEffectCancellation::NotObserved
                    }
                    crate::step::RuntimeHostCallMode::Suspend => {
                        RuntimeControlEffectCancellation::ObservedNoPayload
                    }
                },
            ),
            RuntimeBodyTaskSource::Producer(producer) => (
                producer.family,
                producer.class.clone(),
                RuntimeTaskControlMode::RuntimeAggregate,
                RuntimeControlEffectKind::RuntimeOperation,
                producer.outcome().payload_semantic_identity(),
                RuntimeControlEffectCancellation::PropagatesToChildren,
            ),
            RuntimeBodyTaskSource::Start(target) => (
                NeedProducerFamily::SelectedCallable,
                target.plan().class().clone(),
                RuntimeTaskControlMode::RuntimeAggregate,
                RuntimeControlEffectKind::RuntimeOperation,
                target.plan().payload_type(),
                RuntimeControlEffectCancellation::PropagatesToChildren,
            ),
        };
        let request = request.try_map_types(|ty| {
            builder
                .types
                .get(ty)
                .map(crate::plan::RuntimePlanTypeDeclaration::semantic_identity)
                .ok_or(RuntimeBodySemanticError::UnknownType { ty })
        })?;
        let control = RuntimeControlEffectContractSeed {
            mode,
            effects: Box::new([RuntimeControlEffectRow {
                kind,
                identity: Some(identity),
                inputs: request
                    .arguments()
                    .iter()
                    .map(|argument| argument.ty)
                    .collect(),
                output: Some(output),
                cardinality: RuntimeControlEffectCardinality::ExactlyOnce,
                ordering: RuntimeControlEffectOrdering::SingleTerminal,
                cancellation,
                terminal: RuntimeControlEffectTerminalBehavior::Value,
            }]),
            children: child.into_iter().collect(),
        };
        Ok(RuntimeTaskPlan {
            producer_function: endpoint.function_id(),
            family,
            class,
            request_template: request,
            control_effect: control,
            binding: endpoint.task_semantic_binding(),
        })
    }
}
