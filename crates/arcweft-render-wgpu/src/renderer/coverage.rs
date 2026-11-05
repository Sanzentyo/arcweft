//! Cropped coverage targets preserve the original painter graph and raster scale.

use super::{
    SharedRenderer, SharedRendererError, ViewCompositorError, ViewCompositorFrame,
    ViewCompositorTarget, WgpuPreparedViewMaskTextureProvider, WgpuViewPreparedTextRenderer,
    prepared_text::render_prepared_text_item_with_affine,
};
use crate::geometry::PreparedFrame;
use arcweft_glyphon::{PreparedTextAffine, PreparedTextPhysicalBounds};
use arcweft_text_layout::LayoutRect;

impl SharedRenderer {
    pub(super) fn render_coverage_region_to_view(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target: ViewCompositorTarget<'_>,
        frame: &PreparedFrame,
    ) -> Result<(), SharedRendererError> {
        // The offscreen capture plan has already validated and clipped these
        // integer coordinates to this frame. Translating by whole physical
        // pixels preserves atlas rasterization and fractional glyph placement.
        let scale = frame.viewport.physical_scale_factor_f32();
        let origin_logical = target.origin_logical;
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("arcweft-prepared-text-coverage-region"),
        });
        super::clear_texture_view(
            &mut encoder,
            target.view,
            wgpu::Color::TRANSPARENT,
            "arcweft-coverage-region-clear",
        );
        self.aux_text_renderers.clear();
        for prepared in frame.view_scenes() {
            let mut masks =
                WgpuPreparedViewMaskTextureProvider::prepare(device, queue, &prepared.resources)?;
            let mut direct = self.view_direct_renderer.for_resources(&prepared.resources);
            let mut text = WgpuViewPreparedTextRenderer::new(
                frame,
                self.prepared_text_engine.as_mut(),
                &self.glyphon_cache,
                &mut self.atlas,
                &mut self.aux_text_renderers,
                &mut self.view_text_effect_compositor,
                &self.view_direct_renderer,
                scale,
            );
            self.view_compositor.render_scene_region(
                &mut ViewCompositorFrame {
                    device,
                    queue,
                    encoder: &mut encoder,
                    final_target: target.view,
                    scene: &prepared.scene,
                    target_extent: target.extent,
                    device_pixel_ratio: scale,
                    direct_renderer: &mut direct,
                    text_renderer: &mut text,
                    mask_textures: &mut masks,
                },
                origin_logical,
                target.logical_extent,
            )?;
        }
        for (id, item) in frame.text.iter() {
            if item.paint.glyphs.iter().all(|paint| !paint.visible) {
                continue;
            }
            if frame.view_scenes().iter().any(|prepared| {
                prepared
                    .scene
                    .prepared_text_ids()
                    .any(|painted| painted == id)
            }) {
                continue;
            }
            let affine = PreparedTextAffine::try_new(
                [1.0, 0.0, 0.0, 1.0, -origin_logical[0], -origin_logical[1]],
                1.0,
            )
            .map_err(|error| SharedRendererError::TextPrepare(error.to_string()))?;
            let clip = item
                .clip
                .map(|clip| {
                    PreparedTextPhysicalBounds::try_from_logical(
                        LayoutRect::new(
                            clip.x - origin_logical[0],
                            clip.y - origin_logical[1],
                            clip.width,
                            clip.height,
                        ),
                        item.raster_scale(),
                    )
                })
                .transpose()
                .map_err(ViewCompositorError::from)?;
            render_prepared_text_item_with_affine(
                device,
                queue,
                &mut encoder,
                &mut self.aux_text_renderers,
                self.prepared_text_engine.as_mut(),
                &self.glyphon_cache,
                &mut self.atlas,
                &mut self.view_text_effect_compositor,
                target,
                item,
                affine,
                clip,
                scale,
            )?;
        }
        queue.submit([encoder.finish()]);
        self.atlas.trim();
        Ok(())
    }
}
