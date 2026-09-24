use crate::types::constraints::test_support::ConstraintTestSetup;
use std::{collections::BTreeMap, sync::atomic::AtomicBool};

use crate::{
    effect_row::EffectRow,
    types::{
        ArrayLength, DetachedGenericOwnerId, GenericBinder, GenericConstParameterId,
        GenericParameterOwnerId, GenericScope, GenericScopeError, GenericTypeParameterId,
        ScopedTypeView, TypeKind,
        constraints::{
            NoConstraintClient, TypeConstraintConstEligibility as ConstRole,
            TypeConstraintParameterEligibility as TypeRole, TypeConstraintParameterScope,
            context::{
                LocalConstraintAccounting, TypeConstraintConstParameterScopeRow as ConstRow,
                TypeConstraintLimits, TypeConstraintTypeParameterScopeRow as TypeRow,
            },
        },
    },
};

use super::{TypeConstraintSolution, TypeInstantiationError};

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub(super) enum ProjectionStop {
    #[error("stopped at {kind:?} depth {depth}")]
    Node {
        kind: crate::types::TypeProjectionNodeKind,
        depth: u64,
    },
    #[error("binding projection stopped")]
    Binding,
}

pub(super) struct ProjectionRecorder {
    limit: usize,
    pub(super) reject_binding: bool,
    nodes: Vec<(crate::types::TypeProjectionNodeKind, u64)>,
    bindings: usize,
}

impl ProjectionRecorder {
    pub(super) fn new(limit: usize) -> Self {
        Self {
            limit,
            reject_binding: false,
            nodes: Vec::new(),
            bindings: 0,
        }
    }
}

impl crate::types::TypeProjectionControl for ProjectionRecorder {
    type Error = ProjectionStop;

    fn check(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }

    fn visit_node(
        &mut self,
        kind: crate::types::TypeProjectionNodeKind,
        depth: u64,
    ) -> Result<(), Self::Error> {
        if self.nodes.len() == self.limit {
            return Err(ProjectionStop::Node { kind, depth });
        }
        self.nodes.push((kind, depth));
        Ok(())
    }

    fn visit_binding(&mut self) -> Result<(), Self::Error> {
        if self.reject_binding {
            return Err(ProjectionStop::Binding);
        }
        self.bindings += 1;
        Ok(())
    }
}

type Context<'a> = ConstraintTestSetup<'a, LocalConstraintAccounting<'a>, NoConstraintClient>;

pub(super) fn parameter(slot: u16) -> GenericTypeParameterId {
    GenericTypeParameterId::new(
        GenericParameterOwnerId::Detached(DetachedGenericOwnerId::new(734)),
        slot,
    )
}

pub(super) fn constant() -> GenericConstParameterId {
    GenericConstParameterId::new(
        GenericParameterOwnerId::Detached(DetachedGenericOwnerId::new(734)),
        0,
    )
}

pub(super) fn scope(recursive: bool, future: bool) -> TypeConstraintParameterScope {
    let mut types = vec![TypeRow::new(parameter(0), TypeRole::Bindable)];
    let mut consts = vec![ConstRow::new(constant(), ConstRole::Bindable)];
    if recursive {
        types.push(TypeRow::new(parameter(0), TypeRole::Rigid));
        consts.push(ConstRow::new(constant(), ConstRole::Rigid));
    }
    if future {
        types.push(TypeRow::new(parameter(1), TypeRole::FutureEligible));
        consts = vec![ConstRow::new(constant(), ConstRole::FutureEligible)];
    }
    TypeConstraintParameterScope::seal_call_scope(
        crate::types::GenericBinder::EMPTY,
        types,
        consts,
        crate::types::constraints::TypeConstraintEffectScope::seal_call_scope([], [])
            .expect("empty effect scope"),
        [],
        [],
    )
    .expect("separated namespaces")
}

pub(super) fn context<'a>(
    scope: TypeConstraintParameterScope,
    cancellation: &'a AtomicBool,
) -> Context<'a> {
    Context::with_scope(
        TypeConstraintLimits::new(65_536, 32_768, 1024, 1024),
        cancellation,
        scope,
    )
}

pub(super) fn complete(
    scope: TypeConstraintParameterScope,
    ty: TypeKind,
    length: ArrayLength,
) -> TypeConstraintSolution {
    let cancellation = AtomicBool::new(false);
    let (mut context, mut path) = context(scope, &cancellation).into_path();
    {
        path.bindings = BTreeMap::from([(
            path.applications
                .root_scope()
                .parameters()
                .type_reference(&(parameter(0)).clone().into())
                .expect("callee type"),
            ty,
        )]);
        path.const_bindings = BTreeMap::from([(
            path.applications
                .root_scope()
                .parameters()
                .const_reference(&(constant()).clone().into())
                .expect("callee constant"),
            length,
        )]);
        TypeConstraintSolution::complete_application(
            &path,
            path.applications.root_id(),
            &mut context,
        )
    }
    .expect("completed application")
}

