use super::*;
use crate::effect_row::prepared::{PreparedEffectRow, PreparedEffectSchemeRef};
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
    CheckedCallSite,
) {
    let fixture = crate::final_analysis::tests::fixture("fn caller() { 1; }", None);
    let module = fixture
        .project
        .analysis_view()
        .unwrap()
        .module(&arcweft_lang_syntax::ast::module_path::CanonicalModulePath::crate_root())
        .unwrap();
    let site = CheckedCallSite::HirCall(module.expressions().next().unwrap().0);
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
    (graph, scheme, site)
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
    let (mut graph, scheme, site) = graph(&mut work);
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
    let (mut graph, scheme, site) = graph(&mut work);
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
        .restore_delta(&target, delta, &mut Work(0))
        .unwrap_err()
        .into_parts();
    assert!(matches!(
        error,
        PreparedCallGraphMutationFailure::Control(Exhausted)
    ));
    assert!(!retained(&mut graph, &row));
    assert_eq!(graph.sites().count(), 0);
    graph.restore_delta(&target, delta, &mut work).unwrap();
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
