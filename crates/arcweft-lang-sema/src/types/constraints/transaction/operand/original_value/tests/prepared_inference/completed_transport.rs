use super::*;
use crate::types::constraints::{
    ConstraintClosurePolicy, TypeConstraintParameterEligibility,
    context::TypeConstraintTypeParameterScopeRow,
    normalization::{ConstraintClosedEffects, project_closed_operand},
};
use crate::types::effect_domain::PreparedTypeEffectRoots;

#[test]
fn completed_binding_relocation_uses_the_same_principal_without_new_graph_terms() {
    let cancellation = AtomicBool::new(false);
    let mut context = TypeConstraintContext::with_accounting(LocalConstraintAccounting::new(
        TypeConstraintLimits::new(131_072, 65_536, 1024, 1024),
        &cancellation,
    ));
    let mut graph = PreparedEffectGraph::default();
    context
        .with_binder(GenericBinder::new(0, 0, 1), |context| {
            let source = Source::new(98_928);
            let original = context.original_producer_context(&source)?;
            let parameters = TypeConstraintParameterScope::seal_call_scope(
                GenericBinder::EMPTY,
                [TypeConstraintTypeParameterScopeRow::new(
                    source.types[0].clone(),
                    TypeConstraintParameterEligibility::Bindable,
                )],
                [],
                crate::types::constraints::TypeConstraintEffectScope::seal_call_scope([], [])?,
                [],
                [],
            )?;
            let application =
                context.application_scope((), parameters, original.clone(), &mut graph)?;
            let key = application
                .parameters()
                .type_reference(&source.types[0])
                .unwrap();
            let path = context.start_path_with_imported(
                application,
                None,
                original.clone(),
                graph.read(),
            )?;
            let local = GenericBinder::new(1, 1, 1);
            let nested = original.scope().with_binder(local);
            let formal = GenericScope::default()
                .with_binder(GenericBinder::new(0, 0, 1))
                .bound_effect(0, 0)?;
            let named = graph.declare(
                vec![formal.clone()],
                EffectPredicate::unconstrained(),
                context,
            )?;
            let application = graph.application(
                &named,
                vec![PreparedEffectRow::Canonical(EffectRow::open(
                    EffectSet::new(),
                    nested.bound_effect(1, 0)?,
                ))],
                context,
            )?;
            let row = graph.union(
                vec![
                    application,
                    PreparedEffectRow::Canonical(EffectRow::open(
                        EffectSet::new(),
                        nested.bound_effect(0, 0)?,
                    )),
                ],
                context,
            )?;
            let value = TypeKind::function_with_contract(
                local,
                PreparedEffectPredicate::Canonical(EffectPredicate::unconstrained()),
                [TypeKind::Array {
                    item: Box::new(TypeKind::GenericParam(nested.bound_type(0, 0)?)),
                    len: ArrayLength::Generic(nested.bound_const(0, 0)?),
                }],
                TypeKind::I64,
                row,
            );
            let binding = ConstraintOperand::<NoConstraintClient>::from_prepared_original_in(
                &value,
                Arc::new(original.gamma().clone()),
                original.clone(),
                &mut ConstraintEffectPhase::new(&mut graph),
                context,
            )?;
            let path = context
                .add_binding(path, key.clone(), binding.view(), &mut graph)?
                .unwrap();
            graph.prepare_body(
                &named,
                PreparedEffectRow::Canonical(EffectRow::open(EffectSet::new(), formal)),
                vec![],
                context,
            )?;
            let roots =
                PreparedTypeEffectRoots::collect([binding.value()], std::iter::empty(), context)
                    .unwrap();
            let receipt = path.prepare_effect_constraint(&mut graph, context)?;
            let frozen = graph.read().freeze_scoped(
                roots.rows(),
                roots.predicates(),
                &[&receipt],
                context,
            )?;
            let completed_original = graph
                .bind_original_scoped(&frozen, context)?
                .freeze(graph.read(), context)?;
            // There are no effect inference unknowns in this actual type-only
            // application. No missing Future row is replaced by an empty default.
            let effects = ConstraintClosedEffects::for_path(
                &path,
                &completed_original,
                &[],
                &mut graph,
                context,
            )?;
            context.with_binder(GenericBinder::new(0, 0, 1), |context| {
                let lease = path.applications.import_parameters(&[key.clone().into()])?;
                let occurrence = context.fixture_original_context(Some(&lease))?;
                let template = ConstraintOperand::<NoConstraintClient>::from_prepared_original_in(
                    &TypeKind::GenericParam(key.clone()),
                    Arc::new(occurrence.gamma().clone()),
                    occurrence,
                    &mut ConstraintEffectPhase::new(&mut graph),
                    context,
                )?;
                let retained = graph.retained_terms();
                let closed = project_closed_operand(
                    template.view(),
                    &path,
                    ConstraintClosurePolicy::ProjectionClosed,
                    &effects,
                    graph.read(),
                    context,
                )?;
                assert_eq!(graph.retained_terms(), retained);
                assert_eq!(closed.scope(), template.scope());
                assert_eq!(closed.scope().shape().binders().len(), 2);
                let canonical =
                    effects.canonicalize(closed.value(), closed.scope().shape(), context)?;
                let TypeKind::Function {
                    binder,
                    params,
                    return_type,
                    effects: row,
                    ..
                } = canonical
                else {
                    panic!("completed function")
                };
                assert_eq!(binder, local);
                let target = closed.scope().shape().with_binder(local);
                assert_eq!(
                    params,
                    vec![TypeKind::Array {
                        item: Box::new(TypeKind::GenericParam(target.bound_type(0, 0)?)),
                        len: ArrayLength::Generic(target.bound_const(0, 0)?)
                    }]
                );
                assert_eq!(*return_type, TypeKind::I64);
                let expected = EffectRow::open(EffectSet::new(), target.bound_effect(2, 0)?)
                    .union(
                        &EffectRow::open(EffectSet::new(), target.bound_effect(0, 0)?),
                        context,
                    )?;
                assert_eq!(row, expected);
                path.projection_view()
                    .admit_operand(closed.view(), graph.read(), context)?;
                assert_eq!(graph.retained_terms(), retained);
                Ok::<_, TypeConstraintError>(())
            })
        })
        .unwrap();
}
