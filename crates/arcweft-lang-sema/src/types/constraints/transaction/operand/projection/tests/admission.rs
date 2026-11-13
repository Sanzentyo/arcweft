use super::*;
use crate::types::{
    DetachedGenericOwnerId, GenericConstParameterId, GenericParameterOwnerId,
    GenericTypeParameterId,
    constraints::{
        OriginalRigidScope, TypeConstraintConstEligibility, TypeConstraintParameterEligibility,
        TypeConstraintParameterScope,
        normalization::{ConstraintClosedEffects, close_type_operand},
        references,
    },
};

fn parameters(
    types: &[GenericTypeReference; 2],
    consts: &[GenericConstReference; 2],
    inherited: bool,
) -> TypeConstraintParameterScope {
    TypeConstraintParameterScope::seal_call_scope(
        GenericBinder::EMPTY,
        types
            .iter()
            .cloned()
            .map(|key| {
                crate::types::constraints::context::TypeConstraintTypeParameterScopeRow::new(
                    key,
                    TypeConstraintParameterEligibility::Rigid,
                )
            })
            .chain(types.iter().cloned().map(|key| {
                crate::types::constraints::context::TypeConstraintTypeParameterScopeRow::new(
                    key,
                    TypeConstraintParameterEligibility::Bindable,
                )
            })),
        consts.iter().cloned().map(|key| {
            crate::types::constraints::context::TypeConstraintConstParameterScopeRow::new(
                key,
                TypeConstraintConstEligibility::Rigid,
            )
        }),
        crate::types::constraints::TypeConstraintEffectScope::seal_call_scope([], []).unwrap(),
        if inherited { types.to_vec() } else { vec![] },
        [],
    )
    .unwrap()
}

fn fixture(
    cancellation: &AtomicBool,
) -> (
    Context<'_>,
    ConstraintPath<NoConstraintClient>,
    [ConstraintOperand<NoConstraintClient>; 2],
    [ConstraintOperand<NoConstraintClient, ArrayLength>; 2],
) {
    let owner = GenericParameterOwnerId::Detached(DetachedGenericOwnerId::new(90_704));
    let types = [0, 1].map(|slot| GenericTypeParameterId::new(owner.clone(), slot).into());
    let consts = [0, 1].map(|slot| GenericConstParameterId::new(owner.clone(), slot).into());
    let (mut context, path) = ConstraintTestSetup::with_scope(
        TypeConstraintLimits::new(8192, 4096, 128, 128),
        cancellation,
        parameters(&types, &consts, false),
    )
    .into_path();
    let originals = [0, 1].map(|index| {
        context
            .fixture_original_context_with_rigid(OriginalRigidScope {
                types: &types[index..=index],
                consts: &consts[index..=index],
                effects: &[],
                imported: None,
            })
            .unwrap()
    });
    let values = [0, 1].map(|index| {
        context
            .fixture_operand_in(
                &originals[index],
                &TypeKind::GenericParam(types[index].clone()),
            )
            .unwrap()
    });
    let lengths = [0, 1].map(|index| {
        context
            .fixture_operand_in(
                &originals[index],
                &ArrayLength::Generic(consts[index].clone()),
            )
            .unwrap()
    });
    (context, path, values, lengths)
}

