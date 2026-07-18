use super::*;
use arcweft_bundle::resource_codec::{ValidatedViewProduct, ViewProductValidationLimits};
use arcweft_lang_sema::view_part::check_view_parts;
use arcweft_lang_syntax::ast::{
    items::Item,
    view::{ViewBody, ViewExpr},
};
use arcweft_lang_syntax::parser::{ParseOptions, parse_document_with_source};
use arcweft_runtime_driver::view_runtime::BundleViewRuntime;
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};
use std::sync::Arc;

fn first_view_body(parsed: &arcweft_lang_syntax::source::ParsedSource) -> &ViewBody {
    parsed
        .typed_tree()
        .items()
        .iter()
        .find_map(|item| match item {
            Item::EntityDecl(item) => item.view_body()?.view(),
            _ => None,
        })
        .expect("recovered View body")
}

#[test]
fn ordinary_view_recovery_cannot_create_exported_part_facts() {
    let source = r#"
view Card() {
  UnknownContainer {
    Text("Title").part(title)
  }
}

flow test {
  view(@view.Card)
}
"#;
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("recovered-view.arcw").expect("source identity"),
            SourceName::path("recovered-view.arcw"),
            source,
        )
        .expect("source document"),
    );
    let parsed = parse_document_with_source(document, ParseOptions::default());

    assert!(
        !parsed.errors().is_empty(),
        "non-current View input must use ordinary parser recovery"
    );
    let body = first_view_body(&parsed);
    assert!(body.exports().is_empty());
    assert!(matches!(body.value(), ViewExpr::Raw(_)));

    let hir = arcweft_lang_hir::lower::lower_to_hir(parsed.typed_tree())
        .expect("ordinary recovery lowers without an exported-part node");
    assert!(hir.view_parts().is_empty());

    let (checked, _) = check_view_parts(&hir);
    assert!(
        checked
            .owners()
            .iter()
            .all(|owner| owner.exports().is_empty())
    );

    let sidecars =
        collect_bundle_dsl_view_resources_from_source(&hir, &[], "recovered-view.arcw", source)
            .expect("recovered View lowers to typed product resources");
    let program = sidecars.program.expect("recovered View program");
    assert!(program.source_refs.is_empty());
    assert!(program.exported_parts.is_empty());

    let product = ValidatedViewProduct::try_new(
        None,
        Some(program),
        sidecars.style,
        ViewProductValidationLimits::default(),
    )
    .expect("source-free recovered product validates");
    let runtime = BundleViewRuntime::try_new(product, sidecars.text)
        .expect("runtime accepts recovered product without export facts");
    assert!(
        runtime
            .catalog()
            .expect("accepted View catalog")
            .definitions()
            .all(|(_, definition)| definition.exported_parts().is_empty())
    );
}
