//! Grapheme clustering and UAX #50 orientation resolution for vertical text.

use crate::{
    GlyphOrientation, GlyphVerticalForm,
    vertical_orientation::{UnicodeVerticalOrientation, unicode_vertical_orientation},
};
use arcweft_render_text::RichTextVerticalLatinMode;
use std::{collections::HashSet, ops::Range};
use unicode_linebreak::{BreakOpportunity, linebreaks};
use unicode_segmentation::UnicodeSegmentation as _;

#[derive(Clone, Debug)]
pub(crate) struct VerticalCluster {
    pub(crate) range: Range<usize>,
    pub(crate) text: String,
    pub(crate) orientation: GlyphOrientation,
    pub(crate) vertical_form: GlyphVerticalForm,
    pub(crate) break_allowed_before: bool,
}

const MAX_TEXT_COMBINE_DIGITS: usize = 4;

pub(crate) fn vertical_clusters(
    text: &str,
    vertical_latin: RichTextVerticalLatinMode,
) -> Vec<VerticalCluster> {
    let mut clusters = Vec::new();
    let graphemes: Vec<(usize, &str)> = text.grapheme_indices(true).collect();
    let break_offsets = line_break_offsets(text);
    let mut index = 0;
    while let Some((offset, grapheme)) = graphemes.get(index).copied() {
        if is_ascii_digit_grapheme(grapheme) {
            let mut end = offset + grapheme.len();
            let mut value = grapheme.to_owned();
            let mut digit_count = 1;
            let break_allowed_before = break_offsets.contains(&offset);
            index += 1;
            while let Some((next_offset, next)) = graphemes.get(index).copied() {
                if !is_ascii_digit_grapheme(next) || digit_count >= MAX_TEXT_COMBINE_DIGITS {
                    break;
                }
                value.push_str(next);
                end = next_offset + next.len();
                digit_count += 1;
                index += 1;
            }
            if digit_count >= 2 {
                clusters.push(VerticalCluster {
                    range: offset..end,
                    text: value,
                    orientation: GlyphOrientation::TextCombineUpright,
                    vertical_form: GlyphVerticalForm::None,
                    break_allowed_before,
                });
                continue;
            }
            let orientation = vertical_orientation(grapheme, vertical_latin);
            clusters.push(VerticalCluster {
                range: offset..end,
                text: value,
                orientation,
                vertical_form: vertical_form(grapheme, vertical_latin),
                break_allowed_before,
            });
            continue;
        }
        if is_sideways_latin_run_grapheme(grapheme, vertical_latin) {
            let mut end = offset + grapheme.len();
            let mut value = grapheme.to_owned();
            let break_allowed_before = break_offsets.contains(&offset);
            index += 1;
            while let Some((next_offset, next)) = graphemes.get(index).copied() {
                if !is_sideways_latin_run_grapheme(next, vertical_latin) {
                    break;
                }
                value.push_str(next);
                end = next_offset + next.len();
                index += 1;
            }
            clusters.push(VerticalCluster {
                range: offset..end,
                text: value,
                orientation: GlyphOrientation::SidewaysCw,
                vertical_form: GlyphVerticalForm::None,
                break_allowed_before,
            });
            continue;
        }
        index += 1;
        let orientation = vertical_orientation(grapheme, vertical_latin);
        clusters.push(VerticalCluster {
            range: offset..offset + grapheme.len(),
            text: grapheme.to_owned(),
            orientation,
            vertical_form: vertical_form(grapheme, vertical_latin),
            break_allowed_before: break_offsets.contains(&offset),
        });
    }
    clusters
}

fn is_sideways_latin_run_grapheme(
    grapheme: &str,
    vertical_latin: RichTextVerticalLatinMode,
) -> bool {
    is_latin_or_greek_alphabetic_cluster_text(grapheme)
        && matches!(
            vertical_orientation(grapheme, vertical_latin),
            GlyphOrientation::SidewaysCw
        )
}

pub(crate) fn cluster_is_sideways_latin_run(cluster: &VerticalCluster) -> bool {
    cluster.orientation == GlyphOrientation::SidewaysCw
        && cluster.text.graphemes(true).count() > 1
        && cluster
            .text
            .graphemes(true)
            .all(is_latin_or_greek_alphabetic_cluster_text)
}

pub(crate) fn is_latin_or_greek_alphabetic_cluster_text(text: &str) -> bool {
    let mut has_script_letter = false;
    for ch in text.chars() {
        if is_latin_or_greek_alphabetic_char(ch) {
            has_script_letter = true;
        } else if !(is_combining_mark(ch) || is_variation_selector(ch)) {
            return false;
        }
    }
    has_script_letter
}

