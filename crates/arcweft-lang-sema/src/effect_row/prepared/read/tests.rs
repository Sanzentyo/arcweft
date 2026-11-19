use super::*;
use crate::{
    effect_row::prepared::test_support::{Budget, Exhausted, budget, literal, open},
    effects::EffectSet,
    types::{GenericBinder, GenericScope},
};
use std::sync::atomic::{AtomicBool, Ordering};

struct Candidate {
    delta: PreparedEffectDelta,
    added: PreparedEffectSchemeRef,
    row: PreparedEffectRow,
    predicate: PreparedEffectPredicate,
}

fn candidate(
    graph: &mut PreparedEffectGraph,
    original: &PreparedEffectSchemeRef,
    parameters: &[GenericEffectReference],
    domain: &EffectPredicate,
) -> Candidate {
    let mut work = budget();
    let mut checkpoint = graph.checkpoint(&mut work).unwrap();
    let added = graph
        .declare(parameters.to_vec(), domain.clone(), &mut work)
        .unwrap();
    let row = graph
        .application(&added, parameters.iter().map(open).collect(), &mut work)
        .unwrap();
    let predicate = graph
        .relate(
            PreparedEffectRelation::AdmittedSubset,
            row.clone(),
            literal(&["fs.read"]),
            &mut work,
        )
        .unwrap();
    graph
        .add_scoped_obligation(
            parameters.to_vec(),
            PreparedEffectPredicate::Canonical(domain.clone()),
            predicate.clone(),
            &mut work,
        )
        .unwrap();
    graph
        .prepare_body(original, literal(&["log.write"]), vec![], &mut work)
        .unwrap();
    let delta = graph.extract_delta(&mut checkpoint, &mut work).unwrap();
    Candidate {
        delta,
        added,
        row,
        predicate,
    }
}

fn header() -> (Vec<GenericEffectReference>, EffectPredicate) {
    let scope = GenericScope::default().with_binder(GenericBinder::new(0, 0, 2));
    let parameters = vec![
        scope.bound_effect(0, 0).unwrap(),
        scope.bound_effect(0, 1).unwrap(),
    ];
    let domain = EffectFormula::literal(EffectSet::new(), Some(parameters[0].clone()))
        .subset(
            &EffectFormula::literal(EffectSet::from_labels(["fs.read"]).unwrap(), None),
            &mut budget(),
        )
        .unwrap();
    (parameters, domain)
}

#[test]
fn parameter_reads_admit_actuals_and_unsubstituted_references_without_reading_bodies() {
    let mut graph = PreparedEffectGraph::default();
    let mut work = budget();
    let scope = GenericScope::default().with_binder(GenericBinder::new(0, 0, 4));
    let [formal, retained, actual, other] = [0, 1, 2, 3].map(|i| scope.bound_effect(0, i).unwrap());
    let declaration = graph
        .declare(
            vec![formal.clone()],
            EffectPredicate::unconstrained(),
            &mut work,
        )
        .unwrap();
    let application = graph
        .application(&declaration, vec![open(&actual)], &mut work)
        .unwrap();
    let formula = EffectFormula::variable(formal.clone(), &mut work)
        .unwrap()
        .union(
            &EffectFormula::variable(retained.clone(), &mut work).unwrap(),
            &mut work,
        )
        .unwrap();
    let substituted = graph
        .apply_formula(
            formula.clone(),
            BTreeMap::from([(formal.clone(), application)]),
            &mut work,
        )
        .unwrap();
    let unknown = PreparedEffectRow::Canonical(EffectRow::unknown());
    let choice = graph.choice(&mut work).unwrap();
    assert!(matches!(
        graph.select_row(&choice, substituted.clone(), unknown.clone(), &mut work),
        Err(PreparedEffectFailure::Row(EffectRowError::UnknownRow))
    ));
    graph
        .read()
        .visit_row_parameters(&unknown, &mut work, &mut |_, _| {
            panic!("an omitted row has no declared references")
        })
        .unwrap();
    let row = graph
        .select_row(&choice, substituted.clone(), literal(&[]), &mut work)
        .unwrap();
    let row = graph.union(vec![row, substituted], &mut work).unwrap();
    let expected = BTreeSet::from([retained.clone(), actual.clone()]);
    for prepared in [false, true] {
        if prepared {
            graph
                .prepare_body(&declaration, open(&formal), vec![], &mut work)
                .unwrap();
        }
        let state = graph.state;
        let terms = graph.retained_terms();
        let mut observed = BTreeSet::new();
        graph
            .read()
            .visit_row_parameters(&row, &mut work, &mut |parameter, _| {
                observed.insert(parameter.clone());
                Ok(())
            })
            .unwrap();
        assert_eq!(observed, expected);
        assert_eq!(graph.state, state);
        assert_eq!(graph.retained_terms(), terms);
    }
    let predicate = formula
        .subset(
            &EffectFormula::variable(other.clone(), &mut work).unwrap(),
            &mut work,
        )
        .unwrap();
    let predicate = graph
        .apply_predicate(
            predicate,
            BTreeMap::from([(formal, row.clone())]),
            &mut work,
        )
        .unwrap();
    let relation = graph
        .relate(PreparedEffectRelation::Equal, row, open(&other), &mut work)
        .unwrap();
    let not = graph.not(relation.clone(), &mut work).unwrap();
    let either = graph.or(vec![predicate, not], &mut work).unwrap();
    let both = graph
        .and(vec![either.clone(), relation], &mut work)
        .unwrap();
    let selected = graph
        .select_predicate(&choice, either, both, &mut work)
        .unwrap();
    let mut observed = BTreeSet::new();
    graph
        .read()
        .visit_predicate_parameters(&selected, &mut work, &mut |parameter, _| {
            observed.insert(parameter.clone());
            Ok(())
        })
        .unwrap();
    assert_eq!(observed, BTreeSet::from([retained, actual, other]));
}

