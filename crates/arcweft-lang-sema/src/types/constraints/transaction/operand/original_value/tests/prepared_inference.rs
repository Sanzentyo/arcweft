use super::*;
use crate::types::constraints::{ConstraintPath, TypeConstraintParameterScope};

type Context<'a> = TypeConstraintContext<'a, LocalConstraintAccounting<'a>, NoConstraintClient>;

fn parent(
    context: &mut Context<'_>,
    graph: &mut PreparedEffectGraph,
    ordinal: u64,
) -> (
    OriginalProducerContext,
    ConstraintPath<NoConstraintClient>,
    GenericEffectReference,
) {
    let source = Source::new(ordinal);
    let original = context.original_producer_context(&source).unwrap();
    let parameters = inference_parameters(&source);
    let variable = parameters.effect_reference(&source.effects[0]).unwrap();
    let application = context
        .application_scope((), parameters, original.clone(), graph)
        .unwrap();
    let path = context
        .start_path_with_imported(application, None, original.clone(), graph.read())
        .unwrap();
    (original, path, variable)
}

fn input(variable: GenericEffectReference) -> TypeKind<PreparedEffects> {
    TypeKind::function_with_contract(
        GenericBinder::EMPTY,
        PreparedEffectPredicate::Canonical(EffectPredicate::unconstrained()),
        [],
        TypeKind::I64,
        PreparedEffectRow::Canonical(EffectRow::open(EffectSet::new(), variable)),
    )
}

#[test]
fn relation_prevalidation_uses_the_operands_source_before_header_rejection() {
    use crate::types::constraints::TypeConstraintParameterEligibility;
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let mut graph = PreparedEffectGraph::default();
    let allowed = Source::new(98_610);
    let free = allowed.types[0].clone();
    let GenericTypeReference::Free(parameter) = &free else {
        unreachable!()
    };
    let original = context.original_producer_context(&allowed).unwrap();
    let parameters = TypeConstraintParameterScope::new([(
        parameter.clone(),
        TypeConstraintParameterEligibility::Rigid,
    )])
    .unwrap();
    let owner = context
        .application_scope((), parameters, original.clone(), &mut graph)
        .unwrap();
    let path = context
        .start_path_with_imported(owner, None, original, graph.read())
        .unwrap();
    assert_eq!(
        context.parameter_eligibility(&free, path.projection_view()),
        Some(TypeConstraintParameterEligibility::Rigid)
    );
    let source = Source::new(98_611);
    let original = context.original_producer_context(&source).unwrap();
    let value = TypeKind::<PreparedEffects>::Vec(Box::new(TypeKind::Bool));
    let mut actual = ConstraintOperand::<NoConstraintClient>::from_prepared_original_in(
        &value,
        Arc::new(original.gamma().clone()),
        original,
        &mut ConstraintEffectPhase::new(&mut graph),
        &mut context,
    )
    .unwrap();
    // The receiving path admits this Free, but this exact source does not.
    actual.value = Arc::new(TypeKind::Vec(Box::new(TypeKind::GenericParam(
        free.clone(),
    ))));
    let expected =
        context.fixture_operand(TypeKind::<PreparedEffects>::Tuple(vec![TypeKind::Bool]));
    let retained = graph.retained_terms();
    let frames = context.lexical_frames().clone();
    let result = crate::types::compatibility::binding_plan::relate_selected_call(
        &mut graph,
        &expected.view(),
        &actual.view(),
        path,
        &mut context,
        crate::types::constraints::ConstraintAcceptance::PatternAcceptsActual,
    );
    assert!(matches!(result, Err(error) if error == references::type_out_of_scope(&free)));
    assert_eq!(graph.retained_terms(), retained);
    assert_eq!(context.lexical_frames(), &frames);
}