#[test]
fn completed_binding_admission_cannot_use_a_siblings_retained_capture() {
    let cancellation = AtomicBool::new(false);
    let (mut context, mut path, values, _) = fixture(&cancellation);
    let owner = GenericParameterOwnerId::Detached(DetachedGenericOwnerId::new(90_704));
    let declarations: [GenericTypeReference; 2] =
        [0, 1].map(|slot| GenericTypeParameterId::new(owner.clone(), slot).into());
    let consts = [0, 1].map(|slot| GenericConstParameterId::new(owner.clone(), slot).into());
    for (declaration, value) in declarations.iter().zip(&values) {
        let key = path
            .root_scope()
            .parameters()
            .type_reference(declaration)
            .unwrap();
        path.bindings.insert(key, value.clone());
    }
    let solution = Arc::new(
        TypeConstraintSolution::complete_application(&path, path.root_id(), &mut context).unwrap(),
    );
    let application = context
        .fixture_application_scope((), parameters(&declarations, &consts, true))
        .unwrap();
    let next = context.start_path(application).unwrap();
    let next = solution
        .restore_inherited_path(next.root_id(), next, &mut context)
        .unwrap();
    let keys = declarations
        .each_ref()
        .map(|key| next.root_scope().parameters().type_reference(key).unwrap());
    let effects = ConstraintClosedEffects::for_path(&next, &mut context).unwrap();
    for index in 0..2 {
        let genuine = &next.bindings[&keys[index]];
        assert!(close_type_operand(genuine.view(), &next, &effects, &mut context).is_ok());
        let mut forged = genuine.clone();
        forged.value = Arc::clone(&next.bindings[&keys[1 - index]].value);
        let TypeKind::GenericParam(reference) = forged.value() else {
            unreachable!()
        };
        assert_eq!(
            context.parameter_eligibility(reference, next.projection_view()),
            Some(TypeConstraintParameterEligibility::Rigid)
        );
        assert_eq!(
            close_type_operand(forged.view(), &next, &effects, &mut context).err(),
            Some(references::type_out_of_scope(reference))
        );
    }
}

#[test]
fn visible_free_keys_cannot_borrow_a_different_originals_permission() {
    let cancellation = AtomicBool::new(false);
    let (mut context, path, values, lengths) = fixture(&cancellation);
    let effects = ConstraintClosedEffects::for_path(&path, &mut context).unwrap();
    for index in 0..2 {
        let mut forged = values[index].clone();
        forged.origin = Arc::clone(&values[1 - index].origin);
        let TypeKind::GenericParam(reference) = forged.value() else {
            unreachable!()
        };
        assert_eq!(
            context.parameter_eligibility(reference, path.projection_view()),
            Some(TypeConstraintParameterEligibility::Rigid)
        );
        assert_eq!(
            close_type_operand(forged.view(), &path, &effects, &mut context).err(),
            Some(references::type_out_of_scope(reference))
        );

        let array = context.fixture_operand(TypeKind::Array {
            item: Box::new(TypeKind::Unit),
            len: lengths[index].value().clone(),
        });
        let mut forged = array;
        forged.origin = Arc::clone(&values[1 - index].origin);
        let ArrayLength::Generic(reference) = lengths[index].value() else {
            unreachable!()
        };
        assert_eq!(
            context.const_parameter_eligibility(reference, path.projection_view()),
            Some(TypeConstraintConstEligibility::Rigid)
        );
        assert_eq!(
            close_type_operand(forged.view(), &path, &effects, &mut context).err(),
            Some(references::const_out_of_scope(reference))
        );
    }
}

#[test]
fn mixed_source_ports_keep_independent_type_and_length_admission() {
    let cancellation = AtomicBool::new(false);
    let (mut context, path, values, lengths) = fixture(&cancellation);
    let root = context.fixture_operand(TypeKind::Array {
        item: Box::new(TypeKind::Unit),
        len: ArrayLength::Const(0),
    });
    let seed = root.view().children(&mut context).unwrap().rebuild_seed();
    let value = seed
        .rebuild(
            vec![values[0].view().projected_leaf()],
            Some(lengths[1].view().projected_leaf()),
            &ConstraintOperandEffects::Original,
            &mut context,
        )
        .unwrap()
        .into_operand();
    let effects = ConstraintClosedEffects::for_path(&path, &mut context).unwrap();
    let closed = close_type_operand(value.view(), &path, &effects, &mut context).unwrap();
    assert_eq!(
        closed.value(),
        &TypeKind::Array {
            item: Box::new(values[0].value().clone()),
            len: lengths[1].value().clone()
        }
    );
    let children = closed.view();
    assert!(
        children
            .children(&mut context)
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .admit_type_reference(
                match values[0].value() {
                    TypeKind::GenericParam(key) => key,
                    _ => unreachable!(),
                },
                &mut context
            )
            .is_ok()
    );
}
