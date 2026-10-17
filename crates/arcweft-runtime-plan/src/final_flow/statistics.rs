//! Reports the completed typed program, including its compiled Map consumers.

use super::RuntimePlanLowerStats;
use arcweft_core::plan::{
    FlowOp, RuntimeFlow, RuntimeFunctionSiteBody, RuntimePlan, RuntimePlanTypeDeclaration,
    RuntimePlanTypeProjection, RuntimeStandardMapCompletion,
};
use arcweft_core::runtime_id::RuntimePlanTypeId;
use arcweft_core::value::{RuntimeExpr, RuntimeExprKind, RuntimeExpressionNode};

impl RuntimePlanLowerStats {
    pub(super) fn from_plan(plan: &RuntimePlan) -> Self {
        let mut stats = Self {
            pure_function_candidates: plan.pure_function_candidate_count(),
            ..Self::default()
        };
        let flow_sites = plan
            .flows()
            .iter()
            .map(RuntimeFlow::function_site)
            .collect::<std::collections::BTreeSet<_>>();
        // Flow is an invocation index into these same immutable function rows.
        // Visit the owning row once, irrespective of its entry projections.
        for (site, function) in plan.function_sites().iter_with_ids() {
            let mut body_stats = Self::default();
            match function.body() {
                RuntimeFunctionSiteBody::Expression(expression) => {
                    body_stats.record_expression(expression);
                }
                RuntimeFunctionSiteBody::Executable(body) => {
                    body_stats.record_operations(plan, body.ops());
                }
            }
            stats.sequence_map_sum_fusions += body_stats.sequence_map_sum_fusions;
            stats.optimized_op_slices += body_stats.optimized_op_slices;
            if flow_sites.contains(&site) {
                stats.optimized_flows += usize::from(body_stats.sequence_map_sum_fusions != 0);
                stats.pure_call_exprs += body_stats.pure_call_exprs;
            }
        }
        stats
    }

    fn record_operations(&mut self, plan: &RuntimePlan, operations: &[FlowOp]) {
        let mut pending = vec![operations];
        while let Some(operations) = pending.pop() {
            let before = self.sequence_map_sum_fusions;
            for operation in operations {
                match operation {
                    FlowOp::StandardMap(control) => {
                        if matches!(
                            control.completion(),
                            RuntimeStandardMapCompletion::Sum { .. }
                        ) {
                            self.sequence_map_sum_fusions += 1;
                        }
                        if let Some(mapping) = plan.local_declarations().get(control.mapping()) {
                            self.pure_call_exprs +=
                                usize::from(pure_function_type(plan, mapping.ty()));
                        }
                        // Mapping is one logical invocation site. Its Array
                        // expansion can contain many physical applications.
                        if let Some(continuation) = control.consuming_body() {
                            pending.push(continuation);
                        }
                        continue;
                    }
                    FlowOp::ProjectCall { site } => {
                        if let Some(site) = plan.project_call_sites().get(*site) {
                            self.pure_call_exprs +=
                                usize::from(pure_function_type(plan, site.plan().callee().ty()));
                        }
                    }
                    FlowOp::ApplyGroup { callee, .. } => {
                        self.pure_call_exprs += usize::from(pure_function_type(plan, callee.ty()));
                    }
                    _ => {}
                }
                let result = operation.try_visit_value_roots(&mut |_, node| {
                    if let RuntimeExpressionNode::Expression(expression) = node {
                        self.record_expression(expression);
                    }
                    Ok::<_, std::convert::Infallible>(())
                });
                match result {
                    Ok(()) => {}
                    Err(never) => match never {},
                }
                pending.extend(operation.owned_bodies().map(|(_, body)| body));
            }
            self.optimized_op_slices += usize::from(self.sequence_map_sum_fusions > before);
        }
    }

    fn record_expression(&mut self, expression: &RuntimeExpr) {
        let result = expression.try_visit_owned_tree(&mut |_, node| {
            if let RuntimeExpressionNode::Expression(expression) = node
                && matches!(expression.kind(), RuntimeExprKind::PureCall { .. })
            {
                self.pure_call_exprs += 1;
            }
            Ok::<_, std::convert::Infallible>(())
        });
        match result {
            Ok(()) => {}
            Err(never) => match never {},
        }
    }
}

fn pure_function_type(plan: &RuntimePlan, ty: RuntimePlanTypeId) -> bool {
    matches!(plan.type_table().get(ty).map(RuntimePlanTypeDeclaration::projection),
        Some(RuntimePlanTypeProjection::Function { contract, .. })
            if contract.invocation().is_closed() && contract.invocation().constant_effects().is_empty())
}
