//! JLREQ and Unicode constraints used to validate vertical column breaks.

use crate::{
    JlreqStrictness, TextLayoutConfig, jlreq_punctuation,
    vertical::vertical_cluster_advance,
    vertical_clusters::{
        VerticalCluster, is_latin_or_greek_alphabetic_cluster_text, is_vertical_line_break_cluster,
    },
};

pub(crate) fn vertical_column_segment_overhang_allowance(
    clusters: &[VerticalCluster],
    column_start: usize,
    column_end: usize,
    config: TextLayoutConfig,
) -> f32 {
    let Some(last_cluster_index) = (column_start..column_end)
        .rfind(|index| !is_vertical_line_break_cluster(&clusters[*index].text))
    else {
        return 0.0;
    };

    let mut suffix_start = last_cluster_index + 1;
    for cluster_index in (column_start.saturating_add(1)..=last_cluster_index).rev() {
        if vertical_cluster_requires_previous_as_latin_word_or_unit(cluster_index, clusters) {
            return 0.0;
        }
        if vertical_cluster_requires_previous_in_column(cluster_index, clusters, config) {
            suffix_start = cluster_index;
        } else {
            break;
        }
    }
    if suffix_start > last_cluster_index {
        return 0.0;
    }

    clusters[suffix_start..=last_cluster_index]
        .iter()
        .filter(|cluster| !is_vertical_line_break_cluster(&cluster.text))
        .map(|cluster| vertical_cluster_advance(cluster, config))
        .sum()
}

pub(crate) fn vertical_column_segment_overhang_uses_linebreak_continuation(
    clusters: &[VerticalCluster],
    column_start: usize,
    column_end: usize,
    config: TextLayoutConfig,
) -> bool {
    let Some(last_cluster_index) = (column_start..column_end)
        .rfind(|index| !is_vertical_line_break_cluster(&clusters[*index].text))
    else {
        return false;
    };

    for cluster_index in (column_start.saturating_add(1)..=last_cluster_index).rev() {
        if !vertical_cluster_requires_previous_in_column(cluster_index, clusters, config) {
            return false;
        }
        if vertical_cluster_requires_previous_by_linebreak_only(cluster_index, clusters, config) {
            return true;
        }
    }
    false
}

fn vertical_cluster_requires_previous_in_column(
    cluster_index: usize,
    clusters: &[VerticalCluster],
    config: TextLayoutConfig,
) -> bool {
    let Some(cluster) = clusters.get(cluster_index) else {
        return false;
    };
    if reference_mark_sequence_requires_previous(cluster_index, clusters) {
        return true;
    }
    if vertical_cluster_requires_previous_by_linebreak_only(cluster_index, clusters, config) {
        return true;
    }
    if jlreq_punctuation::is_line_head_prohibited_cluster(&cluster.text) {
        return true;
    }
    vertical_cluster_has_jlreq_separation_prohibited_before(
        cluster_index,
        clusters,
        config.jlreq_strictness,
    )
}

fn vertical_cluster_requires_previous_as_latin_word_or_unit(
    cluster_index: usize,
    clusters: &[VerticalCluster],
) -> bool {
    latin_word_sequence_requires_previous(cluster_index, clusters)
        || numeric_unit_symbol_sequence_requires_previous(cluster_index, clusters)
}

