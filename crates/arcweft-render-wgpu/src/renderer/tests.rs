use super::*;
use crate::geometry::{RenderFontFamily, RenderTextSelectionPolicy};

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
