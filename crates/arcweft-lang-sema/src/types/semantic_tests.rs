//! Focused semantic-tag tests for standard checked fields.

use super::{
    CharacterField, GenericParameterOwnerId, GenericTypeParameterId, LanguageIntrinsicGenericOwner,
    ProgressField, TypeKind,
};
use arcweft_core::{
    pattern::RuntimeCheckedType,
    value::{RuntimeSignedIntWidth, RuntimeUnsignedIntWidth},
};
use std::collections::BTreeSet;

#[test]
fn standard_field_semantic_tags_are_unique() {
    let progress = ProgressField::ALL
        .iter()
        .copied()
        .map(ProgressField::semantic_tag)
        .collect::<BTreeSet<_>>();
    let character = CharacterField::ALL
        .iter()
        .copied()
        .map(CharacterField::semantic_tag)
        .collect::<BTreeSet<_>>();

    assert_eq!(progress.len(), ProgressField::ALL.len());
    assert_eq!(character.len(), CharacterField::ALL.len());
}

#[test]
fn language_intrinsic_generic_owner_tags_and_type_digests_are_unique() {
    let tags = LanguageIntrinsicGenericOwner::ALL
        .iter()
        .copied()
        .map(LanguageIntrinsicGenericOwner::semantic_tag)
        .collect::<BTreeSet<_>>();
    let digests = LanguageIntrinsicGenericOwner::ALL
        .iter()
        .copied()
        .map(|owner| {
            TypeKind::GenericParam(GenericTypeParameterId::new(
                GenericParameterOwnerId::LanguageIntrinsic(owner),
                0,
            ))
            .semantic_identity_digest()
        })
        .collect::<BTreeSet<_>>();

    assert_eq!(tags.len(), LanguageIntrinsicGenericOwner::ALL.len());
    assert_eq!(digests.len(), LanguageIntrinsicGenericOwner::ALL.len());
}

#[test]
fn runtime_primitive_digests_use_the_core_checked_type_authority() {
    let pairs = [
        (TypeKind::Never, RuntimeCheckedType::Never),
        (TypeKind::Unit, RuntimeCheckedType::Unit),
        (TypeKind::Bool, RuntimeCheckedType::Bool),
        (
            TypeKind::I8,
            RuntimeCheckedType::Signed(RuntimeSignedIntWidth::I8),
        ),
        (
            TypeKind::I16,
            RuntimeCheckedType::Signed(RuntimeSignedIntWidth::I16),
        ),
        (
            TypeKind::I32,
            RuntimeCheckedType::Signed(RuntimeSignedIntWidth::I32),
        ),
        (
            TypeKind::I64,
            RuntimeCheckedType::Signed(RuntimeSignedIntWidth::I64),
        ),
        (
            TypeKind::I128,
            RuntimeCheckedType::Signed(RuntimeSignedIntWidth::I128),
        ),
        (
            TypeKind::ISize,
            RuntimeCheckedType::Signed(RuntimeSignedIntWidth::ISize),
        ),
        (
            TypeKind::U8,
            RuntimeCheckedType::Unsigned(RuntimeUnsignedIntWidth::U8),
        ),
        (
            TypeKind::U16,
            RuntimeCheckedType::Unsigned(RuntimeUnsignedIntWidth::U16),
        ),
        (
            TypeKind::U32,
            RuntimeCheckedType::Unsigned(RuntimeUnsignedIntWidth::U32),
        ),
        (
            TypeKind::U64,
            RuntimeCheckedType::Unsigned(RuntimeUnsignedIntWidth::U64),
        ),
        (
            TypeKind::U128,
            RuntimeCheckedType::Unsigned(RuntimeUnsignedIntWidth::U128),
        ),
        (
            TypeKind::USize,
            RuntimeCheckedType::Unsigned(RuntimeUnsignedIntWidth::USize),
        ),
        (TypeKind::F32, RuntimeCheckedType::F32),
        (TypeKind::F64, RuntimeCheckedType::F64),
        (TypeKind::String, RuntimeCheckedType::String),
        (TypeKind::Char, RuntimeCheckedType::Char),
        (TypeKind::Bytes, RuntimeCheckedType::Bytes),
        (TypeKind::Duration, RuntimeCheckedType::Duration),
        (TypeKind::Progress, RuntimeCheckedType::Progress),
    ];

    let mut digests = BTreeSet::new();
    for (source, checked) in &pairs {
        let source = source.semantic_identity_digest();
        assert_eq!(
            source.as_bytes(),
            checked.semantic_identity_digest().as_bytes()
        );
        assert!(
            digests.insert(source),
            "duplicate primitive identity for {checked:?}"
        );
    }
}
