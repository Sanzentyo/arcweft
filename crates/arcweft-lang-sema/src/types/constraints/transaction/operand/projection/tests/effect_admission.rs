use super::*;
use crate::{
    effect_row::{EffectConstraintEligibility, EffectConstraintVariable, EffectFormula, EffectRow},
    effects::EffectSet,
    types::{
        DetachedGenericOwnerId, GenericEffectParameterId, GenericEffectReference,
        GenericParameterOwnerId,
        constraints::{
            OriginalRigidScope, TypeConstraintEffectScope, TypeConstraintParameterScope,
            normalization::{
                ConstraintClosedEffects, ConstraintClosurePolicy, close_type_operand,
                project_closed_type, seal_type_operand,
            },
            references,
        },
    },
};

fn fixture(
    cancellation: &AtomicBool,
) -> (
    Context<'_>,
    ConstraintPath<NoConstraintClient>,
    [GenericEffectReference; 2],
    [GenericEffectReference; 2],
) {
    let owner = GenericParameterOwnerId::Detached(DetachedGenericOwnerId::new(90_804));
    let rigid = [0, 1].map(|slot| GenericEffectParameterId::new(owner.clone(), slot).into());
    let declarations: [GenericEffectReference; 2] =
        [2, 3].map(|slot| GenericEffectParameterId::new(owner.clone(), slot).into());
    let parameters = TypeConstraintParameterScope::seal_call_scope(
        GenericBinder::EMPTY,
        [],
        [],
        TypeConstraintEffectScope::seal_call_scope(
            rigid
                .iter()
                .cloned()
                .map(|r| EffectConstraintVariable::new(r, EffectConstraintEligibility::Rigid))
                .chain(declarations.iter().cloned().map(|r| {
                    EffectConstraintVariable::new(r, EffectConstraintEligibility::Bindable)
                })),
            [],
        )
        .unwrap(),
        [],
        [],
    )
    .unwrap();
    let keys = declarations.map(|r| parameters.effect_reference(&r).unwrap());
    let (context, path) = ConstraintTestSetup::with_scope(
        TypeConstraintLimits::new(65_536, 32_768, 1024, 1024),
        cancellation,
        parameters,
    )
    .into_path();
    (context, path, rigid, keys)
}

fn row(reference: &GenericEffectReference) -> EffectRow {
    EffectRow::open(EffectSet::new(), reference.clone())
}

#[test]
fn prepared_headers_admit_their_own_gamma_before_body_preparation_or_erasure() {
    use crate::effect_row::prepared::{
        PreparedEffectGraph, PreparedEffectPredicate, PreparedEffectRelation, PreparedEffectRow,
    };
    use crate::types::constraints::transaction::operand::ConstraintEffectPhase;
    use crate::types::{GenericScope, PreparedEffects};

    let cancellation = AtomicBool::new(false);
    let (mut context, _, rigid, _) = fixture(&cancellation);
    let original = context
        .fixture_original_context_with_rigid(OriginalRigidScope {
            types: &[],
            consts: &[],
            effects: &rigid[..1],
            imported: None,
        })
        .unwrap();
    let mut graph = PreparedEffectGraph::default();
    let formal = GenericScope::default()
        .with_binder(GenericBinder::new(0, 0, 1))
        .bound_effect(0, 0)
        .unwrap();
    let declaration = graph
        .declare(
            vec![formal],
            crate::effect_row::EffectPredicate::unconstrained(),
            &mut context,
        )
        .unwrap();
    let allowed = graph
        .application(
            &declaration,
            vec![PreparedEffectRow::Canonical(row(&rigid[0]))],
            &mut context,
        )
        .unwrap();
    let value = TypeKind::<PreparedEffects>::Function {
        binder: GenericBinder::EMPTY,
        predicate: PreparedEffectPredicate::Canonical(
            crate::effect_row::EffectPredicate::unconstrained(),
        ),
        params: vec![],
        return_type: Box::new(TypeKind::Unit),
        effects: allowed,
    };
    let mut input = ConstraintOperand::from_prepared_original_in(
        &value,
        Arc::new(original.gamma().clone()),
        original,
        &mut ConstraintEffectPhase::new(&mut graph),
        &mut context,
    )
    .unwrap();
    input
        .view()
        .children(&mut context)
        .unwrap()
        .function_header()
        .unwrap()
        .unwrap()
        .admit(graph.read(), &mut context)
        .unwrap();

    let unauthorized = graph
        .application(
            &declaration,
            vec![PreparedEffectRow::Canonical(row(&rigid[1]))],
            &mut context,
        )
        .unwrap();
    let predicate = graph
        .relate(
            PreparedEffectRelation::Equal,
            unauthorized.clone(),
            unauthorized.clone(),
            &mut context,
        )
        .unwrap();
    let TypeKind::Function { effects, .. } = Arc::make_mut(&mut input.value) else {
        unreachable!()
    };
    let allowed = std::mem::replace(effects, unauthorized);
    let result = input
        .view()
        .children(&mut context)
        .unwrap()
        .function_header()
        .unwrap()
        .unwrap()
        .admit(graph.read(), &mut context);
    assert_eq!(result, Err(references::effect_out_of_scope(&rigid[1])));

    let TypeKind::Function {
        effects,
        predicate: input_predicate,
        ..
    } = Arc::make_mut(&mut input.value)
    else {
        unreachable!()
    };
    *effects = allowed;
    *input_predicate = predicate;
    // Even equality of the very same term cannot erase an unauthorized
    // reference before its predicate field's original Γ is checked.
    let result = input
        .view()
        .children(&mut context)
        .unwrap()
        .function_header()
        .unwrap()
        .unwrap()
        .admit(graph.read(), &mut context);
    assert_eq!(result, Err(references::effect_out_of_scope(&rigid[1])));
    // No body was supplied anywhere in this fixture. Admission must not need it.
    let declaration = graph.declaration(&declaration, &mut context).unwrap();
    declaration.validate(&graph).unwrap();
}

