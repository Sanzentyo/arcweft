use super::*;
use crate::{
    effect_row::EffectRow,
    effects::EffectSet,
    types::{
        DetachedGenericOwnerId, GenericBinder, GenericConstParameterId, GenericParameterOwnerId,
        GenericTypeParameterId,
        constraints::{
            ConstraintAcceptance, NoConstraintClient, TypeConstraintConstEligibility,
            TypeConstraintEffectScope, TypeConstraintParameterEligibility,
            TypeConstraintParameterScope, TypeConstraintProjectionClosure,
            context::{
                LocalConstraintAccounting, TypeConstraintConstParameterScopeRow,
                TypeConstraintLimits, TypeConstraintTypeParameterScopeRow,
            },
            test_support::ConstraintTestSetup,
            transaction::TypeConstraintTransaction,
        },
    },
};
use std::sync::atomic::{AtomicBool, Ordering};

type Context<'a> = TypeConstraintContext<'a, LocalConstraintAccounting<'a>, NoConstraintClient>;
type Component = OpenConstraintComponent<NoConstraintClient, &'static str>;

fn context(cancellation: &AtomicBool) -> Context<'_> {
    ConstraintTestSetup::new(
        TypeConstraintLimits::new(65536, 65536, 1024, 1024),
        cancellation,
    )
    .into_parts()
    .0
}

fn parameters(
    types: u16,
    lengths: u16,
) -> (
    TypeConstraintParameterScope,
    Vec<GenericTypeReference>,
    Vec<GenericConstReference>,
) {
    let owner = GenericParameterOwnerId::Detached(DetachedGenericOwnerId::new(9481));
    let types: Vec<_> = (0..types)
        .map(|slot| GenericTypeParameterId::new(owner.clone(), slot))
        .collect();
    let lengths: Vec<_> = (0..lengths)
        .map(|slot| GenericConstParameterId::new(owner.clone(), slot))
        .collect();
    let scope = TypeConstraintParameterScope::seal_call_scope(
        GenericBinder::EMPTY,
        types.iter().map(|parameter| {
            TypeConstraintTypeParameterScopeRow::new(
                parameter.clone(),
                TypeConstraintParameterEligibility::Bindable,
            )
        }),
        lengths.iter().map(|parameter| {
            TypeConstraintConstParameterScopeRow::new(
                parameter.clone(),
                TypeConstraintConstEligibility::Bindable,
            )
        }),
        TypeConstraintEffectScope::seal_call_scope([], []).unwrap(),
        [],
        [],
    )
    .unwrap();
    let types = types
        .into_iter()
        .map(|parameter| scope.type_reference(&parameter.into()).unwrap())
        .collect();
    let lengths = lengths
        .into_iter()
        .map(|parameter| scope.const_reference(&parameter.into()).unwrap())
        .collect();
    (scope, types, lengths)
}

fn frontier(
    context: &mut Context<'_>,
    scope: TypeConstraintParameterScope,
    equations: &[(TypeKind, TypeKind)],
) -> SuspendedConstraintFrontier<NoConstraintClient> {
    let mut lower = TypeConstraintTransaction::initialize(context, (), scope, None).unwrap();
    for (pattern, actual) in equations {
        lower.constrain(
            context,
            pattern,
            actual,
            ConstraintAcceptance::PatternAcceptsActual,
        );
    }
    lower.request_projection(
        context,
        (),
        &TypeKind::I64,
        TypeConstraintProjectionClosure::Closed,
    );
    lower.suspend_result(context, ()).unwrap()
}

fn array(length: ArrayLength) -> TypeKind {
    TypeKind::Array {
        item: Box::new(TypeKind::Bool),
        len: length,
    }
}

