use super::*;
use crate::effect_row::prepared::{PreparedEffectRelation, PreparedEffectSchemeRef};
use crate::effect_row::{DecisionControl, DecisionWork, EffectFormula, EffectPredicate, EffectRow};
use crate::effects::EffectSet;
use crate::types::GenericBinder;
use std::convert::Infallible;

struct Setup;
impl DecisionControl for Setup {
    type Error = Infallible;
    fn charge(&mut self, _: DecisionWork) -> Result<(), Infallible> {
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
#[error("lifting stopped")]
struct Stopped;
struct Meter {
    remaining: u64,
    used: u64,
    cancelled: bool,
}
impl Meter {
    fn new(remaining: u64) -> Self {
        Self {
            remaining,
            used: 0,
            cancelled: false,
        }
    }
    fn charge(&mut self) -> Result<(), Stopped> {
        self.check()?;
        self.remaining = self.remaining.checked_sub(1).ok_or(Stopped)?;
        self.used += 1;
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
fn row(reference: crate::types::GenericEffectReference) -> PreparedEffectRow {
    PreparedEffectRow::Canonical(EffectRow::open(EffectSet::new(), reference))
}
fn literal() -> PreparedEffectRow {
    PreparedEffectRow::Canonical(EffectRow::closed(
        EffectSet::from_labels(["fs.read"]).unwrap(),
    ))
}
fn fixture() -> (
    PreparedEffectGraph,
    PreparedEffectSchemeRef,
    GenericScope,
    GenericScope,
    TypeKind<PreparedEffects>,
) {
    let original = GenericScope::default().with_binder(GenericBinder::new(1, 1, 1));
    let target = original.with_binder(GenericBinder::new(2, 2, 2));
    let binder = GenericBinder::new(1, 1, 1);
    let nested = original.with_binder(binder);
    let formal_scope = GenericScope::default().with_binder(GenericBinder::new(0, 0, 1));
    let formal = formal_scope.bound_effect(0, 0).unwrap();
    let mut graph = PreparedEffectGraph::default();
    let scheme = graph
        .declare(vec![formal], EffectPredicate::unconstrained(), &mut Setup)
        .unwrap();
    let live = graph
        .application(
            &scheme,
            vec![row(nested.bound_effect(1, 0).unwrap())],
            &mut Setup,
        )
        .unwrap();
    let permitted = graph
        .union(
            vec![row(nested.bound_effect(0, 0).unwrap()), literal()],
            &mut Setup,
        )
        .unwrap();
    let predicate = graph
        .relate(
            PreparedEffectRelation::AdmittedSubset,
            live.clone(),
            permitted,
            &mut Setup,
        )
        .unwrap();
    let source = TypeKind::Function {
        binder,
        params: vec![
            TypeKind::GenericParam(nested.bound_type(0, 0).unwrap()),
            TypeKind::Array {
                item: Box::new(TypeKind::GenericParam(nested.bound_type(1, 0).unwrap())),
                len: ArrayLength::Generic(nested.bound_const(1, 0).unwrap()),
            },
            TypeKind::Function {
                binder: GenericBinder::EMPTY,
                predicate: PreparedEffectPredicate::Canonical(EffectPredicate::unconstrained()),
                params: vec![],
                return_type: Box::new(TypeKind::I64),
                effects: PreparedEffectRow::Canonical(EffectRow::unknown()),
            },
        ],
        return_type: Box::new(TypeKind::GenericParam(nested.bound_type(1, 0).unwrap())),
        effects: live,
        predicate,
    };
    (graph, scheme, original, target, source)
}

#[test]
fn lifting_a_live_arrow_keeps_original_scheme_until_body_closure() {
    let (mut graph, scheme, original, target, source) = fixture();
    let before = source.clone();
    let lifted = source
        .lift_under_with_control(&original, &target, &mut graph, &mut Meter::new(1_000_000))
        .unwrap();
    // The only body is prepared after the value has crossed the added binder.
    let formal_scope = GenericScope::default().with_binder(GenericBinder::new(0, 0, 1));
    let body = graph
        .union(
            vec![row(formal_scope.bound_effect(0, 0).unwrap()), literal()],
            &mut Setup,
        )
        .unwrap();
    graph
        .prepare_body(&scheme, body, vec![], &mut Setup)
        .unwrap();
    let principal = graph.solve_principal(&mut Setup).unwrap();
    let result = lifted
        .project_canonical_effects(&target, &principal, &mut Meter::new(1_000_000))
        .unwrap();
    let nested = target.with_binder(GenericBinder::new(1, 1, 1));
    let TypeKind::Function {
        params,
        return_type,
        effects,
        predicate,
        ..
    } = result
    else {
        panic!("lifted arrow")
    };
    assert_eq!(
        params[0],
        TypeKind::GenericParam(nested.bound_type(0, 0).unwrap())
    );
    assert_eq!(
        params[1],
        TypeKind::Array {
            item: Box::new(TypeKind::GenericParam(nested.bound_type(2, 0).unwrap())),
            len: ArrayLength::Generic(nested.bound_const(2, 0).unwrap())
        }
    );
    assert_eq!(
        *return_type,
        TypeKind::GenericParam(nested.bound_type(2, 0).unwrap())
    );
    assert_eq!(
        effects,
        EffectRow::open(
            EffectSet::from_labels(["fs.read"]).unwrap(),
            nested.bound_effect(2, 0).unwrap()
        )
    );
    let TypeKind::Function { effects, .. } = &params[2] else {
        panic!("omitted nested annotation")
    };
    assert_eq!(effects, &EffectRow::unknown());
    let known =
        EffectFormula::from_set(&EffectSet::from_labels(["fs.read"]).unwrap(), &mut Setup).unwrap();
    let actual = EffectFormula::variable(nested.bound_effect(2, 0).unwrap(), &mut Setup)
        .unwrap()
        .union(&known, &mut Setup)
        .unwrap();
    let permitted = EffectFormula::variable(nested.bound_effect(0, 0).unwrap(), &mut Setup)
        .unwrap()
        .union(&known, &mut Setup)
        .unwrap();
    assert_eq!(predicate, actual.subset(&permitted, &mut Setup).unwrap());
    assert_eq!(source, before);
}

#[test]
fn lifting_preserves_inputs_and_graph_at_early_and_late_work_aborts() {
    let (mut graph, _, original, target, source) = fixture();
    let mut full = Meter::new(1_000_000);
    source
        .lift_under_with_control(&original, &target, &mut graph, &mut full)
        .unwrap();
    for budget in [0, full.used / 2, full.used - 1] {
        let (mut graph, _, original, target, source) = fixture();
        let before = source.clone();
        let retained = graph.retained_terms();
        assert!(matches!(
            source.lift_under_with_control(&original, &target, &mut graph, &mut Meter::new(budget)),
            Err(TypeProjectionError::Control(
                PreparedTypeEffectProjectionFailure::Control(Stopped)
            ))
        ));
        assert_eq!(graph.retained_terms(), retained);
        assert_eq!(source, before);
    }
    let (mut graph, _, original, target, source) = fixture();
    let retained = graph.retained_terms();
    let mut cancelled = Meter::new(1_000_000);
    cancelled.cancelled = true;
    assert!(matches!(
        source.lift_under_with_control(&original, &target, &mut graph, &mut cancelled),
        Err(TypeProjectionError::Control(
            PreparedTypeEffectProjectionFailure::Control(Stopped)
        ))
    ));
    assert_eq!(graph.retained_terms(), retained);
}

#[test]
fn lifting_rejects_foreign_term_issuers_without_retaining_partial_terms() {
    let (_, _, original, target, source) = fixture();
    let mut foreign = PreparedEffectGraph::default();
    let before = foreign.retained_terms();
    assert!(matches!(
        source.lift_under_with_control(
            &original,
            &target,
            &mut foreign,
            &mut Meter::new(1_000_000)
        ),
        Err(TypeProjectionError::Control(
            PreparedTypeEffectProjectionFailure::Effects(PreparedEffectFailure::Invariant(_))
        ))
    ));
    assert_eq!(foreign.retained_terms(), before);
}

#[test]
fn a_later_scalar_failure_invalidates_the_enclosing_edit_and_earlier_lift() {
    use crate::effect_row::prepared::PreparedEffectInvariant;
    let (mut graph, _, original, target, source) = fixture();
    let before = graph.retained_terms();
    let mut edit = graph.edit_terms();
    let lifted = source
        .lift_under_in(&original, &target, 1, &mut edit, &mut Meter::new(1_000_000))
        .unwrap();
    let wider = GenericScope::default().with_binder(GenericBinder::new(2, 1, 1));
    let escaped = TypeKind::<PreparedEffects>::GenericParam(wider.bound_type(0, 1).unwrap());
    assert!(matches!(
        escaped.lift_under_in(&original, &target, 1, &mut edit, &mut Meter::new(1_000_000),),
        Err(TypeProjectionError::Instantiation(
            TypeInstantiationError::Scope(_)
        ))
    ));
    assert_eq!(edit.commit(), Err(PreparedEffectInvariant::TermEditState));
    assert_eq!(graph.retained_terms(), before);
    assert!(
        lifted
            .retain_effects(&target, graph.read(), &mut Meter::new(1_000_000))
            .is_err()
    );
}
