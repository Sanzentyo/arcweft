use std::sync::Arc;

use arcweft_lang_hir::{
    lower::lower_document_to_hir,
    project::{HirProject, HirProjectModule},
};
use arcweft_lang_syntax::{ast::module_path::CanonicalModulePath, parser::parse_source};
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName, SourceRange};

use crate::{
    canonicalization::CanonicalizationSourceSet,
    checker::{CallableExecutionMode, TypeCheckReport, analyze_project_types_for_canonicalization},
    env::TypeCheckEnv,
    types::TypeKind,
};

fn analyze_project(source: &str) -> TypeCheckReport {
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-project://function-execution/src/main.arcw")
                .expect("document id"),
            SourceName::path("src/main.arcw"),
            source,
        )
        .expect("source document"),
    );
    let parsed = parse_source(source);
    assert!(parsed.errors().is_empty(), "{:?}", parsed.errors());
    let hir = lower_document_to_hir(&document, parsed.typed_tree()).expect("lowered HIR");
    let root = CanonicalModulePath::crate_root();
    let project = HirProject::new(
        "function-execution",
        [
            HirProjectModule::try_new(root.clone(), document.identity().clone(), hir)
                .expect("root module"),
        ],
    )
    .expect("HIR project");
    let source_span = document
        .span(SourceRange::new(0, source.len()))
        .expect("complete source span");
    let sources =
        CanonicalizationSourceSet::try_new(project.package().clone(), [(root, source_span)])
            .expect("canonicalization source set");
    analyze_project_types_for_canonicalization(&project, &TypeCheckEnv::new(), &sources)
        .expect("project semantic analysis")
}

fn execution<'a>(report: &'a TypeCheckReport, name: &str) -> &'a CallableExecutionMode {
    report
        .callable_executions
        .iter()
        .find(|fact| fact.declaration().name() == name)
        .unwrap_or_else(|| panic!("missing execution fact for `{name}`"))
        .mode()
}

#[test]
fn ordinary_fn_with_owned_yield_is_stream_factory() {
    let report = analyze_project(
        r"
fn produce() -> Stream<i64, ArcError> {
    yield 1i64
}
",
    );
    assert!(report.diagnostics.is_empty(), "{:?}", report.diagnostics);
    let CallableExecutionMode::StreamFactory {
        item,
        error,
        generator,
    } = execution(&report, "produce")
    else {
        panic!("owned yield must classify ordinary fn as StreamFactory")
    };
    assert_eq!(item, &TypeKind::I64);
    assert_eq!(error, &TypeKind::Named("ArcError".to_owned()));
    assert_eq!(generator.own_scope_yield_count(), 1);
}

#[test]
fn stream_return_without_owned_yield_remains_direct_frame() {
    let report = analyze_project(
        r"
fn passthrough(stream: Stream<i64, ArcError>) -> Stream<i64, ArcError> {
    stream
}
",
    );
    assert!(report.diagnostics.is_empty(), "{:?}", report.diagnostics);
    assert_eq!(
        execution(&report, "passthrough"),
        &CallableExecutionMode::DirectFrame
    );
}

#[test]
fn syntactic_yield_in_unreachable_branch_still_classifies_factory() {
    let report = analyze_project(
        r"
fn maybe() -> Stream<i64, ArcError> {
    if false {
        yield 1i64
    }
}
",
    );
    assert!(report.diagnostics.is_empty(), "{:?}", report.diagnostics);
    let CallableExecutionMode::StreamFactory { generator, .. } = execution(&report, "maybe") else {
        panic!("syntactic owned yield must classify before reachability pruning")
    };
    assert_eq!(generator.own_scope_yield_count(), 1);
}

#[test]
fn seq_owned_yield_does_not_classify_enclosing_stream_passthrough() {
    let report = analyze_project(
        r"
fn passthrough(stream: Stream<i64, ArcError>) -> Stream<i64, ArcError> {
    let values = || -> Seq<i64> {
        seq {
            yield 1i64
        }
    }
    stream
}
",
    );
    assert!(report.diagnostics.is_empty(), "{:?}", report.diagnostics);
    assert_eq!(
        execution(&report, "passthrough"),
        &CallableExecutionMode::DirectFrame
    );
}

#[test]
fn yield_records_static_suspend_effect() {
    let report = analyze_project(
        r"
fn produce() -> Stream<i64, ArcError> {
    yield 1i64
}
",
    );
    assert!(
        report.effects.summaries().any(|(_, summary)| summary
            .inferred()
            .iter()
            .any(|effect| effect.as_str() == "control.suspend")),
        "yield must infer control.suspend: {:?}",
        report.effects
    );
}
