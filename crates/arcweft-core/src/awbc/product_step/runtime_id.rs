use super::ProductStepError;
use crate::plan::RuntimeLineId;

pub(super) fn line_id_from_awbc_public_id(
    public_id: &str,
) -> Result<RuntimeLineId, ProductStepError> {
    RuntimeLineId::from_runtime_line_value(public_id)
        .map_err(|_| invalid_awbc_content_line_id(public_id))
}

fn invalid_awbc_content_line_id(public_id: &str) -> ProductStepError {
    ProductStepError::Internal(format!("invalid AWBC content line id `{public_id}`"))
}
