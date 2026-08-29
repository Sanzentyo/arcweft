use super::*;

use crate::types::SemanticTypeDigest;
use arcweft_lang_hir::identity::{ItemId, TypeId};

fn ingress_fixture_ids() -> (CallableDeclarationKey, TypeId, Box<[ItemId]>) {
    let fixture =
        crate::final_analysis::tests::fixture("fn first(value: i64) {}\nfn second() {}\n", None);
    let module = fixture
        .project
        .executable_view()
        .expect("executable HIR")
        .module(&arcweft_lang_syntax::ast::module_path::CanonicalModulePath::crate_root())
        .expect("root module");
    let declaration = fixture
        .symbols
        .callable_symbols()
        .next()
        .expect("callable declaration")
        .declaration()
        .clone();
    let event_type = module
        .types()
        .next()
        .map(|(owner, _)| owner)
        .expect("event type fixture");
    let contributors = module
        .items()
        .map(|(owner, _)| owner)
        .collect::<Vec<_>>()
        .into_boxed_slice();
    assert!(contributors.len() >= 2);
    (declaration, event_type, contributors)
}

fn ingress_fixture_declarations() -> (Box<[CallableDeclarationKey]>, TypeId, Box<[ItemId]>) {
    let fixture = crate::final_analysis::tests::fixture(
        "fn first(value: i64) {}\nfn second(value: i64) {}\nfn third(value: i64) {}\n",
        None,
    );
    let module = fixture
        .project
        .executable_view()
        .expect("executable HIR")
        .module(&arcweft_lang_syntax::ast::module_path::CanonicalModulePath::crate_root())
        .expect("root module");
    let declarations = fixture
        .symbols
        .callable_symbols()
        .map(|symbol| symbol.declaration().clone())
        .collect::<Vec<_>>()
        .into_boxed_slice();
    assert!(declarations.len() >= 3);
    let event_type = module
        .types()
        .next()
        .map(|(owner, _)| owner)
        .expect("event type fixture");
    let contributors = module
        .items()
        .map(|(owner, _)| owner)
        .collect::<Vec<_>>()
        .into_boxed_slice();
    assert!(contributors.len() >= 3);
    (declarations, event_type, contributors)
}

#[test]
fn worklist_rejects_the_first_charge_beyond_its_exact_bound() {
    let mut worklist = PreparedExecutableIngressWorklist {
        facts: PreparedExecutableIngressFacts::default(),
        pending: BTreeMap::new(),
        adjacency: BTreeMap::new(),
        cached_edge_count: 0,
        roots: PreparedEntryRootCatalog::default(),
        includes: BTreeMap::new(),
        limits: StatementPreparationLimits::for_test(0, 0, 0, 0, 1),
        work: 0,
    };

    assert_eq!(worklist.charge(1), Ok(()));
    assert!(matches!(
        worklist.charge(1),
        Err(FinalSemanticAnalysisError::AccountingOverflow)
    ));
    assert_eq!(worklist.work, 1);
}

#[test]
fn admission_charges_every_new_row_before_write_and_is_atomic_at_n_plus_one() {
    let (declaration, event_type, contributors) = ingress_fixture_ids();
    let digest = SemanticTypeDigest::from_bytes([0xA5; 32]);
    let mut exact = PreparedExecutableIngressWorklist {
        facts: PreparedExecutableIngressFacts::default(),
        pending: BTreeMap::new(),
        adjacency: BTreeMap::new(),
        cached_edge_count: 0,
        roots: PreparedEntryRootCatalog::default(),
        includes: BTreeMap::new(),
        limits: StatementPreparationLimits::for_test(1, 0, 2, 0, 4),
        work: 0,
    };

    exact
        .admit_one(declaration.clone(), event_type, digest, contributors[0])
        .expect("declaration, contributor, and pending rows fit exact work bound");
    assert_eq!(exact.work, 4);
    assert_eq!(exact.facts.declarations.len(), 1);
    assert_eq!(exact.pending.len(), 1);
    assert_eq!(
        exact
            .facts
            .contributors(&declaration)
            .expect("retained declaration contributors"),
        &BTreeSet::from([contributors[0]])
    );

    exact
        .admit_one(declaration.clone(), event_type, digest, contributors[0])
        .expect("duplicate contributor is a zero-write admission");
    assert_eq!(exact.work, 4);

    assert!(matches!(
        exact.admit_one(declaration.clone(), event_type, digest, contributors[1]),
        Err(FinalSemanticAnalysisError::AccountingOverflow)
    ));
    assert_eq!(exact.work, 4);
    assert_eq!(exact.facts.declarations.len(), 1);
    assert_eq!(exact.pending.len(), 1);
    assert_eq!(
        exact
            .facts
            .contributors(&declaration)
            .expect("failed N+1 admission preserves contributors"),
        &BTreeSet::from([contributors[0]])
    );
}

