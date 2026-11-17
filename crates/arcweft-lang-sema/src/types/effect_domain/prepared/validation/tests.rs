use super::*;
use crate::effect_row::prepared::{
    PreparedEffectGraph, PreparedEffectPredicate, PreparedEffectRow,
};
use crate::effect_row::{DecisionControl, DecisionWork};
use crate::effects::EffectSet;
use crate::types::GenericBinder;

#[derive(Debug, Eq, PartialEq, thiserror::Error)]
enum Stop {
    #[error("original work exhausted")]
    Budget,
    #[error("depth {actual} exceeds {maximum}")]
    Depth { actual: u64, maximum: u64 },
    #[error("original operation cancelled")]
    Cancelled,
}

struct Work {
    remaining: u64,
    used: u64,
    maximum_depth: u64,
    cancelled: bool,
}

impl Work {
    fn new(remaining: u64, maximum_depth: u64) -> Self {
        Self {
            remaining,
            used: 0,
            maximum_depth,
            cancelled: false,
        }
    }
    fn bill(&mut self) -> Result<(), Stop> {
        self.check()?;
        self.remaining = self.remaining.checked_sub(1).ok_or(Stop::Budget)?;
        self.used += 1;
        Ok(())
    }
}

impl TypeProjectionControl for Work {
    type Error = Stop;
    fn check(&mut self) -> Result<(), Stop> {
        if self.cancelled {
            Err(Stop::Cancelled)
        } else {
            Ok(())
        }
    }
    fn visit_node(&mut self, _: TypeProjectionNodeKind, depth: u64) -> Result<(), Stop> {
        self.check()?;
        if depth > self.maximum_depth {
            return Err(Stop::Depth {
                actual: depth,
                maximum: self.maximum_depth,
            });
        }
        self.bill()
    }
    fn visit_binding(&mut self) -> Result<(), Stop> {
        self.bill()
    }
}

impl DecisionControl for Work {
    type Error = Stop;
    fn charge(&mut self, _: DecisionWork) -> Result<(), Stop> {
        self.bill()
    }
}

#[test]
fn metatype_retains_actual_scope_live_terms_and_unknown_latent_effects() {
    let scope = GenericScope::default().with_binder(GenericBinder::new(1, 1, 1));
    let binder = GenericBinder::new(1, 1, 1);
    let inner = scope.with_binder(binder);
    let mut work = Work::new(100_000, 64);
    let mut graph = PreparedEffectGraph::default();
    let scheme = graph
        .declare(vec![], EffectPredicate::unconstrained(), &mut work)
        .unwrap();
    let live = graph.application(&scheme, vec![], &mut work).unwrap();
    let live_arrow = TypeKind::Function {
        binder,
        predicate: PreparedEffectPredicate::Canonical(EffectPredicate::unconstrained()),
        params: vec![TypeKind::Array {
            item: Box::new(TypeKind::GenericParam(inner.bound_type(0, 0).unwrap())),
            len: ArrayLength::Generic(inner.bound_const(1, 0).unwrap()),
        }],
        return_type: Box::new(TypeKind::GenericParam(inner.bound_type(1, 0).unwrap())),
        effects: live,
    };
    let unknown_arrow = TypeKind::Function {
        binder: GenericBinder::EMPTY,
        predicate: PreparedEffectPredicate::Canonical(EffectPredicate::unconstrained()),
        params: vec![],
        return_type: Box::new(TypeKind::I64),
        effects: PreparedEffectRow::Canonical(EffectRow::unknown()),
    };
    let ty = TypeKind::Tuple(vec![live_arrow, unknown_arrow]);
    let before = graph.retained_terms();
    // No original scheme body has been prepared or solved at this point.
    let meta = ty
        .retain_as_meta_type(&scope, graph.read(), &mut work)
        .unwrap();
    let TypeKind::MetaType(retained) = &meta else {
        panic!("retained metatype")
    };
    // This compares the retained operational handles, not semantic equivalence.
    assert_eq!(retained.as_ref(), &ty);
    assert_eq!(graph.retained_terms(), before);

    let completed = EffectRow::closed(EffectSet::from_labels(["fs.read"]).unwrap());
    graph
        .prepare_body(
            &scheme,
            PreparedEffectRow::Canonical(completed.clone()),
            vec![],
            &mut work,
        )
        .unwrap();
    let principal = graph.solve_principal(&mut work).unwrap();
    let closed = meta
        .project_canonical_effects(&scope, &principal, &mut work)
        .unwrap();
    let TypeKind::MetaType(inner) = closed else {
        panic!("closed metatype")
    };
    let TypeKind::Tuple(fields) = *inner else {
        panic!("retained tuple")
    };
    let TypeKind::Function { effects, .. } = &fields[0] else {
        panic!("live arrow")
    };
    assert_eq!(effects, &completed);
    let TypeKind::Function { effects, .. } = &fields[1] else {
        panic!("unknown arrow")
    };
    assert_eq!(effects, &EffectRow::unknown());
}

#[test]
fn metatype_charges_complete_depth_and_keeps_original_budget_and_cancellation() {
    let scope = GenericScope::default();
    let graph = PreparedEffectGraph::default();
    let ty = TypeKind::<PreparedEffects>::I64;
    let mut bare = Work::new(10, 1);
    ty.retain_effects(&scope, graph.read(), &mut bare).unwrap();
    let mut shallow = Work::new(10, 1);
    assert!(matches!(
        ty.retain_as_meta_type(&scope, graph.read(), &mut shallow),
        Err(TypeProjectionError::Control(
            PreparedTypeRetentionFailure::Control(Stop::Depth {
                actual: 2,
                maximum: 1
            })
        ))
    ));
    assert_eq!(shallow.used, 1);
    for budget in [0, 1] {
        let mut work = Work::new(budget, 64);
        assert!(matches!(
            ty.retain_as_meta_type(&scope, graph.read(), &mut work),
            Err(TypeProjectionError::Control(
                PreparedTypeRetentionFailure::Control(Stop::Budget)
            ))
        ));
        assert_eq!(work.used, budget);
    }
    let mut cancelled = Work::new(10, 64);
    cancelled.cancelled = true;
    assert!(matches!(
        ty.retain_as_meta_type(&scope, graph.read(), &mut cancelled),
        Err(TypeProjectionError::Control(
            PreparedTypeRetentionFailure::Control(Stop::Cancelled)
        ))
    ));
    assert_eq!(cancelled.used, 0);
    assert_eq!(graph.retained_terms(), (0, 0));
}