#[test]
fn recursive_instance_closes_binding_values_once_in_the_caller() {
    let caller = complete(scope(false, false), TypeKind::I32, ArrayLength::Const(5))
        .close_instantiation(None)
        .expect("root instance");
    let recursive = complete(
        scope(true, false),
        TypeKind::Vec(Box::new(TypeKind::generic_parameter(parameter(0)))),
        ArrayLength::generic_parameter(constant()),
    );
    let callee = recursive
        .close_instantiation(Some(&caller))
        .expect("recursive instance");
    let declaration = TypeKind::Array {
        item: Box::new(TypeKind::generic_parameter(parameter(0))),
        len: ArrayLength::generic_parameter(constant()),
    };
    assert_eq!(
        callee.instantiate_type(&declaration).expect("body type"),
        TypeKind::Array {
            item: Box::new(TypeKind::Vec(Box::new(TypeKind::I32))),
            len: ArrayLength::Const(5),
        }
    );
    assert!(matches!(
        recursive.close_instantiation(None),
        Err(TypeInstantiationError::UnboundType { .. })
    ));

    let direct = complete(
        scope(false, false),
        TypeKind::Vec(Box::new(TypeKind::I32)),
        ArrayLength::Const(5),
    )
    .close_instantiation(None)
    .expect("direct instance");
    assert_eq!(
        callee, direct,
        "equal flat environments retain no calling history"
    );
}

#[test]
fn controlled_projection_visits_type_const_and_effect_occurrences() {
    use crate::types::TypeProjectionNodeKind::{Const, Effect, Type};
    let ty = TypeKind::function_with_effects(
        [TypeKind::I32, TypeKind::I64],
        TypeKind::Array {
            item: Box::new(TypeKind::Bool),
            len: ArrayLength::Const(3),
        },
        EffectRow::closed(
            crate::effects::EffectSet::from_labels(["fs.read", "fs.write"]).expect("effects"),
        ),
    );
    let mut control = ProjectionRecorder::new(19);
    let actual = super::ClosedTypeInstantiation::default()
        .instantiate_type_with_control(&ty, &mut control)
        .expect("six type/constant visits, three predicate tokens and ten row tokens");
    assert_eq!(actual, ty);
    assert_eq!(
        control.nodes,
        [
            (Type, 1),
            (Type, 2),
            (Type, 2),
            (Type, 2),
            (Type, 3),
            (Const, 3),
            (Effect, 2),
            (Effect, 2),
            (Effect, 2),
            (Effect, 2),
            (Effect, 2),
            (Effect, 2),
            (Effect, 2),
            (Effect, 3),
            (Effect, 2),
            (Effect, 2),
            (Effect, 3),
            (Effect, 2),
            (Effect, 2),
        ]
    );
}

#[test]
fn controlled_replacement_keeps_the_insertion_depth_and_stops_before_later_children() {
    use crate::types::{TypeProjectionError, TypeProjectionNodeKind::Type};
    let replacement = TypeKind::Vec(Box::new(TypeKind::Vec(Box::new(TypeKind::I32))));
    let closed = complete(
        scope(false, false),
        replacement.clone(),
        ArrayLength::Const(5),
    )
    .close_instantiation(None)
    .expect("closed replacement");
    let template = TypeKind::Tuple(vec![
        TypeKind::generic_parameter(parameter(0)),
        TypeKind::Unit,
    ]);
    let mut stopped = ProjectionRecorder::new(3);
    assert!(matches!(
        closed.instantiate_type_with_control(&template, &mut stopped),
        Err(TypeProjectionError::Control(ProjectionStop::Node {
            kind: Type,
            depth: 3
        }))
    ));
    assert_eq!(stopped.nodes, [(Type, 1), (Type, 2), (Type, 2)]);
    let mut complete = ProjectionRecorder::new(6);
    assert_eq!(
        closed
            .instantiate_type_with_control(&template, &mut complete)
            .expect("same immutable solution remains usable"),
        TypeKind::Tuple(vec![replacement, TypeKind::Unit])
    );
    assert_eq!(
        complete.nodes,
        [
            (Type, 1),
            (Type, 2),
            (Type, 2),
            (Type, 3),
            (Type, 4),
            (Type, 2)
        ]
    );
}

