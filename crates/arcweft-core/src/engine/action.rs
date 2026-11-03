//! Action delivery is authorized by the current original code operation.

use super::Engine;
use super::audio::RuntimeEffectExecutionError;
use super::code_cursor::NativeSourceOperation;
use crate::effect::{RuntimeEffectExpr, RuntimeEffectMaterializeError};
use crate::plan::FlowOp;
use crate::pure::RuntimeCallBackend;
use crate::task::RuntimeProgramOwner;
use crate::value::{RuntimePayload, RuntimeRetainedValueAdmission};

impl Engine {
    pub(super) fn execute_action_emit(
        &mut self,
        source: &NativeSourceOperation,
        backend: &mut impl RuntimeCallBackend,
    ) -> Result<(), RuntimeEffectExecutionError> {
        let owner = RuntimeProgramOwner::Plan(self.plan.clone());
        if !self.fiber.frame.program.same_program(&owner) {
            return Err(RuntimeEffectMaterializeError::ActionContextRequired.into());
        }
        let plan = self.plan.clone();
        let Some(FlowOp::EvaluatedEffect(RuntimeEffectExpr::EmitAction { payload })) =
            source.operation(&plan)
        else {
            return Err(RuntimeEffectMaterializeError::ActionContextRequired.into());
        };
        let payload_type = plan
            .type_table()
            .get(payload.ty())
            .ok_or(RuntimeEffectMaterializeError::ActionContextRequired)?
            .semantic_identity();
        let mut admission = RuntimeRetainedValueAdmission::for_program(
            owner.clone(),
            crate::entry::RuntimeSchemaLimits::engine_default(),
        );
        crate::action::RuntimeActionMailbox::<RuntimePayload>::validate_destination(
            payload_type,
            &mut admission,
        )?;
        self.admit_live_retained_values(&mut admission)?;
        self.actions
            .preflight_emission_header(payload_type, &mut admission)?;
        let image = self
            .inert_rollback_image()
            .map_err(|message| RuntimeEffectExecutionError::Checkpoint { message })?;
        let result = (|| {
            let payload = self.evaluate_expr_with_backend(payload, backend)?;
            self.actions
                .emit(payload_type, RuntimePayload::new(payload), &mut admission)
                .map_err(|error| RuntimeEffectExecutionError::Admission(error.error))
        })();
        if let Err(cause) = result {
            *self = Self::from_rollback_image(image, &owner).map_err(|message| {
                RuntimeEffectExecutionError::Rollback {
                    cause: Box::new(cause.clone()),
                    message,
                }
            })?;
            return Err(cause);
        }
        Ok(())
    }
}
