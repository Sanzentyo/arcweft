//! Original builder capabilities and source definitions enter the common image.
//! This phase issues no F/Q/C proof, completed task key, table, or public plan.

use super::{RuntimePlanBuildError, RuntimePlanBuilder};
use crate::plan::body_semantic::RuntimeBodySemanticError;
use crate::plan::body_semantic::task_image::{RuntimeTaskPlanSealError, UnsealedRuntimePlanImage};
use crate::plan::{RuntimeTaskPlanSealLimits, RuntimeTaskPlanSeed, RuntimeTaskSemanticBinding};
use crate::task::semantic::{TaskSemanticEncodingError, TaskSemanticMeter};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

mod original_source;

#[derive(Debug)]
pub(super) enum RuntimeTaskPlanDefinition {
    Explicit(Option<RuntimeTaskPlanSeed>),
    FunctionRoot(crate::plan::RuntimeTaskPlanFunctionRootSeed),
    OriginalSource,
    OriginalControl(super::RuntimeControlEffectContractSeedId),
    OriginalComplete(NormalizedTaskPlanSeed),
}

pub(super) type NormalizedTaskPlanSeed = crate::plan::RuntimeTaskPlan<
    crate::plan::RuntimeLineTaskGroupSeedId,
    crate::runtime_id::RuntimeFunctionSiteId,
    crate::plan::RuntimeTaskRequestTemplateSeed,
    crate::plan::RuntimeControlEffectContractSeedId,
>;

pub(super) type RuntimeTaskPlanDefinitions = Arc<Mutex<Option<Vec<RuntimeTaskPlanDefinition>>>>;

/// A construction capability for the original aggregate's sole source slots.
/// It cannot issue a digest or remain mutable after the builder is consumed.
/// Successful reservations define source order. Completed plans retain no
/// mutable source slots.
#[derive(Clone, Debug)]
pub struct RuntimeTaskPlanConstructionContext {
    issuer: Arc<super::seed::RuntimePlanConstructionIssuer>,
    definitions: RuntimeTaskPlanDefinitions,
    poisoned: Arc<AtomicBool>,
}

impl RuntimeTaskPlanConstructionContext {
    fn with_definitions<T>(
        &self,
        operation: impl FnOnce(
            &mut Vec<RuntimeTaskPlanDefinition>,
        ) -> Result<T, RuntimeTaskPlanSealError>,
    ) -> Result<T, RuntimeTaskPlanSealError> {
        if self.poisoned.load(Ordering::Acquire) {
            return Err(RuntimePlanBuildError::Poisoned.into());
        }
        let mut definitions = self.definitions.lock().map_err(|_| {
            self.poisoned.store(true, Ordering::Release);
            RuntimePlanBuildError::Poisoned
        })?;
        if self.poisoned.load(Ordering::Acquire) {
            return Err(RuntimePlanBuildError::Poisoned.into());
        }
        let Some(definitions) = definitions.as_mut() else {
            return Err(RuntimeTaskPlanSealError::ConstructionClosed);
        };
        let result = operation(definitions);
        if result.is_err() {
            self.poisoned.store(true, Ordering::Release);
        }
        result
    }

    pub fn reserve(
        &self,
    ) -> Result<crate::plan::RuntimeTaskPlanBuildCoordinate, RuntimeTaskPlanSealError> {
        self.reserve_definition(RuntimeTaskPlanDefinition::Explicit(None))
    }

    /// Reserves a task whose definition belongs to its actual executable source.
    /// Finish requires exactly one occurrence in an original function body.
    pub fn reserve_original_source(
        &self,
    ) -> Result<crate::plan::RuntimeTaskPlanBuildCoordinate, RuntimeTaskPlanSealError> {
        self.reserve_definition(RuntimeTaskPlanDefinition::OriginalSource)
    }

    fn reserve_definition(
        &self,
        definition: RuntimeTaskPlanDefinition,
    ) -> Result<crate::plan::RuntimeTaskPlanBuildCoordinate, RuntimeTaskPlanSealError> {
        self.with_definitions(|definitions| {
            let ordinal = u32::try_from(definitions.len())
                .map_err(|_| TaskSemanticEncodingError::ArithmeticOverflow)?;
            let rows = ordinal
                .checked_add(1)
                .ok_or(TaskSemanticEncodingError::ArithmeticOverflow)?;
            definitions.push(definition);
            super::task_coordinates::RuntimeTaskPlanCoordinateOwner::new(
                Arc::clone(&self.issuer),
                rows,
            )
            .resolve(ordinal)
            .ok_or(TaskSemanticEncodingError::OwnerRejected.into())
        })
    }

