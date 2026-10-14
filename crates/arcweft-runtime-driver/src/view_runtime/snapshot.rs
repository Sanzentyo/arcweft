//! Program-bound inert wire projection of retained View values.

use super::{
    BundleViewMountRuntimeSnapshot, BundleViewRuntimeSnapshot, BundleViewStateFieldSnapshot,
};
use arcweft_core::{
    task::RuntimeProgramOwner,
    value::{
        AwbcRuntimeValueSnapshot, AwbcRuntimeValueSnapshotError, RuntimeBinding,
        RuntimeRetainedValueAdmission,
    },
};
use serde::{Deserialize, Serialize};

#[cfg(test)]
mod tests;

// Both representations retain the same View-owned metadata. This one borrowed
// traversal precedes live projection and inert activation; only the Core value
// admission method differs. No View rows or schema are reconstructed here.
macro_rules! admit_view_snapshot_resources {
    ($snapshot:ty, $value:ident, $typed_value:ident) => {
        impl $snapshot {
            pub(crate) fn visit_borrowed_runtime_values(
                &self,
                mut visit: impl for<'value> FnMut(
                    arcweft_core::value::RuntimeBorrowedValue<'value>,
                ) -> Result<(), String>,
            ) -> Result<(), String> {
                let limits = arcweft_core::entry::RuntimeSchemaLimits::engine_default();
                for value in self
                    .view_root_bindings
                    .iter()
                    .map(|binding| &binding.value)
                    .chain(self.mounts.iter().flat_map(|mount| {
                        mount
                            .runtime_parameters
                            .iter()
                            .map(|binding| &binding.value)
                            .chain(mount.local_state.iter().map(|field| &field.value))
                    }))
                {
                    arcweft_core::value::RuntimeBorrowedValue::from(value)
                        .try_visit(limits, &mut visit)
                        .map_err(|error| error.to_string())?;
                }
                Ok(())
            }

            pub(crate) fn admit_resources(
                &self,
                admission: &mut RuntimeRetainedValueAdmission,
            ) -> Result<(), AwbcRuntimeValueSnapshotError> {
                if let Some(program) = &self.program_id {
                    admission.validate_metadata_string(program.as_str())?;
                }
                admission.validate_metadata_collection(self.view_root_bindings.len())?;
                admission.validate_metadata_collection(self.mounts.len())?;
                for binding in &self.view_root_bindings {
                    admission.validate_metadata_string(&binding.name)?;
                }
                for mount in &self.mounts {
                    admission.validate_metadata_string(mount.handle.as_str())?;
                    admission.validate_metadata_collection(mount.path.segments().len())?;
                    match &mount.owner {
                        super::SavedViewOwner::Rust { view, .. } => {
                            admission.validate_metadata_string(view.as_str())?;
                        }
                        super::SavedViewOwner::Arcweft { view, program, .. } => {
                            admission.validate_metadata_string(view.as_str())?;
                            admission.validate_metadata_string(program.as_str())?;
                        }
                    }
                    admission.validate_metadata_string(mount.state.program.as_str())?;
                    for count in [
                        mount.state.parameters.len(),
                        mount.state.state.len(),
                        mount.initialized_parameters.len(),
                        mount.initialized_state.len(),
                        mount.runtime_parameters.len(),
                        mount.local_state.len(),
                    ] {
                        admission.validate_metadata_collection(count)?;
                    }
                    for binding in &mount.runtime_parameters {
                        admission.validate_metadata_string(&binding.name)?;
                    }
                    for field in &mount.local_state {
                        admission.validate_metadata_collection(field.path.segments().len())?;
                    }
                }
                admission.validate_metadata_collection(self.axis_seeds.pending.len())?;
                admission.validate_metadata_collection(self.axis_seeds.mounted.len())?;
                for handle in self
                    .axis_seeds
                    .pending
                    .iter()
                    .map(|seed| &seed.handle)
                    .chain(self.axis_seeds.mounted.iter().map(|seed| &seed.handle))
                {
                    admission.validate_metadata_string(handle.as_str())?;
                }
                // Complete metadata preflight precedes payload/type admission,
                // so a late table/name limit cannot be masked by an earlier
                // malformed payload or trigger a prefix copy/activation.
                for binding in &self.view_root_bindings {
                    admission.$value(&binding.value)?;
                }
                for mount in &self.mounts {
                    for binding in &mount.runtime_parameters {
                        admission.$value(&binding.value)?;
                    }
                    for field in &mount.local_state {
                        admission.$typed_value(&field.value, field.value_type)?;
                    }
                }
                Ok(())
            }
        }
    };
}