#[test]
fn controlled_instance_closure_preserves_binding_abort_and_caller_ownership() {
    use crate::types::TypeProjectionError;
    let caller = complete(scope(false, false), TypeKind::I32, ArrayLength::Const(5))
        .close_instantiation(None)
        .expect("caller");
    let recursive = complete(
        scope(true, false),
        TypeKind::Vec(Box::new(TypeKind::generic_parameter(parameter(0)))),
        ArrayLength::generic_parameter(constant()),
    );
    let mut stopped = ProjectionRecorder::new(10);
    stopped.reject_binding = true;
    assert!(matches!(
        recursive.close_instantiation_with_control(Some(&caller), &mut stopped),
        Err(TypeProjectionError::Control(ProjectionStop::Binding))
    ));
    assert!(stopped.nodes.is_empty());
    let mut control = ProjectionRecorder::new(10);
    let closed = recursive
        .close_instantiation_with_control(Some(&caller), &mut control)
        .expect("controlled recursive closure");
    assert_eq!(control.bindings, 2);
    assert_eq!(
        closed
            .instantiate_type(&TypeKind::generic_parameter(parameter(0)))
            .expect("closed declaration"),
        TypeKind::Vec(Box::new(TypeKind::I32))
    );
    assert_eq!(
        closed,
        recursive
            .close_instantiation(Some(&caller))
            .expect("same closure algorithm")
    );
}

#[test]
fn template_type_and_const_substitution_is_simultaneous() {
    let actual = TypeKind::Array {
        item: Box::new(TypeKind::generic_parameter(parameter(0))),
        len: ArrayLength::generic_parameter(constant()),
    };
    let solution = complete(scope(true, false), actual.clone(), ArrayLength::Const(7));
    let result = solution
        .apply_template(&TypeKind::generic_parameter(parameter(0)))
        .expect("template");
    assert_eq!(
        result.view().to_root_type().expect("caller-owned value"),
        actual,
        "the callee's N=7 must not rewrite caller N inside the replacement for T"
    );
    let direct_length = solution
        .apply_template(&TypeKind::Array {
            item: Box::new(TypeKind::Bool),
            len: ArrayLength::generic_parameter(constant()),
        })
        .expect("constant template");
    assert_eq!(
        direct_length.view().to_root_type().expect("closed length"),
        TypeKind::Array {
            item: Box::new(TypeKind::Bool),
            len: ArrayLength::Const(7),
        }
    );
}

#[test]
fn closed_instance_preserves_function_local_quantification_in_all_namespaces() {
    let instance = complete(scope(false, false), TypeKind::I32, ArrayLength::Const(5))
        .close_instantiation(None)
        .expect("root");
    let binder = GenericBinder::new(1, 1, 1);
    let local = GenericScope::default().with_binder(binder);
    let parameter_type = TypeKind::Array {
        item: Box::new(TypeKind::GenericParam(
            local.bound_type(0, 0).expect("local type"),
        )),
        len: ArrayLength::Generic(local.bound_const(0, 0).expect("local const")),
    };
    let function = TypeKind::function_with_binder(
        binder,
        [parameter_type.clone()],
        TypeKind::generic_parameter(parameter(0)),
        EffectRow::open(
            crate::effects::EffectSet::new(),
            local.bound_effect(0, 0).expect("local effect"),
        ),
    );
    let projected = instance
        .instantiate_type(&function)
        .expect("local scheme remains quantified");
    assert_eq!(
        projected,
        TypeKind::function_with_binder(
            binder,
            [parameter_type],
            TypeKind::I32,
            EffectRow::open(
                crate::effects::EffectSet::new(),
                local.bound_effect(0, 0).expect("local effect")
            )
        )
    );
    assert!(projected.semantic_identity_digest().is_ok());
}

#[test]
fn incoming_and_function_binders_are_fused_without_capturing_nested_variables() {
    let outer = GenericBinder::new(1, 1, 0);
    let own = GenericBinder::new(1, 1, 0);
    let incoming = GenericScope::default().with_binder(outer);
    let source = incoming.with_binder(own);
    let nested = source.with_binder(own);
    let ty = TypeKind::function_with_binder(
        own,
        [TypeKind::GenericParam(
            source.bound_type(0, 0).expect("root local"),
        )],
        TypeKind::function_with_binder(
            own,
            [TypeKind::GenericParam(
                nested.bound_type(0, 0).expect("nested local"),
            )],
            TypeKind::Array {
                item: Box::new(TypeKind::GenericParam(
                    nested.bound_type(2, 0).expect("incoming type"),
                )),
                len: ArrayLength::Generic(nested.bound_const(2, 0).expect("incoming length")),
            },
            EffectRow::closed(crate::effects::EffectSet::new()),
        ),
        EffectRow::closed(crate::effects::EffectSet::new()),
    );
    let scoped = ScopedTypeView::sealed(&ty, &incoming);
    assert!(
        scoped.to_root_type().is_err(),
        "an incoming binder cannot be discarded"
    );
    let quantified = scoped.to_quantified_type().expect("quantified function");
    let merged = GenericBinder::new(2, 2, 0);
    let target = GenericScope::default().with_binder(merged);
    let nested = target.with_binder(own);
    let expected = TypeKind::function_with_binder(
        merged,
        [TypeKind::GenericParam(
            target
                .bound_type(0, 1)
                .expect("root slot follows incoming slot"),
        )],
        TypeKind::function_with_binder(
            own,
            [TypeKind::GenericParam(
                nested.bound_type(0, 0).expect("nested local"),
            )],
            TypeKind::Array {
                item: Box::new(TypeKind::GenericParam(
                    nested.bound_type(1, 0).expect("transferred incoming type"),
                )),
                len: ArrayLength::Generic(
                    nested
                        .bound_const(1, 0)
                        .expect("transferred incoming length"),
                ),
            },
            EffectRow::closed(crate::effects::EffectSet::new()),
        ),
        EffectRow::closed(crate::effects::EffectSet::new()),
    );
    assert_eq!(quantified, expected);
    assert!(quantified.semantic_identity_digest().is_ok());
}