fn vertical_cluster_requires_previous_by_linebreak_only(
    cluster_index: usize,
    clusters: &[VerticalCluster],
    config: TextLayoutConfig,
) -> bool {
    let Some(cluster) = clusters.get(cluster_index) else {
        return false;
    };
    let Some(previous) = cluster_index
        .checked_sub(1)
        .and_then(|previous_index| clusters.get(previous_index))
    else {
        return false;
    };
    let requires_ascii_digit_sequence = !cluster.break_allowed_before
        && is_ascii_digit_cluster_text(&previous.text)
        && is_ascii_digit_cluster_text(&cluster.text);
    let requires_ascii_number_separator_sequence =
        ascii_number_separator_sequence_requires_previous(cluster_index, clusters);
    let requires_numeric_abbreviation_sequence =
        numeric_abbreviation_sequence_requires_previous(cluster_index, clusters);
    let requires_latin_word_sequence =
        latin_word_sequence_requires_previous(cluster_index, clusters);
    let requires_numeric_unit_symbol_sequence =
        numeric_unit_symbol_sequence_requires_previous(cluster_index, clusters);
    let requires_sub_superscript_object_sequence =
        sub_superscript_object_sequence_requires_previous(cluster_index, clusters);
    !jlreq_punctuation::is_line_end_prohibited_cluster(&cluster.text)
        && !jlreq_punctuation::is_line_head_prohibited_cluster(&cluster.text)
        && (requires_ascii_digit_sequence
            || requires_ascii_number_separator_sequence
            || requires_numeric_abbreviation_sequence
            || requires_latin_word_sequence
            || requires_numeric_unit_symbol_sequence
            || requires_sub_superscript_object_sequence)
        && !vertical_cluster_has_jlreq_separation_prohibited_before(
            cluster_index,
            clusters,
            config.jlreq_strictness,
        )
}

fn is_ascii_digit_cluster_text(text: &str) -> bool {
    !text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit())
}

fn ascii_number_separator_sequence_requires_previous(
    cluster_index: usize,
    clusters: &[VerticalCluster],
) -> bool {
    let Some(cluster) = clusters.get(cluster_index) else {
        return false;
    };
    if is_ascii_number_separator_cluster_text(&cluster.text) {
        return cluster_index
            .checked_sub(1)
            .and_then(|previous_index| clusters.get(previous_index))
            .is_some_and(|previous| is_ascii_digit_cluster_text(&previous.text))
            && clusters
                .get(cluster_index + 1)
                .is_some_and(|next| is_ascii_digit_cluster_text(&next.text));
    }
    is_ascii_digit_cluster_text(&cluster.text)
        && cluster_index
            .checked_sub(1)
            .and_then(|previous_index| clusters.get(previous_index))
            .is_some_and(|previous| is_ascii_number_separator_cluster_text(&previous.text))
        && cluster_index
            .checked_sub(2)
            .and_then(|before_separator_index| clusters.get(before_separator_index))
            .is_some_and(|before_separator| is_ascii_digit_cluster_text(&before_separator.text))
}

fn is_ascii_number_separator_cluster_text(text: &str) -> bool {
    matches!(text, "," | "." | " ")
}

fn numeric_abbreviation_sequence_requires_previous(
    cluster_index: usize,
    clusters: &[VerticalCluster],
) -> bool {
    let Some(cluster) = clusters.get(cluster_index) else {
        return false;
    };
    if is_numeric_suffix_abbreviation_cluster_text(&cluster.text) {
        return cluster_index
            .checked_sub(1)
            .and_then(|previous_index| clusters.get(previous_index))
            .is_some_and(|previous| is_jlreq_numeric_cluster_text(&previous.text));
    }
    if postfixed_abbreviation_unit_tail_requires_previous(cluster_index, clusters) {
        return true;
    }
    is_jlreq_numeric_cluster_text(&cluster.text)
        && cluster_index
            .checked_sub(1)
            .and_then(|previous_index| clusters.get(previous_index))
            .is_some_and(|previous| is_numeric_prefix_abbreviation_cluster_text(&previous.text))
}

fn is_jlreq_numeric_cluster_text(text: &str) -> bool {
    is_ascii_digit_cluster_text(text) || is_ideographic_numeral_cluster_text(text)
}

fn is_ideographic_numeral_cluster_text(text: &str) -> bool {
    let mut chars = text.chars();
    let Some(ch) = chars.next() else {
        return false;
    };
    chars.next().is_none()
        && matches!(
            ch,
            '〇' | '零'
                | '一'
                | '二'
                | '三'
                | '四'
                | '五'
                | '六'
                | '七'
                | '八'
                | '九'
                | '十'
                | '百'
                | '千'
                | '万'
                | '億'
                | '兆'
        )
}

