//! Deterministic View instruction evaluation and mount reconciliation.

mod support;
mod text;

use support::{
    await_extent, branch_bounds, checked_span_end, control_flow_failure, derive_fx_instance,
    instruction_ordinal, resolve_path,
};

use super::style_scope::{
    BundleViewStyleNodeKind, ViewStyleNodeInput, ViewStyleScopeAllocator, ViewStyleScopeError,
    ViewStyleScopeRuntime, ViewStyleScopeStack,
};

use super::catalog::{ViewDefinitionIndex, ViewProgramCatalog};
use super::owner::{AcceptedViewProgramGeneration, ResolvedMountedViewOwner};
use super::part::ViewPartRuntimeCatalog;
use super::value::{fx_placeholder, fx_to_runtime, runtime_to_fx};
use super::{
    BundleViewDiagnostic, BundleViewDiagnosticCode, BundleViewFrame, BundleViewFxApplication,
    BundleViewFxArgument, BundleViewInstancePath, BundleViewInstancePathSegment,
    BundleViewMountOutput, BundleViewPaintItem, BundleViewRuntime, BundleViewTextOutput,
    MountedView, ViewOccurrenceKey, deterministic_mount_seed,
};
use crate::dialogue::DialogueViewInput;
use crate::presentation_handles::{
    PresentationHandleId, PresentationHandleKind, PresentationHandleRecord,
    PresentationResourceState,
};
use arcweft_bundle::resource_codec::view::{ViewParameterRole, ViewProgramInstruction};
use arcweft_bundle::resource_codec::{
    ViewDefinitionResource, ViewProgramResource, ViewTextResource, ViewValueInputNamespace,
    ViewValueInputSource,
};
use arcweft_core::value::{RuntimeBinding, RuntimeValue};
use arcweft_presentation::fx::{
    FxEvaluationBudget, FxEvaluationError, FxGraphChildPath, FxRuntimeValue, FxSampleContext,
};
use arcweft_view::{
    ViewId, ViewMountState, ViewRegistry, ViewValueEvaluationError, ViewValueProgramId,
    ViewValueProgramInventory,
};
use std::collections::{BTreeMap, BTreeSet};

const VIEW_FRAME_OPERATION_BUDGET: u32 = 65_536;
const VIEW_VALUE_OPERATION_BUDGET: u32 = 65_536;
const VIEW_REPEAT_LIMIT: i32 = 4_096;
const VIEW_RECURSION_LIMIT: usize = 64;

#[derive(Debug)]
struct EvaluationFailure {
    code: BundleViewDiagnosticCode,
    instruction: Option<u32>,
    message: String,
}

impl EvaluationFailure {
    fn new(
        code: BundleViewDiagnosticCode,
        instruction: Option<usize>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            code,
            instruction: instruction.and_then(|value| u32::try_from(value).ok()),
            message: message.into(),
        }
    }

    fn value(instruction: Option<usize>, error: &ViewValueEvaluationError) -> Self {
        let code = match error {
            ViewValueEvaluationError::Program(FxEvaluationError::BudgetExceeded { .. }) => {
                BundleViewDiagnosticCode::EvaluationBudgetExceeded
            }
            ViewValueEvaluationError::UnknownProgram { .. }
            | ViewValueEvaluationError::InputCount { .. }
            | ViewValueEvaluationError::InputType { .. }
            | ViewValueEvaluationError::SlotOutOfBounds { .. }
            | ViewValueEvaluationError::RevisionExhausted { .. }
            | ViewValueEvaluationError::ProgramMismatch { .. }
            | ViewValueEvaluationError::StateSchemaMismatch { .. }
            | ViewValueEvaluationError::Program(_) => BundleViewDiagnosticCode::InvalidValueProgram,
        };
        Self::new(code, instruction, error.to_string())
    }

    fn style_scope(instruction: Option<usize>, error: ViewStyleScopeError) -> Self {
        Self::new(
            BundleViewDiagnosticCode::InvalidControlFlow,
            instruction,
            error.to_string(),
        )
    }
}

struct MountRenderBuilder {
    targets: BTreeSet<String>,
    images: BTreeSet<String>,
    text: Vec<BundleViewTextOutput>,
    paint: Vec<BundleViewPaintItem>,
    fx: Vec<BundleViewFxApplication>,
    style_scopes: ViewStyleScopeRuntime,
    element_targets: Vec<Option<String>>,
    last_target: Option<String>,
}

impl MountRenderBuilder {
    fn new(style_scopes: ViewStyleScopeStack) -> Self {
        Self {
            targets: BTreeSet::new(),
            images: BTreeSet::new(),
            text: Vec::new(),
            paint: Vec::new(),
            fx: Vec::new(),
            style_scopes: ViewStyleScopeRuntime::new(style_scopes),
            element_targets: Vec::new(),
            last_target: None,
        }
    }

    fn is_root_node(&self) -> bool {
        self.element_targets.is_empty()
    }

    fn retain_target(&mut self, target: &str) {
        self.targets.insert(target.to_owned());
        self.last_target = Some(target.to_owned());
    }

    fn fx_target(&self, definition: &str) -> String {
        self.element_targets
            .iter()
            .rev()
            .flatten()
            .next()
            .cloned()
            .or_else(|| self.last_target.clone())
            .unwrap_or_else(|| definition.to_owned())
    }
}

struct ViewEvaluator<'a> {
    catalog: &'a ViewProgramCatalog,
    registry: &'a ViewRegistry,
    generation: AcceptedViewProgramGeneration,
    program: &'a ViewProgramResource,
    program_id: &'a arcweft_view::ViewProgramId,
    parts: &'a ViewPartRuntimeCatalog,
    text: Option<&'a ViewTextResource>,
    inventory: &'a ViewValueProgramInventory,
    logical_time: arcweft_presentation::fx::FxLogicalTime,
    allocator: &'a mut arcweft_view::ViewMountAllocator,
    root_bindings: &'a BTreeMap<String, RuntimeValue>,
    dialogue_inputs:
        BTreeMap<crate::presentation_handles::PresentationHandleId, DialogueTextInput<'a>>,
    mounts: &'a mut BTreeMap<ViewOccurrenceKey, MountedView>,
    axis_seeds: &'a mut super::axis_seed::BundleViewAxisSeedRegistry,
    reduce_motion: bool,
    instruction_budget: u32,
    value_budget: FxEvaluationBudget,
    style_scope_allocator: ViewStyleScopeAllocator,
    visited: BTreeSet<ViewOccurrenceKey>,
    diagnostics: Vec<BundleViewDiagnostic>,
}

