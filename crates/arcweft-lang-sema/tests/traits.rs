use arcweft_lang_hir::lower::lower_to_hir;
use arcweft_lang_sema::check::analyze_types;
use arcweft_lang_sema::diagnostics::{TraitDiagnosticKind, TypeCheckErrorKind};
use arcweft_lang_sema::env::TypeCheckEnv;
use arcweft_lang_syntax::parser::parse_source;

fn diagnostics(source: &str) -> Vec<TypeCheckErrorKind> {
    let tree = parse_source(source).into_typed_tree();
    let hir = lower_to_hir(&tree).expect("HIR lowers");
    analyze_types(&hir, &TypeCheckEnv::standard())
        .diagnostics
        .into_iter()
        .map(|diagnostic| diagnostic.kind().clone())
        .collect()
}

#[test]
fn complete_trait_impl_is_accepted() {
    let diags = diagnostics(include_str!(
        "../../../fixtures/traits/ok-basic-trait-impl.arcw"
    ));
    assert_eq!(diags, []);
}

#[test]
fn missing_associated_type_is_structured() {
    let diags = diagnostics(include_str!(
        "../../../fixtures/traits/err-missing-associated-type.arcw"
    ));
    assert!(diags.iter().any(
        |kind| matches!(kind, TypeCheckErrorKind::Trait { diagnostic }
        if matches!(diagnostic.kind(), TraitDiagnosticKind::MissingAssociatedType { .. }))
    ));
}

#[test]
fn duplicate_impl_is_structured() {
    let diags = diagnostics(include_str!(
        "../../../fixtures/traits/err-duplicate-impl.arcw"
    ));
    assert!(diags.iter().any(
        |kind| matches!(kind, TypeCheckErrorKind::Trait { diagnostic }
        if matches!(diagnostic.kind(), TraitDiagnosticKind::DuplicateImpl { .. }))
    ));
}

#[test]
fn gat_like_assoc_constructor_is_rejected_for_seq08_1() {
    let diags = diagnostics(include_str!(
        "../../../fixtures/traits/err-gat-deferred.arcw"
    ));
    assert!(diags.iter().any(|kind| matches!(kind, TypeCheckErrorKind::Trait { diagnostic }
        if matches!(diagnostic.kind(), TraitDiagnosticKind::AssociatedTypeConstructorUnsupported { .. }))));
}