#[test]
fn effect_binder_fusion_preserves_local_and_incoming_owners() {
    let row = |reference| EffectRow::open(crate::effects::EffectSet::new(), reference);
    let outer = GenericBinder::new(0, 0, 2);
    let own = GenericBinder::new(0, 0, 1);
    let incoming = GenericScope::default().with_binder(outer);
    let source = incoming.with_binder(own);
    let nested = source.with_binder(own);
    let function = TypeKind::function_with_binder(
        own,
        [TypeKind::function_with_binder(
            own,
            [],
            TypeKind::Unit,
            row(nested.bound_effect(0, 0).expect("inner effect")),
        )],
        TypeKind::function_with_effects(
            [],
            TypeKind::Unit,
            row(source.bound_effect(1, 1).expect("incoming effect")),
        ),
        row(source.bound_effect(0, 0).expect("own effect")),
    );
    let value = ScopedTypeView::sealed(&function, &incoming);
    assert!(
        value.to_root_type().is_err(),
        "an incoming effect owner cannot disappear"
    );
    let projected = value.to_quantified_type().expect("fused binders");
    let merged = GenericBinder::new(0, 0, 3);
    let target = GenericScope::default().with_binder(merged);
    let nested = target.with_binder(own);
    assert_eq!(
        projected,
        TypeKind::function_with_binder(
            merged,
            [TypeKind::function_with_binder(
                own,
                [],
                TypeKind::Unit,
                row(nested.bound_effect(0, 0).expect("inner effect unchanged"))
            )],
            TypeKind::function_with_effects(
                [],
                TypeKind::Unit,
                row(target.bound_effect(0, 1).expect("incoming slot retained"))
            ),
            row(target
                .bound_effect(0, 2)
                .expect("own slot follows incoming slots")),
        )
    );
    assert!(projected.semantic_identity_digest().is_ok());
}