#[test]
fn admission_failure_never_publishes_an_empty_declaration_row() {
    let (declaration, event_type, contributors) = ingress_fixture_ids();
    let digest = SemanticTypeDigest::from_bytes([0x5A; 32]);
    let mut worklist = PreparedExecutableIngressWorklist {
        facts: PreparedExecutableIngressFacts::default(),
        pending: BTreeMap::new(),
        adjacency: BTreeMap::new(),
        cached_edge_count: 0,
        roots: PreparedEntryRootCatalog::default(),
        includes: BTreeMap::new(),
        limits: StatementPreparationLimits::for_test(1, 0, 1, 0, 2),
        work: 0,
    };

    assert!(matches!(
        worklist.admit_one(declaration, event_type, digest, contributors[0]),
        Err(FinalSemanticAnalysisError::AccountingOverflow)
    ));
    assert_eq!(worklist.work, 0);
    assert!(worklist.facts.declarations.is_empty());
    assert!(worklist.pending.is_empty());
}

#[test]
fn declaration_limit_accepts_exact_n_and_rejects_n_plus_one_atomically() {
    let (declarations, event_type, contributors) = ingress_fixture_declarations();
    let digest = SemanticTypeDigest::from_bytes([0x17; 32]);
    let mut worklist = PreparedExecutableIngressWorklist {
        facts: PreparedExecutableIngressFacts::default(),
        pending: BTreeMap::new(),
        adjacency: BTreeMap::new(),
        cached_edge_count: 0,
        roots: PreparedEntryRootCatalog::default(),
        includes: BTreeMap::new(),
        limits: StatementPreparationLimits::for_test(2, 0, 1, 0, 16),
        work: 0,
    };

    for declaration in declarations.iter().take(2).cloned() {
        worklist
            .admit_one(declaration, event_type, digest, contributors[0])
            .expect("exact declaration bound admits N rows");
    }
    let before_work = worklist.work;
    let before_declarations = worklist
        .facts
        .declarations
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    let before_pending = worklist
        .pending
        .iter()
        .map(|(declaration, delta)| (declaration.clone(), delta.clone()))
        .collect::<Vec<_>>();
    assert!(matches!(
        worklist.admit_one(declarations[2].clone(), event_type, digest, contributors[0],),
        Err(FinalSemanticAnalysisError::AccountingOverflow)
    ));
    assert_eq!(worklist.work, before_work);
    assert_eq!(
        worklist
            .facts
            .declarations
            .keys()
            .cloned()
            .collect::<Vec<_>>(),
        before_declarations
    );
    assert_eq!(
        worklist
            .pending
            .iter()
            .map(|(declaration, delta)| (declaration.clone(), delta.clone()))
            .collect::<Vec<_>>(),
        before_pending,
        "N+1 declaration admission publishes no partial facts"
    );
}

#[test]
fn contributor_limit_accepts_exact_n_and_rejects_n_plus_one_atomically() {
    let (declarations, event_type, contributors) = ingress_fixture_declarations();
    let declaration = declarations[0].clone();
    let digest = SemanticTypeDigest::from_bytes([0x28; 32]);
    let mut worklist = PreparedExecutableIngressWorklist {
        facts: PreparedExecutableIngressFacts::default(),
        pending: BTreeMap::new(),
        adjacency: BTreeMap::new(),
        cached_edge_count: 0,
        roots: PreparedEntryRootCatalog::default(),
        includes: BTreeMap::new(),
        limits: StatementPreparationLimits::for_test(1, 0, 2, 0, 6),
        work: 0,
    };

    worklist
        .admit_one(declaration.clone(), event_type, digest, contributors[0])
        .expect("first contributor fits the exact contributor bound");
    worklist
        .admit_one(declaration.clone(), event_type, digest, contributors[1])
        .expect("second contributor fits the exact contributor bound");
    let before_work = worklist.work;
    let before_contributors = worklist
        .facts
        .contributors(&declaration)
        .expect("retained contributor set")
        .iter()
        .copied()
        .collect::<Vec<_>>();
    let before_pending = worklist
        .pending
        .iter()
        .map(|(declaration, delta)| (declaration.clone(), delta.clone()))
        .collect::<Vec<_>>();
    assert!(matches!(
        worklist.admit_one(declaration.clone(), event_type, digest, contributors[2]),
        Err(FinalSemanticAnalysisError::AccountingOverflow)
    ));
    assert_eq!(worklist.work, before_work);
    assert_eq!(
        worklist
            .facts
            .contributors(&declaration)
            .expect("failed admission retains contributor set")
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        before_contributors
    );
    assert_eq!(
        worklist
            .pending
            .iter()
            .map(|(declaration, delta)| (declaration.clone(), delta.clone()))
            .collect::<Vec<_>>(),
        before_pending,
        "N+1 contributor admission publishes no partial facts"
    );
}

