use super::*;
use crate::geometry::{RenderGlyphTransformKind, RenderTextReveal, RenderTextSelectionPolicy};

#[test]
fn plain_text_block_spacing_stays_compact_in_wide_buffer() {
    let mut font_system = new_font_system();
    let block = RenderTextBlock {
        target: None,
        text: "Alpha beta".to_owned(),
        bounds: HitRect::new(0.0, 0.0, 400.0, 40.0),
        clip_bounds: None,
        buffer_width: Some(400.0),
        buffer_height: Some(40.0),
        font_size: 20.0,
        line_height: 24.0,
        font_family: RenderFontFamily::SansSerif,
        weight: RenderTextWeight::Regular,
        slant: RenderTextSlant::Upright,
        rgba: [255, 255, 255, 255],
        selection_policy: RenderTextSelectionPolicy::Disabled,
        selection: None,
        selection_rgba: [0.0, 0.0, 0.0, 0.0],
    };

    let buffer = text_buffer(&mut font_system, &block);
    let right_edge = layout_text_right_edge(&buffer);

    assert!(right_edge > 20.0, "text did not produce visible glyphs");
    assert!(
        right_edge < 220.0,
        "text layout should not stretch word spacing across the full buffer: {right_edge}"
    );
}

#[test]
fn styled_paragraph_spacing_stays_compact_in_wide_buffer() {
    let mut font_system = new_font_system();
    let paragraph = RenderStyledParagraph {
        text: "Alpha beta".to_owned(),
        bounds: HitRect::new(0.0, 0.0, 400.0, 40.0),
        default_style: RenderTextStyle {
            font_size: 20.0,
            line_height: 24.0,
            color: [255, 255, 255, 255],
            font_family: RenderFontFamily::SansSerif,
            weight: RenderTextWeight::Regular,
            slant: RenderTextSlant::Upright,
        },
        spans: Vec::new(),
        reveal: RenderTextReveal {
            visible_end: 10,
            complete: true,
        },
        glyph_transforms: Vec::new(),
        visual_time_millis: 0,
    };

    let buffer = styled_paragraph_buffer(&mut font_system, &paragraph);
    let right_edge = layout_text_right_edge(&buffer);

    assert!(right_edge > 20.0, "text did not produce visible glyphs");
    assert!(
        right_edge < 220.0,
        "styled paragraph layout should not stretch word spacing across the full buffer: {right_edge}"
    );
}

#[test]
fn motion_overlay_keeps_transformed_text_after_hard_break() {
    let mut font_system = new_font_system();
    let text = "Captured the view-backed brief.\nIdea42".to_owned();
    let brief_start = "Captured the view-backed brief.\n".len();
    let brief_end = text.len();
    let brief_style = RenderTextStyle {
        font_size: 38.0,
        line_height: 51.3,
        color: [255, 64, 80, 255],
        font_family: RenderFontFamily::SansSerif,
        weight: RenderTextWeight::Bold,
        slant: RenderTextSlant::Italic,
    };
    let paragraph = RenderStyledParagraph {
        text,
        bounds: HitRect::new(32.0, 300.0, 760.0, 180.0),
        default_style: RenderTextStyle {
            font_size: 25.0,
            line_height: 34.0,
            color: [255, 255, 255, 255],
            font_family: RenderFontFamily::SansSerif,
            weight: RenderTextWeight::Regular,
            slant: RenderTextSlant::Upright,
        },
        spans: vec![RenderStyledTextSpan {
            range: RichTextRange::new(brief_start, brief_end),
            style: brief_style,
            node_index: 2,
        }],
        reveal: RenderTextReveal {
            visible_end: brief_end,
            complete: true,
        },
        glyph_transforms: vec![RenderGlyphTransformSpan {
            range: RichTextRange::new(brief_start, brief_end),
            motion: RenderGlyphMotion {
                kind: RenderGlyphTransformKind::Wave,
                amplitude: 5.0,
                frequency: 7.0,
            },
            node_index: 2,
        }],
        visual_time_millis: 1_000,
    };

    let layout_buffer = styled_paragraph_buffer(&mut font_system, &paragraph);
    let overlays = styled_paragraph_motion_overlays(&layout_buffer, &paragraph);
    let overlay_text = overlays
        .iter()
        .map(|overlay| overlay.text.as_str())
        .collect::<String>();

    assert_eq!(overlay_text, "Idea42");
    assert!(overlays.iter().all(|overlay| {
        overlay.top > paragraph.bounds.y + paragraph.default_style.line_height * 0.5
    }));
}

#[test]
fn text_bounds_are_scaled_to_physical_pixels() {
    let bounds = HitRect::new(10.25, 20.5, 100.25, 40.25);

    assert_eq!(
        scale_text_bounds(bounds, 2.0),
        TextBounds {
            left: 20,
            top: 41,
            right: 221,
            bottom: 122,
        }
    );
}

#[test]
fn text_bounds_keep_default_scale_pixel_rounding() {
    let bounds = HitRect::new(10.25, 20.5, 100.25, 40.25);

    assert_eq!(
        scale_text_bounds(bounds, 1.0),
        TextBounds {
            left: 10,
            top: 20,
            right: 111,
            bottom: 61,
        }
    );
}

fn layout_text_right_edge(buffer: &Buffer) -> f32 {
    buffer
        .layout_runs()
        .flat_map(|run| run.glyphs.iter())
        .map(|glyph| glyph.x + glyph.w)
        .fold(0.0_f32, f32::max)
}
