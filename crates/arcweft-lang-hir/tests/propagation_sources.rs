use arcweft_lang_hir::{
    lower::lower_document_to_hir,
    model::HirTopLevelDecl,
    project::{HirProject, HirProjectModule},
    symbol::CallableDeclarationId,
};
use arcweft_lang_syntax::{
    ast::{common::TextRange, items::ImplMember, module_path::CanonicalModulePath},
    expr::Expr,
    parser::parse_source,
};
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};

fn lower_bound(label: &str, source: &str) -> (SourceDocument, arcweft_lang_hir::model::HirModule) {
    let document = SourceDocument::try_new(
        SourceDocumentId::try_new(format!("memory:///{label}.arcw")).expect("document ID"),
        SourceName::Generated,
        source,
    )
    .expect("source document");
    let parsed = parse_source(source);
    assert!(parsed.errors().is_empty(), "{:?}", parsed.errors());
    let hir = lower_document_to_hir(&document, parsed.typed_tree()).expect("source lowers");
    (document, hir)
}

fn range_of(source: &str, needle: &str) -> TextRange {
    let start = source
        .find(needle)
        .expect("fixture contains source fragment");
    TextRange::new(start, start + needle.len())
}

#[test]
fn try_source_survives_document_bound_hir_unchanged() {
    let source = concat!(
        "// 前置き\n",
        "fn propagate(value: Result<i64, String>) -> Result<i64, String> {\n",
        "    value?\n",
        "}\n",
    );
    let (_, hir) = lower_bound("try-source-hir", source);
    let Expr::Try(try_expr) = hir.functions()[0]
        .value()
        .expect("function tail value")
        .expr()
    else {
        panic!("expected retained Try expression")
    };

    let whole = range_of(source, "value?");
    assert_eq!(try_expr.source().whole(), whole);
    assert_eq!(
        try_expr.source().operand(),
        TextRange::new(whole.start(), whole.end() - '?'.len_utf8())
    );
    assert_eq!(try_expr.source().operator_range(), range_of(source, "?"));
    assert_eq!(
        hir.source_span(try_expr.source().operator_range())
            .expect("operator binds to source")
            .range()
            .start(),
        range_of(source, "?").start()
    );
}

#[test]
fn function_result_source_survives_document_bound_hir_unchanged() {
    let source = "fn demo(value: Result<i64, String>) -> Result<i64, i64> {\n    Ok(value?)\n}\n";
    let (_, hir) = lower_bound("function-result-source-hir", source);

    assert_eq!(
        hir.functions()[0].signature_source().result(),
        Some(TextRange::new(39, 55))
    );
}

#[test]
fn flow_method_and_closure_sources_survive_hir_with_exact_result_ranges() {
    let source = concat!(
        "// 型境界\n",
        "flow @flow.audit audit(value: Result<i64, String>) -> Result<i64, String> {\n",
        "    let unwrapped = value?\n",
        "}\n",
        "impl Handler {\n",
        "    fn handle(value: Result<i64, String>) -> Result<i64, String> {\n",
        "        value?\n",
        "    }\n",
        "}\n",
        "fn build() -> Handler {\n",
        "    |value: Result<i64, String>| -> Result<i64, String> { value? }\n",
        "}\n",
    );
    let (_, hir) = lower_bound("boundary-source-hir", source);

    let flow = &hir.flows()[0];
    let flow_header = "flow @flow.audit audit(value: Result<i64, String>) -> Result<i64, String>";
    assert_eq!(
        flow.signature_source().header(),
        range_of(source, flow_header)
    );
    let first_result = range_of(source, "Result<i64, String>");
    let flow_result_start = source[first_result.end()..]
        .find("Result<i64, String>")
        .expect("flow result")
        + first_result.end();
    assert_eq!(
        flow.signature_source().result(),
        Some(TextRange::new(
            flow_result_start,
            flow_result_start + "Result<i64, String>".len()
        ))
    );

    let HirTopLevelDecl::Impl(impl_item) = &hir.declarations()[0] else {
        panic!("expected retained impl declaration")
    };
    let ImplMember::Function {
        signature_source, ..
    } = &impl_item.members()[0]
    else {
        panic!("expected retained method")
    };
    let method_signature = "fn handle(value: Result<i64, String>) -> Result<i64, String>";
    assert_eq!(
        signature_source.signature(),
        range_of(source, method_signature)
    );
    let method_result_start = source.find(method_signature).expect("method signature")
        + method_signature
            .rfind("Result<i64, String>")
            .expect("method result");
    assert_eq!(
        signature_source.result(),
        Some(TextRange::new(
            method_result_start,
            method_result_start + "Result<i64, String>".len()
        ))
    );

    let Expr::Closure {
        body,
        source: closure_source,
        ..
    } = hir.functions()[0]
        .value()
        .expect("builder tail value")
        .expr()
    else {
        panic!("expected retained closure")
    };
    let closure_whole = "|value: Result<i64, String>| -> Result<i64, String> { value? }";
    assert_eq!(closure_source.whole(), range_of(source, closure_whole));
    assert_eq!(closure_source.body(), range_of(source, "{ value? }"));
    let Expr::Block {
        value: Some(value), ..
    } = body.as_ref()
    else {
        panic!("closure body remains a typed block")
    };
    let Expr::Try(try_expr) = value.as_ref() else {
        panic!("closure body retains nested Try")
    };
    let closure_start = source.find(closure_whole).expect("closure source");
    let question = closure_start + closure_whole.rfind('?').expect("closure Try operator");
    assert_eq!(
        try_expr.source().operator_range(),
        TextRange::new(question, question + '?'.len_utf8())
    );
}

#[test]
fn propagation_does_not_publish_a_second_callable_catalog_record() {
    let source = concat!(
        "pub fn controller(value: Result<i64, String>) -> Result<i64, String> {\n",
        "    value?\n",
        "}\n",
    );
    let (document, hir) = lower_bound("single-callable-source", source);
    let module = HirProjectModule::try_new(
        CanonicalModulePath::crate_root(),
        document.identity().clone(),
        hir,
    )
    .expect("root project module");
    let project = HirProject::new("propagation-test", [module]).expect("HIR project");
    let records = project.callable_signature_sources().collect::<Vec<_>>();
    assert_eq!(records.len(), 1);
    let function = &project
        .module(&CanonicalModulePath::crate_root())
        .expect("root module")
        .functions()[0];
    let expected = CallableDeclarationId::for_function(project.package(), function)
        .expect("function declaration identity");
    assert_eq!(records[0].declaration(), &expected);
}
