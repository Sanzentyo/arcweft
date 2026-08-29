use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, atomic::AtomicBool},
};

use arcweft_lang_hir::{
    dialogue_application::HirPostfixBracketCandidates, expr::HirExprKind,
    project::HirLocalValueOrigin,
};
use arcweft_lang_syntax::ast::module_path::CanonicalModulePath;

use super::expression_error::AnalyzerExpressionError;
use super::state::CandidateFactTransactionAction;
use super::*;

fn assert_outer_expression_failure_rolls_back_call_publication(
    fixture: &crate::final_analysis::tests::Fixture,
) {
    let module = fixture
        .project
        .executable_view()
        .expect("executable HIR")
        .module(&CanonicalModulePath::crate_root())
        .expect("root module");
    let (call_owner, call) = module
        .expressions()
        .find_map(|(owner, expression)| match expression.kind() {
            HirExprKind::Call(call) => Some((owner, call)),
            _ => None,
        })
        .expect("nested call expression");
    let argument_owners = call
        .arguments()
        .iter()
        .map(HirCallArgument::value)
        .collect::<Vec<_>>();
    let local_owner = module
        .locals()
        .next()
        .map(|(owner, _)| owner)
        .expect("local");
    let pattern_owner = module
        .patterns()
        .next()
        .map(|(owner, _)| owner)
        .expect("pattern");
    let mut parents = BTreeMap::new();
    for (owner, expression) in module.expressions() {
        for child in expression.kind().direct_expression_children() {
            parents.insert(child, owner);
        }
    }
    let mut outer_owner = call_owner;
    while let Some(parent) = parents.get(&outer_owner).copied() {
        outer_owner = parent;
    }

    let cancellation = AtomicBool::new(false);
    let mut analyzer = Analyzer::new(
        fixture.project.executable_view().expect("executable HIR"),
        &fixture.symbols,
        FinalSemanticCatalogs::production(&fixture.registered),
        FinalSemanticAnalysisControl::new(&cancellation),
    )
    .expect("analyzer");
    let first_error = analyzer
        .analyze_staged(NoPreparedStatementMutation)
        .expect_err("the outer expression fails after the inner call");
    assert!(
        analyzer
            .facts
            .physical_candidate_argument_evaluations()
            .values()
            .flatten()
            .all(|evaluation| evaluation.call_expression() != call_owner),
        "candidate-neutral recovery must not publish a synthetic physical candidate trace; error: {first_error:?}"
    );
    assert!(!analyzer.facts.calls().contains_key(&call_owner));
    assert!(!analyzer.facts.expressions().contains_key(&call_owner));
    assert!(
        !analyzer
            .facts
            .prepared_calls()
            .expect("prepared call graph")
            .selected_nodes()
            .any(|node| node.site() == crate::callable::CheckedCallSite::HirCall(call_owner))
    );
    assert_eq!(
        analyzer
            .facts
            .prepared_calls()
            .expect("prepared call graph")
            .selected_nodes()
            .count(),
        0,
        "rolled-back call publication must not retain prepared graph nodes"
    );
    for argument in &argument_owners {
        assert!(
            !analyzer.facts.expressions().contains_key(argument),
            "retained argument facts must roll back with call publication"
        );
    }

    let baseline_iteration = CheckedIteration::Builtin {
        family: CheckedIteratorFamily::Seq,
        item: TypeKind::I64,
    };
    analyzer
        .facts
        .set_local_type(local_owner, TypeKind::I64)
        .expect("baseline local");
    analyzer
        .facts
        .set_pattern_type(pattern_owner, TypeKind::U64)
        .expect("baseline pattern");
    analyzer
        .facts
        .set_iteration_fact(call_owner, baseline_iteration.clone())
        .expect("baseline iteration");
    let retry_error = analyzer
        .run_candidate_fact_transaction(|this, _authority, _transaction| {
            this.facts
                .set_local_type(local_owner, TypeKind::Bool)
                .map_err(AnalyzerExpressionError::fact)?;
            this.facts
                .set_pattern_type(pattern_owner, TypeKind::I16)
                .map_err(AnalyzerExpressionError::fact)?;
            this.facts
                .set_iteration_fact(
                    call_owner,
                    CheckedIteration::Builtin {
                        family: CheckedIteratorFamily::Array,
                        item: TypeKind::Bool,
                    },
                )
                .map_err(AnalyzerExpressionError::fact)?;
            this.check_expression_published(outer_owner, None)
                .map(|_| CandidateFactTransactionAction::Commit(()))
                .map_err(AnalyzerExpressionError::fatal)
        })
        .map(|_| ())
        .map_err(|error| error.into_public(outer_owner))
        .expect_err("retry reaches the authored outer failure again");
    assert!(!matches!(
        first_error,
        crate::final_analysis::FinalSemanticProjectError::Semantic(error)
            if matches!(error.as_ref(), FinalSemanticAnalysisError::ExpressionCycle { .. })
    ));
    assert!(!matches!(
        retry_error,
        FinalSemanticAnalysisError::ExpressionCycle { .. }
    ));
    assert!(!analyzer.facts.calls().contains_key(&call_owner));
    assert!(!analyzer.facts.expressions().contains_key(&call_owner));
    assert!(
        !analyzer
            .facts
            .prepared_calls()
            .expect("prepared call graph")
            .selected_nodes()
            .any(|node| node.site() == crate::callable::CheckedCallSite::HirCall(call_owner))
    );
    assert_eq!(
        analyzer
            .facts
            .prepared_calls()
            .expect("prepared call graph")
            .selected_nodes()
            .count(),
        0,
        "retry rollback must not retain prepared graph nodes"
    );
    assert_eq!(
        analyzer.facts.locals().get(&local_owner),
        Some(&TypeKind::I64)
    );
    assert_eq!(
        analyzer.facts.patterns().get(&pattern_owner),
        Some(&TypeKind::U64)
    );
    assert_eq!(
        analyzer.facts.iteration_facts().get(&call_owner),
        Some(&baseline_iteration)
    );
}

#[test]
fn outer_failure_rolls_back_inner_selected_call_publication() {
    let fixture = crate::final_analysis::tests::fixture(
        concat!(
            "fn consume(value: i64) -> i64 { value }\n",
            "fn caller(seed: i64) { (consume(1), missing); }\n",
        ),
        None,
    );
    assert_outer_expression_failure_rolls_back_call_publication(&fixture);
}

#[test]
fn outer_failure_rolls_back_inner_ambiguous_call_publication() {
    let fixture = crate::final_analysis::tests::environment_overload_fixture(
        "fn caller(seed: i64) { (choose(1), missing); }\n",
    );
    assert_outer_expression_failure_rolls_back_call_publication(&fixture);
}

