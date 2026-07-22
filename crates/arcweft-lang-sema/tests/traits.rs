use std::sync::Arc;

use arcweft_lang_hir::{
    lower::{lower_document_to_hir, lower_to_hir},
    project::{HirProject, HirProjectModule},
    symbol::{CallablePackageId, ProjectSymbolWorldId},
};
use arcweft_lang_sema::check::{analyze_registered_project_types, analyze_types};
use arcweft_lang_sema::diagnostics::{TraitDiagnosticKind, TypeCheckErrorKind};
use arcweft_lang_sema::env::TypeCheckEnv;
use arcweft_lang_sema::registration::{
    CharacterRegistrar, CharacterRegistrationRequest, ProjectRegistrationFacts,
};
use arcweft_lang_sema::types::TypeKind;
use arcweft_lang_syntax::{ast::module_path::CanonicalModulePath, parser::parse_source};
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};

fn diagnostics(source: &str) -> Vec<TypeCheckErrorKind> {
    let tree = parse_source(source).into_typed_tree();
    let hir = lower_to_hir(&tree).expect("HIR lowers");
    analyze_types(&hir, &TypeCheckEnv::standard())
        .diagnostics
        .into_iter()
        .map(|diagnostic| diagnostic.kind().clone())
        .collect()
}

fn registered_report(source: &str) -> arcweft_lang_sema::checker::TypeCheckReport {
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("memory:///trait-nominal.arcw").expect("source ID"),
            SourceName::path("memory:///trait-nominal.arcw"),
            source,
        )
        .expect("source document"),
    );
    let parsed = parse_source(source);
    assert!(
        parsed.errors().is_empty(),
        "trait nominal fixture must parse: {:?}",
        parsed.errors()
    );
    let hir = lower_document_to_hir(&document, parsed.typed_tree()).expect("fixture lowers");
    let package = CallablePackageId::try_new("trait-nominal").expect("package");
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
    let world =
        ProjectSymbolWorldId::try_new(package, document.identity().id().clone(), "trait-nominal")
            .expect("symbol world");
    let registration = ProjectRegistrationFacts::try_new(
        world,
        vec![Arc::clone(&document)],
        Vec::new(),
        Vec::new(),
    )
    .expect("registration facts");
    let registered = CharacterRegistrar::register(CharacterRegistrationRequest::new(
        Arc::new(TypeCheckEnv::standard()),
        &project,
        &registration,
        None,
    ))
    .expect("registered semantic world");
    analyze_registered_project_types(&project.linked_module(), &registered)
}

#[test]
fn complete_trait_impl_is_accepted() {
    let diags = diagnostics(include_str!(
        "../../../fixtures/traits/ok-basic-trait-impl.arcw"
    ));
    assert_eq!(diags, []);
}

#[test]
fn predicate_associated_type_uses_nested_resolver_product() {
    let diags = diagnostics(
        r"
trait SourceLike {
    type Item
    fn current(self) -> Self::Item
}

fn current_items<T>(source: T) -> Vec<i32>
where T: SourceLike<Item = Vec<i32>>
{
    source.current()
}
",
    );
    assert_eq!(diags, []);
}

#[test]
fn source_backed_predicate_keeps_project_nominal_identity() {
    let report = registered_report(
        r"
struct ChapterId {
    value: i32
}

trait SourceLike {
    type Item
    fn current(self) -> Self::Item
}

fn current_items<T>(source: T) -> Vec<ChapterId>
where T: SourceLike<Item = Vec<ChapterId>>
{
    source.current()
}
",
    );
    assert_eq!(report.diagnostics, []);
    assert!(report.nominal_resolutions.nodes().any(|(_, node)| {
        matches!(
            node.recovered(),
            Some(TypeKind::Vec(item)) if matches!(item.as_ref(), TypeKind::ProjectNominal(_))
        )
    }));
}

#[test]
fn trait_method_generic_parameters_are_compared_by_position() {
    let diags = diagnostics(
        r"
trait Mapper {
    fn map<T>(self, value: T) -> T
}

impl Mapper for i32 {
    fn map<U>(self, value: U) -> U {
        value
    }
}
",
    );
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