const fn is_latin_or_greek_alphabetic_char(ch: char) -> bool {
    matches!(
        ch,
        'A'..='Z'
            | 'a'..='z'
            | '\u{00b5}'
            | '\u{00c0}'..='\u{00ff}'
            | '\u{0100}'..='\u{024f}'
            | '\u{0370}'..='\u{03ff}'
            | '\u{1f00}'..='\u{1fff}'
            | '\u{1e00}'..='\u{1eff}'
            | '\u{ff21}'..='\u{ff3a}'
            | '\u{ff41}'..='\u{ff5a}'
    )
}

pub(crate) fn line_break_offsets(text: &str) -> HashSet<usize> {
    linebreaks(text)
        .filter_map(|(offset, opportunity)| match opportunity {
            BreakOpportunity::Allowed | BreakOpportunity::Mandatory if offset < text.len() => {
                Some(offset)
            }
            BreakOpportunity::Allowed | BreakOpportunity::Mandatory => None,
        })
        .collect()
}

fn is_ascii_digit_grapheme(grapheme: &str) -> bool {
    matches!(grapheme.as_bytes(), [b'0'..=b'9'])
}

pub(crate) fn is_vertical_line_break_cluster(grapheme: &str) -> bool {
    matches!(grapheme, "\n" | "\r\n")
}

fn vertical_orientation(
    grapheme: &str,
    vertical_latin: RichTextVerticalLatinMode,
) -> GlyphOrientation {
    match vertical_latin {
        RichTextVerticalLatinMode::Upright => GlyphOrientation::Upright,
        RichTextVerticalLatinMode::Sideways => GlyphOrientation::SidewaysCw,
        RichTextVerticalLatinMode::Mixed => {
            match unicode_vertical_orientation_for_grapheme(grapheme) {
                UnicodeVerticalOrientation::Upright
                | UnicodeVerticalOrientation::TransformedUpright => GlyphOrientation::Upright,
                UnicodeVerticalOrientation::Rotated
                | UnicodeVerticalOrientation::TransformedRotated => GlyphOrientation::SidewaysCw,
            }
        }
    }
}

fn vertical_form(grapheme: &str, vertical_latin: RichTextVerticalLatinMode) -> GlyphVerticalForm {
    if !matches!(vertical_latin, RichTextVerticalLatinMode::Mixed) {
        return GlyphVerticalForm::None;
    }
    match unicode_vertical_orientation_for_grapheme(grapheme) {
        UnicodeVerticalOrientation::Upright | UnicodeVerticalOrientation::Rotated => {
            GlyphVerticalForm::None
        }
        UnicodeVerticalOrientation::TransformedUpright => GlyphVerticalForm::UprightAlternate,
        UnicodeVerticalOrientation::TransformedRotated => GlyphVerticalForm::RotatedAlternate,
    }
}

fn unicode_vertical_orientation_for_grapheme(grapheme: &str) -> UnicodeVerticalOrientation {
    if is_keycap_grapheme(grapheme) {
        return UnicodeVerticalOrientation::Upright;
    }
    grapheme
        .chars()
        .find(|ch| !is_grapheme_modifier_or_join_control(*ch))
        .or_else(|| grapheme.chars().next())
        .map_or(
            UnicodeVerticalOrientation::Rotated,
            unicode_vertical_orientation,
        )
}

fn is_keycap_grapheme(grapheme: &str) -> bool {
    let Some(head) = grapheme.chars().next() else {
        return false;
    };
    matches!(head, '#' | '*' | '0'..='9') && grapheme.chars().any(|ch| ch == '\u{20e3}')
}

const fn is_grapheme_modifier_or_join_control(ch: char) -> bool {
    is_combining_mark(ch) || is_variation_selector(ch) || matches!(ch, '\u{200c}' | '\u{200d}')
}

pub(crate) const fn is_combining_mark(ch: char) -> bool {
    matches!(
        ch,
        '\u{0300}'..='\u{036f}'
            | '\u{1ab0}'..='\u{1aff}'
            | '\u{1dc0}'..='\u{1dff}'
            | '\u{20d0}'..='\u{20ff}'
            | '\u{fe20}'..='\u{fe2f}'
    )
}

pub(crate) const fn is_variation_selector(ch: char) -> bool {
    matches!(ch, '\u{fe00}'..='\u{fe0f}' | '\u{e0100}'..='\u{e01ef}')
}
