//! Action delivery is authorized by the current original code operation.

use super::Engine;
use super::audio::RuntimeEffectExecutionError;
use super::code_cursor::NativeSourceOperation;
use crate::effect::{RuntimeEffectExpr, RuntimeEffectMaterializeError};
use crate::plan::FlowOp;
use crate::pure::RuntimeCallBackend;
use crate::task::RuntimeProgramOwner;
use crate::value::{RuntimePayload, RuntimeRetainedValueAdmission};

/// A parked original receive operation owns no message or reconstructed code.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeActionReceiveState {
    pub(super) source: NativeSourceOperation,
    pub(super) payload: crate::pattern::RuntimeSemanticTypeId,
}

/// Native custody or an observation of a Product-owned suspended instruction.
#[derive(Clone, Debug, PartialEq)]
pub enum ActionWaitingStatus {
    Native(NativeActionReceiveState),
    Observed {
        payload: crate::pattern::RuntimeSemanticTypeId,
    },
}

impl ActionWaitingStatus {
    pub(crate) const fn payload(&self) -> crate::pattern::RuntimeSemanticTypeId {
        match self {
            Self::Native(state) => state.payload,
            Self::Observed { payload } => *payload,
        }
    }
}

impl NativeActionReceiveState {
    pub(super) fn admit_frame<V: crate::value::RuntimeStoredValue>(
        source: &NativeSourceOperation,
        env: &crate::value::RuntimeEnv<V>,
        admission: &mut RuntimeRetainedValueAdmission,
    ) -> Result<crate::pattern::RuntimeSemanticTypeId, crate::value::AwbcRuntimeValueSnapshotError>
    {
        source.admit_retained(admission)?;
        let owner = admission
            .program_owner()
            .expect("source admitted its program")
            .clone();
        let RuntimeProgramOwner::Plan(plan) = &owner else {
            unreachable!("Native source owns a Plan");
        };
        let Some(FlowOp::ReceiveAction { channel, result }) = source.operation(plan) else {
            return Err(crate::program_types::RuntimeProgramTypeError::InvalidActionChannel.into());
        };
        let declaration = |local| {
            plan.local_declarations()
                .get(local)
                .and_then(|row| plan.type_table().get(row.ty()))
                .map(|row| row.semantic_identity())
                .ok_or(crate::program_types::RuntimeProgramTypeError::InvalidActionChannel)
        };
        let channel_type = declaration(*channel)?;
        let result_type = declaration(*result)?;
        let channel = env
            .get_stored(*channel)
            .ok_or(crate::program_types::RuntimeProgramTypeError::InvalidActionChannel)?;
        let payload = admission.admit_composite(V::admission_origin(), |_, resources| {
            owner
                .types()
                .action_receive_payload(
                    channel.borrowed_view(),
                    channel_type,
                    result_type,
                    env.function_instantiation(),
                    resources,
                )
                .map_err(Into::into)
        })?;
        if env.get_stored(*result).is_some() {
            return Err(crate::program_types::RuntimeProgramTypeError::InvalidActionChannel.into());
        }
        Ok(payload)
    }
}

impl Engine {
    pub(crate) fn emit_external_action(
        &mut self,
        payload_type: crate::pattern::RuntimeSemanticTypeId,
        payload: crate::value::RuntimeValue,
    ) -> Result<(), crate::executor::RuntimeActionEmissionError> {
        let mut admission = RuntimeRetainedValueAdmission::for_program(
            RuntimeProgramOwner::Plan(self.plan.clone()),
            crate::entry::RuntimeSchemaLimits::engine_default(),
        );
        self.admit_live_retained_values(&mut admission)
            .map_err(Box::new)?;
        let prepared = self
            .actions
            .prepare_emit_in_admitted_group(payload_type, (&payload).into(), &mut admission)
            .map_err(Box::new)?;
        prepared.commit(RuntimePayload::new(payload));
        Ok(())
    }

    pub(super) fn execute_action_receive(
        &mut self,
        source: &NativeSourceOperation,
    ) -> Result<(), RuntimeEffectExecutionError> {
        use crate::value::RuntimeStoredValue;
        let owner = RuntimeProgramOwner::Plan(self.plan.clone());
        let mut admission = RuntimeRetainedValueAdmission::for_program(
            owner,
            crate::entry::RuntimeSchemaLimits::engine_default(),
        );
        self.admit_live_retained_values(&mut admission)?;
        let payload =
            NativeActionReceiveState::admit_frame(source, &self.fiber.frame.env, &mut admission)?;
        let Some(FlowOp::ReceiveAction { result, .. }) = source.operation(&self.plan) else {
            unreachable!("admitted receive source");
        };
        let result = *result;
        let ty = self
            .plan
            .type_table()
            .get(
                self.plan
                    .local_declarations()
                    .get(result)
                    .expect("original result")
                    .ty(),
            )
            .expect("original type")
            .semantic_identity();
        if let Some(value) = self.actions.peek(payload) {
            admission.validate_owned_typed_borrowed_in_invocation(
                value.borrowed_value(),
                ty,
                self.fiber.frame.env.function_instantiation(),
            )?;
        }
        if let Some(value) = self.actions.receive(payload, &mut admission)? {
            self.fiber.frame.env.set(result, value.into_value());
        } else {
            self.fiber.frame.status = super::FlowFiberStatus::ActionWaiting(
                ActionWaitingStatus::Native(NativeActionReceiveState {
                    source: source.clone(),
                    payload,
                }),
            );
        }
        Ok(())
    }

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
