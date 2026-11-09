//! A pure scope joins its value or residual before its enclosing continuation.

use super::{
    ExprId, FinalExprLowerer, PureTryContinuation, RuntimeExprMatchArmSeed, RuntimeExprSeed,
    RuntimeExprSeedKind, RuntimeLocalReadMode, RuntimeLocalReadSeed, RuntimeNormalizedType,
    RuntimeScopeOwner, RuntimeTryBoundaryOwner, RuntimeTypeShape, StmtId,
    normalized_variant_binding_pattern_seed, normalized_variant_expression_seed,
};

impl FinalExprLowerer<'_> {
    pub(super) fn lower_scoped_block(
        &self,
        owner: ExprId,
        statements: &[StmtId],
        tail: ExprId,
    ) -> Result<RuntimeExprSeed, String> {
        let fact = self
            .semantic_facts
            .expression_scope(owner)
            .ok_or_else(|| format!("Scope {owner:?} has no checked fact"))?;
        let body = if let Some(unit) = fact.semantic_unit() {
            self.lower_function_block(
                statements,
                tail,
                PureTryContinuation::DiscardValue {
                    body: Box::new(RuntimeExprSeed::new(
                        unit.identity(),
                        RuntimeExprSeedKind::Value(arcweft_core::value::RuntimeValue::Unit),
                    )),
                    outer: Box::new(PureTryContinuation::Return),
                },
            )?
        } else {
            self.lower_block(owner, statements, tail)?
        };
        Ok(scoped_value(fact.identity(), body))
    }

    pub(super) fn lower_scoped_carrier(
        &self,
        owner: ExprId,
        block: &arcweft_lang_hir::expr::HirComputationBlockExpr,
    ) -> Result<RuntimeExprSeed, String> {
        let fact = self
            .semantic_facts
            .expression_scope(owner)
            .ok_or_else(|| format!("Carrier scope {owner:?} has no checked fact"))?;
        if !fact
            .carrier_block()
            .ok_or_else(|| format!("Carrier scope {owner:?} has no local carrier type"))?
            .delivers_value()
        {
            return self.lower_scoped_block(owner, block.statements(), block.tail());
        }
        let body = self.lower_function_block(
            block.statements(),
            block.tail(),
            PureTryContinuation::WrapCarrier { owner },
        )?;
        Ok(scoped_value(fact.identity(), body))
    }

    pub(super) fn propagate_pure_residual(
        &self,
        boundary: RuntimeTryBoundaryOwner,
        boundary_type: &RuntimeNormalizedType,
        residual: Option<RuntimeExprSeed>,
        outer: PureTryContinuation,
    ) -> Result<RuntimeExprSeed, String> {
        if let Some(scope) = self
            .scope_continuations
            .iter()
            .rev()
            .find(|scope| scope.boundary() == boundary)
        {
            return normalized_variant_expression_seed(scope.carrier_type(), 1, residual)
                .map_err(|error| format!("scope residual join is invalid: {error}"));
        }
        let propagated = normalized_variant_expression_seed(boundary_type, 1, residual)
            .map_err(|error| format!("propagated residual is invalid: {error}"))?;
        if let RuntimeTryBoundaryOwner::CarrierBlock(owner) = boundary
            && !outer.targets_carrier(owner)
        {
            return Err(format!(
                "propagation has no active carrier continuation for {owner:?}"
            ));
        }
        Ok(propagated)
    }

    pub(super) fn lower_scope_continuation(
        &self,
        owner: ExprId,
        statements: &[StmtId],
        tail: ExprId,
        outer: PureTryContinuation,
    ) -> Result<RuntimeExprSeed, String> {
        let fact = self
            .semantic_facts
            .expression_scope(owner)
            .ok_or_else(|| format!("Scope {owner:?} has no checked fact"))?;
        let Some(continuation) = fact.continuation() else {
            return self
                .apply_try_continuation(self.lower_scoped_block(owner, statements, tail)?, outer);
        };
        let (statements, tail) =
            if matches!(continuation.value_type().shape(), RuntimeTypeShape::Never) {
                let (prefix, terminal) = self.terminal_block_prefix(statements, tail)?;
                if !self.block_contains_selected_try(prefix, terminal, &|owner| {
                    continuation.exits().binary_search(&owner).is_ok()
                })? {
                    return self.apply_try_continuation(
                        self.lower_scoped_block(owner, prefix, terminal)?,
                        outer,
                    );
                }
                (prefix, terminal)
            } else {
                (statements, tail)
            };
        let locals = self
            .scope_locals
            .and_then(|locals| locals.get(&RuntimeScopeOwner::Expression(owner)))
            .ok_or_else(|| format!("Scope {owner:?} has no admitted continuation locals"))?;
        let mut body_lowerer = self.clone_with_overrides(super::RuntimeExpressionOverrides::new());
        body_lowerer.scope_continuations.push(continuation.clone());
        let success = PureTryContinuation::WrapSuccess {
            boundary: Box::new(continuation.carrier_type().clone()),
        };
        let inside = if let Some(unit) = fact.semantic_unit() {
            let wrapped = body_lowerer.apply_try_continuation(
                RuntimeExprSeed::new(
                    unit.identity(),
                    RuntimeExprSeedKind::Value(arcweft_core::value::RuntimeValue::Unit),
                ),
                success.clone(),
            )?;
            PureTryContinuation::DiscardValue {
                body: Box::new(wrapped),
                outer: Box::new(success),
            }
        } else {
            success
        };
        let body = body_lowerer.lower_function_block(statements, tail, inside)?;
        let scrutinee = RuntimeExprSeed::new(
            continuation.carrier_type().identity(),
            RuntimeExprSeedKind::Scope {
                identity: fact.identity().clone(),
                body: Box::new(body),
            },
        );
        let success = self.apply_try_continuation(
            RuntimeExprSeed::new(
                continuation.value_type().identity(),
                RuntimeExprSeedKind::Local(RuntimeLocalReadSeed::new(
                    locals.success.clone(),
                    RuntimeLocalReadMode::Move,
                )),
            ),
            outer.clone(),
        )?;
        let residual = match (continuation.residual_type(), &locals.residual) {
            (Some(ty), Some(local)) => Some(RuntimeExprSeed::new(
                ty.identity(),
                RuntimeExprSeedKind::Local(RuntimeLocalReadSeed::new(
                    local.clone(),
                    RuntimeLocalReadMode::Move,
                )),
            )),
            (None, None) => None,
            _ => {
                return Err(format!(
                    "Scope {owner:?} residual local does not match its carrier"
                ));
            }
        };
        let failure = self.propagate_pure_residual(
            continuation.boundary(),
            continuation.boundary_type(),
            residual,
            outer,
        )?;
        if success.ty() != failure.ty() {
            return Err(format!(
                "Scope {owner:?} continuation branches have different types"
            ));
        }
        Ok(RuntimeExprSeed::new(
            success.ty(),
            RuntimeExprSeedKind::Match {
                scrutinee: Box::new(scrutinee),
                arms: vec![
                    RuntimeExprMatchArmSeed::new(
                        normalized_variant_binding_pattern_seed(
                            continuation.carrier_type(),
                            0,
                            Some(locals.success.clone()),
                        )
                        .map_err(|error| error.to_string())?,
                        None,
                        success,
                    ),
                    RuntimeExprMatchArmSeed::new(
                        normalized_variant_binding_pattern_seed(
                            continuation.carrier_type(),
                            1,
                            locals.residual.clone(),
                        )
                        .map_err(|error| error.to_string())?,
                        None,
                        failure,
                    ),
                ]
                .into_boxed_slice(),
            },
        ))
    }
}

fn scoped_value(
    identity: &arcweft_core::scope::RuntimeScopeIdentity,
    body: RuntimeExprSeed,
) -> RuntimeExprSeed {
    RuntimeExprSeed::new(
        body.ty(),
        RuntimeExprSeedKind::Scope {
            identity: identity.clone(),
            body: Box::new(body),
        },
    )
}
