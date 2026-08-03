use std::fmt::Write;

use arcweft_source::{SourceDocument, SourceDocumentId, SourceName, SourceRange};

use super::document::parse_shadow_document;
use crate::grammar::build::GrammarBuildError;
use crate::grammar::build::{GrammarBuild, UnattachedGrammarEntry};
use crate::grammar::kinds::SyntaxKind;
use crate::incremental::SyntaxLimit;
use crate::parser::ParseOptions;

fn document(source: &str) -> SourceDocument {
    SourceDocument::try_new(
        SourceDocumentId::try_new("memory:retained-layer").unwrap(),
        SourceName::Memory,
        source,
    )
    .unwrap()
}

fn parse(source: &str) -> GrammarBuild {
    parse_shadow_document(&document(source), crate::parser::ParseOptions::default())
        .expect("Layer grammar builds")
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

fn nth_source_range(source: &str, fragment: &str, occurrence: usize) -> SourceRange {
    let start = source
        .match_indices(fragment)
        .nth(occurrence)
        .map(|(start, _)| start)
        .expect("fixture occurrence");
    SourceRange::new(start, start + fragment.len())
}

#[test]
fn canonical_layer_owns_kind_singletons_policies_and_references() {
    let source = concat!(
        "pub layer @layer.dialogue dialogue_ui: dialogue {\n",
        "    parent = @layer.root\n",
        "    phase = dialogue\n",
        "    z = 10\n",
        "    visible = true\n",
        "    transform = Transform.identity()\n",
        "    input = hit_test\n",
        "    hit_test = view_tree\n",
        "    capture = color\n",
        "    accessibility = exposed\n",
        "    view = @view.MainDialogue\n",
        "}\n",
    );
    let built = parse(source);
    assert_eq!(count_kind(&built, SyntaxKind::LayerDeclarationItem), 1);
    assert_eq!(count_kind(&built, SyntaxKind::LayerMember), 10);
    assert_eq!(count_kind(&built, SyntaxKind::RetainedReference), 2);
    assert_eq!(count_kind(&built, SyntaxKind::LayerPolicyValue), 5);
    assert!(built.diagnostics().is_empty(), "{:?}", built.diagnostics());
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn layer_header_and_duplicate_member_errors_keep_typed_nodes() {
    let source = concat!(
        "layer Broken World {\n",
        "    z = 1\n",
        "    z = 2\n",
        "}\n",
        "signal ready: Watch<bool>\n",
    );
    let built = parse(source);
    assert_eq!(count_kind(&built, SyntaxKind::LayerDeclarationItem), 1);
    assert_eq!(count_kind(&built, SyntaxKind::LayerMember), 2);
    assert_eq!(count_kind(&built, SyntaxKind::SignalDeclarationItem), 1);
    for code in [
        "syntax.layer.missing_colon",
        "syntax.layer.unknown_kind",
        "syntax.layer.duplicate_member",
    ] {
        assert!(
            built
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.code() == code),
            "missing {code}: {:?}",
            built.diagnostics()
        );
    }
    let duplicate = built
        .diagnostics()
        .iter()
        .find(|diagnostic| diagnostic.code() == "syntax.layer.duplicate_member")
        .expect("duplicate layer member diagnostic");
    assert_eq!(duplicate.range(), nth_source_range(source, "z", 1));
    assert_eq!(
        duplicate.related_range(),
        Some(nth_source_range(source, "z", 0))
    );
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn engine_root_and_unknown_layer_members_are_rejected() {
    let source = concat!(
        "layer RootLayer: root {\n",
        "    children = [@layer.other]\n",
        "    activity =\n",
        "}\n",
    );
    let built = parse(source);
    assert!(
        built
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code() == "syntax.layer.unknown_kind")
    );
    assert!(
        built
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code() == "syntax.layer.unknown_member")
    );
    assert!(
        built
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code() == "syntax.layer.missing_reference")
    );
}

#[test]
fn layer_reference_families_and_closed_policies_are_checked_in_the_typed_tree() {
    let source = concat!(
        "layer Wrong: game_view {\n",
        "    parent = @view.parent\n",
        "    view = @activity.game\n",
        "    input = unknown\n",
        "}\n",
    );
    let built = parse(source);
    assert_eq!(count_kind(&built, SyntaxKind::WrongFamilyReference), 2);
    assert_eq!(
        built
            .diagnostics()
            .iter()
            .filter(|diagnostic| diagnostic.code() == "syntax.layer.wrong_reference_family")
            .count(),
        2
    );
    assert!(
        built
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code() == "syntax.layer.unknown_policy")
    );
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn unknown_layer_members_share_the_exact_layer_limit_and_rollback_cleanly() {
    let accepted = layer_with_unknown_members(SyntaxLimit::LayerMembers.maximum());
    assert!(parse_shadow_document(&document(&accepted), ParseOptions::default()).is_ok());

    let rejected = layer_with_unknown_members(SyntaxLimit::LayerMembers.maximum() + 1);
    assert!(matches!(
        parse_shadow_document(&document(&rejected), ParseOptions::default()),
        Err(GrammarBuildError::LimitExceeded(SyntaxLimit::LayerMembers))
    ));
    assert!(
        parse_shadow_document(
            &document("layer Ready: overlay {}\n"),
            ParseOptions::default()
        )
        .is_ok()
    );
}

fn layer_with_unknown_members(count: usize) -> String {
    let mut members = String::new();
    for index in 0..count {
        writeln!(members, "    unknown_{index} = true").expect("String writes are infallible");
    }
    format!("layer Many: custom {{\n{members}}}\n")
}
