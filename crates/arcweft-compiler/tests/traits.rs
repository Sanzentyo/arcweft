use arcweft_compiler::{
    error::ValidateHirError,
    hir::{lower_source_document, validate_hir_with_env},
};
use arcweft_lang_sema::diagnostics::TypeCheckError;
use arcweft_lang_sema::env::TypeCheckEnv;
use arcweft_lang_syntax::parser::{ParseOptions, parse_document_with_source};
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};
use std::sync::Arc;

#[test]
fn compiler_hir_validation_surfaces_trait_diagnostic_code() {
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new(
                "arcweft-test://compiler/traits/err-missing-associated-type.arcw",
            )
            .expect("trait diagnostic fixture source ID"),
            SourceName::path("fixtures/traits/err-missing-associated-type.arcw"),
            include_str!("../../../fixtures/traits/err-missing-associated-type.arcw"),
        )
        .expect("trait diagnostic fixture source document"),
    );
    let parsed = parse_document_with_source(Arc::clone(&document), ParseOptions::default());
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