#[test]
fn outer_failure_rolls_back_inner_rejected_call_publication() {
    let fixture = crate::final_analysis::tests::environment_overload_fixture(
        "fn caller(seed: i64) { (choose(\"no\"), missing); }\n",
    );
    assert_outer_expression_failure_rolls_back_call_publication(&fixture);
}

#[test]
fn candidate_call_keeps_nested_ordinary_call_on_candidate_context() {
    let fixture = crate::final_analysis::tests::fixture(
        concat!(
            "fn inner(value: i64) -> i64 { value }\n",
            "fn consume(value: i64) -> i64 { value }\n",
            "fn caller(seed: i64) { consume(inner(seed)); }\n",
        ),
        None,
    );
    let module = fixture
        .project
        .executable_view()
        .expect("executable HIR")
        .module(&CanonicalModulePath::crate_root())
        .expect("root module");
    let (outer_owner, inner_owner) = module
        .expressions()
        .find_map(|(owner, expression)| {
            if !matches!(expression.kind(), HirExprKind::Call(_)) {
                return None;
            }
            expression
                .kind()
                .direct_expression_children()
                .into_iter()
                .find(|child| {
                    module
                        .resolve_expr(*child)
                        .is_ok_and(|expression| matches!(expression.kind(), HirExprKind::Call(_)))
                })
                .map(|child| (owner, child))
        })
        .expect("nested ordinary calls");
    let cancellation = AtomicBool::new(false);
    let mut analyzer = Analyzer::new(
        fixture.project.executable_view().expect("executable HIR"),
        &fixture.symbols,
        FinalSemanticCatalogs::production(&fixture.registered),
        FinalSemanticAnalysisControl::new(&cancellation),
    )
    .expect("analyzer");
    analyzer.resolve_all_types().expect("types");
    analyzer.seed_local_types().expect("locals");
    let staged = analyzer
        .stage_checked_callables()
        .expect("checked callables");
    for (owner, fact) in &staged.effect_expressions {
        analyzer
            .facts
            .publish_new_expression(*owner, fact.clone())
            .expect("effect expression");
    }
    analyzer.staged_callables = Some(staged);
    analyzer.infer_statement_bindings().expect("bindings");

    analyzer
        .facts
        .publish_new_expression(
            inner_owner,
            CheckedExpression::new(
                TypeKind::String,
                CheckedTypeSelection::Inferred,
                EffectSet::new(),
                CheckedExpressionResolution::Structural,
            ),
        )
        .expect("unstable published cache");

    let outcome = analyzer
        .run_candidate_fact_transaction(|this, authority, _transaction| {
            let context =
                AnalyzerExpressionContext::candidate(authority, Rc::clone(&this.call_frames));
            let checked = this.evaluate_expression(&context, outer_owner, None)?;
            drop(context);
            Ok::<CandidateFactTransactionAction<PreparedExpressionFact>, AnalyzerExpressionError>(
                CandidateFactTransactionAction::Commit(checked),
            )
        })
        .expect("candidate call transaction");
    let checked = outcome.into_committed().expect("candidate call result");
    assert_eq!(checked.ty(), &TypeKind::I64);
    let selected_sites = analyzer
        .facts
        .prepared_calls()
        .expect("prepared call graph")
        .selected_nodes()
        .map(|node| node.site())
        .collect::<BTreeSet<_>>();
    assert!(selected_sites.contains(&crate::callable::CheckedCallSite::HirCall(outer_owner,)));
    assert!(selected_sites.contains(&crate::callable::CheckedCallSite::HirCall(inner_owner,)));
}

#[test]
fn selected_call_publishes_one_prepared_graph_node() {
    let fixture = crate::final_analysis::tests::fixture(
        "fn consume(value: i64) -> i64 { value }\nfn caller(value: i64) { consume(value); }\n",
        None,
    );
    let cancellation = AtomicBool::new(false);
    let mut analyzer = Analyzer::new(
        fixture.project.executable_view().expect("executable HIR"),
        &fixture.symbols,
        FinalSemanticCatalogs::production(&fixture.registered),
        FinalSemanticAnalysisControl::new(&cancellation),
    )
    .expect("analyzer");
    let analysis = analyzer
        .analyze_staged(NoPreparedStatementMutation)
        .expect("simple selected call");
    let calls = analysis.calls().collect::<Vec<_>>();
    assert_eq!(calls.len(), 1);
    let (owner, facts) = calls[0];
    let application = facts
        .selected_application()
        .expect("selected call application");
    assert_eq!(
        application.core().site(),
        crate::callable::CheckedCallSite::HirCall(owner)
    );
    assert!(matches!(
        application.core().callee(),
        crate::callable::CheckedCallCalleeExecution::Direct
    ));
    assert!(matches!(
        application.core().candidates().selected().state(),
        crate::callable::ResolvedCallableState::Base
    ));
    assert!(matches!(
        application.result(),
        crate::callable::CheckedCallResult::Value(TypeKind::I64)
    ));
    assert_eq!(application.result().ty(), &TypeKind::I64);
}