    pub fn define(
        &self,
        coordinate: &crate::plan::RuntimeTaskPlanBuildCoordinate,
        seed: RuntimeTaskPlanSeed,
    ) -> Result<(), RuntimeTaskPlanSealError> {
        self.define_definition(coordinate, RuntimeTaskPlanDefinition::Explicit(Some(seed)))
    }

    pub fn define_function_root(
        &self,
        coordinate: &crate::plan::RuntimeTaskPlanBuildCoordinate,
        seed: crate::plan::RuntimeTaskPlanFunctionRootSeed,
    ) -> Result<(), RuntimeTaskPlanSealError> {
        self.define_definition(coordinate, RuntimeTaskPlanDefinition::FunctionRoot(seed))
    }

    fn define_definition(
        &self,
        coordinate: &crate::plan::RuntimeTaskPlanBuildCoordinate,
        definition: RuntimeTaskPlanDefinition,
    ) -> Result<(), RuntimeTaskPlanSealError> {
        self.with_definitions(|definitions| {
            let rows = u32::try_from(definitions.len())
                .map_err(|_| TaskSemanticEncodingError::ArithmeticOverflow)?;
            let owner = super::task_coordinates::RuntimeTaskPlanCoordinateOwner::new(
                Arc::clone(&self.issuer),
                rows,
            );
            if !owner.contains(coordinate) {
                return Err(RuntimeTaskPlanSealError::ForeignCoordinate);
            }
            let slot = &mut definitions[coordinate.ordinal() as usize];
            if !matches!(slot, RuntimeTaskPlanDefinition::Explicit(None)) {
                return Err(RuntimeTaskPlanSealError::DuplicateDefinition {
                    ordinal: coordinate.ordinal(),
                });
            }
            *slot = definition;
            Ok(())
        })
    }
}

impl RuntimePlanBuilder {
    /// Projects request definition data from this builder's actual defined
    /// function and source coordinate. It issues no F, Q, or task key.
    pub fn task_request_template_for_source(
        &self,
        function: &crate::plan::RuntimeFunctionSiteSeedId,
        coordinate: &crate::plan::RuntimeTaskPlanBuildCoordinate,
        limits: RuntimeTaskPlanSealLimits,
    ) -> Result<crate::plan::RuntimeTaskRequestTemplateSeed, RuntimeTaskPlanSealError> {
        use crate::plan::body_semantic::function::OriginalRequestEndpoint;
        let source = self.original_function_source(function)?;
        let rows = {
            let definitions = self.task_plan_definitions.lock().map_err(|_| {
                self.poisoned.store(true, Ordering::Release);
                RuntimePlanBuildError::Poisoned
            })?;
            u32::try_from(
                definitions
                    .as_ref()
                    .ok_or(RuntimeTaskPlanSealError::ConstructionClosed)?
                    .len(),
            )
            .map_err(|_| TaskSemanticEncodingError::ArithmeticOverflow)?
        };
        let owner = self.task_coordinate_owner(rows);
        let mut meter =
            TaskSemanticMeter::new(limits.max_semantic_work, limits.max_transcript_bytes);
        let endpoint = OriginalRequestEndpoint::from_construction(
            self, source, coordinate, &owner, &mut meter, limits,
        )?;
        self.task_request_seed_for_endpoint(endpoint, &mut meter, limits)
    }

    /// Projects the actual ordered parameter/capture ABI of a defined root.
    /// This returns inert request data and cannot issue F/Q/task proofs.
    pub fn task_request_template_for_function_root(
        &self,
        function: &crate::plan::RuntimeFunctionSiteSeedId,
        limits: RuntimeTaskPlanSealLimits,
    ) -> Result<crate::plan::RuntimeTaskRequestTemplateSeed, RuntimeTaskPlanSealError> {
        let source = self.original_function_source(function)?;
        let mut meter =
            TaskSemanticMeter::new(limits.max_semantic_work, limits.max_transcript_bytes);
        let endpoint =
            crate::plan::body_semantic::function::OriginalRequestEndpoint::from_construction_root(
                self, source, &mut meter, limits,
            )?;
        self.task_request_seed_for_endpoint(endpoint, &mut meter, limits)
    }

    fn original_function_source(
        &self,
        function: &crate::plan::RuntimeFunctionSiteSeedId,
    ) -> Result<
        crate::plan::body_semantic::function::OriginalFunctionSource<'_>,
        RuntimeTaskPlanSealError,
    > {
        self.ensure_usable()?;
        let (function, ..) = function
            .resolve(&self.issuer)
            .ok_or(RuntimePlanBuildError::ForeignFunctionSiteSeed)?;
        let defined = &self.function_sites[function.get().get() as usize - 1];
        let body = defined
            .body
            .as_ref()
            .ok_or(RuntimeBodySemanticError::MissingOriginalRequestEndpoint { function })?;
        Ok(
            crate::plan::body_semantic::function::OriginalFunctionSource {
                function,
                body,
                inputs: &defined.inputs,
            },
        )
    }