#[test]
fn shared_original_type_and_const_aliases_are_unified_without_overwriting_either_root() {
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let mut component = Component::new(&mut context).unwrap();
    let (scope, ty, length) = parameters(2, 2);
    let first = frontier(
        &mut context,
        scope.clone(),
        &[
            (
                TypeKind::GenericParam(ty[0].clone()),
                TypeKind::GenericParam(ty[1].clone()),
            ),
            (
                array(ArrayLength::Generic(length[0].clone())),
                array(ArrayLength::Generic(length[1].clone())),
            ),
        ],
    );
    let first = component
        .admit_roots(&mut context, vec![first], "T=U,N=M")
        .unwrap();
    let second = frontier(
        &mut context,
        scope,
        &[
            (TypeKind::GenericParam(ty[0].clone()), TypeKind::Bool),
            (
                array(ArrayLength::Generic(length[0].clone())),
                array(ArrayLength::Const(7)),
            ),
        ],
    );
    component
        .admit_roots(&mut context, vec![second], "T=bool,N=7")
        .unwrap();
    let forest = component.infer_original_products(&mut context).unwrap();
    assert_eq!(forest.products.len(), 1);
    let product = &forest.products[0];
    assert_eq!(product.members.len(), 2);
    let solutions = product.result.as_ref().ok().expect("one joint MGU exists");
    assert_eq!(solutions.len(), 1);
    let solved = &solutions[0];
    assert!(Arc::ptr_eq(&solved.branch.lineage, &product.members));
    for parameter in &ty {
        assert_eq!(solved.branch.bindings.get(parameter), Some(&TypeKind::Bool));
    }
    for parameter in &length {
        assert_eq!(
            solved.branch.const_bindings.get(parameter),
            Some(&ArrayLength::Const(7))
        );
    }
    assert!(solved.effects.predicate.is_unconstrained());
    let ValueDefinition::Constraint { frontiers, .. } =
        &forest.original.value(&first).unwrap().definition
    else {
        unreachable!()
    };
    let original = &frontiers[0].transaction.frontier[0];
    assert_eq!(
        original.bindings.get(&ty[0]),
        Some(&TypeKind::GenericParam(ty[1].clone()))
    );
    assert_eq!(
        original.const_bindings.get(&length[0]),
        Some(&ArrayLength::Generic(length[1].clone()))
    );
    assert!(!original.bindings.contains_key(&ty[1]));
    assert!(!original.const_bindings.contains_key(&length[1]));
}

#[test]
fn original_products_keep_conflict_addresses_and_exclude_diagnostic_witnesses() {
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let mut component = Component::new(&mut context).unwrap();
    let (scope, ty, length) = parameters(1, 1);
    let equations = |value, count| {
        vec![
            (TypeKind::GenericParam(ty[0].clone()), value),
            (
                array(ArrayLength::Generic(length[0].clone())),
                array(ArrayLength::Const(count)),
            ),
        ]
    };
    let root = frontier(&mut context, scope.clone(), &equations(TypeKind::Bool, 3));
    component
        .admit_roots(&mut context, vec![root], "first")
        .unwrap();
    let mut roots = Vec::new();
    for (value, count) in [
        (TypeKind::Bool, 3),
        (TypeKind::String, 3),
        (TypeKind::Bool, 4),
    ] {
        roots.push(frontier(
            &mut context,
            scope.clone(),
            &equations(value, count),
        ));
    }
    let second = component
        .admit_roots(&mut context, roots, "second")
        .unwrap();
    let forest = component.infer_original_products(&mut context).unwrap();
    assert_eq!(forest.products.len(), 3);
    assert!(forest.products[0].result.is_ok());
    let failure = forest.products[1].result.as_ref().err().unwrap();
    assert!(
        matches!(&failure.check, OriginalProductCheck::TypeBinding {member,parameter} if member.producer == second.ordinal && member.frontier == 1 && parameter == &ty[0])
    );
    assert_eq!(failure.reason, TypeConstraintRejection::Mismatch);
    let failure = forest.products[2].result.as_ref().err().unwrap();
    assert!(
        matches!(&failure.check, OriginalProductCheck::ConstBinding {member,parameter} if member.producer == second.ordinal && member.frontier == 2 && parameter == &length[0])
    );
    assert_eq!(failure.reason, TypeConstraintRejection::Mismatch);
}

#[test]
fn cross_root_structural_occurs_cycles_remain_original_product_rejections() {
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let mut component = Component::new(&mut context).unwrap();
    let (scope, ty, _) = parameters(2, 0);
    for (left, right) in [(0, 1), (1, 0)] {
        let root = frontier(
            &mut context,
            scope.clone(),
            &[(
                TypeKind::GenericParam(ty[left].clone()),
                TypeKind::Tuple(vec![TypeKind::GenericParam(ty[right].clone())]),
            )],
        );
        component
            .admit_roots(&mut context, vec![root], "acyclic original")
            .unwrap();
    }
    let forest = component.infer_original_products(&mut context).unwrap();
    assert_eq!(forest.products.len(), 1);
    assert!(matches!(
        forest.products[0].result.as_ref().err().unwrap().reason,
        TypeConstraintRejection::CyclicInstantiation { .. }
    ));
    assert_eq!(forest.products[0].members.len(), 2);
}

