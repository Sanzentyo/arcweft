use super::*;
use crate::effect_row::{DecisionControl, DecisionWork, EffectPredicate};
use crate::effects::EffectSet;
use crate::types::{
    DetachedGenericOwnerId, GenericBinder, GenericParameterOwnerId, GenericTypeParameterId,
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
#[error("type argument projection stopped")]
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
fn parameter(slot: u16) -> GenericTypeReference {
    GenericTypeParameterId::new(
        GenericParameterOwnerId::Detached(DetachedGenericOwnerId::new(11)),
        slot,
    )
    .into()
}
fn declaration(parameters: Vec<GenericTypeReference>) -> GenericDeclarationBinder {
    GenericDeclarationBinder::new(
        GenericScope::default(),
        parameters.into_boxed_slice(),
        Box::new([]),
        Box::new([]),
    )
    .unwrap()
}
fn row(reference: crate::types::GenericEffectReference) -> PreparedEffectRow {
    PreparedEffectRow::Canonical(EffectRow::open(EffectSet::new(), reference))
}

#[test]
fn application_is_simultaneous_and_preserves_rigid_template_binders() {
    let t = parameter(0);
    let u = parameter(1);
    let declaration = declaration(vec![t.clone(), u.clone()]);
    let caller = GenericScope::default().with_binder(GenericBinder::new(1, 1, 1));
    let binder = GenericBinder::new(1, 1, 1);
    let template_inner = GenericScope::default().with_binder(binder);
    let template = TypeKind::function_with_binder(
        binder,
        [
            TypeKind::GenericParam(t),
            TypeKind::GenericParam(u.clone()),
            TypeKind::GenericParam(template_inner.bound_type(0, 0).unwrap()),
        ],
        TypeKind::Array {
            item: Box::new(TypeKind::I64),
            len: ArrayLength::Generic(template_inner.bound_const(0, 0).unwrap()),
        },
        EffectRow::unknown(),
    );
    let arguments = [
        TypeKind::GenericParam(u.clone()),
        TypeKind::GenericParam(caller.bound_type(0, 0).unwrap()),
    ];
    let mut graph = PreparedEffectGraph::default();
    let result = template
        .prepare_type_arguments_with_control(
            &declaration,
            &arguments,
            &caller,
            &mut graph,
            &mut Meter::new(1_000_000),
        )
        .unwrap();
    let nested = caller.with_binder(binder);
    let TypeKind::Function {
        params,
        return_type,
        effects,
        ..
    } = result
    else {
        panic!("prepared template arrow")
    };
    assert_eq!(params[0], TypeKind::GenericParam(u));
    assert_eq!(
        params[1],
        TypeKind::GenericParam(nested.bound_type(1, 0).unwrap())
    );
    assert_eq!(
        params[2],
        TypeKind::GenericParam(nested.bound_type(0, 0).unwrap())
    );
    assert_eq!(
        *return_type,
        TypeKind::Array {
            item: Box::new(TypeKind::I64),
            len: ArrayLength::Generic(nested.bound_const(0, 0).unwrap())
        }
    );
    assert_eq!(effects, PreparedEffectRow::Canonical(EffectRow::unknown()));
}

#[test]
fn a_live_argument_crosses_the_template_binder_before_original_body_preparation() {
    let t = parameter(0);
    let declaration = declaration(vec![t.clone()]);
    let caller = GenericScope::default().with_binder(GenericBinder::new(1, 1, 1));
    let binder = GenericBinder::new(1, 1, 1);
    let argument_inner = caller.with_binder(binder);
    let formal_scope = GenericScope::default().with_binder(GenericBinder::new(0, 0, 1));
    let formal = formal_scope.bound_effect(0, 0).unwrap();
    let mut graph = PreparedEffectGraph::default();
    let scheme = graph
        .declare(
            vec![formal.clone()],
            EffectPredicate::unconstrained(),
            &mut Setup,
        )
        .unwrap();
    let live = graph
        .application(
            &scheme,
            vec![row(argument_inner.bound_effect(1, 0).unwrap())],
            &mut Setup,
        )
        .unwrap();
    let argument = TypeKind::Function {
        binder,
        predicate: PreparedEffectPredicate::Canonical(EffectPredicate::unconstrained()),
        params: vec![TypeKind::GenericParam(
            argument_inner.bound_type(0, 0).unwrap(),
        )],
        return_type: Box::new(TypeKind::Array {
            item: Box::new(TypeKind::GenericParam(
                argument_inner.bound_type(1, 0).unwrap(),
            )),
            len: ArrayLength::Generic(argument_inner.bound_const(1, 0).unwrap()),
        }),
        effects: live,
    };
    let template = TypeKind::function_with_binder(
        binder,
        [TypeKind::GenericParam(t.clone())],
        TypeKind::GenericParam(t),
        EffectRow::unknown(),
    );
    let original = argument.clone();
    let result = template
        .prepare_type_arguments_with_control(
            &declaration,
            &[argument.clone()],
            &caller,
            &mut graph,
            &mut Meter::new(1_000_000),
        )
        .unwrap();
    let literal = PreparedEffectRow::Canonical(EffectRow::closed(
        EffectSet::from_labels(["fs.read"]).unwrap(),
    ));
    let body = graph.union(vec![row(formal), literal], &mut Setup).unwrap();
    graph
        .prepare_body(&scheme, body, vec![], &mut Setup)
        .unwrap();
    let principal = graph.solve_principal(&mut Setup).unwrap();
    let canonical = result
        .project_canonical_effects(&caller, &principal, &mut Meter::new(1_000_000))
        .unwrap();
    let TypeKind::Function {
        params,
        return_type,
        effects,
        ..
    } = canonical
    else {
        panic!("outer template")
    };
    assert_eq!(params[0], *return_type);
    let TypeKind::Function {
        params,
        return_type,
        effects: inner_effects,
        ..
    } = &params[0]
    else {
        panic!("retained caller arrow")
    };
    let nested = caller.with_binder(binder).with_binder(binder);
    assert_eq!(
        params[0],
        TypeKind::GenericParam(nested.bound_type(0, 0).unwrap())
    );
    assert_eq!(
        **return_type,
        TypeKind::Array {
            item: Box::new(TypeKind::GenericParam(nested.bound_type(2, 0).unwrap())),
            len: ArrayLength::Generic(nested.bound_const(2, 0).unwrap())
        }
    );
    assert_eq!(
        inner_effects,
        &EffectRow::open(
            EffectSet::from_labels(["fs.read"]).unwrap(),
            nested.bound_effect(2, 0).unwrap()
        )
    );
    assert_eq!(effects, EffectRow::unknown());
    assert_eq!(argument, original);
}

#[test]
fn bound_formals_and_captured_references_use_the_supplied_ambient_scope() {
    let ambient = GenericScope::default().with_binder(GenericBinder::new(2, 1, 1));
    let caller = ambient.with_binder(GenericBinder::new(1, 1, 1));
    let binder = GenericBinder::new(1, 1, 1);
    let source = ambient.with_binder(binder);
    let target = caller.with_binder(binder);
    let declaration = GenericDeclarationBinder::new(
        ambient.clone(),
        Box::new([ambient.bound_type(0, 0).unwrap()]),
        Box::new([]),
        Box::new([]),
    )
    .unwrap();
    let template = TypeKind::function_with_binder(
        binder,
        [
            TypeKind::GenericParam(source.bound_type(1, 0).unwrap()),
            TypeKind::GenericParam(source.bound_type(1, 1).unwrap()),
            TypeKind::GenericParam(source.bound_type(0, 0).unwrap()),
        ],
        TypeKind::Array {
            item: Box::new(TypeKind::I64),
            len: ArrayLength::Generic(source.bound_const(1, 0).unwrap()),
        },
        EffectRow::open(EffectSet::new(), source.bound_effect(1, 0).unwrap()),
    );
    let mut graph = PreparedEffectGraph::default();
    let prepared = template
        .prepare_type_arguments_with_control(
            &declaration,
            &[TypeKind::GenericParam(caller.bound_type(0, 0).unwrap())],
            &caller,
            &mut graph,
            &mut Meter::new(1_000_000),
        )
        .unwrap();
    let principal = graph.solve_principal(&mut Setup).unwrap();
    let canonical = prepared
        .project_canonical_effects(&caller, &principal, &mut Meter::new(1_000_000))
        .unwrap();
    let TypeKind::Function {
        params,
        return_type,
        effects,
        ..
    } = canonical
    else {
        panic!("instantiated ambient template")
    };
    assert_eq!(
        params[0],
        TypeKind::GenericParam(target.bound_type(1, 0).unwrap())
    );
    assert_eq!(
        params[1],
        TypeKind::GenericParam(target.bound_type(2, 1).unwrap())
    );
    assert_eq!(
        params[2],
        TypeKind::GenericParam(target.bound_type(0, 0).unwrap())
    );
    assert_eq!(
        *return_type,
        TypeKind::Array {
            item: Box::new(TypeKind::I64),
            len: ArrayLength::Generic(target.bound_const(2, 0).unwrap())
        }
    );
    assert_eq!(
        effects,
        EffectRow::open(EffectSet::new(), target.bound_effect(2, 0).unwrap())
    );
}

#[test]
fn malformed_argument_inventory_and_late_template_failure_publish_no_terms() {
    let t = parameter(0);
    let caller = GenericScope::default();
    let template = TypeKind::Tuple(vec![
        TypeKind::GenericParam(t.clone()),
        TypeKind::Array {
            item: Box::new(TypeKind::I64),
            len: ArrayLength::Inferred,
        },
    ]);
    let mut graph = PreparedEffectGraph::default();
    let before = graph.retained_terms();
    assert!(matches!(
        template.prepare_type_arguments_with_control(
            &declaration(vec![t.clone()]),
            &[],
            &caller,
            &mut graph,
            &mut Meter::new(1_000_000)
        ),
        Err(TypeProjectionError::Instantiation(
            TypeInstantiationError::TypeArgumentCount {
                expected: 1,
                actual: 0
            }
        ))
    ));
    assert!(matches!(
        template.prepare_type_arguments_with_control(
            &declaration(vec![t.clone(), t.clone()]),
            &[TypeKind::I64, TypeKind::Bool],
            &caller,
            &mut graph,
            &mut Meter::new(1_000_000)
        ),
        Err(TypeProjectionError::Instantiation(
            TypeInstantiationError::DuplicateTypeArgumentParameter { .. }
        ))
    ));
    let scheme = graph
        .declare(vec![], EffectPredicate::unconstrained(), &mut Setup)
        .unwrap();
    let live = graph.application(&scheme, vec![], &mut Setup).unwrap();
    let argument = TypeKind::Function {
        binder: GenericBinder::EMPTY,
        predicate: PreparedEffectPredicate::Canonical(EffectPredicate::unconstrained()),
        params: vec![],
        return_type: Box::new(TypeKind::I64),
        effects: live,
    };
    let before_projection = graph.retained_terms();
    assert!(matches!(
        template.prepare_type_arguments_with_control(
            &declaration(vec![t]),
            &[argument],
            &caller,
            &mut graph,
            &mut Meter::new(1_000_000)
        ),
        Err(TypeProjectionError::Instantiation(
            TypeInstantiationError::UnresolvedType
        ))
    ));
    assert_eq!(graph.retained_terms(), before_projection);
    assert_ne!(before, before_projection);
}

#[test]
fn original_work_and_cancellation_abort_without_partial_application() {
    let t = parameter(0);
    let declaration = declaration(vec![t.clone()]);
    let caller = GenericScope::default();
    let template = TypeKind::Tuple(vec![
        TypeKind::GenericParam(t.clone()),
        TypeKind::GenericParam(t),
    ]);
    let argument = TypeKind::Tuple(vec![TypeKind::I64, TypeKind::Bool]);
    let mut graph = PreparedEffectGraph::default();
    let before = graph.retained_terms();
    let mut full = Meter::new(1_000_000);
    template
        .prepare_type_arguments_with_control(
            &declaration,
            &[argument.clone()],
            &caller,
            &mut graph,
            &mut full,
        )
        .unwrap();
    for budget in [0, full.used / 2, full.used - 1] {
        assert!(matches!(
            template.prepare_type_arguments_with_control(
                &declaration,
                &[argument.clone()],
                &caller,
                &mut graph,
                &mut Meter::new(budget)
            ),
            Err(TypeProjectionError::Control(
                PreparedTypeEffectProjectionFailure::Control(Stopped)
            ))
        ));
        assert_eq!(graph.retained_terms(), before);
    }
    let mut cancelled = Meter::new(1_000_000);
    cancelled.cancelled = true;
    assert!(matches!(
        template.prepare_type_arguments_with_control(
            &declaration,
            &[argument],
            &caller,
            &mut graph,
            &mut cancelled
        ),
        Err(TypeProjectionError::Control(
            PreparedTypeEffectProjectionFailure::Control(Stopped)
        ))
    ));
    assert_eq!(graph.retained_terms(), before);
}

#[test]
fn a_shared_argument_index_keeps_all_fields_in_one_failure_atomic_edit() {
    use crate::effect_row::prepared::PreparedEffectInvariant;
    let t = parameter(0);
    let declaration = declaration(vec![t.clone()]);
    let caller = GenericScope::default();
    let mut graph = PreparedEffectGraph::default();
    let scheme = graph
        .declare(vec![], EffectPredicate::unconstrained(), &mut Setup)
        .unwrap();
    let live = graph.application(&scheme, vec![], &mut Setup).unwrap();
    let arguments = [TypeKind::Function {
        binder: GenericBinder::EMPTY,
        predicate: PreparedEffectPredicate::Canonical(EffectPredicate::unconstrained()),
        params: vec![],
        return_type: Box::new(TypeKind::I64),
        effects: live,
    }];
    let mut work = Meter::new(1_000_000);
    let application =
        PreparedTypeArguments::new(&declaration, &arguments, &caller, &mut work).unwrap();
    let before = graph.retained_terms();
    let mut edit = graph.edit_terms();
    let first = application
        .project_in(&TypeKind::GenericParam(t), &mut edit, &mut work)
        .unwrap();
    let retained = first
        .retain_effects(&caller, edit.read(), &mut work)
        .unwrap();
    assert_eq!(retained, first);
    let invalid = TypeKind::Array {
        item: Box::new(TypeKind::I64),
        len: ArrayLength::Inferred,
    };
    assert!(matches!(
        application.project_in(&invalid, &mut edit, &mut work),
        Err(TypeProjectionError::Instantiation(
            TypeInstantiationError::UnresolvedType
        ))
    ));
    assert_eq!(edit.commit(), Err(PreparedEffectInvariant::TermEditState));
    assert_eq!(graph.retained_terms(), before);
    assert!(
        first
            .retain_effects(&caller, graph.read(), &mut work)
            .is_err()
    );
    assert!(
        retained
            .retain_effects(&caller, graph.read(), &mut work)
            .is_err()
    );
}