struct DialogueTextInput<'a> {
    frame: &'a arcweft_text_model::LineDisplayFrame,
    state: crate::dialogue::DialogueViewState,
}

type ReconciledRootHandles<'a> = (
    Vec<PresentationHandleRecord>,
    BTreeMap<PresentationHandleId, DialogueTextInput<'a>>,
    Vec<BundleViewDiagnostic>,
);

fn reconcile_root_handles<'a>(
    handles: &[PresentationHandleRecord],
    dialogue: &'a [DialogueViewInput<'a>],
) -> ReconciledRootHandles<'a> {
    let dialogue_handles = dialogue
        .iter()
        .map(|input| {
            PresentationHandleRecord::new(
                input.handle.clone(),
                PresentationHandleKind::View,
                input.view.as_str().to_owned(),
                Some("dialogue".to_owned()),
                PresentationResourceState::Mounted,
                None,
                0,
            )
        })
        .collect::<Vec<_>>();
    let dialogue_handle_ids = dialogue_handles
        .iter()
        .map(|handle| handle.id.clone())
        .collect::<BTreeSet<_>>();
    let diagnostics = handles
        .iter()
        .filter(|handle| dialogue_handle_ids.contains(&handle.id))
        .map(|handle| BundleViewDiagnostic {
            code: BundleViewDiagnosticCode::InvalidControlFlow,
            handle: Some(handle.id.clone()),
            mount: None,
            view: Some(handle.resource_id.clone()),
            instruction: None,
            message: format!(
                "presentation handle `{}` collides with a retained dialogue occurrence",
                handle.id
            ),
        })
        .collect();
    let handles = handles
        .iter()
        .filter(|handle| !dialogue_handle_ids.contains(&handle.id))
        .cloned()
        .chain(dialogue_handles)
        .collect();
    let inputs = dialogue
        .iter()
        .map(|input| {
            (
                input.handle.clone(),
                DialogueTextInput {
                    frame: input.frame,
                    state: input.state,
                },
            )
        })
        .collect();
    (handles, inputs, diagnostics)
}

impl BundleViewRuntime {
    /// Reconciles live presentation handles and evaluates every visible View occurrence.
    ///
    /// Bindings update persistent projections by name. A later frame that omits a
    /// binding retains its last typed value; an input that has never been supplied
    /// fails with a structured diagnostic if an executed program consumes it.
    pub fn evaluate(
        &mut self,
        handles: &[PresentationHandleRecord],
        bindings: &[RuntimeBinding],
        reduce_motion: bool,
    ) -> BundleViewFrame {
        self.evaluate_with_dialogue(handles, &[], bindings, reduce_motion)
    }

    /// Reconciles ordinary handles together with typed dialogue View occurrences.
    #[expect(
        clippy::too_many_lines,
        reason = "frame reconciliation keeps ordered handle lifecycle, retained mounts, dialogue roots, and evaluation commit in one atomic orchestration"
    )]
    pub fn evaluate_with_dialogue<'a>(
        &mut self,
        handles: &[PresentationHandleRecord],
        dialogue: &'a [DialogueViewInput<'a>],
        bindings: &[RuntimeBinding],
        reduce_motion: bool,
    ) -> BundleViewFrame {
        if let Err(error) = self.validate_dialogue_inputs(dialogue) {
            return BundleViewFrame {
                mounts: Vec::new(),
                diagnostics: vec![BundleViewDiagnostic::invalid_dialogue_view_owner(&error)],
            };
        }
        let active_required_dialogue_views =
            dialogue.iter().map(|input| input.view.clone()).collect();
        for binding in bindings {
            self.root_bindings
                .insert(binding.name.clone(), binding.value.clone());
        }
        let mut axis_diagnostics = self.discard_invalid_axis_seed_reservations(handles);
        let Some(catalog) = self.catalog.as_ref() else {
            self.mounts.clear();
            self.axis_seeds.retain_mounts(&BTreeSet::new());
            return BundleViewFrame {
                mounts: Vec::new(),
                diagnostics: axis_diagnostics,
            };
        };
        let program = catalog.resource();

        let (all_handles, dialogue_inputs, collisions) = reconcile_root_handles(handles, dialogue);
        let live_handles = all_handles
            .iter()
            .filter(|handle| !handle.is_terminal())
            .filter(|handle| {
                ViewId::parse_public(handle.resource_id.clone())
                    .ok()
                    .and_then(|view| catalog.definition_index(&view))
                    .is_some()
            })
            .map(|handle| handle.id.clone())
            .collect::<BTreeSet<_>>();
        self.mounts
            .retain(|key, _| live_handles.contains(&key.handle));
        let live_root_mounts = self
            .mounts
            .iter()
            .filter(|(key, _)| key.path.segments().is_empty())
            .map(|(_, mounted)| mounted.state.mount())
            .collect::<BTreeSet<_>>();
        self.axis_seeds.retain_mounts(&live_root_mounts);

        let mut evaluator = ViewEvaluator {
            catalog,
            registry: &self.registry,
            generation: self.generation,
            program,
            program_id: catalog.program_id(),
            parts: catalog.parts(),
            text: self.text.as_ref(),
            inventory: &self.inventory,
            logical_time: self.logical_time,
            allocator: &mut self.allocator,
            root_bindings: &self.root_bindings,
            dialogue_inputs,
            mounts: &mut self.mounts,
            axis_seeds: &mut self.axis_seeds,
            reduce_motion,
            instruction_budget: VIEW_FRAME_OPERATION_BUDGET,
            value_budget: FxEvaluationBudget::new(VIEW_VALUE_OPERATION_BUDGET),
            style_scope_allocator: ViewStyleScopeAllocator::default(),
            visited: BTreeSet::new(),
            diagnostics: {
                axis_diagnostics.extend(collisions);
                axis_diagnostics
            },
        };
        let mut output = Vec::new();
        let mut evaluated_handles = BTreeSet::new();

        for handle in &all_handles {
            if handle.is_terminal() {
                continue;
            }
            let Ok(view) = ViewId::parse_public(handle.resource_id.clone()) else {
                if handle.kind == PresentationHandleKind::View
                    && handle.state == PresentationResourceState::Mounted
                {
                    evaluator.diagnostics.push(BundleViewDiagnostic {
                        code: BundleViewDiagnosticCode::MissingDefinition,
                        handle: Some(handle.id.clone()),
                        mount: None,
                        view: Some(handle.resource_id.clone()),
                        instruction: None,
                        message: format!(
                            "presentation handle `{}` references missing View definition `{}`",
                            handle.id, handle.resource_id
                        ),
                    });
                }
                continue;
            };
            let Some(definition_index) = evaluator.catalog.definition_index(&view) else {
                if handle.kind == PresentationHandleKind::View
                    && handle.state == PresentationResourceState::Mounted
                {
                    evaluator.diagnostics.push(BundleViewDiagnostic {
                        code: BundleViewDiagnosticCode::MissingDefinition,
                        handle: Some(handle.id.clone()),
                        mount: None,
                        view: Some(handle.resource_id.clone()),
                        instruction: None,
                        message: format!(
                            "presentation handle `{}` references missing View definition `{}`",
                            handle.id, handle.resource_id
                        ),
                    });
                }
                continue;
            };
            let key = ViewOccurrenceKey {
                handle: handle.id.clone(),
                path: BundleViewInstancePath::default(),
            };
            match evaluator.prepare_occurrence(&key, definition_index, None) {
                Ok(()) => {
                    evaluator.visited.insert(key.clone());
                }
                Err(error) => {
                    evaluator.record_failure(&key, &view, None, error);
                    continue;
                }
            }
            if handle.is_render_visible() {
                evaluated_handles.insert(handle.id.clone());
                output.extend(evaluator.evaluate_occurrence(
                    key,
                    definition_index,
                    0,
                    ViewStyleScopeStack::default(),
                ));
            }
        }

        evaluator.mounts.retain(|key, _| {
            !evaluated_handles.contains(&key.handle) || evaluator.visited.contains(key)
        });
        output.sort_by(|left, right| {
            (&left.handle, &left.path, left.mount).cmp(&(&right.handle, &right.path, right.mount))
        });
        let diagnostics = std::mem::take(&mut evaluator.diagnostics);
        self.required_dialogue_views = active_required_dialogue_views;
        BundleViewFrame {
            mounts: output,
            diagnostics,
        }
    }

    fn discard_invalid_axis_seed_reservations(
        &mut self,
        handles: &[PresentationHandleRecord],
    ) -> Vec<BundleViewDiagnostic> {
        self.axis_seeds
            .cleanup_known_handles(handles)
            .into_iter()
            .map(|handle| BundleViewDiagnostic {
                code: BundleViewDiagnosticCode::InvalidControlFlow,
                handle: Some(handle.clone()),
                mount: None,
                view: None,
                instruction: None,
                message: format!(
                    "pending View axis seed for `{handle}` was discarded because the handle resolved to a non-View resource"
                ),
            })
            .collect()
    }
}

