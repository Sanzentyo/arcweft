//! Value-producing control stays in its caller's frame and joins one result.

use super::*;

impl AwbcExprLowerer<'_, '_, '_> {
    pub(super) fn lower_value_control_expr(&mut self, expression: &RuntimeExpr) -> AwbcRegisterId {
        let ty = admitted_plan_type(self.inventory, self.plan, expression.ty());
        let result = self.frame.temp(ty);
        let mut joins = Vec::new();
        match expression.kind() {
            RuntimeExprKind::StandardMap {
                family,
                order,
                mapping,
                source,
            } => {
                let scope = self.frame.enter_scope();
                self.inventory
                    .push_instruction(AwbcInstruction::EnterScope { scope });
                let value = lower_standard_map_value(
                    self.inventory,
                    self.frame,
                    self.plan,
                    StandardMapValueExprInput {
                        family: *family,
                        order: *order,
                        mapping,
                        source,
                        result: expression,
                        path: &self.path,
                    },
                );
                self.inventory.push_instruction(AwbcInstruction::Move {
                    dst: result,
                    src: value,
                });
                self.inventory
                    .push_instruction(AwbcInstruction::ExitScope { scope });
                self.frame.exit_scope();
                return result;
            }
            RuntimeExprKind::If {
                condition,
                then_expr,
                else_expr,
            } => {
                let condition = self.lower(condition);
                let branch = self.branch_value(condition);
                joins.push(self.finish_value_arm(then_expr, result, None));
                let otherwise = self.inventory.reopen_function_block();
                patch_branch_else_block(self.inventory, branch, otherwise);
                joins.push(self.finish_value_arm(else_expr, result, None));
            }
            RuntimeExprKind::IfLet {
                pattern,
                expr,
                guard,
                then_expr,
                else_expr,
            } => {
                let value = self.lower(expr);
                let pattern = lower_branch_pattern(self.inventory, self.plan, self.frame, pattern);
                let matched = self.test_value_pattern(pattern, value);
                let branch = self.branch_value(matched);
                let guard_false = self.finish_pattern_value_arm(
                    pattern,
                    value,
                    guard.as_deref(),
                    then_expr,
                    result,
                    &mut joins,
                );
                let otherwise = self.inventory.reopen_function_block();
                patch_branch_else_block(self.inventory, branch, otherwise);
                if let Some(guard_false) = guard_false {
                    patch_jump_target(self.inventory, guard_false, otherwise);
                }
                joins.push(self.finish_value_arm(else_expr, result, None));
            }
            RuntimeExprKind::Match { scrutinee, arms } => {
                let value = self.lower(scrutinee);
                for arm in arms {
                    let pattern =
                        lower_branch_pattern(self.inventory, self.plan, self.frame, arm.pattern());
                    let matched = self.test_value_pattern(pattern, value);
                    let branch = self.branch_value(matched);
                    let guard_false = self.finish_pattern_value_arm(
                        pattern,
                        value,
                        arm.guard(),
                        arm.value(),
                        result,
                        &mut joins,
                    );
                    let next = self.inventory.reopen_function_block();
                    patch_branch_else_block(self.inventory, branch, next);
                    if let Some(guard_false) = guard_false {
                        patch_jump_target(self.inventory, guard_false, next);
                    }
                }
                let message = self.inventory.intern_string("match pattern did not match");
                self.inventory.close_function_block(
                    AwbcTerminator::Trap {
                        code: AwbcTrapCode::PatternMismatch,
                        message: Some(message),
                    },
                    AwbcSafePointKind::None,
                );
            }
            _ => unreachable!("value control has an exhaustive admitted family"),
        }
        let join = self.inventory.reopen_function_block();
        for jump in joins {
            patch_jump_target(self.inventory, jump, join);
        }
        result
    }

    fn test_value_pattern(
        &mut self,
        pattern: AwbcPatternId,
        value: AwbcRegisterId,
    ) -> AwbcRegisterId {
        let matched = self.frame.temp(self.inventory.bool_ty());
        self.inventory
            .push_instruction(AwbcInstruction::TestPattern {
                dst: matched,
                pattern,
                value,
            });
        matched
    }

    fn branch_value(&mut self, condition: AwbcRegisterId) -> AwbcBlockId {
        let next = AwbcBlockId(table_index(self.inventory.program.blocks.len() + 1));
        self.inventory.close_function_block(
            AwbcTerminator::Branch {
                condition,
                then_block: next,
                else_block: next,
            },
            AwbcSafePointKind::None,
        )
    }

    fn finish_value_arm(
        &mut self,
        expression: &RuntimeExpr,
        result: AwbcRegisterId,
        exit: Option<AwbcScopeId>,
    ) -> AwbcBlockId {
        let value = self.lower(expression);
        self.inventory.push_instruction(AwbcInstruction::Move {
            dst: result,
            src: value,
        });
        if let Some(scope) = exit {
            self.inventory
                .push_instruction(AwbcInstruction::ExitScope { scope });
            self.frame.exit_scope();
        }
        self.inventory.close_function_block(
            AwbcTerminator::Jump {
                target: AwbcBlockId::default(),
            },
            AwbcSafePointKind::None,
        )
    }

    fn finish_pattern_value_arm(
        &mut self,
        pattern: AwbcPatternId,
        value: AwbcRegisterId,
        guard: Option<&RuntimeExpr>,
        expression: &RuntimeExpr,
        result: AwbcRegisterId,
        joins: &mut Vec<AwbcBlockId>,
    ) -> Option<AwbcBlockId> {
        let outer = self.frame.scope_checkpoint();
        if let Some(guard) = guard {
            let scope = enter_guard_pattern_scope(
                self.inventory,
                self.plan,
                self.frame,
                pattern,
                value,
                guard,
            );
            let guard = self.lower(guard);
            let branch = self.branch_value(guard);
            self.inventory
                .push_instruction(AwbcInstruction::ExitScope { scope });
            self.frame.exit_scope();
            let body_scope = enter_pattern_scope(self.inventory, self.frame, pattern, value);
            joins.push(self.finish_value_arm(expression, result, Some(body_scope)));
            let otherwise = self.inventory.reopen_function_block();
            patch_branch_else_block(self.inventory, branch, otherwise);
            self.inventory
                .push_instruction(AwbcInstruction::ExitScope { scope });
            self.frame.restore_scopes_after_branch(outer);
            Some(self.inventory.close_function_block(
                AwbcTerminator::Jump {
                    target: AwbcBlockId::default(),
                },
                AwbcSafePointKind::None,
            ))
        } else {
            let scope = enter_pattern_scope(self.inventory, self.frame, pattern, value);
            joins.push(self.finish_value_arm(expression, result, Some(scope)));
            self.frame.restore_scopes_after_branch(outer);
            None
        }
    }
}