admit_view_snapshot_resources!(BundleViewRuntimeSnapshot, validate, validate_typed);
admit_view_snapshot_resources!(
    BundleViewRuntimeSaveSnapshot,
    validate_snapshot,
    validate_typed_snapshot
);

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BundleViewRuntimeSaveSnapshot {
    program_id: Option<arcweft_view::ViewProgramId>,
    logical_time: arcweft_presentation::fx::FxLogicalTime,
    next_mount_id: u64,
    view_root_bindings: Vec<BindingSnapshot>,
    mounts: Vec<MountSnapshot>,
    axis_seeds: super::BundleViewAxisSeedRegistrySnapshot,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct BindingSnapshot {
    name: String,
    value: AwbcRuntimeValueSnapshot,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct MountSnapshot {
    handle: crate::presentation_handles::PresentationHandleId,
    path: super::BundleViewInstancePath,
    owner: super::SavedViewOwner,
    activation_logical_time: arcweft_presentation::fx::FxLogicalTime,
    deterministic_seed: u64,
    state: arcweft_view::ViewMountSnapshot,
    initialized_parameters: Vec<u16>,
    initialized_state: Vec<u16>,
    runtime_parameters: Vec<BindingSnapshot>,
    local_state: Vec<StateFieldSnapshot>,
    next_handler_seal_revision: u64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct StateFieldSnapshot {
    path: super::BundleViewInstancePath,
    field: arcweft_view::ViewStateFieldId,
    value_type: arcweft_id::RuntimeSemanticTypeId,
    value: AwbcRuntimeValueSnapshot,
}

impl BindingSnapshot {
    fn from_live(value: &RuntimeBinding, owner: &RuntimeProgramOwner) -> Result<Self, String> {
        Ok(Self {
            name: value.name.clone(),
            value: AwbcRuntimeValueSnapshot::from_unrestricted_runtime_value_for_program(
                &value.value,
                owner,
            )
            .map_err(|error| error.to_string())?,
        })
    }
    fn into_live(self, owner: &RuntimeProgramOwner) -> Result<RuntimeBinding, String> {
        Ok(RuntimeBinding {
            name: self.name,
            value: self
                .value
                .into_unrestricted_runtime_value_for_program(owner)
                .map_err(|error| error.to_string())?,
        })
    }
}

impl BundleViewRuntimeSaveSnapshot {
    pub(crate) fn from_live(
        value: &BundleViewRuntimeSnapshot,
        owner: &RuntimeProgramOwner,
    ) -> Result<Self, String> {
        let mut admission = RuntimeRetainedValueAdmission::for_program(
            owner.clone(),
            arcweft_core::entry::RuntimeSchemaLimits::engine_default(),
        );
        value
            .admit_resources(&mut admission)
            .map_err(|error| error.to_string())?;
        Ok(Self {
            program_id: value.program_id.clone(),
            logical_time: value.logical_time,
            next_mount_id: value.next_mount_id,
            view_root_bindings: value
                .view_root_bindings
                .iter()
                .map(|v| BindingSnapshot::from_live(v, owner))
                .collect::<Result<_, _>>()?,
            mounts: value
                .mounts
                .iter()
                .map(|mount| {
                    Ok(MountSnapshot {
                        handle: mount.handle.clone(),
                        path: mount.path.clone(),
                        owner: mount.owner.clone(),
                        activation_logical_time: mount.activation_logical_time,
                        deterministic_seed: mount.deterministic_seed,
                        state: mount.state.clone(),
                        initialized_parameters: mount.initialized_parameters.clone(),
                        initialized_state: mount.initialized_state.clone(),
                        runtime_parameters: mount
                            .runtime_parameters
                            .iter()
                            .map(|v| BindingSnapshot::from_live(v, owner))
                            .collect::<Result<_, _>>()?,
                        local_state: mount
                            .local_state
                            .iter()
                            .map(|field| {
                                Ok(StateFieldSnapshot {
                                    path: field.path.clone(),
                                    field: field.field,
                                    value_type: field.value_type,
                                    value: AwbcRuntimeValueSnapshot::from_unrestricted_runtime_value_for_program(
                                        &field.value, owner,
                                    ).map_err(|error| error.to_string())?,
                                })
                            })
                            .collect::<Result<_, String>>()?,
                        next_handler_seal_revision: mount.next_handler_seal_revision,
                    })
                })
                .collect::<Result<_, String>>()?,
            axis_seeds: value.axis_seeds.clone(),
        })
    }

    pub(crate) fn into_live(
        self,
        owner: &RuntimeProgramOwner,
    ) -> Result<BundleViewRuntimeSnapshot, String> {
        let mut admission = RuntimeRetainedValueAdmission::for_program(
            owner.clone(),
            arcweft_core::entry::RuntimeSchemaLimits::engine_default(),
        );
        self.admit_resources(&mut admission)
            .map_err(|error| error.to_string())?;
        Ok(BundleViewRuntimeSnapshot {
            program_id: self.program_id,
            logical_time: self.logical_time,
            next_mount_id: self.next_mount_id,
            view_root_bindings: self
                .view_root_bindings
                .into_iter()
                .map(|v| v.into_live(owner))
                .collect::<Result<_, _>>()?,
            mounts: self
                .mounts
                .into_iter()
                .map(|mount| {
                    Ok(BundleViewMountRuntimeSnapshot {
                        handle: mount.handle,
                        path: mount.path,
                        owner: mount.owner,
                        activation_logical_time: mount.activation_logical_time,
                        deterministic_seed: mount.deterministic_seed,
                        state: mount.state,
                        initialized_parameters: mount.initialized_parameters,
                        initialized_state: mount.initialized_state,
                        runtime_parameters: mount
                            .runtime_parameters
                            .into_iter()
                            .map(|v| v.into_live(owner))
                            .collect::<Result<_, _>>()?,
                        local_state: mount
                            .local_state
                            .into_iter()
                            .map(|field| {
                                Ok(BundleViewStateFieldSnapshot {
                                    path: field.path,
                                    field: field.field,
                                    value_type: field.value_type,
                                    value: field
                                        .value
                                        .into_unrestricted_runtime_value_for_program(owner)
                                        .map_err(|error| error.to_string())?,
                                })
                            })
                            .collect::<Result<_, String>>()?,
                        next_handler_seal_revision: mount.next_handler_seal_revision,
                    })
                })
                .collect::<Result<_, String>>()?,
            axis_seeds: self.axis_seeds,
        })
    }
}