#[test]
fn p21_equal_entry_ingress_contributors_converge_independent_of_order() {
    let (declarations, event_type, contributors) = ingress_fixture_declarations();
    let digest = SemanticTypeDigest::from_bytes([0x39; 32]);
    let limits = StatementPreparationLimits::for_test(1, 0, 2, 0, 6);

    let mut forward = PreparedExecutableIngressWorklist {
        facts: PreparedExecutableIngressFacts::default(),
        pending: BTreeMap::new(),
        adjacency: BTreeMap::new(),
        cached_edge_count: 0,
        roots: PreparedEntryRootCatalog::default(),
        includes: BTreeMap::new(),
        limits,
        work: 0,
    };
    forward
        .admit_one(declarations[0].clone(), event_type, digest, contributors[0])
        .expect("first stateful Entry root is admitted");
    forward
        .admit_one(declarations[0].clone(), event_type, digest, contributors[1])
        .expect("second equal-typed Entry root joins the declaration");

    let mut reverse = PreparedExecutableIngressWorklist {
        facts: PreparedExecutableIngressFacts::default(),
        pending: BTreeMap::new(),
        adjacency: BTreeMap::new(),
        cached_edge_count: 0,
        roots: PreparedEntryRootCatalog::default(),
        includes: BTreeMap::new(),
        limits,
        work: 0,
    };
    reverse
        .admit_one(declarations[0].clone(), event_type, digest, contributors[1])
        .expect("second Entry root is admitted first");
    reverse
        .admit_one(declarations[0].clone(), event_type, digest, contributors[0])
        .expect("first Entry root joins in reverse traversal order");

    assert_eq!(forward.work, reverse.work);
    assert_eq!(forward.pending, reverse.pending);
    assert_eq!(
        forward
            .facts
            .contributors(&declarations[0])
            .expect("converged declaration retains both Entry contributors"),
        reverse
            .facts
            .contributors(&declarations[0])
            .expect("reverse traversal retains both Entry contributors")
    );
    assert_eq!(
        forward.facts.event_digest(&declarations[0]),
        Some(digest),
        "the converged declaration keeps one Event semantic digest"
    );
}

#[test]
fn n21_pending_retains_only_new_contributor_deltas_and_adjacency_is_one_shot() {
    let (declaration, event_type, contributors) = ingress_fixture_ids();
    let digest = SemanticTypeDigest::from_bytes([0x4A; 32]);
    let mut worklist = PreparedExecutableIngressWorklist {
        facts: PreparedExecutableIngressFacts::default(),
        pending: BTreeMap::new(),
        adjacency: BTreeMap::new(),
        cached_edge_count: 0,
        roots: PreparedEntryRootCatalog::default(),
        includes: BTreeMap::new(),
        limits: StatementPreparationLimits::for_test(1, 2, 2, 0, 32),
        work: 0,
    };

    worklist
        .admit_one(declaration.clone(), event_type, digest, contributors[0])
        .expect("first contributor creates one pending delta");
    assert_eq!(
        worklist.pending.get(&declaration),
        Some(&BTreeSet::from([contributors[0]]))
    );
    let (popped, delta) = worklist
        .pop_pending()
        .expect("queue pop is within the exact bound")
        .expect("first contributor is queued");
    assert_eq!(popped, declaration);
    assert_eq!(delta.as_ref(), &[contributors[0]]);
    assert!(worklist.pending.is_empty());
    worklist
        .facts
        .declarations
        .get_mut(&declaration)
        .expect("admitted declaration")
        .checked = true;

    worklist
        .cache_adjacency(
            declaration.clone(),
            vec![declaration.clone(), declaration.clone()].into_boxed_slice(),
        )
        .expect("typed adjacency is cached once");
    assert_eq!(worklist.cached_edge_count, 2);
    let before_work = worklist.work;
    let before_cached_edge_count = worklist.cached_edge_count;
    assert!(matches!(
        worklist.cache_adjacency(declaration.clone(), Box::new([])),
        Err(FinalSemanticAnalysisError::WrongPayloadFamily)
    ));
    assert_eq!(worklist.work, before_work);
    assert_eq!(worklist.cached_edge_count, before_cached_edge_count);

    worklist
        .admit_one(declaration.clone(), event_type, digest, contributors[0])
        .expect("repeated contributor is not a new pending delta");
    assert!(worklist.pending.is_empty());
    worklist
        .admit_one(declaration.clone(), event_type, digest, contributors[1])
        .expect("new contributor creates only its delta");
    assert_eq!(
        worklist.pending.get(&declaration),
        Some(&BTreeSet::from([contributors[1]]))
    );
}

