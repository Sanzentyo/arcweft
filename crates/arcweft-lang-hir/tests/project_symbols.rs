use arcweft_lang_hir::symbol::{
    ExternalDeclarationSeed, ExternalDeclarationSeedError, ProjectDirectBinding,
};
use arcweft_lang_syntax::ast::{
    common::Visibility,
    module_path::{CanonicalModulePath, ModulePathRoot},
    symbol_path::{ProjectSymbolPath, ProjectSymbolSegment, SymbolPath},
};
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
fn public_direct_binding_api_owns_the_exact_typed_path() {
    let path = ProjectSymbolPath::new(
        ModulePathRoot::ImplicitCrate,
        [
            ProjectSymbolSegment::try_new("character").expect("qualified segment"),
            ProjectSymbolSegment::try_new("hero-pack").expect("external segment"),
        ],
    )
    .expect("typed project path");
    let binding = ProjectDirectBinding::try_new(
        CanonicalModulePath::crate_root(),
        path.clone(),
        Some(Visibility::Public),
        declaration_span(),
        true,
    )
    .expect("public typed direct binding");

    assert_eq!(binding.path(), &path);
    assert!(binding.authored_alias());
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
