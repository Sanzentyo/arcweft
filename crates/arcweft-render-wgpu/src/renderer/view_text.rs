//! Prepared-text callback used at an exact View compositor painter position.

use super::prepared_text::render_prepared_text_item_with_affine;
use crate::geometry::PreparedFrame;
use crate::view_compositor::{
    ViewCompositor, ViewCompositorError, ViewTextRenderFrame, ViewTextRenderer,
};
use crate::view_direct_renderer::WgpuViewDirectPrimitiveRenderer;
use crate::view_scene::{PreparedTextId, ViewAffine2D, ViewClip, ViewColorRgba8, ViewSolidRect};
use arcweft_glyphon::{GlyphonTextEngine, PreparedTextAffine, PreparedTextPhysicalBounds};
use arcweft_text_layout::LayoutRect;
use glyphon::{Cache, TextAtlas, TextRenderer};

pub(super) struct WgpuViewPreparedTextRenderer<'a> {
    frame: &'a PreparedFrame,
    engine: Option<&'a mut GlyphonTextEngine>,
    cache: &'a Cache,
    atlas: &'a mut TextAtlas,
    text_renderers: &'a mut Vec<TextRenderer>,
    effect_compositor: &'a mut ViewCompositor,
    direct_renderer: &'a WgpuViewDirectPrimitiveRenderer,
    device_pixel_ratio: f32,
}

impl<'a> WgpuViewPreparedTextRenderer<'a> {
    #[expect(
        clippy::too_many_arguments,
        reason = "The callback borrows disjoint long-lived renderer resources for one View scene."
    )]
    pub(super) const fn new(
        frame: &'a PreparedFrame,
        engine: Option<&'a mut GlyphonTextEngine>,
        cache: &'a Cache,
        atlas: &'a mut TextAtlas,
        text_renderers: &'a mut Vec<TextRenderer>,
        effect_compositor: &'a mut ViewCompositor,
        direct_renderer: &'a WgpuViewDirectPrimitiveRenderer,
        device_pixel_ratio: f32,
    ) -> Self {
        Self {
            frame,
            engine,
            cache,
            atlas,
            text_renderers,
            effect_compositor,
            direct_renderer,
            device_pixel_ratio,
        }
    }
}

impl ViewTextRenderer for WgpuViewPreparedTextRenderer<'_> {
    fn render_text(
        &mut self,
        frame: &mut ViewTextRenderFrame<'_>,
        text: PreparedTextId,
    ) -> Result<(), ViewCompositorError> {
        let item = self
            .frame
            .text
            .get(text)
            .ok_or(ViewCompositorError::MissingPreparedText {
                text_index: text.index(),
            })?;
        let transform = frame.context.transform;
        let affine = PreparedTextAffine::try_new(
            [
                transform.m11,
                transform.m12,
                transform.m21,
                transform.m22,
                transform.tx - frame.target.origin_logical[0],
                transform.ty - frame.target.origin_logical[1],
            ],
            frame.context.opacity,
        )
        .map_err(text_error)?;
        let clip = resolved_clip(
            item.clip,
            transform,
            frame.context.clip.as_ref(),
            frame.target,
        );
        let physical_clip_bounds = clip
            .map(|clip| PreparedTextPhysicalBounds::try_from_logical(clip, item.raster_scale()))
            .transpose()?;
        let selection_color = selection_color(item.interaction.selection_rgba)?;
        let selection = item
            .interaction
            .selection_rects
            .iter()
            .copied()
            .filter_map(|bounds| solid_rect(bounds, item.clip, selection_color))
            .collect::<Vec<_>>();
        self.direct_renderer.render_solid_rects(frame, &selection)?;
        render_prepared_text_item_with_affine(
            frame.device,
            frame.queue,
            frame.encoder,
            self.text_renderers,
            self.engine.as_deref_mut(),
            self.cache,
            self.atlas,
            self.effect_compositor,
            frame.target,
            item,
            affine,
            physical_clip_bounds,
            self.device_pixel_ratio,
        )
        .map_err(text_error)?;
        self.direct_renderer
            .render_solid_rects(frame, &interaction_foreground(item))
    }
}