impl ViewEvaluator<'_> {
    fn prepare_occurrence(
        &mut self,
        key: &ViewOccurrenceKey,
        definition_index: ViewDefinitionIndex,
        call_arguments: Option<&BTreeMap<u16, FxRuntimeValue>>,
    ) -> Result<(), EvaluationFailure> {
        let definition = self.definition(definition_index).clone();
        let definition_view = definition.public_id.view_id().clone();
        if let Some(existing) = self.mounts.get(key)
            && existing.view() != &definition_view
        {
            return Err(EvaluationFailure::new(
                BundleViewDiagnosticCode::InvalidControlFlow,
                None,
                format!(
                    "retained occurrence changed definition from `{}` to `{}`",
                    existing.view(),
                    definition.public_id
                ),
            ));
        }
        if !self.mounts.contains_key(key) {
            self.create_occurrence(key, definition_index, &definition, &definition_view)?;
        }

        let mut mounted = self
            .mounts
            .remove(key)
            .expect("prepared occurrence was inserted above");
        let result = self
            .refresh_projected_state(&definition, &mut mounted)
            .and_then(|()| self.refresh_parameters(key, &definition, &mut mounted, call_arguments));
        self.mounts.insert(key.clone(), mounted);
        result
    }

    fn create_occurrence(
        &mut self,
        key: &ViewOccurrenceKey,
        definition_index: ViewDefinitionIndex,
        definition: &ViewDefinitionResource,
        definition_view: &ViewId,
    ) -> Result<(), EvaluationFailure> {
        let registry = self.registry.resolve(definition_view).ok_or_else(|| {
            EvaluationFailure::new(
                BundleViewDiagnosticCode::MissingDefinition,
                None,
                format!("View `{definition_view}` is absent from the accepted registry"),
            )
        })?;
        let mount = self.allocator.allocate().map_err(|error| {
            EvaluationFailure::new(
                BundleViewDiagnosticCode::EvaluationBudgetExceeded,
                None,
                error.to_string(),
            )
        })?;
        let root_axis_seed = key
            .path
            .segments()
            .is_empty()
            .then(|| self.axis_seeds.prepare_root_mount(&key.handle, mount))
            .transpose()
            .map_err(|error| {
                EvaluationFailure::new(
                    BundleViewDiagnosticCode::InvalidControlFlow,
                    None,
                    error.to_string(),
                )
            })?;
        let parameters = self
            .inventory
            .parameter_types()
            .iter()
            .copied()
            .map(fx_placeholder)
            .collect();
        let state = self
            .inventory
            .state_types()
            .iter()
            .copied()
            .map(fx_placeholder)
            .collect();
        let mount_state = ViewMountState::new(
            mount,
            self.program_id.clone(),
            definition.state_schema_hash,
            parameters,
            state,
            self.inventory,
        )
        .map_err(|error| EvaluationFailure::value(None, &error))?;
        self.mounts.insert(
            key.clone(),
            MountedView {
                owner: ResolvedMountedViewOwner::Arcweft {
                    view: definition_view.clone(),
                    registry,
                    definition: definition_index,
                    program: self.program_id.clone(),
                    revision: self.catalog.revision(),
                    generation: self.generation,
                },
                activation_logical_time: self.logical_time,
                deterministic_seed: deterministic_mount_seed(
                    &key.handle,
                    &key.path,
                    definition_view,
                ),
                state: mount_state,
                initialized_parameters: BTreeSet::new(),
                initialized_state: BTreeSet::new(),
                runtime_parameters: BTreeMap::new(),
            },
        );
        if let Some(plan) = root_axis_seed
            && let Err(error) = self.axis_seeds.commit_root_mount(plan)
        {
            self.mounts.remove(key);
            return Err(EvaluationFailure::new(
                BundleViewDiagnosticCode::InvalidControlFlow,
                None,
                error.to_string(),
            ));
        }
        Ok(())
    }

    fn refresh_projected_state(
        &self,
        definition: &ViewDefinitionResource,
        mounted: &mut MountedView,
    ) -> Result<(), EvaluationFailure> {
        for input in self.program.value_inputs.iter().filter(|input| {
            input.namespace == ViewValueInputNamespace::State
                && matches!(
                    input.source,
                    ViewValueInputSource::Projection { .. }
                        | ViewValueInputSource::LifetimeProjection { .. }
                )
        }) {
            let path = match &input.source {
                ViewValueInputSource::Projection { path } => path.clone(),
                ViewValueInputSource::LifetimeProjection { scope, path } => {
                    std::iter::once(scope.clone())
                        .chain(path.iter().cloned())
                        .collect()
                }
                ViewValueInputSource::DefinitionParameter { .. }
                | ViewValueInputSource::Local { .. }
                | ViewValueInputSource::RepeatOrdinal { .. } => unreachable!(),
            };
            let Some(value) = resolve_path(self.root_bindings, &path) else {
                continue;
            };
            let converted = runtime_to_fx(value, input.value_type).map_err(|error| {
                EvaluationFailure::new(
                    BundleViewDiagnosticCode::InputType,
                    None,
                    format!(
                        "View `{}` state projection `{}` cannot initialize slot {}: {error}",
                        definition.public_id,
                        path.join("."),
                        input.slot
                    ),
                )
            })?;
            mounted
                .state
                .set_state(input.slot, converted, self.inventory)
                .map_err(|error| EvaluationFailure::value(None, &error))?;
            mounted.initialized_state.insert(input.slot);
        }
        Ok(())
    }

    #[expect(
        clippy::too_many_lines,
        reason = "supplied values, ordered defaults, and required-parameter validation form one atomic mount hydration rule"
    )]
    fn refresh_parameters(
        &mut self,
        key: &ViewOccurrenceKey,
        definition: &ViewDefinitionResource,
        mounted: &mut MountedView,
        call_arguments: Option<&BTreeMap<u16, FxRuntimeValue>>,
    ) -> Result<(), EvaluationFailure> {
        for parameter in &definition.parameters {
            let supplied_fx =
                call_arguments.and_then(|arguments| arguments.get(&parameter.ordinal));
            let supplied_runtime = match supplied_fx.copied() {
                Some(value) => Some(fx_to_runtime(value).map_err(|error| {
                    EvaluationFailure::new(
                        BundleViewDiagnosticCode::InputType,
                        None,
                        format!(
                            "View `{}` parameter `{}` cannot cross into runtime state: {error}",
                            definition.public_id, parameter.name
                        ),
                    )
                })?),
                None => self.root_bindings.get(&parameter.name).cloned(),
            };
            if let Some(value) = supplied_runtime {
                mounted
                    .runtime_parameters
                    .insert(parameter.name.clone(), value.clone());
                if let (Some(value_type), Some(slot)) = (parameter.value_type, parameter.value_slot)
                {
                    let converted = supplied_fx.copied().map_or_else(
                        || runtime_to_fx(&value, value_type),
                        |value| {
                            if value.value_type() == value_type {
                                Ok(value)
                            } else {
                                Err(super::BundleViewValueConversionError::Type {
                                    expected: value_type,
                                    actual: "Fx argument with another type",
                                })
                            }
                        },
                    )
                    .map_err(|error| {
                        EvaluationFailure::new(
                            BundleViewDiagnosticCode::InputType,
                            None,
                            format!(
                                "View `{}` parameter `{}` cannot initialize slot {slot}: {error}",
                                definition.public_id, parameter.name
                            ),
                        )
                    })?;
                    mounted
                        .state
                        .set_parameter(slot, converted, self.inventory)
                        .map_err(|error| EvaluationFailure::value(None, &error))?;
                    mounted.initialized_parameters.insert(slot);
                }
            }
        }

        for parameter in &definition.parameters {
            let (Some(slot), Some(default_program)) =
                (parameter.value_slot, parameter.default_program)
            else {
                continue;
            };
            if mounted.initialized_parameters.contains(&slot) {
                continue;
            }
            let context = self.sample_context(mounted, 0)?;
            let value = evaluate_value(
                mounted,
                default_program,
                self.inventory,
                context,
                &mut self.value_budget,
                None,
            )?;
            mounted
                .state
                .set_parameter(slot, value, self.inventory)
                .map_err(|error| EvaluationFailure::value(None, &error))?;
            mounted.initialized_parameters.insert(slot);
            let runtime_value = fx_to_runtime(value).map_err(|error| {
                EvaluationFailure::new(
                    BundleViewDiagnosticCode::InputType,
                    None,
                    format!(
                        "View `{}` default parameter `{}` cannot cross into runtime state: {error}",
                        definition.public_id, parameter.name
                    ),
                )
            })?;
            mounted
                .runtime_parameters
                .insert(parameter.name.clone(), runtime_value);
        }

        for parameter in &definition.parameters {
            let initialized = parameter
                .value_slot
                .is_some_and(|slot| mounted.initialized_parameters.contains(&slot))
                || mounted.runtime_parameters.contains_key(&parameter.name)
                || (parameter.role == ViewParameterRole::Dialogue
                    && self.dialogue_inputs.contains_key(&key.handle));
            if !initialized && parameter.default_program.is_none() {
                return Err(EvaluationFailure::new(
                    BundleViewDiagnosticCode::MissingInput,
                    None,
                    format!(
                        "View `{}` requires parameter `{}`",
                        definition.public_id, parameter.name
                    ),
                ));
            }
        }
        Ok(())
    }

    fn evaluate_occurrence(
        &mut self,
        key: ViewOccurrenceKey,
        definition_index: ViewDefinitionIndex,
        depth: usize,
        style_scopes: ViewStyleScopeStack,
    ) -> Vec<BundleViewMountOutput> {
        let definition = self.definition(definition_index).clone();
        if depth >= VIEW_RECURSION_LIMIT {
            self.record_failure(
                &key,
                definition.public_id.view_id(),
                None,
                EvaluationFailure::new(
                    BundleViewDiagnosticCode::RecursionLimitExceeded,
                    None,
                    format!("View call depth exceeds {VIEW_RECURSION_LIMIT}"),
                ),
            );
            return Vec::new();
        }
        self.visited.insert(key.clone());
        let mut mounted = self
            .mounts
            .remove(&key)
            .expect("visible occurrence was prepared before evaluation");
        let rollback = mounted.clone();
        let mut style_scopes = style_scopes;
        let root_style_result = style_scopes
            .enter_definition(&definition.styles, &mut self.style_scope_allocator)
            .map_err(|error| EvaluationFailure::style_scope(None, error));
        let mut builder = MountRenderBuilder::new(style_scopes);
        let mut descendants = Vec::new();
        let start = definition.body.start_instruction as usize;
        let end = definition.body.end_instruction as usize;
        let result = root_style_result.and_then(|()| {
            self.execute_span(
                &key,
                &definition,
                &mut mounted,
                &key.path,
                start,
                end,
                depth,
                &mut builder,
                &mut descendants,
            )
        });
        match result {
            Ok(()) => {
                let mount_id = mounted.state.mount();
                let host_axis_seed = if key.path.segments().is_empty() {
                    let Some(seed) = self.axis_seeds.mounted_seed(mount_id) else {
                        self.mounts.insert(key.clone(), mounted);
                        self.record_failure(
                            &key,
                            definition.public_id.view_id(),
                            Some(mount_id),
                            EvaluationFailure::new(
                                BundleViewDiagnosticCode::InvalidControlFlow,
                                None,
                                "root View mount has no host axis seed",
                            ),
                        );
                        return Vec::new();
                    };
                    Some(seed)
                } else {
                    None
                };
                let dialogue = self
                    .dialogue_inputs
                    .get(&key.handle)
                    .map(|input| input.state);
                self.mounts.insert(key.clone(), mounted);
                let mut output = vec![BundleViewMountOutput {
                    handle: key.handle,
                    mount: mount_id,
                    view: definition.public_id.view_id().clone(),
                    path: key.path,
                    host_axis_seed,
                    dialogue,
                    active_targets: builder.targets.into_iter().collect(),
                    active_images: builder.images.into_iter().collect(),
                    paint: builder.paint,
                    text: builder.text,
                    fx: builder.fx,
                    style_nodes: builder.style_scopes.into_nodes(),
                }];
                output.extend(descendants);
                output
            }
            Err(error) => {
                let mount = rollback.state.mount();
                self.mounts.insert(key.clone(), rollback);
                self.record_failure(&key, definition.public_id.view_id(), Some(mount), error);
                Vec::new()
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    #[expect(
        clippy::too_many_lines,
        reason = "the bounded instruction dispatch is one cohesive interpreter loop; state transitions remain visible in one exhaustive match"
    )]
    fn execute_span(
        &mut self,
        key: &ViewOccurrenceKey,
        definition: &ViewDefinitionResource,
        mounted: &mut MountedView,
        structural_path: &BundleViewInstancePath,
        start: usize,
        end: usize,
        depth: usize,
        builder: &mut MountRenderBuilder,
        descendants: &mut Vec<BundleViewMountOutput>,
    ) -> Result<(), EvaluationFailure> {
        let mut cursor = start;
        while cursor < end {
            self.charge_instruction(cursor)?;
            let instruction = self.program.instructions.get(cursor).ok_or_else(|| {
                EvaluationFailure::new(
                    BundleViewDiagnosticCode::InvalidControlFlow,
                    Some(cursor),
                    "View instruction index is out of bounds",
                )
            })?;
            match instruction {
                ViewProgramInstruction::Branch {
                    condition_program,
                    then_span,
                    else_span,
                    ..
                } => {
                    let (then_start, then_end, else_end) =
                        branch_bounds(cursor, *then_span, *else_span, end)?;
                    let context = self.sample_context(mounted, instruction_ordinal(cursor)?)?;
                    let condition = evaluate_value(
                        mounted,
                        *condition_program,
                        self.inventory,
                        context,
                        &mut self.value_budget,
                        Some(cursor),
                    )?;
                    match condition {
                        FxRuntimeValue::Bool(true) => self.execute_span(
                            key,
                            definition,
                            mounted,
                            structural_path,
                            then_start,
                            then_end,
                            depth,
                            builder,
                            descendants,
                        )?,
                        FxRuntimeValue::Bool(false) if else_span.is_some() => self.execute_span(
                            key,
                            definition,
                            mounted,
                            structural_path,
                            then_end,
                            else_end,
                            depth,
                            builder,
                            descendants,
                        )?,
                        FxRuntimeValue::Bool(false) => {}
                        value => {
                            return Err(EvaluationFailure::new(
                                BundleViewDiagnosticCode::InvalidValueProgram,
                                Some(cursor),
                                format!(
                                    "branch program returned {:?}, expected Bool",
                                    value.value_type()
                                ),
                            ));
                        }
                    }
                    cursor = else_end;
                }
                ViewProgramInstruction::RepeatKeyed {
                    source_program,
                    key_program,
                    body_span,
                    ..
                } => {
                    let body_start = cursor.checked_add(1).ok_or_else(|| {
                        control_flow_failure(cursor, "repeat body start overflow")
                    })?;
                    let body_end = checked_span_end(body_start, *body_span, end, cursor)?;
                    let context = self.sample_context(mounted, instruction_ordinal(cursor)?)?;
                    let count = evaluate_value(
                        mounted,
                        *source_program,
                        self.inventory,
                        context,
                        &mut self.value_budget,
                        Some(cursor),
                    )?;
                    let FxRuntimeValue::I32(count) = count else {
                        return Err(EvaluationFailure::new(
                            BundleViewDiagnosticCode::InvalidValueProgram,
                            Some(cursor),
                            "repeat source must return I32",
                        ));
                    };
                    if !(0..=VIEW_REPEAT_LIMIT).contains(&count) {
                        return Err(EvaluationFailure::new(
                            BundleViewDiagnosticCode::RepeatLimitExceeded,
                            Some(cursor),
                            format!("repeat count {count} is outside 0..={VIEW_REPEAT_LIMIT}"),
                        ));
                    }
                    let repeat_slots =
                        self.repeat_slots(*key_program, definition.public_id.as_str())?;
                    let mut keys = BTreeSet::new();
                    for ordinal in 0..count {
                        for slot in &repeat_slots {
                            mounted
                                .state
                                .set_state(*slot, FxRuntimeValue::I32(ordinal), self.inventory)
                                .map_err(|error| EvaluationFailure::value(Some(cursor), &error))?;
                            mounted.initialized_state.insert(*slot);
                        }
                        let context = self.sample_context(mounted, ordinal.cast_unsigned())?;
                        let item_key = evaluate_value(
                            mounted,
                            *key_program,
                            self.inventory,
                            context,
                            &mut self.value_budget,
                            Some(cursor),
                        )?;
                        let FxRuntimeValue::I32(item_key) = item_key else {
                            return Err(EvaluationFailure::new(
                                BundleViewDiagnosticCode::InvalidValueProgram,
                                Some(cursor),
                                "repeat key must return I32",
                            ));
                        };
                        if !keys.insert(item_key) {
                            return Err(EvaluationFailure::new(
                                BundleViewDiagnosticCode::DuplicateRepeatKey,
                                Some(cursor),
                                format!("repeat key {item_key} occurs more than once"),
                            ));
                        }
                        let repeated_path = structural_path
                            .with_segment(BundleViewInstancePathSegment::Repeat {
                                instruction: instruction_ordinal(cursor)?,
                                key: item_key,
                            })
                            .map_err(|error| {
                                EvaluationFailure::new(
                                    BundleViewDiagnosticCode::RecursionLimitExceeded,
                                    Some(cursor),
                                    error.to_string(),
                                )
                            })?;
                        self.execute_span(
                            key,
                            definition,
                            mounted,
                            &repeated_path,
                            body_start,
                            body_end,
                            depth,
                            builder,
                            descendants,
                        )?;
                    }
                    cursor = body_end;
                }
                ViewProgramInstruction::Await {
                    source_program,
                    pending_branch,
                    ready_branch,
                    error_branch,
                    denied_branch,
                    ..
                } => {
                    let context = self.sample_context(mounted, instruction_ordinal(cursor)?)?;
                    let state = evaluate_value(
                        mounted,
                        *source_program,
                        self.inventory,
                        context,
                        &mut self.value_budget,
                        Some(cursor),
                    )?;
                    let FxRuntimeValue::I32(state) = state else {
                        return Err(EvaluationFailure::new(
                            BundleViewDiagnosticCode::InvalidAwaitState,
                            Some(cursor),
                            "await state program must return I32",
                        ));
                    };
                    let selected = match state {
                        0 => pending_branch,
                        1 => ready_branch,
                        2 => error_branch,
                        3 => denied_branch,
                        _ => {
                            return Err(EvaluationFailure::new(
                                BundleViewDiagnosticCode::InvalidAwaitState,
                                Some(cursor),
                                format!(
                                    "await state discriminant {state} is not pending=0, ready=1, error=2, or denied=3"
                                ),
                            ));
                        }
                    };
                    let await_end = await_extent(
                        cursor,
                        end,
                        [
                            pending_branch.as_ref(),
                            ready_branch.as_ref(),
                            error_branch.as_ref(),
                            denied_branch.as_ref(),
                        ],
                    )?;
                    if let Some(branch) = selected {
                        let branch_start = cursor
                            .checked_add(1)
                            .and_then(|start| start.checked_add(branch.start_offset as usize))
                            .ok_or_else(|| {
                                control_flow_failure(cursor, "await branch start overflow")
                            })?;
                        let branch_end =
                            checked_span_end(branch_start, branch.body_span, end, cursor)?;
                        self.execute_span(
                            key,
                            definition,
                            mounted,
                            structural_path,
                            branch_start,
                            branch_end,
                            depth,
                            builder,
                            descendants,
                        )?;
                    }
                    cursor = await_end;
                }
                ViewProgramInstruction::CallView {
                    view,
                    arguments,
                    styles,
                    part,
                    key: authored_key,
                    ..
                } => {
                    let root = builder.is_root_node();
                    let local_styles = builder
                        .style_scopes
                        .retain_node(
                            ViewStyleNodeInput {
                                parts: self.parts,
                                owner: &mounted.owner,
                                path: structural_path,
                                instruction: instruction_ordinal(cursor)?,
                                kind: BundleViewStyleNodeKind::CallView {
                                    view: view.view_id().clone(),
                                },
                                part: part.as_ref(),
                                local: styles,
                                root,
                            },
                            &mut self.style_scope_allocator,
                        )
                        .map_err(|error| EvaluationFailure::style_scope(Some(cursor), error))?;
                    let child_style_scopes = builder
                        .style_scopes
                        .for_nested_view(&local_styles)
                        .map_err(|error| EvaluationFailure::style_scope(Some(cursor), error))?;
                    let child_view = view.view_id().clone();
                    let Some(child_index) = self.catalog.definition_index(&child_view) else {
                        return Err(EvaluationFailure::new(
                            BundleViewDiagnosticCode::MissingDefinition,
                            Some(cursor),
                            format!("nested View definition `{view}` does not exist"),
                        ));
                    };
                    let mut evaluated_arguments = BTreeMap::new();
                    for argument in arguments {
                        let context = self.sample_context(mounted, instruction_ordinal(cursor)?)?;
                        let value = evaluate_value(
                            mounted,
                            argument.value_program,
                            self.inventory,
                            context,
                            &mut self.value_budget,
                            Some(cursor),
                        )?;
                        if evaluated_arguments
                            .insert(argument.ordinal, value)
                            .is_some()
                        {
                            return Err(EvaluationFailure::new(
                                BundleViewDiagnosticCode::InvalidControlFlow,
                                Some(cursor),
                                format!(
                                    "nested View `{view}` repeats argument ordinal {}",
                                    argument.ordinal
                                ),
                            ));
                        }
                    }
                    let child_path = structural_path
                        .with_segment(BundleViewInstancePathSegment::Call {
                            instruction: instruction_ordinal(cursor)?,
                            authored_key: *authored_key,
                        })
                        .map_err(|error| {
                            EvaluationFailure::new(
                                BundleViewDiagnosticCode::RecursionLimitExceeded,
                                Some(cursor),
                                error.to_string(),
                            )
                        })?;
                    let child_key = ViewOccurrenceKey {
                        handle: key.handle.clone(),
                        path: child_path,
                    };
                    match self.prepare_occurrence(
                        &child_key,
                        child_index,
                        Some(&evaluated_arguments),
                    ) {
                        Ok(()) => {
                            self.visited.insert(child_key.clone());
                            let child_output = self.evaluate_occurrence(
                                child_key,
                                child_index,
                                depth + 1,
                                child_style_scopes,
                            );
                            if let Some(child) = child_output.first() {
                                builder
                                    .paint
                                    .push(BundleViewPaintItem::Mount { mount: child.mount });
                            }
                            descendants.extend(child_output);
                        }
                        Err(error) => {
                            self.record_failure(&child_key, &child_view, None, error);
                        }
                    }
                    cursor += 1;
                }
                ViewProgramInstruction::BindLocal {
                    binding,
                    value_program,
                    ..
                } => {
                    let context = self.sample_context(mounted, instruction_ordinal(cursor)?)?;
                    let value = evaluate_value(
                        mounted,
                        *value_program,
                        self.inventory,
                        context,
                        &mut self.value_budget,
                        Some(cursor),
                    )?;
                    let slots = self.local_slots(definition.public_id.as_str(), binding);
                    if slots.is_empty() {
                        return Err(EvaluationFailure::new(
                            BundleViewDiagnosticCode::InvalidControlFlow,
                            Some(cursor),
                            format!("local `{binding}` has no typed input slot"),
                        ));
                    }
                    for slot in slots {
                        mounted
                            .state
                            .set_state(slot, value, self.inventory)
                            .map_err(|error| EvaluationFailure::value(Some(cursor), &error))?;
                        mounted.initialized_state.insert(slot);
                    }
                    cursor += 1;
                }
                ViewProgramInstruction::ApplyFx {
                    fx,
                    arguments,
                    key_program,
                    application_ordinal,
                    ..
                } => {
                    let mut evaluated_arguments = Vec::with_capacity(arguments.len());
                    for argument in arguments {
                        let context = self.sample_context(mounted, instruction_ordinal(cursor)?)?;
                        evaluated_arguments.push(BundleViewFxArgument {
                            parameter: argument.parameter.clone(),
                            value: evaluate_value(
                                mounted,
                                argument.value_program,
                                self.inventory,
                                context,
                                &mut self.value_budget,
                                Some(cursor),
                            )?,
                        });
                    }
                    let reactive_key = match key_program {
                        Some(program) => {
                            let context =
                                self.sample_context(mounted, instruction_ordinal(cursor)?)?;
                            match evaluate_value(
                                mounted,
                                *program,
                                self.inventory,
                                context,
                                &mut self.value_budget,
                                Some(cursor),
                            )? {
                                FxRuntimeValue::I32(value) => Some(value),
                                _ => {
                                    return Err(EvaluationFailure::new(
                                        BundleViewDiagnosticCode::InvalidFxApplication,
                                        Some(cursor),
                                        "Fx application key must return I32",
                                    ));
                                }
                            }
                        }
                        None => None,
                    };
                    let target = builder.fx_target(definition.public_id.as_str());
                    let instance = derive_fx_instance(
                        fx,
                        key,
                        structural_path,
                        &target,
                        *application_ordinal,
                        reactive_key,
                    );
                    let child_path = FxGraphChildPath::try_new(vec![*application_ordinal])
                        .map_err(|error| {
                            EvaluationFailure::new(
                                BundleViewDiagnosticCode::InvalidFxApplication,
                                Some(cursor),
                                error.to_string(),
                            )
                        })?;
                    builder.fx.push(BundleViewFxApplication {
                        instance,
                        definition: fx.clone(),
                        target,
                        application_ordinal: *application_ordinal,
                        arguments: evaluated_arguments,
                        child_path,
                    });
                    cursor += 1;
                }
                ViewProgramInstruction::OpenElement {
                    element,
                    target,
                    styles,
                    part,
                    ..
                } => {
                    let root = builder.is_root_node();
                    let mut local_styles = builder
                        .style_scopes
                        .retain_node(
                            ViewStyleNodeInput {
                                parts: self.parts,
                                owner: &mounted.owner,
                                path: structural_path,
                                instruction: instruction_ordinal(cursor)?,
                                kind: BundleViewStyleNodeKind::Element {
                                    element: *element,
                                    target: target.clone(),
                                },
                                part: part.as_ref(),
                                local: styles,
                                root,
                            },
                            &mut self.style_scope_allocator,
                        )
                        .map_err(|error| EvaluationFailure::style_scope(Some(cursor), error))?;
                    builder.style_scopes.enter_element(&mut local_styles);
                    if let Some(target) = target {
                        builder.retain_target(target);
                        builder.paint.push(BundleViewPaintItem::Element {
                            target: target.clone(),
                        });
                    }
                    builder.element_targets.push(target.clone());
                    cursor += 1;
                }
                ViewProgramInstruction::CloseElement => {
                    if builder.element_targets.pop().is_none() {
                        return Err(EvaluationFailure::new(
                            BundleViewDiagnosticCode::InvalidControlFlow,
                            Some(cursor),
                            "CloseElement has no matching OpenElement",
                        ));
                    }
                    builder
                        .style_scopes
                        .leave_element()
                        .map_err(|error| EvaluationFailure::style_scope(Some(cursor), error))?;
                    cursor += 1;
                }
                ViewProgramInstruction::EmitText {
                    text_source,
                    styles,
                    part,
                    ..
                } => {
                    let root = builder.is_root_node();
                    let _local_styles = builder
                        .style_scopes
                        .retain_node(
                            ViewStyleNodeInput {
                                parts: self.parts,
                                owner: &mounted.owner,
                                path: structural_path,
                                instruction: instruction_ordinal(cursor)?,
                                kind: BundleViewStyleNodeKind::Text {
                                    text_source: text_source.clone(),
                                },
                                part: part.as_ref(),
                                local: styles,
                                root,
                            },
                            &mut self.style_scope_allocator,
                        )
                        .map_err(|error| EvaluationFailure::style_scope(Some(cursor), error))?;
                    let text =
                        self.resolve_text(&key.handle, definition, mounted, text_source, cursor)?;
                    for target in &text.targets {
                        builder.retain_target(&target.public_id);
                        builder.paint.push(BundleViewPaintItem::Text {
                            source_id: text.source_id.clone(),
                            target: target.public_id.clone(),
                        });
                    }
                    builder.text.push(text);
                    builder.last_target = Some(text_source.clone());
                    cursor += 1;
                }
                ViewProgramInstruction::EmitImage {
                    image,
                    target,
                    styles,
                    part,
                    ..
                } => {
                    let root = builder.is_root_node();
                    let _local_styles = builder
                        .style_scopes
                        .retain_node(
                            ViewStyleNodeInput {
                                parts: self.parts,
                                owner: &mounted.owner,
                                path: structural_path,
                                instruction: instruction_ordinal(cursor)?,
                                kind: BundleViewStyleNodeKind::Image {
                                    image: image.clone(),
                                    target: target.clone(),
                                },
                                part: part.as_ref(),
                                local: styles,
                                root,
                            },
                            &mut self.style_scope_allocator,
                        )
                        .map_err(|error| EvaluationFailure::style_scope(Some(cursor), error))?;
                    if let Some(target) = target {
                        builder.images.insert(target.clone());
                        builder.retain_target(target);
                        builder.paint.push(BundleViewPaintItem::Image {
                            target: target.clone(),
                        });
                    }
                    cursor += 1;
                }
                ViewProgramInstruction::AttachSemantic {
                    target,
                    label_text_source,
                    ..
                } => {
                    builder.retain_target(target);
                    if let Some(source) = label_text_source
                        && !builder.text.iter().any(|text| text.source_id == *source)
                    {
                        let text =
                            self.resolve_text(&key.handle, definition, mounted, source, cursor)?;
                        for target in &text.targets {
                            builder.retain_target(&target.public_id);
                        }
                        builder.text.push(text);
                    }
                    cursor += 1;
                }
                ViewProgramInstruction::EmitCustom {
                    element,
                    styles,
                    part,
                    ..
                } => {
                    let root = builder.is_root_node();
                    let _local_styles = builder
                        .style_scopes
                        .retain_node(
                            ViewStyleNodeInput {
                                parts: self.parts,
                                owner: &mounted.owner,
                                path: structural_path,
                                instruction: instruction_ordinal(cursor)?,
                                kind: BundleViewStyleNodeKind::Custom {
                                    element: element.clone(),
                                },
                                part: part.as_ref(),
                                local: styles,
                                root,
                            },
                            &mut self.style_scope_allocator,
                        )
                        .map_err(|error| EvaluationFailure::style_scope(Some(cursor), error))?;
                    builder.last_target = Some(element.clone());
                    cursor += 1;
                }
                ViewProgramInstruction::BindHandler { .. } => {
                    cursor += 1;
                }
            }
        }
        Ok(())
    }

    fn repeat_slots(
        &self,
        key_program: ViewValueProgramId,
        definition: &str,
    ) -> Result<Vec<u16>, EvaluationFailure> {
        let program = self.inventory.get(key_program).ok_or_else(|| {
            EvaluationFailure::new(
                BundleViewDiagnosticCode::InvalidValueProgram,
                None,
                format!("repeat references unknown value program {key_program:?}"),
            )
        })?;
        Ok(program
            .state_dependencies()
            .iter()
            .copied()
            .filter(|slot| {
                self.program.value_inputs.iter().any(|input| {
                    input.namespace == ViewValueInputNamespace::State
                        && input.slot == *slot
                        && matches!(
                            &input.source,
                            ViewValueInputSource::RepeatOrdinal { view, .. }
                                if view == definition
                        )
                })
            })
            .collect())
    }

    fn local_slots(&self, definition: &str, name: &str) -> Vec<u16> {
        self.program
            .value_inputs
            .iter()
            .filter_map(|input| match &input.source {
                ViewValueInputSource::Local { view, name: local }
                    if input.namespace == ViewValueInputNamespace::State
                        && view == definition
                        && local == name =>
                {
                    Some(input.slot)
                }
                ViewValueInputSource::DefinitionParameter { .. }
                | ViewValueInputSource::Projection { .. }
                | ViewValueInputSource::LifetimeProjection { .. }
                | ViewValueInputSource::Local { .. }
                | ViewValueInputSource::RepeatOrdinal { .. } => None,
            })
            .collect()
    }

    fn sample_context(
        &self,
        mounted: &MountedView,
        ordinal: u32,
    ) -> Result<FxSampleContext, EvaluationFailure> {
        FxSampleContext::from_logical_times(
            self.logical_time,
            mounted.activation_logical_time,
            ordinal,
            mounted.deterministic_seed,
            self.reduce_motion,
        )
        .map_err(|error| {
            EvaluationFailure::new(
                BundleViewDiagnosticCode::InvalidValueProgram,
                None,
                error.to_string(),
            )
        })
    }

    fn charge_instruction(&mut self, instruction: usize) -> Result<(), EvaluationFailure> {
        if self.instruction_budget == 0 {
            Err(EvaluationFailure::new(
                BundleViewDiagnosticCode::EvaluationBudgetExceeded,
                Some(instruction),
                format!("View frame exceeded its {VIEW_FRAME_OPERATION_BUDGET} instruction budget"),
            ))
        } else {
            self.instruction_budget -= 1;
            Ok(())
        }
    }

    fn definition(&self, index: ViewDefinitionIndex) -> &ViewDefinitionResource {
        self.catalog.execution_definition(index)
    }

    fn record_failure(
        &mut self,
        key: &ViewOccurrenceKey,
        view: &ViewId,
        mount: Option<arcweft_view::ViewMountId>,
        failure: EvaluationFailure,
    ) {
        self.diagnostics.push(BundleViewDiagnostic {
            code: failure.code,
            handle: Some(key.handle.clone()),
            mount,
            view: Some(view.as_str().to_owned()),
            instruction: failure.instruction,
            message: failure.message,
        });
    }
}

