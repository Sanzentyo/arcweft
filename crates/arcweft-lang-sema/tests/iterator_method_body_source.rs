use std::sync::Arc;

use arcweft_lang_hir::{
    lower::lower_document_to_hir,
    project::{HirProject, HirProjectModule},
    symbol::{CallablePackageId, ProjectSymbolWorldId},
};
use arcweft_lang_sema::{
    check::analyze_registered_project_types,
    env::TypeCheckEnv,
    registration::{CharacterRegistrar, CharacterRegistrationRequest, ProjectRegistrationFacts},
};
use arcweft_lang_syntax::{ast::module_path::CanonicalModulePath, parser::parse_source};
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};

fn analyze_fixture(source: &str) -> arcweft_lang_sema::check::TypeCheckReport {
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("memory:///iterator-method-body.arcw").expect("source ID"),
            SourceName::path("memory:///iterator-method-body.arcw"),
            source,
        )
        .expect("source document"),
    );
    let parsed = parse_source(source);
    assert!(parsed.errors().is_empty(), "{:?}", parsed.errors());
    let hir = lower_document_to_hir(&document, parsed.typed_tree()).expect("fixture lowers to HIR");
    let package = CallablePackageId::try_new("iterator-method-body").expect("package");
    let project = HirProject::new(
        package.as_str(),
        [HirProjectModule::try_new(
            CanonicalModulePath::crate_root(),
            document.identity().clone(),
            hir,
        )
        .expect("root module")],
    )
    .expect("HIR project");
    let world = ProjectSymbolWorldId::try_new(
        package,
        document.identity().id().clone(),
        "iterator-method-body",
    )
    .expect("symbol world");
    let registration = ProjectRegistrationFacts::try_new(
        world,
        vec![Arc::clone(&document)],
        Vec::new(),
        Vec::new(),
    )
    .expect("registration facts");
    let registered = CharacterRegistrar::register(CharacterRegistrationRequest::new(
        Arc::new(TypeCheckEnv::default()),
        &project,
        &registration,
        None,
    ))
    .expect("registered semantic world");
    analyze_registered_project_types(&project.linked_module(), &registered)
}

#[test]
fn full_iterator_method_body_typechecks() {
    let report = analyze_fixture(include_str!(
        "../../../fixtures/iterator-witness/user-defined.arcw"
    ));
    assert!(
        report.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        report.diagnostics
    );
    assert!(
        report
            .for_iteration_evidence
            .iter()
            .any(|evidence| matches!(
                evidence.family,
                arcweft_lang_sema::check::ForIterationEvidenceFamily::Witness { .. }
            ))
    );
}

#[test]
fn nested_assignment_target_is_structured_diagnostic() {
    let report = analyze_fixture(include_str!(
        "../../../fixtures/iterator-witness/invalid-nested-assignment.arcw"
    ));
    assert!(report.diagnostics.iter().any(
        |diagnostic| diagnostic.stable_code() == "sema.typecheck.unsupported_assignment_target"
    ));
}

#[test]
fn branch_mismatch_under_option_return_is_rejected() {
    let report = analyze_fixture(include_str!(
        "../../../fixtures/iterator-witness/branch-mismatch.arcw"
    ));
    assert!(
        report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message().contains("if expression else branch"))
    );
}
