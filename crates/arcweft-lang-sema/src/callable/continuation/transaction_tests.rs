use super::*;
use crate::effect_row::prepared::{
    PreparedEffectInvariant, PreparedEffectRow, PreparedEffectSchemeRef,
};
use crate::effect_row::{DecisionWork, EffectPredicate, EffectRow};
use crate::effects::EffectSet;

#[derive(Debug, Eq, PartialEq, thiserror::Error)]
#[error("relation transfer budget exhausted")]
struct Exhausted;

struct Work(usize);

impl DecisionControl for Work {
    type Error = Exhausted;

    fn charge(&mut self, _: DecisionWork) -> Result<(), Self::Error> {
        self.0 = self.0.checked_sub(1).ok_or(Exhausted)?;
        Ok(())
    }
}

fn graph(
    work: &mut Work,
) -> (
    PreparedCallGraph<()>,
    PreparedEffectSchemeRef,
    [CheckedCallSite; 2],
) {
    let fixture = crate::final_analysis::tests::fixture("fn caller() { 1; 2; }", None);
    let module = fixture
        .project
        .analysis_view()
        .unwrap()
        .module(&arcweft_lang_syntax::ast::module_path::CanonicalModulePath::crate_root())
        .unwrap();
    let mut expressions = module.expressions();
    let sites = std::array::from_fn(|_| CheckedCallSite::HirCall(expressions.next().unwrap().0));
    let mut declarations = PreparedCallableEffectDeclarations::declare(
        std::iter::empty::<(
            &super::super::CheckedCallableId,
            &super::super::CallableSignatureSchema,
        )>(),
        work,
    )
    .unwrap();
    let scheme = declarations
        .relation()
        .0
        .declare(vec![], EffectPredicate::unconstrained(), work)
        .unwrap();
    let mut graph = PreparedCallGraph::new();
    graph.initialize_effect_declarations(declarations).unwrap();
    (graph, scheme, sites)
}

fn prepare_body(
    graph: &mut PreparedCallGraph<()>,
    scheme: &PreparedEffectSchemeRef,
    work: &mut Work,
) -> PreparedEffectRow {
    let relation = graph.effect_declarations_mut().unwrap().relation().0;
    let row = relation.application(scheme, vec![], work).unwrap();
    relation
        .prepare_body(
            scheme,
            PreparedEffectRow::Canonical(EffectRow::closed(
                EffectSet::from_labels(["fs.read"]).unwrap(),
            )),
            vec![],
            work,
        )
        .unwrap();
    row
}

fn retained(graph: &mut PreparedCallGraph<()>, row: &PreparedEffectRow) -> bool {
    graph
        .effect_declarations_mut()
        .unwrap()
        .relation()
        .0
        .validate_retained_row(row)
        .is_ok()
}

#[test]
fn outer_rollback_removes_nested_committed_call_nodes_and_original_body_writes() {
    let mut work = Work(100_000);
    let (mut graph, scheme, [site, _]) = graph(&mut work);
    let outer = graph.begin_delta(&mut work).unwrap();
    let inner = graph.begin_delta(&mut work).unwrap();
    graph
        .seal_unselected(site, Vec::new().into_boxed_slice(), ())
        .unwrap();
    let row = prepare_body(&mut graph, &scheme, &mut work);
    let (error, outer) = graph.rollback_delta(outer).unwrap_err().into_parts();
    assert!(matches!(
        error,
        PreparedCallGraphMutationFailure::Invariant(
            CallConstraintInvariant::PreparedGraphDeltaOrder
        )
    ));
    assert!(retained(&mut graph, &row));
    assert_eq!(graph.sites().count(), 1);
    graph.commit_delta(inner).unwrap();
    graph.rollback_delta(outer).unwrap();
    assert!(!retained(&mut graph, &row));
    assert_eq!(graph.sites().count(), 0);
    let retry = graph.begin_delta(&mut work).unwrap();
    prepare_body(&mut graph, &scheme, &mut work);
    graph.rollback_delta(retry).unwrap();
}

#[test]
fn aborted_extraction_and_restore_keep_both_affine_payloads_for_retry() {
    let mut work = Work(100_000);
    let (mut graph, scheme, [site, _]) = graph(&mut work);
    let source = graph.begin_delta(&mut work).unwrap();
    graph
        .seal_unselected(site, Vec::new().into_boxed_slice(), ())
        .unwrap();
    let row = prepare_body(&mut graph, &scheme, &mut work);
    let failure = match graph.extract_delta(source, &mut Work(0)) {
        Err(failure) => failure,
        Ok(_) => panic!("zero-budget extraction must fail"),
    };
    let (error, source) = failure.into_parts();
    assert!(matches!(
        error,
        PreparedCallGraphMutationFailure::Control(Exhausted)
    ));
    assert!(retained(&mut graph, &row));
    assert_eq!(graph.sites().count(), 1);
    let delta = graph.extract_delta(source, &mut work).unwrap();
    assert!(!retained(&mut graph, &row));
    assert_eq!(graph.sites().count(), 0);
    let target = graph.begin_delta(&mut work).unwrap();
    let (error, delta) = graph
        .restore_deltas(&target, vec![delta], &mut Work(0))
        .unwrap_err()
        .into_parts();
    assert!(matches!(
        error,
        PreparedCallGraphMutationFailure::Control(Exhausted)
    ));
    assert!(!retained(&mut graph, &row));
    assert_eq!(graph.sites().count(), 0);
    graph.restore_deltas(&target, delta, &mut work).unwrap();
    assert!(retained(&mut graph, &row));
    assert_eq!(graph.sites().count(), 1);
    let closed = graph
        .effect_declarations_mut()
        .unwrap()
        .relation()
        .0
        .solve_principal(&mut work)
        .unwrap();
    assert_eq!(
        closed.resolve_row(&row, &mut work).unwrap(),
        EffectRow::closed(EffectSet::from_labels(["fs.read"]).unwrap())
    );
    graph.rollback_delta(target).unwrap();
    assert!(!retained(&mut graph, &row));
    assert_eq!(graph.sites().count(), 0);
}

