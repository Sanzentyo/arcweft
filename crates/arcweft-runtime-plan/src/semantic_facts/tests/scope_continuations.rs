use super::super::{
    RuntimeScopeContinuation, RuntimeScopeFact, RuntimeScopeOrigin, RuntimeScopeOwner,
    RuntimeTryBoundaryOwner, RuntimeTryCarrierFact, RuntimeTryFact,
};
use super::*;
use arcweft_core::scope::RuntimeScopeIdentity;

#[test]
fn scope_origins_reject_another_owner_family_and_generation() {
    let source = "fn root() -> Unit {\nlet first = scope first { () };\nlet second = scope second { () };\n()\n}\nflow scopes {\nscope { let inner = () }\nscope { let other = () }\nreturn ()\n}\n";
    let project = project_fixture("scope-origin-admission", source);
    let view = project.analysis_view().unwrap();
    let analysis = analyze_identity_fixture(&project);
    let scopes = |project: &HirProject| {
        let analysis = analyze_identity_fixture(project);
        let mut input = complete_type_input(project);
        for (_, module) in project.analysis_view().unwrap().modules() {
            for (owner, expression) in module.expressions() {
                if let HirExprKind::NamedBlock(block) = expression.kind() {
                    let arcweft_lang_hir::expr::HirNamedBlockName::Resolved(name) = block.name()
                    else {
                        panic!("named fixture scope");
                    };
                    input.push_expression_scope(
                        owner,
                        RuntimeScopeFact::new(
                            RuntimeScopeOrigin::Expression(
                                analysis.expression_origin(owner).unwrap(),
                            ),
                            RuntimeScopeIdentity::Named(
                                arcweft_id::DeclarationName::try_new(name.as_str()).unwrap(),
                            ),
                            None,
                        ),
                    );
                }
            }
            for (owner, statement) in module.statements() {
                if matches!(statement.kind(), HirStmtKind::Scope(_)) {
                    input.push_statement_scope(
                        owner,
                        RuntimeScopeFact::new(
                            RuntimeScopeOrigin::Statement(
                                analysis.statement_origin(owner).unwrap(),
                            ),
                            RuntimeScopeIdentity::Anonymous,
                            None,
                        ),
                    );
                }
            }
        }
        input
    };
    let input = scopes(&project);
    assert_eq!(input.expression_scopes.len(), 2);
    assert_eq!(input.statement_scopes.len(), 2);
    let expression = input.expression_scopes[0].0;
    let statement = input.statement_scopes[0].0;
    let expected = analysis.statement_origin(statement).unwrap();
    let authority = arcweft_lang_sema::final_analysis::CheckedLocalUseAuthority::Global(
        Arc::clone(analysis.checked_local_uses()),
    );
    assert!(expected.validate_authority(&authority, statement));
    let facts = runtime_facts(&project, input).unwrap();
    let facts = super::super::RuntimeExecutableSemanticFactView::Global(&facts);
    assert_eq!(
        facts
            .statement_scope(statement)
            .unwrap()
            .origin()
            .coordinate(),
        expected.coordinate().path()
    );
    let statements = scopes(&project).statement_scopes;
    assert_ne!(
        statements[0].1.origin().coordinate(),
        statements[1].1.origin().coordinate(),
        "anonymous scopes have distinct accepted coordinates"
    );

    let mut wrong_expression = scopes(&project);
    wrong_expression.expression_scopes[0].1 = RuntimeScopeFact::new(
        wrong_expression.expression_scopes[1].1.origin().clone(),
        wrong_expression.expression_scopes[0].1.identity().clone(),
        None,
    );
    assert_eq!(
        runtime_facts(&project, wrong_expression).unwrap_err(),
        RuntimeSemanticFactsError::InvalidScopeOrigin {
            owner: RuntimeScopeOwner::Expression(expression)
        }
    );
    let mut wrong_statement = scopes(&project);
    wrong_statement.statement_scopes[0].1 = RuntimeScopeFact::new(
        wrong_statement.statement_scopes[1].1.origin().clone(),
        RuntimeScopeIdentity::Anonymous,
        None,
    );
    assert_eq!(
        runtime_facts(&project, wrong_statement).unwrap_err(),
        RuntimeSemanticFactsError::InvalidScopeOrigin {
            owner: RuntimeScopeOwner::Statement(statement)
        }
    );
    let mut wrong_family = scopes(&project);
    wrong_family.statement_scopes[0].1 = RuntimeScopeFact::new(
        wrong_family.expression_scopes[0].1.origin().clone(),
        RuntimeScopeIdentity::Anonymous,
        None,
    );
    assert_eq!(
        runtime_facts(&project, wrong_family).unwrap_err(),
        RuntimeSemanticFactsError::InvalidScopeOrigin {
            owner: RuntimeScopeOwner::Statement(statement)
        }
    );

    let foreign = project_fixture("scope-origin-admission", source);
    let foreign_analysis = analyze_identity_fixture(&foreign);
    let foreign_authority = arcweft_lang_sema::final_analysis::CheckedLocalUseAuthority::Global(
        Arc::clone(foreign_analysis.checked_local_uses()),
    );
    assert!(!expected.validate_owner(foreign.analysis_view().unwrap(), statement));
    assert!(!expected.validate_authority(&foreign_authority, statement));
    assert!(!expected.validate_authority(&authority, statements[1].0));
    assert!(expected.validate_owner(view, statement));
    let mut wrong_generation = scopes(&project);
    wrong_generation.statement_scopes[0].1 = scopes(&foreign).statement_scopes[0].1.clone();
    assert_eq!(
        runtime_facts(&project, wrong_generation).unwrap_err(),
        RuntimeSemanticFactsError::InvalidScopeOrigin {
            owner: RuntimeScopeOwner::Statement(statement)
        }
    );
}

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
            RuntimeScopeOrigin::Expression(fixture_expression_origin(&project, scope_owner)),
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
        validate(
            &RuntimeScopeFact::new(
                RuntimeScopeOrigin::Expression(fixture_expression_origin(&project, scope_owner)),
                identity.clone(),
                None
            ),
            &unit_type()
        ),
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
