//! Behavioral tests for the reviewed policy contract.

use crate::JlreqStrictness;

use super::planner::{DpState, compare_candidate, normalize_metric};
use super::{
    MAX_VERTICAL_BREAK_CLUSTERS, VerticalBreakCluster, VerticalBreakError, VerticalBreakMetricRole,
    VerticalBreakPlan, VerticalBreakPlanStatus, VerticalBreakPolicy, VerticalBreakScore,
    plan_vertical_breaks,
};

#[test]
fn normal_keeps_leaders_together() {
    let clusters = [
        cluster("月"),
        cluster("火"),
        cluster("…"),
        cluster("…"),
        cluster("人"),
    ];
    let normal = plan(&clusters, 0.0, 0.0, 90.0, JlreqStrictness::Normal);

    assert!(!normal.breaks_before(3));
    assert!(normal.explain().rejected.jlreq_keep_together > 0);
}

#[test]
fn strictness_changes_closing_opening_paragraph_plan() {
    let clusters =
        ["天", "地", "。", "「", "人", "山", "川", "海"].map(|text| VerticalBreakCluster {
            text,
            advance: if text == "。" { 15.0 } else { 30.0 },
            break_allowed_before: true,
        });
    let loose = plan(&clusters, 0.0, 0.0, 105.0, JlreqStrictness::Loose);
    let strict = plan(&clusters, 0.0, 0.0, 105.0, JlreqStrictness::Strict);

    assert_eq!(loose.break_offsets(), &[3, 6]);
    assert_eq!(strict.break_offsets(), &[1, 5]);
}

