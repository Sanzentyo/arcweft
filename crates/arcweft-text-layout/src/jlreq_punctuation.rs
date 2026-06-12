//! JLREQ punctuation classes and pair adjustment rules for vertical layout.

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

const fn char_class(ch: char) -> Option<JlreqPunctuationClass> {
    if is_jlreq_closing_punctuation_char(ch) {
        Some(JlreqPunctuationClass::Closing)
    } else if is_jlreq_line_end_prohibited_char(ch) {
        Some(JlreqPunctuationClass::Opening)
    } else if is_jlreq_small_kana_char(ch) {
        Some(JlreqPunctuationClass::SmallKana)
    } else if is_jlreq_dash_char(ch) {
        Some(JlreqPunctuationClass::Dash)
    } else if is_jlreq_leader_char(ch) {
        Some(JlreqPunctuationClass::Leader)
    } else if is_jlreq_middle_dot_char(ch) {
        Some(JlreqPunctuationClass::MiddleDot)
    } else if is_jlreq_repeat_mark_char(ch) {
        Some(JlreqPunctuationClass::RepeatMark)
    } else {
        None
    }
}

const fn is_jlreq_repeat_mark_char(ch: char) -> bool {
    matches!(
        ch,
        '\u{3005}' | '\u{303b}' | '\u{3031}'
            ..='\u{3035}' | '\u{309d}' | '\u{309e}' | '\u{30fd}' | '\u{30fe}'
    )
}

const fn is_jlreq_dash_char(ch: char) -> bool {
    matches!(ch, '\u{2014}' | '\u{2015}' | '\u{2500}' | '\u{30fc}')
}

const fn is_jlreq_leader_char(ch: char) -> bool {
    matches!(ch, '\u{2025}' | '\u{2026}')
}

const fn is_jlreq_middle_dot_char(ch: char) -> bool {
    matches!(ch, '\u{00b7}' | '\u{30fb}' | '\u{ff65}')
}

const fn is_jlreq_closing_punctuation_char(ch: char) -> bool {
    matches!(
        ch,
        ')' | ']'
            | '}'
            | '\u{2019}'
            | '\u{201d}'
            | '\u{3001}'
            | '\u{3002}'
            | '\u{3009}'
            | '\u{300b}'
            | '\u{300d}'
            | '\u{300f}'
            | '\u{3011}'
            | '\u{3015}'
            | '\u{3017}'
            | '\u{3019}'
            | '\u{301b}'
            | '\u{301e}'
            | '\u{301f}'
            | '\u{ff09}'
            | '\u{ff0c}'
            | '\u{ff0e}'
            | '\u{ff3d}'
            | '\u{ff5d}'
            | '\u{ff60}'
    )
}

const fn is_jlreq_small_kana_char(ch: char) -> bool {
    matches!(
        ch,
        '\u{3041}'
            | '\u{3043}'
            | '\u{3045}'
            | '\u{3047}'
            | '\u{3049}'
            | '\u{3063}'
            | '\u{3083}'
            | '\u{3085}'
            | '\u{3087}'
            | '\u{308e}'
            | '\u{3095}'
            | '\u{3096}'
            | '\u{30a1}'
            | '\u{30a3}'
            | '\u{30a5}'
            | '\u{30a7}'
            | '\u{30a9}'
            | '\u{30c3}'
            | '\u{30e3}'
            | '\u{30e5}'
            | '\u{30e7}'
            | '\u{30ee}'
            | '\u{30f5}'
            | '\u{30f6}'
            | '\u{31f0}'..='\u{31ff}'
    )
}

const fn is_jlreq_line_end_prohibited_char(ch: char) -> bool {
    matches!(
        ch,
        '(' | '['
            | '{'
            | '\u{2018}'
            | '\u{201c}'
            | '\u{3008}'
            | '\u{300a}'
            | '\u{300c}'
            | '\u{300e}'
            | '\u{3010}'
            | '\u{3014}'
            | '\u{3016}'
            | '\u{3018}'
            | '\u{301a}'
            | '\u{301d}'
            | '\u{ff08}'
            | '\u{ff3b}'
            | '\u{ff5b}'
            | '\u{ff5f}'
    )
}

#[cfg(test)]
mod tests {
    use super::*;

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