#[test]
fn a_final_header_projects_the_actual_function_beneath_its_local_binder() {
    use crate::effect_row::prepared::PreparedEffectRelation;
    use crate::types::constraints::normalization::{ConstraintClosedEffects, close_type_operand};
    use crate::types::constraints::transaction::ConstraintEffectField;
    let cancellation = AtomicBool::new(false);
    // This fixture runs final closure as well as source admission. The older
    // admission-only fixtures retain their smaller, unchanged work allowance.
    let mut context = TypeConstraintContext::with_accounting(LocalConstraintAccounting::new(
        TypeConstraintLimits::new(131_072, 65_536, 1024, 1024),
        &cancellation,
    ));
    let mut graph = PreparedEffectGraph::default();
    context
        .with_binder(GenericBinder::new(0, 0, 1), |context| {
            let (source, mut path, key) = parent(context, &mut graph, 98_607);
            let outer = path.lineage.scope.shape().bound_effect(0, 0)?;
            path.effects.constrain_relation(
                &mut graph,
                PreparedEffectRelation::Equal,
                &PreparedEffectRow::Canonical(EffectRow::open(EffectSet::new(), key.clone())),
                &PreparedEffectRow::Canonical(EffectRow::open(EffectSet::new(), outer)),
                context,
            )?;
            let lease = path.applications.import_parameters(&[key.clone().into()])?;
            let original = context.fixture_original_context(Some(&lease))?;
            let value: TypeKind<PreparedEffects> = TypeKind::function_with_contract(
                GenericBinder::new(0, 0, 1),
                PreparedEffectPredicate::Canonical(EffectPredicate::unconstrained()),
                [],
                TypeKind::I64,
                PreparedEffectRow::Canonical(EffectRow::open(EffectSet::new(), key.clone())),
            );
            let input = ConstraintOperand::<NoConstraintClient>::from_prepared_original_in(
                &value,
                Arc::new(original.gamma().clone()),
                original,
                &mut ConstraintEffectPhase::new(&mut graph),
                context,
            )?;
            let previous = context.enter_binder_scope(GenericBinder::new(0, 0, 1));
            let children = input.view().children(context)?;
            let header = children.function_header()?.unwrap();
            context.restore_binder_scope(previous);
            assert!(matches!(header.row(), PreparedEffectRow::Term(_)));
            let receipt = path.prepare_effect_constraint(&mut graph, context)?;
            let frozen = graph.read().freeze_scoped(
                &[header.row()],
                &[header.predicate()],
                &[&receipt],
                context,
            )?;
            let original = graph
                .bind_original_scoped(&frozen, context)?
                .freeze(graph.read(), context)?;
            let effects =
                ConstraintClosedEffects::for_path(&path, &original, &[], &mut graph, context)?;
            let mut projected = effects.project_header(&header, graph.read(), context)?;
            let row = projected.take_row(header.row());
            let predicate = projected.take_predicate(header.predicate());
            let proof = projected.finish()?.unwrap();
            assert_eq!(proof.row_keys.as_ref(), &[key]);
            assert!(proof.predicate_keys.is_empty());
            assert!(!proof.admits(
                ConstraintEffectField::Row,
                &source.rigid_scope().effects[0].clone().into()
            ));
            assert_eq!(
                row,
                PreparedEffectRow::Canonical(EffectRow::open(
                    EffectSet::new(),
                    header.lexical_scope().shape().bound_effect(1, 0)?,
                ))
            );
            assert_eq!(
                predicate,
                PreparedEffectPredicate::Canonical(EffectPredicate::unconstrained())
            );
            let closed = close_type_operand(input.view(), &path, &effects, graph.read(), context)?;
            assert_eq!(closed.scope(), &path.lineage.scope);
            let TypeKind::Function {
                effects: final_row, ..
            } = closed.value()
            else {
                unreachable!()
            };
            assert_eq!(final_row, &row);
            Ok::<_, TypeConstraintError>(())
        })
        .unwrap();
}