#[test]
fn balanced_v1_loose_and_normal_choose_distinct_uax_permitted_plans() {
    let texts = [
        "天", "地", "。", "」", "人", "山", "川", "海", "。", "『", "火", "水", "木",
    ];
    let uax_break_allowed_before = [
        false, true, false, false, true, true, true, true, false, true, false, true, true,
    ];
    let clusters: [VerticalBreakCluster<'_>; 13] =
        std::array::from_fn(|index| VerticalBreakCluster {
            text: texts[index],
            advance: if matches!(texts[index], "。" | "」" | "『") {
                15.0
            } else {
                30.0
            },
            break_allowed_before: uax_break_allowed_before[index],
        });

    let loose = plan(&clusters, 0.0, 0.0, 125.6, JlreqStrictness::Loose);
    let normal = plan(&clusters, 0.0, 0.0, 125.6, JlreqStrictness::Normal);

    assert_eq!(loose.break_offsets(), &[5, 9]);
    assert_eq!(normal.break_offsets(), &[4, 7, 11]);
}

#[test]
fn uniform_scale_preserves_break_offsets() {
    let base = [
        cluster("春"),
        cluster("の"),
        cluster("雨"),
        cluster("、"),
        cluster("窓"),
        cluster("辺"),
    ];
    let expected = plan(&base, 10.0, 10.0, 95.0, JlreqStrictness::Normal)
        .break_offsets()
        .to_vec();
    for scale in [0.5_f32, 1.25, 2.0, 3.75] {
        let scaled = base.map(|cluster| VerticalBreakCluster {
            advance: cluster.advance * scale,
            ..cluster
        });
        assert_eq!(
            plan(
                &scaled,
                10.0 * scale,
                10.0 * scale,
                95.0 * scale,
                JlreqStrictness::Normal,
            )
            .break_offsets(),
            expected,
        );
    }
}

#[test]
fn hanging_can_beat_a_much_earlier_legal_break() {
    let clusters = [
        cluster("天"),
        cluster("地"),
        half_cluster("。"),
        cluster("人"),
    ];
    let selected = plan(&clusters, 0.0, 0.0, 70.0, JlreqStrictness::Normal);

    assert_eq!(selected.break_offsets(), &[3]);
    assert!(selected.explain().columns[0].used_hanging_units > 0);
    assert_eq!(selected.explain().columns[0].forced_overflow_units, 0);
}

#[test]
fn non_hanging_overflow_cannot_be_chosen_strategically() {
    let clusters = [
        cluster("天"),
        cluster("地"),
        half_cluster("。"),
        cluster("人"),
    ];
    let selected = plan(&clusters, 0.0, 0.0, 65.0, JlreqStrictness::Normal);

    assert_eq!(selected.break_offsets().first(), Some(&1));
    assert_eq!(selected.explain().score.forced_overflow_units, 0);
}

#[test]
fn terminal_unbreakable_fragment_uses_typed_forced_overflow_status() {
    let clusters = [VerticalBreakCluster {
        text: "長大語",
        advance: 90.0,
        break_allowed_before: false,
    }];
    let selected = plan(&clusters, 0.0, 0.0, 60.0, JlreqStrictness::Normal);

    assert_eq!(
        selected.explain().status,
        VerticalBreakPlanStatus::ForcedOverflow
    );
    assert!(selected.explain().score.forced_overflow_units > 0);
}

#[test]
fn jlreq_head_and_end_prohibitions_remove_edges() {
    let clusters = [
        cluster("天"),
        cluster("「"),
        cluster("人"),
        half_cluster("。"),
        cluster("山"),
    ];
    let selected = plan(&clusters, 0.0, 0.0, 60.0, JlreqStrictness::Normal);

    assert!(
        !selected.breaks_before(2),
        "opening punctuation cannot end a column"
    );
    assert!(
        !selected.breaks_before(3),
        "closing punctuation cannot head a column"
    );
    assert!(selected.explain().rejected.jlreq_line_end_prohibited > 0);
    assert!(selected.explain().rejected.jlreq_line_head_prohibited > 0);
}

#[test]
fn vertical_lr_and_rl_share_the_same_inline_break_plan() {
    let clusters = [cluster("天"), cluster("地"), cluster("春"), cluster("夏")];
    let selected = plan(&clusters, 10.0, 10.0, 60.0, JlreqStrictness::Normal);

    assert!(selected.breaks_before(2));
}

#[test]
fn styled_run_boundary_continues_the_current_column_when_content_fits() {
    let clusters = [cluster("2026")];
    let selected = plan(&clusters, 10.0, 70.0, 180.0, JlreqStrictness::Normal);

    assert!(!selected.breaks_before(0));
    assert!(!selected.explain().restarted_partial_column);
}

#[test]
fn styled_run_boundary_restarts_when_first_legal_fragment_does_not_fit() {
    let clusters = [cluster("春"), cluster("夏")];
    let selected = plan(&clusters, 10.0, 165.0, 180.0, JlreqStrictness::Normal);

    assert!(selected.breaks_before(0));
    assert!(selected.explain().restarted_partial_column);
}

#[test]
fn later_break_offsets_are_the_total_tie_break() {
    let score = VerticalBreakScore {
        soft_cost: 10,
        column_count: 2,
        ..VerticalBreakScore::default()
    };
    let mut states = vec![None; 4];
    states[0] = Some(DpState {
        score: VerticalBreakScore::default(),
        previous_break: 0,
        tie_break_used: false,
    });
    states[2] = Some(DpState {
        score,
        previous_break: 0,
        tie_break_used: false,
    });
    states[3] = Some(DpState {
        score,
        previous_break: 0,
        tie_break_used: false,
    });
    let current_same = Some(DpState {
        score,
        previous_break: 3,
        tie_break_used: false,
    });
    let comparison = compare_candidate(&states, current_same, score, 3).expect("comparison");
    assert!(
        !comparison.is_better,
        "identical path cannot replace itself"
    );
    let current_later = Some(DpState {
        score,
        previous_break: 3,
        tie_break_used: false,
    });
    let comparison = compare_candidate(&states, current_later, score, 2).expect("comparison");
    assert!(!comparison.is_better, "earlier break must lose");
    let current_earlier = Some(DpState {
        score,
        previous_break: 2,
        tie_break_used: false,
    });
    let comparison = compare_candidate(&states, current_earlier, score, 3).expect("comparison");
    assert!(comparison.is_better, "later break must win");
    assert!(comparison.used_tie_break);
}

#[test]
fn invalid_and_resource_boundary_inputs_return_typed_errors() {
    let invalid = [VerticalBreakCluster {
        text: "天",
        advance: f32::NAN,
        break_allowed_before: true,
    }];
    assert!(matches!(
        plan_vertical_breaks(
            &invalid,
            0.0,
            0.0,
            60.0,
            JlreqStrictness::Normal,
            VerticalBreakPolicy::BalancedV1,
        ),
        Err(VerticalBreakError::InvalidMetric {
            role: VerticalBreakMetricRole::ClusterAdvance,
            cluster_index: Some(0),
        })
    ));

    let oversized = vec![cluster("天"); MAX_VERTICAL_BREAK_CLUSTERS + 1];
    assert!(matches!(
        plan_vertical_breaks(
            &oversized,
            0.0,
            0.0,
            60.0,
            JlreqStrictness::Normal,
            VerticalBreakPolicy::BalancedV1,
        ),
        Err(VerticalBreakError::ResourceLimitExceeded { .. })
    ));
}

#[test]
fn zero_tiny_huge_and_cursor_boundary_inputs_are_typed() {
    let zero = [VerticalBreakCluster {
        text: "零",
        advance: 0.0,
        break_allowed_before: false,
    }];
    assert_eq!(
        plan_vertical_breaks(
            &zero,
            0.0,
            0.0,
            1.0,
            JlreqStrictness::Normal,
            VerticalBreakPolicy::BalancedV1,
        ),
        Err(VerticalBreakError::ZeroReferenceAdvance)
    );

    let tiny_advance = f32::from_bits(1);
    let tiny = [VerticalBreakCluster {
        text: "微",
        advance: tiny_advance,
        break_allowed_before: false,
    }];
    assert_eq!(
        plan_vertical_breaks(
            &tiny,
            0.0,
            0.0,
            tiny_advance,
            JlreqStrictness::Normal,
            VerticalBreakPolicy::BalancedV1,
        )
        .expect("subnormal reference remains representable")
        .explain()
        .status,
        VerticalBreakPlanStatus::Normal
    );

    let unit = [VerticalBreakCluster {
        text: "大",
        advance: 1.0,
        break_allowed_before: false,
    }];
    assert!(matches!(
        plan_vertical_breaks(
            &unit,
            0.0,
            0.0,
            f32::MAX,
            JlreqStrictness::Normal,
            VerticalBreakPolicy::BalancedV1,
        ),
        Err(VerticalBreakError::NormalizedMetricOutOfRange {
            role: VerticalBreakMetricRole::Height,
            ..
        })
    ));
    assert_eq!(
        plan_vertical_breaks(
            &unit,
            10.0,
            9.0,
            30.0,
            JlreqStrictness::Normal,
            VerticalBreakPolicy::BalancedV1,
        ),
        Err(VerticalBreakError::InitialCursorBeforeOrigin)
    );
}

#[test]
fn binary_normalization_uses_stable_half_up_rounding() {
    assert_eq!(
        normalize_metric(30.0, 30.0, VerticalBreakMetricRole::ClusterAdvance, None),
        Ok(4_096)
    );
    assert_eq!(
        normalize_metric(15.0, 30.0, VerticalBreakMetricRole::ClusterAdvance, None),
        Ok(2_048)
    );
    assert_eq!(
        normalize_metric(1.0, 8_192.0, VerticalBreakMetricRole::ClusterAdvance, None),
        Ok(1)
    );
}

fn plan(
    clusters: &[VerticalBreakCluster<'_>],
    origin_y: f32,
    initial_y: f32,
    height: f32,
    strictness: JlreqStrictness,
) -> VerticalBreakPlan {
    plan_vertical_breaks(
        clusters,
        origin_y,
        initial_y,
        height,
        strictness,
        VerticalBreakPolicy::BalancedV1,
    )
    .expect("test vertical plan")
}

const fn cluster(text: &str) -> VerticalBreakCluster<'_> {
    VerticalBreakCluster {
        text,
        advance: 30.0,
        break_allowed_before: true,
    }
}

const fn half_cluster(text: &str) -> VerticalBreakCluster<'_> {
    VerticalBreakCluster {
        text,
        advance: 15.0,
        break_allowed_before: true,
    }
}

