//! Conditional evaluation in the flow-value continuation lowering.
//!
//! A branch evaluates its selector before entering exactly one body. Calls,
//! Try and suspension in a body stay under that body's control edge.

use std::collections::BTreeMap;

use arcweft_core::plan::{
    RuntimeExprSeed, RuntimeExprSeedKind, RuntimeFlowMatchArmSeed, RuntimeFlowOpSeed,
};
use arcweft_core::value::RuntimeValue;
use arcweft_lang_hir::expr::{HirBinaryOp, HirExprKind};
use arcweft_lang_hir::identity::ExprId;

use crate::errors::RuntimePlanLowerError;

use super::{FinalFlowLowerer, RuntimeFlowValueContinuation};

impl FinalFlowLowerer<'_> {
    pub(super) fn lower_value_branch(
        &mut self,
        owner: ExprId,
        selector: ExprId,
        continuation: RuntimeFlowValueContinuation,
        overrides: BTreeMap<ExprId, RuntimeExprSeed>,
    ) -> Result<Vec<RuntimeFlowOpSeed>, RuntimePlanLowerError> {
        self.lower_flow_value_with_overrides(
            selector,
            RuntimeFlowValueContinuation::Branch {
                owner,
                overrides: overrides.clone(),
                outer: Box::new(continuation),
            },
            overrides,
        )
    }

    #[expect(
        clippy::too_many_lines,
        reason = "one conditional evaluation boundary keeps each selector paired with its own lazy branch bodies"
    )]
    pub(super) fn finish_value_branch(
        &mut self,
        owner: ExprId,
        selector: RuntimeExprSeed,
        continuation: RuntimeFlowValueContinuation,
        overrides: BTreeMap<ExprId, RuntimeExprSeed>,
    ) -> Result<Vec<RuntimeFlowOpSeed>, RuntimePlanLowerError> {
        let expression = self
            .module
            .resolve_expr(owner)
            .map_err(|error| RuntimePlanLowerError::new(error.to_string()))?;
        let op = match expression.kind() {
            HirExprKind::If(branch) => RuntimeFlowOpSeed::If {
                condition: selector,
                then_ops: self.lower_flow_value_with_overrides(
                    branch.then_branch(),
                    continuation.clone(),
                    overrides.clone(),
                )?,
                else_ops: self.lower_flow_value_with_overrides(
                    branch.else_branch(),
                    continuation,
                    overrides,
                )?,
            },
            HirExprKind::IfLet(branch) => RuntimeFlowOpSeed::IfLet {
                pattern: self
                    .pattern_lowerer()
                    .lower(branch.pattern())
                    .map_err(RuntimePlanLowerError::new)?,
                expr: selector,
                guard: branch
                    .guard()
                    .map(|guard| {
                        self.expr_lowerer()
                            .with_overrides(overrides.clone())
                            .lower(guard)
                            .map_err(RuntimePlanLowerError::new)
                    })
                    .transpose()?,
                then_ops: self.lower_flow_value_with_overrides(
                    branch.then_branch(),
                    continuation.clone(),
                    overrides.clone(),
                )?,
                else_ops: self.lower_flow_value_with_overrides(
                    branch.else_branch(),
                    continuation,
                    overrides,
                )?,
            },
            HirExprKind::Match(branch) => {
                let mut arms = Vec::with_capacity(branch.arms().len());
                for arm in branch.arms() {
                    arms.push(RuntimeFlowMatchArmSeed {
                        pattern: self
                            .pattern_lowerer()
                            .lower(arm.pattern())
                            .map_err(RuntimePlanLowerError::new)?,
                        guard: arm
                            .guard()
                            .map(|guard| {
                                self.expr_lowerer()
                                    .with_overrides(overrides.clone())
                                    .lower(guard)
                                    .map_err(RuntimePlanLowerError::new)
                            })
                            .transpose()?,
                        ops: self.lower_flow_value_with_overrides(
                            arm.value(),
                            continuation.clone(),
                            overrides.clone(),
                        )?,
                    });
                }
                RuntimeFlowOpSeed::Match {
                    scrutinee: selector,
                    arms,
                }
            }
            HirExprKind::Binary(binary) => {
                let skipped = match binary.operator() {
                    HirBinaryOp::And => false,
                    HirBinaryOp::Or | HirBinaryOp::Implies => true,
                    _ => {
                        return Err(RuntimePlanLowerError::new(
                            "strict binary operator reached conditional value lowering",
                        ));
                    }
                };
                let skipped = self.apply_value_continuation(
                    RuntimeExprSeed::new(
                        selector.ty(),
                        RuntimeExprSeedKind::Value(RuntimeValue::Bool(skipped)),
                    ),
                    continuation.clone(),
                )?;
                let evaluated =
                    self.lower_flow_value_with_overrides(binary.right(), continuation, overrides)?;
                let (then_ops, else_ops) = match binary.operator() {
                    HirBinaryOp::Or => (skipped, evaluated),
                    _ => (evaluated, skipped),
                };
                RuntimeFlowOpSeed::If {
                    condition: selector,
                    then_ops,
                    else_ops,
                }
            }
            _ => {
                return Err(RuntimePlanLowerError::new(
                    "conditional value continuation has a non-branch owner",
                ));
            }
        };
        Ok(vec![op])
    }
}
