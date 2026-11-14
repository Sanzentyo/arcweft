use super::*;
use crate::effect_row::prepared::PreparedEffectGraph;
use crate::{
    effect_row::{
        DecisionControl, DecisionWork, EffectFormula,
        prepared::{PreparedEffectInvariant, PreparedEffectRelation},
    },
    effects::EffectSet,
    types::{ArrayLength, GenericBinder, GenericEffectReference},
};
use std::convert::Infallible;

struct Setup;
impl DecisionControl for Setup {
    type Error = Infallible;
    fn charge(&mut self, _: DecisionWork) -> Result<(), Infallible> {
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
#[error("canonical projection stopped")]
struct Stopped;
struct Meter {
    remaining: u64,
    cancelled: bool,
}
impl Meter {
    fn new() -> Self {
        Self {
            remaining: 1_000_000,
            cancelled: false,
        }
    }
    fn charge(&mut self) -> Result<(), Stopped> {
        self.remaining = self.remaining.checked_sub(1).ok_or(Stopped)?;
        Ok(())
    }
}
impl TypeProjectionControl for Meter {
    type Error = Stopped;
    fn check(&mut self) -> Result<(), Stopped> {
        if self.cancelled { Err(Stopped) } else { Ok(()) }
    }
    fn visit_node(&mut self, _: TypeProjectionNodeKind, _: u64) -> Result<(), Stopped> {
        self.charge()
    }
    fn visit_binding(&mut self) -> Result<(), Stopped> {
        self.charge()
    }
}
fn row(parameter: &GenericEffectReference) -> PreparedEffectRow {
    PreparedEffectRow::Canonical(EffectRow::open(EffectSet::new(), parameter.clone()))
}
fn literal(labels: &[&str]) -> PreparedEffectRow {
    PreparedEffectRow::Canonical(EffectRow::closed(
        EffectSet::from_labels(labels.iter().copied()).unwrap(),
    ))
}
fn arrow(
    effects: PreparedEffectRow,
    predicate: PreparedEffectPredicate,
) -> TypeKind<PreparedEffects> {
    TypeKind::Function {
        binder: GenericBinder::EMPTY,
        params: vec![TypeKind::I64],
        return_type: Box::new(TypeKind::I64),
        effects,
        predicate,
    }
}

#[test]
fn canonical_projection_preserves_nested_type_const_binders_and_nonconstant_predicates() {
    let incoming = GenericScope::default().with_binder(GenericBinder::new(1, 1, 1));
    let binder = GenericBinder::new(1, 1, 1);
    let nested = incoming.with_binder(binder);
    let outer = nested.bound_effect(1, 0).unwrap();
    let inner = nested.bound_effect(0, 0).unwrap();
    let formal = GenericScope::default()
        .with_binder(GenericBinder::new(0, 0, 1))
        .bound_effect(0, 0)
        .unwrap();
    let mut graph = PreparedEffectGraph::default();
    let scheme = graph
        .declare(
            vec![formal.clone()],
            EffectPredicate::unconstrained(),
            &mut Setup,
        )
        .unwrap();
    let body = graph
        .union(vec![row(&formal), literal(&["fs.read"])], &mut Setup)
        .unwrap();
    graph
        .prepare_body(&scheme, body, vec![], &mut Setup)
        .unwrap();
    let live = graph
        .application(&scheme, vec![row(&outer)], &mut Setup)
        .unwrap();
    let permitted = graph
        .union(vec![row(&inner), literal(&["fs.read"])], &mut Setup)
        .unwrap();
    let predicate = graph
        .relate(
            PreparedEffectRelation::AdmittedSubset,
            live.clone(),
            permitted,
            &mut Setup,
        )
        .unwrap();
    let known =
        EffectFormula::from_set(&EffectSet::from_labels(["fs.read"]).unwrap(), &mut Setup).unwrap();
    let actual = EffectFormula::variable(outer.clone(), &mut Setup)
        .unwrap()
        .union(&known, &mut Setup)
        .unwrap();
    let permitted = EffectFormula::variable(inner, &mut Setup)
        .unwrap()
        .union(&known, &mut Setup)
        .unwrap();
    let expected = TypeKind::Function {
        binder,
        params: vec![
            TypeKind::GenericParam(nested.bound_type(0, 0).unwrap()),
            TypeKind::Array {
                item: Box::new(TypeKind::I64),
                len: ArrayLength::Generic(nested.bound_const(0, 0).unwrap()),
            },
        ],
        return_type: Box::new(TypeKind::Tuple(vec![
            TypeKind::GenericParam(nested.bound_type(1, 0).unwrap()),
            TypeKind::Array {
                item: Box::new(TypeKind::Bool),
                len: ArrayLength::Generic(nested.bound_const(1, 0).unwrap()),
            },
        ])),
        effects: EffectRow::open(EffectSet::from_labels(["fs.read"]).unwrap(), outer),
        predicate: actual.subset(&permitted, &mut Setup).unwrap(),
    };
    let mut source = expected
        .prepare_effects(&incoming, &mut Meter::new())
        .unwrap();
    let TypeKind::Function {
        effects,
        predicate: source_predicate,
        ..
    } = &mut source
    else {
        unreachable!()
    };
    *effects = live;
    *source_predicate = predicate;
    let before = source.clone();
    let retained = graph.retained_terms();
    let principal = graph.solve_principal(&mut Setup).unwrap();
    assert_eq!(
        source
            .project_canonical_effects(&incoming, &principal, &mut Meter::new())
            .unwrap(),
        expected
    );
    assert_eq!(source, before);
    assert_eq!(graph.retained_terms(), retained);
}

#[test]
fn omitted_annotations_remain_unknown_while_ambiguous_live_rows_fail() {
    let mut graph = PreparedEffectGraph::default();
    let choice = graph.choice(&mut Setup).unwrap();
    let live = graph
        .select_row(&choice, literal(&["fs.read"]), literal(&[]), &mut Setup)
        .unwrap();
    let principal = graph.solve_principal(&mut Setup).unwrap();
    let source = arrow(
        PreparedEffectRow::Canonical(EffectRow::unknown()),
        PreparedEffectPredicate::Canonical(EffectPredicate::unconstrained()),
    );
    let canonical = source
        .project_canonical_effects(&GenericScope::default(), &principal, &mut Meter::new())
        .unwrap();
    let TypeKind::Function { effects, .. } = canonical else {
        unreachable!()
    };
    assert_eq!(effects, EffectRow::unknown());
    let source = arrow(
        live,
        PreparedEffectPredicate::Canonical(EffectPredicate::unconstrained()),
    );
    assert!(matches!(
        source.project_canonical_effects(&GenericScope::default(), &principal, &mut Meter::new()),
        Err(TypeProjectionError::Control(
            PreparedTypeEffectProjectionFailure::Effects(
                PreparedEffectFailure::ObservationNotTotal
            )
        ))
    ));
}

#[test]
fn nonpointwise_predicates_cannot_become_canonical_type_contracts() {
    let scope = GenericScope::default().with_binder(GenericBinder::new(0, 0, 2));
    let e = scope.bound_effect(0, 0).unwrap();
    let f = scope.bound_effect(0, 1).unwrap();
    let mut graph = PreparedEffectGraph::default();
    let subset = graph
        .relate(
            PreparedEffectRelation::AdmittedSubset,
            row(&e),
            row(&f),
            &mut Setup,
        )
        .unwrap();
    let superset = graph
        .relate(
            PreparedEffectRelation::AdmittedSubset,
            row(&f),
            row(&e),
            &mut Setup,
        )
        .unwrap();
    let predicate = graph.or(vec![subset, superset], &mut Setup).unwrap();
    let source = arrow(literal(&[]), predicate);
    let before = source.clone();
    let retained = graph.retained_terms();
    let principal = graph.solve_principal(&mut Setup).unwrap();
    assert!(matches!(
        source.project_canonical_effects(&scope, &principal, &mut Meter::new()),
        Err(TypeProjectionError::Control(
            PreparedTypeEffectProjectionFailure::Effects(
                PreparedEffectFailure::NonPointwiseObservation
            )
        ))
    ));
    assert_eq!(source, before);
    assert_eq!(graph.retained_terms(), retained);
}

#[test]
fn original_term_issuer_and_rollback_failures_survive_type_projection() {
    let mut graph = PreparedEffectGraph::default();
    let mut checkpoint = graph.checkpoint(&mut Setup).unwrap();
    let stale = graph.union(vec![literal(&[])], &mut Setup).unwrap();
    graph.rollback(&mut checkpoint).unwrap();
    let mut other = PreparedEffectGraph::default();
    let foreign = other.union(vec![literal(&[])], &mut Setup).unwrap();
    let principal = graph.solve_principal(&mut Setup).unwrap();
    for (row, expected) in [
        (stale, PreparedEffectInvariant::StaleReference),
        (foreign, PreparedEffectInvariant::ForeignIssuer),
    ] {
        let source = arrow(
            row,
            PreparedEffectPredicate::Canonical(EffectPredicate::unconstrained()),
        );
        match source.project_canonical_effects(
            &GenericScope::default(),
            &principal,
            &mut Meter::new(),
        ) {
            Err(TypeProjectionError::Control(PreparedTypeEffectProjectionFailure::Effects(
                PreparedEffectFailure::Invariant(actual),
            ))) => assert_eq!(actual, expected),
            _ => panic!("invalid term authority cannot produce a canonical type"),
        }
    }
}

#[test]
fn projection_uses_one_inclusive_budget_and_preserves_abort_without_partial_output() {
    let mut graph = PreparedEffectGraph::default();
    let live = graph
        .union(vec![literal(&["fs.read"])], &mut Setup)
        .unwrap();
    let arrow = arrow(
        live,
        PreparedEffectPredicate::Canonical(EffectPredicate::unconstrained()),
    );
    let source = TypeKind::Tuple(vec![arrow.clone(), arrow]);
    let before = source.clone();
    let principal = graph.solve_principal(&mut Setup).unwrap();
    let mut measured = Meter::new();
    let initial = measured.remaining;
    let expected = source
        .project_canonical_effects(&GenericScope::default(), &principal, &mut measured)
        .unwrap();
    let cost = initial - measured.remaining;
    assert!(cost > 0);
    for mut control in [
        Meter {
            remaining: cost - 1,
            cancelled: false,
        },
        Meter {
            remaining: initial,
            cancelled: true,
        },
    ] {
        assert!(matches!(
            source.project_canonical_effects(&GenericScope::default(), &principal, &mut control),
            Err(TypeProjectionError::Control(
                PreparedTypeEffectProjectionFailure::Control(Stopped)
            ))
        ));
    }
    let actual = source
        .project_canonical_effects(
            &GenericScope::default(),
            &principal,
            &mut Meter {
                remaining: cost,
                cancelled: false,
            },
        )
        .unwrap();
    assert_eq!(actual, expected);
    assert_eq!(source, before);
}