#[test]
fn batch_restore_retries_after_relation_preflight_aborts_and_rolls_back_as_one() {
    let mut work = Work(100_000);
    let (mut graph, first_scheme, [first_site, second_site]) = graph(&mut work);
    let second_scheme = graph
        .effect_declarations_mut()
        .unwrap()
        .relation()
        .0
        .declare(vec![], EffectPredicate::unconstrained(), &mut work)
        .unwrap();
    let mut sources = Vec::new();
    let mut rows = Vec::new();
    for (site, scheme) in [(first_site, first_scheme), (second_site, second_scheme)] {
        let source = graph.begin_delta(&mut work).unwrap();
        graph
            .seal_unselected(site, Vec::new().into_boxed_slice(), ())
            .unwrap();
        rows.push(prepare_body(&mut graph, &scheme, &mut work));
        sources.push(graph.extract_delta(source, &mut work).unwrap());
    }
    assert!(rows.iter().all(|row| !retained(&mut graph, row)));
    let target = graph.begin_delta(&mut work).unwrap();
    let mut preflight_budget = Work(100_000);
    graph
        .preflight_restore_deltas(&target, &sources, &mut preflight_budget)
        .unwrap();
    let high_graph_cost = 100_000 - preflight_budget.0;
    let (error, sources) = graph
        .restore_deltas(&target, sources, &mut Work(high_graph_cost + 1))
        .unwrap_err()
        .into_parts();
    assert!(matches!(
        error,
        PreparedCallGraphMutationFailure::Control(Exhausted)
    ));
    assert_eq!(sources.len(), 2);
    assert_eq!(sources[0].site_states().unwrap()[0].0, first_site);
    assert_eq!(sources[1].site_states().unwrap()[0].0, second_site);
    assert_eq!(graph.sites().count(), 0);
    assert!(rows.iter().all(|row| !retained(&mut graph, row)));
    graph.restore_deltas(&target, sources, &mut work).unwrap();
    assert_eq!(graph.sites().count(), 2);
    assert!(rows.iter().all(|row| retained(&mut graph, row)));
    let closed = graph
        .effect_declarations_mut()
        .unwrap()
        .relation()
        .0
        .solve_principal(&mut work)
        .unwrap();
    for row in &rows {
        assert_eq!(
            closed.resolve_row(row, &mut work).unwrap(),
            EffectRow::closed(EffectSet::from_labels(["fs.read"]).unwrap()),
        );
    }
    graph.rollback_delta(target).unwrap();
    assert_eq!(graph.sites().count(), 0);
    assert!(rows.iter().all(|row| !retained(&mut graph, row)));
}

#[test]
fn conflicting_relation_permits_do_not_publish_disjoint_call_sites() {
    let mut work = Work(100_000);
    let (mut graph, scheme, [first_site, second_site]) = graph(&mut work);
    let mut sources = Vec::new();
    let mut rows = Vec::new();
    for site in [first_site, second_site] {
        let source = graph.begin_delta(&mut work).unwrap();
        graph
            .seal_unselected(site, Vec::new().into_boxed_slice(), ())
            .unwrap();
        rows.push(prepare_body(&mut graph, &scheme, &mut work));
        sources.push(graph.extract_delta(source, &mut work).unwrap());
    }
    let target = graph.begin_delta(&mut work).unwrap();
    let (error, mut sources) = graph
        .restore_deltas(&target, sources, &mut work)
        .unwrap_err()
        .into_parts();
    assert!(matches!(
        error,
        PreparedCallGraphMutationFailure::Invariant(CallConstraintInvariant::EffectTransfer(
            PreparedEffectFailure::Invariant(PreparedEffectInvariant::ReadScopeConflict)
        ))
    ));
    assert_eq!(sources.len(), 2);
    assert_eq!(graph.sites().count(), 0);
    assert!(rows.iter().all(|row| !retained(&mut graph, row)));
    let second = sources.pop().unwrap();
    graph
        .restore_deltas(&target, vec![second], &mut work)
        .unwrap();
    assert_eq!(graph.sites().collect::<Vec<_>>(), [second_site]);
    assert!(!retained(&mut graph, &rows[0]));
    assert!(retained(&mut graph, &rows[1]));
    graph.rollback_delta(target).unwrap();
}
