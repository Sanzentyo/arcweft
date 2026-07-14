use std::collections::BTreeMap;

use arcweft_text_layout::{
    JlreqStrictness, TextLayoutRequest, VerticalBreakCluster, VerticalBreakPlan,
    VerticalBreakPlanStatus, VerticalBreakPolicy, plan_vertical_breaks,
};
use serde::Deserialize;

const CORPUS: &str = include_str!("fixtures/vertical_break_quality/v1/manifest.json");

#[derive(Debug, Deserialize)]
struct Corpus {
    schema_id: String,
    #[serde(rename = "corpus_version")]
    version: u32,
    policy_id: String,
    license: String,
    provenance: String,
    approval_status: String,
    metric_fixtures: Vec<MetricFixture>,
    review_thresholds: ReviewThresholds,
    cases: Vec<Case>,
}

#[derive(Debug, Deserialize)]
struct MetricFixture {
    id: String,
    units_per_em: u16,
    description: String,
}

#[derive(Debug, Deserialize)]
struct ReviewThresholds {
    hard_invariant_regressions_allowed: u32,
    preferred_break_regressions_allowed_without_owner_review: u32,
    acceptable_break_regressions_allowed_without_owner_review: u32,
    new_forced_overflow_cases_allowed_without_owner_review: u32,
}