fn interaction_foreground(item: &arcweft_glyphon::PreparedTextItem) -> Vec<ViewSolidRect> {
    let mut rects = item
        .interaction
        .composition_underlines
        .iter()
        .filter_map(|underline| {
            let mut bounds = underline.bounds;
            bounds.height = underline.thickness;
            solid_rect(bounds, item.clip, ViewColorRgba8::from(underline.color))
        })
        .collect::<Vec<_>>();
    if let Some(caret) = item.interaction.caret.filter(|caret| caret.visible) {
        rects.extend(solid_rect(
            caret.bounds,
            item.clip,
            ViewColorRgba8::from(caret.color),
        ));
    }
    rects
}

fn solid_rect(
    bounds: LayoutRect,
    clip: Option<LayoutRect>,
    color: ViewColorRgba8,
) -> Option<ViewSolidRect> {
    let bounds = clip.map_or(bounds, |clip| intersection(bounds, clip));
    (bounds.width > 0.0 && bounds.height > 0.0).then_some(ViewSolidRect {
        bounds: arcweft_presentation::hit::HitRect::new(
            bounds.x,
            bounds.y,
            bounds.width,
            bounds.height,
        ),
        color,
    })
}

fn selection_color(channels: [f32; 4]) -> Result<ViewColorRgba8, ViewCompositorError> {
    if channels
        .iter()
        .any(|channel| !channel.is_finite() || !(0.0..=1.0).contains(channel))
    {
        return Err(ViewCompositorError::TextRender {
            reason: "selection color contains a non-finite or out-of-range channel".into(),
        });
    }
    Ok(ViewColorRgba8 {
        red: unit_channel(channels[0]),
        green: unit_channel(channels[1]),
        blue: unit_channel(channels[2]),
        alpha: unit_channel(channels[3]),
    })
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn unit_channel(channel: f32) -> u8 {
    (channel * 255.0).round() as u8
}

fn resolved_clip(
    item_clip: Option<LayoutRect>,
    transform: ViewAffine2D,
    context_clip: Option<&ViewClip>,
    target: crate::view_compositor::ViewCompositorTarget<'_>,
) -> Option<LayoutRect> {
    let item_clip = item_clip.map(|clip| transformed_bounds(clip, transform));
    let context_clip = context_clip.map(|clip| match clip {
        ViewClip::Rect(bounds) | ViewClip::RoundedRect { bounds, .. } => {
            LayoutRect::new(bounds.x, bounds.y, bounds.width, bounds.height)
        }
    });
    item_clip
        .into_iter()
        .chain(context_clip)
        .reduce(intersection)
        .map(|clip| {
            LayoutRect::new(
                clip.x - target.origin_logical[0],
                clip.y - target.origin_logical[1],
                clip.width,
                clip.height,
            )
        })
}

fn transformed_bounds(bounds: LayoutRect, transform: ViewAffine2D) -> LayoutRect {
    let points = [
        transformed_point(bounds.x, bounds.y, transform),
        transformed_point(bounds.right(), bounds.y, transform),
        transformed_point(bounds.right(), bounds.bottom(), transform),
        transformed_point(bounds.x, bounds.bottom(), transform),
    ];
    let left = points
        .iter()
        .map(|point| point[0])
        .fold(f32::INFINITY, f32::min);
    let top = points
        .iter()
        .map(|point| point[1])
        .fold(f32::INFINITY, f32::min);
    let right = points
        .iter()
        .map(|point| point[0])
        .fold(f32::NEG_INFINITY, f32::max);
    let bottom = points
        .iter()
        .map(|point| point[1])
        .fold(f32::NEG_INFINITY, f32::max);
    LayoutRect::new(left, top, (right - left).max(0.0), (bottom - top).max(0.0))
}

fn transformed_point(x: f32, y: f32, transform: ViewAffine2D) -> [f32; 2] {
    [
        transform
            .m11
            .mul_add(x, transform.m21.mul_add(y, transform.tx)),
        transform
            .m12
            .mul_add(x, transform.m22.mul_add(y, transform.ty)),
    ]
}

fn intersection(left: LayoutRect, right: LayoutRect) -> LayoutRect {
    let x = left.x.max(right.x);
    let y = left.y.max(right.y);
    let right_edge = left.right().min(right.right());
    let bottom_edge = left.bottom().min(right.bottom());
    LayoutRect::new(x, y, (right_edge - x).max(0.0), (bottom_edge - y).max(0.0))
}

fn text_error(error: impl std::fmt::Display) -> ViewCompositorError {
    ViewCompositorError::TextRender {
        reason: error.to_string().into_boxed_str(),
    }
}