    fn task_request_seed_for_endpoint(
        &self,
        endpoint: crate::plan::body_semantic::function::OriginalRequestEndpoint<'_>,
        meter: &mut TaskSemanticMeter,
        limits: RuntimeTaskPlanSealLimits,
    ) -> Result<crate::plan::RuntimeTaskRequestTemplateSeed, RuntimeTaskPlanSealError> {
        endpoint.definition(meter, limits)?.try_map_types(|ty| {
            self.types
                .get(ty)
                .map(crate::plan::RuntimePlanTypeDeclaration::semantic_identity)
                .ok_or_else(|| {
                    RuntimeTaskPlanSealError::from(RuntimeBodySemanticError::UnknownType { ty })
                })
        })
    }

    pub(in crate::plan) fn source_type_is_need(
        &self,
        ty: crate::runtime_id::RuntimePlanTypeId,
    ) -> Result<bool, RuntimeBodySemanticError> {
        self.types
            .get(ty)
            .map(|definition| {
                matches!(
                    definition.projection(),
                    crate::plan::RuntimePlanTypeProjection::Need(_)
                )
            })
            .ok_or(RuntimeBodySemanticError::UnknownType { ty })
    }

    pub(in crate::plan) fn source_local_is_capture(
        &self,
        function: crate::runtime_id::RuntimeFunctionSiteId,
        local: crate::runtime_id::RuntimeLocalDeclarationId,
    ) -> Result<bool, RuntimeBodySemanticError> {
        if !self
            .locals
            .lock()
            .expect("source local queries run no user callbacks")
            .contains(local)
        {
            return Err(RuntimeBodySemanticError::UnknownLocal { local });
        }
        let function = self
            .function_sites
            .get(function.get().get() as usize - 1)
            .ok_or(RuntimeBodySemanticError::MissingRow {
                table: "function sites",
                ordinal: function.get().get() as usize - 1,
            })?;
        Ok(function.inputs.iter().any(|input| {
            input.input_local() == local
                && matches!(
                    input.source(),
                    crate::plan::RuntimeFunctionInputSource::Capture { .. }
                        | crate::plan::RuntimeFunctionInputSource::CapturedParameter { .. }
                )
        }))
    }

    pub(in crate::plan) fn source_local_type(
        &self,
        local: crate::runtime_id::RuntimeLocalDeclarationId,
    ) -> Result<crate::runtime_id::RuntimePlanTypeId, RuntimeBodySemanticError> {
        self.locals
            .lock()
            .expect("source local queries run no user callbacks")
            .ty(local)
            .ok_or(RuntimeBodySemanticError::UnknownLocal { local })
    }

    #[must_use]
    pub fn task_plan_construction(&self) -> RuntimeTaskPlanConstructionContext {
        RuntimeTaskPlanConstructionContext {
            issuer: Arc::clone(&self.issuer),
            definitions: Arc::clone(&self.task_plan_definitions),
            poisoned: Arc::clone(&self.poisoned),
        }
    }
    pub fn reserve_task_plan(
        &mut self,
    ) -> Result<crate::plan::RuntimeTaskPlanBuildCoordinate, RuntimeTaskPlanSealError> {
        self.task_plan_construction().reserve()
    }

    pub fn define_task_plan(
        &mut self,
        coordinate: &crate::plan::RuntimeTaskPlanBuildCoordinate,
        seed: RuntimeTaskPlanSeed,
    ) -> Result<(), RuntimeTaskPlanSealError> {
        self.task_plan_construction().define(coordinate, seed)
    }

    pub fn push_task_plan(
        &mut self,
        seed: RuntimeTaskPlanSeed,
    ) -> Result<crate::plan::RuntimeTaskPlanBuildCoordinate, RuntimeTaskPlanSealError> {
        let coordinate = self.reserve_task_plan()?;
        self.define_task_plan(&coordinate, seed)?;
        Ok(coordinate)
    }

    pub fn push_task_plan_function_root(
        &mut self,
        seed: crate::plan::RuntimeTaskPlanFunctionRootSeed,
    ) -> Result<crate::plan::RuntimeTaskPlanBuildCoordinate, RuntimeTaskPlanSealError> {
        let coordinate = self.reserve_task_plan()?;
        self.task_plan_construction()
            .define_function_root(&coordinate, seed)?;
        Ok(coordinate)
    }