#[test]
fn an_unchosen_original_cannot_lend_required_parameters_to_another_product() {
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let mut component = Component::new(&mut context).unwrap();
    let first = frontier(&mut context, TypeConstraintParameterScope::empty(), &[]);
    let first_id = first.transaction.application;
    let (scope, _, _) = parameters(1, 0);
    let required = scope.iter().next().unwrap().0.clone();
    let second = frontier(&mut context, scope, &[]);
    let second_id = second.transaction.application;
    component
        .admit_roots(&mut context, vec![first, second], "initializer")
        .unwrap();
    let forest = component.infer_original_products(&mut context).unwrap();
    assert_eq!(forest.products.len(), 2);
    let solved = &forest.products[0].result.as_ref().ok().unwrap()[0];
    assert!(solved.branch.applications.application(first_id).is_some());
    assert!(solved.branch.applications.application(second_id).is_none());
    assert!(
        matches!(&forest.products[1].result.as_ref().err().unwrap().reason,TypeConstraintRejection::IncompleteInstantiation {parameter: crate::types::constraints::ConstraintGenericParameterId::Type(parameter)} if parameter == &required)
    );
    let mut component = Component::new(&mut context).unwrap();
    component
        .admit_roots(&mut context, Vec::new(), "rejected original")
        .unwrap();
    assert!(
        component
            .infer_original_products(&mut context)
            .unwrap()
            .products
            .is_empty()
    );
}

#[test]
fn identical_scope_arities_do_not_identify_independently_opened_ambient_binders() {
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let mut component = Component::new(&mut context).unwrap();
    let (scope, ty, _) = parameters(1, 0);
    for _ in 0..2 {
        let enclosing = context.enter_binder_scope(GenericBinder::new(1, 0, 0));
        let value = TypeKind::GenericParam(context.lexical_scope().bound_type(0, 0).unwrap());
        let root = frontier(
            &mut context,
            scope.clone(),
            &[(TypeKind::GenericParam(ty[0].clone()), value)],
        );
        component
            .admit_roots(&mut context, vec![root], "ambient")
            .unwrap();
        context.restore_binder_scope(enclosing);
    }
    let forest = component.infer_original_products(&mut context).unwrap();
    assert_eq!(forest.products[0].scope.shape().binders().len(), 2);
    assert_eq!(
        forest.products[0].result.as_ref().err().unwrap().reason,
        TypeConstraintRejection::Mismatch
    );
    assert!(context.lexical_scope().binders().is_empty());
}

fn scoped_function(scope: &GenericScope, ambient_depth: u32) -> TypeKind {
    let binder = GenericBinder::new(1, 1, 1);
    let nested = scope.with_binder(binder);
    TypeKind::function_with_binder(
        binder,
        [
            TypeKind::GenericParam(nested.bound_type(0, 0).unwrap()),
            TypeKind::Array {
                item: Box::new(TypeKind::GenericParam(
                    nested.bound_type(ambient_depth, 0).unwrap(),
                )),
                len: ArrayLength::Generic(nested.bound_const(ambient_depth, 0).unwrap()),
            },
        ],
        TypeKind::Array {
            item: Box::new(TypeKind::Bool),
            len: ArrayLength::Generic(nested.bound_const(0, 0).unwrap()),
        },
        EffectRow::open(
            EffectSet::new(),
            nested.bound_effect(ambient_depth, 0).unwrap(),
        ),
    )
}

#[test]
fn shared_ambient_frames_transport_under_own_binders_across_different_scope_shapes() {
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let mut component = Component::new(&mut context).unwrap();
    let (scope, ty, _) = parameters(1, 0);
    let root_scope = context.enter_binder_scope(GenericBinder::new(1, 1, 1));
    let value = scoped_function(context.lexical_scope(), 1);
    let first = frontier(
        &mut context,
        scope.clone(),
        &[(TypeKind::GenericParam(ty[0].clone()), value)],
    );
    component
        .admit_roots(&mut context, vec![first], "outer")
        .unwrap();
    let outer = context.enter_binder_scope(GenericBinder::new(1, 0, 0));
    let expected = scoped_function(context.lexical_scope(), 2);
    let second = frontier(
        &mut context,
        scope,
        &[(TypeKind::GenericParam(ty[0].clone()), expected.clone())],
    );
    component
        .admit_roots(&mut context, vec![second], "outer beneath another frame")
        .unwrap();
    context.restore_binder_scope(outer);
    context.restore_binder_scope(root_scope);
    let forest = component.infer_original_products(&mut context).unwrap();
    let product = &forest.products[0];
    assert_eq!(product.scope.shape().binders().len(), 2);
    let solution = &product
        .result
        .as_ref()
        .ok()
        .expect("same original ambient binder")[0];
    assert_eq!(solution.branch.bindings.get(&ty[0]), Some(&expected));
    assert!(context.lexical_scope().binders().is_empty());
}

