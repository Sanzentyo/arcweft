use arcweft_compiler::{
    error::ValidateHirError,
    hir::{lower_source_document, validate_hir_with_env},
};
use arcweft_lang_sema::diagnostics::TypeCheckError;
use arcweft_lang_sema::env::TypeCheckEnv;
use arcweft_lang_syntax::parser::parse_source;

#[test]
fn compiler_hir_validation_surfaces_trait_diagnostic_code() {
    let parsed = parse_source(include_str!(
        "../../../fixtures/traits/err-missing-associated-type.arcw"
    ));
    let hir = lower_source_document(parsed.document(), parsed.typed_tree()).expect("HIR lowers");
    assert_eq!(hir.source_identity(), Some(parsed.document().identity()));
    let err = validate_hir_with_env(&hir, &TypeCheckEnv::standard()).expect_err("trait error");
    let ValidateHirError::Type(diagnostics) = err else {
        panic!("expected type diagnostics");
    };
    let codes = diagnostics
        .iter()
        .map(TypeCheckError::stable_code)
        .collect::<Vec<_>>();
    assert!(
        codes
            .iter()
            .any(|code| code == "sema.trait.missing_associated_type")
    );
}