#[test]
fn source_inference_uses_its_actual_application_beneath_local_binders() {
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let mut graph = PreparedEffectGraph::default();
    context
        .with_binder(GenericBinder::new(0, 0, 1), |context| {
            let (source, path, variable) = parent(context, &mut graph, 98_601);
            let lease = path
                .applications
                .import_parameters(&[variable.clone().into()])?;
            let original = context.fixture_original_context(Some(&lease))?;
            let target = original.gamma().with_binder(GenericBinder::new(0, 0, 1));
            let local = GenericBinder::new(1, 1, 1);
            let occurrence = original.scope().with_binder(local);
            let formal = GenericScope::default()
                .with_binder(GenericBinder::new(0, 0, 1))
                .bound_effect(0, 0)?;
            let declaration = graph.declare(
                vec![formal.clone()],
                EffectPredicate::unconstrained(),
                context,
            )?;
            let application = graph.application(
                &declaration,
                vec![PreparedEffectRow::Canonical(EffectRow::open(
                    EffectSet::new(),
                    variable.clone(),
                ))],
                context,
            )?;
            let effects = graph.union(
                vec![
                    application,
                    PreparedEffectRow::Canonical(EffectRow::open(
                        EffectSet::new(),
                        occurrence.bound_effect(0, 0)?,
                    )),
                ],
                context,
            )?;
            let predicate = graph.relate(
                crate::effect_row::prepared::PreparedEffectRelation::Equal,
                PreparedEffectRow::Canonical(EffectRow::open(EffectSet::new(), variable)),
                PreparedEffectRow::Canonical(EffectRow::open(
                    EffectSet::new(),
                    occurrence.bound_effect(0, 0)?,
                )),
                context,
            )?;
            let value: TypeKind<PreparedEffects> = TypeKind::function_with_contract(
                local,
                predicate,
                [TypeKind::Array {
                    item: Box::new(TypeKind::GenericParam(occurrence.bound_type(0, 0)?)),
                    len: ArrayLength::Generic(occurrence.bound_const(0, 0)?),
                }],
                TypeKind::I64,
                effects,
            );
            let admitted = ConstraintOperand::<NoConstraintClient>::from_prepared_original_in(
                &value,
                Arc::new(target.clone()),
                original,
                &mut ConstraintEffectPhase::new(&mut graph),
                context,
            )?;
            let TypeKind::Function {
                binder,
                predicate,
                effects,
                ..
            } = admitted.value()
            else {
                unreachable!()
            };
            assert_eq!(*binder, local);
            let nested = target.shape().with_binder(local);
            let mut expected = std::collections::BTreeSet::from([
                nested.bound_effect(0, 0)?,
                nested.bound_effect(2, 0)?,
            ]);
            expected.extend(source.rigid_scope().effects.iter().cloned());
            let mut references = std::collections::BTreeSet::new();
            graph
                .read()
                .visit_row_parameters(effects, context, &mut |reference, _| {
                    references.insert(reference.clone());
                    Ok(())
                })?;
            assert_eq!(references, expected);
            references.clear();
            graph
                .read()
                .visit_predicate_parameters(predicate, context, &mut |reference, _| {
                    references.insert(reference.clone());
                    Ok(())
                })?;
            assert_eq!(references, expected);
            // Admission preserves a declaration whose body has not been prepared.
            graph.prepare_body(
                &declaration,
                PreparedEffectRow::Canonical(EffectRow::closed(EffectSet::new())),
                vec![],
                context,
            )?;
            assert_eq!(admitted.scope(), &target);
            Ok(())
        })
        .unwrap();
}

#[test]
fn a_receiving_application_cannot_supply_the_sources_missing_import() {
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let mut graph = PreparedEffectGraph::default();
    let (original, path, variable) = parent(&mut context, &mut graph, 98_602);
    assert!(path.applications.effect_application(&variable).is_some());
    let target = Arc::new(original.gamma().clone());
    let retained = graph.retained_terms();
    let error = ConstraintOperand::<NoConstraintClient>::from_prepared_original_in(
        &input(variable.clone()),
        target,
        original,
        &mut ConstraintEffectPhase::new(&mut graph),
        &mut context,
    )
    .err();
    assert_eq!(error, Some(references::effect_out_of_scope(&variable)));
    assert_eq!(graph.retained_terms(), retained);
}