#[test]
fn type_replacement_retains_caller_effect_reference_without_callee_substitution() {
    use crate::effect_row::{EffectConstraintEligibility as EffectRole, EffectConstraintVariable};
    let reference =
        crate::types::GenericEffectReference::Free(crate::types::GenericEffectParameterId::new(
            GenericParameterOwnerId::Detached(DetachedGenericOwnerId::new(734)),
            0,
        ));
    let actual = TypeKind::function_with_effects(
        [],
        TypeKind::Unit,
        EffectRow::open(crate::effects::EffectSet::new(), reference.clone()),
    );
    let complete_effect = |ty: Option<TypeKind>, effect: &str| {
        let roles = ty
            .as_ref()
            .map(|_| EffectRole::Rigid)
            .into_iter()
            .chain([EffectRole::Bindable]);
        let scope = TypeConstraintParameterScope::seal_call_scope(
            GenericBinder::EMPTY,
            ty.as_ref()
                .map(|_| TypeRow::new(parameter(0), TypeRole::Bindable)),
            [],
            crate::types::constraints::TypeConstraintEffectScope::seal_call_scope(
                roles.map(|role| EffectConstraintVariable::new(reference.clone(), role)),
                [],
            )
            .expect("caller and callee effect namespaces"),
            [],
            [],
        )
        .expect("parameter scope");
        let cancellation = AtomicBool::new(false);
        let (mut context, mut path) = context(scope, &cancellation).into_path();
        if let Some(ty) = ty {
            let key = path
                .applications
                .root_scope()
                .parameters()
                .type_reference(&parameter(0).into())
                .expect("callee type");
            path.bindings.insert(key, ty);
        }
        let effect_key = path
            .applications
            .root_scope()
            .parameters()
            .effect_reference(&reference)
            .expect("callee effect opening");
        let opened = EffectRow::open(crate::effects::EffectSet::new(), effect_key);
        let bound =
            EffectRow::closed(crate::effects::EffectSet::from_labels([effect]).expect("effect"));
        path.effects
            .constrain_subset(&opened, &bound, &mut context)
            .expect("effect upper bound");
        path.effects
            .constrain_subset(&bound, &opened, &mut context)
            .expect("effect lower bound");
        TypeConstraintSolution::complete_application(
            &path,
            path.applications.root_id(),
            &mut context,
        )
        .expect("simultaneous effect solution")
    };
    let solution = complete_effect(Some(actual.clone()), "log.write");
    let projected = solution
        .apply_template(&TypeKind::generic_parameter(parameter(0)))
        .expect("template projection");
    assert_eq!(
        projected
            .view()
            .to_root_type()
            .expect("free effect retains its declaration"),
        actual
    );
    let caller = complete_effect(None, "fs.read")
        .close_instantiation(None)
        .expect("caller instance");
    let callee = solution
        .close_instantiation(Some(&caller))
        .expect("recursive callee instance");
    assert_eq!(
        callee
            .instantiate_type(&TypeKind::generic_parameter(parameter(0)))
            .expect("caller-owned operand"),
        TypeKind::function_with_effects(
            [],
            TypeKind::Unit,
            EffectRow::closed(
                crate::effects::EffectSet::from_labels(["fs.read"]).expect("caller effect")
            )
        )
    );
    assert_eq!(
        callee
            .instantiate_type(&actual)
            .expect("callee-owned template"),
        TypeKind::function_with_effects(
            [],
            TypeKind::Unit,
            EffectRow::closed(
                crate::effects::EffectSet::from_labels(["log.write"]).expect("callee effect")
            )
        )
    );
}

#[test]
fn residual_effect_replacements_lift_through_existing_function_binders() {
    use crate::effect_row::{EffectConstraintEligibility, EffectConstraintVariable};
    let effect =
        crate::types::GenericEffectReference::Free(crate::types::GenericEffectParameterId::new(
            GenericParameterOwnerId::Detached(DetachedGenericOwnerId::new(734)),
            0,
        ));
    let scope = TypeConstraintParameterScope::seal_call_scope(
        GenericBinder::EMPTY,
        [TypeRow::new(parameter(0), TypeRole::Bindable)],
        [],
        crate::types::constraints::TypeConstraintEffectScope::seal_call_scope(
            [EffectConstraintVariable::new(
                effect.clone(),
                EffectConstraintEligibility::FutureEligible,
            )],
            [],
        )
        .expect("future effect contract"),
        [],
        [],
    )
    .expect("parameter scope");
    let cancellation = AtomicBool::new(false);
    let (mut context, mut path) = context(scope, &cancellation).into_path();
    let opening = path.applications.root_scope().parameters();
    let effect_key = opening.effect_reference(&effect).expect("effect opening");
    let type_key = opening
        .type_reference(&parameter(0).into())
        .expect("type opening");
    path.bindings.insert(
        type_key,
        TypeKind::function_with_effects(
            [],
            TypeKind::Unit,
            EffectRow::open(crate::effects::EffectSet::new(), effect_key),
        ),
    );
    let solution = TypeConstraintSolution::complete_application(
        &path,
        path.applications.root_id(),
        &mut context,
    )
    .expect("residual function binding");
    let own = GenericBinder::new(0, 0, 1);
    let local = GenericScope::default().with_binder(own);
    let template = TypeKind::function_with_binder(
        own,
        [],
        TypeKind::Tuple(vec![
            TypeKind::generic_parameter(parameter(0)),
            TypeKind::function_with_effects(
                [],
                TypeKind::Unit,
                EffectRow::open(crate::effects::EffectSet::new(), effect),
            ),
        ]),
        EffectRow::open(
            crate::effects::EffectSet::new(),
            local.bound_effect(0, 0).expect("local effect"),
        ),
    );
    let projected = solution
        .apply_template(&template)
        .expect("scoped projection");
    assert!(projected.view().to_root_type().is_err());
    let quantified = projected
        .view()
        .to_quantified_type()
        .expect("quantified projection");
    let merged = GenericBinder::new(0, 0, 2);
    let scope = GenericScope::default().with_binder(merged);
    let result = TypeKind::function_with_effects(
        [],
        TypeKind::Unit,
        EffectRow::open(
            crate::effects::EffectSet::new(),
            scope.bound_effect(0, 0).expect("residual slot"),
        ),
    );
    assert_eq!(
        quantified,
        TypeKind::function_with_binder(
            merged,
            [],
            TypeKind::Tuple(vec![result.clone(), result]),
            EffectRow::open(
                crate::effects::EffectSet::new(),
                scope.bound_effect(0, 1).expect("local follows residual")
            )
        )
    );
    assert!(quantified.semantic_identity_digest().is_ok());
}