#[test]
fn computed_effect_primitives_retain_only_their_own_used_gamma() {
    let mut effect_graph = crate::effect_row::prepared::PreparedEffectGraph::default();
    let cancellation = AtomicBool::new(false);
    let (mut context, mut path, rigid, keys) = fixture(&cancellation);
    for index in 0..2 {
        path.effects
            .constrain_fixture_subset(
                &mut effect_graph,
                &row(&rigid[index]),
                &row(&keys[index]),
                &mut context,
            )
            .unwrap();
    }
    let effects = path
        .close_original_path_effects(&mut effect_graph, &mut context)
        .unwrap();
    for index in 0..2 {
        let genuine = effects.row(&keys[index]).unwrap();
        assert_eq!(genuine.value(), &row(&rigid[index]));
        genuine
            .admit_effect_reference(&rigid[index], &mut context)
            .unwrap();
        let mut forged = genuine.clone();
        forged.value = Arc::new(row(&rigid[1 - index]));
        assert_eq!(
            forged.admit_effect_reference(&rigid[1 - index], &mut context),
            Err(references::effect_out_of_scope(&rigid[1 - index]))
        );
    }
    let mut forged = effects.predicate().clone();
    forged.value = Arc::new(
        EffectFormula::variable(rigid[0].clone(), &mut context)
            .unwrap()
            .subset(
                &EffectFormula::literal(EffectSet::new(), None),
                &mut context,
            )
            .unwrap(),
    );
    assert_eq!(
        forged.admit_effect_reference(&rigid[0], &mut context),
        Err(references::effect_out_of_scope(&rigid[0]))
    );
}