fn evaluate_value(
    mounted: &mut MountedView,
    program_id: ViewValueProgramId,
    inventory: &ViewValueProgramInventory,
    context: FxSampleContext,
    budget: &mut FxEvaluationBudget,
    instruction: Option<usize>,
) -> Result<FxRuntimeValue, EvaluationFailure> {
    let program = inventory.get(program_id).ok_or_else(|| {
        EvaluationFailure::new(
            BundleViewDiagnosticCode::InvalidValueProgram,
            instruction,
            format!("unknown View value program {program_id:?}"),
        )
    })?;
    if let Some(slot) = program
        .parameter_dependencies()
        .iter()
        .find(|slot| !mounted.initialized_parameters.contains(slot))
    {
        return Err(EvaluationFailure::new(
            BundleViewDiagnosticCode::MissingInput,
            instruction,
            format!("parameter slot {slot} is not initialized"),
        ));
    }
    if let Some(slot) = program
        .state_dependencies()
        .iter()
        .find(|slot| !mounted.initialized_state.contains(slot))
    {
        return Err(EvaluationFailure::new(
            BundleViewDiagnosticCode::MissingInput,
            instruction,
            format!("state slot {slot} is not initialized"),
        ));
    }
    mounted
        .state
        .evaluate(program_id, inventory, context, budget)
        .map(arcweft_view::ViewValueEvaluation::value)
        .map_err(|error| EvaluationFailure::value(instruction, &error))
}