#[test]
fn numeral_decimal_and_place_separators_preserve_one_unbreakable_unit() {
    for separator in [".", ",", " "] {
        let clusters = ["天", "12", separator, "345", "人"].map(cluster);
        for strictness in [
            JlreqStrictness::Loose,
            JlreqStrictness::Normal,
            JlreqStrictness::Strict,
        ] {
            let selected = plan(&clusters, 0.0, 0.0, 125.6, strictness);
            assert_eq!(selected.break_offsets(), &[4]);
            assert!(!selected.breaks_before(2));
            assert!(!selected.breaks_before(3));
            assert_eq!(selected.explain().rejected.jlreq_keep_together, 2);
            assert_eq!(selected.explain().status, VerticalBreakPlanStatus::Normal);
        }
    }
}

#[test]
fn ordinary_spaces_and_unpaired_numeric_separators_remain_breakable() {
    for texts in [
        ["天", " ", "人"],
        ["12", " ", "word"],
        ["word", " ", "12"],
        ["12", ",", "word"],
        ["word", ".", "12"],
    ] {
        let selected = plan(&texts.map(cluster), 0.0, 0.0, 60.0, JlreqStrictness::Normal);
        assert_eq!(selected.break_offsets(), &[2]);
        assert_eq!(selected.explain().rejected.jlreq_keep_together, 0);
        assert_eq!(selected.explain().status, VerticalBreakPlanStatus::Normal);
    }
}

