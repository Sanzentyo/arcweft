//! Prepared text glyph submission and shared compositor effects.

use super::{
    PreparedTextRangeRenderRequest, SharedRendererError, clear_texture_view, draw_rectangle_buffer,
    frame_logical_extent, rectangle_vertex_buffer, runtime_control_filter_texture, slice_range,
    text_index_is_excluded,
};
use crate::geometry::PaintRect;
use crate::view_compositor::{ViewCompositor, ViewCompositorTarget, ViewPreparedTextEffectFrame};
use crate::view_effects::ViewTextureExtent;
use arcweft_glyphon::{
    GlyphonTextEngine, PreparedTextAffine, PreparedTextItem, PreparedTextPhysicalBounds,
    PreparedTextSubmission,
};
use arcweft_text_layout::LayoutRect;
use glyphon::{
    Cache, FontSystem, Resolution, SwashCache, TextAtlas, TextBounds, TextRenderer, Viewport,
};

#[expect(
    clippy::too_many_arguments,
    reason = "Prepared glyph submission shares renderer atlas and project-font state."
)]
pub(super) fn render_prepared_text_range_with_renderer(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    encoder: &mut wgpu::CommandEncoder,
    rectangle_pipeline: &wgpu::RenderPipeline,
    text_renderers: &mut Vec<TextRenderer>,
    engine: Option<&mut GlyphonTextEngine>,
    atlas: &mut TextAtlas,
    viewport: &Viewport,
    view_compositor: &mut ViewCompositor,
    request: &PreparedTextRangeRenderRequest<'_>,
) -> Result<(), SharedRendererError> {
    let range = request.range.clone();
    let items = slice_range(request.frame.text.items(), range.clone())
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
        render_prepared_text_item(
            device,
            queue,
            encoder,
            rectangle_pipeline,
            text_renderers,
            font_system,
            swash_cache,
            atlas,
            viewport,
            view_compositor,
            request,
            target_extent,
            logical_extent,
            item,
        )?;
    }
    Ok(())
}

#[expect(
    clippy::too_many_arguments,
    reason = "One prepared item binds shared glyph, interaction, and compositor resources."
)]
fn render_prepared_text_item(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    encoder: &mut wgpu::CommandEncoder,
    rectangle_pipeline: &wgpu::RenderPipeline,
    text_renderers: &mut Vec<TextRenderer>,
    font_system: &mut FontSystem,
    swash_cache: &mut SwashCache,
    atlas: &mut TextAtlas,
    viewport: &Viewport,
    view_compositor: &mut ViewCompositor,
    request: &PreparedTextRangeRenderRequest<'_>,
    target_extent: ViewTextureExtent,
    logical_extent: [f32; 2],
    item: &PreparedTextItem,
) -> Result<(), SharedRendererError> {
    let backgrounds = interaction_background(item);
    render_interaction_rectangles(
        device,
        encoder,
        rectangle_pipeline,
        request.target,
        &backgrounds,
        logical_extent,
        "arcweft-prepared-text-interaction-background",
    );
    text_renderers.push(TextRenderer::new(
        atlas,
        device,
        wgpu::MultisampleState::default(),
        None,
    ));
    let text_renderer = text_renderers
        .last_mut()
        .expect("prepared text renderer was just pushed");
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
            item.physical_clip_bounds(),
            &submission,
        )?;
    } else {
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
            item.physical_clip_bounds(),
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
    let foregrounds = interaction_foreground(item);
    render_interaction_rectangles(
        device,
        encoder,
        rectangle_pipeline,
        request.target,
        &foregrounds,
        logical_extent,
        "arcweft-prepared-text-interaction-foreground",
    );
    Ok(())
}

fn interaction_background(item: &PreparedTextItem) -> Vec<PaintRect> {
    item.interaction
        .selection_rects
        .iter()
        .filter_map(|bounds| clipped_rect(*bounds, item.clip))
        .map(|bounds| PaintRect::new(bounds, item.interaction.selection_rgba))
        .collect()
}

fn interaction_foreground(item: &PreparedTextItem) -> Vec<PaintRect> {
    let mut rectangles = item
        .interaction
        .composition_underlines
        .iter()
        .filter_map(|underline| {
            let mut bounds = underline.bounds;
            bounds.height = underline.thickness;
            clipped_rect(bounds, item.clip)
                .map(|bounds| PaintRect::new(bounds, color_channels(underline.color)))
        })
        .collect::<Vec<_>>();
    if let Some(caret) = item.interaction.caret.filter(|caret| caret.visible)
        && let Some(bounds) = clipped_rect(caret.bounds, item.clip)
    {
        rectangles.push(PaintRect::new(bounds, color_channels(caret.color)));
    }
    rectangles
}

