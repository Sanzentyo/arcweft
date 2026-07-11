//! Prepared text glyph submission and shared compositor effects.

use super::{
    PreparedTextRangeRenderRequest, SharedRendererError, clear_texture_view, frame_logical_extent,
    runtime_control_filter_texture, slice_range, text_index_is_excluded,
};
use crate::convert::{pixel_ceil_as_i32, pixel_floor_as_i32};
use crate::view_compositor::{ViewCompositor, ViewCompositorTarget, ViewPreparedTextEffectFrame};
use crate::view_effects::ViewTextureExtent;
use arcweft_glyphon::{GlyphonTextEngine, PreparedTextItem, PreparedTextSubmission};
use glyphon::{FontSystem, SwashCache, TextAtlas, TextBounds, TextRenderer, Viewport};

#[expect(
    clippy::too_many_arguments,
    reason = "Prepared glyph submission shares renderer atlas and project-font state."
)]
pub(super) fn render_prepared_text_range_with_renderer(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    encoder: &mut wgpu::CommandEncoder,
    text_renderer: &mut TextRenderer,
    engine: Option<&mut GlyphonTextEngine>,
    atlas: &mut TextAtlas,
    viewport: &Viewport,
    view_compositor: &mut ViewCompositor,
    request: &PreparedTextRangeRenderRequest<'_>,
) -> Result<(), SharedRendererError> {
    let range = request.range.clone();
    let items = slice_range(request.frame.prepared_text.items(), range.clone())
        .iter()
        .enumerate()
        .filter_map(|(offset, item)| {
            let index = range.start + offset;
            (!text_index_is_excluded(index, request.excluded_ranges)).then_some(item)
        })
        .collect::<Vec<_>>();
    if items.is_empty() {
        return Ok(());
    }
    let engine = engine.ok_or(SharedRendererError::MissingPreparedTextFonts)?;
    let (font_system, swash_cache) = engine.raster_parts_mut();
    let target_extent = ViewTextureExtent::new(
        request.frame.viewport.physical_width.max(1),
        request.frame.viewport.physical_height.max(1),
    );
    let logical_extent = frame_logical_extent(request.frame);
    for item in items {
        let submission = item.submission();
        if item.paint.offscreen_passes.is_empty() && item.paint.post_processes.is_empty() {
            render_prepared_submission_with_renderer(
                device,
                queue,
                encoder,
                text_renderer,
                font_system,
                atlas,
                viewport,
                swash_cache,
                request.target,
                item,
                &submission,
            )?;
            continue;
        }

        let effect_texture = runtime_control_filter_texture(
            device,
            view_compositor.format(),
            target_extent,
            "arcweft-prepared-text-effect-input",
        );
        let effect_view = effect_texture.create_view(&wgpu::TextureViewDescriptor::default());
        clear_texture_view(
            encoder,
            &effect_view,
            wgpu::Color::TRANSPARENT,
            "arcweft-prepared-text-effect-clear",
        );
        render_prepared_submission_with_renderer(
            device,
            queue,
            encoder,
            text_renderer,
            font_system,
            atlas,
            viewport,
            swash_cache,
            &effect_view,
            item,
            &submission,
        )?;
        view_compositor.render_prepared_text_effects(&mut ViewPreparedTextEffectFrame {
            device,
            encoder,
            source: ViewCompositorTarget {
                texture: &effect_texture,
                view: &effect_view,
                extent: target_extent,
                origin_logical: [0.0, 0.0],
                logical_extent,
            },
            output: request.target,
            offscreen_passes: &item.paint.offscreen_passes,
            post_processes: &item.paint.post_processes,
            device_pixel_ratio: request.frame.viewport.physical_scale_factor_f32(),
        })?;
    }
    Ok(())
}

#[expect(
    clippy::too_many_arguments,
    reason = "One prepared submission borrows the shared glyph cache and caller render target."
)]
fn render_prepared_submission_with_renderer(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    encoder: &mut wgpu::CommandEncoder,
    text_renderer: &mut TextRenderer,
    font_system: &mut FontSystem,
    atlas: &mut TextAtlas,
    viewport: &Viewport,
    swash_cache: &mut SwashCache,
    target: &wgpu::TextureView,
    item: &PreparedTextItem,
    submission: &PreparedTextSubmission,
) -> Result<(), SharedRendererError> {
    let area = submission.glyph_area(prepared_text_bounds(item.clip, submission.raster_scale()));
    text_renderer
        .prepare_glyph_areas(
            device,
            queue,
            font_system,
            atlas,
            viewport,
            [area],
            swash_cache,
        )
        .map_err(|error| SharedRendererError::TextPrepare(error.to_string()))?;
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("arcweft-shared-prepared-text-render-pass"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: target,
            resolve_target: None,
            depth_slice: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Load,
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    text_renderer
        .render(atlas, viewport, &mut pass)
        .map_err(|error| SharedRendererError::TextRender(error.to_string()))
}

fn prepared_text_bounds(
    clip: Option<arcweft_text_layout::LayoutRect>,
    raster_scale: f32,
) -> TextBounds {
    clip.map_or_else(TextBounds::default, |clip| TextBounds {
        left: pixel_floor_as_i32(clip.x * raster_scale),
        top: pixel_floor_as_i32(clip.y * raster_scale),
        right: pixel_ceil_as_i32(clip.right() * raster_scale),
        bottom: pixel_ceil_as_i32(clip.bottom() * raster_scale),
    })
}