#[test]
fn live_use_tickets_and_cancellation_cannot_publish_a_partial_original_forest() {
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let mut component = Component::new(&mut context).unwrap();
    let root = frontier(&mut context, TypeConstraintParameterScope::empty(), &[]);
    let value = component
        .admit_roots(&mut context, vec![root], "initializer")
        .unwrap();
    let use_ = component.issue_use(&mut context, &[value]).unwrap();
    assert!(matches!(
        component.infer_original_products(&mut context),
        Err(PreparedValueFailure::UseState)
    ));
    drop(use_);
    let component = Component::new(&mut context).unwrap();
    cancellation.store(true, Ordering::Relaxed);
    assert!(matches!(
        component.infer_original_products(&mut context),
        Err(PreparedValueFailure::Constraint(
            TypeConstraintError::Abort(_)
        ))
    ));
    cancellation.store(false, Ordering::Relaxed);
    assert!(context.lexical_scope().binders().is_empty());
}

#[test]
fn cloned_imported_leases_share_inference_without_making_the_parent_a_source_root() {
    use crate::types::constraints::{
        ConstraintGenericParameterId, application::ConstraintApplicationScope,
    };
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let mut component = Component::new(&mut context).unwrap();
    let (scope, ty, _) = parameters(2, 0);
    let parent = context
        .start_path(ConstraintApplicationScope::new((), scope))
        .unwrap();
    let parent_id = parent.root_id();
    let lease = parent
        .applications
        .import_parameters(
            &ty.iter()
                .cloned()
                .map(ConstraintGenericParameterId::Type)
                .collect::<Vec<_>>(),
        )
        .unwrap();
    drop(parent);
    for actual in [TypeKind::GenericParam(ty[1].clone()), TypeKind::Bool] {
        let mut lower = TypeConstraintTransaction::initialize_with_imported(
            &mut context,
            (),
            TypeConstraintParameterScope::empty(),
            None,
            Some(lease.clone()),
        )
        .unwrap();
        lower.constrain(
            &mut context,
            &TypeKind::GenericParam(ty[0].clone()),
            &actual,
            ConstraintAcceptance::PatternAcceptsActual,
        );
        lower.request_projection(
            &mut context,
            (),
            &TypeKind::I64,
            TypeConstraintProjectionClosure::Closed,
        );
        let root = lower.suspend_result(&mut context, ()).unwrap();
        component
            .admit_roots(&mut context, vec![root], "imported original")
            .unwrap();
    }
    let forest = component.infer_original_products(&mut context).unwrap();
    assert_eq!(forest.products.len(), 1);
    assert_eq!(forest.products[0].members.len(), 2);
    let solution = &forest.products[0].result.as_ref().ok().unwrap()[0];
    assert!(
        solution
            .branch
            .applications
            .application(parent_id)
            .is_none()
    );
    for parameter in &ty {
        assert_eq!(
            solution.branch.bindings.get(parameter),
            Some(&TypeKind::Bool)
        );
    }
}

#[test]
fn exact_agreement_keeps_whole_choice_types_and_rejects_bottom_subtyping() {
    let cancellation = AtomicBool::new(false);
    let choice = TypeKind::Tuple(vec![TypeKind::Choice(vec![
        TypeKind::Bool,
        TypeKind::String,
    ])]);
    let function =
        |result| TypeKind::function_with_effects([], result, EffectRow::closed(EffectSet::new()));
    for (left, right, succeeds) in [
        (choice.clone(), choice, true),
        (function(TypeKind::Never), function(TypeKind::Bool), false),
    ] {
        let mut context = context(&cancellation);
        let mut component = Component::new(&mut context).unwrap();
        let (scope, ty, _) = parameters(1, 0);
        for value in [left, right] {
            let root = frontier(
                &mut context,
                scope.clone(),
                &[(TypeKind::GenericParam(ty[0].clone()), value)],
            );
            component
                .admit_roots(&mut context, vec![root], "whole type")
                .unwrap();
        }
        let forest = component.infer_original_products(&mut context).unwrap();
        assert_eq!(forest.products.len(), 1);
        assert_eq!(forest.products[0].result.is_ok(), succeeds);
        if !succeeds {
            assert_eq!(
                forest.products[0].result.as_ref().err().unwrap().reason,
                TypeConstraintRejection::Mismatch
            );
        }
    }
}

