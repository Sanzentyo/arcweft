//! External Action ingress uses the same original-program mailbox as VM emit.

use super::{AwbcProductStepExecutor, ProductCheckpointTarget};
use crate::executor::RuntimeActionEmissionError;
use crate::pattern::RuntimeSemanticTypeId;
use crate::step::RuntimeDiagnostic;
use crate::value::{RuntimePayload, RuntimeValue};

impl AwbcProductStepExecutor {
    pub(crate) fn emit_external_action(
        &mut self,
        payload_type: RuntimeSemanticTypeId,
        payload: RuntimeValue,
    ) -> Result<(), RuntimeActionEmissionError> {
        let mut admission = self
            .retained_execution_admission(ProductCheckpointTarget::Main)
            .map_err(|error| RuntimeActionEmissionError::Execution {
                diagnostic: RuntimeDiagnostic::categorized(error.category(), error.to_string()),
            })?;
        let prepared = self
            .actions
            .prepare_emit_in_admitted_group(payload_type, (&payload).into(), &mut admission)
            .map_err(Box::new)?;
        prepared.commit(RuntimePayload::new(payload));
        Ok(())
    }
}