#[test]
fn original_effect_header_admission_precedes_substitution_and_canonical_erasure() {
    let mut effect_graph = crate::effect_row::prepared::PreparedEffectGraph::default();
    let cancellation = AtomicBool::new(false);
    let (mut context, path, rigid, keys) = fixture(&cancellation);
    let original = context
        .fixture_original_context_with_rigid(OriginalRigidScope {
            types: &[],
            consts: &[],
            effects: &rigid[..1],
            imported: None,
        })
        .unwrap();
    let mut input = context
        .fixture_operand_in(
            &original,
            &TypeKind::function_with_effects([], TypeKind::Unit, row(&rigid[0])),
        )
        .unwrap();
    input.value = Arc::new(TypeKind::function_with_effects(
        [],
        TypeKind::Unit,
        row(&rigid[1]),
    ));
    let effects = path
        .close_original_path_effects(&mut effect_graph, &mut context)
        .unwrap();
    assert!(
        matches!(close_type_operand(input.view(), &path, &effects, effect_graph.read(), &mut context), Err(error) if error == references::effect_out_of_scope(&rigid[1]))
    );
    assert!(
        matches!(seal_type_operand(input.view(), path.projection_view(), &mut Default::default(), &mut effect_graph, &mut context), Err(error) if error == references::effect_out_of_scope(&rigid[1]))
    );
    // Substituting the unconstrained bindable row with empty makes this
    // predicate true, erasing the unauthorized occurrence from its result.
    let predicate = EffectFormula::variable(keys[0].clone(), &mut context)
        .unwrap()
        .subset(
            &EffectFormula::variable(rigid[1].clone(), &mut context).unwrap(),
            &mut context,
        )
        .unwrap();
    input.value = Arc::new(TypeKind::function_with_contract(
        GenericBinder::EMPTY,
        predicate,
        [],
        TypeKind::Unit,
        row(&rigid[0]),
    ));
    let inspected = project_closed_type(
        input.value(),
        &path,
        ConstraintClosurePolicy::ProjectionClosed,
        &effects,
        effect_graph.read(),
        &mut context,
    )
    .unwrap();
    let TypeKind::Function { predicate, .. } = inspected else {
        unreachable!()
    };
    assert!(predicate.is_unconstrained());
    assert!(
        matches!(close_type_operand(input.view(), &path, &effects, effect_graph.read(), &mut context), Err(error) if error == references::effect_out_of_scope(&rigid[1]))
    );
}

#[test]
fn completed_header_gamma_is_field_specific_and_survives_reprojection() {
    let mut effect_graph = crate::effect_row::prepared::PreparedEffectGraph::default();
    let cancellation = AtomicBool::new(false);
    let (mut context, mut path, rigid, keys) = fixture(&cancellation);
    for index in 0..2 {
        path.effects
            .constrain_fixture_subset(
                &mut effect_graph,
                &row(&rigid[index]),
                &row(&keys[index]),
                &mut context,
            )
            .unwrap();
    }
    let input = ConstraintOperand::fixture_template(
        TypeKind::function_with_contract(
            GenericBinder::EMPTY,
            EffectFormula::variable(keys[1].clone(), &mut context)
                .unwrap()
                .subset(
                    &EffectFormula::literal(EffectSet::new(), None),
                    &mut context,
                )
                .unwrap(),
            [],
            TypeKind::Unit,
            row(&keys[0]),
        ),
        Arc::new(context.capture_lexical_scope().unwrap()),
        path.applications
            .require_application_owner(path.root_id())
            .unwrap(),
    );
    let effects = path
        .close_original_path_effects(&mut effect_graph, &mut context)
        .unwrap();
    let closed = close_type_operand(
        input.view(),
        &path,
        &effects,
        effect_graph.read(),
        &mut context,
    )
    .unwrap();
    assert!(
        close_type_operand(
            closed.view(),
            &path,
            &effects,
            effect_graph.read(),
            &mut context
        )
        .is_ok()
    );
    let mut wrong_row = closed.clone();
    let TypeKind::Function {
        effects: actual_row,
        ..
    } = Arc::make_mut(&mut wrong_row.value)
    else {
        unreachable!()
    };
    *actual_row = crate::effect_row::prepared::PreparedEffectRow::Canonical(row(&rigid[1]));
    assert!(
        matches!(close_type_operand(wrong_row.view(), &path, &effects, effect_graph.read(), &mut context), Err(error) if error == references::effect_out_of_scope(&rigid[1]))
    );
    let mut wrong_predicate = closed.clone();
    let TypeKind::Function { predicate, .. } = Arc::make_mut(&mut wrong_predicate.value) else {
        unreachable!()
    };
    *predicate = crate::effect_row::prepared::PreparedEffectPredicate::Canonical(
        EffectFormula::variable(rigid[0].clone(), &mut context)
            .unwrap()
            .subset(
                &EffectFormula::literal(EffectSet::new(), None),
                &mut context,
            )
            .unwrap(),
    );
    assert!(
        matches!(close_type_operand(wrong_predicate.view(), &path, &effects, effect_graph.read(), &mut context), Err(error) if error == references::effect_out_of_scope(&rigid[0]))
    );
}