#[test]
fn multi_group_function_values_use_initializer_origin_and_shared_dependency() {
    let fixture = crate::final_analysis::tests::fixture(
        concat!(
            "fn make(first: i64)(second: i64) -> i64 { second }\n",
            "fn caller() { let partial = make(1i64); partial(2i64); partial(3i64); }\n",
        ),
        None,
    );
    let cancellation = AtomicBool::new(false);
    let mut analyzer = Analyzer::new(
        fixture.project.executable_view().expect("executable HIR"),
        &fixture.symbols,
        FinalSemanticCatalogs::production(&fixture.registered),
        FinalSemanticAnalysisControl::new(&cancellation),
    )
    .expect("analyzer");
    let analysis = analyzer
        .analyze_staged(NoPreparedStatementMutation)
        .expect("multi-group function-value analysis");
    let applications = analysis
        .calls()
        .filter_map(|(_, facts)| facts.selected_application())
        .collect::<Vec<_>>();
    assert_eq!(applications.len(), 3, "partial and both shared local calls");
    let origin = applications
        .iter()
        .find(|application| {
            matches!(
                application.result(),
                crate::callable::CheckedCallResult::Continuation(_)
            )
        })
        .expect("initializer partial application");
    assert_eq!(origin.core().current_group().get(), 0);
    assert!(matches!(
        origin.core().candidates().selected().state(),
        crate::callable::ResolvedCallableState::Base
    ));
    let dependents = applications
        .iter()
        .filter(|application| {
            matches!(
                application.core().callee(),
                crate::callable::CheckedCallCalleeExecution::Value { .. }
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        dependents.len(),
        2,
        "both local uses retain the origin authority"
    );
    for dependent in dependents {
        assert!(matches!(
            dependent.core().candidates().selected().state(),
            crate::callable::ResolvedCallableState::Continuation(_)
        ));
        assert!(matches!(
            dependent.result(),
            crate::callable::CheckedCallResult::Value(TypeKind::I64)
        ));
        let crate::callable::ResolvedCallableState::Continuation(continuation) =
            dependent.core().candidates().selected().state()
        else {
            unreachable!("function-value dependent has a continuation state");
        };
        assert_eq!(
            continuation.prefix_application_core(),
            origin.core().digest()
        );
        assert_eq!(
            continuation.prefix_application_site(),
            origin.core().stable_site()
        );
        assert_eq!(
            continuation.inherited_solution().digest(),
            origin.core().solution().digest()
        );
    }
}

#[test]
fn direct_nested_function_value_call_uses_inner_graph_site() {
    let fixture = crate::final_analysis::tests::fixture(
        concat!(
            "fn make(first: i64)(second: i64) -> i64 { second }\n",
            "fn caller() { make(1i64)(2i64); }\n",
        ),
        None,
    );
    let cancellation = AtomicBool::new(false);
    let mut analyzer = Analyzer::new(
        fixture.project.executable_view().expect("executable HIR"),
        &fixture.symbols,
        FinalSemanticCatalogs::production(&fixture.registered),
        FinalSemanticAnalysisControl::new(&cancellation),
    )
    .expect("analyzer");
    let analysis = analyzer
        .analyze_staged(NoPreparedStatementMutation)
        .expect("direct nested function-value analysis");
    let applications = analysis
        .calls()
        .filter_map(|(_, facts)| facts.selected_application())
        .collect::<Vec<_>>();
    assert_eq!(applications.len(), 2, "inner and outer applications");
    let inner = applications
        .iter()
        .find(|application| {
            matches!(
                application.result(),
                crate::callable::CheckedCallResult::Continuation(_)
            )
        })
        .expect("inner application");
    let outer = applications
        .iter()
        .find(|application| {
            matches!(
                application.core().callee(),
                crate::callable::CheckedCallCalleeExecution::Value { .. }
            )
        })
        .expect("outer application");
    assert_eq!(inner.core().current_group().get(), 0);
    assert!(matches!(
        inner.core().candidates().selected().state(),
        crate::callable::ResolvedCallableState::Base
    ));
    assert!(matches!(
        outer.core().callee(),
        crate::callable::CheckedCallCalleeExecution::Value { .. }
    ));
    assert!(matches!(
        outer.core().candidates().selected().state(),
        crate::callable::ResolvedCallableState::Continuation(_)
    ));
    assert!(matches!(
        inner.result(),
        crate::callable::CheckedCallResult::Continuation(_)
    ));
    assert!(matches!(
        outer.result(),
        crate::callable::CheckedCallResult::Value(TypeKind::I64)
    ));
    let crate::callable::ResolvedCallableState::Continuation(continuation) =
        outer.core().candidates().selected().state()
    else {
        unreachable!("outer function-value application has a continuation state");
    };
    assert_eq!(
        continuation.prefix_application_core(),
        inner.core().digest()
    );
    assert_eq!(
        continuation.prefix_application_site(),
        inner.core().stable_site()
    );
    assert_eq!(
        continuation.inherited_solution().digest(),
        inner.core().solution().digest()
    );
}

#[test]
fn independent_function_parameter_value_applies_without_prepared_dependency() {
    let fixture = crate::final_analysis::tests::fixture(
        concat!("fn caller(callback: (i64) -> i64 effects {}) { callback(1i64); }\n",),
        None,
    );
    let cancellation = AtomicBool::new(false);
    let mut analyzer = Analyzer::new(
        fixture.project.executable_view().expect("executable HIR"),
        &fixture.symbols,
        FinalSemanticCatalogs::production(&fixture.registered),
        FinalSemanticAnalysisControl::new(&cancellation),
    )
    .expect("analyzer");
    let analysis = analyzer
        .analyze_staged(NoPreparedStatementMutation)
        .expect("independent function parameter analysis");
    let applications = analysis
        .calls()
        .filter_map(|(_, facts)| facts.selected_application())
        .collect::<Vec<_>>();
    assert_eq!(applications.len(), 1, "independent callback call");
    let independent = applications[0];
    assert!(matches!(
        independent.core().callee(),
        crate::callable::CheckedCallCalleeExecution::Value { .. }
    ));
    assert!(matches!(
        independent.core().candidates().selected().state(),
        crate::callable::ResolvedCallableState::Base
    ));
    assert!(matches!(
        independent
            .core()
            .candidates()
            .selected()
            .base()
            .authority()
            .stable(),
        crate::callable::ResolvedCallableStableIdentity::Lexical(_)
    ));
    assert!(matches!(
        independent.result(),
        crate::callable::CheckedCallResult::Value(TypeKind::I64)
    ));
}

#[test]
fn terminal_function_result_enters_independent_origin_without_dependency() {
    let fixture = crate::final_analysis::tests::fixture(
        concat!(
            "fn make_loader() -> ((Unit) -> Unit effects {}) {\n",
            "    |_unit: Unit| -> Unit {}\n",
            "}\n",
            "fn caller() { let loader = make_loader(); loader(()); }\n",
        ),
        None,
    );
    let cancellation = AtomicBool::new(false);
    let mut analyzer = Analyzer::new(
        fixture.project.executable_view().expect("executable HIR"),
        &fixture.symbols,
        FinalSemanticCatalogs::production(&fixture.registered),
        FinalSemanticAnalysisControl::new(&cancellation),
    )
    .expect("analyzer");
    let analysis = analyzer
        .analyze_staged(NoPreparedStatementMutation)
        .expect("terminal function result analysis");
    let applications = analysis
        .calls()
        .filter_map(|(_, facts)| facts.selected_application())
        .collect::<Vec<_>>();
    assert_eq!(
        applications.len(),
        2,
        "factory and independent function-value call"
    );
    let producer = applications
        .iter()
        .find(|application| {
            matches!(
                application.result(),
                crate::callable::CheckedCallResult::Value(TypeKind::Function { .. })
            )
        })
        .expect("terminal function producer");
    let consumer = applications
        .iter()
        .find(|application| {
            matches!(
                application.core().callee(),
                crate::callable::CheckedCallCalleeExecution::Value { .. }
            )
        })
        .expect("independent function-value consumer");
    assert!(matches!(
        producer.core().callee(),
        crate::callable::CheckedCallCalleeExecution::Direct
    ));
    assert!(matches!(
        producer.core().candidates().selected().state(),
        crate::callable::ResolvedCallableState::Base
    ));
    assert!(matches!(
        consumer.core().candidates().selected().state(),
        crate::callable::ResolvedCallableState::Base
    ));
    assert!(matches!(
        consumer
            .core()
            .candidates()
            .selected()
            .base()
            .authority()
            .stable(),
        crate::callable::ResolvedCallableStableIdentity::FunctionValue(_)
    ));
    assert!(matches!(
        consumer.result(),
        crate::callable::CheckedCallResult::Value(TypeKind::Unit)
    ));
}

#[test]
fn three_group_function_values_follow_prepared_adjacency() {
    let fixture = crate::final_analysis::tests::fixture(
        concat!(
            "fn make(first: i64)(second: i64)(third: i64) -> i64 { third }\n",
            "fn caller() { let first = make(1i64); let second = first(2i64); second(3i64); }\n",
        ),
        None,
    );
    let cancellation = AtomicBool::new(false);
    let mut analyzer = Analyzer::new(
        fixture.project.executable_view().expect("executable HIR"),
        &fixture.symbols,
        FinalSemanticCatalogs::production(&fixture.registered),
        FinalSemanticAnalysisControl::new(&cancellation),
    )
    .expect("analyzer");
    let analysis = analyzer
        .analyze_staged(NoPreparedStatementMutation)
        .expect("three-group function-value analysis");
    let mut applications = analysis
        .calls()
        .filter_map(|(_, facts)| facts.selected_application())
        .collect::<Vec<_>>();
    applications.sort_by_key(|application| application.core().current_group());
    assert_eq!(applications.len(), 3, "three selected applications");
    let origin = applications[0];
    let middle = applications[1];
    let terminal = applications[2];
    assert_eq!(origin.core().current_group().get(), 0);
    assert_eq!(middle.core().current_group().get(), 1);
    assert_eq!(terminal.core().current_group().get(), 2);
    assert!(matches!(
        origin.core().candidates().selected().state(),
        crate::callable::ResolvedCallableState::Base
    ));
    assert!(matches!(
        middle.core().candidates().selected().state(),
        crate::callable::ResolvedCallableState::Continuation(_)
    ));
    assert!(matches!(
        terminal.core().candidates().selected().state(),
        crate::callable::ResolvedCallableState::Continuation(_)
    ));
    assert!(matches!(
        origin.result(),
        crate::callable::CheckedCallResult::Continuation(_)
    ));
    assert!(matches!(
        middle.result(),
        crate::callable::CheckedCallResult::Continuation(_)
    ));
    assert!(matches!(
        terminal.result(),
        crate::callable::CheckedCallResult::Value(TypeKind::I64)
    ));
    assert!(matches!(
        middle.core().callee(),
        crate::callable::CheckedCallCalleeExecution::Value { .. }
    ));
    assert!(matches!(
        terminal.core().callee(),
        crate::callable::CheckedCallCalleeExecution::Value { .. }
    ));
    let crate::callable::ResolvedCallableState::Continuation(middle_continuation) =
        middle.core().candidates().selected().state()
    else {
        unreachable!("middle function-value application has a continuation state");
    };
    let crate::callable::ResolvedCallableState::Continuation(terminal_continuation) =
        terminal.core().candidates().selected().state()
    else {
        unreachable!("terminal function-value application has a continuation state");
    };
    assert_eq!(
        middle_continuation.prefix_application_core(),
        origin.core().digest()
    );
    assert_eq!(
        middle_continuation.inherited_solution().digest(),
        origin.core().solution().digest()
    );
    assert_eq!(
        terminal_continuation.prefix_application_core(),
        middle.core().digest()
    );
    assert_eq!(
        terminal_continuation.inherited_solution().digest(),
        middle.core().solution().digest()
    );
}

#[test]
fn rolled_back_prepared_continuation_is_stale_without_independent_fallback() {
    let fixture = crate::final_analysis::tests::fixture(
        concat!(
            "fn make(first: i64)(second: i64) -> i64 { second }\n",
            "fn caller() { make(1i64); }\n",
        ),
        None,
    );
    let module = fixture
        .project
        .executable_view()
        .expect("executable HIR")
        .module(&CanonicalModulePath::crate_root())
        .expect("root module");
    let call_owner = module
        .expressions()
        .find_map(|(owner, expression)| {
            matches!(expression.kind(), HirExprKind::Call(_)).then_some(owner)
        })
        .expect("call expression");
    let cancellation = AtomicBool::new(false);
    let mut analyzer = Analyzer::new(
        fixture.project.executable_view().expect("executable HIR"),
        &fixture.symbols,
        FinalSemanticCatalogs::production(&fixture.registered),
        FinalSemanticAnalysisControl::new(&cancellation),
    )
    .expect("analyzer");
    analyzer.resolve_all_types().expect("types");
    analyzer.seed_local_types().expect("locals");
    let staged = analyzer.stage_checked_callables().expect("callables");
    for (owner, fact) in &staged.effect_expressions {
        analyzer
            .facts
            .publish_new_expression(*owner, fact.clone())
            .expect("effect expression");
    }
    analyzer.staged_callables = Some(staged);
    analyzer.infer_statement_bindings().expect("bindings");

    let captured = Rc::new(RefCell::new(None));
    let captured_for_transaction = Rc::clone(&captured);
    let captured_actual = Rc::new(RefCell::new(None));
    let captured_actual_for_transaction = Rc::clone(&captured_actual);
    let outcome = analyzer
        .run_candidate_fact_transaction(|this, authority, _transaction| {
            let context =
                AnalyzerExpressionContext::candidate(authority, Rc::clone(&this.call_frames));
            this.evaluate_expression(&context, call_owner, None)?;
            drop(context);
            let actual = this
                .facts
                .prepared_calls()
                .expect("prepared call graph")
                .selected_nodes()
                .find(|node| node.site() == crate::callable::CheckedCallSite::HirCall(call_owner))
                .expect("candidate call graph node")
                .prefix()
                .application()
                .result_type()
                .expect("candidate call result type");
            *captured_actual_for_transaction.borrow_mut() = Some(actual.clone());
            let reference = match crate::callable::PreparedCallGraphIngress::new(
                this.facts.prepared_calls().expect("prepared call graph"),
            )
            .continuation_at(
                crate::callable::CheckedCallSite::HirCall(call_owner),
                &actual,
            ) {
                Ok(crate::callable::PreparedCallSiteContinuation::Prepared(reference)) => reference,
                Ok(crate::callable::PreparedCallSiteContinuation::Independent) => {
                    return Err(AnalyzerExpressionError::Call {
                        owner: call_owner,
                        failure: CallAnalysisFailure::Invariant(
                            super::calls::CallAnalysisInvariant::Constraint(
                                crate::callable::CallConstraintInvariant::InvalidPreparedNodeState,
                            ),
                        ),
                    });
                }
                Err(invariant) => {
                    return Err(AnalyzerExpressionError::Call {
                        owner: call_owner,
                        failure: CallAnalysisFailure::Invariant(
                            super::calls::CallAnalysisInvariant::Constraint(invariant),
                        ),
                    });
                }
            };
            *captured_for_transaction.borrow_mut() = Some(reference.clone());
            Ok::<_, AnalyzerExpressionError>(CandidateFactTransactionAction::Rollback(reference))
        })
        .expect("candidate rollback");
    assert!(matches!(
        outcome,
        super::state::CandidateFactTransactionOutcome::RolledBack(_)
    ));
    assert_eq!(
        analyzer
            .facts
            .prepared_calls()
            .expect("prepared call graph")
            .selected_nodes()
            .count(),
        0,
        "rollback removes the issued continuation node"
    );
    let stale = captured.borrow_mut().take().expect("captured continuation");
    let result = crate::callable::PreparedCallContinuationAuthority::resolve_prepared_continuation(
        analyzer
            .facts
            .prepared_calls()
            .expect("prepared call graph"),
        &stale,
        &captured_actual
            .borrow_mut()
            .take()
            .expect("captured call result type"),
    );
    assert!(matches!(
        result,
        Err(crate::callable::CallConstraintInvariant::MissingOrStalePreparedNode)
    ));
}

#[test]
fn postfix_both_fail_rolls_back_both_candidate_subgraphs_and_guard() {
    let fixture = crate::final_analysis::tests::fixture("fn caller() { 1[true]; }\n", None);
    let module = fixture
        .project
        .executable_view()
        .expect("executable HIR")
        .module(&CanonicalModulePath::crate_root())
        .expect("root module");
    let (owner, postfix) = module
        .expressions()
        .find_map(|(owner, expression)| match expression.kind() {
            HirExprKind::PostfixBracket(postfix) => Some((owner, postfix)),
            _ => None,
        })
        .expect("postfix expression");
    let HirPostfixBracketCandidates::Ambiguous { index, dialogue } = postfix.candidates() else {
        panic!("postfix retains both interpretation candidates");
    };
    let candidate_owners = [*index, *dialogue];
    let target = postfix.target();
    let cancellation = AtomicBool::new(false);
    let mut analyzer = Analyzer::new(
        fixture.project.executable_view().expect("executable HIR"),
        &fixture.symbols,
        FinalSemanticCatalogs::production(&fixture.registered),
        FinalSemanticAnalysisControl::new(&cancellation),
    )
    .expect("analyzer");

    for _ in 0..2 {
        assert!(matches!(
            analyzer.check_expression_published(owner, None),
            Err(FinalSemanticAnalysisError::UnresolvedPostfixBracket {
                owner: failed
            }) if failed == owner
        ));
        assert!(!analyzer.facts.expressions().contains_key(&owner));
        assert!(!analyzer.facts.expressions().contains_key(&target));
        for candidate in candidate_owners {
            assert!(!analyzer.facts.expressions().contains_key(&candidate));
            assert!(!analyzer.facts.calls().contains_key(&candidate));
        }
    }
}

#[test]
fn postfix_ambiguous_rolls_back_both_successful_candidate_rows() {
    let fixture = crate::final_analysis::tests::fixture(
        "fn caller(items: Seq<i64>, key: usize) { items[key]; }\n",
        None,
    );
    let module = fixture
        .project
        .executable_view()
        .expect("executable HIR")
        .module(&CanonicalModulePath::crate_root())
        .expect("root module");
    let (owner, postfix) = module
        .expressions()
        .find_map(|(owner, expression)| match expression.kind() {
            HirExprKind::PostfixBracket(postfix) => Some((owner, postfix)),
            _ => None,
        })
        .expect("postfix expression");
    let HirPostfixBracketCandidates::Ambiguous { index, dialogue } = postfix.candidates() else {
        panic!("postfix retains both interpretation candidates");
    };
    let index = *index;
    let dialogue = *dialogue;
    let cancellation = AtomicBool::new(false);
    let mut analyzer = Analyzer::new(
        fixture.project.executable_view().expect("executable HIR"),
        &fixture.symbols,
        FinalSemanticCatalogs::production(&fixture.registered),
        FinalSemanticAnalysisControl::new(&cancellation),
    )
    .expect("analyzer");

    // The closed type algebra intentionally has no value that is both an
    // index source and a dialogue target. Exercise the defensive
    // ambiguity branch with two already-checked candidate rows, but mint
    // and roll them back through the real semantic-fact transaction.
    let outcome = analyzer.run_candidate_fact_transaction(|this, authority, _transaction| {
        this.facts
            .publish_new_expression(
                index,
                CheckedExpression::new(
                    TypeKind::I64,
                    CheckedTypeSelection::Inferred,
                    EffectSet::new(),
                    CheckedExpressionResolution::Structural,
                ),
            )
            .map_err(|_| {
                AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::WrongPayloadFamily)
            })?;
        this.facts
            .publish_new_expression(
                dialogue,
                CheckedExpression::new(
                    TypeKind::I64,
                    CheckedTypeSelection::Inferred,
                    EffectSet::new(),
                    CheckedExpressionResolution::Structural,
                ),
            )
            .map_err(|_| {
                AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::WrongPayloadFamily)
            })?;
        let context = AnalyzerExpressionContext::candidate(authority, Rc::clone(&this.call_frames));
        let _ = this.evaluate_expression(&context, owner, None)?;
        drop(context);
        Ok::<CandidateFactTransactionAction<()>, AnalyzerExpressionError>(
            CandidateFactTransactionAction::Commit(()),
        )
    });
    let outcome = outcome
        .map(|_| ())
        .map_err(|error| error.into_public(owner));
    assert!(matches!(
        outcome,
        Err(FinalSemanticAnalysisError::AmbiguousPostfixBracket {
            owner: failed
        }) if failed == owner
    ));
    assert!(!analyzer.facts.expressions().contains_key(&owner));
    assert!(!analyzer.facts.expressions().contains_key(&index));
    assert!(!analyzer.facts.expressions().contains_key(&dialogue));
    assert!(!matches!(
        analyzer.check_expression_published(owner, None),
        Err(FinalSemanticAnalysisError::ExpressionCycle { .. })
    ));
}

