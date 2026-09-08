//! Focused owner-table tests for non-Project variant families.

use arcweft_character::id::CharacterId;

use super::{CheckedVariantOwner, CheckedVariantResolution, EnvironmentBindingId, TypeKind};
use crate::types::{CharacterNominalType, VariantPayloadShape};

#[test]
fn option_and_result_own_complete_ordered_payload_rows() {
    let option = CheckedVariantOwner::try_option(TypeKind::I64).expect("Option owner");
    assert_eq!(option.cases().len(), 2);
    assert_eq!(option.cases()[0].ordinal(), 0);
    assert_eq!(option.cases()[0].diagnostic_name(), Some("Some"));
    assert!(matches!(
        option.cases()[0].payload(),
        VariantPayloadShape::Tuple(fields)
            if fields.len() == 1 && fields[0].ty() == &TypeKind::I64
    ));
    assert_eq!(option.cases()[1].ordinal(), 1);
    assert_eq!(option.cases()[1].diagnostic_name(), Some("None"));
    assert!(option.cases()[1].payload().is_unit());
    assert_ne!(
        option.cases()[0].semantic_id(),
        option.cases()[1].semantic_id()
    );

    let result =
        CheckedVariantOwner::try_result(TypeKind::I64, TypeKind::String).expect("Result owner");
    assert_eq!(result.cases().len(), 2);
    assert_eq!(result.cases()[0].diagnostic_name(), Some("Ok"));
    assert!(matches!(
        result.cases()[0].payload(),
        VariantPayloadShape::Tuple(fields)
            if fields.len() == 1 && fields[0].ty() == &TypeKind::I64
    ));
    assert_eq!(result.cases()[1].diagnostic_name(), Some("Err"));
    assert!(matches!(
        result.cases()[1].payload(),
        VariantPayloadShape::Tuple(fields)
            if fields.len() == 1 && fields[0].ty() == &TypeKind::String
    ));
}

#[test]
fn selected_ordinal_is_the_only_resolution_join() {
    let owner = CheckedVariantOwner::try_option(TypeKind::I64).expect("Option owner");
    assert!(CheckedVariantResolution::try_new(owner.clone(), 2).is_none());

    let selected = CheckedVariantResolution::try_new(owner, 1).expect("None owner row");
    assert_eq!(selected.ordinal(), 1);
    assert_eq!(selected.selected().ordinal(), 1);
    assert_eq!(selected.selected().diagnostic_name(), Some("None"));
}

#[test]
fn character_case_names_are_diagnostic_only_but_source_order_selects_ordinal() {
    let nominal = CharacterNominalType::Look {
        character: CharacterId::try_new("character.variant-owner-test").expect("Character ID"),
    };
    let first = CheckedVariantOwner::try_character_nominal(
        nominal.clone(),
        ["calm".to_owned(), "alert".to_owned()],
    )
    .expect("Character rows");
    let reordered = CheckedVariantOwner::try_character_nominal(
        nominal,
        ["alert".to_owned(), "calm".to_owned()],
    )
    .expect("reordered Character rows");

    assert_eq!(first.cases()[0].diagnostic_name(), Some("calm"));
    assert_eq!(reordered.cases()[0].diagnostic_name(), Some("alert"));
    assert_eq!(
        first.cases()[0].semantic_id(),
        reordered.cases()[0].semantic_id()
    );
}

#[test]
fn builtin_case_identity_commits_payload_presence_and_type() {
    let nominal = EnvironmentBindingId::try_new("VariantOwnerTest").expect("binding ID");
    let owner_type = TypeKind::Named("VariantOwnerTest".into());
    let unit = CheckedVariantOwner::try_builtin_closed(
        nominal.clone(),
        owner_type.clone(),
        [(None, Some("Unit".into()))],
    )
    .expect("unit row");
    let i64_payload = CheckedVariantOwner::try_builtin_closed(
        nominal.clone(),
        owner_type.clone(),
        [(Some(TypeKind::I64), Some("Payload".into()))],
    )
    .expect("payload row");
    let string_payload = CheckedVariantOwner::try_builtin_closed(
        nominal,
        owner_type,
        [(Some(TypeKind::String), Some("Payload".into()))],
    )
    .expect("other payload row");

    assert_ne!(
        unit.cases()[0].semantic_id(),
        i64_payload.cases()[0].semantic_id()
    );
    assert_ne!(
        i64_payload.cases()[0].semantic_id(),
        string_payload.cases()[0].semantic_id()
    );
}

#[test]
fn owners_reject_escaped_bound_and_active_inference_types() {
    use super::CheckedVariantOwnerError;
    use crate::types::{
        GenericBinder, GenericParameterKind, GenericParameterOwnerId, GenericScope,
        GenericScopeError, GenericTypeParameterId, LanguageIntrinsicGenericOwner,
        constraints::{TypeConstraintParameterEligibility, TypeConstraintParameterScope},
    };

    let scope = GenericScope::default().with_binder(GenericBinder::new(1, 0, 0));
    let escaped = TypeKind::GenericParam(scope.bound_type(0, 0).expect("bound type"));
    assert_eq!(
        CheckedVariantOwner::try_option(escaped),
        Err(CheckedVariantOwnerError::GenericScope(
            GenericScopeError::UnknownDepth { depth: 0 }
        )),
    );

    let parameter = GenericTypeParameterId::new(
        GenericParameterOwnerId::LanguageIntrinsic(
            LanguageIntrinsicGenericOwner::OptionConstructor,
        ),
        0,
    );
    let application = TypeConstraintParameterScope::new([(
        parameter.clone(),
        TypeConstraintParameterEligibility::Bindable,
    )])
    .expect("application scope");
    let active = TypeKind::GenericParam(
        application
            .type_reference(&(parameter).clone().into())
            .expect("active type"),
    );
    assert_eq!(
        CheckedVariantOwner::try_result(TypeKind::I64, active),
        Err(CheckedVariantOwnerError::GenericScope(
            GenericScopeError::EscapedInference {
                kind: GenericParameterKind::Type,
            }
        )),
    );
}

#[test]
fn payload_function_binders_keep_their_local_type_and_const_references() {
    use crate::{
        effect_row::EffectRow,
        effects::EffectSet,
        types::{ArrayLength, GenericBinder, GenericScope},
    };
    let binder = GenericBinder::new(1, 1, 0);
    let scope = GenericScope::default().with_binder(binder);
    let parameter = TypeKind::GenericParam(scope.bound_type(0, 0).expect("bound type"));
    let function = TypeKind::function_with_binder(
        binder,
        [parameter.clone()],
        TypeKind::Array {
            item: Box::new(parameter),
            len: ArrayLength::Generic(scope.bound_const(0, 0).expect("bound length")),
        },
        EffectRow::closed(EffectSet::new()),
    );
    let owner =
        CheckedVariantOwner::try_option(function.clone()).expect("locally bound function payload");
    assert_eq!(
        owner.cases()[0].payload().single_tuple_field(),
        Some(&function)
    );
    assert_eq!(
        owner.semantic_type(),
        TypeKind::Option(Box::new(function))
            .semantic_identity_digest()
            .expect("stable owner type"),
    );
    assert!(owner.case_payload_type(0).flatten().is_some());
}