#[test]
fn parameter_reads_stream_wide_application_arguments_in_source_order() {
    let scope = GenericScope::default().with_binder(GenericBinder::new(0, 0, 32));
    let parameters = (0..16)
        .map(|index| scope.bound_effect(0, index).unwrap())
        .collect();
    let actuals: Vec<_> = (16..32)
        .rev()
        .map(|index| scope.bound_effect(0, index).unwrap())
        .collect();
    let mut graph = PreparedEffectGraph::default();
    let mut setup = budget();
    let declaration = graph
        .declare(parameters, EffectPredicate::unconstrained(), &mut setup)
        .unwrap();
    let row = graph
        .application(&declaration, actuals.iter().map(open).collect(), &mut setup)
        .unwrap();
    let mut observed = Vec::new();
    graph
        .read()
        .visit_row_parameters(&row, &mut Budget(35), &mut |parameter, _| {
            observed.push(parameter.clone());
            Ok(())
        })
        .unwrap();
    assert_eq!(observed, actuals);
}

#[test]
fn parameter_reads_use_the_exact_view_and_original_abort_control() {
    let mut graph = PreparedEffectGraph::default();
    let mut work = budget();
    let (parameters, domain) = header();
    let declaration = graph
        .declare(parameters.clone(), domain.clone(), &mut work)
        .unwrap();
    let candidate = candidate(&mut graph, &declaration, &parameters, &domain);
    let state = graph.state;
    let terms = graph.retained_terms();
    let mut visited = Vec::new();
    let view = graph.read_extracted(&candidate.delta, &mut work).unwrap();
    let mut required = Budget(4096);
    view.visit_predicate_parameters(&candidate.predicate, &mut required, &mut |parameter, _| {
        visited.push(parameter.clone());
        Ok(())
    })
    .unwrap();
    assert_eq!(visited, parameters);
    let required = 4096 - required.0;
    for available in 0..=required {
        let result = view.visit_predicate_parameters(
            &candidate.predicate,
            &mut Budget(available),
            &mut |_, _| Ok(()),
        );
        if available < required {
            assert!(matches!(
                result,
                Err(PreparedEffectFailure::Control(Exhausted))
            ));
        } else {
            result.unwrap();
        }
    }
    assert!(matches!(
        graph
            .read()
            .visit_row_parameters(&candidate.row, &mut work, &mut |_, _| Ok(())),
        Err(PreparedEffectFailure::Invariant(
            PreparedEffectInvariant::StaleReference
        ))
    ));
    assert!(matches!(
        PreparedEffectGraph::default().read().visit_row_parameters(
            &candidate.row,
            &mut work,
            &mut |_, _| Ok(())
        ),
        Err(PreparedEffectFailure::Invariant(
            PreparedEffectInvariant::ForeignIssuer
        ))
    ));
    let cancelled = AtomicBool::new(true);
    assert!(matches!(
        view.visit_row_parameters(
            &candidate.row,
            &mut Cancellation(&cancelled),
            &mut |_, _| Ok(())
        ),
        Err(PreparedEffectFailure::Control(Cancelled))
    ));
    cancelled.store(false, Ordering::Relaxed);
    assert!(matches!(
        view.visit_row_parameters(
            &candidate.row,
            &mut Cancellation(&cancelled),
            &mut |_, _| Err(Cancelled)
        ),
        Err(PreparedEffectFailure::Control(Cancelled))
    ));
    assert_eq!(graph.state, state);
    assert_eq!(graph.retained_terms(), terms);
    assert!(matches!(
        graph.scheme(&declaration).unwrap().state,
        SchemeState::Declared
    ));
}

