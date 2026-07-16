//! Interaction-aware retained View paint lowering for the shared wgpu renderer.

use crate::geometry::PaintRect;
use arcweft_presentation::appearance::{
    PresentationColor, PresentationEnvironment, SystemPaletteSet,
};
use arcweft_presentation::hit::HitRect;
use arcweft_view::{
    ComputedViewStyle, ResolvedDisplayList, ViewColorValue, ViewDisplay, ViewPropertyKind,
    ViewSpecifiedValue,
};
use num_traits::ToPrimitive;

/// Paint rectangles generated from one resolved retained View display list.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ViewPaintPlan {
    rectangles: Vec<PaintRect>,
}

impl ViewPaintPlan {
    pub fn from_resolved_display(
        display: &ResolvedDisplayList,
        environment: &PresentationEnvironment,
        palettes: &SystemPaletteSet,
    ) -> Self {
        let rectangles = display
            .as_slice()
            .iter()
            .flat_map(|item| paint_item(item.item().layout(), item.style(), environment, palettes))
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

fn paint_item(
    layout: arcweft_view::LayoutBox,
    style: &ComputedViewStyle,
    environment: &PresentationEnvironment,
    palettes: &SystemPaletteSet,
) -> Vec<PaintRect> {
    if !is_visible(style) {
        return Vec::new();
    }

    let [x, y, width, height] = layout.milli_rect();
    let physical_box = style.physical_box();
    let translate_x = physical_box.translate_x.value();
    let translate_y = physical_box.translate_y.value();
    let scale = physical_box.scale.value();
    let bounds = HitRect::new(
        milli_pixels(x),
        milli_pixels(y),
        milli_pixels(width).max(0.0),
        milli_pixels(height).max(0.0),
    )
    .translated(milli_pixels(translate_x), milli_pixels(translate_y))
    .scaled_about_center(milli_scalar(scale));
    let opacity = ratio(style, ViewPropertyKind::Opacity)
        .map_or(1.0, milli_ratio)
        .clamp(0.0, 1.0);

    let mut rectangles = Vec::new();
    if let Some(color) = color(
        style,
        ViewPropertyKind::BackgroundColor,
        environment,
        palettes,
    ) {
        rectangles.push(PaintRect::new(bounds, rgba(color, opacity)));
    }

    let outline_width = length(style, ViewPropertyKind::OutlineWidth)
        .map_or(0.0, |width| milli_pixels(width).max(0.0));
    if outline_width > 0.0
        && let Some(color) = color(style, ViewPropertyKind::OutlineColor, environment, palettes)
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
        PaintRect::new(HitRect::new(outer.x, outer.y, outer.width, width), color),
        PaintRect::new(
            HitRect::new(outer.x, outer.y + outer.height - width, outer.width, width),
            color,
        ),
        PaintRect::new(
            HitRect::new(outer.x, outer.y + width, width, outer.height - width * 2.0),
            color,
        ),
        PaintRect::new(
            HitRect::new(
                outer.x + outer.width - width,
                outer.y + width,
                width,
                outer.height - width * 2.0,
            ),
            color,
        ),
    ]
}

fn milli_pixels(value: i32) -> f32 {
    value.to_f32().unwrap_or_default() / 1_000.0
}

fn milli_scalar(value: u32) -> f32 {
    value.to_f32().unwrap_or(f32::MAX) / 1_000.0
}

fn milli_ratio(value: u16) -> f32 {
    f32::from(value) / 1_000.0
}

fn is_visible(style: &ComputedViewStyle) -> bool {
    !matches!(
        style.value(ViewPropertyKind::Visibility),
        Some(ViewSpecifiedValue::Bool { value: false })
    ) && !matches!(
        style.value(ViewPropertyKind::Display),
        Some(ViewSpecifiedValue::Display {
            value: ViewDisplay::None
        })
    )
}

fn length(style: &ComputedViewStyle, property: ViewPropertyKind) -> Option<i32> {
    match style.value(property) {
        Some(ViewSpecifiedValue::Length { value }) => Some(value.value()),
        _ => None,
    }
}

fn ratio(style: &ComputedViewStyle, property: ViewPropertyKind) -> Option<u16> {
    match style.value(property) {
        Some(ViewSpecifiedValue::Ratio { value }) => Some(value.value()),
        _ => None,
    }
}

fn color(
    style: &ComputedViewStyle,
    property: ViewPropertyKind,
    environment: &PresentationEnvironment,
    palettes: &SystemPaletteSet,
) -> Option<PresentationColor> {
    match style.value(property) {
        Some(ViewSpecifiedValue::Color {
            value: ViewColorValue::Literal { color },
        }) => Some(*color),
        Some(ViewSpecifiedValue::Color {
            value: ViewColorValue::System { role },
        }) => Some(palettes.color(environment.color_scheme(), *role)),
        _ => None,
    }
}

fn rgba(color: PresentationColor, opacity: f32) -> [f32; 4] {
    [
        f32::from(color.red) / 255.0,
        f32::from(color.green) / 255.0,
        f32::from(color.blue) / 255.0,
        (f32::from(color.alpha) / 255.0) * opacity,
    ]
}