#[test]
fn n21_exact_work_bound_accepts_and_one_over_fails_before_propagation_write() {
    let (declaration, event_type, contributors) = ingress_fixture_ids();
    let digest = SemanticTypeDigest::from_bytes([0x5B; 32]);
    // D=1, S=0, X=0, M=1, K=1: D + 2S + X + M + 4*(D*K) + M*K = 7.
    let mut exact = PreparedExecutableIngressWorklist {
        facts: PreparedExecutableIngressFacts::default(),
        pending: BTreeMap::new(),
        adjacency: BTreeMap::new(),
        cached_edge_count: 0,
        roots: PreparedEntryRootCatalog::default(),
        includes: BTreeMap::new(),
        limits: StatementPreparationLimits::for_test(1, 1, 1, 0, 7),
        work: 0,
    };
    exact
        .admit_one(declaration.clone(), event_type, digest, contributors[0])
        .expect("exact declaration/fact/delta/push units fit");
    let (_, delta) = exact
        .pop_pending()
        .expect("exact queue-pop unit fits")
        .expect("root is queued");
    exact
        .facts
        .declarations
        .get_mut(&declaration)
        .expect("admitted declaration")
        .checked = true;
    exact
        .cache_adjacency(
            declaration.clone(),
            vec![declaration.clone()].into_boxed_slice(),
        )
        .expect("exact adjacency extraction unit fits");
    assert_eq!(exact.cached_edge_count, 1);
    exact
        .charge(1)
        .expect("exact edge/contributor propagation unit fits");
    exact
        .admit_one(declaration.clone(), event_type, digest, delta[0])
        .expect("duplicate propagation does not write another contributor");
    assert_eq!(exact.work, 7);

    let mut one_over = PreparedExecutableIngressWorklist {
        facts: PreparedExecutableIngressFacts::default(),
        pending: BTreeMap::new(),
        adjacency: BTreeMap::new(),
        cached_edge_count: 0,
        roots: PreparedEntryRootCatalog::default(),
        includes: BTreeMap::new(),
        limits: StatementPreparationLimits::for_test(1, 1, 1, 0, 6),
        work: 0,
    };
    one_over
        .admit_one(declaration.clone(), event_type, digest, contributors[0])
        .expect("all units through queue pop fit at six");
    let _ = one_over
        .pop_pending()
        .expect("queue pop is charged before removal")
        .expect("root is queued");
    one_over
        .facts
        .declarations
        .get_mut(&declaration)
        .expect("admitted declaration")
        .checked = true;
    one_over
        .cache_adjacency(
            declaration.clone(),
            vec![declaration.clone()].into_boxed_slice(),
        )
        .expect("adjacency charge is the sixth unit");
    assert_eq!(one_over.cached_edge_count, 1);
    let before_contributors = one_over
        .facts
        .contributors(&declaration)
        .expect("admitted declaration contributors")
        .clone();
    let before_adjacency = one_over.adjacency.clone();
    assert!(matches!(
        one_over.charge(1),
        Err(FinalSemanticAnalysisError::AccountingOverflow)
    ));
    assert_eq!(one_over.work, 6);
    assert_eq!(one_over.cached_edge_count, 1);
    assert_eq!(
        one_over
            .facts
            .contributors(&declaration)
            .expect("failed propagation retains contributors"),
        &before_contributors
    );
    assert_eq!(one_over.adjacency, before_adjacency);
}

