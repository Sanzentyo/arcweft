use std::{fmt::Write as _, sync::Arc};

use arcweft_lang_hir::{
    lower::lower_document_to_hir,
    project::{HirProject, HirProjectModule},
    symbol::{CallablePackageId, ProjectSymbolWorldId},
};
use arcweft_lang_sema::{
    callable::{
        CallPoison, CallTargetFact, CallTargetFacts, CallableDiagnosticCode, CallableGroupIndex,
        CallableParameterCoordinate,
    },
    checker::{
        TypeCheckReport, TypeExpressionId, analyze_registered_project_types,
        analyze_registered_project_types_for_focused_call,
    },
    env::TypeCheckEnv,
    registration::{
        CharacterRegistrar, CharacterRegistrationRequest, ProjectRegistrationFacts,
        RegisteredSemanticWorld,
    },
    types::TypeKind,
};
use arcweft_lang_syntax::{
    ast::module_path::CanonicalModulePath,
    parser::{ParseOptions, parse_document_with_source},
};
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName, SourceRange};

fn registered_fixture(source: &str) -> (Arc<SourceDocument>, HirProject, RegisteredSemanticWorld) {
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("memory:///call-target-facts.arcw").expect("source ID"),
            SourceName::path("memory:///call-target-facts.arcw"),
            source,
        )
        .expect("source document"),
    );
    let parsed = parse_document_with_source(Arc::clone(&document), ParseOptions::default());
    assert!(
        parsed.errors().is_empty(),
        "public fact fixture must parse: {:?}",
        parsed.errors()
    );
    let hir = lower_document_to_hir(&document, parsed.typed_tree()).expect("fixture lowers");
    let package = CallablePackageId::try_new("call-target-facts-api").expect("package");
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
        "call-target-facts",
    )
    .expect("symbol world");
    let registration = ProjectRegistrationFacts::try_new(
        world,
        vec![Arc::clone(&document)],
        Vec::new(),
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
    (document, project, registered)
}

fn analyze(source: &str) -> (Arc<SourceDocument>, TypeCheckReport) {
    let (document, project, registered) = registered_fixture(source);
    let report = analyze_registered_project_types(&project.linked_module(), &registered);
    (document, report)
}

#[test]
fn public_focused_entry_returns_a_report_with_the_exact_requested_call_fact() {
    const SOURCE: &str = r"
fn identity(value: i32) -> i32 { value }
flow @flow.main main {
    let first: i32 = identity(1i32)
    let second: i32 = identity(2i32)
}
";
    let (document, project, registered) = registered_fixture(SOURCE);
    let call = "identity(2i32)";
    let start = document.text().find(call).expect("focused call text");
    let span = document
        .span(SourceRange::new(start, start + call.len()))
        .expect("focused call span");

    let report = analyze_registered_project_types_for_focused_call(
        &project.linked_module(),
        &registered,
        span.clone(),
    )
    .expect("public focused analysis");
    let facts = report
        .focused_call_target_facts()
        .expect("focused report owns one fact");

    assert_eq!(facts.call_span(), &span);
    assert_eq!(facts.document(), document.identity());
    assert!(matches!(facts.target(), CallTargetFact::Selected { .. }));
    assert_eq!(facts.result(), Some(&TypeKind::I32));
}

fn fact_for_text<'a>(
    document: &SourceDocument,
    report: &'a TypeCheckReport,
    text: &str,
) -> (TypeExpressionId, &'a CallTargetFacts) {
    let start = document.text().find(text).expect("call text exists");
    assert!(
        document.text()[start + text.len()..].find(text).is_none(),
        "call text must be unique"
    );
    let end = start + text.len();
    (0..report.stats.expressions)
        .find_map(|index| {
            let expression = TypeExpressionId::from_index(index);
            report
                .call_target_facts(expression)
                .expect("fact report is internally valid")
                .filter(|facts| {
                    facts.call_span().range().start() == start
                        && facts.call_span().range().end() == end
                })
                .map(|facts| (expression, facts))
        })
        .expect("exact call fact")
}

fn facts_for_text<'a>(
    document: &SourceDocument,
    report: &'a TypeCheckReport,
    text: &str,
) -> Vec<&'a CallTargetFacts> {
    let start = document.text().find(text).expect("call text exists");
    assert!(
        document.text()[start + text.len()..].find(text).is_none(),
        "call text must be unique"
    );
    let end = start + text.len();
    (0..report.stats.expressions)
        .filter_map(|index| {
            report
                .call_target_facts(TypeExpressionId::from_index(index))
                .expect("fact report is internally valid")
                .filter(|facts| {
                    facts.call_span().range().start() == start
                        && facts.call_span().range().end() == end
                })
        })
        .collect()
}