#[test]
fn closed_instantiation_enforces_retained_effect_predicate_and_work_limits() {
    use crate::effect_row::{EffectConstraintEligibility as Role, EffectConstraintVariable};
    let reference =
        crate::types::GenericEffectReference::Free(crate::types::GenericEffectParameterId::new(
            GenericParameterOwnerId::Detached(DetachedGenericOwnerId::new(735)),
            0,
        ));
    let solve = |role, labels: &[&str]| {
        let scope = TypeConstraintParameterScope::seal_call_scope(
            GenericBinder::EMPTY,
            [],
            [],
            crate::types::constraints::TypeConstraintEffectScope::seal_call_scope(
                [EffectConstraintVariable::new(reference.clone(), role)],
                [],
            )
            .expect("effect scope"),
            [],
            [],
        )
        .expect("parameter scope");
        let cancellation = AtomicBool::new(false);
        let (mut context, mut path) = context(scope, &cancellation).into_path();
        let variable = if role == Role::Rigid {
            reference.clone()
        } else {
            path.applications
                .root_scope()
                .parameters()
                .effect_reference(&reference)
                .expect("opened effect")
        };
        let row = EffectRow::open(crate::effects::EffectSet::new(), variable);
        let permitted = EffectRow::closed(
            crate::effects::EffectSet::from_labels(labels.iter().copied()).expect("effects"),
        );
        path.effects
            .constrain_subset(&row, &permitted, &mut context)
            .expect("upper bound");
        if role == Role::Bindable {
            path.effects
                .constrain_subset(&permitted, &row, &mut context)
                .expect("exact binding");
        }
        TypeConstraintSolution::complete_application(
            &path,
            path.applications.root_id(),
            &mut context,
        )
        .expect("retained predicate")
    };
    let restricted = solve(Role::Rigid, &["fs.read"]);
    assert!(matches!(
        restricted.close_instantiation(None),
        Err(TypeInstantiationError::UnboundEffect { .. })
    ));
    let invalid = solve(Role::Bindable, &["fs.write"])
        .close_instantiation(None)
        .expect("caller instance");
    assert!(matches!(
        restricted.close_instantiation(Some(&invalid)),
        Err(TypeInstantiationError::UnsatisfiedEffectConstraint)
    ));
    for labels in [&[][..], &["fs.read"][..]] {
        let caller = solve(Role::Bindable, labels)
            .close_instantiation(None)
            .expect("valid caller");
        let mut recorder = ProjectionRecorder::new(usize::MAX);
        let expected = restricted
            .close_instantiation_with_control(Some(&caller), &mut recorder)
            .expect("predicate satisfied");
        assert!(!recorder.nodes.is_empty());
        for limit in 0..recorder.nodes.len() {
            assert!(matches!(
                restricted.close_instantiation_with_control(
                    Some(&caller),
                    &mut ProjectionRecorder::new(limit)
                ),
                Err(crate::types::TypeProjectionError::Control(
                    ProjectionStop::Node { .. }
                ))
            ));
        }
        assert_eq!(
            restricted
                .close_instantiation(Some(&caller))
                .expect("inputs unchanged after aborts"),
            expected
        );
    }
}

#[test]
fn residual_replacement_is_lifted_beneath_existing_template_binders() {
    let cancellation = AtomicBool::new(false);
    let (mut context, mut path) = context(scope(false, true), &cancellation).into_path();
    let future = path
        .applications
        .root_scope()
        .parameters()
        .type_reference(&(parameter(1)).clone().into())
        .expect("future type");
    let length = path
        .applications
        .root_scope()
        .parameters()
        .const_reference(&(constant()).clone().into())
        .expect("future const");
    let solution = {
        path.bindings = BTreeMap::from([(
            path.applications
                .root_scope()
                .parameters()
                .type_reference(&(parameter(0)).clone().into())
                .expect("bound type"),
            TypeKind::Array {
                item: Box::new(TypeKind::GenericParam(future)),
                len: ArrayLength::Generic(length),
            },
        )]);
        path.const_bindings = BTreeMap::new();
        TypeConstraintSolution::complete_application(
            &path,
            path.applications.root_id(),
            &mut context,
        )
    }
    .expect("residual solution");
    assert!(matches!(
        solution.close_instantiation(None),
        Err(TypeInstantiationError::Residual { .. })
    ));
    let binder = GenericBinder::new(1, 0, 0);
    let local = GenericScope::default().with_binder(binder);
    let template = TypeKind::function_with_binder(
        binder,
        [TypeKind::GenericParam(
            local.bound_type(0, 0).expect("local type"),
        )],
        TypeKind::generic_parameter(parameter(0)),
        EffectRow::closed(crate::effects::EffectSet::new()),
    );
    let projected = solution
        .apply_template(&template)
        .expect("capture-avoiding template projection");
    assert!(projected.view().to_root_type().is_err());
    let result = projected
        .view()
        .to_quantified_type()
        .expect("quantified continuation");
    let merged = GenericBinder::new(2, 1, 0);
    let target = GenericScope::default().with_binder(merged);
    assert_eq!(
        result,
        TypeKind::function_with_binder(
            merged,
            [TypeKind::GenericParam(
                target.bound_type(0, 1).expect("template local slot")
            )],
            TypeKind::Array {
                item: Box::new(TypeKind::GenericParam(
                    target.bound_type(0, 0).expect("residual type slot")
                )),
                len: ArrayLength::Generic(target.bound_const(0, 0).expect("residual const slot")),
            },
            EffectRow::closed(crate::effects::EffectSet::new())
        )
    );
}

