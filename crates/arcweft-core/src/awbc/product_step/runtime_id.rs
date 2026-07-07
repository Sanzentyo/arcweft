use super::ProductStepError;
use crate::plan::{FlowRuntimeId, RuntimeLineId};

pub(super) fn line_id_from_awbc_public_id(
    public_id: &str,
) -> Result<RuntimeLineId, ProductStepError> {
    RuntimeLineId::from_runtime_line_value(public_id)
        .map_err(|_| invalid_awbc_content_line_id(public_id))
}

pub(super) fn flow_id_from_awbc_public_id(
    public_id: &str,
) -> Result<FlowRuntimeId, ProductStepError> {
    FlowRuntimeId::from_runtime_target_value(public_id)
        .map_err(|error| invalid_awbc_goto_target(public_id, &error))
}

fn invalid_awbc_content_line_id(public_id: &str) -> ProductStepError {
    ProductStepError::Internal(format!("invalid AWBC content line id `{public_id}`"))
}

fn invalid_awbc_goto_target(public_id: &str, error: &impl std::fmt::Display) -> ProductStepError {
    ProductStepError::Internal(format!("invalid AWBC goto target `{public_id}`: {error}"))
}
