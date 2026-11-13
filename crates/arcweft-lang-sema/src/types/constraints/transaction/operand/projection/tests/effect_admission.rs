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
fn computed_effect_primitives_retain_only_their_own_used_gamma() {
    let cancellation = AtomicBool::new(false);
    let (mut context, mut path, rigid, keys) = fixture(&cancellation);
    for index in 0..2 {
        path.effects
            .constrain_subset(&row(&rigid[index]), &row(&keys[index]), &mut context)
            .unwrap();
    }
    let effects = ConstraintClosedEffects::for_path(&path, &mut context).unwrap();
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
    let effects = ConstraintClosedEffects::for_path(&path, &mut context).unwrap();
    assert!(
        matches!(close_type_operand(input.view(), &path, &effects, &mut context), Err(error) if error == references::effect_out_of_scope(&rigid[1]))
    );
    assert!(
        matches!(seal_type_operand(input.view(), path.projection_view(), &mut Default::default(), &mut context), Err(error) if error == references::effect_out_of_scope(&rigid[1]))
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
        &mut context,
    )
    .unwrap();
    let TypeKind::Function { predicate, .. } = inspected else {
        unreachable!()
    };
    assert!(predicate.is_unconstrained());
    assert!(
        matches!(close_type_operand(input.view(), &path, &effects, &mut context), Err(error) if error == references::effect_out_of_scope(&rigid[1]))
    );
}

#[test]
fn completed_header_gamma_is_field_specific_and_survives_reprojection() {
    let cancellation = AtomicBool::new(false);
    let (mut context, mut path, rigid, keys) = fixture(&cancellation);
    for index in 0..2 {
        path.effects
            .constrain_subset(&row(&rigid[index]), &row(&keys[index]), &mut context)
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
    let effects = ConstraintClosedEffects::for_path(&path, &mut context).unwrap();
    let closed = close_type_operand(input.view(), &path, &effects, &mut context).unwrap();
    assert!(close_type_operand(closed.view(), &path, &effects, &mut context).is_ok());
    let mut wrong_row = closed.clone();
    let TypeKind::Function {
        effects: actual_row,
        ..
    } = Arc::make_mut(&mut wrong_row.value)
    else {
        unreachable!()
    };
    *actual_row = row(&rigid[1]);
    assert!(
        matches!(close_type_operand(wrong_row.view(), &path, &effects, &mut context), Err(error) if error == references::effect_out_of_scope(&rigid[1]))
    );
    let mut wrong_predicate = closed.clone();
    let TypeKind::Function { predicate, .. } = Arc::make_mut(&mut wrong_predicate.value) else {
        unreachable!()
    };
    *predicate = EffectFormula::variable(rigid[0].clone(), &mut context)
        .unwrap()
        .subset(
            &EffectFormula::literal(EffectSet::new(), None),
            &mut context,
        )
        .unwrap();
    assert!(
        matches!(close_type_operand(wrong_predicate.view(), &path, &effects, &mut context), Err(error) if error == references::effect_out_of_scope(&rigid[0]))
    );
}
