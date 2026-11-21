use super::*;
use crate::{
    effect_row::{
        EffectConstraintEligibility, EffectConstraintVariable, EffectPredicate, EffectRow,
        prepared::{
            PreparedEffectGraph, PreparedEffectInvariant, PreparedEffectPredicate,
            PreparedEffectRelation, PreparedEffectRow,
        },
    },
    effects::EffectSet,
    types::{
        GenericBinder, PreparedEffects,
        constraints::{
            NoConstraintClient, TypeConstraintAbort, TypeConstraintConstEligibility,
            TypeConstraintParameterEligibility,
            context::{
                LocalConstraintAccounting, TypeConstraintConstParameterScopeRow,
                TypeConstraintEffectScope, TypeConstraintLimits, TypeConstraintParameterScope,
                TypeConstraintTypeParameterScopeRow,
            },
        },
    },
};
use std::sync::atomic::{AtomicBool, Ordering};

type Context<'a> = TypeConstraintContext<'a, LocalConstraintAccounting<'a>, NoConstraintClient>;

fn context(cancellation: &AtomicBool) -> Context<'_> {
    Context::with_accounting(LocalConstraintAccounting::new(
        TypeConstraintLimits::new(131_072, 65_536, 1024, 1024),
        cancellation,
    ))
}

fn parameters() -> TypeConstraintParameterScope {
    let binder = GenericBinder::new(1, 1, 1);
    let scope = GenericScope::default().with_binder(binder);
    TypeConstraintParameterScope::seal_call_scope(
        binder,
        [TypeConstraintTypeParameterScopeRow::new(
            scope.bound_type(0, 0).unwrap(),
            TypeConstraintParameterEligibility::Bindable,
        )],
        [TypeConstraintConstParameterScopeRow::new(
            scope.bound_const(0, 0).unwrap(),
            TypeConstraintConstEligibility::Bindable,
        )],
        TypeConstraintEffectScope::seal_call_scope(
            [EffectConstraintVariable::new(
                scope.bound_effect(0, 0).unwrap(),
                EffectConstraintEligibility::Bindable,
            )],
            [],
        )
        .unwrap(),
        [],
        [],
    )
    .unwrap()
}

fn open(reference: GenericEffectReference) -> PreparedEffectRow {
    PreparedEffectRow::Canonical(EffectRow::open(EffectSet::new(), reference))
}

fn truth() -> PreparedEffectPredicate {
    PreparedEffectPredicate::Canonical(EffectPredicate::unconstrained())
}

#[test]
fn prepared_opening_keeps_function_local_ports_and_lifts_the_actual_effect_application() {
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let mut graph = PreparedEffectGraph::default();
    context.with_binder(GenericBinder::new(0, 0, 1), |context| {
        let parameters = parameters();
        let declaration = parameters.completed_contract().template_scope();
        let type_key = declaration.bound_type(0, 0)?;
        let const_key = declaration.bound_const(0, 0)?;
        let effect_key = declaration.bound_effect(0, 0)?;
        let expected_type = parameters.type_reference(&type_key).unwrap();
        let expected_const = parameters.const_reference(&const_key).unwrap();
        let effect_variable = parameters.effect_reference(&effect_key).unwrap();
        let owner = context.fixture_application_scope((), parameters, &mut graph)?;
        let function = owner.effect_function(&effect_variable).unwrap().clone();
        let path = context.start_path(owner, &mut graph)?;
        let ambient = context.capture_lexical_scope()?;
        let binder = GenericBinder::new(1, 1, 1);
        let source = GenericScope::default().with_binder(GenericBinder::new(1, 1, 1)).with_binder(binder);
        let effects = graph.union(vec![open(source.bound_effect(1, 0)?), open(source.bound_effect(0, 0)?)], context)?;
        let predicate = graph.relate(PreparedEffectRelation::Equal,
            open(source.bound_effect(1, 0)?), open(source.bound_effect(0, 0)?), context)?;
        let template: TypeKind<PreparedEffects> = TypeKind::function_with_contract(
            binder, predicate,
            [TypeKind::Tuple(vec![
                TypeKind::GenericParam(source.bound_type(1, 0)?),
                TypeKind::GenericParam(source.bound_type(0, 0)?),
                TypeKind::Array { item: Box::new(TypeKind::I64), len: ArrayLength::Generic(source.bound_const(1, 0)?) },
                TypeKind::Array { item: Box::new(TypeKind::I64), len: ArrayLength::Generic(source.bound_const(0, 0)?) },
            ])], TypeKind::GenericParam(source.bound_type(0, 0)?), effects,
        );
        let opened = context.open_template_type(&template, &path, path.root_id(), &mut graph)?;
        let TypeKind::Function { binder: opened_binder, predicate, params, return_type, effects } = opened else { unreachable!() };
        assert_eq!(opened_binder, binder);
        let TypeKind::Tuple(items) = &params[0] else { unreachable!() };
        assert_eq!(items[0], TypeKind::GenericParam(expected_type));
        assert_eq!(items[1], TypeKind::GenericParam(source.bound_type(0, 0)?));
        assert!(matches!(&items[2], TypeKind::Array { len: ArrayLength::Generic(reference), .. } if reference == &expected_const));
        assert!(matches!(&items[3], TypeKind::Array { len: ArrayLength::Generic(reference), .. } if reference == &source.bound_const(0, 0)?));
        assert_eq!(*return_type, TypeKind::GenericParam(source.bound_type(0, 0)?));
        let target = ambient.shape().with_binder(binder);
        let expected = std::collections::BTreeSet::from([target.bound_effect(0, 0)?, target.bound_effect(1, 0)?]);
        let mut references = std::collections::BTreeSet::new();
        graph.read().visit_row_parameters(&effects, context, &mut |reference, _| { references.insert(reference.clone()); Ok(()) })?;
        assert_eq!(references, expected);
        references.clear();
        graph.read().visit_predicate_parameters(&predicate, context, &mut |reference, _| { references.insert(reference.clone()); Ok(()) })?;
        assert_eq!(references, expected);
        assert_eq!(graph.read().unknown_dependencies(&function)?, [ambient.shape().bound_effect(0, 0)?]);
        assert_eq!(context.lexical_frames(), &ambient);
        Ok(())
    }).unwrap();
}

