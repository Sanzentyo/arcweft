//! Extraction admission over the final selected execution/input authority.

use std::collections::BTreeSet;

use arcweft_lang_hir::identity::{ExprId, LocalId, StmtId};

use crate::{effects::EffectSet, semantic_coordinate::CheckedControlTransferTarget};

use super::{
    CheckedCallableBoundary, CheckedClosedExecutionContext, CheckedExecutionBodyOwner,
    CheckedExecutionContextError, CheckedExecutionInputAbi, CheckedExecutionInputRole,
    CheckedExecutionSource, CheckedExpressionResolution, CheckedLocalPlaceMode,
    CheckedStatementPayload, CheckedSuspensionRole, CheckedTryBoundaryOwner,
};

/// A deterministic extracted root with boundary-relative control and place
/// admission. Owned inputs may move or change inside the root. The runtime
/// ingress still must discharge any borrowed input's deep Copy obligation.
#[derive(Debug)]
pub struct CheckedDeterministicProgram {
    inputs: CheckedExecutionInputAbi,
}

impl CheckedDeterministicProgram {
    pub const fn input_abi(&self) -> &CheckedExecutionInputAbi {
        &self.inputs
    }

    /// Projects the admitted execution intent into HIR dependency reachability.
    /// This selects an owner; the same input proof still owns actual execution.
    pub fn reachability_owner(&self) -> arcweft_lang_hir::project::HirRuntimeExecutableOwner {
        use arcweft_lang_hir::project::HirRuntimeExecutableOwner;
        match self.inputs.source() {
            CheckedExecutionSource::ExportBinding(owner) => {
                HirRuntimeExecutableOwner::Statement(*owner)
            }
            CheckedExecutionSource::SelectMatch(owner) => {
                HirRuntimeExecutableOwner::MatchSelection(*owner)
            }
            CheckedExecutionSource::ExportIteration(owner) => {
                HirRuntimeExecutableOwner::IterationBindings(*owner)
            }
            CheckedExecutionSource::EvaluateValue(owner) => {
                HirRuntimeExecutableOwner::Value(*owner)
            }
            CheckedExecutionSource::InvokeBody(CheckedExecutionBodyOwner::CallableValue(owner))
            | CheckedExecutionSource::ExportMutation(CheckedExecutionBodyOwner::CallableValue(
                owner,
            )) => HirRuntimeExecutableOwner::CallableBody(*owner),
            CheckedExecutionSource::InvokeBody(CheckedExecutionBodyOwner::Declaration {
                declaration,
                role,
            })
            | CheckedExecutionSource::ExportMutation(CheckedExecutionBodyOwner::Declaration {
                declaration,
                role,
            }) => HirRuntimeExecutableOwner::DeclarationBody {
                declaration: declaration.clone(),
                role: *role,
            },
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CheckedProgramAdmissionError {
    #[error(transparent)]
    Context(#[from] Box<CheckedExecutionContextError>),
    #[error("extracted program has external effects: {effects:?}")]
    Effects { effects: EffectSet },
    #[error("extracted program may suspend")]
    Suspension,
    #[error("extracted program changes external local {local:?} through {mode:?}")]
    ExternalPlace {
        local: LocalId,
        mode: CheckedLocalPlaceMode,
    },
    #[error("statement {statement:?} transfers control outside the extracted program")]
    ExternalControl { statement: StmtId },
    #[error("Try expression {expression:?} propagates outside the extracted program")]
    ExternalTry { expression: ExprId },
}

impl From<CheckedExecutionContextError> for CheckedProgramAdmissionError {
    fn from(error: CheckedExecutionContextError) -> Self {
        Self::Context(Box::new(error))
    }
}

impl CheckedClosedExecutionContext<'_> {
    /// Issues extraction admission from the same eager DAG, closed instance,
    /// typed control targets and local-use rows that issue the input ABI.
    /// Flow-required control is permitted when its target belongs to this
    /// root; that classification alone does not imply an external effect.
    pub fn checked_deterministic_program(
        &self,
        source: impl Into<CheckedExecutionSource>,
    ) -> Result<CheckedDeterministicProgram, CheckedProgramAdmissionError> {
        let inputs = self.checked_execution_input_abi(source)?;
        if !inputs.effects().is_empty() {
            return Err(CheckedProgramAdmissionError::Effects {
                effects: inputs.effects().clone(),
            });
        }
        if inputs.suspension() != CheckedSuspensionRole::NonSuspending {
            return Err(CheckedProgramAdmissionError::Suspension);
        }
        let external_places = inputs
            .inputs()
            .iter()
            .filter(|input| input.role() == &CheckedExecutionInputRole::Free)
            .map(|input| input.binding().local())
            .collect::<BTreeSet<_>>();
        let exported_places =
            if matches!(inputs.source(), CheckedExecutionSource::ExportMutation(_)) {
                inputs
                    .binding_outputs()
                    .iter()
                    .map(|output| output.local())
                    .collect()
            } else {
                BTreeSet::new()
            };
        let mut cleanup = Vec::new();
        self.validate_program_frame(&external_places, &exported_places, &inputs, &mut cleanup)?;
        let mut visited = BTreeSet::new();
        while let Some(owner) = cleanup.pop() {
            if visited.insert(owner) {
                let body = self.checked_execution_input_abi(owner)?;
                self.validate_program_frame(
                    &external_places,
                    // Cleanup owns a separate captured frame and has no
                    // returned publication ABI for its caller's bindings.
                    &BTreeSet::new(),
                    &body,
                    &mut cleanup,
                )?;
            }
        }
        Ok(CheckedDeterministicProgram { inputs })
    }

    // Defer executes in its own captured frame. Its effects/suspension are
    // already folded into the root, while its control targets must be checked
    // relative to that independent cleanup body. Follow checked dependencies,
    // never HIR children or the creation root's eager input inventory.
    fn validate_program_frame(
        &self,
        external_places: &BTreeSet<LocalId>,
        exported_places: &BTreeSet<LocalId>,
        inputs: &CheckedExecutionInputAbi,
        cleanup: &mut Vec<ExprId>,
    ) -> Result<(), CheckedProgramAdmissionError> {
        for input in inputs.inputs() {
            if !external_places.contains(&input.binding().local()) {
                continue;
            }
            for usage in input.uses() {
                if let Some(access) = usage.access().place_access() {
                    if exported_places.contains(&access.place().local_id()) {
                        continue;
                    }
                    return Err(CheckedProgramAdmissionError::ExternalPlace {
                        local: access.place().local_id(),
                        mode: access.mode(),
                    });
                }
            }
        }
        for &statement in inputs.statements() {
            let checked = self
                .analysis()
                .statement(statement)
                .ok_or(super::FinalSemanticAnalysisError::InvalidOwner)
                .map_err(CheckedExecutionContextError::from)?;
            let target = match checked.payload() {
                CheckedStatementPayload::ControlTransfer(target) => target,
                CheckedStatementPayload::Defer(defer) => {
                    cleanup.push(defer.body());
                    continue;
                }
                _ => continue,
            };
            let admitted = match target {
                CheckedControlTransferTarget::Return(boundary) => inputs.admits_callable(boundary),
                CheckedControlTransferTarget::Output(target) => {
                    inputs.contains_expression(target.application())
                }
                CheckedControlTransferTarget::Loop(target) => target
                    .body()
                    .path()
                    .is_at_or_below(inputs.coordinate().path()),
            };
            if !admitted {
                return Err(CheckedProgramAdmissionError::ExternalControl { statement });
            }
        }
        let invoked_callable = match inputs.source() {
            CheckedExecutionSource::InvokeBody(CheckedExecutionBodyOwner::CallableValue(owner))
            | CheckedExecutionSource::ExportMutation(CheckedExecutionBodyOwner::CallableValue(
                owner,
            )) => Some(*owner),
            _ => None,
        };
        for expression in inputs.expressions().iter().copied().chain(invoked_callable) {
            let checked = self
                .analysis()
                .expression(expression)
                .ok_or(super::FinalSemanticAnalysisError::InvalidOwner)
                .map_err(CheckedExecutionContextError::from)?;
            let checked = match checked.resolution() {
                CheckedExpressionResolution::Try(checked) => Some(checked),
                CheckedExpressionResolution::ImplicitCallable(callable)
                    if invoked_callable == Some(expression) =>
                {
                    match callable.body() {
                        super::CheckedImplicitCallableBody::Try(checked) => Some(checked),
                        _ => None,
                    }
                }
                _ => None,
            };
            let Some(checked) = checked else {
                continue;
            };
            let admitted = match checked.boundary().owner() {
                CheckedTryBoundaryOwner::Infallible => true,
                CheckedTryBoundaryOwner::CarrierBlock(boundary) => {
                    inputs.contains_expression(boundary.lookup_owner())
                }
                CheckedTryBoundaryOwner::Callable(boundary) => inputs.admits_callable(boundary),
            };
            if !admitted {
                return Err(CheckedProgramAdmissionError::ExternalTry { expression });
            }
        }
        Ok(())
    }
}

impl CheckedExecutionInputAbi {
    fn admits_callable(&self, boundary: &CheckedCallableBoundary) -> bool {
        match (self.source(), boundary) {
            (
                CheckedExecutionSource::InvokeBody(CheckedExecutionBodyOwner::CallableValue(owner))
                | CheckedExecutionSource::ExportMutation(CheckedExecutionBodyOwner::CallableValue(
                    owner,
                )),
                CheckedCallableBoundary::FunctionSite(site),
            ) => {
                *owner == site.site().lookup_owner()
                    && self.coordinate().path() == site.site().coordinate()
            }
            (
                CheckedExecutionSource::InvokeBody(CheckedExecutionBodyOwner::Declaration {
                    declaration,
                    ..
                })
                | CheckedExecutionSource::ExportMutation(CheckedExecutionBodyOwner::Declaration {
                    declaration,
                    ..
                }),
                CheckedCallableBoundary::Declaration(target),
            ) => declaration == target.declaration(),
            _ => false,
        }
    }
}
