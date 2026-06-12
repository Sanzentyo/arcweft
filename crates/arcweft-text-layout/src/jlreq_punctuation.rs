//! JLREQ punctuation classes and pair adjustment rules for vertical layout.

use crate::jlreq_punctuation_data::{JLREQ_PUNCTUATION_RANGES, JlreqPunctuationClass};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct JlreqPairAdjustment {
    pub(crate) keep_together: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct JlreqPairAdjustmentRule {
    left: Option<JlreqPunctuationClass>,
    right: JlreqPunctuationClass,
    adjustment: JlreqPairAdjustment,
}

const JLREQ_PAIR_ADJUSTMENTS: &[JlreqPairAdjustmentRule] = &[
    JlreqPairAdjustmentRule {
        left: None,
        right: JlreqPunctuationClass::RepeatMark,
        adjustment: JlreqPairAdjustment {
            keep_together: true,
        },
    },
    JlreqPairAdjustmentRule {
        left: Some(JlreqPunctuationClass::Dash),
        right: JlreqPunctuationClass::Dash,
        adjustment: JlreqPairAdjustment {
            keep_together: true,
        },
    },
    JlreqPairAdjustmentRule {
        left: Some(JlreqPunctuationClass::Leader),
        right: JlreqPunctuationClass::Leader,
        adjustment: JlreqPairAdjustment {
            keep_together: true,
        },
    },
];

pub(crate) fn is_line_end_prohibited_cluster(grapheme: &str) -> bool {
    cluster_class(grapheme) == Some(JlreqPunctuationClass::Opening)
}

pub(crate) fn is_line_head_prohibited_cluster(grapheme: &str) -> bool {
    matches!(
        cluster_class(grapheme),
        Some(
            JlreqPunctuationClass::Closing
                | JlreqPunctuationClass::SmallKana
                | JlreqPunctuationClass::Dash
                | JlreqPunctuationClass::MiddleDot
        )
    )
}

pub(crate) fn is_compressible_cluster(grapheme: &str) -> bool {
    matches!(
        cluster_class(grapheme),
        Some(JlreqPunctuationClass::Closing | JlreqPunctuationClass::MiddleDot)
    )
}

pub(crate) fn is_hanging_cluster(grapheme: &str) -> bool {
    is_compressible_cluster(grapheme)
}

pub(crate) fn pair_adjustment_for_clusters(left: &str, right: &str) -> JlreqPairAdjustment {
    let left = cluster_class(left);
    let Some(right) = cluster_class(right) else {
        return JlreqPairAdjustment::default();
    };
    JLREQ_PAIR_ADJUSTMENTS
        .iter()
        .find(|rule| {
            rule.right == right && rule.left.is_none_or(|rule_left| left == Some(rule_left))
        })
        .map_or_else(JlreqPairAdjustment::default, |rule| rule.adjustment)
}

fn cluster_class(grapheme: &str) -> Option<JlreqPunctuationClass> {
    grapheme.chars().next().and_then(char_class)
}

fn char_class(ch: char) -> Option<JlreqPunctuationClass> {
    let codepoint = ch as u32;
    let mut low = 0usize;
    let mut high = JLREQ_PUNCTUATION_RANGES.len();
    while low < high {
        let mid = low + (high - low) / 2;
        let range = JLREQ_PUNCTUATION_RANGES[mid];
        if codepoint < range.start {
            high = mid;
        } else if codepoint > range.end {
            low = mid + 1;
        } else {
            return Some(range.class);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jlreq_punctuation_data::JLREQ_PUNCTUATION_DATA_VERSION;

    #[test]
    fn records_seed_data_version() {
        assert_eq!(JLREQ_PUNCTUATION_DATA_VERSION, "arcweft-seed-2026-06-12");
    }

    #[test]
    fn range_table_is_sorted_and_non_overlapping() {
        for pair in JLREQ_PUNCTUATION_RANGES.windows(2) {
            assert!(
                pair[0].start <= pair[0].end,
                "range start must be <= end: {pair:?}"
            );
            assert!(
                pair[0].end < pair[1].start,
                "ranges must be sorted and non-overlapping: {pair:?}"
            );
        }
        assert!(
            JLREQ_PUNCTUATION_RANGES
                .last()
                .is_some_and(|range| range.start <= range.end),
            "last range start must be <= end"
        );
    }

    #[test]
    fn classifies_vertical_kinsoku_clusters() {
        assert!(is_line_head_prohibited_cluster("。"));
        assert!(is_line_head_prohibited_cluster("ぁ"));
        assert!(is_line_head_prohibited_cluster("ー"));
        assert!(is_line_head_prohibited_cluster("・"));
        assert!(is_line_end_prohibited_cluster("「"));
        assert!(!is_line_head_prohibited_cluster("「"));
        assert!(!is_line_end_prohibited_cluster("。"));
    }

    #[test]
    fn classifies_compression_and_hanging_clusters() {
        assert!(is_compressible_cluster("。"));
        assert!(is_compressible_cluster("・"));
        assert!(is_hanging_cluster("、"));
        assert!(!is_compressible_cluster("ー"));
        assert!(!is_hanging_cluster("ぁ"));
    }

    #[test]
    fn pair_table_keeps_jlreq_separation_pairs_together() {
        assert!(pair_adjustment_for_clusters("山", "々").keep_together);
        assert!(pair_adjustment_for_clusters("ー", "ー").keep_together);
        assert!(pair_adjustment_for_clusters("…", "…").keep_together);
        assert!(!pair_adjustment_for_clusters("。", "人").keep_together);
        assert!(!pair_adjustment_for_clusters("ー", "人").keep_together);
    }
}
