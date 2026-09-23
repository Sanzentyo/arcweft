//! Typed lexical frame joins: close the frame before either outer continuation.

use super::{
    ExprId, FinalFlowLowerer, HirContextualStmtBody, RuntimeExprSeed, RuntimeFlowMatchArmSeed,
    RuntimeFlowOpSeed, RuntimeFlowTail, RuntimeFlowValueContinuation, RuntimePlanLowerError,
    RuntimeScopeContinuation, RuntimeScopeFact, RuntimeScopeOwner, RuntimeTypeShape, RuntimeValue,
    StmtId, bind_seed, local_seed, normalized_variant_binding_pattern_seed,
    normalized_variant_expression_seed,
};

#[derive(Clone)]
pub(super) struct ScopeContinuationFrame {
    pub(super) owner: RuntimeScopeOwner,
    pub(super) fact: RuntimeScopeContinuation,
    outer: RuntimeFlowValueContinuation,
}

impl FinalFlowLowerer<'_> {
    pub(super) fn lower_scope_value(
        &mut self,
        expression: ExprId,
        fact: &RuntimeScopeFact,
        statements: &[StmtId],
        tail: ExprId,
        outer: RuntimeFlowValueContinuation,
    ) -> Result<Vec<RuntimeFlowOpSeed>, RuntimePlanLowerError> {
        let owner = RuntimeScopeOwner::Expression(expression);
        let continuation = fact
            .continuation()
            .ok_or_else(|| RuntimePlanLowerError::new("scope value has no continuation fact"))?;
        self.scope_continuations.push(ScopeContinuationFrame {
            owner,
            fact: continuation.clone(),
            outer,
        });
        let body = self.lower_value_block(
            statements,
            tail,
            RuntimeFlowValueContinuation::ScopeSuccess { owner },
        );
        self.scope_continuations.pop();
        let mut ops = vec![RuntimeFlowOpSeed::EnterScope {
            identity: fact.identity().clone(),
        }];
        ops.extend(body?);
        if matches!(continuation.value_type().shape(), RuntimeTypeShape::Never) {
            ops.push(RuntimeFlowOpSeed::ExitScope);
        }
        Ok(ops)
    }

    pub(super) fn lower_scope_statement(
        &mut self,
        statement: StmtId,
        body: &HirContextualStmtBody,
        outer: RuntimeFlowTail,
    ) -> Result<Vec<RuntimeFlowOpSeed>, RuntimePlanLowerError> {
        let fact = self
            .semantic_facts
            .statement_scope(statement)
            .cloned()
            .ok_or_else(|| {
                RuntimePlanLowerError::new(format!("Scope {statement:?} has no checked fact"))
            })?;
        let Some(continuation) = fact.continuation() else {
            let mut ops = vec![RuntimeFlowOpSeed::Scope {
                identity: fact.identity().clone(),
                body: self.lower_contextual_body(body)?,
            }];
            ops.extend(self.lower_flow_tail(outer)?);
            return Ok(ops);
        };
        let owner = RuntimeScopeOwner::Statement(statement);
        let unit = RuntimeExprSeed::new(
            continuation.value_type().identity(),
            arcweft_core::plan::RuntimeExprSeedKind::Value(RuntimeValue::Unit),
        );
        self.scope_continuations.push(ScopeContinuationFrame {
            owner,
            fact: continuation.clone(),
            outer: RuntimeFlowValueContinuation::Ignore(outer),
        });
        let tail = RuntimeFlowTail::ContinueValue {
            value: unit,
            continuation: Box::new(RuntimeFlowValueContinuation::ScopeSuccess { owner }),
        };
        let lowered = match body {
            HirContextualStmtBody::Ordinary { statements, .. } => {
                self.lower_statement_ids_with_tail(statements, tail)
            }
            HirContextualStmtBody::Thread(body) => {
                self.lower_thread_items_with_tail(body.items(), tail)
            }
        };
        self.scope_continuations.pop();
        let mut ops = vec![RuntimeFlowOpSeed::EnterScope {
            identity: fact.identity().clone(),
        }];
        ops.extend(lowered?);
        Ok(ops)
    }

    pub(super) fn complete_scope_success(
        &mut self,
        owner: RuntimeScopeOwner,
        value: RuntimeExprSeed,
    ) -> Result<Vec<RuntimeFlowOpSeed>, RuntimePlanLowerError> {
        let frame = self
            .scope_continuations
            .last()
            .filter(|frame| frame.owner == owner)
            .ok_or_else(|| {
                RuntimePlanLowerError::new("scope success targets a non-active frame")
            })?;
        let carrier = normalized_variant_expression_seed(frame.fact.carrier_type(), 0, Some(value))
            .map_err(|error| RuntimePlanLowerError::new(error.to_string()))?;
        self.complete_scope_carrier(owner, carrier)
    }

    pub(super) fn complete_scope_carrier(
        &mut self,
        owner: RuntimeScopeOwner,
        value: RuntimeExprSeed,
    ) -> Result<Vec<RuntimeFlowOpSeed>, RuntimePlanLowerError> {
        let frame = self
            .scope_continuations
            .pop()
            .filter(|frame| frame.owner == owner)
            .ok_or_else(|| {
                RuntimePlanLowerError::new("scope residual must leave its innermost active frame")
            })?;
        // The continuation is compiled under the parent lexical context too.
        // Restoring this construction stack afterward lets sibling paths reuse
        // the same accepted frame without executing the parent inside it.
        let lowered = self.dispatch_scope_carrier(&frame, value);
        self.scope_continuations.push(frame);
        lowered
    }

    fn dispatch_scope_carrier(
        &mut self,
        frame: &ScopeContinuationFrame,
        value: RuntimeExprSeed,
    ) -> Result<Vec<RuntimeFlowOpSeed>, RuntimePlanLowerError> {
        let locals = self
            .control
            .scopes
            .get(&frame.owner)
            .cloned()
            .ok_or_else(|| {
                RuntimePlanLowerError::new("scope continuation has no admitted locals")
            })?;
        let carrier = frame.fact.carrier_type();
        let success = self.apply_value_continuation(
            local_seed(frame.fact.value_type(), locals.success.clone()),
            frame.outer.clone(),
        )?;
        let residual = match (frame.fact.residual_type(), &locals.residual) {
            (Some(ty), Some(local)) => Some(local_seed(ty, local.clone())),
            (None, None) => None,
            _ => {
                return Err(RuntimePlanLowerError::new(
                    "scope residual local differs from its carrier",
                ));
            }
        };
        let failure = self.propagate_scope_residual(
            frame.fact.boundary(),
            frame.fact.boundary_type(),
            residual,
        )?;
        Ok(vec![
            RuntimeFlowOpSeed::ExitScopeBind {
                pattern: bind_seed(carrier, locals.carrier.clone()),
                expr: value,
            },
            RuntimeFlowOpSeed::Match {
                scrutinee: local_seed(carrier, locals.carrier),
                arms: vec![
                    RuntimeFlowMatchArmSeed {
                        pattern: normalized_variant_binding_pattern_seed(
                            carrier,
                            0,
                            Some(locals.success),
                        )
                        .map_err(|error| RuntimePlanLowerError::new(error.to_string()))?,
                        guard: None,
                        ops: success,
                    },
                    RuntimeFlowMatchArmSeed {
                        pattern: normalized_variant_binding_pattern_seed(
                            carrier,
                            1,
                            locals.residual,
                        )
                        .map_err(|error| RuntimePlanLowerError::new(error.to_string()))?,
                        guard: None,
                        ops: failure,
                    },
                ],
            },
        ])
    }
}