#[test]
fn a_later_invalid_length_releases_all_earlier_opening_terms() {
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let mut graph = PreparedEffectGraph::default();
    let parameters = parameters();
    let formal = parameters
        .completed_contract()
        .template_scope()
        .bound_effect(0, 0)
        .unwrap();
    let variable = parameters.effect_reference(&formal).unwrap();
    let owner = context
        .fixture_application_scope((), parameters, &mut graph)
        .unwrap();
    let function = owner.effect_function(&variable).unwrap().clone();
    let path = context.start_path(owner, &mut graph).unwrap();
    let template: TypeKind<PreparedEffects> = TypeKind::Tuple(vec![
        TypeKind::function_with_contract(
            GenericBinder::EMPTY,
            truth(),
            [],
            TypeKind::I64,
            open(formal),
        ),
        TypeKind::Array {
            item: Box::new(TypeKind::I64),
            len: ArrayLength::Inferred,
        },
    ]);
    let retained = graph.retained_terms();
    let scope = context.capture_lexical_scope().unwrap();
    assert!(
        context
            .open_template_type(&template, &path, path.root_id(), &mut graph)
            .is_err()
    );
    assert_eq!(graph.retained_terms(), retained);
    graph.read().validate_unknown(&function).unwrap();
    assert_eq!(context.lexical_frames(), &scope);
}

#[test]
fn an_omitted_effect_annotation_stays_unknown_after_template_opening() {
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let mut graph = PreparedEffectGraph::default();
    let owner = context
        .fixture_application_scope((), TypeConstraintParameterScope::empty(), &mut graph)
        .unwrap();
    let path = context.start_path(owner, &mut graph).unwrap();
    let template: TypeKind<PreparedEffects> = TypeKind::function_with_contract(
        GenericBinder::EMPTY,
        truth(),
        [],
        TypeKind::I64,
        PreparedEffectRow::Canonical(EffectRow::unknown()),
    );
    let opened = context
        .open_template_type(&template, &path, path.root_id(), &mut graph)
        .unwrap();
    assert_eq!(opened, template);
}

#[test]
fn template_opening_preserves_a_foreign_graph_failure() {
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let mut graph = PreparedEffectGraph::default();
    let owner = context
        .fixture_application_scope((), TypeConstraintParameterScope::empty(), &mut graph)
        .unwrap();
    let path = context.start_path(owner, &mut graph).unwrap();
    let mut foreign = PreparedEffectGraph::default();
    let row = foreign
        .union(
            vec![PreparedEffectRow::Canonical(EffectRow::closed(
                EffectSet::new(),
            ))],
            &mut context,
        )
        .unwrap();
    let template: TypeKind<PreparedEffects> =
        TypeKind::function_with_contract(GenericBinder::EMPTY, truth(), [], TypeKind::I64, row);
    let retained = graph.retained_terms();
    let error = context.open_template_type(&template, &path, path.root_id(), &mut graph);
    assert!(matches!(
        error,
        Err(TypeConstraintError::Invariant(
            TypeConstraintInvariant::PreparedEffect(
                crate::effect_row::prepared::PreparedEffectFailure::Invariant(
                    PreparedEffectInvariant::ForeignIssuer
                ),
            )
        ))
    ));
    assert_eq!(graph.retained_terms(), retained);
}

#[test]
fn cancelled_template_opening_allocates_no_terms() {
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let mut graph = PreparedEffectGraph::default();
    let owner = context
        .fixture_application_scope((), parameters(), &mut graph)
        .unwrap();
    let path = context.start_path(owner, &mut graph).unwrap();
    let retained = graph.retained_terms();
    cancellation.store(true, Ordering::Relaxed);
    let template: TypeKind<PreparedEffects> = TypeKind::I64;
    assert!(matches!(
        context.open_template_type(&template, &path, path.root_id(), &mut graph),
        Err(TypeConstraintError::Abort(TypeConstraintAbort::Cancelled))
    ));
    assert_eq!(graph.retained_terms(), retained);
}