#[test]
fn contextual_literal_cache_rewrite_rolls_back_and_retry_replaces_baseline() {
    let fixture = crate::final_analysis::tests::fixture("fn caller() { 1; }\n", None);
    let module = fixture
        .project
        .executable_view()
        .expect("executable HIR")
        .module(&CanonicalModulePath::crate_root())
        .expect("root module");
    let owner = module
        .expressions()
        .find_map(|(owner, expression)| {
            matches!(expression.kind(), HirExprKind::Literal(_)).then_some(owner)
        })
        .expect("literal expression");
    let cancellation = AtomicBool::new(false);
    let mut analyzer = Analyzer::new(
        fixture.project.executable_view().expect("executable HIR"),
        &fixture.symbols,
        FinalSemanticCatalogs::production(&fixture.registered),
        FinalSemanticAnalysisControl::new(&cancellation),
    )
    .expect("analyzer");
    let baseline = analyzer
        .check_expression_published(owner, None)
        .expect("default literal fact");

    let outcome = analyzer.run_candidate_fact_transaction(|this, _authority, _transaction| {
        let contextual = this
            .check_expression_published(owner, Some(&TypeKind::I64))
            .map_err(AnalyzerExpressionError::fatal)?;
        assert_eq!(contextual.ty(), &TypeKind::I64);
        Err::<CandidateFactTransactionAction<()>, _>(AnalyzerExpressionError::fatal(
            FinalSemanticAnalysisError::CheckedCallableCatalog,
        ))
    });
    let outcome = outcome
        .map(|_| ())
        .map_err(|error| error.into_public(owner));
    assert!(matches!(
        outcome,
        Err(FinalSemanticAnalysisError::CheckedCallableCatalog)
    ));
    assert_eq!(analyzer.facts.expressions().get(&owner), Some(&baseline));

    let retry = analyzer
        .check_expression_published(owner, Some(&TypeKind::U64))
        .expect("contextual retry");
    assert_eq!(retry.ty(), &TypeKind::U64);
    assert_eq!(analyzer.facts.expressions().get(&owner), Some(&retry));
}

