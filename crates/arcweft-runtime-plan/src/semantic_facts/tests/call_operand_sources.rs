//! Original authored spread sources authenticate every expanded physical slot.

use super::*;

#[test]
fn scalar_spread_sources_reject_parent_other_child_and_out_of_range_ordinal() {
    let project = project_fixture(
        "spread-physical-source",
        "fn target(left: Unit, right: Unit) -> Unit { () }\nfn root() { target([(), ()]...) }\n",
    );
    let view = project.analysis_view().unwrap();
    let modules = view
        .modules()
        .map(|(_, module)| (module.module_id(), module.as_ref()))
        .collect::<BTreeMap<_, _>>();
    let (owner, call) = modules
        .values()
        .flat_map(|module| module.expressions())
        .find_map(|(owner, expression)| match expression.kind() {
            HirExprKind::Call(call) => Some((owner, call)),
            _ => None,
        })
        .unwrap();
    let module = modules[&owner.module()];
    let parent = call.arguments()[0].value();
    let HirExprKind::BracketSequence(sequence) = module.resolve_expr(parent).unwrap().kind() else {
        panic!("literal spread")
    };
    let [first, second] = sequence.elements() else {
        panic!("two original child expressions")
    };
    let ty = unit_type();
    let types = module
        .expressions()
        .map(|(owner, _)| (owner, &ty))
        .collect();
    let analysis = analyze_identity_fixture(&project);
    let role = analysis
        .call(owner)
        .unwrap()
        .selected_application()
        .unwrap()
        .core()
        .execution()
        .arguments()[0]
        .slots()[0]
        .request_role_identity();
    let validate = |source, slot| {
        let resolved = RuntimeResolvedCall::try_new(
            RuntimeResolvedCallDispatch::Value {
                callee: call.callee().value_expression().unwrap(),
            },
            arcweft_lang_sema::callable::CallableGroupIndex::try_from_usize(0).unwrap(),
            vec![RuntimeResolvedCallOperand::new(
                0,
                RuntimeResolvedCallOperandOrigin::Argument { argument: 0, slot },
                RuntimeResolvedCallOperandSource::Expression(source),
                ty.clone(),
                RuntimeResolvedCallOperandBinding::Positional,
                RuntimeResolvedCallOperandProjection::Scalar,
                None,
                Some(role),
            )],
            None,
            None,
            RuntimeCallResultShape::Value,
        )
        .unwrap();
        super::super::validate_call(&modules, &types, owner, Some(&ty), call, None, &resolved)
            .map_err(Box::new)
    };
    assert_eq!(validate(*first, 0), Ok(()));
    assert_eq!(validate(*second, 1), Ok(()));
    for (source, slot) in [(parent, 0), (*second, 0), (*first, 1), (*first, 2)] {
        assert_eq!(
            validate(source, slot),
            Err(Box::new(
                RuntimeSemanticFactsError::InvalidRuntimeCallDisposition { expression: owner }
            ))
        );
    }
}