#[derive(Debug, Deserialize)]
struct Case {
    id: String,
    #[serde(default)]
    plan_group: Option<String>,
    source: String,
    writing_mode: String,
    strictness: String,
    metric_fixture: String,
    origin_units: u16,
    initial_units: u16,
    height_units: u16,
    clusters: Vec<Cluster>,
    preferred_break_offsets: Vec<usize>,
    acceptable_break_offsets: Vec<Vec<usize>>,
    expected_status: String,
    tags: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct Cluster {
    text: String,
    advance_units: u16,
    break_allowed_before: bool,
}

#[test]
fn reviewed_vertical_break_corpus_matches_balanced_v1_at_multiple_scales() {
    let corpus: Corpus = serde_json::from_str(CORPUS).expect("valid vertical-break corpus");
    assert_corpus_contract(&corpus);

    let fixtures = corpus
        .metric_fixtures
        .iter()
        .map(|fixture| (fixture.id.as_str(), fixture))
        .collect::<BTreeMap<_, _>>();
    let mut parity_groups = BTreeMap::<String, Vec<usize>>::new();

    for case in &corpus.cases {
        let fixture = fixtures
            .get(case.metric_fixture.as_str())
            .unwrap_or_else(|| panic!("{} references unknown metric fixture", case.id));
        evaluate_case(case, fixture);
        record_plan_group(&mut parity_groups, case);
    }
}

fn assert_corpus_contract(corpus: &Corpus) {
    assert_eq!(corpus.schema_id, "arcweft.vertical-break-quality-corpus.v1");
    assert_eq!(corpus.version, 1);
    assert_eq!(
        corpus.policy_id,
        VerticalBreakPolicy::BalancedV1.stable_id()
    );
    assert_eq!(corpus.license, "CC0-1.0");
    assert!(!corpus.provenance.is_empty());
    assert_eq!(corpus.approval_status, "owner_approved");
    assert_eq!(
        corpus.review_thresholds.hard_invariant_regressions_allowed,
        0
    );
    assert_eq!(
        corpus
            .review_thresholds
            .preferred_break_regressions_allowed_without_owner_review,
        0
    );
    assert_eq!(
        corpus
            .review_thresholds
            .acceptable_break_regressions_allowed_without_owner_review,
        0
    );
    assert_eq!(
        corpus
            .review_thresholds
            .new_forced_overflow_cases_allowed_without_owner_review,
        0
    );
}

fn evaluate_case(case: &Case, fixture: &MetricFixture) {
    assert!(!case.id.is_empty());
    assert!(!case.source.is_empty());
    assert!(!case.tags.is_empty());
    assert!(matches!(
        case.writing_mode.as_str(),
        "vertical_rl" | "vertical_lr"
    ));
    assert!(fixture.units_per_em > 0);
    assert!(!fixture.description.is_empty());
    let strictness = strictness(&case.strictness);
    let mut reference_breaks: Option<Vec<usize>> = None;

    for scale in [0.5_f32, 1.0, 2.0, 4.0] {
        let pixels_per_unit = 30.0 * scale / f32::from(fixture.units_per_em);
        let clusters = case
            .clusters
            .iter()
            .map(|cluster| VerticalBreakCluster {
                text: &cluster.text,
                advance: f32::from(cluster.advance_units) * pixels_per_unit,
                break_allowed_before: cluster.break_allowed_before,
            })
            .collect::<Vec<_>>();
        let plan = plan_vertical_breaks(
            &clusters,
            f32::from(case.origin_units) * pixels_per_unit,
            f32::from(case.initial_units) * pixels_per_unit,
            f32::from(case.height_units) * pixels_per_unit,
            strictness,
            VerticalBreakPolicy::BalancedV1,
        )
        .unwrap_or_else(|error| panic!("{} failed at scale {scale}: {error}", case.id));

        assert_case_plan(case, scale, &plan);
        if let Some(expected) = &reference_breaks {
            assert_eq!(
                plan.break_offsets(),
                expected.as_slice(),
                "{} lost scale invariance",
                case.id
            );
        } else {
            reference_breaks = Some(plan.break_offsets().to_vec());
        }
    }
}

fn assert_case_plan(case: &Case, scale: f32, plan: &VerticalBreakPlan) {
    assert_eq!(
        plan.break_offsets(),
        case.preferred_break_offsets.as_slice(),
        "{} preferred break drift at scale {scale}",
        case.id
    );
    assert!(
        case.acceptable_break_offsets
            .iter()
            .any(|accepted| accepted.as_slice() == plan.break_offsets()),
        "{} selected an unreviewed break at scale {scale}",
        case.id
    );
    assert_eq!(plan.explain().policy, VerticalBreakPolicy::BalancedV1);
    match case.expected_status.as_str() {
        "normal" => {
            assert_eq!(plan.explain().status, VerticalBreakPlanStatus::Normal);
            assert_eq!(plan.explain().score.forced_overflow_units, 0);
        }
        "forced_overflow" => {
            assert_eq!(
                plan.explain().status,
                VerticalBreakPlanStatus::ForcedOverflow
            );
            assert!(plan.explain().score.forced_overflow_units > 0);
        }
        other => panic!("{} has unknown expected status {other}", case.id),
    }
    if case.tags.iter().any(|tag| tag == "restart") {
        assert!(plan.explain().restarted_partial_column);
        assert_eq!(plan.break_offsets().first(), Some(&0));
    }
}

fn record_plan_group(parity_groups: &mut BTreeMap<String, Vec<usize>>, case: &Case) {
    let Some(group) = &case.plan_group else {
        return;
    };
    match parity_groups.get(group) {
        Some(expected) => assert_eq!(
            expected, &case.preferred_break_offsets,
            "{} differs from shared direction plan group {group}",
            case.id
        ),
        None => {
            parity_groups.insert(group.clone(), case.preferred_break_offsets.clone());
        }
    }
}

#[test]
fn closed_policy_id_round_trips_and_unknown_versions_are_rejected() {
    let encoded = serde_json::to_string(&VerticalBreakPolicy::BalancedV1)
        .expect("policy serialization succeeds");
    assert_eq!(encoded, "\"balanced_v1\"");
    let decoded: VerticalBreakPolicy =
        serde_json::from_str(&encoded).expect("policy round trip succeeds");
    assert_eq!(decoded, VerticalBreakPolicy::BalancedV1);
    assert!(serde_json::from_str::<VerticalBreakPolicy>("\"future_v2\"").is_err());
    assert!(serde_json::from_str::<VerticalBreakPolicy>("{}").is_err());
}

#[test]
fn layout_request_requires_one_well_formed_closed_policy() {
    let encoded =
        serde_json::to_value(TextLayoutRequest::default()).expect("layout request serializes");
    assert_eq!(encoded["vertical_break_policy"], "balanced_v1");

    let mut missing = encoded.clone();
    missing
        .as_object_mut()
        .expect("request is an object")
        .remove("vertical_break_policy");
    assert!(serde_json::from_value::<TextLayoutRequest>(missing).is_err());

    for malformed in [
        serde_json::Value::Null,
        serde_json::json!({}),
        serde_json::json!("future_v2"),
    ] {
        let mut request = encoded.clone();
        request["vertical_break_policy"] = malformed;
        assert!(serde_json::from_value::<TextLayoutRequest>(request).is_err());
    }
}

fn strictness(value: &str) -> JlreqStrictness {
    match value {
        "loose" => JlreqStrictness::Loose,
        "normal" => JlreqStrictness::Normal,
        "strict" => JlreqStrictness::Strict,
        other => panic!("unknown corpus strictness {other}"),
    }
}