#[test]
fn recomputed_admission_uses_the_same_exact_atomic_bound() {
    let (declaration, event_type, contributors) = ingress_fixture_ids();
    let digest = SemanticTypeDigest::from_bytes([0xC3; 32]);
    let limits = StatementPreparationLimits::for_test(1, 0, 1, 0, 2);
    let mut facts = PreparedExecutableIngressFacts::default();
    let mut pending = BTreeMap::new();
    let mut work = 0;

    assert!(matches!(
        admit_recomputed_one(
            &mut facts,
            &mut pending,
            declaration,
            event_type,
            digest,
            contributors[0],
            &mut work,
            limits,
        ),
        Err(FinalSemanticAnalysisError::AccountingOverflow)
    ));
    assert_eq!(work, 0);
    assert!(facts.declarations.is_empty());
    assert!(pending.is_empty());
}

#[test]
fn preallocation_accepts_exact_edge_inventory_and_rejects_n_plus_one() {
    let fixture =
        crate::final_analysis::tests::fixture("flow @flow.opening opening { return unit }\n", None);
    let executable = fixture.project.executable_view().expect("executable HIR");
    let topology = executable
        .accept_symbol_generation(&fixture.symbols)
        .expect("accepted symbol generation")
        .into_evaluation_topology()
        .expect("evaluation topology");
    let body = topology
        .modules()
        .iter()
        .flat_map(|module| module.entries())
        .filter_map(|entry| entry.body())
        .next()
        .expect("flow executable body");
    let declaration = body.declaration().clone();
    let CallableDeclarationKey::Flow(target) = &declaration else {
        panic!("fixture body must be Flow-owned");
    };
    let target = target.clone();
    let module = executable
        .module(&arcweft_lang_syntax::ast::module_path::CanonicalModulePath::crate_root())
        .expect("root module");
    let statement = module
        .statements()
        .next()
        .map(|(owner, _)| owner)
        .expect("flow statement");
    let inventory = PreparedExecutableDeclarationInventory {
        declarations: BTreeMap::from([(
            declaration.clone(),
            PreparedExecutableDeclaration {
                declaration: declaration.clone(),
                module: module.module_id(),
                item: body.source_item(),
                statements: vec![statement].into_boxed_slice(),
                expressions: Box::new([]),
            },
        )]),
    };
    let include = PreparedIncludeFlowProof::new(statement, declaration, target.clone());

    let rejected = PreparedExecutableIngressWorklist::new(
        &inventory,
        PreparedEntryRootCatalog::default(),
        BTreeMap::from([(statement, include)]),
        StatementPreparationLimits::for_test(1, 0, 0, 1, 0),
    );
    assert!(matches!(
        rejected,
        Err(FinalSemanticAnalysisError::AccountingOverflow)
    ));

    let include = PreparedIncludeFlowProof::new(statement, body.declaration().clone(), target);
    let exact = PreparedExecutableIngressWorklist::new(
        &inventory,
        PreparedEntryRootCatalog::default(),
        BTreeMap::from([(statement, include)]),
        StatementPreparationLimits::for_test(1, 1, 0, 1, 0),
    )
    .expect("exact preallocated edge inventory");
    assert!(exact.facts.declarations.is_empty());
    assert_eq!(exact.includes.len(), 1);
    assert_eq!(exact.work, 0);
}

#[test]
fn preallocation_accepts_exact_contextual_statement_inventory_and_rejects_n_plus_one() {
    let fixture =
        crate::final_analysis::tests::fixture("flow @flow.opening opening { return unit }\n", None);
    let executable = fixture.project.executable_view().expect("executable HIR");
    let topology = executable
        .accept_symbol_generation(&fixture.symbols)
        .expect("accepted symbol generation")
        .into_evaluation_topology()
        .expect("evaluation topology");
    let body = topology
        .modules()
        .iter()
        .flat_map(|module| module.entries())
        .filter_map(|entry| entry.body())
        .next()
        .expect("flow executable body");
    let declaration = body.declaration().clone();
    let module = executable
        .module(&arcweft_lang_syntax::ast::module_path::CanonicalModulePath::crate_root())
        .expect("root module");
    let statement = module
        .statements()
        .next()
        .map(|(owner, _)| owner)
        .expect("flow statement");
    let inventory = PreparedExecutableDeclarationInventory {
        declarations: BTreeMap::from([(
            declaration.clone(),
            PreparedExecutableDeclaration {
                declaration,
                module: module.module_id(),
                item: body.source_item(),
                statements: vec![statement].into_boxed_slice(),
                expressions: Box::new([]),
            },
        )]),
    };

    assert!(matches!(
        PreparedExecutableIngressWorklist::new(
            &inventory,
            PreparedEntryRootCatalog::default(),
            BTreeMap::new(),
            StatementPreparationLimits::for_test(1, 0, 0, 0, 0),
        ),
        Err(FinalSemanticAnalysisError::AccountingOverflow)
    ));
    let exact = PreparedExecutableIngressWorklist::new(
        &inventory,
        PreparedEntryRootCatalog::default(),
        BTreeMap::new(),
        StatementPreparationLimits::for_test(1, 0, 0, 1, 0),
    )
    .expect("exact contextual statement inventory is accepted");
    assert_eq!(exact.work, 0);
    assert_eq!(exact.facts.declarations.len(), 0);
}