#[test]
fn quantifier_transfer_reports_arity_overflow() {
    let incoming = GenericScope::default().with_binder(GenericBinder::new(u16::MAX, 0, 0));
    let function = TypeKind::function_with_binder(
        GenericBinder::new(1, 0, 0),
        [],
        TypeKind::Unit,
        EffectRow::closed(crate::effects::EffectSet::new()),
    );
    assert!(matches!(
        ScopedTypeView::sealed(&function, &incoming).to_quantified_type(),
        Err(TypeInstantiationError::Scope(
            GenericScopeError::BinderArityOverflow { .. }
        ))
    ));
}

#[test]
fn anonymous_scheme_slots_open_freshly_and_preserve_nested_function_variables() {
    let binder = GenericBinder::new(1, 1, 0);
    let template_scope = GenericScope::default().with_binder(binder);
    let type_key = template_scope.bound_type(0, 0).expect("scheme type slot");
    let const_key = template_scope.bound_const(0, 0).expect("scheme const slot");
    let fresh = || {
        TypeConstraintParameterScope::seal_call_scope(
            binder,
            [TypeRow::new(type_key.clone(), TypeRole::Bindable)],
            [ConstRow::new(const_key.clone(), ConstRole::Bindable)],
            crate::types::constraints::TypeConstraintEffectScope::seal_call_scope([], [])
                .expect("empty effect scope"),
            [],
            [],
        )
        .expect("complete scheme inventory")
    };
    let first = fresh();
    let second = fresh();
    assert_ne!(
        first.type_reference(&type_key),
        second.type_reference(&type_key)
    );
    assert_ne!(
        first.const_reference(&const_key),
        second.const_reference(&const_key)
    );
    let foreign = second.type_reference(&type_key).expect("other opening");
    assert_eq!(first.eligibility(&foreign), None);

    let cancellation = AtomicBool::new(false);
    let (mut context, mut path) = context(first, &cancellation).into_path();
    let local_binder = GenericBinder::new(1, 0, 0);
    let nested = template_scope.with_binder(local_binder);
    let template = TypeKind::function_with_binder(
        local_binder,
        [TypeKind::GenericParam(
            nested.bound_type(0, 0).expect("nested local"),
        )],
        TypeKind::Array {
            item: Box::new(TypeKind::GenericParam(
                nested.bound_type(1, 0).expect("scheme type"),
            )),
            len: ArrayLength::Generic(nested.bound_const(1, 0).expect("scheme const")),
        },
        EffectRow::closed(crate::effects::EffectSet::new()),
    );
    let opened = context
        .open_template_type(&template, &path, path.applications.root_id())
        .expect("opened template");
    let TypeKind::Function {
        params,
        return_type,
        ..
    } = &opened
    else {
        panic!("function");
    };
    assert_eq!(
        params,
        &[TypeKind::GenericParam(
            nested.bound_type(0, 0).expect("local unchanged")
        )]
    );
    let inferred_type = path
        .applications
        .root_scope()
        .parameters()
        .type_reference(&type_key)
        .expect("own type opening");
    let inferred_const = path
        .applications
        .root_scope()
        .parameters()
        .const_reference(&const_key)
        .expect("own const opening");
    assert_eq!(
        return_type.as_ref(),
        &TypeKind::Array {
            item: Box::new(TypeKind::GenericParam(inferred_type.clone())),
            len: ArrayLength::Generic(inferred_const.clone()),
        }
    );
    assert!(
        context.lexical_scope().binders().is_empty(),
        "opening restores the active relation scope"
    );
    let solution = {
        path.bindings = BTreeMap::from([(inferred_type, TypeKind::I64)]);
        path.const_bindings = BTreeMap::from([(inferred_const, ArrayLength::Const(4))]);
        TypeConstraintSolution::complete_application(
            &path,
            path.applications.root_id(),
            &mut context,
        )
    }
    .expect("completed scheme instance");
    let (key, value) = solution.bindings().next().expect("type binding");
    assert_eq!(key.value(), &type_key);
    assert_eq!(key.scope(), &template_scope);
    assert!(key.semantic_identity_digest().is_ok());
    assert_eq!(value.value(), &TypeKind::I64);
    let expected = TypeKind::function_with_binder(
        local_binder,
        [TypeKind::GenericParam(
            nested.bound_type(0, 0).expect("nested local"),
        )],
        TypeKind::Array {
            item: Box::new(TypeKind::I64),
            len: ArrayLength::Const(4),
        },
        EffectRow::closed(crate::effects::EffectSet::new()),
    );
    assert_eq!(
        solution
            .apply_template(&template)
            .expect("project template")
            .view()
            .to_root_type()
            .expect("closed value"),
        expected
    );
    let instance = solution
        .close_instantiation(None)
        .expect("flat scheme instance");
    assert_eq!(
        instance
            .instantiate_type(&template)
            .expect("instance template"),
        expected
    );
}

