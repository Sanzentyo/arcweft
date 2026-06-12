//! JLREQ punctuation classes and pair adjustment rules for vertical layout.

/// Checked-in JLREQ punctuation seed data version.
///
/// This is not a Unicode data version. It identifies Arcweft's current
/// hand-curated seed table, which is shaped so a generated table can replace the
/// range data without changing layout call sites.
pub const JLREQ_PUNCTUATION_DATA_VERSION: &str = "arcweft-seed-2026-06-12";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum JlreqPunctuationClass {
    Closing,
    Opening,
    SmallKana,
    Dash,
    Leader,
    MiddleDot,
    RepeatMark,
}

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct JlreqPunctuationRange {
    start: u32,
    end: u32,
    class: JlreqPunctuationClass,
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

const JLREQ_PUNCTUATION_RANGES: &[JlreqPunctuationRange] = &[
    JlreqPunctuationRange {
        start: 0x0028,
        end: 0x0028,
        class: JlreqPunctuationClass::Opening,
    },
    JlreqPunctuationRange {
        start: 0x0029,
        end: 0x0029,
        class: JlreqPunctuationClass::Closing,
    },
    JlreqPunctuationRange {
        start: 0x005B,
        end: 0x005B,
        class: JlreqPunctuationClass::Opening,
    },
    JlreqPunctuationRange {
        start: 0x005D,
        end: 0x005D,
        class: JlreqPunctuationClass::Closing,
    },
    JlreqPunctuationRange {
        start: 0x007B,
        end: 0x007B,
        class: JlreqPunctuationClass::Opening,
    },
    JlreqPunctuationRange {
        start: 0x007D,
        end: 0x007D,
        class: JlreqPunctuationClass::Closing,
    },
    JlreqPunctuationRange {
        start: 0x00B7,
        end: 0x00B7,
        class: JlreqPunctuationClass::MiddleDot,
    },
    JlreqPunctuationRange {
        start: 0x2014,
        end: 0x2015,
        class: JlreqPunctuationClass::Dash,
    },
    JlreqPunctuationRange {
        start: 0x2018,
        end: 0x2018,
        class: JlreqPunctuationClass::Opening,
    },
    JlreqPunctuationRange {
        start: 0x2019,
        end: 0x2019,
        class: JlreqPunctuationClass::Closing,
    },
    JlreqPunctuationRange {
        start: 0x201C,
        end: 0x201C,
        class: JlreqPunctuationClass::Opening,
    },
    JlreqPunctuationRange {
        start: 0x201D,
        end: 0x201D,
        class: JlreqPunctuationClass::Closing,
    },
    JlreqPunctuationRange {
        start: 0x2025,
        end: 0x2026,
        class: JlreqPunctuationClass::Leader,
    },
    JlreqPunctuationRange {
        start: 0x2500,
        end: 0x2500,
        class: JlreqPunctuationClass::Dash,
    },
    JlreqPunctuationRange {
        start: 0x3001,
        end: 0x3002,
        class: JlreqPunctuationClass::Closing,
    },
    JlreqPunctuationRange {
        start: 0x3005,
        end: 0x3005,
        class: JlreqPunctuationClass::RepeatMark,
    },
    JlreqPunctuationRange {
        start: 0x3008,
        end: 0x3008,
        class: JlreqPunctuationClass::Opening,
    },
    JlreqPunctuationRange {
        start: 0x3009,
        end: 0x3009,
        class: JlreqPunctuationClass::Closing,
    },
    JlreqPunctuationRange {
        start: 0x300A,
        end: 0x300A,
        class: JlreqPunctuationClass::Opening,
    },
    JlreqPunctuationRange {
        start: 0x300B,
        end: 0x300B,
        class: JlreqPunctuationClass::Closing,
    },
    JlreqPunctuationRange {
        start: 0x300C,
        end: 0x300C,
        class: JlreqPunctuationClass::Opening,
    },
    JlreqPunctuationRange {
        start: 0x300D,
        end: 0x300D,
        class: JlreqPunctuationClass::Closing,
    },
    JlreqPunctuationRange {
        start: 0x300E,
        end: 0x300E,
        class: JlreqPunctuationClass::Opening,
    },
    JlreqPunctuationRange {
        start: 0x300F,
        end: 0x300F,
        class: JlreqPunctuationClass::Closing,
    },
    JlreqPunctuationRange {
        start: 0x3010,
        end: 0x3010,
        class: JlreqPunctuationClass::Opening,
    },
    JlreqPunctuationRange {
        start: 0x3011,
        end: 0x3011,
        class: JlreqPunctuationClass::Closing,
    },
    JlreqPunctuationRange {
        start: 0x3014,
        end: 0x3014,
        class: JlreqPunctuationClass::Opening,
    },
    JlreqPunctuationRange {
        start: 0x3015,
        end: 0x3015,
        class: JlreqPunctuationClass::Closing,
    },
    JlreqPunctuationRange {
        start: 0x3016,
        end: 0x3016,
        class: JlreqPunctuationClass::Opening,
    },
    JlreqPunctuationRange {
        start: 0x3017,
        end: 0x3017,
        class: JlreqPunctuationClass::Closing,
    },
    JlreqPunctuationRange {
        start: 0x3018,
        end: 0x3018,
        class: JlreqPunctuationClass::Opening,
    },
    JlreqPunctuationRange {
        start: 0x3019,
        end: 0x3019,
        class: JlreqPunctuationClass::Closing,
    },
    JlreqPunctuationRange {
        start: 0x301A,
        end: 0x301A,
        class: JlreqPunctuationClass::Opening,
    },
    JlreqPunctuationRange {
        start: 0x301B,
        end: 0x301B,
        class: JlreqPunctuationClass::Closing,
    },
    JlreqPunctuationRange {
        start: 0x301D,
        end: 0x301D,
        class: JlreqPunctuationClass::Opening,
    },
    JlreqPunctuationRange {
        start: 0x301E,
        end: 0x301F,
        class: JlreqPunctuationClass::Closing,
    },
    JlreqPunctuationRange {
        start: 0x3031,
        end: 0x3035,
        class: JlreqPunctuationClass::RepeatMark,
    },
    JlreqPunctuationRange {
        start: 0x303B,
        end: 0x303B,
        class: JlreqPunctuationClass::RepeatMark,
    },
    JlreqPunctuationRange {
        start: 0x3041,
        end: 0x3041,
        class: JlreqPunctuationClass::SmallKana,
    },
    JlreqPunctuationRange {
        start: 0x3043,
        end: 0x3043,
        class: JlreqPunctuationClass::SmallKana,
    },
    JlreqPunctuationRange {
        start: 0x3045,
        end: 0x3045,
        class: JlreqPunctuationClass::SmallKana,
    },
    JlreqPunctuationRange {
        start: 0x3047,
        end: 0x3047,
        class: JlreqPunctuationClass::SmallKana,
    },
    JlreqPunctuationRange {
        start: 0x3049,
        end: 0x3049,
        class: JlreqPunctuationClass::SmallKana,
    },
    JlreqPunctuationRange {
        start: 0x3063,
        end: 0x3063,
        class: JlreqPunctuationClass::SmallKana,
    },
    JlreqPunctuationRange {
        start: 0x3083,
        end: 0x3083,
        class: JlreqPunctuationClass::SmallKana,
    },
    JlreqPunctuationRange {
        start: 0x3085,
        end: 0x3085,
        class: JlreqPunctuationClass::SmallKana,
    },
    JlreqPunctuationRange {
        start: 0x3087,
        end: 0x3087,
        class: JlreqPunctuationClass::SmallKana,
    },
    JlreqPunctuationRange {
        start: 0x308E,
        end: 0x308E,
        class: JlreqPunctuationClass::SmallKana,
    },
    JlreqPunctuationRange {
        start: 0x3095,
        end: 0x3096,
        class: JlreqPunctuationClass::SmallKana,
    },
    JlreqPunctuationRange {
        start: 0x309D,
        end: 0x309E,
        class: JlreqPunctuationClass::RepeatMark,
    },
    JlreqPunctuationRange {
        start: 0x30A1,
        end: 0x30A1,
        class: JlreqPunctuationClass::SmallKana,
    },
    JlreqPunctuationRange {
        start: 0x30A3,
        end: 0x30A3,
        class: JlreqPunctuationClass::SmallKana,
    },
    JlreqPunctuationRange {
        start: 0x30A5,
        end: 0x30A5,
        class: JlreqPunctuationClass::SmallKana,
    },
    JlreqPunctuationRange {
        start: 0x30A7,
        end: 0x30A7,
        class: JlreqPunctuationClass::SmallKana,
    },
    JlreqPunctuationRange {
        start: 0x30A9,
        end: 0x30A9,
        class: JlreqPunctuationClass::SmallKana,
    },
    JlreqPunctuationRange {
        start: 0x30C3,
        end: 0x30C3,
        class: JlreqPunctuationClass::SmallKana,
    },
    JlreqPunctuationRange {
        start: 0x30E3,
        end: 0x30E3,
        class: JlreqPunctuationClass::SmallKana,
    },
    JlreqPunctuationRange {
        start: 0x30E5,
        end: 0x30E5,
        class: JlreqPunctuationClass::SmallKana,
    },
    JlreqPunctuationRange {
        start: 0x30E7,
        end: 0x30E7,
        class: JlreqPunctuationClass::SmallKana,
    },
    JlreqPunctuationRange {
        start: 0x30EE,
        end: 0x30EE,
        class: JlreqPunctuationClass::SmallKana,
    },
    JlreqPunctuationRange {
        start: 0x30F5,
        end: 0x30F6,
        class: JlreqPunctuationClass::SmallKana,
    },
    JlreqPunctuationRange {
        start: 0x30FB,
        end: 0x30FB,
        class: JlreqPunctuationClass::MiddleDot,
    },
    JlreqPunctuationRange {
        start: 0x30FC,
        end: 0x30FC,
        class: JlreqPunctuationClass::Dash,
    },
    JlreqPunctuationRange {
        start: 0x30FD,
        end: 0x30FE,
        class: JlreqPunctuationClass::RepeatMark,
    },
    JlreqPunctuationRange {
        start: 0x31F0,
        end: 0x31FF,
        class: JlreqPunctuationClass::SmallKana,
    },
    JlreqPunctuationRange {
        start: 0xFF08,
        end: 0xFF08,
        class: JlreqPunctuationClass::Opening,
    },
    JlreqPunctuationRange {
        start: 0xFF09,
        end: 0xFF09,
        class: JlreqPunctuationClass::Closing,
    },
    JlreqPunctuationRange {
        start: 0xFF0C,
        end: 0xFF0C,
        class: JlreqPunctuationClass::Closing,
    },
    JlreqPunctuationRange {
        start: 0xFF0E,
        end: 0xFF0E,
        class: JlreqPunctuationClass::Closing,
    },
    JlreqPunctuationRange {
        start: 0xFF3B,
        end: 0xFF3B,
        class: JlreqPunctuationClass::Opening,
    },
    JlreqPunctuationRange {
        start: 0xFF3D,
        end: 0xFF3D,
        class: JlreqPunctuationClass::Closing,
    },
    JlreqPunctuationRange {
        start: 0xFF5B,
        end: 0xFF5B,
        class: JlreqPunctuationClass::Opening,
    },
    JlreqPunctuationRange {
        start: 0xFF5D,
        end: 0xFF5D,
        class: JlreqPunctuationClass::Closing,
    },
    JlreqPunctuationRange {
        start: 0xFF5F,
        end: 0xFF5F,
        class: JlreqPunctuationClass::Opening,
    },
    JlreqPunctuationRange {
        start: 0xFF60,
        end: 0xFF60,
        class: JlreqPunctuationClass::Closing,
    },
    JlreqPunctuationRange {
        start: 0xFF65,
        end: 0xFF65,
        class: JlreqPunctuationClass::MiddleDot,
    },
];

#[cfg(test)]
mod tests {
    use super::*;

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
