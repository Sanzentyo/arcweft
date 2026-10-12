//! Program-bound inert wire projection of retained View values.

use super::{
    BundleViewMountRuntimeSnapshot, BundleViewRuntimeSnapshot, BundleViewStateFieldSnapshot,
};
use arcweft_core::{
    task::RuntimeProgramOwner,
    value::{AwbcRuntimeValueSnapshot, RuntimeBinding, RuntimeValue},
};
use serde::{Deserialize, Serialize};

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

fn snapshot_value(
    value: &RuntimeValue,
    owner: &RuntimeProgramOwner,
) -> Result<AwbcRuntimeValueSnapshot, String> {
    if !value.ownership().permits_copy() {
        return Err("retained View snapshot requires unrestricted values".to_owned());
    }
    AwbcRuntimeValueSnapshot::from_runtime_value_for_program(value, owner)
        .map_err(|error| error.to_string())
}

fn restore_value(
    value: AwbcRuntimeValueSnapshot,
    owner: &RuntimeProgramOwner,
) -> Result<RuntimeValue, String> {
    let value = value
        .into_runtime_value_for_program(owner)
        .map_err(|error| error.to_string())?;
    if !value.ownership().permits_copy() {
        return Err("retained View restore requires unrestricted values".to_owned());
    }
    Ok(value)
}

impl BindingSnapshot {
    fn from_live(value: &RuntimeBinding, owner: &RuntimeProgramOwner) -> Result<Self, String> {
        Ok(Self {
            name: value.name.clone(),
            value: snapshot_value(&value.value, owner)?,
        })
    }
    fn into_live(self, owner: &RuntimeProgramOwner) -> Result<RuntimeBinding, String> {
        Ok(RuntimeBinding {
            name: self.name,
            value: restore_value(self.value, owner)?,
        })
    }
}

impl BundleViewRuntimeSaveSnapshot {
    pub(crate) fn from_live(
        value: &BundleViewRuntimeSnapshot,
        owner: &RuntimeProgramOwner,
    ) -> Result<Self, String> {
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
                                    value: snapshot_value(&field.value, owner)?,
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
                                    value: restore_value(field.value, owner)?,
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
