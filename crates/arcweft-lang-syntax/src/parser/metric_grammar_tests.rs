use std::fmt::Write as _;

use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};

use super::document::parse_shadow_document;
use crate::grammar::build::{GrammarBuild, GrammarBuildError, UnattachedGrammarEntry};
use crate::grammar::kinds::{MetricKindSyntaxValue, SyntaxKind, SyntaxRole};
use crate::incremental::SyntaxLimit;

fn document(source: &str) -> SourceDocument {
    SourceDocument::try_new(
        SourceDocumentId::try_new("memory:retained-metric").unwrap(),
        SourceName::Memory,
        source,
    )
    .unwrap()
}

fn parse(source: &str) -> GrammarBuild {
    parse_shadow_document(&document(source), crate::parser::ParseOptions::default())
        .expect("Metric grammar builds")
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
        "pub metric gauge frame_time: f32 {\n",
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
    assert_eq!(count_kind(&built, SyntaxKind::EqualsNode), 2);
    assert_eq!(count_kind(&built, SyntaxKind::ColonNode), 4);
    assert!(built.diagnostics().is_empty(), "{:?}", built.diagnostics());
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn metric_kind_role_is_closed_for_known_values_and_recovery_for_other_shapes() {
    let built = parse(concat!(
        "metric counter Count: u64 {}\n",
        "metric gauge Current: f32 {}\n",
        "metric histogram Latency: f64 {}\n",
        "metric mystery Unknown: f64 {}\n",
        "metric @metric.Missing Missing: f64 {}\n",
    ));
    let roles = built
        .index()
        .entries()
        .iter()
        .filter(|entry| entry.kind() == SyntaxKind::MetricKind)
        .map(UnattachedGrammarEntry::role)
        .collect::<Vec<_>>();
    assert_eq!(
        roles,
        [
            SyntaxRole::MetricKindValue(MetricKindSyntaxValue::Counter),
            SyntaxRole::MetricKindValue(MetricKindSyntaxValue::Gauge),
            SyntaxRole::MetricKindValue(MetricKindSyntaxValue::Histogram),
            SyntaxRole::Kind,
            SyntaxRole::Kind,
        ]
    );
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
    assert_eq!(count_kind(&built, SyntaxKind::ColonNode), 1);
    assert!(
        built
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code() == "syntax.metric.missing_kind")
    );
}

#[test]
fn metric_declaration_member_limit_counts_recovery_entries_transactionally() {
    let exact = metric_with_unknown_entries(SyntaxLimit::DeclarationMembers.maximum());
    let built = parse_shadow_document(&document(&exact), crate::parser::ParseOptions::default())
        .expect("exact Metric declaration-member limit builds");
    assert_eq!(
        count_kind(&built, SyntaxKind::ErrorDeclarationMember),
        SyntaxLimit::DeclarationMembers.maximum()
    );

    let one_over = metric_with_unknown_entries(SyntaxLimit::DeclarationMembers.maximum() + 1);
    assert!(matches!(
        parse_shadow_document(&document(&one_over), crate::parser::ParseOptions::default()),
        Err(GrammarBuildError::LimitExceeded(
            SyntaxLimit::DeclarationMembers
        ))
    ));
    assert!(
        parse_shadow_document(
            &document("metric counter Retry: u64 {}\n"),
            crate::parser::ParseOptions::default()
        )
        .is_ok()
    );
}

#[test]
fn metric_labels_also_consume_the_shared_declaration_member_budget() {
    let exact = metric_with_units_and_labels(959, 64);
    assert!(
        parse_shadow_document(&document(&exact), crate::parser::ParseOptions::default()).is_ok()
    );

    let one_over = metric_with_units_and_labels(960, 64);
    assert!(matches!(
        parse_shadow_document(&document(&one_over), crate::parser::ParseOptions::default()),
        Err(GrammarBuildError::LimitExceeded(
            SyntaxLimit::DeclarationMembers
        ))
    ));
    assert!(
        parse_shadow_document(
            &document("metric gauge Retry: f32 { labels { ready: bool } }\n"),
            crate::parser::ParseOptions::default()
        )
        .is_ok()
    );
}

fn metric_with_unknown_entries(count: usize) -> String {
    let mut source = String::from("metric counter Many: u64 {\n");
    for ordinal in 0..count {
        writeln!(source, "    unknown_{ordinal}").expect("String writes cannot fail");
    }
    source.push_str("}\n");
    source
}

fn metric_with_units_and_labels(units: usize, labels: usize) -> String {
    let mut source = String::from("metric gauge Many: f32 {\n");
    for _ in 0..units {
        source.push_str("    unit = \"ms\"\n");
    }
    source.push_str("    labels {\n");
    for ordinal in 0..labels {
        writeln!(source, "        label_{ordinal}: u32").expect("String writes cannot fail");
    }
    source.push_str("    }\n}\n");
    source
}

#[test]
fn duplicate_unit_retains_both_typed_members_and_related_evidence() {
    let source = concat!(
        "metric gauge DuplicateUnit: f32 {\n",
        "    unit = \"ms\"\n",
        "    unit = \"s\"\n",
        "}\n",
    );
    let built = parse(source);
    let diagnostic = built
        .diagnostics()
        .iter()
        .find(|diagnostic| diagnostic.code() == "syntax.metric.duplicate_member")
        .expect("duplicate unit diagnostic");
    let first = source.find("unit").expect("first unit");
    let second = source[first + "unit".len()..]
        .find("unit")
        .map(|relative| first + "unit".len() + relative)
        .expect("second unit");
    assert_eq!(
        diagnostic.range(),
        arcweft_source::SourceRange::new(second, second + "unit".len())
    );
    assert_eq!(
        diagnostic.related_range(),
        Some(arcweft_source::SourceRange::new(
            first,
            first + "unit".len()
        ))
    );
    assert_eq!(count_kind(&built, SyntaxKind::MetricUnitMember), 2);
    assert_eq!(built.green().to_string(), source);
}