#[test]
fn extracted_reads_keep_original_headers_and_candidate_definitions_without_publication() {
    let mut work = budget();
    let mut graph = PreparedEffectGraph::default();
    let (parameters, domain) = header();
    let original = graph
        .declare(parameters.clone(), domain.clone(), &mut work)
        .unwrap();
    let original_header = graph.declaration(&original, &mut work).unwrap();
    let owner = graph.owner(&mut work).unwrap();
    let candidate = candidate(&mut graph, &original, &parameters, &domain);
    let state = graph.state;
    let terms = graph.retained_terms();
    assert!(matches!(
        graph.scheme(&original).unwrap().state,
        SchemeState::Declared
    ));
    assert_eq!(
        graph.validate_retained_row(&candidate.row),
        Err(PreparedEffectInvariant::StaleReference)
    );
    let view = graph.read_extracted(&candidate.delta, &mut work).unwrap();
    view.validate_owner(&owner).unwrap();
    view.validate_retained_row(&candidate.row).unwrap();
    view.validate_retained_predicate(&candidate.predicate)
        .unwrap();
    assert!(matches!(
        view.scheme(&original).unwrap().state,
        SchemeState::BodyPrepared { .. }
    ));
    original_header.validate_read(view).unwrap();
    let declaration = view.declaration(&candidate.added, &mut work).unwrap();
    assert_eq!(declaration.parameters(), parameters);
    assert_eq!(declaration.domain(), &domain);
    assert!(matches!(
        view.scheme(&candidate.added).unwrap().state,
        SchemeState::Declared
    ));
    declaration.validate_read(view).unwrap();
    assert_eq!(
        declaration.validate(&graph),
        Err(PreparedEffectInvariant::StaleReference)
    );
    assert_eq!(graph.state, state);
    assert_eq!(graph.retained_terms(), terms);
    graph.restore_delta(candidate.delta, &mut work).unwrap();
    graph.validate_retained_row(&candidate.row).unwrap();
    graph
        .validate_retained_predicate(&candidate.predicate)
        .unwrap();
    declaration.validate(&graph).unwrap();
}

#[derive(Debug, thiserror::Error)]
#[error("original read cancellation")]
struct Cancelled;

struct Cancellation<'a>(&'a AtomicBool);
impl DecisionControl for Cancellation<'_> {
    type Error = Cancelled;
    fn charge(&mut self, _: DecisionWork) -> Result<(), Cancelled> {
        if self.0.load(Ordering::Relaxed) {
            Err(Cancelled)
        } else {
            Ok(())
        }
    }
}

#[test]
fn extracted_read_preflight_rejects_siblings_changed_bases_foreign_owners_and_aborts_atomically() {
    let mut work = budget();
    let mut graph = PreparedEffectGraph::default();
    let (parameters, domain) = header();
    let original = graph
        .declare(parameters.clone(), domain.clone(), &mut work)
        .unwrap();
    let first = candidate(&mut graph, &original, &parameters, &domain);
    let second = candidate(&mut graph, &original, &parameters, &domain);
    let state = graph.state;
    let mut probe = Budget(4096);
    graph.read_extracted(&first.delta, &mut probe).unwrap();
    let required = 4096 - probe.0;
    for available in 0..=required {
        let result = graph.read_extracted(&first.delta, &mut Budget(available));
        if available < required {
            assert!(matches!(
                result,
                Err(PreparedEffectFailure::Control(Exhausted))
            ));
        } else {
            result.unwrap().validate_retained_row(&first.row).unwrap();
        }
        assert_eq!(graph.state, state);
        assert_eq!(graph.retained_terms(), (0, 0));
    }
    let first_view = graph.read_extracted(&first.delta, &mut work).unwrap();
    assert_eq!(
        first_view.validate_retained_row(&second.row),
        Err(PreparedEffectInvariant::StaleReference)
    );
    assert_eq!(
        first_view.validate_retained_predicate(&second.predicate),
        Err(PreparedEffectInvariant::StaleReference)
    );
    let second_view = graph.read_extracted(&second.delta, &mut work).unwrap();
    assert_eq!(
        second_view.validate_retained_row(&first.row),
        Err(PreparedEffectInvariant::StaleReference)
    );
    let foreign = PreparedEffectGraph::default();
    assert!(matches!(
        foreign.read_extracted(&first.delta, &mut work),
        Err(PreparedEffectFailure::Invariant(
            PreparedEffectInvariant::ForeignIssuer
        ))
    ));
    let cancellation = AtomicBool::new(true);
    assert!(matches!(
        graph.read_extracted(&first.delta, &mut Cancellation(&cancellation)),
        Err(PreparedEffectFailure::Control(Cancelled))
    ));
    let mut changed = graph.checkpoint(&mut work).unwrap();
    graph
        .add_scoped_obligation(
            vec![],
            PreparedEffectPredicate::Canonical(EffectPredicate::unconstrained()),
            PreparedEffectPredicate::Canonical(EffectPredicate::unconstrained()),
            &mut work,
        )
        .unwrap();
    assert!(matches!(
        graph.read_extracted(&first.delta, &mut work),
        Err(PreparedEffectFailure::Invariant(
            PreparedEffectInvariant::DeltaBase
        ))
    ));
    graph.rollback(&mut changed).unwrap();
    graph
        .read_extracted(&first.delta, &mut work)
        .unwrap()
        .validate_retained_row(&first.row)
        .unwrap();
    assert_eq!(graph.state, state);
    graph.restore_delta(first.delta, &mut work).unwrap();
    graph.validate_retained_row(&first.row).unwrap();
}