fn is_numeric_prefix_abbreviation_cluster_text(text: &str) -> bool {
    matches!(text, "$" | "¢" | "¥" | "￥")
}

fn is_numeric_suffix_abbreviation_cluster_text(text: &str) -> bool {
    matches!(text, "%" | "％" | "‰" | "°" | "′" | "″" | "℃")
}

fn postfixed_abbreviation_unit_tail_requires_previous(
    cluster_index: usize,
    clusters: &[VerticalCluster],
) -> bool {
    let Some(cluster) = clusters.get(cluster_index) else {
        return false;
    };
    is_latin_or_greek_alphabetic_cluster_text(&cluster.text)
        && cluster_index
            .checked_sub(1)
            .and_then(|previous_index| clusters.get(previous_index))
            .is_some_and(|previous| is_postfixed_abbreviation_unit_leader(&previous.text))
        && cluster_index
            .checked_sub(2)
            .and_then(|numeric_index| clusters.get(numeric_index))
            .is_some_and(|numeric| is_jlreq_numeric_cluster_text(&numeric.text))
}

fn is_postfixed_abbreviation_unit_leader(text: &str) -> bool {
    matches!(text, "°" | "′" | "″")
}

fn numeric_unit_symbol_sequence_requires_previous(
    cluster_index: usize,
    clusters: &[VerticalCluster],
) -> bool {
    let Some(cluster) = clusters.get(cluster_index) else {
        return false;
    };
    is_latin_or_greek_alphabetic_cluster_text(&cluster.text)
        && cluster_index
            .checked_sub(1)
            .and_then(|previous_index| clusters.get(previous_index))
            .is_some_and(|previous| is_jlreq_numeric_cluster_text(&previous.text))
}

fn latin_word_sequence_requires_previous(
    cluster_index: usize,
    clusters: &[VerticalCluster],
) -> bool {
    let Some(cluster) = clusters.get(cluster_index) else {
        return false;
    };
    let previous = cluster_index
        .checked_sub(1)
        .and_then(|previous_index| clusters.get(previous_index));
    let next = clusters.get(cluster_index + 1);
    if is_latin_word_joiner_cluster_text(&cluster.text) {
        return previous
            .is_some_and(|previous| is_latin_or_greek_alphabetic_cluster_text(&previous.text))
            && next.is_some_and(|next| is_latin_or_greek_alphabetic_cluster_text(&next.text));
    }
    if !is_latin_or_greek_alphabetic_cluster_text(&cluster.text) {
        return false;
    }
    previous.is_some_and(|previous| {
        (!cluster.break_allowed_before && is_latin_or_greek_alphabetic_cluster_text(&previous.text))
            || (is_latin_word_joiner_cluster_text(&previous.text)
                && cluster_index
                    .checked_sub(2)
                    .and_then(|before_joiner_index| clusters.get(before_joiner_index))
                    .is_some_and(|before_joiner| {
                        is_latin_or_greek_alphabetic_cluster_text(&before_joiner.text)
                    }))
    })
}

fn is_latin_word_joiner_cluster_text(text: &str) -> bool {
    matches!(text, "'" | "\u{2019}" | "-" | "\u{2010}" | "\u{2011}")
}

fn sub_superscript_object_sequence_requires_previous(
    cluster_index: usize,
    clusters: &[VerticalCluster],
) -> bool {
    let Some(cluster) = clusters.get(cluster_index) else {
        return false;
    };
    let Some(previous) = cluster_index
        .checked_sub(1)
        .and_then(|previous_index| clusters.get(previous_index))
    else {
        return false;
    };
    (is_sub_superscript_cluster_text(&cluster.text)
        && (is_sub_superscript_base_cluster_text(&previous.text)
            || is_sub_superscript_cluster_text(&previous.text)))
        || (is_sub_superscript_base_cluster_text(&cluster.text)
            && is_sub_superscript_cluster_text(&previous.text))
}

fn is_sub_superscript_base_cluster_text(text: &str) -> bool {
    is_ascii_digit_cluster_text(text) || is_latin_or_greek_alphabetic_cluster_text(text)
}

