use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};

use super::document::parse_shadow_document;
use crate::grammar::build::{GrammarBuild, UnattachedGrammarEntry};
use crate::grammar::kinds::SyntaxKind;

fn document(source: &str) -> SourceDocument {
    SourceDocument::try_new(
        SourceDocumentId::try_new("memory:retained-metric").unwrap(),
        SourceName::Memory,
        source,
    )
    .unwrap()
}

fn parse(source: &str) -> GrammarBuild {
    parse_shadow_document(&document(source)).expect("Metric grammar builds")
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
fn canonical_metric_rows_own_kind_type_members_labels_and_buckets() {
    let source = concat!(
        "pub metric gauge @metric.frame_time frame_time: f32 {\n",
        "    unit = \"ms\"\n",
        "    labels {\n",
        "        scene: String\n",
        "        quality: RenderQuality\n",
        "    }\n",
        "}\n",
        "metric histogram latency: f64 {\n",
        "    buckets = [1.0, 2.0, 4.0]\n",
        "}\n",
    );
    let built = parse(source);
    assert_eq!(count_kind(&built, SyntaxKind::MetricDeclarationItem), 2);
    assert_eq!(count_kind(&built, SyntaxKind::MetricKind), 2);
    assert_eq!(count_kind(&built, SyntaxKind::MetricUnitMember), 1);
    assert_eq!(count_kind(&built, SyntaxKind::MetricLabelsBlock), 1);
    assert_eq!(count_kind(&built, SyntaxKind::MetricLabel), 2);
    assert_eq!(count_kind(&built, SyntaxKind::MetricBucketsMember), 1);
    assert!(built.diagnostics().is_empty(), "{:?}", built.diagnostics());
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn metric_member_recovery_is_typed_and_preserves_following_siblings() {
    let source = concat!(
        "metric mystery unknown: f32 {\n",
        "    labels {\n",
        "        scene: String\n",
        "        scene: bool\n",
        "    }\n",
        "    unit = milliseconds\n",
        "    extra = true\n",
        "    buckets = []\n",
        "    buckets = [1.0]\n",
        "}\n",
        "signal ready: Watch<bool>\n",
    );
    let built = parse(source);
    assert_eq!(count_kind(&built, SyntaxKind::MetricDeclarationItem), 1);
    assert_eq!(count_kind(&built, SyntaxKind::SignalDeclarationItem), 1);
    for code in [
        "syntax.metric.unknown_kind",
        "syntax.metric.duplicate_label",
        "syntax.metric.member_order",
        "syntax.metric.unit_not_string",
        "syntax.metric.unknown_member",
        "syntax.metric.empty_buckets",
        "syntax.metric.duplicate_member",
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
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn metric_missing_kind_type_and_body_have_zero_width_recovery() {
    let source = "metric @metric.frame frame\n";
    let built = parse(source);
    assert_eq!(count_kind(&built, SyntaxKind::MetricDeclarationItem), 1);
    assert!(count_kind(&built, SyntaxKind::MissingType) >= 1);
    assert!(count_kind(&built, SyntaxKind::MissingBody) >= 1);
    assert!(
        built
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code() == "syntax.metric.missing_kind")
    );
}