#[test]
fn caller_scheme_keys_do_not_capture_quantifiers_inside_an_operand() {
    let binder = GenericBinder::new(1, 0, 0);
    let template_scope = GenericScope::default().with_binder(binder);
    let key = template_scope.bound_type(0, 0).expect("scheme slot");
    let parameter_scope = TypeConstraintParameterScope::seal_call_scope(
        binder,
        [TypeRow::new(key.clone(), TypeRole::Bindable)],
        [],
        crate::types::constraints::TypeConstraintEffectScope::seal_call_scope([], [])
            .expect("empty effect scope"),
        [],
        [],
    )
    .expect("scheme scope");
    let cancellation = AtomicBool::new(false);
    let (mut context, mut path) = context(parameter_scope, &cancellation).into_path();
    let caller = {
        path.bindings = BTreeMap::from([(
            path.applications
                .root_scope()
                .parameters()
                .type_reference(&key)
                .expect("opened slot"),
            TypeKind::I64,
        )]);
        path.const_bindings = BTreeMap::new();
        TypeConstraintSolution::complete_application(
            &path,
            path.applications.root_id(),
            &mut context,
        )
    }
    .expect("caller solution")
    .close_instantiation(None)
    .expect("closed caller");
    let local = TypeKind::GenericParam(
        template_scope
            .bound_type(0, 0)
            .expect("operand-owned quantifier"),
    );
    let operand = TypeKind::function_with_binder(
        binder,
        [local.clone()],
        local,
        EffectRow::closed(crate::effects::EffectSet::new()),
    );
    let callee = complete(scope(false, false), operand.clone(), ArrayLength::Const(1))
        .close_instantiation(Some(&caller))
        .expect("closed operand belongs to its own scheme");
    assert_eq!(
        callee
            .instantiate_type(&TypeKind::generic_parameter(parameter(0)))
            .expect("callee template"),
        operand
    );
}

#[test]
fn scheme_scope_rejects_missing_slots_and_application_keys() {
    let binder = GenericBinder::new(2, 0, 0);
    let template = GenericScope::default().with_binder(binder);
    let first = template.bound_type(0, 0).expect("first slot");
    assert!(
        TypeConstraintParameterScope::seal_call_scope(
            binder,
            [TypeRow::new(first.clone(), TypeRole::Bindable)],
            [],
            crate::types::constraints::TypeConstraintEffectScope::seal_call_scope([], [])
                .expect("empty effect scope"),
            [],
            [],
        )
        .is_err(),
        "the whole binder inventory is required"
    );
    let scope = TypeConstraintParameterScope::seal_call_scope(
        binder,
        [
            TypeRow::new(first.clone(), TypeRole::Bindable),
            TypeRow::new(
                template.bound_type(0, 1).expect("second slot"),
                TypeRole::Bindable,
            ),
        ],
        [],
        crate::types::constraints::TypeConstraintEffectScope::seal_call_scope([], [])
            .expect("empty effect scope"),
        [],
        [],
    )
    .expect("complete inventory");
    let active = scope.type_reference(&first).expect("active key candidate");
    assert!(
        TypeConstraintParameterScope::seal_call_scope(
            GenericBinder::EMPTY,
            [TypeRow::new(active, TypeRole::Bindable)],
            [],
            crate::types::constraints::TypeConstraintEffectScope::seal_call_scope([], [])
                .expect("empty effect scope"),
            [],
            [],
        )
        .is_err(),
        "application variables cannot become persistent template keys"
    );
}