fn is_sub_superscript_cluster_text(text: &str) -> bool {
    let mut chars = text.chars();
    let Some(ch) = chars.next() else {
        return false;
    };
    chars.next().is_none()
        && (matches!(ch, '\u{00b2}' | '\u{00b3}' | '\u{00b9}')
            || matches!(ch, '\u{2070}'..='\u{209f}'))
}

fn reference_mark_sequence_requires_previous(
    cluster_index: usize,
    clusters: &[VerticalCluster],
) -> bool {
    let Some(cluster) = clusters.get(cluster_index) else {
        return false;
    };
    let Some(previous) = cluster_index
        .checked_sub(1)
        .and_then(|previous_index| clusters.get(previous_index))
    else {
        return false;
    };
    if is_reference_mark_part_cluster_text(&cluster.text) {
        return true;
    }
    is_reference_mark_following_full_stop_cluster_text(&cluster.text)
        && is_reference_mark_part_cluster_text(&previous.text)
}

fn is_reference_mark_part_cluster_text(text: &str) -> bool {
    matches!(
        text,
        "¹" | "²" | "³" | "⁰" | "⁴" | "⁵" | "⁶" | "⁷" | "⁸" | "⁹" | "⁽" | "⁾"
    )
}

fn is_reference_mark_following_full_stop_cluster_text(text: &str) -> bool {
    matches!(text, "。" | "．" | ".")
}

pub(crate) fn vertical_cluster_can_start_column(
    cluster_index: usize,
    clusters: &[VerticalCluster],
    strictness: JlreqStrictness,
) -> bool {
    let Some(cluster) = clusters.get(cluster_index) else {
        return false;
    };
    let can_break_before = cluster.break_allowed_before
        || jlreq_punctuation::is_line_end_prohibited_cluster(&cluster.text);
    can_break_before
        && !jlreq_punctuation::is_line_head_prohibited_cluster(&cluster.text)
        && !ascii_number_separator_sequence_requires_previous(cluster_index, clusters)
        && !numeric_abbreviation_sequence_requires_previous(cluster_index, clusters)
        && !numeric_unit_symbol_sequence_requires_previous(cluster_index, clusters)
        && !latin_word_sequence_requires_previous(cluster_index, clusters)
        && !sub_superscript_object_sequence_requires_previous(cluster_index, clusters)
        && !reference_mark_sequence_requires_previous(cluster_index, clusters)
        && !vertical_cluster_has_jlreq_separation_prohibited_before(
            cluster_index,
            clusters,
            strictness,
        )
}

pub(crate) fn vertical_column_ends_with_line_end_prohibited(
    column_start: usize,
    column_end: usize,
    clusters: &[VerticalCluster],
) -> bool {
    clusters[column_start..column_end]
        .iter()
        .rev()
        .find(|cluster| !is_vertical_line_break_cluster(&cluster.text))
        .is_some_and(|cluster| jlreq_punctuation::is_line_end_prohibited_cluster(&cluster.text))
}

pub(crate) fn vertical_cluster_origin_y(
    grapheme: &str,
    cursor_y: f32,
    advance: f32,
    config: TextLayoutConfig,
) -> f32 {
    let column_end = config.origin.y + config.size.height;
    if jlreq_punctuation::is_hanging_cluster(grapheme)
        && cursor_y + config.font_size > column_end
        && cursor_y > config.origin.y
    {
        (column_end - advance).max(config.origin.y)
    } else {
        cursor_y
    }
}

fn vertical_cluster_has_jlreq_separation_prohibited_before(
    cluster_index: usize,
    clusters: &[VerticalCluster],
    strictness: JlreqStrictness,
) -> bool {
    let Some(cluster) = clusters.get(cluster_index) else {
        return false;
    };
    clusters[..cluster_index]
        .iter()
        .rev()
        .find(|candidate| !is_vertical_line_break_cluster(&candidate.text))
        .is_some_and(|previous| {
            jlreq_punctuation::pair_adjustment_for_clusters(
                &previous.text,
                &cluster.text,
                strictness,
            )
            .keep_together
        })
}