#[test]
fn a_narrow_type_lease_does_not_lend_its_owners_effect_variables() {
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let mut graph = PreparedEffectGraph::default();
    let (_, path, variable) = parent(&mut context, &mut graph, 98_603);
    let scope = path
        .applications
        .require_application(path.root_id())
        .unwrap();
    let type_key = scope.parameters().iter().next().unwrap().0;
    let ty = scope.parameters().type_reference(type_key).unwrap();
    let lease = path.applications.import_parameters(&[ty.into()]).unwrap();
    let original = context.fixture_original_context(Some(&lease)).unwrap();
    let error = ConstraintOperand::<NoConstraintClient>::from_prepared_original_in(
        &input(variable.clone()),
        Arc::new(original.gamma().clone()),
        original,
        &mut ConstraintEffectPhase::new(&mut graph),
        &mut context,
    )
    .err();
    assert_eq!(error, Some(references::effect_out_of_scope(&variable)));
}

#[test]
fn source_admission_does_not_recover_a_stale_application_from_its_live_function() {
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let mut graph = PreparedEffectGraph::default();
    let (source, path, variable) = parent(&mut context, &mut graph, 98_604);
    let owner = path
        .applications
        .require_application(path.root_id())
        .unwrap();
    let parameters: TypeConstraintParameterScope = owner.parameters().clone();
    let function = owner.effect_function(&variable).unwrap().clone();
    let mut checkpoint = graph.checkpoint(&mut context).unwrap();
    let owner = context
        .application_scope((), parameters, source.clone(), &mut graph)
        .unwrap();
    let stale = context
        .start_path_with_imported(owner, None, source, graph.read())
        .unwrap();
    let lease = stale
        .applications
        .import_parameters(&[variable.clone().into()])
        .unwrap();
    graph.rollback(&mut checkpoint).unwrap();
    graph.read().validate_unknown(&function).unwrap();
    let original = context.fixture_original_context(Some(&lease)).unwrap();
    let result = ConstraintOperand::<NoConstraintClient>::from_prepared_original_in(
        &input(variable),
        Arc::new(original.gamma().clone()),
        original,
        &mut ConstraintEffectPhase::new(&mut graph),
        &mut context,
    );
    assert!(matches!(
        result,
        Err(TypeConstraintError::Invariant(
            crate::types::constraints::TypeConstraintInvariant::PreparedEffect(
                PreparedEffectFailure::Invariant(
                    crate::effect_row::prepared::PreparedEffectInvariant::StaleReference,
                )
            ),
        ))
    ));
}

#[test]
fn a_later_source_rejection_releases_an_already_prepared_effect_replacement() {
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let mut graph = PreparedEffectGraph::default();
    context
        .with_binder(GenericBinder::new(0, 0, 1), |context| {
            let (_, path, variable) = parent(context, &mut graph, 98_605);
            let lease = path
                .applications
                .import_parameters(&[variable.clone().into()])?;
            let original = context.fixture_original_context(Some(&lease))?;
            let target = original.gamma().with_binder(GenericBinder::new(0, 0, 1));
            let foreign: GenericTypeReference = GenericTypeParameterId::new(
                GenericParameterOwnerId::Detached(DetachedGenericOwnerId::new(98_606)),
                0,
            )
            .into();
            let value: TypeKind<PreparedEffects> = TypeKind::Tuple(vec![
                input(variable),
                TypeKind::GenericParam(foreign.clone()),
            ]);
            let retained = graph.retained_terms();
            let source_scope = context.capture_lexical_scope()?;
            let error = ConstraintOperand::<NoConstraintClient>::from_prepared_original_in(
                &value,
                Arc::new(target),
                original,
                &mut ConstraintEffectPhase::new(&mut graph),
                context,
            )
            .err();
            assert_eq!(error, Some(references::type_out_of_scope(&foreign)));
            assert_eq!(graph.retained_terms(), retained);
            assert_eq!(context.lexical_frames(), &source_scope);
            Ok(())
        })
        .unwrap();
}
