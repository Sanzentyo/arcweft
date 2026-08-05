use arcweft_source::{SourceDocument, SourceDocumentId, SourceName, SourceRange};

use super::document::parse_shadow_document;
use crate::grammar::build::{GrammarBuild, UnattachedGrammarEntry};
use crate::grammar::kinds::SyntaxKind;

fn document(source: &str) -> SourceDocument {
    SourceDocument::try_new(
        SourceDocumentId::try_new("memory:retained-header").unwrap(),
        SourceName::Memory,
        source,
    )
    .unwrap()
}

fn parse(source: &str) -> GrammarBuild {
    parse_shadow_document(&document(source), crate::parser::ParseOptions::default())
        .expect("retained header grammar builds")
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

fn source_range(source: &str, fragment: &str) -> SourceRange {
    let start = source.find(fragment).expect("fixture fragment");
    SourceRange::new(start, start + fragment.len())
}

#[derive(Clone, Copy)]
struct FamilyFixture {
    kind: SyntaxKind,
    missing_name: &'static str,
    wrong_family_id: &'static str,
    relative_id: &'static str,
    keyword_name: &'static str,
    wrong_id_text: &'static str,
    keyword_text: &'static str,
}

const FAMILY_FIXTURES: [FamilyFixture; 7] = [
    FamilyFixture {
        kind: SyntaxKind::CharacterDeclarationItem,
        missing_name: "character {}\n",
        wrong_family_id: "character @view.Wrong Wrong {}\n",
        relative_id: "character @character:.Wrong Wrong {}\n",
        keyword_name: "character view {}\n",
        wrong_id_text: "@view.Wrong",
        keyword_text: "view",
    },
    FamilyFixture {
        kind: SyntaxKind::ViewDeclarationItem,
        missing_name: "view () {}\n",
        wrong_family_id: "view @action.Wrong Wrong() {}\n",
        relative_id: "view @view:.Wrong Wrong() {}\n",
        keyword_name: "view action() {}\n",
        wrong_id_text: "@action.Wrong",
        keyword_text: "action",
    },
    FamilyFixture {
        kind: SyntaxKind::ActionDeclarationItem,
        missing_name: "action ()\n",
        wrong_family_id: "action @signal.Wrong Wrong()\n",
        relative_id: "action @action:.Wrong Wrong()\n",
        keyword_name: "action signal()\n",
        wrong_id_text: "@signal.Wrong",
        keyword_text: "signal",
    },
    FamilyFixture {
        kind: SyntaxKind::ActivityDeclarationItem,
        missing_name: "activity {}\n",
        wrong_family_id: "activity @layer.Wrong Wrong {}\n",
        relative_id: "activity @activity:.Wrong Wrong {}\n",
        keyword_name: "activity layer {}\n",
        wrong_id_text: "@layer.Wrong",
        keyword_text: "layer",
    },
    FamilyFixture {
        kind: SyntaxKind::SignalDeclarationItem,
        missing_name: "signal : Watch<bool>\n",
        wrong_family_id: "signal @metric.Wrong Wrong: Watch<bool>\n",
        relative_id: "signal @signal:.Wrong Wrong: Watch<bool>\n",
        keyword_name: "signal metric: Watch<bool>\n",
        wrong_id_text: "@metric.Wrong",
        keyword_text: "metric",
    },
    FamilyFixture {
        kind: SyntaxKind::MetricDeclarationItem,
        missing_name: "metric gauge : f32 {}\n",
        wrong_family_id: "metric gauge @character.Wrong Wrong: f32 {}\n",
        relative_id: "metric gauge @metric:.Wrong Wrong: f32 {}\n",
        keyword_name: "metric gauge character: f32 {}\n",
        wrong_id_text: "@character.Wrong",
        keyword_text: "character",
    },
    FamilyFixture {
        kind: SyntaxKind::LayerDeclarationItem,
        missing_name: "layer : overlay {}\n",
        wrong_family_id: "layer @activity.Wrong Wrong: overlay {}\n",
        relative_id: "layer @layer:.Wrong Wrong: overlay {}\n",
        keyword_name: "layer activity: overlay {}\n",
        wrong_id_text: "@activity.Wrong",
        keyword_text: "activity",
    },
];

#[test]
fn retained_headers_keep_docs_attributes_visibility_ids_names_and_aliases_typed() {
    let source = concat!(
        "/// Character documentation\n",
        "#[verify.reviewed]\n",
        "pub(crate) character @character.alice Alice as alice {\n",
        "    display_name = \"Alice\"\n",
        "}\n",
    );
    let built = parse(source);
    for kind in [
        SyntaxKind::CharacterDeclarationItem,
        SyntaxKind::DeclarationHeader,
        SyntaxKind::DocBlock,
        SyntaxKind::OuterAttribute,
        SyntaxKind::Visibility,
        SyntaxKind::DeclarationPublicId,
        SyntaxKind::NameDefinition,
        SyntaxKind::SurfaceAlias,
        SyntaxKind::CharacterDisplayNameMember,
        SyntaxKind::LiteralExpression,
    ] {
        assert!(
            count_kind(&built, kind) >= 1,
            "canonical retained header omitted {kind:?}"
        );
    }
    assert!(built.diagnostics().is_empty(), "{:?}", built.diagnostics());
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn blank_logical_line_detaches_documentation_from_a_retained_header() {
    let attached = parse("/// attached\ncharacter Alice {}\n");
    let detached = parse("/// detached\n\ncharacter Alice {}\n");
    assert_eq!(count_kind(&attached, SyntaxKind::DocBlock), 1);
    assert_eq!(count_kind(&detached, SyntaxKind::DocBlock), 0);
    assert_eq!(
        count_kind(&detached, SyntaxKind::CharacterDeclarationItem),
        1
    );
}

#[test]
fn retained_headers_are_lossless_for_lf_crlf_and_unicode_identifiers() {
    let lf = "character 会話2 {\n    display_name = \"アリス\"\n}\n";
    let crlf = lf.replace('\n', "\r\n");
    for source in [lf, crlf.as_str()] {
        let built = parse(source);
        assert_eq!(count_kind(&built, SyntaxKind::CharacterDeclarationItem), 1);
        assert!(built.diagnostics().is_empty(), "{:?}", built.diagnostics());
        assert_eq!(built.green().to_string(), source);
    }
}

#[test]
fn every_retained_family_reports_zero_width_missing_name_without_stealing_a_sibling() {
    for fixture in FAMILY_FIXTURES {
        let source = format!(
            "{}proof tail() {{ assert.check(true) }}\n",
            fixture.missing_name
        );
        let built = parse(&source);
        let diagnostic = built
            .diagnostics()
            .iter()
            .find(|diagnostic| diagnostic.code() == "syntax.declaration.missing_name")
            .unwrap_or_else(|| panic!("missing diagnostic for {:?}", fixture.kind));
        assert_eq!(diagnostic.range().start(), diagnostic.range().end());
        assert_eq!(count_kind(&built, fixture.kind), 1);
        let missing_names = built
            .index()
            .entries()
            .iter()
            .filter(|entry| entry.kind() == SyntaxKind::MissingName)
            .collect::<Vec<_>>();
        assert_eq!(
            missing_names.len(),
            1,
            "unexpected MissingName owners for {:?}: {missing_names:?}",
            fixture.kind
        );
        assert_eq!(count_kind(&built, SyntaxKind::ProofItem), 1);
        assert_eq!(built.green().to_string(), source);
    }
}

#[test]
fn every_retained_family_reports_exact_wrong_family_id_and_preserves_a_sibling() {
    for fixture in FAMILY_FIXTURES {
        let source = format!(
            "{}proof tail() {{ assert.check(true) }}\n",
            fixture.wrong_family_id
        );
        let built = parse(&source);
        let diagnostic = built
            .diagnostics()
            .iter()
            .find(|diagnostic| diagnostic.code() == "syntax.declaration.wrong_family_id")
            .unwrap_or_else(|| panic!("missing wrong-family diagnostic for {:?}", fixture.kind));
        assert_eq!(
            diagnostic.range(),
            source_range(&source, fixture.wrong_id_text)
        );
        assert_eq!(count_kind(&built, SyntaxKind::WrongFamilyReference), 1);
        assert_eq!(count_kind(&built, fixture.kind), 1);
        assert_eq!(count_kind(&built, SyntaxKind::ProofItem), 1);
        assert_eq!(built.green().to_string(), source);
    }
}

#[test]
fn every_retained_family_normalizes_relative_id_and_preserves_a_sibling() {
    for fixture in FAMILY_FIXTURES {
        let source = format!(
            "{}proof tail() {{ assert.check(true) }}\n",
            fixture.relative_id
        );
        let built = parse(&source);
        assert!(
            built.diagnostics().is_empty(),
            "relative declaration identity should be accepted for {:?}: {:?}",
            fixture.kind,
            built.diagnostics()
        );
        assert_eq!(count_kind(&built, SyntaxKind::DeclarationPublicId), 1);
        assert_eq!(count_kind(&built, fixture.kind), 1);
        assert_eq!(count_kind(&built, SyntaxKind::ProofItem), 1);
        assert_eq!(built.green().to_string(), source);
    }
}

#[test]
fn every_retained_family_reports_exact_keyword_name_and_preserves_a_sibling() {
    for fixture in FAMILY_FIXTURES {
        let source = format!(
            "{}proof tail() {{ assert.check(true) }}\n",
            fixture.keyword_name
        );
        let built = parse(&source);
        let diagnostic = built
            .diagnostics()
            .iter()
            .find(|diagnostic| diagnostic.code() == "syntax.declaration.invalid_name")
            .unwrap_or_else(|| panic!("missing invalid-name diagnostic for {:?}", fixture.kind));
        assert_eq!(
            diagnostic.range(),
            source_range(&source, fixture.keyword_text)
        );
        let missing_names = built
            .index()
            .entries()
            .iter()
            .filter(|entry| entry.kind() == SyntaxKind::MissingName)
            .collect::<Vec<_>>();
        assert_eq!(
            missing_names.len(),
            1,
            "unexpected MissingName owners for {:?}: {missing_names:?}",
            fixture.kind
        );
        assert_eq!(count_kind(&built, fixture.kind), 1);
        assert_eq!(count_kind(&built, SyntaxKind::ProofItem), 1);
        assert_eq!(built.green().to_string(), source);
    }
}