#[test]
fn opening_admits_unused_rigid_source_inputs_through_the_branch_gamma() {
    use crate::types::{DetachedGenericOwnerId, GenericEffectParameterId, GenericParameterOwnerId};
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let mut graph = PreparedEffectGraph::default();
    let captured: GenericEffectReference = GenericEffectParameterId::new(
        GenericParameterOwnerId::Detached(DetachedGenericOwnerId::new(98_501)),
        0,
    )
    .into();
    let original = context
        .fixture_original_context_with_rigid(
            crate::types::constraints::original_context::OriginalRigidScope {
                types: &[],
                consts: &[],
                effects: std::slice::from_ref(&captured),
                imported: None,
            },
        )
        .unwrap();
    let parameters = parameters();
    let formal = parameters
        .completed_contract()
        .template_scope()
        .bound_effect(0, 0)
        .unwrap();
    let owner = context
        .application_scope((), parameters, original.clone(), &mut graph)
        .unwrap();
    let path = context
        .start_path_with_imported(owner, None, original, graph.read())
        .unwrap();
    assert_eq!(
        context.effect_eligibility(&captured, path.projection_view()),
        Some(EffectConstraintEligibility::Rigid)
    );
    let template: TypeKind<PreparedEffects> = TypeKind::function_with_contract(
        GenericBinder::EMPTY,
        truth(),
        [],
        TypeKind::I64,
        open(formal),
    );
    let opened = context
        .open_template_type(&template, &path, path.root_id(), &mut graph)
        .unwrap();
    let TypeKind::Function { effects, .. } = opened else {
        unreachable!()
    };
    path.effects
        .ambient
        .validate_row(&effects, graph.read(), &mut context)
        .unwrap();
    let mut references = std::collections::BTreeSet::new();
    graph
        .read()
        .visit_row_parameters(&effects, &mut context, &mut |reference, _| {
            references.insert(reference.clone());
            Ok(())
        })
        .unwrap();
    assert_eq!(references, std::collections::BTreeSet::from([captured]));
}

#[test]
fn mathematical_gamma_does_not_grant_a_template_its_parents_private_capture() {
    use crate::types::{DetachedGenericOwnerId, GenericEffectParameterId, GenericParameterOwnerId};
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let mut graph = PreparedEffectGraph::default();
    let captured: GenericEffectReference = GenericEffectParameterId::new(
        GenericParameterOwnerId::Detached(DetachedGenericOwnerId::new(98_502)),
        0,
    )
    .into();
    let original = context
        .fixture_original_context_with_rigid(
            crate::types::constraints::original_context::OriginalRigidScope {
                types: &[],
                consts: &[],
                effects: std::slice::from_ref(&captured),
                imported: None,
            },
        )
        .unwrap();
    let parent_parameters = parameters();
    let formal = parent_parameters
        .completed_contract()
        .template_scope()
        .bound_effect(0, 0)
        .unwrap();
    let parent_variable = parent_parameters.effect_reference(&formal).unwrap();
    let parent = context
        .application_scope((), parent_parameters, original.clone(), &mut graph)
        .unwrap();
    let parent = context
        .start_path_with_imported(parent, None, original, graph.read())
        .unwrap();
    let imported = parent
        .applications
        .import_parameters(&[parent_variable.into()], &mut context)
        .unwrap();
    let original = context.fixture_original_context(Some(&imported)).unwrap();
    let owner = context
        .application_scope((), parameters(), original.clone(), &mut graph)
        .unwrap();
    let child = context
        .start_path_with_imported(owner, Some(imported), original, graph.read())
        .unwrap();
    assert_eq!(
        context.effect_eligibility(&captured, child.projection_view()),
        Some(EffectConstraintEligibility::Rigid)
    );
    let forbidden: TypeKind<PreparedEffects> = TypeKind::function_with_contract(
        GenericBinder::EMPTY,
        truth(),
        [],
        TypeKind::I64,
        open(captured),
    );
    let retained = graph.retained_terms();
    assert!(
        context
            .open_template_type(&forbidden, &child, child.root_id(), &mut graph)
            .is_err()
    );
    assert_eq!(graph.retained_terms(), retained);
    let permitted: TypeKind<PreparedEffects> = TypeKind::function_with_contract(
        GenericBinder::EMPTY,
        truth(),
        [],
        TypeKind::I64,
        open(formal),
    );
    let opened = context
        .open_template_type(&permitted, &child, child.root_id(), &mut graph)
        .unwrap();
    let TypeKind::Function { effects, .. } = opened else {
        unreachable!()
    };
    child
        .effects
        .ambient
        .validate_row(&effects, graph.read(), &mut context)
        .unwrap();
}