#[test]
fn uncached_expression_failure_retry_cleans_structured_guard() {
    let fixture = crate::final_analysis::tests::fixture("fn caller() { missing; }\n", None);
    let module = fixture
        .project
        .executable_view()
        .expect("executable HIR")
        .module(&CanonicalModulePath::crate_root())
        .expect("root module");
    let owner = module
        .expressions()
        .find_map(|(owner, expression)| {
            matches!(expression.kind(), HirExprKind::Path(_)).then_some(owner)
        })
        .expect("unresolved path expression");
    let cancellation = AtomicBool::new(false);
    let mut analyzer = Analyzer::new(
        fixture.project.executable_view().expect("executable HIR"),
        &fixture.symbols,
        FinalSemanticCatalogs::production(&fixture.registered),
        FinalSemanticAnalysisControl::new(&cancellation),
    )
    .expect("analyzer");

    for _ in 0..2 {
        assert!(matches!(
            analyzer.check_expression_published(owner, None),
            Err(FinalSemanticAnalysisError::ValueResolutionFailed {
                owner: failed
            }) if failed == owner
        ));
        assert!(!analyzer.facts.expressions().contains_key(&owner));
    }
}

#[test]
fn function_value_origin_query_resumes_exact_checked_owner() {
    let fixture = crate::final_analysis::tests::fixture(
        concat!(
            "fn make(first: i64)(second: i64) -> i64 { second }\n",
            "fn caller() { let first = make(1i64); let alias = first; alias(2i64); }\n",
        ),
        None,
    );
    let cancellation = AtomicBool::new(false);
    let mut analyzer = Analyzer::new(
        fixture.project.executable_view().expect("executable HIR"),
        &fixture.symbols,
        FinalSemanticCatalogs::production(&fixture.registered),
        FinalSemanticAnalysisControl::new(&cancellation),
    )
    .expect("analyzer");
    analyzer
        .analyze_staged(NoPreparedStatementMutation)
        .expect("alias function-value analysis");
    let module = fixture
        .project
        .executable_view()
        .expect("executable HIR")
        .module(&CanonicalModulePath::crate_root())
        .expect("root HIR module");
    let topology = Arc::clone(&analyzer.topology);
    let local_origins = topology
        .module(module.module_id())
        .expect("module topology")
        .local_origins();
    let facts = analyzer.facts.expressions();
    let candidate = module
        .expressions()
        .filter_map(|(owner, _expression)| {
            let checked = facts.get(&owner)?;
            let Some(CheckedExpressionResolution::Value(CheckedValueResolution::Local(local))) =
                checked.checked_resolution()
            else {
                return None;
            };
            if !matches!(checked.ty(), TypeKind::Function { .. }) {
                return None;
            }
            let HirLocalValueOrigin::DirectInitializer(initializer) =
                local_origins.origin(*local)?
            else {
                return None;
            };
            let HirExprKind::Path(_) = module.resolve_expr(initializer).ok()?.kind() else {
                return None;
            };
            Some((owner, checked.clone()))
        })
        .next()
        .expect("aliased function-value path");

    let mut progress = crate::callable::prepare_function_value_origin_query(
        Arc::clone(&topology),
        module,
        candidate.0,
        &BTreeMap::new(),
    )
    .expect("origin query starts");
    let mut needs = 0_u32;
    loop {
        match progress {
            crate::callable::PreparedFunctionValueOriginProgress::Need(need) => {
                needs = needs.checked_add(1).expect("query depth");
                let owner = need.expression();
                let checked = facts
                    .get(&owner)
                    .expect("query owner has checked expression");
                progress = need
                    .resume(owner, checked, module)
                    .expect("exact checked owner resumes query");
            }
            crate::callable::PreparedFunctionValueOriginProgress::Ready(evidence) => {
                assert!(needs >= 2, "the alias chain should cross two local origins");
                assert!(matches!(
                    evidence.producer(),
                    crate::callable::PreparedFunctionValueOriginProducer::Call(
                        crate::callable::CheckedCallSite::HirCall(_)
                    )
                ));
                break;
            }
        }
    }

    let wrong_query = crate::callable::prepare_function_value_origin_query(
        Arc::clone(&topology),
        module,
        candidate.0,
        &BTreeMap::new(),
    )
    .expect("wrong-owner query starts");
    let crate::callable::PreparedFunctionValueOriginProgress::Need(need) = wrong_query else {
        panic!("the initial path must require its checked fact");
    };
    let wrong_owner = module
        .expressions()
        .map(|(owner, _)| owner)
        .find(|owner| *owner != need.expression())
        .expect("foreign expression owner");
    let wrong_checked = facts.get(&need.expression()).expect("query owner fact");
    let error = need.resume(wrong_owner, wrong_checked, module);
    assert!(matches!(
        error,
        Err(crate::callable::PreparedFunctionValueOriginQueryError::Invalid)
    ));
}

