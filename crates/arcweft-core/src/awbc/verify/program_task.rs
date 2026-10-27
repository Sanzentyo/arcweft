//! Program fanout control and capture ABI verification, before code execution.

use super::AwbcVerifyError;
use crate::awbc::schema::{
    AwbcProgram, AwbcProgramFanoutInput, AwbcProgramTaskKernel, AwbcProgramTaskPlan,
    AwbcRuntimeTypeShape, AwbcTaskPlan, AwbcTaskPlanExecution, AwbcTaskPolicy, AwbcUnsignedIntKind,
};

impl AwbcProgramTaskPlan {
    pub(super) fn verify_fanout_contract(
        &self,
        program: &AwbcProgram,
        aggregate: &AwbcTaskPlan,
        at: &str,
    ) -> Result<(), AwbcVerifyError> {
        let AwbcProgramTaskKernel::Fanout {
            worker,
            source,
            limit,
            inputs,
        } = &self.body
        else {
            return Ok(());
        };
        let invalid = || {
            AwbcVerifyError::InvalidInvariant {
            at: at.to_owned(),
            message: "Program Fanout requires its owned collection, U32 limit and exact Flow-worker capture ABI".into(),
        }
        };
        let parent = program
            .functions
            .get(self.kernel.index())
            .ok_or_else(invalid)?;
        let params = &program
            .signatures
            .get(parent.signature.index())
            .ok_or_else(invalid)?
            .params;
        let source_type = params.get(*source as usize).ok_or_else(invalid)?;
        let limit_type = params.get(*limit as usize).ok_or_else(invalid)?;
        if source == limit
            || !matches!(
                program
                    .runtime_types
                    .get(limit_type.index())
                    .map(|ty| ty.shape()),
                Some(AwbcRuntimeTypeShape::UInt(AwbcUnsignedIntKind::U32))
            )
        {
            return Err(invalid());
        }
        let item = match program
            .runtime_types
            .get(source_type.index())
            .map(|ty| ty.shape())
        {
            Some(
                AwbcRuntimeTypeShape::Sequence { item, .. }
                | AwbcRuntimeTypeShape::Array { item, .. },
            ) => *item,
            _ => return Err(invalid()),
        };
        let child = program.task_plans.get(worker.index()).ok_or_else(invalid)?;
        let AwbcTaskPlanExecution::Program(child_body) = &child.execution else {
            return Err(invalid());
        };
        if !matches!(child_body.body, AwbcProgramTaskKernel::Flow)
            || child.policy != AwbcTaskPolicy::AlwaysStart
            || !matches!(
                aggregate.handle_type,
                crate::task::TaskHandleType::Need { .. }
            )
            || !matches!(child.handle_type, crate::task::TaskHandleType::Need { .. })
            || child_body.capture_modes.len() != inputs.len()
            || child_body
                .capture_modes
                .iter()
                .any(|mode| *mode != crate::value::RuntimeLocalReadMode::Move)
            || inputs
                .iter()
                .filter(|input| matches!(input, AwbcProgramFanoutInput::Item))
                .count()
                != 1
        {
            return Err(invalid());
        }
        let child_function = program
            .functions
            .get(child_body.kernel.index())
            .ok_or_else(invalid)?;
        let child_params = &program
            .signatures
            .get(child_function.signature.index())
            .ok_or_else(invalid)?
            .params;
        if child_params.len() != inputs.len() {
            return Err(invalid());
        }
        for (input, ty) in inputs.iter().zip(child_params) {
            let expected = match input {
                AwbcProgramFanoutInput::Item => item,
                AwbcProgramFanoutInput::Capture { position } => {
                    if position == source || position == limit {
                        return Err(invalid());
                    }
                    *params.get(*position as usize).ok_or_else(invalid)?
                }
            };
            if expected != *ty {
                return Err(invalid());
            }
        }
        let aggregate_type = program
            .runtime_types
            .get(aggregate.payload_type().index())
            .ok_or_else(invalid)?;
        let child_type = program
            .runtime_types
            .get(child.payload_type().index())
            .ok_or_else(invalid)?;
        crate::program_types::RuntimeProgramTypes::Awbc(program)
            .fanout_output_kind(
                aggregate_type.semantic_identity(),
                child_type.semantic_identity(),
            )
            .map_err(|_| invalid())?;
        Ok(())
    }
}
