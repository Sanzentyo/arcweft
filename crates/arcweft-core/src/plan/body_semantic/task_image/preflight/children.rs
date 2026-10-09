//! Fixed-table/source-order children, from the actual admitted row owners.
//! Count traversal uses explicit cursors and does not resolve semantic IDs.

use super::{RuntimeTaskPlanImageError, UnsealedRuntimePlanImage};
use crate::plan::RuntimeFunctionSiteBody;
use crate::plan::RuntimeTaskPlanSealLimits;
use crate::plan::body_semantic::callable::preflight::RuntimeCallableChildPreflight;
use crate::task::semantic::TaskSemanticMeter;
use crate::value::{RuntimeExpressionNode, expression_tree::RuntimeExpressionTreeEvent};

struct Children<'a> {
    table: u8,
    ordinal: usize,
    maximum: u32,
    meter: &'a mut TaskSemanticMeter,
}
impl Children<'_> {
    fn count(&mut self, count: usize) -> Result<(), RuntimeTaskPlanImageError> {
        let actual = self.meter.checked_count_sum(count, 0)?;
        if actual > self.maximum as usize {
            self.meter.reject_owner();
            return Err(RuntimeTaskPlanImageError::Children {
                table: self.table,
                ordinal: self.ordinal,
                actual,
                maximum: self.maximum,
            });
        }
        Ok(())
    }
    fn node(
        &mut self,
        auxiliary: &mut RuntimeCallableChildPreflight<'_>,
        root: RuntimeExpressionNode<'_>,
    ) -> Result<(), RuntimeTaskPlanImageError> {
        root.try_visit_owned_events(&mut |event| {
            if let RuntimeExpressionTreeEvent::Enter { node, .. } = event {
                self.count(node.owned_children().count())?;
                let literal = match node {
                    RuntimeExpressionNode::Expression(expression) => {
                        if let crate::value::RuntimeExprKind::Value(value) = expression.kind() {
                            Some(value)
                        } else {
                            None
                        }
                    }
                    RuntimeExpressionNode::Pattern(pattern) => {
                        if let crate::pattern::RuntimePatternKind::Literal(value) = pattern.kind() {
                            Some(value)
                        } else {
                            None
                        }
                    }
                };
                if let RuntimeExpressionNode::Expression(expression) = node {
                    auxiliary.expression(expression, &mut |count| self.count(count))?;
                }
                if let Some(value) = literal {
                    value.try_visit_static_literal_child_counts(&mut |count| self.count(count))?;
                }
            }
            Ok(())
        })
    }
    fn stream(
        &mut self,
        auxiliary: &mut RuntimeCallableChildPreflight<'_>,
        root: &[crate::stream::StreamOp],
    ) -> Result<(), RuntimeTaskPlanImageError> {
        use crate::stream::StreamOp;
        enum Frame<'a> {
            Body(&'a [StreamOp]),
            Ops(std::slice::Iter<'a, StreamOp>),
            Arms(std::slice::Iter<'a, crate::stream::StreamMatchArm>),
        }
        let mut stack = vec![Frame::Body(root)];
        while let Some(frame) = stack.pop() {
            self.meter.status()?;
            match frame {
                Frame::Body(body) => {
                    self.count(body.len())?;
                    stack.push(Frame::Ops(body.iter()));
                }
                Frame::Arms(mut arms) => {
                    if let Some(arm) = arms.next() {
                        stack.push(Frame::Arms(arms));
                        self.node(auxiliary, RuntimeExpressionNode::Pattern(&arm.pattern))?;
                        if let Some(guard) = &arm.guard {
                            self.node(auxiliary, RuntimeExpressionNode::Expression(guard))?;
                        }
                        stack.push(Frame::Body(&arm.ops));
                    }
                }
                Frame::Ops(mut ops) => {
                    if let Some(op) = ops.next() {
                        stack.push(Frame::Ops(ops));
                        match op {
                            StreamOp::Let { pattern, expr } => {
                                self.node(auxiliary, RuntimeExpressionNode::Pattern(pattern))?;
                                self.node(auxiliary, RuntimeExpressionNode::Expression(expr))?;
                            }
                            StreamOp::ForNext {
                                pattern,
                                source,
                                body,
                            } => {
                                self.node(auxiliary, RuntimeExpressionNode::Pattern(pattern))?;
                                self.node(auxiliary, RuntimeExpressionNode::Expression(source))?;
                                stack.push(Frame::Body(body));
                            }
                            StreamOp::Yield { expr } => {
                                self.node(auxiliary, RuntimeExpressionNode::Expression(expr))?;
                            }
                            StreamOp::If {
                                condition,
                                then_ops,
                                else_ops,
                            } => {
                                self.node(auxiliary, RuntimeExpressionNode::Expression(condition))?;
                                stack.push(Frame::Body(else_ops));
                                stack.push(Frame::Body(then_ops));
                            }
                            StreamOp::Match { scrutinee, arms } => {
                                self.node(auxiliary, RuntimeExpressionNode::Expression(scrutinee))?;
                                self.count(arms.len())?;
                                stack.push(Frame::Arms(arms.iter()));
                            }
                            StreamOp::Close { source } => {
                                self.node(auxiliary, RuntimeExpressionNode::Expression(source))?;
                            }
                            StreamOp::Return => {}
                        }
                    }
                }
            }
        }
        Ok(())
    }
    fn flow(
        &mut self,
        auxiliary: &mut RuntimeCallableChildPreflight<'_>,
        ops: &[crate::plan::FlowOp],
    ) -> Result<(), RuntimeTaskPlanImageError> {
        crate::plan::try_visit_ops_events(ops, &mut |event| {
            match event {
                crate::plan::RuntimeFlowTreeEvent::EnterBody { ops, .. } => {
                    self.count(ops.len())?;
                }
                crate::plan::RuntimeFlowTreeEvent::EnterOperation { op, .. } => {
                    self.count(op.owned_bodies().count())?;
                    let mut values = 0;
                    op.try_visit_value_roots(&mut |_, _| {
                        values = self.meter.checked_count_sum(values, 1)?;
                        Ok::<(), RuntimeTaskPlanImageError>(())
                    })?;
                    self.count(values)?;
                    op.try_visit_value_roots(&mut |_, node| self.node(auxiliary, node))?;
                }
                crate::plan::RuntimeFlowTreeEvent::ExitBody
                | crate::plan::RuntimeFlowTreeEvent::ExitOperation => {}
            }
            Ok(())
        })
    }
}

impl UnsealedRuntimePlanImage {
    pub(in crate::plan::body_semantic::task_image) fn preflight_children(
        &self,
        limits: RuntimeTaskPlanSealLimits,
        meter: &mut TaskSemanticMeter,
    ) -> Result<(), RuntimeTaskPlanImageError> {
        let mut auxiliary = RuntimeCallableChildPreflight::new(&self.inventory);
        self.preflight_layout_children(limits, meter)?;
        self.preflight_execution_children(limits, meter, &mut auxiliary)?;
        self.preflight_owned_body_children(limits, meter, &mut auxiliary)
    }

    fn preflight_layout_children(
        &self,
        limits: RuntimeTaskPlanSealLimits,
        meter: &mut TaskSemanticMeter,
    ) -> Result<(), RuntimeTaskPlanImageError> {
        meter.status()?;
        let plan = &self.inventory;
        for (ordinal, ty) in plan.type_table().declarations().enumerate() {
            let mut check = Children {
                table: 0,
                ordinal,
                maximum: limits.max_children_per_row,
                meter,
            };
            ty.scope()
                .try_visit_semantic_child_counts(&mut |count| check.count(count))?;
            ty.projection()
                .try_visit_semantic_metadata_child_counts(&mut |count| check.count(count))?;
            let count = ty.projection().child_count().ok_or_else(|| {
                check.meter.reject_owner();
                crate::task::semantic::TaskSemanticEncodingError::ArithmeticOverflow
            })?;
            check.count(count)?;
            if let Some(codec) = ty.data_codec() {
                codec.try_visit_semantic_child_counts(&mut |count| check.count(count))?;
            }
        }
        // E1 scalar source/storage/type roles do not descend into bodies.
        // A Stream owner contributes its canonical path list when present.
        for (ordinal, row) in plan.local_declarations().declarations().enumerate() {
            if let crate::plan::RuntimeLocalOwner::Stream { ordinal: stream } =
                row.placement().owner()
                && let Some(stream) = plan.stream_plans().get(stream as usize)
            {
                Children {
                    table: 1,
                    ordinal,
                    maximum: limits.max_children_per_row,
                    meter,
                }
                .count(stream.id().path().segments().len())?;
            }
        }
        for (ordinal, row) in plan.nominal_record_domains().domains().enumerate() {
            let mut check = Children {
                table: 2,
                ordinal,
                maximum: limits.max_children_per_row,
                meter,
            };
            check.count(row.fields().len())?;
            if let Some(codec) = row.data_codec() {
                codec.try_visit_semantic_child_counts(&mut |count| check.count(count))?;
            }
        }
        for (ordinal, row) in plan.variant_domains().domains().enumerate() {
            let mut check = Children {
                table: 3,
                ordinal,
                maximum: limits.max_children_per_row,
                meter,
            };
            check.count(row.cases().len())?;
            if let Some(codec) = row.data_codec() {
                codec.try_visit_semantic_child_counts(&mut |count| check.count(count))?;
            }
        }
        Ok(())
    }

    fn preflight_execution_children(
        &self,
        limits: RuntimeTaskPlanSealLimits,
        meter: &mut TaskSemanticMeter,
        auxiliary: &mut RuntimeCallableChildPreflight<'_>,
    ) -> Result<(), RuntimeTaskPlanImageError> {
        let plan = &self.inventory;
        for (ordinal, row) in plan.function_sites().iter().enumerate() {
            let mut check = Children {
                table: 4,
                ordinal,
                maximum: limits.max_children_per_row,
                meter,
            };
            check.count(row.inputs().len())?;
            for input in row.inputs() {
                check.node(auxiliary, RuntimeExpressionNode::Pattern(input.pattern()))?;
            }
            match row.body() {
                RuntimeFunctionSiteBody::Expression(body) => {
                    check.node(auxiliary, RuntimeExpressionNode::Expression(body))?;
                }
                RuntimeFunctionSiteBody::Executable(body) => {
                    check.count(body.effects().len())?;
                    check.flow(auxiliary, body.ops())?;
                }
            }
        }
        for (ordinal, row) in plan.dialogue_content().rows().iter().enumerate() {
            let mut check = Children {
                table: 5,
                ordinal,
                maximum: limits.max_children_per_row,
                meter,
            };
            check.count(row.values().len())?;
            check.count(row.effect_sites().len())?;
            check.count(row.marks().len())?;
            for value in row.values() {
                check.count(value.captures().len())?;
                for expression in value.captures() {
                    check.node(auxiliary, RuntimeExpressionNode::Expression(expression))?;
                }
            }
            for effect in row.effect_sites() {
                check.count(effect.captures().len())?;
                for expression in effect.captures() {
                    check.node(auxiliary, RuntimeExpressionNode::Expression(expression))?;
                }
            }
        }
        for (ordinal, row) in plan.entries().iter().enumerate() {
            let mut check = Children {
                table: 6,
                ordinal,
                maximum: limits.max_children_per_row,
                meter,
            };
            check.count(row.id.path().segments().len())?;
            match &row.target {
                crate::plan::RuntimeEntryTarget::Routes(routes) => {
                    check.count(routes.len())?;
                    for route in routes {
                        check.count(route.path.segments().len())?;
                        check.count(route.bindings.len())?;
                    }
                }
                crate::plan::RuntimeEntryTarget::Flow(_)
                | crate::plan::RuntimeEntryTarget::Controller(_) => {}
            }
        }
        // E7's code references are scalar leaves; its actual ABI/body belongs
        // to E4/E9/E10. E8's schema parameters are explicit ordered children.
        for (ordinal, row) in plan.flow_executables().iter().enumerate() {
            if let Some(schema) = plan.flows.schema(&row.flow) {
                Children {
                    table: 8,
                    ordinal,
                    maximum: limits.max_children_per_row,
                    meter,
                }
                .count(schema.parameters.len())?;
            }
        }
        for (ordinal, row) in plan.flows().iter().enumerate() {
            Children {
                table: 9,
                ordinal,
                maximum: limits.max_children_per_row,
                meter,
            }
            .count(row.params.len())?;
        }
        Ok(())
    }

    fn preflight_owned_body_children(
        &self,
        limits: RuntimeTaskPlanSealLimits,
        meter: &mut TaskSemanticMeter,
        auxiliary: &mut RuntimeCallableChildPreflight<'_>,
    ) -> Result<(), RuntimeTaskPlanImageError> {
        let plan = &self.inventory;
        for (ordinal, row) in plan.pure_helpers().iter().enumerate() {
            let mut check = Children {
                table: 10,
                ordinal,
                maximum: limits.max_children_per_row,
                meter,
            };
            check.count(row.inputs.len())?;
            check.node(auxiliary, RuntimeExpressionNode::Expression(&row.expr))?;
        }
        for (ordinal, row) in plan.trait_methods().iter().enumerate() {
            let mut check = Children {
                table: 11,
                ordinal,
                maximum: limits.max_children_per_row,
                meter,
            };
            check.count(row.inputs.len())?;
            check.node(auxiliary, RuntimeExpressionNode::Expression(&row.body))?;
        }
        for (ordinal, row) in plan.line_task_groups().iter().enumerate() {
            let count = row.semantic_child_count(meter)?;
            Children {
                table: 12,
                ordinal,
                maximum: limits.max_children_per_row,
                meter,
            }
            .count(count)?;
            let mut check = Children {
                table: 12,
                ordinal,
                maximum: limits.max_children_per_row,
                meter,
            };
            check.flow(auxiliary, row.activation_ops())?;
            for node in row.nodes() {
                if let crate::line_task::LineTaskNode::Action(ops) = node {
                    check.flow(auxiliary, ops)?;
                }
            }
            for rule in row.cancel_rules() {
                check.flow(auxiliary, rule.action())?;
            }
            for exit in [
                crate::line_task::ScopeExit::Completed,
                crate::line_task::ScopeExit::Cancelled,
                crate::line_task::ScopeExit::Failed,
            ] {
                check.flow(auxiliary, row.cleanup().actions(exit))?;
            }
        }
        // Stream and task-request child grammars are checked by their owners
        // below; no raw source or alternate executable carrier is consulted.
        for (ordinal, row) in plan.stream_plans().iter().enumerate() {
            let mut check = Children {
                table: 13,
                ordinal,
                maximum: limits.max_children_per_row,
                meter,
            };
            check.count(row.id().path().segments().len())?;
            check.stream(auxiliary, row.ops())?;
        }
        for (ordinal, row) in self.task_plans.iter().enumerate() {
            let mut check = Children {
                table: 14,
                ordinal,
                maximum: limits.max_children_per_row,
                meter,
            };
            row.request_template
                .try_visit_path_lengths(&mut |count| check.count(count))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