#[test]
fn function_value_origin_query_classifies_independent_parameters_and_cycles() {
    let independent_fixture = crate::final_analysis::tests::fixture(
        "fn caller(callback: (i64) -> i64 effects {}) { callback(1i64); }\n",
        None,
    );
    let independent_module = independent_fixture
        .project
        .executable_view()
        .expect("executable HIR")
        .module(&CanonicalModulePath::crate_root())
        .expect("root HIR module");
    let independent_topology = independent_fixture
        .project
        .executable_view()
        .expect("executable HIR")
        .accept_symbol_generation(&independent_fixture.symbols)
        .expect("accepted HIR generation")
        .into_evaluation_topology()
        .expect("evaluation topology");
    let independent_local = independent_module
        .locals()
        .find_map(|(owner, local)| (local.name().as_str() == "callback").then_some(owner))
        .expect("callback local");
    let independent_expression = independent_module
        .expressions()
        .find_map(|(owner, expression)| {
            let HirExprKind::Path(path) = expression.kind() else {
                return None;
            };
            (path.as_resolved().and_then(|path| path.lexical_name()) == Some("callback"))
                .then_some(owner)
        })
        .expect("callback path");
    let independent_checked = CheckedExpression::new(
        TypeKind::function([TypeKind::I64], TypeKind::I64),
        CheckedTypeSelection::Inferred,
        EffectSet::new(),
        CheckedExpressionResolution::Value(CheckedValueResolution::Local(independent_local)),
    );
    let independent_facts = BTreeMap::from([(independent_expression, independent_checked.into())]);
    let independent = crate::callable::prepare_function_value_origin_query(
        Arc::clone(&independent_topology),
        independent_module,
        independent_expression,
        &independent_facts,
    )
    .expect("independent origin query");
    assert!(matches!(
        independent,
        crate::callable::PreparedFunctionValueOriginProgress::Ready(evidence)
            if matches!(
                evidence.producer(),
                crate::callable::PreparedFunctionValueOriginProducer::Lexical { .. }
                    | crate::callable::PreparedFunctionValueOriginProducer::IndependentExpression {
                        ..
                    }
            )
    ));

    let cycle_fixture =
        crate::final_analysis::tests::fixture("fn caller() { let x = x; x(1i64); }\n", None);
    let cycle_module = cycle_fixture
        .project
        .executable_view()
        .expect("executable HIR")
        .module(&CanonicalModulePath::crate_root())
        .expect("root HIR module");
    let cycle_topology = cycle_fixture
        .project
        .executable_view()
        .expect("executable HIR")
        .accept_symbol_generation(&cycle_fixture.symbols)
        .expect("accepted HIR generation")
        .into_evaluation_topology()
        .expect("evaluation topology");
    let cycle_local = cycle_module
        .locals()
        .find_map(|(owner, local)| (local.name().as_str() == "x").then_some(owner))
        .expect("cycle local");
    let cycle_expression = cycle_module
        .expressions()
        .find_map(|(owner, expression)| {
            let HirExprKind::Path(path) = expression.kind() else {
                return None;
            };
            (path.as_resolved().and_then(|path| path.lexical_name()) == Some("x")).then_some(owner)
        })
        .expect("cycle path");
    let cycle_checked = CheckedExpression::new(
        TypeKind::function([TypeKind::I64], TypeKind::I64),
        CheckedTypeSelection::Inferred,
        EffectSet::new(),
        CheckedExpressionResolution::Value(CheckedValueResolution::Local(cycle_local)),
    );
    let cycle_checked: PreparedExpressionFact = cycle_checked.into();
    let cycle_facts = BTreeMap::from([(cycle_expression, cycle_checked.clone())]);
    let progress = crate::callable::prepare_function_value_origin_query(
        Arc::clone(&cycle_topology),
        cycle_module,
        cycle_expression,
        &cycle_facts,
    )
    .expect("cycle query begins with a checked path");
    let crate::callable::PreparedFunctionValueOriginProgress::Need(need) = progress else {
        panic!("cycle query must request the direct initializer fact");
    };
    let error = match need.resume(cycle_expression, &cycle_checked, cycle_module) {
        Ok(_) => panic!("revisiting the same local must be a typed cycle"),
        Err(error) => error,
    };
    assert!(matches!(
        error,
        crate::callable::PreparedFunctionValueOriginQueryError::Cycle
    ));
}

