use arcweft_lang_hir::{lower::lower_to_hir, symbol::CallablePackageId};
use arcweft_lang_sema::{
    canonicalization::{CanonicalizationSourceSet, SemanticDocumentId, SemanticSourceIdentity},
    view_part::{CheckedViewPartTargetKind, ViewPartDiagnosticCode, check_view_parts},
};
use arcweft_lang_syntax::{ast::module_path::CanonicalModulePath, parser::parse_source};

fn checked(
    source: &str,
) -> (
    arcweft_lang_sema::view_part::CheckedViewPartCatalog,
    Vec<arcweft_lang_sema::view_part::ViewPartDiagnostic>,
) {
    let parsed = parse_source(source);
    assert_eq!(parsed.errors(), &[]);
    let hir = lower_to_hir(parsed.typed_tree()).unwrap();
    check_view_parts(&hir, None)
}

#[test]
fn checked_view_part_catalog_separates_private_and_public_names() {
    let source = r"pub view Card() {
    export part header as heading
    Panel().part(header)
}
";
    let parsed = parse_source(source);
    let hir = lower_to_hir(parsed.typed_tree()).unwrap();
    let package = CallablePackageId::try_new("example").unwrap();
    let identity = SemanticSourceIdentity::from_source(
        package.clone(),
        SemanticDocumentId::new("file:///card.arcw"),
        CanonicalModulePath::crate_root(),
        source,
    );
    let sources = CanonicalizationSourceSet::try_new(package, [identity.clone()]).unwrap();
    let (catalog, diagnostics) = check_view_parts(&hir, Some(&sources));
    assert_eq!(diagnostics, []);

    let owner = &catalog.owners()[0];
    let local = &owner.local_parts()[0];
    let export = &owner.exports()[0];
    assert_eq!(local.name().public_id().as_str(), "header");
    assert_eq!(export.public_name().public_id().as_str(), "heading");
    assert_eq!(export.source().identity(), Some(&identity));
    assert_eq!(local.target_kind(), CheckedViewPartTargetKind::Element);
}

#[test]
fn checked_view_part_rejects_missing_and_duplicate_targets() {
    let (catalog, diagnostics) = checked(
        r"pub view Card() {
    export part missing as heading
    Panel().part(header)
    Panel().part(header)
}
",
    );
    assert!(catalog.owners()[0].exports().is_empty());
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.code() == ViewPartDiagnosticCode::DuplicateLocalTarget
        })
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.code() == ViewPartDiagnosticCode::MissingLocalTarget })
    );
}

#[test]
fn checked_view_part_rejects_call_view_reexport() {
    let (catalog, diagnostics) = checked(
        r"pub view Parent() {
    export part child as exposed
    Child().part(child)
}
",
    );
    assert!(catalog.owners()[0].exports().is_empty());
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code() == ViewPartDiagnosticCode::UnsupportedCallViewExport
    }));
}

#[test]
fn checked_view_part_records_repeat_occurrence_shape() {
    let (catalog, diagnostics) = checked(
        r"pub view List() {
    for item in items key item.id {
        Text(item.label).part(row)
    }
}
",
    );
    assert_eq!(diagnostics, []);
    let occurrence = catalog.owners()[0].local_parts()[0].occurrence();
    assert!(occurrence.can_be_absent());
    assert!(occurrence.can_repeat());
}
