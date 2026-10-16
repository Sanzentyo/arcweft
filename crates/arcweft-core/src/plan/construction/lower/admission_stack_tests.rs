use crate::pattern::RuntimeSemanticTypeId;
use crate::plan::{
    RuntimeExprSeed, RuntimeExprSeedKind, RuntimeFunctionDefinitionIdentity,
    RuntimeFunctionSemanticRole, RuntimeFunctionSiteBody, RuntimePlanBuilder,
    RuntimePlanTypeProjection, RuntimePlanTypeSeed,
};
use crate::value::{RuntimeUnaryOp, RuntimeValue};

#[test]
fn recursive_expression_variants_admit_complete_nested_bodies_on_the_default_stack() {
    let mut builder = RuntimePlanBuilder::new();
    let ty = RuntimeSemanticTypeId::from_bytes([31; 32]);
    builder
        .admit_type_batch(
            [RuntimePlanTypeSeed::new(
                ty,
                RuntimePlanTypeProjection::Bool,
            )],
            [],
        )
        .unwrap();
    let literal = || RuntimeExprSeed::new(ty, RuntimeExprSeedKind::Value(RuntimeValue::Bool(true)));
    let mut unary = literal();
    let mut branch = literal();
    for _ in 0..64 {
        unary = RuntimeExprSeed::new(
            ty,
            RuntimeExprSeedKind::Unary {
                op: RuntimeUnaryOp::Not,
                expr: Box::new(unary),
            },
        );
        branch = RuntimeExprSeed::new(
            ty,
            RuntimeExprSeedKind::If {
                condition: Box::new(literal()),
                then_expr: Box::new(branch),
                else_expr: Box::new(literal()),
            },
        );
    }
    for (definition, body) in [(1, unary), (2, branch)] {
        builder
            .push_function_site_seed(
                RuntimeFunctionDefinitionIdentity::from_accepted_identity([definition; 32]),
                RuntimeFunctionSemanticRole::Ordinary,
                [],
                body,
            )
            .unwrap();
    }
    let plan = builder.finish().unwrap();
    let counts = plan
        .function_sites()
        .iter()
        .map(|site| {
            let RuntimeFunctionSiteBody::Expression(body) = site.body() else {
                panic!("original expression body")
            };
            let mut count = 0;
            body.try_visit_owned_tree(&mut |_, _| {
                count += 1;
                Ok::<(), ()>(())
            })
            .unwrap();
            count
        })
        .collect::<Vec<_>>();
    assert_eq!(counts, [65, 193]);
}