    pub(in crate::plan) fn prepare_task_image(
        self,
        limits: RuntimeTaskPlanSealLimits,
    ) -> Result<UnsealedRuntimePlanImage, RuntimeTaskPlanSealError> {
        self.prepare_task_image_with_meter(
            limits,
            TaskSemanticMeter::new(limits.max_semantic_work, limits.max_transcript_bytes),
        )
    }

    /// Source admission transfers its original bank into this same preparation
    /// path. Tightening policy preserves usage and the first failure.
    pub(in crate::plan) fn prepare_task_image_with_meter(
        mut self,
        limits: RuntimeTaskPlanSealLimits,
        mut meter: TaskSemanticMeter,
    ) -> Result<UnsealedRuntimePlanImage, RuntimeTaskPlanSealError> {
        meter.constrain(limits.max_semantic_work, limits.max_transcript_bytes)?;
        self.ensure_usable()?;
        let definitions = {
            let mut definitions = self.task_plan_definitions.lock().map_err(|_| {
                self.poisoned.store(true, Ordering::Release);
                RuntimePlanBuildError::Poisoned
            })?;
            self.ensure_usable()?;
            definitions
                .take()
                .ok_or(RuntimeTaskPlanSealError::ConstructionClosed)?
        };
        let tasks = meter.checked_ordinal(definitions.len())?;
        if tasks > limits.max_task_plan_rows {
            return Err(RuntimeTaskPlanSealError::TaskRows {
                actual: definitions.len(),
                maximum: limits.max_task_plan_rows,
            });
        }
        self.preflight_executable_inventory(limits, tasks)?;
        let definitions = self.complete_original_source_tasks(definitions, &mut meter, limits)?;
        // Source roles are inert definitions. Bound every row and path before
        // the owning type mapping allocates its candidate metadata buffers.
        let mut views = 0;
        for (ordinal, seed) in definitions.iter().enumerate() {
            let seed = seed
                .as_ref()
                .ok_or(RuntimeTaskPlanSealError::IncompleteDefinition { ordinal })?;
            let roles = seed.request_template.role_count(&mut meter)?;
            if roles > limits.max_request_roles as usize {
                return Err(RuntimeBodySemanticError::RequestRoles {
                    actual: roles,
                    maximum: limits.max_request_roles,
                }
                .into());
            }
            seed.request_template
                .try_visit_path_lengths(&mut |actual| {
                    if actual > limits.max_children_per_row as usize {
                        Err(RuntimeTaskPlanSealError::Children {
                            table: 14,
                            ordinal,
                            actual,
                            maximum: limits.max_children_per_row,
                        })
                    } else {
                        Ok(())
                    }
                })?;
            if matches!(seed.binding, RuntimeTaskSemanticBinding::View) {
                views = meter.checked_count_sum(views, 1)?;
            }
        }
        if views > limits.max_view_bindings as usize {
            return Err(RuntimeTaskPlanSealError::ViewRows {
                actual: views,
                maximum: limits.max_view_bindings,
            });
        }
        let owner = self.task_coordinate_owner(tasks);
        let mut rows = Vec::with_capacity(definitions.len());
        for (ordinal, seed) in definitions.into_iter().enumerate() {
            let seed = seed.ok_or(RuntimeTaskPlanSealError::IncompleteDefinition { ordinal })?;
            seed.family
                .validate_runtime_task_binding(&seed.binding)
                .map_err(|source| RuntimeTaskPlanSealError::TaskDefinition { ordinal, source })?;
            let producer_function = seed.producer_function;
            let control_effect = seed.control_effect.resolve(&self)?;
            let binding = seed.binding.try_complete_line(|line| {
                line.resolve(&self.issuer).ok_or_else(|| {
                    RuntimeTaskPlanSealError::from(RuntimePlanBuildError::ForeignLineTaskGroupSeed)
                })
            })?;
            let request_template = seed.request_template.try_map_types(|semantic_identity| {
                self.types
                    .id_for_semantic(semantic_identity)
                    .ok_or_else(|| {
                        RuntimeTaskPlanSealError::from(RuntimePlanBuildError::UnknownSeedType {
                            context: "task request role",
                            semantic_identity,
                        })
                    })
            })?;
            rows.push(crate::plan::RuntimeTaskPlan {
                producer_function,
                family: seed.family,
                class: seed.class,
                request_template,
                control_effect,
                binding,
            });
        }
        let inventory = self.prepare_inventory(limits, tasks)?;
        inventory.verify()?;
        UnsealedRuntimePlanImage::new(inventory, owner, rows.into_boxed_slice(), None, meter)
    }
}
