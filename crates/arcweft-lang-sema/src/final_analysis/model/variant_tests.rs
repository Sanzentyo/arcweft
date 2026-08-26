//! Focused owner-table tests for non-Project variant families.

use arcweft_character::id::CharacterId;

use super::{
    CharacterNominalType, CheckedVariantOwner, CheckedVariantResolution, EnvironmentBindingId,
    TypeKind,
};

#[test]
fn option_and_result_own_complete_ordered_payload_rows() {
    let option = CheckedVariantOwner::option(TypeKind::I64);
    assert_eq!(option.cases().len(), 2);
    assert_eq!(option.cases()[0].ordinal(), 0);
    assert_eq!(option.cases()[0].diagnostic_name(), Some("Some"));
    assert_eq!(option.cases()[0].payload(), Some(&TypeKind::I64));
    assert_eq!(option.cases()[1].ordinal(), 1);
    assert_eq!(option.cases()[1].diagnostic_name(), Some("None"));
    assert_eq!(option.cases()[1].payload(), None);
    assert_ne!(
        option.cases()[0].semantic_id(),
        option.cases()[1].semantic_id()
    );
    assert!(option.has_valid_case_rows());

    let result = CheckedVariantOwner::result(TypeKind::I64, TypeKind::String);
    assert_eq!(result.cases().len(), 2);
    assert_eq!(result.cases()[0].diagnostic_name(), Some("Ok"));
    assert_eq!(result.cases()[0].payload(), Some(&TypeKind::I64));
    assert_eq!(result.cases()[1].diagnostic_name(), Some("Err"));
    assert_eq!(result.cases()[1].payload(), Some(&TypeKind::String));
    assert!(result.has_valid_case_rows());
}

#[test]
fn selected_ordinal_is_the_only_resolution_join() {
    let owner = CheckedVariantOwner::option(TypeKind::I64);
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
    assert!(first.has_valid_case_rows());
    assert!(reordered.has_valid_case_rows());
}

#[test]
fn builtin_case_identity_commits_payload_presence_and_type() {
    let nominal = EnvironmentBindingId::try_new("VariantOwnerTest").expect("binding ID");
    let owner_type = TypeKind::Named("VariantOwnerTest".into()).semantic_identity_digest();
    let unit = CheckedVariantOwner::try_builtin_closed(
        nominal.clone(),
        owner_type,
        [(None, Some("Unit".into()))],
    )
    .expect("unit row");
    let i64_payload = CheckedVariantOwner::try_builtin_closed(
        nominal.clone(),
        owner_type,
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
    assert!(unit.has_valid_case_rows());
    assert!(i64_payload.has_valid_case_rows());
    assert!(string_payload.has_valid_case_rows());
}
