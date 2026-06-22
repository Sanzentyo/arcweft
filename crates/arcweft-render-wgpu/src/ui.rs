//! Interaction-aware retained UI paint lowering for the shared wgpu renderer.

use crate::geometry::PaintRect;
use arcweft_presentation::hit::HitRect;
use arcweft_ui::{Milli, ResolvedDisplayList, ResolvedUiStyle, Rgba8, UiPropertyKind};
use num_traits::ToPrimitive;

/// Paint rectangles generated from one resolved retained UI display list.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct UiPaintPlan {
    rectangles: Vec<PaintRect>,
}

impl UiPaintPlan {
    pub fn from_resolved_display(display: &ResolvedDisplayList) -> Self {
        let rectangles = display
            .as_slice()
            .iter()
            .flat_map(|item| paint_item(item.item().layout(), item.style()))
            .collect();
        Self { rectangles }
    }

    pub fn rectangles(&self) -> &[PaintRect] {
        &self.rectangles
    }

    pub fn into_rectangles(self) -> Vec<PaintRect> {
        self.rectangles
    }

    pub fn is_empty(&self) -> bool {
        self.rectangles.is_empty()
    }
}

fn paint_item(layout: arcweft_ui::LayoutBox, style: &ResolvedUiStyle) -> Vec<PaintRect> {
    if !style.is_visible() {
        return Vec::new();
    }

    let [x, y, width, height] = layout.milli_rect();
    let translate_x = style
        .milli(UiPropertyKind::TranslateX)
        .unwrap_or(Milli::ZERO);
    let translate_y = style
        .milli(UiPropertyKind::TranslateY)
        .unwrap_or(Milli::ZERO);
    let scale = milli_scalar(style.scale()).max(0.0);
    let bounds = HitRect::new(
        milli_pixels(x),
        milli_pixels(y),
        milli_pixels(width).max(0.0),
        milli_pixels(height).max(0.0),
    )
    .translated(
        milli_pixels(translate_x.value()),
        milli_pixels(translate_y.value()),
    )
    .scaled_about_center(scale);
    let opacity = milli_scalar(style.opacity()).clamp(0.0, 1.0);

    let mut rectangles = Vec::new();
    if let Some(color) = style.color(UiPropertyKind::BackgroundColor) {
        rectangles.push(PaintRect {
            bounds,
            rgba: rgba(color, opacity),
        });
    }

    let outline_width = style
        .milli(UiPropertyKind::OutlineWidth)
        .map_or(0.0, |width| milli_pixels(width.value()).max(0.0));
    if outline_width > 0.0
        && let Some(color) = style.color(UiPropertyKind::OutlineColor)
    {
        rectangles.extend(outline_rectangles(
            bounds,
            outline_width,
            rgba(color, opacity),
        ));
    }

    rectangles
}

fn outline_rectangles(bounds: HitRect, width: f32, color: [f32; 4]) -> [PaintRect; 4] {
    let outer = bounds.outset(width);
    [
        PaintRect {
            bounds: HitRect::new(outer.x, outer.y, outer.width, width),
            rgba: color,
        },
        PaintRect {
            bounds: HitRect::new(outer.x, outer.y + outer.height - width, outer.width, width),
            rgba: color,
        },
        PaintRect {
            bounds: HitRect::new(outer.x, outer.y + width, width, outer.height - width * 2.0),
            rgba: color,
        },
        PaintRect {
            bounds: HitRect::new(
                outer.x + outer.width - width,
                outer.y + width,
                width,
                outer.height - width * 2.0,
            ),
            rgba: color,
        },
    ]
}

fn milli_pixels(value: i32) -> f32 {
    value.to_f32().unwrap_or_default() / 1_000.0
}

fn milli_scalar(value: Milli) -> f32 {
    milli_pixels(value.value())
}

fn rgba(color: Rgba8, opacity: f32) -> [f32; 4] {
    [
        f32::from(color.red) / 255.0,
        f32::from(color.green) / 255.0,
        f32::from(color.blue) / 255.0,
        (f32::from(color.alpha) / 255.0) * opacity,
    ]
}
