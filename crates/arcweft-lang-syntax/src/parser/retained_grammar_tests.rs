use std::fmt::Write as _;

use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};

use super::document::parse_shadow_document;
use crate::grammar::build::{GrammarBuild, GrammarBuildError, UnattachedGrammarEntry};
use crate::grammar::kinds::SyntaxKind;
use crate::incremental::SyntaxLimit;

fn document(source: &str) -> SourceDocument {
    SourceDocument::try_new(
        SourceDocumentId::try_new("memory:retained-stage-one").unwrap(),
        SourceName::Memory,
        source,
    )
    .unwrap()
}

fn parse(source: &str) -> GrammarBuild {
    parse_shadow_document(&document(source)).expect("retained Stage 1 grammar builds")
}

fn count_kind(built: &GrammarBuild, kind: SyntaxKind) -> usize {
    built
        .index()
        .entries()
        .iter()
        .map(UnattachedGrammarEntry::kind)
        .filter(|actual| *actual == kind)
        .count()
}

#[test]
fn canonical_mixed_document_dispatches_all_seven_retained_rows() {
    let source = concat!(
        "character Alice {}\n",
        "view Main() { Panel {} }\n",
        "action Continue()\n",
        "activity MiniGame {}\n",
        "signal Ready: Watch<bool>\n",
        "metric counter Frames: u64 {}\n",
        "layer Overlay: overlay {}\n",
        "res dialogue_resource: DialogueResource {}\n",
        "fn helper() { true }\n",
        "proof invariant() { assert true }\n",
        "style @style.native { Button { opacity = 1.0 } }\n",
    );
    let built = parse(source);
    for kind in [
        SyntaxKind::CharacterDeclarationItem,
        SyntaxKind::ViewDeclarationItem,
        SyntaxKind::ActionDeclarationItem,
        SyntaxKind::ActivityDeclarationItem,
        SyntaxKind::SignalDeclarationItem,
        SyntaxKind::MetricDeclarationItem,
        SyntaxKind::LayerDeclarationItem,
    ] {
        assert_eq!(count_kind(&built, kind), 1, "missing {kind:?}");
    }
    assert_eq!(count_kind(&built, SyntaxKind::ErrorItem), 0);
    assert!(built.diagnostics().is_empty(), "{:?}", built.diagnostics());
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn comment_rich_mixed_document_remains_byte_exact() {
    let source = concat!(
        "/// character docs\n",
        "character Alice {} // character tail\n",
        "view Main() { Panel {} } // view tail\n",
        "action Continue() // action tail\n",
        "activity MiniGame {} // activity tail\n",
        "signal Ready: Watch<bool> // signal tail\n",
        "metric counter Frames: u64 {} // metric tail\n",
        "layer Overlay: overlay {} // layer tail\n",
        "res dialogue_resource: DialogueResource {} // resource tail\n",
        "fn helper() { true } // function tail\n",
        "proof invariant() { assert true } // proof tail\n",
        "style @style.native { Button { opacity = 1.0 } } // style tail\n",
    );
    let built = parse(source);
    for kind in [
        SyntaxKind::CharacterDeclarationItem,
        SyntaxKind::ViewDeclarationItem,
        SyntaxKind::ActionDeclarationItem,
        SyntaxKind::ActivityDeclarationItem,
        SyntaxKind::SignalDeclarationItem,
        SyntaxKind::MetricDeclarationItem,
        SyntaxKind::LayerDeclarationItem,
    ] {
        assert_eq!(count_kind(&built, kind), 1, "missing {kind:?}");
    }
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn removed_top_level_families_and_statements_use_ordinary_error_items() {
    let source = concat!(
        "asset room { file = \"room.png\" }\n",
        "content chapter {}\n",
        "extern rust mod native from crate \"native\" {}\n",
        "dialogue defaults {}\n",
        "state GameState {}\n",
        "image portrait {}\n",
        "voice alice {}\n",
        "let top = true\n",
        "character Alice {}\n",
    );
    let built = parse(source);
    assert_eq!(count_kind(&built, SyntaxKind::ErrorItem), 8);
    assert_eq!(count_kind(&built, SyntaxKind::CharacterDeclarationItem), 1);
    assert_eq!(
        built
            .diagnostics()
            .iter()
            .filter(|diagnostic| diagnostic.code() == "syntax.item.expected_declaration")
            .count(),
        8
    );
    assert!(built.diagnostics().iter().all(|diagnostic| {
        !diagnostic.code().contains("removed")
            && !diagnostic.code().contains("asset")
            && !diagnostic.code().contains("source")
    }));
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn top_level_retained_namespace_calls_are_error_items_not_declarations() {
    let source = concat!(
        "action.invoke(@action.Continue)\n",
        "view.mount(@view.Main)\n",
        "activity.start(@activity.MiniGame)\n",
        "character Alice {}\n",
    );
    let built = parse(source);
    assert_eq!(count_kind(&built, SyntaxKind::ErrorItem), 3);
    assert_eq!(count_kind(&built, SyntaxKind::ActionDeclarationItem), 0);
    assert_eq!(count_kind(&built, SyntaxKind::ViewDeclarationItem), 0);
    assert_eq!(count_kind(&built, SyntaxKind::ActivityDeclarationItem), 0);
    assert_eq!(count_kind(&built, SyntaxKind::CharacterDeclarationItem), 1);
    assert_eq!(
        built
            .diagnostics()
            .iter()
            .filter(|diagnostic| diagnostic.code() == "syntax.item.expected_declaration")
            .count(),
        3
    );
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn fixed_parameter_budget_accepts_256_and_rejects_257_transactionally() {
    let accepted = action_with_parameters(256);
    assert!(parse_shadow_document(&document(&accepted)).is_ok());
    let rejected = action_with_parameters(257);
    assert!(matches!(
        parse_shadow_document(&document(&rejected)),
        Err(GrammarBuildError::LimitExceeded(
            SyntaxLimit::FixedParameters
        ))
    ));
    assert!(parse_shadow_document(&document("action Ready()\n")).is_ok());
}

#[test]
fn metric_label_budget_accepts_64_and_rejects_65_transactionally() {
    let accepted = metric_with_labels(64);
    assert!(parse_shadow_document(&document(&accepted)).is_ok());
    let rejected = metric_with_labels(65);
    assert!(matches!(
        parse_shadow_document(&document(&rejected)),
        Err(GrammarBuildError::LimitExceeded(SyntaxLimit::MetricLabels))
    ));
    assert!(parse_shadow_document(&document("metric gauge Ready: f32 {}\n")).is_ok());
}

#[test]
fn layer_member_budget_accepts_64_and_rejects_65_transactionally() {
    let accepted = layer_with_members(64);
    assert!(parse_shadow_document(&document(&accepted)).is_ok());
    let rejected = layer_with_members(65);
    assert!(matches!(
        parse_shadow_document(&document(&rejected)),
        Err(GrammarBuildError::LimitExceeded(SyntaxLimit::LayerMembers))
    ));
    assert!(parse_shadow_document(&document("layer Ready: overlay {}\n")).is_ok());
}

#[test]
fn activity_port_budget_accepts_256_and_rejects_257_transactionally() {
    let accepted = activity_with_ports(256);
    assert!(parse_shadow_document(&document(&accepted)).is_ok());
    let rejected = activity_with_ports(257);
    assert!(matches!(
        parse_shadow_document(&document(&rejected)),
        Err(GrammarBuildError::LimitExceeded(SyntaxLimit::ActivityPorts))
    ));
    assert!(parse_shadow_document(&document("activity Ready {}\n")).is_ok());
}

#[test]
fn metric_bucket_budget_accepts_1024_and_rejects_1025_transactionally() {
    let accepted = metric_with_buckets(1_024);
    assert!(parse_shadow_document(&document(&accepted)).is_ok());
    let rejected = metric_with_buckets(1_025);
    assert!(matches!(
        parse_shadow_document(&document(&rejected)),
        Err(GrammarBuildError::LimitExceeded(SyntaxLimit::MetricBuckets))
    ));
    assert!(
        parse_shadow_document(&document("metric histogram Ready: f64 { buckets = [1] }\n")).is_ok()
    );
}

#[test]
fn view_export_budget_accepts_256_and_rejects_257_transactionally() {
    let accepted = view_with_exports(256);
    assert!(parse_shadow_document(&document(&accepted)).is_ok());
    let rejected = view_with_exports(257);
    assert!(matches!(
        parse_shadow_document(&document(&rejected)),
        Err(GrammarBuildError::LimitExceeded(SyntaxLimit::ViewExports))
    ));
    assert!(parse_shadow_document(&document("view Ready() { Panel {} }\n")).is_ok());
}

#[test]
fn declaration_member_budget_accepts_1024_and_rejects_1025_transactionally() {
    let accepted = character_with_members(1_024);
    assert!(parse_shadow_document(&document(&accepted)).is_ok());
    let rejected = character_with_members(1_025);
    assert!(matches!(
        parse_shadow_document(&document(&rejected)),
        Err(GrammarBuildError::LimitExceeded(
            SyntaxLimit::DeclarationMembers
        ))
    ));
    assert!(parse_shadow_document(&document("character Ready {}\n")).is_ok());
}

fn action_with_parameters(count: usize) -> String {
    let parameters = (0..count)
        .map(|index| format!("value_{index}: u32"))
        .collect::<Vec<_>>()
        .join(", ");
    format!("action Many({parameters})\n")
}

fn metric_with_labels(count: usize) -> String {
    let mut labels = String::new();
    for index in 0..count {
        writeln!(labels, "        label_{index}: String").expect("String writes are infallible");
    }
    format!("metric gauge Many: f32 {{\n    labels {{\n{labels}    }}\n}}\n")
}

fn layer_with_members(count: usize) -> String {
    let mut members = String::new();
    for index in 0..count {
        writeln!(members, "    z = {index}").expect("String writes are infallible");
    }
    format!("layer Many: overlay {{\n{members}}}\n")
}

fn activity_with_ports(count: usize) -> String {
    let mut ports = String::new();
    for index in 0..count {
        writeln!(ports, "        port_{index}: u32").expect("String writes are infallible");
    }
    format!("activity Many {{\n    input {{\n{ports}    }}\n}}\n")
}

fn metric_with_buckets(count: usize) -> String {
    let buckets = (0..count)
        .map(|index| index.to_string())
        .collect::<Vec<_>>()
        .join(", ");
    format!("metric histogram Many: f64 {{\n    buckets = [{buckets}]\n}}\n")
}

fn view_with_exports(count: usize) -> String {
    let mut exports = String::new();
    for index in 0..count {
        writeln!(exports, "    export part local_{index} as public_{index}")
            .expect("String writes are infallible");
    }
    format!("view Many() {{\n{exports}    Panel {{}}\n}}\n")
}

fn character_with_members(count: usize) -> String {
    let mut members = String::new();
    for index in 0..count {
        writeln!(members, "    display_name = \"name_{index}\"")
            .expect("String writes are infallible");
    }
    format!("character Many {{\n{members}}}\n")
}