#[test]
fn lexical_function_parameter_issues_top_level_prepared_identity() {
    let fixture = crate::final_analysis::tests::fixture(
        "fn caller(callback: (i64) -> i64 effects {}) { callback(1i64); }\n",
        None,
    );
    let cancellation = AtomicBool::new(false);
    let mut analyzer = Analyzer::new(
        fixture.project.executable_view().expect("executable HIR"),
        &fixture.symbols,
        FinalSemanticCatalogs::production(&fixture.registered),
        FinalSemanticAnalysisControl::new(&cancellation),
    )
    .expect("analyzer");
    analyzer
        .analyze_staged(NoPreparedStatementMutation)
        .expect("lexical function call");
    let selected = analyzer
        .facts
        .calls()
        .values()
        .find_map(|call| call.selected_application())
        .map(|application| application.core().candidates().selected())
        .expect("selected lexical candidate");
    assert!(matches!(
        selected.base().authority().stable(),
        crate::callable::ResolvedCallableStableIdentity::Lexical(_)
    ));
}

#[test]
fn evaluator_records_implicit_and_explicit_capture_modes_on_terminal_facts() {
    let fixture = crate::final_analysis::tests::fixture(
        concat!(
            "fn caller() {\n",
            "    let mut outer = 1i64;\n",
            "    let implicit_read = _ + outer;\n",
            "    result { outer; outer = _; () };\n",
            "    let explicit_read = || { outer };\n",
            "    let explicit_write = || { outer; outer = 2i64; () };\n",
            "}\n",
        ),
        None,
    );
    let cancellation = AtomicBool::new(false);
    let mut analyzer = Analyzer::new(
        fixture.project.executable_view().expect("executable HIR"),
        &fixture.symbols,
        FinalSemanticCatalogs::production(&fixture.registered),
        FinalSemanticAnalysisControl::new(&cancellation),
    )
    .expect("analyzer");
    analyzer.resolve_all_types().expect("resolved types");
    analyzer.seed_local_types().expect("seeded locals");
    let staged = analyzer
        .stage_checked_callables()
        .expect("staged checked callables");
    for (owner, fact) in &staged.effect_expressions {
        analyzer
            .facts
            .publish_new_expression(*owner, fact.clone())
            .expect("published effect expression");
    }
    analyzer.staged_callables = Some(staged);
    analyzer
        .infer_statement_bindings()
        .expect("inferred statement bindings");
    assert!(
        analyzer
            .topology
            .modules()
            .iter()
            .flat_map(|module| module.expression_uses().rows())
            .any(|row| row.capture_access() == arcweft_lang_hir::scope::CaptureAccess::Reassign),
        "typed statement target must be classified as Reassign"
    );
    let implicit_write = analyzer
        .modules
        .values()
        .flat_map(|module| module.expressions())
        .find_map(|(owner, expression)| {
            matches!(expression.kind(), HirExprKind::ComputationBlock(_)).then_some(owner)
        })
        .expect("implicit reassign producer");
    let context = AnalyzerExpressionContext::published(Rc::clone(&analyzer.call_frames));
    analyzer
        .evaluate_expression(
            &context,
            implicit_write,
            Some(&TypeKind::function(
                [TypeKind::I64],
                TypeKind::Result {
                    ok: Box::new(TypeKind::Unit),
                    error: Box::new(TypeKind::Never),
                },
            )),
        )
        .expect("contextual implicit reassign producer");

    let mut implicit = Vec::new();
    let mut explicit = Vec::new();
    for checked in analyzer.facts.expressions().values() {
        match checked.checked_resolution() {
            Some(CheckedExpressionResolution::ImplicitCallable(callable)) => {
                assert!(Arc::ptr_eq(callable.topology(), &analyzer.topology));
                let [capture] = callable.captures() else {
                    continue;
                };
                implicit.push(capture.mode());
            }
            Some(CheckedExpressionResolution::Closure(closure)) => {
                assert!(Arc::ptr_eq(closure.topology(), &analyzer.topology));
                let [capture] = closure.captures() else {
                    continue;
                };
                explicit.push(capture.mode());
            }
            _ => {}
        }
    }
    implicit.sort();
    explicit.sort();
    let expected = vec![
        arcweft_lang_hir::scope::CaptureAccess::Read,
        arcweft_lang_hir::scope::CaptureAccess::Reassign,
    ];
    assert_eq!(implicit, expected);
    assert_eq!(explicit, expected);
}