#[test]
fn n21_work_bound_charges_all_expression_candidates_and_include_probes() {
    let fixture =
        crate::final_analysis::tests::fixture("flow @flow.opening opening { return unit }\n", None);
    let executable = fixture.project.executable_view().expect("executable HIR");
    let topology = executable
        .accept_symbol_generation(&fixture.symbols)
        .expect("accepted symbol generation")
        .into_evaluation_topology()
        .expect("evaluation topology");
    let body = topology
        .modules()
        .iter()
        .flat_map(|module| module.entries())
        .filter_map(|entry| entry.body())
        .next()
        .expect("flow executable body");
    let declaration = body.declaration().clone();
    let module = executable
        .module(&arcweft_lang_syntax::ast::module_path::CanonicalModulePath::crate_root())
        .expect("root module");
    let statement = module
        .statements()
        .next()
        .map(|(owner, _)| owner)
        .expect("flow statement");
    let expression = module
        .expressions()
        .next()
        .map(|(owner, _)| owner)
        .expect("return expression candidate");
    let inventory = PreparedExecutableDeclarationInventory {
        declarations: BTreeMap::from([(
            declaration.clone(),
            PreparedExecutableDeclaration {
                declaration,
                module: module.module_id(),
                item: body.source_item(),
                statements: vec![statement].into_boxed_slice(),
                expressions: vec![expression].into_boxed_slice(),
            },
        )]),
    };
    let limits =
        StatementPreparationLimits::production(&inventory, &PreparedEntryRootCatalog::default(), 2)
            .expect("bounded inventory arithmetic");

    // D=1, S=1, X=1, I=2, M=X+I=3, K=0: 1 + 2 + 1 + 3 = 7.
    assert_eq!(limits.max_contextual_statements, 1);
    assert_eq!(limits.max_edges, 3);
    assert_eq!(limits.max_work, 7);
}

#[test]
fn arithmetic_overflow_does_not_mutate_the_work_counter() {
    let mut worklist = PreparedExecutableIngressWorklist {
        facts: PreparedExecutableIngressFacts::default(),
        pending: BTreeMap::new(),
        adjacency: BTreeMap::new(),
        cached_edge_count: 0,
        roots: PreparedEntryRootCatalog::default(),
        includes: BTreeMap::new(),
        limits: StatementPreparationLimits::for_test(
            u64::MAX,
            u64::MAX,
            u64::MAX,
            u64::MAX,
            u64::MAX,
        ),
        work: u64::MAX,
    };
    assert!(matches!(
        worklist.charge(1),
        Err(FinalSemanticAnalysisError::AccountingOverflow)
    ));
    assert_eq!(worklist.work, u64::MAX);
}

#[test]
fn recomputed_arithmetic_overflow_does_not_mutate_the_work_counter() {
    let limits =
        StatementPreparationLimits::for_test(u64::MAX, u64::MAX, u64::MAX, u64::MAX, u64::MAX);
    let mut work = u64::MAX;
    assert!(matches!(
        charge_recomputed_work(&mut work, limits),
        Err(FinalSemanticAnalysisError::AccountingOverflow)
    ));
    assert_eq!(work, u64::MAX);
    assert!(matches!(
        require_recomputed_charge_capacity(u64::MAX, 1, limits),
        Err(FinalSemanticAnalysisError::AccountingOverflow)
    ));
}

#[test]
fn completed_ingress_seal_splits_into_two_consuming_phase_seals() {
    let seal = PreparedExecutableIngressSeal::empty_for_call_free_fixture();

    let (entry, statement) = seal.into_phase_seals();
    let (facts, roots, events) = entry.into_parts();
    let (includes, scrutinees) = statement.into_parts();
    assert!(facts.declarations.is_empty());
    assert!(roots.is_empty());
    assert!(events.is_empty());
    assert!(includes.is_empty());
    assert!(scrutinees.is_empty());
}
