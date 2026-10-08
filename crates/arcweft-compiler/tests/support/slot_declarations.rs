pub(super) fn function_body_declarations(
    plan: &arcweft_core::plan::RuntimePlan,
) -> std::collections::BTreeMap<
    arcweft_core::runtime_id::RuntimeLocalDeclarationId,
    std::collections::BTreeSet<usize>,
> {
    use arcweft_core::plan::RuntimeFunctionSiteBody;
    use arcweft_core::value::{RuntimeExprKind, RuntimeExpressionNode as Node};
    let mut owners = std::collections::BTreeMap::<_, std::collections::BTreeSet<_>>::new();
    for (owner, site) in plan.function_sites().iter().enumerate() {
        for input in site.inputs() {
            owners.entry(input.input_local()).or_default().insert(owner);
        }
        let mut visit = |root: Node<'_>| {
            let mut pending = vec![root];
            while let Some(node) = pending.pop() {
                match node {
                    Node::Pattern(pattern) => {
                        for declaration in pattern.binding_declarations() {
                            owners.entry(declaration.local()).or_default().insert(owner);
                        }
                    }
                    Node::Expression(expression) => {
                        if let RuntimeExprKind::Let { binding, .. } = expression.kind() {
                            owners.entry(*binding).or_default().insert(owner);
                        }
                    }
                }
                pending.extend(node.owned_children().map(|(_, child)| child));
            }
        };
        for input in site.inputs() {
            visit(Node::Pattern(input.pattern()));
        }
        match site.body() {
            RuntimeFunctionSiteBody::Expression(body) => visit(Node::Expression(body)),
            RuntimeFunctionSiteBody::Executable(body) => {
                let mut pending = vec![body.ops()];
                while let Some(operations) = pending.pop() {
                    for operation in operations {
                        if let arcweft_core::plan::FlowOp::ProjectCall { site } = operation {
                            let call = plan.project_call_sites().get(*site).unwrap();
                            visit(Node::Expression(call.plan().callee()));
                            for operand in call.plan().operands() {
                                visit(Node::Expression(operand.value()));
                            }
                            visit(Node::Pattern(call.result()));
                        }
                        operation
                            .try_visit_value_roots(&mut |_, root| {
                                visit(root);
                                Ok::<_, std::convert::Infallible>(())
                            })
                            .unwrap();
                        pending.extend(operation.owned_bodies().map(|(_, body)| body));
                    }
                }
            }
        }
    }
    owners
}
