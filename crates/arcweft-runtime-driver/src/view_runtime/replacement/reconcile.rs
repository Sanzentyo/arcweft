//! Scratch reconciliation of retained View occurrences.

use std::collections::{BTreeMap, BTreeSet};

use arcweft_bundle::resource_codec::view::{
    DialogueViewContractError, ViewInstructionSpan, ViewProgramInstruction,
};
use arcweft_presentation::fx::FxRuntimeValue;
use arcweft_view::{
    ViewId, ViewMountId, ViewMountState, ViewProgramId, ViewRegistry, ViewValueEvaluationError,
    ViewValueProgramInventory,
};
use thiserror::Error;

use super::super::catalog::ViewProgramCatalog;
use super::super::owner::{AcceptedViewProgramGeneration, ResolvedMountedViewOwner};
use super::super::value::fx_placeholder;
use super::super::{
    BundleViewInstancePath, BundleViewInstancePathSegment, MountedView, ViewOccurrenceKey,
};

/// Failure while preserving or resetting retained mount state.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ViewMountReconcileError {
    #[error("replacement View value-program inventory is invalid")]
    Inventory(#[from] arcweft_view::ViewValueInventoryError),
    #[error("replacement View mount state could not be constructed")]
    State(#[from] ViewValueEvaluationError),
    #[error("replacement View registry has no accepted owner {0}")]
    MissingRegistryOwner(ViewId),
    #[error(transparent)]
    DialogueContract(#[from] DialogueViewContractError),
}

pub(super) fn reconcile_mounts(
    mounts: &BTreeMap<ViewOccurrenceKey, MountedView>,
    candidate: &ViewProgramCatalog,
    registry: &ViewRegistry,
    generation: AcceptedViewProgramGeneration,
    inventory: &ViewValueProgramInventory,
) -> Result<
    (
        BTreeMap<ViewOccurrenceKey, MountedView>,
        BTreeSet<ViewMountId>,
    ),
    ViewMountReconcileError,
> {
    let mut reconciled = BTreeMap::new();
    let mut retired = BTreeSet::new();
    for (key, mounted) in mounts {
        let expected_view = resolve_candidate_path(mounts, key, candidate);
        if expected_view.as_ref() != Some(mounted.view()) {
            retired.insert(mounted.state.mount());
            continue;
        }
        let view = mounted.view().clone();
        let Some(definition_index) = candidate.definition_index(&view) else {
            retired.insert(mounted.state.mount());
            continue;
        };
        let definition = candidate.execution_definition(definition_index);
        let registry_id = registry
            .resolve(&view)
            .ok_or_else(|| ViewMountReconcileError::MissingRegistryOwner(view.clone()))?;
        let snapshot = mounted.state.snapshot();
        let (state, preserved) = match ViewMountState::from_snapshot(
            &snapshot,
            candidate.program_id(),
            definition.state_schema_hash,
            inventory,
        ) {
            Ok(state) => (state, true),
            Err(
                ViewValueEvaluationError::StateSchemaMismatch { .. }
                | ViewValueEvaluationError::InputCount { .. }
                | ViewValueEvaluationError::InputType { .. },
            ) => (
                fresh_mount_state(
                    mounted.state.mount(),
                    candidate.program_id(),
                    definition.state_schema_hash,
                    inventory,
                )?,
                false,
            ),
            Err(error) => return Err(error.into()),
        };
        let mut next = mounted.clone();
        next.owner = ResolvedMountedViewOwner::Arcweft {
            view,
            registry: registry_id,
            definition: definition_index,
            program: candidate.program_id().clone(),
            revision: candidate.revision(),
            generation,
        };
        next.state = state;
        next.handler_seals.clear();
        if !preserved {
            next.initialized_parameters.clear();
            next.initialized_state.clear();
            next.runtime_parameters.clear();
        }
        reconciled.insert(key.clone(), next);
    }
    Ok((reconciled, retired))
}

fn fresh_mount_state(
    mount: ViewMountId,
    program: &ViewProgramId,
    state_schema_hash: u64,
    inventory: &ViewValueProgramInventory,
) -> Result<ViewMountState, ViewValueEvaluationError> {
    let parameters = inventory
        .parameter_types()
        .iter()
        .copied()
        .map(fx_placeholder)
        .collect::<Vec<FxRuntimeValue>>();
    let state = inventory
        .state_types()
        .iter()
        .copied()
        .map(fx_placeholder)
        .collect::<Vec<FxRuntimeValue>>();
    ViewMountState::new(
        mount,
        program.clone(),
        state_schema_hash,
        parameters,
        state,
        inventory,
    )
}

fn resolve_candidate_path(
    mounts: &BTreeMap<ViewOccurrenceKey, MountedView>,
    key: &ViewOccurrenceKey,
    candidate: &ViewProgramCatalog,
) -> Option<ViewId> {
    let root_key = ViewOccurrenceKey {
        handle: key.handle.clone(),
        path: BundleViewInstancePath::default(),
    };
    let mut view = mounts.get(&root_key)?.view().clone();
    let mut definition = candidate
        .definition_index(&view)
        .map(|index| candidate.execution_definition(index))?;
    let mut allowed = definition.body;
    for segment in key.path.segments() {
        let instruction_index = match segment {
            BundleViewInstancePathSegment::Call { instruction, .. }
            | BundleViewInstancePathSegment::Repeat { instruction, .. } => *instruction,
        };
        if instruction_index < allowed.start_instruction
            || instruction_index >= allowed.end_instruction
        {
            return None;
        }
        let instruction = candidate
            .resource()
            .instructions
            .get(usize::try_from(instruction_index).ok()?)?;
        match (segment, instruction) {
            (
                BundleViewInstancePathSegment::Repeat { .. },
                ViewProgramInstruction::RepeatKeyed { body_span, .. },
            ) => {
                let start = instruction_index.checked_add(1)?;
                allowed = ViewInstructionSpan::new(start, start.checked_add(*body_span)?);
            }
            (
                BundleViewInstancePathSegment::Call { authored_key, .. },
                ViewProgramInstruction::CallView {
                    view: target, key, ..
                },
            ) if authored_key == key => {
                view = target.view_id().clone();
                definition = candidate
                    .definition_index(&view)
                    .map(|index| candidate.execution_definition(index))?;
                allowed = definition.body;
            }
            _ => return None,
        }
    }
    Some(view)
}