#[test]
fn another_original_binding_can_resolve_a_nonprincipal_effect_relation_before_completion() {
    use crate::{
        effect_row::{EffectConstraintEligibility, EffectConstraintVariable},
        types::GenericEffectParameterId,
    };
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let mut component = Component::new(&mut context).unwrap();
    let owner = GenericParameterOwnerId::Detached(DetachedGenericOwnerId::new(9482));
    let ty = GenericTypeParameterId::new(owner.clone(), 0);
    let effects: Vec<crate::types::GenericEffectReference> = (0..2)
        .map(|slot| GenericEffectParameterId::new(owner.clone(), slot).into())
        .collect();
    let scope = TypeConstraintParameterScope::seal_call_scope(
        GenericBinder::EMPTY,
        [TypeConstraintTypeParameterScopeRow::new(
            ty.clone(),
            TypeConstraintParameterEligibility::Bindable,
        )],
        [],
        TypeConstraintEffectScope::seal_call_scope(
            effects.iter().map(|reference| {
                EffectConstraintVariable::new(
                    reference.clone(),
                    EffectConstraintEligibility::Bindable,
                )
            }),
            [],
        )
        .unwrap(),
        [],
        [],
    )
    .unwrap();
    let ty = scope.type_reference(&ty.into()).unwrap();
    let effects: Vec<_> = effects
        .iter()
        .map(|reference| scope.effect_reference(reference).unwrap())
        .collect();
    let row = |index: usize| EffectRow::open(EffectSet::new(), effects[index].clone());
    let function = |row| TypeKind::function_with_effects([], TypeKind::Bool, row);
    let log = EffectRow::closed(EffectSet::from_labels(["log.write"]).unwrap());
    let mut first = frontier(
        &mut context,
        scope.clone(),
        &[(TypeKind::GenericParam(ty.clone()), function(row(0)))],
    );
    let union = row(0).union(&row(1), &mut context).unwrap();
    first.transaction.frontier[0]
        .effects
        .constrain_subset(&log, &union, &mut context)
        .unwrap();
    assert!(matches!(
        first.transaction.frontier[0].effects.complete(&mut context),
        Err(TypeConstraintError::Rejected(_))
    ));
    let first = component
        .admit_roots(&mut context, vec![first], "original disjunction")
        .unwrap();
    let second = frontier(
        &mut context,
        scope,
        &[(TypeKind::GenericParam(ty), function(log.clone()))],
    );
    component
        .admit_roots(&mut context, vec![second], "original resolves e1")
        .unwrap();
    let forest = component.infer_original_products(&mut context).unwrap();
    let solution = &forest.products[0]
        .result
        .as_ref()
        .ok()
        .expect("joint effect inference is principal")[0];
    assert_eq!(solution.effects.substitution.get(&effects[0]), Some(&log));
    assert_eq!(
        solution.effects.substitution.get(&effects[1]),
        Some(&EffectRow::closed(EffectSet::new()))
    );
    let ValueDefinition::Constraint { frontiers, .. } =
        &forest.original.value(&first).unwrap().definition
    else {
        unreachable!()
    };
    assert!(matches!(
        frontiers[0].transaction.frontier[0]
            .effects
            .complete(&mut context),
        Err(TypeConstraintError::Rejected(_))
    ));
}

#[test]
fn a_later_false_fit_and_different_use_result_do_not_constrain_original_inference() {
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let mut component = Component::new(&mut context).unwrap();
    let (scope, ty, _) = parameters(1, 0);
    let root = frontier(
        &mut context,
        scope,
        &[(TypeKind::GenericParam(ty[0].clone()), TypeKind::Bool)],
    );
    let producer = component
        .admit_roots(&mut context, vec![root], "original")
        .unwrap();
    let mut use_ = component.issue_use(&mut context, &[producer]).unwrap();
    while let Some(alternative) = component
        .next_use_alternative(&mut context, &mut use_)
        .unwrap()
    {
        let outcome = if alternative.blocked {
            PreparedValueOutcome::Blocked("producer diagnostic")
        } else {
            PreparedValueOutcome::Conditional {
                fits: PreparedEffectPredicate::Canonical(EffectPredicate::impossible()),
                accepted: PreparedValueOutput {
                    ty: TypeKind::String
                        .prepare_effects(&GenericScope::default(), &mut context)
                        .unwrap(),
                    scope: GenericScope::default(),
                    preparation: "later result",
                },
                rejected: "false fit",
            }
        };
        component
            .submit_use(&mut context, &mut use_, alternative, outcome)
            .unwrap();
    }
    let use_ = component.finish_use(use_).unwrap();
    component.retain_use(&mut context, use_).unwrap();
    let forest = component.infer_original_products(&mut context).unwrap();
    assert_eq!(forest.products.len(), 1);
    let solution = &forest.products[0].result.as_ref().ok().unwrap()[0];
    assert_eq!(solution.branch.bindings.get(&ty[0]), Some(&TypeKind::Bool));
    assert_eq!(forest.original.values.len(), 2);
}