fn clipped_rect(
    bounds: LayoutRect,
    clip: Option<LayoutRect>,
) -> Option<arcweft_presentation::hit::HitRect> {
    let bounds = clip.map_or(bounds, |clip| {
        let x = bounds.x.max(clip.x);
        let y = bounds.y.max(clip.y);
        let right = bounds.right().min(clip.right());
        let bottom = bounds.bottom().min(clip.bottom());
        LayoutRect::new(x, y, (right - x).max(0.0), (bottom - y).max(0.0))
    });
    (bounds.width > 0.0 && bounds.height > 0.0).then(|| {
        arcweft_presentation::hit::HitRect::new(bounds.x, bounds.y, bounds.width, bounds.height)
    })
}

fn color_channels(color: arcweft_render_text::TextColor) -> [f32; 4] {
    color.channels().map(|channel| f32::from(channel) / 255.0)
}

fn render_interaction_rectangles(
    device: &wgpu::Device,
    encoder: &mut wgpu::CommandEncoder,
    pipeline: &wgpu::RenderPipeline,
    target: &wgpu::TextureView,
    rectangles: &[PaintRect],
    logical_extent: [f32; 2],
    label: &'static str,
) {
    let Some((vertices, count)) = rectangle_vertex_buffer(
        device,
        label,
        rectangles,
        logical_extent[0],
        logical_extent[1],
    ) else {
        return;
    };
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some(label),
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
    draw_rectangle_buffer(&mut pass, pipeline, &vertices, count);
}

#[expect(
    clippy::too_many_arguments,
    reason = "View text rendering binds one prepared item to the active compositor target."
)]
pub(super) fn render_prepared_text_item_with_affine(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    encoder: &mut wgpu::CommandEncoder,
    text_renderers: &mut Vec<TextRenderer>,
    engine: Option<&mut GlyphonTextEngine>,
    cache: &Cache,
    atlas: &mut TextAtlas,
    effect_compositor: &mut ViewCompositor,
    target: ViewCompositorTarget<'_>,
    item: &PreparedTextItem,
    affine: PreparedTextAffine,
    physical_clip_bounds: Option<PreparedTextPhysicalBounds>,
    device_pixel_ratio: f32,
) -> Result<(), SharedRendererError> {
    let engine = engine.ok_or(SharedRendererError::MissingPreparedTextFonts)?;
    let (font_system, swash_cache) = engine.raster_parts_mut();
    let mut viewport = Viewport::new(device, cache);
    viewport.update(
        queue,
        Resolution {
            width: target.extent.width,
            height: target.extent.height,
        },
    );
    let submission = item.submission_with_affine(affine);
    text_renderers.push(TextRenderer::new(
        atlas,
        device,
        wgpu::MultisampleState::default(),
        None,
    ));
    let text_renderer = text_renderers
        .last_mut()
        .expect("prepared text renderer was just pushed");
    if item.paint.offscreen_passes.is_empty() && item.paint.post_processes.is_empty() {
        return render_prepared_submission_with_renderer(
            device,
            queue,
            encoder,
            text_renderer,
            font_system,
            atlas,
            &viewport,
            swash_cache,
            target.view,
            physical_clip_bounds,
            &submission,
        );
    }

    let effect_texture = runtime_control_filter_texture(
        device,
        effect_compositor.format(),
        target.extent,
        "arcweft-view-text-effect-input",
    );
    let effect_view = effect_texture.create_view(&wgpu::TextureViewDescriptor::default());
    clear_texture_view(
        encoder,
        &effect_view,
        wgpu::Color::TRANSPARENT,
        "arcweft-view-text-effect-clear",
    );
    render_prepared_submission_with_renderer(
        device,
        queue,
        encoder,
        text_renderer,
        font_system,
        atlas,
        &viewport,
        swash_cache,
        &effect_view,
        physical_clip_bounds,
        &submission,
    )?;
    effect_compositor.render_prepared_text_effects(&mut ViewPreparedTextEffectFrame {
        device,
        encoder,
        source: ViewCompositorTarget {
            texture: &effect_texture,
            view: &effect_view,
            extent: target.extent,
            origin_logical: [0.0, 0.0],
            logical_extent: target.logical_extent,
        },
        output: target.view,
        offscreen_passes: &item.paint.offscreen_passes,
        post_processes: &item.paint.post_processes,
        device_pixel_ratio,
    })?;
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
    physical_clip_bounds: Option<PreparedTextPhysicalBounds>,
    submission: &PreparedTextSubmission,
) -> Result<(), SharedRendererError> {
    let bounds = physical_clip_bounds.map_or_else(TextBounds::default, TextBounds::from);
    let area = submission.glyph_area(bounds);
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