#[test]
fn accepted_report_exposes_exact_identity_groups_and_checked_mapping() {
    const SOURCE: &str = r#"
fn surround(prefix: String)(value: i32) -> String {
    prefix
}

flow @flow.main main {
    let staged = surround(prefix = ">")
    let complete: String = staged(1i32)
}
"#;
    let (document, report) = analyze(SOURCE);
    assert!(report.diagnostics.is_empty(), "{:?}", report.diagnostics);

    let (expression, facts) = fact_for_text(&document, &report, "surround(prefix = \">\")");
    let first_group = CallableGroupIndex::try_from_usize(0).expect("first group");
    assert_eq!(facts.expression(), expression);
    assert_eq!(facts.document(), document.identity());
    assert_eq!(facts.call_span().source(), document.identity());
    assert_eq!(facts.current_group(), first_group);
    assert_eq!(
        facts.next_group(),
        Some(CallableGroupIndex::try_from_usize(1).expect("second group"))
    );
    assert_eq!(facts.poison(), CallPoison::Clean);
    assert!(matches!(facts.target(), CallTargetFact::Selected { .. }));

    let [argument] = facts.arguments() else {
        panic!("one authored argument must be retained")
    };
    assert_eq!(argument.index().get(), 0);
    assert_eq!(
        &document.text()[argument
            .source()
            .expect("argument source")
            .range()
            .as_range()],
        "prefix = \">\""
    );
    assert_eq!(
        argument
            .authored_name()
            .map(arcweft_lang_sema::callable::CallableName::as_str),
        Some("prefix")
    );
    assert_eq!(
        &document.text()[argument
            .authored_name_source()
            .expect("authored name source")
            .range()
            .as_range()],
        "prefix"
    );
    assert!(!argument.spread());
    let [slot] = argument.slots() else {
        panic!("ordinary argument must retain one typed slot")
    };
    assert_eq!(slot.slot().get(), 0);
    assert_eq!(slot.inferred(), Some(&TypeKind::String));
    assert_eq!(slot.expected(), Some(&TypeKind::String));
    assert_eq!(
        slot.mapped(),
        Some(CallableParameterCoordinate::new(
            first_group,
            arcweft_lang_sema::callable::CallableParameterIndex::try_from_usize(0)
                .expect("first parameter"),
        ))
    );
    assert_ne!(slot.expression(), facts.expression());
}

#[test]
fn nested_facts_survive_a_rejected_outer_candidate_transaction() {
    const SOURCE: &str = r#"
fn text_value(value: i32) -> String {
    "text"
}

fn accept_number(value: i32) -> String {
    "number"
}

flow @flow.main main {
    let invalid: String = accept_number(text_value(1i32))
}
"#;
    let (document, report) = analyze(SOURCE);
    let (_, outer) = fact_for_text(&document, &report, "accept_number(text_value(1i32))");
    let (_, inner) = fact_for_text(&document, &report, "text_value(1i32)");

    let CallTargetFact::Rejected { candidates } = outer.target() else {
        panic!("a resolved but non-viable call must retain a rejected target")
    };
    assert_eq!(candidates.len(), 1);
    assert_eq!(
        outer
            .diagnostics()
            .iter()
            .map(arcweft_lang_sema::callable::CallableDiagnostic::code)
            .collect::<Vec<_>>(),
        vec![CallableDiagnosticCode::NoViableSignature]
    );
    assert_eq!(outer.poison(), CallPoison::Rejected);
    assert!(matches!(inner.target(), CallTargetFact::Selected { .. }));
    assert_eq!(inner.result(), Some(&TypeKind::String));
    assert_eq!(inner.poison(), CallPoison::Clean);
}

#[test]
fn collecting_many_ordinary_facts_does_not_apply_focused_query_limits() {
    let mut source =
        String::from("fn identity(value: i32) -> i32 { value }\nflow @flow.main main {\n");
    for index in 0..96 {
        writeln!(source, "    let value_{index}: i32 = identity({index}i32)")
            .expect("write fixture call");
    }
    source.push_str("}\n");

    let (_, report) = analyze(&source);
    assert!(report.diagnostics.is_empty(), "{:?}", report.diagnostics);
    let retained = (0..report.stats.expressions)
        .filter(|index| {
            report
                .call_target_facts(TypeExpressionId::from_index(*index))
                .expect("ordinary collection has no focused-query error")
                .is_some()
        })
        .count();
    assert_eq!(retained, 96);
}

#[test]
fn whole_module_missing_calls_retain_facts_without_swallowing_ordinary_rejection() {
    const SOURCE: &str = r"
fn identity(value: u8) -> u8 { value }
flow @flow.main main {
    let free: u8 = unknown(identity(1u8))
    let dotted: u8 = unregistered.resolve(identity(300u8))
}
";
    let (document, report) = analyze(SOURCE);

    assert!(
        report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message() == "unknown function `unknown`"),
        "whole-module facts must not consume the ordinary free-call rejection: {:?}",
        report.diagnostics
    );
    assert!(
        report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message() == "unknown symbol `unregistered`"),
        "whole-module facts must not consume the ordinary dotted-call rejection: {:?}",
        report.diagnostics
    );
    assert_eq!(
        report
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.message().contains("300u8"))
            .count(),
        1,
        "the dotted-call argument must be type checked exactly once"
    );

    for call in [
        "unknown(identity(1u8))",
        "unregistered.resolve(identity(300u8))",
    ] {
        let facts = facts_for_text(&document, &report, call);
        assert_eq!(facts.len(), 1, "one Missing fact per authored call");
        assert!(matches!(facts[0].target(), CallTargetFact::Missing { .. }));
        assert_eq!(facts[0].arguments().len(), 1);
    }
    for call in ["identity(1u8)", "identity(300u8)"] {
        let facts = facts_for_text(&document, &report, call);
        assert_eq!(facts.len(), 1, "nested argument calls are checked once");
        assert!(matches!(facts[0].target(), CallTargetFact::Selected { .. }));
    }
}
