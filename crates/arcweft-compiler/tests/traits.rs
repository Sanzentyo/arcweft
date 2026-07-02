use arcweft_compiler::{
    error::ValidateHirError,
    hir::{lower_source_tree, validate_hir_with_env},
};
use arcweft_lang_sema::diagnostics::TypeCheckError;
use arcweft_lang_sema::env::TypeCheckEnv;
use arcweft_lang_syntax::parser::parse_source;

#[test]
fn compiler_hir_validation_surfaces_trait_diagnostic_code() {
    let tree = parse_source(include_str!(
        "../../../fixtures/traits/err-missing-associated-type.arcw"
    ))
    .into_typed_tree();
    let hir = lower_source_tree(&tree).expect("HIR lowers");
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