#[test]
fn original_product_publication_obeys_an_inclusive_shared_node_budget() {
    use crate::types::constraints::context::{
        TypeConstraintAccounting, TypeConstraintContextIssuer, TypeConstraintWorkReport,
    };
    use std::cell::Cell;
    struct Accounting<'a> {
        nodes: Rc<Cell<u64>>,
        cancellation: &'a AtomicBool,
        limits: TypeConstraintLimits,
    }
    impl TypeConstraintAccounting for Accounting<'_> {
        fn charge_constraint(
            &mut self,
            delta: &TypeConstraintWorkReport,
            _: TypeConstraintLimits,
        ) -> Result<(), TypeConstraintError> {
            self.nodes.set(self.nodes.get() + delta.nodes());
            Ok(())
        }
        fn commit(&mut self) {}
    }
    impl<'a> TypeConstraintContextIssuer<'a> for Accounting<'a> {
        fn context_limits(&self) -> TypeConstraintLimits {
            self.limits
        }
        fn context_cancellation(&self) -> &'a AtomicBool {
            self.cancellation
        }
    }
    let cancellation = AtomicBool::new(false);
    let execute = |limit| {
        let nodes = Rc::new(Cell::new(0));
        let mut context =
            TypeConstraintContext::<_, NoConstraintClient>::with_accounting(Accounting {
                nodes: Rc::clone(&nodes),
                cancellation: &cancellation,
                limits: TypeConstraintLimits::new(65536, limit, 1024, 1024),
            });
        let component = Component::new(&mut context).unwrap();
        let result = component.infer_original_products(&mut context);
        (nodes.get(), result)
    };
    let (required, result) = execute(65536);
    assert!(result.is_ok());
    assert!(required > 3);
    let (used, result) = execute(required);
    assert!(result.is_ok());
    assert_eq!(used, required);
    let (_, result) = execute(required - 1);
    assert!(matches!(
        result,
        Err(PreparedValueFailure::Constraint(
            TypeConstraintError::Abort(
                crate::types::constraints::TypeConstraintAbort::NodeLimit { .. }
            )
        ))
    ));
}

#[test]
fn joint_inference_cannot_capture_a_frame_absent_from_an_original_application() {
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let mut component = Component::new(&mut context).unwrap();
    let (scope, ty, _) = parameters(1, 0);
    let enclosing = context.enter_binder_scope(GenericBinder::new(1, 0, 0));
    let first = frontier(&mut context, scope.clone(), &[]);
    let first = component
        .admit_roots(&mut context, vec![first], "no binding in its own frame")
        .unwrap();
    context.restore_binder_scope(enclosing);
    let enclosing = context.enter_binder_scope(GenericBinder::new(1, 0, 0));
    let actual = TypeKind::GenericParam(context.lexical_scope().bound_type(0, 0).unwrap());
    let second = frontier(
        &mut context,
        scope,
        &[(TypeKind::GenericParam(ty[0].clone()), actual)],
    );
    component
        .admit_roots(&mut context, vec![second], "another original frame")
        .unwrap();
    context.restore_binder_scope(enclosing);
    let forest = component.infer_original_products(&mut context).unwrap();
    let failure = forest.products[0]
        .result
        .as_ref()
        .err()
        .expect("joint visibility is not an original capture capability");
    assert!(
        matches!(&failure.check,OriginalProductCheck::BindingScope {member,parameter:crate::types::constraints::ConstraintGenericParameterId::Type(parameter)} if member.producer==first.ordinal && parameter==&ty[0])
    );
    assert!(
        matches!(&failure.reason,TypeConstraintRejection::EscapingInstantiation {parameter:crate::types::constraints::ConstraintGenericParameterId::Type(parameter)} if parameter==&ty[0])
    );
    assert!(context.lexical_scope().binders().is_empty());
}