#[test]
fn function_value_origin_retains_terminal_captures_through_aliases() {
    for (source, mode) in [
        (
            "fn caller() { let outer = 1i64; let producer = || -> i64 { outer }; let alias = producer; alias(); }\n",
            arcweft_lang_hir::scope::CaptureAccess::Read,
        ),
        (
            "fn caller() { let mut outer = 1i64; let producer: (i64) -> Result<Unit, Never> = result { outer; outer = _; () }; let alias = producer; alias(2i64); }\n",
            arcweft_lang_hir::scope::CaptureAccess::Reassign,
        ),
    ] {
        let fixture = crate::final_analysis::tests::fixture(source, None);
        let module = fixture
            .project
            .executable_view()
            .expect("executable HIR")
            .module(&CanonicalModulePath::crate_root())
            .expect("root module");
        let outer = module
            .locals()
            .find_map(|(id, local)| (local.name().as_str() == "outer").then_some(id))
            .expect("outer local");
        let alias_use = module
            .expressions()
            .find_map(|(id, expression)| {
                let HirExprKind::Path(path) = expression.kind() else {
                    return None;
                };
                (path.as_resolved().and_then(|path| path.lexical_name()) == Some("alias"))
                    .then_some(id)
            })
            .expect("alias use");
        let cancellation = AtomicBool::new(false);
        let mut analyzer = Analyzer::new(
            fixture.project.executable_view().expect("executable HIR"),
            &fixture.symbols,
            FinalSemanticCatalogs::production(&fixture.registered),
            FinalSemanticAnalysisControl::new(&cancellation),
        )
        .expect("analyzer");
        analyzer.resolve_all_types().expect("resolved types");
        analyzer.seed_local_types().expect("seeded locals");
        let staged = analyzer
            .stage_checked_callables()
            .expect("staged checked callables");
        for (owner, fact) in &staged.effect_expressions {
            analyzer
                .facts
                .publish_new_expression(*owner, fact.clone())
                .expect("published effect expression");
        }
        analyzer.staged_callables = Some(staged);
        analyzer
            .infer_statement_bindings()
            .expect("inferred statement bindings");
        analyzer
            .validate_callable_body_results()
            .expect("validated callable results");
        analyzer
            .analyze_all_expressions()
            .expect("analyzed expression facts");
        let producer = analyzer
            .facts
            .expressions()
            .iter()
            .find_map(|(owner, checked)| {
                matches!(
                    checked.checked_resolution(),
                    Some(
                        CheckedExpressionResolution::Closure(_)
                            | CheckedExpressionResolution::ImplicitCallable(_)
                    )
                )
                .then_some(*owner)
            })
            .expect("terminal function-value producer");
        let mut progress = crate::callable::prepare_function_value_origin_query(
            Arc::clone(&analyzer.topology),
            module,
            alias_use,
            analyzer.facts.expressions(),
        )
        .expect("origin query");
        let evidence = loop {
            match progress {
                crate::callable::PreparedFunctionValueOriginProgress::Ready(evidence) => {
                    break evidence;
                }
                crate::callable::PreparedFunctionValueOriginProgress::Need(need) => {
                    let owner = need.expression();
                    progress = need
                        .resume(
                            owner,
                            analyzer
                                .facts
                                .expressions()
                                .get(&owner)
                                .expect("queried fact"),
                            module,
                        )
                        .expect("resume origin query");
                }
            }
        };
        assert_eq!(evidence.captures().len(), 1);
        assert_eq!(evidence.captures()[0].local(), outer);
        assert_eq!(evidence.captures()[0].mode(), mode);
        assert!(matches!(
            evidence.producer(),
            crate::callable::PreparedFunctionValueOriginProducer::IndependentExpression {
                producer: actual
            } if *actual == producer
        ));

        let foreign_topology = fixture
            .project
            .executable_view()
            .expect("executable HIR")
            .accept_symbol_generation(&fixture.symbols)
            .expect("accepted HIR generation")
            .into_evaluation_topology()
            .expect("foreign allocation");
        assert!(!Arc::ptr_eq(&foreign_topology, &analyzer.topology));
        let mut foreign_progress = crate::callable::prepare_function_value_origin_query(
            foreign_topology,
            module,
            alias_use,
            analyzer.facts.expressions(),
        )
        .expect("foreign query starts before terminal fact");
        loop {
            match foreign_progress {
                crate::callable::PreparedFunctionValueOriginProgress::Need(need) => {
                    let owner = need.expression();
                    match need.resume(
                        owner,
                        analyzer
                            .facts
                            .expressions()
                            .get(&owner)
                            .expect("foreign query owner fact"),
                        module,
                    ) {
                        Err(
                            crate::callable::PreparedFunctionValueOriginQueryError::CaptureTopologyMismatch(
                                crate::final_analysis::CheckedCaptureAuthorityViolation::TopologyMismatch
                            )
                        ) => break,
                        Err(error) => {
                            panic!("unexpected foreign topology result: {error:?}")
                        }
                        Ok(progress) => foreign_progress = progress,
                    }
                }
                crate::callable::PreparedFunctionValueOriginProgress::Ready(_) => {
                    panic!("foreign topology query unexpectedly succeeded")
                }
            }
        }

        let wrong_producer = module
            .expressions()
            .find_map(|(owner, expression)| {
                (owner != producer && matches!(expression.kind(), HirExprKind::Literal(_)))
                    .then_some(owner)
            })
            .expect("independent wrong producer");
        let terminal = analyzer
            .facts
            .expressions()
            .get(&producer)
            .expect("terminal checked fact")
            .clone();
        assert!(matches!(
            crate::callable::prepare_function_value_origin_query(
                Arc::clone(&analyzer.topology),
                module,
                wrong_producer,
                &BTreeMap::from([(wrong_producer, terminal)]),
            ),
            Err(
                crate::callable::PreparedFunctionValueOriginQueryError::CaptureProducerMismatch(
                    crate::final_analysis::CheckedCaptureAuthorityViolation::ProducerMismatch {
                        expected,
                        actual,
                    }
                )
            ) if expected == wrong_producer && actual == producer
        ));
    }
}
