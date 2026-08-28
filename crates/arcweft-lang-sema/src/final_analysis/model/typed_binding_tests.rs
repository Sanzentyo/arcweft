use super::super::CheckedPatternResolution;
use super::CheckedTypedBinding;
use crate::types::TypeKind;

#[test]
fn typed_binding_retains_exact_annotation_and_semantic_digest() {
    let i64_binding =
        CheckedTypedBinding::try_new(TypeKind::I64, &TypeKind::I64).expect("checked ordinal");
    let u64_binding =
        CheckedTypedBinding::try_new(TypeKind::U64, &TypeKind::U64).expect("checked ordinal");

    assert_eq!(i64_binding.annotation(), &TypeKind::I64);
    assert_eq!(
        i64_binding.annotation_digest(),
        TypeKind::I64.semantic_identity_digest()
    );
    assert!(i64_binding.has_valid_semantic_identity());
    assert_ne!(
        i64_binding.annotation_digest(),
        u64_binding.annotation_digest()
    );
    assert_eq!(
        CheckedPatternResolution::TypedBinding(i64_binding).semantic_tag(),
        0x0605
    );
}