#[test]
fn numeral_separator_unit_uses_the_existing_bounded_forced_overflow_escape() {
    let clusters = ["12", " ", "345"].map(cluster);
    let selected = plan(&clusters, 0.0, 0.0, 60.0, JlreqStrictness::Normal);
    assert!(selected.break_offsets().is_empty());
    assert_eq!(selected.explain().rejected.jlreq_keep_together, 2);
    assert_eq!(
        selected.explain().status,
        VerticalBreakPlanStatus::ForcedOverflow
    );
    assert_eq!(selected.explain().score.forced_overflow_columns, 1);
    assert!(selected.explain().score.forced_overflow_units > 0);
}

#[test]
fn repeated_numeral_separator_groups_are_admitted_without_breaking_the_unit() {
    let clusters = ["12", " ", "345", ",", "678", ".", "9"].map(cluster);
    let selected = plan(&clusters, 0.0, 0.0, 90.0, JlreqStrictness::Normal);
    assert!(selected.break_offsets().is_empty());
    assert_eq!(selected.explain().rejected.jlreq_keep_together, 6);
    assert_eq!(
        selected.explain().status,
        VerticalBreakPlanStatus::ForcedOverflow
    );
}

#[test]
fn measured_jlreq_paragraph_uses_balanced_quality_after_its_required_groups() {
    // Captured font30_000/line40_500 physical advances from the actual native
    // issued layout, at the standard View's Layout-consumer height125_600.
    // The same owned planner chooses optional boundaries; this test does not
    // reproduce its objective or infer container capacity from used bounds.
    let text = "天地春夏秋冬月火、山々人「川」あっいおーえ―中・外………終";
    let opportunities = crate::vertical_clusters::line_break_offsets(text);
    let source = text
        .char_indices()
        .map(|(start, character)| (start, start + character.len_utf8()))
        .collect::<Vec<_>>();
    let clusters = source
        .iter()
        .map(|&(start, end)| VerticalBreakCluster {
            text: &text[start..end],
            advance: if matches!(start, 24 | 42 | 69) {
                15.0
            } else {
                30.0
            },
            break_allowed_before: opportunities.contains(&start),
        })
        .collect::<Vec<_>>();
    let selected = plan_vertical_breaks(
        &clusters,
        0.0,
        0.0,
        125.6,
        JlreqStrictness::Normal,
        VerticalBreakPolicy::BalancedV1,
    )
    .expect("actual measured paragraph admits a balanced plan");
    assert_eq!(selected.break_offsets(), &[4, 7, 11, 15, 18, 20, 24, 28]);
    let offsets = std::iter::once(0)
        .chain(
            selected
                .break_offsets()
                .iter()
                .map(|&index| source[index].0),
        )
        .collect::<Vec<_>>();
    assert_eq!(offsets, [0, 12, 21, 33, 45, 54, 60, 72, 84]);
    let explain = selected.explain();
    assert_eq!(explain.policy, VerticalBreakPolicy::BalancedV1);
    assert_eq!(explain.reference_advance_bits, 30.0_f32.to_bits());
    assert_eq!(explain.score.forced_overflow_units, 0);
    assert_eq!(explain.score.forced_overflow_columns, 0);
    assert_eq!(explain.score.column_count, 9);
    assert_eq!(explain.status, VerticalBreakPlanStatus::Normal);
    for index in [10, 13, 14, 16, 19, 21, 23, 26, 27] {
        assert!(
            !selected.breaks_before(index),
            "required group split at {}",
            source[index].0
        );
    }
    assert!(
        !selected.breaks_before(22),
        "a dash does not require the following ideograph to restart"
    );
    assert!(
        selected.breaks_before(24),
        "the middle dot permits the following ordinary ideograph to restart"
    );
    assert!(
        selected.breaks_before(28),
        "the leader chain ends before the final ideograph"
    );
}
