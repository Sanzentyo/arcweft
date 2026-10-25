//! Compiler access to an original Program task body through the common Source
//! owner. Borrowing code grants no task receipt or runtime execution authority.

use super::{RuntimeTaskSourceError, RuntimeTaskSourceRef};
use crate::plan::{
    FlowOp, RuntimeFunctionSite, RuntimePlan, RuntimeTaskPlanBuildCoordinate, RuntimeTaskPlanRef,
    RuntimeTaskPlanSealLimits,
};
use crate::runtime_id::RuntimeLocalDeclarationId;
use crate::task::NeedProducerDefinition;
use crate::task::semantic::TaskSemanticMeter;

/// One cumulative allowance for original Source reads in a compilation.
/// It cannot be cloned to refresh consumed or rejected work.
pub struct RuntimeTaskSourceReadContext {
    limits: RuntimeTaskPlanSealLimits,
    meter: TaskSemanticMeter,
}

impl RuntimeTaskSourceReadContext {
    #[must_use]
    pub const fn new(limits: RuntimeTaskPlanSealLimits) -> Self {
        Self {
            limits,
            meter: TaskSemanticMeter::new(limits.max_semantic_work, limits.max_transcript_bytes),
        }
    }
}

impl std::fmt::Debug for RuntimeTaskSourceReadContext {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RuntimeTaskSourceReadContext")
            .finish_non_exhaustive()
    }
}

/// Read-only code projection of a sealed task in its actual original program.
/// It cannot be decoded or constructed from an asserted key or copied body.
pub struct RuntimeProgramTaskBodyRef<'program> {
    source: RuntimeTaskSourceRef<'program>,
    function: &'program RuntimeFunctionSite,
    producer: &'program NeedProducerDefinition,
    captures: &'program [RuntimeLocalDeclarationId],
    body: &'program [FlowOp],
}

impl RuntimePlan {
    /// Borrows original Program code under the same Source allowance used by
    /// runtime admission. Equal coordinates from another plan do not resolve.
    pub fn task_program_body(
        &self,
        coordinate: &RuntimeTaskPlanBuildCoordinate,
        context: &mut RuntimeTaskSourceReadContext,
    ) -> Result<RuntimeProgramTaskBodyRef<'_>, RuntimeTaskSourceError> {
        let source = self.task_source(coordinate, context.limits, &mut context.meter)?;
        let endpoint = source.request;
        let Some(FlowOp::ProgramTask {
            producer,
            captures,
            body,
            ..
        }) = endpoint.operation()
        else {
            context.meter.reject_owner();
            return Err(RuntimeTaskSourceError::NotProgramBody);
        };
        context.meter.charge_work(1).map_err(|error| {
            RuntimeTaskSourceError::Function(crate::plan::RuntimeBodySemanticError::from(error))
        })?;
        if captures.len() > context.limits.max_function_roles as usize {
            context.meter.reject_owner();
            return Err(RuntimeTaskSourceError::Function(
                crate::plan::RuntimeBodySemanticError::FunctionRoles {
                    actual: captures.len(),
                    maximum: context.limits.max_function_roles,
                },
            ));
        }
        let function = self
            .function_sites()
            .get(endpoint.function_id())
            .expect("an admitted original endpoint retains its actual function");
        Ok(RuntimeProgramTaskBodyRef {
            source,
            function,
            producer,
            captures,
            body,
        })
    }
}

impl RuntimeProgramTaskBodyRef<'_> {
    #[must_use]
    pub fn task_plan(&self) -> RuntimeTaskPlanRef<'_> {
        self.source.task_plan()
    }

    /// Actual enclosing declaration, including its lexical type contract.
    #[must_use]
    pub const fn function(&self) -> &RuntimeFunctionSite {
        self.function
    }

    #[must_use]
    pub const fn producer(&self) -> &NeedProducerDefinition {
        self.producer
    }

    #[must_use]
    pub const fn captures(&self) -> &[RuntimeLocalDeclarationId] {
        self.captures
    }

    #[must_use]
    pub const fn body(&self) -> &[FlowOp] {
        self.body
    }
}

#[cfg(test)]
mod tests;
