use arcweft_lang_hir::symbol::{ExternalDeclarationSeed, ExternalDeclarationSeedError};
use arcweft_lang_syntax::ast::{module_path::ModulePathRoot, symbol_path::SymbolPath};
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName, SourceRange};

fn declaration_span() -> arcweft_source::SourceSpan {
    SourceDocument::try_new(
        SourceDocumentId::try_new("arcweft-generated://project-symbol-tests/external")
            .expect("document id"),
        SourceName::Generated,
        "character.akane",
    )
    .expect("document")
    .span(SourceRange::new(0, "character.akane".len()))
    .expect("span")
}

#[test]
fn external_declarations_reject_missing_direct_binding() {
    let canonical_path =
        SymbolPath::try_new(ModulePathRoot::ImplicitCrate, Vec::new(), "character.akane")
            .expect("canonical path");
    let declaration = declaration_span();

    assert_eq!(
        ExternalDeclarationSeed::try_new(
            canonical_path.clone(),
            None,
            declaration.clone(),
            Vec::new(),
        ),
        Err(ExternalDeclarationSeedError::MissingDirectBinding {
            canonical_path,
            declaration,
        })
    );
}

#[test]
fn external_id_is_opaque() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/external_id_*.rs");
}

#[test]
fn external_input_constructors_are_the_only_public_construction_path() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/external_input_fields_private.rs");
}
