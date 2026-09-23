use super::super::{
    RuntimeScopeContinuation, RuntimeScopeFact, RuntimeScopeOwner, RuntimeTryBoundaryOwner,
    RuntimeTryCarrierFact, RuntimeTryFact,
};
use super::*;
use arcweft_core::scope::RuntimeScopeIdentity;

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one adversarial admission test shares the exact accepted HIR owners across all forged facts"
)]
fn scope_continuation_admission_rejects_missing_foreign_and_mistyped_exits() {
    let project = project_fixture(
        "scope-continuation-inventory",
        r"
fn root(input: Option<Unit>) -> Option<Unit> {
    option {
        let first = scope first { try input }
        let second = scope second { try input }
        first
    }
}
",
    );
    let view = project.analysis_view().unwrap();
    let modules = view
        .modules()
        .map(|(_, module)| (module.module_id(), module.as_ref()))
        .collect::<BTreeMap<_, _>>();
    let module = *modules.values().next().unwrap();
    let scope_owner = module.expressions().find_map(|(owner, expression)| {
        match expression.kind() {
            HirExprKind::NamedBlock(block) if matches!(block.name(), arcweft_lang_hir::expr::HirNamedBlockName::Resolved(name) if name.as_str() == "first") => Some(owner),
            _ => None,
        }
    }).unwrap();
    let boundary_owner = module
        .expressions()
        .find_map(|(owner, expression)| {
            matches!(expression.kind(), HirExprKind::ComputationBlock(_)).then_some(owner)
        })
        .unwrap();
    let carrier = option_unit_type();
    let boundary = RuntimeTryBoundaryOwner::CarrierBlock(boundary_owner);
    let tries = module
        .expressions()
        .filter_map(|(owner, expression)| {
            let HirExprKind::Try(tried) = expression.kind() else {
                return None;
            };
            Some((
                owner,
                RuntimeTryFact::new(
                    tried.operand(),
                    carrier.clone(),
                    RuntimeTryCarrierFact::Option {
                        success: unit_type(),
                    },
                    boundary,
                    carrier.clone(),
                ),
            ))
        })
        .collect::<Vec<_>>();
    assert_eq!(tries.len(), 2);
    let own_exit = tries
        .iter()
        .find_map(|(owner, _)| {
            let scope = module.resolve_expr(*owner).unwrap().scope();
            (*module.resolve_scope(scope).unwrap().owner()
                == arcweft_lang_hir::scope::HirScopeOwner::Expr(scope_owner))
            .then_some(*owner)
        })
        .unwrap();
    let sibling_exit = tries
        .iter()
        .find_map(|(owner, _)| (*owner != own_exit).then_some(*owner))
        .unwrap();
    let identity =
        RuntimeScopeIdentity::Named(arcweft_id::DeclarationName::try_new("first").unwrap());
    let make_fact = |exit, boundary_type| {
        RuntimeScopeFact::new(
            identity.clone(),
            Some(
                RuntimeScopeContinuation::try_new(
                    carrier.clone(),
                    boundary,
                    boundary_type,
                    Box::new([exit]),
                )
                .unwrap(),
            ),
        )
    };
    let validate = |fact: &RuntimeScopeFact, ty: &super::super::RuntimeNormalizedType| {
        super::super::lexical_scope::validate_continuation(
            &modules,
            RuntimeScopeOwner::Expression(scope_owner),
            fact,
            Some(ty),
            tries.iter().map(|(owner, fact)| (*owner, fact)),
        )
    };
    let valid = make_fact(own_exit, carrier.clone());
    assert!(validate(&valid, &unit_type()).is_ok());
    let invalid = RuntimeSemanticFactsError::InvalidScopeContinuation {
        owner: RuntimeScopeOwner::Expression(scope_owner),
    };
    assert_eq!(
        validate(&RuntimeScopeFact::new(identity.clone(), None), &unit_type()),
        Err(invalid.clone())
    );
    assert_eq!(
        validate(&make_fact(sibling_exit, carrier.clone()), &unit_type()),
        Err(invalid.clone())
    );
    let boolean = normalized_type(0x77, RuntimeTypeShape::Bool);
    assert_eq!(validate(&valid, &boolean), Err(invalid.clone()));
    let wrong_boundary = normalized_type(
        0x78,
        RuntimeTypeShape::Option {
            item: Box::new(boolean.clone()),
            some_payload: Box::new(tuple_payload(0x79, boolean)),
        },
    );
    assert_eq!(
        validate(&make_fact(own_exit, wrong_boundary), &unit_type()),
        Err(invalid)
    );
}
