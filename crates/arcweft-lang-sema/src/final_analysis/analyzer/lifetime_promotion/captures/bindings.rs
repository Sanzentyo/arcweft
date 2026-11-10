//! The input that supplies an immutable pattern binding's stored value.

use arcweft_lang_hir::{
    expr::HirExprKind,
    identity::{ExprId, LocalId, PatternId},
    module::HirModule,
    project::{
        HirBindingSite, HirExpressionBindingRole, HirLocalBindingOrigin,
        HirLocalBindingStatementRole,
    },
    stmt::HirStmtKind,
};

use super::{
    Analyzer, BorrowDependencies, CaptureWorklist, FinalSemanticAnalysisError, Observation,
    ReceiverBorrowQuery, StorageLexicalSite, ValueOrigin,
};

impl Analyzer<'_, '_, '_> {
    pub(super) fn stored_binding_dependencies(
        &self,
        module: &HirModule,
        (local, observation): (LocalId, Observation),
        origin: ValueOrigin,
        work: &mut CaptureWorklist,
        receiver: &mut ReceiverBorrowQuery<'_>,
    ) -> Result<BorrowDependencies, FinalSemanticAnalysisError> {
        let binding = self
            .topology
            .module(module.module_id())
            .and_then(|topology| topology.local_origins().binding(local))
            .ok_or(FinalSemanticAnalysisError::InvalidOwner)?;
        if let Some(expression) = binding.value() {
            return work.demand(
                ValueOrigin {
                    expression,
                    observation,
                    ..origin
                },
                origin,
                receiver,
            );
        }
        if let Some((expression, pattern)) = pattern_input(module, binding)? {
            let Some(observation) = self.binding_storage_observation(
                module,
                (pattern, local),
                observation,
                work,
                receiver,
            )?
            else {
                return Ok(BorrowDependencies::default());
            };
            let frame = self.lexical_frame(
                module,
                StorageLexicalSite::Binding(local),
                origin.frame,
                work,
                receiver,
            )?;
            return work.demand(
                ValueOrigin {
                    expression,
                    frame,
                    observation,
                },
                origin,
                receiver,
            );
        }
        let parameter = self.parameter_demand(
            module,
            local,
            ValueOrigin {
                observation,
                ..origin
            },
            work,
            receiver,
        )?;
        Ok(BorrowDependencies::Parameters(
            parameter.into_iter().collect(),
        ))
    }
}

fn pattern_input(
    module: &HirModule,
    binding: &HirLocalBindingOrigin,
) -> Result<Option<(ExprId, PatternId)>, FinalSemanticAnalysisError> {
    if binding.is_mutable_binding() {
        return Ok(None);
    }
    let invalid = || FinalSemanticAnalysisError::InvalidOwner;
    let input = match binding.site() {
        HirBindingSite::Statement { statement, role } => {
            use HirLocalBindingStatementRole as Role;
            let statement = module.resolve_stmt(statement).map_err(|_| invalid())?;
            match (role, statement.kind()) {
                (
                    Role::Let,
                    HirStmtKind::Let {
                        pattern,
                        initializer,
                        ..
                    },
                )
                | (
                    Role::LetElse,
                    HirStmtKind::LetElse {
                        pattern,
                        initializer,
                        ..
                    },
                ) => (*initializer, *pattern),
                (Role::IfLet, HirStmtKind::IfLet(conditional)) => {
                    (conditional.scrutinee(), conditional.pattern())
                }
                (Role::WhileLet, HirStmtKind::WhileLet(conditional)) => {
                    (conditional.scrutinee(), conditional.pattern())
                }
                (Role::MatchArm { arm }, HirStmtKind::Match(matched)) => (
                    matched.scrutinee(),
                    matched
                        .arms()
                        .get(arm as usize)
                        .ok_or_else(invalid)?
                        .pattern(),
                ),
                (
                    Role::For
                    | Role::SelectPattern { .. }
                    | Role::SelectBinding { .. }
                    | Role::OnTrigger,
                    _,
                ) => return Ok(None),
                _ => return Err(invalid()),
            }
        }
        HirBindingSite::Expression { expression, role } => {
            let expression = module.resolve_expr(expression).map_err(|_| invalid())?;
            match (role, expression.kind()) {
                (HirExpressionBindingRole::IfLet, HirExprKind::IfLet(conditional)) => {
                    (conditional.scrutinee(), conditional.pattern())
                }
                (HirExpressionBindingRole::MatchArm { arm }, HirExprKind::Match(matched)) => (
                    matched.scrutinee(),
                    matched
                        .arms()
                        .get(arm as usize)
                        .ok_or_else(invalid)?
                        .pattern(),
                ),
                (
                    HirExpressionBindingRole::IfLet | HirExpressionBindingRole::MatchArm { .. },
                    _,
                ) => return Err(invalid()),
                _ => return Ok(None),
            }
        }
        _ => return Ok(None),
    };
    if binding.pattern() != Some(input.1) {
        return Err(invalid());
    }
    Ok(Some(input))
}
