use super::{
    NativeCaptureViewport, NativeEffectExecution, NativeFrameCapture, NativeFrameDebugRegion,
    NativeFrameElementBounds, NativeImageDebugQuad, NativeImageQuad, NativeOffscreenCaptureSession,
    NativeOffscreenTextRenderer, NativeRenderLayout, NativeRenderReadback, NativeRenderTarget,
    NativeTextOrigin, NativeWindowError, RichTextEffectDescriptor, RichTextEffectRegistry,
    RichTextMotionRegistry, RichTextShaderRef, RichTextShaderRegistry, RichTextStateStore,
    WindowRichText, clear_transparent_rgb, color_rich_text_for_regions, color_selected_text_ranges,
    debug_rich_text_for_regions, debug_selected_text_ranges, display_map_non_empty_page_range_at,
    fill_native_rect, layout_page_range, layout_page_range_with_selected_text,
    measure_frame_elements_at_page_with_effects, native_default_effect_registry,
    native_default_motion_registry, native_default_shader_registry, native_frame_content_stats,
    native_text_layout_config_at, page_from_display_map_range, post_process_effects_for_page,
    post_process_effects_for_regions, post_process_shaders_for_page,
    post_process_shaders_for_regions, readback_texture_rgba, recolor_image_debug_quad,
    render_image_quads_texture, request_capture_device, solid_rgba,
};
use arcweft_render_text::LineDisplayFrame;
use wgpu::TextureFormat;

impl NativeOffscreenCaptureSession {
    /// Creates a reusable offscreen capture session.
    pub fn new() -> Result<Self, NativeWindowError> {
        let (device, queue) = pollster::block_on(request_capture_device())?;
        let format = TextureFormat::Rgba8UnormSrgb;
        let renderer = NativeOffscreenTextRenderer::new(&device, &queue, format);
        Ok(Self {
            device,
            queue,
            format,
            renderer,
            effect_registry: native_default_effect_registry(),
            shader_registry: native_default_shader_registry(),
            motion_registry: native_default_motion_registry(),
            effect_state: RichTextStateStore::default(),
        })
    }

    /// Mutable registry used by custom rich-text effects during offscreen capture.
    pub fn effect_registry_mut(&mut self) -> &mut RichTextEffectRegistry {
        &mut self.effect_registry
    }

    /// Mutable registry used by rich-text shaders during offscreen capture.
    pub fn shader_registry_mut(&mut self) -> &mut RichTextShaderRegistry {
        &mut self.shader_registry
    }

    /// Mutable registry used by `.motion fn=...` during offscreen capture.
    pub fn motion_registry_mut(&mut self) -> &mut RichTextMotionRegistry {
        &mut self.motion_registry
    }

    /// Mutable state store shared by custom rich-text effects during offscreen capture.
    pub fn effect_state_mut(&mut self) -> &mut RichTextStateStore {
        &mut self.effect_state
    }

    /// Renders RGBA image quads through the native wgpu textured-quad path.
    pub fn capture_image_quads_rgba(
        &mut self,
        quads: &[NativeImageQuad<'_>],
        width: u32,
        height: u32,
    ) -> Result<NativeFrameCapture, NativeWindowError> {
        let width = width.max(1);
        let height = height.max(1);
        let background = [0, 0, 0, 0];
        let texture = render_image_quads_texture(
            &self.device,
            &self.queue,
            quads,
            width,
            height,
            self.format,
            wgpu::Color::TRANSPARENT,
        )?;
        let rgba = readback_texture_rgba(&self.device, &self.queue, &texture, width, height)?;
        let stats = native_frame_content_stats(&rgba, width, height, background);
        Ok(NativeFrameCapture {
            width,
            height,
            rgba,
            content_bbox: stats.content_bbox,
            content_pixels: stats.content_pixels,
            diagnostics: Vec::new(),
        })
    }

    /// Renders image quads recolored for object-id or mask debug captures.
    pub fn capture_image_debug_quads_rgba(
        &mut self,
        quads: &[NativeImageDebugQuad<'_>],
        width: u32,
        height: u32,
    ) -> Result<NativeFrameCapture, NativeWindowError> {
        let recolored = quads
            .iter()
            .map(|quad| recolor_image_debug_quad(*quad))
            .collect::<Result<Vec<_>, NativeWindowError>>()?;
        let render_quads = recolored
            .iter()
            .zip(quads)
            .map(|(rgba, source)| NativeImageQuad {
                width: source.quad.width,
                height: source.quad.height,
                rgba,
                opacity_milli: 1_000,
                dst: source.quad.dst,
                transform: source.quad.transform,
            })
            .collect::<Vec<_>>();
        self.capture_image_quads_rgba(&render_quads, width, height)
    }

    /// Measures first-page rich-text element bounds with this session's custom effects.
    pub fn measure_frame_elements_at(
        &mut self,
        frame: &LineDisplayFrame,
        width: u32,
        height: u32,
        left: f32,
        top: f32,
    ) -> Result<Vec<NativeFrameElementBounds>, NativeWindowError> {
        self.measure_frame_elements_in(
            frame,
            NativeCaptureViewport::new(width, height, left, top, 0),
        )
    }

    /// Measures rich-text element bounds with this session's custom effects.
    pub fn measure_frame_elements_in(
        &mut self,
        frame: &LineDisplayFrame,
        viewport: NativeCaptureViewport,
    ) -> Result<Vec<NativeFrameElementBounds>, NativeWindowError> {
        measure_frame_elements_at_page_with_effects(
            frame,
            viewport,
            viewport.time_seconds,
            Some(&mut self.effect_registry),
            Some(&mut self.shader_registry),
            Some(&mut self.motion_registry),
            &mut self.effect_state,
        )
    }

    /// Renders the first page of a rich-text frame at a viewport origin.
    pub fn capture_frame_rgba_at(
        &mut self,
        frame: &LineDisplayFrame,
        width: u32,
        height: u32,
        left: f32,
        top: f32,
    ) -> Result<NativeFrameCapture, NativeWindowError> {
        self.capture_frame_rgba_in(
            frame,
            NativeCaptureViewport::new(width, height, left, top, 0),
        )
    }

    /// Renders a page of a rich-text frame within a viewport.
    pub fn capture_frame_rgba_in(
        &mut self,
        frame: &LineDisplayFrame,
        viewport: NativeCaptureViewport,
    ) -> Result<NativeFrameCapture, NativeWindowError> {
        let width = viewport.width.max(1);
        let height = viewport.height.max(1);
        let page_range = display_map_non_empty_page_range_at(frame, viewport.page_index)?;
        let page_layout = layout_page_range(
            frame,
            page_range.clone(),
            native_text_layout_config_at(
                width,
                height,
                viewport.left,
                viewport.top,
                viewport.time_seconds,
            ),
        )?;
        let post_process_effects = post_process_effects_for_page(frame, &page_range);
        let post_process_shaders = post_process_shaders_for_page(frame, &page_range);
        let Some(page) = page_from_display_map_range(frame, page_range) else {
            return Err(NativeWindowError::EmptyPages);
        };
        let line_label = frame.line.public_label().into_string();
        self.capture_rich_text_rgba(
            &page.rich_text,
            NativeRenderLayout::glyph_area(&page_layout.layout),
            NativeRenderTarget {
                width,
                height,
                origin: NativeTextOrigin {
                    left: viewport.left,
                    top: viewport.top,
                },
                time_seconds: viewport.time_seconds,
                force_alpha_mask: false,
            },
            line_label.as_str(),
            &post_process_effects,
            &post_process_shaders,
        )
    }

    /// Builds a native-layout debug capture for object-id and mask capture modes.
    pub fn capture_frame_debug_regions_at(
        &mut self,
        frame: &LineDisplayFrame,
        width: u32,
        height: u32,
        left: f32,
        top: f32,
        regions: &[NativeFrameDebugRegion],
    ) -> Result<NativeFrameCapture, NativeWindowError> {
        self.capture_frame_debug_regions_in(
            frame,
            NativeCaptureViewport::new(width, height, left, top, 0),
            regions,
        )
    }

    /// Builds a native-layout debug capture for object-id and mask capture modes on a page.
    pub fn capture_frame_debug_regions_in(
        &mut self,
        frame: &LineDisplayFrame,
        viewport: NativeCaptureViewport,
        regions: &[NativeFrameDebugRegion],
    ) -> Result<NativeFrameCapture, NativeWindowError> {
        let width = viewport.width.max(1);
        let height = viewport.height.max(1);
        let background = [0, 0, 0, 0];
        let mut readback = self
            .capture_debug_text_regions_rgba_at(frame, viewport, regions)?
            .unwrap_or_else(|| NativeRenderReadback {
                rgba: solid_rgba(width, height, background),
                diagnostics: Vec::new(),
            });
        for region in regions {
            if region.element.is_none() {
                fill_native_rect(
                    &mut readback.rgba,
                    width,
                    height,
                    region.fallback_bbox,
                    region.color,
                );
            }
        }
        let stats = native_frame_content_stats(&readback.rgba, width, height, background);
        Ok(NativeFrameCapture {
            width,
            height,
            rgba: readback.rgba,
            content_bbox: stats.content_bbox,
            content_pixels: stats.content_pixels,
            diagnostics: readback.diagnostics,
        })
    }

    /// Builds an isolated native-layout color capture for selected rich-text regions.
    pub fn capture_frame_color_regions_at(
        &mut self,
        frame: &LineDisplayFrame,
        width: u32,
        height: u32,
        left: f32,
        top: f32,
        regions: &[NativeFrameDebugRegion],
    ) -> Result<NativeFrameCapture, NativeWindowError> {
        self.capture_frame_color_regions_in(
            frame,
            NativeCaptureViewport::new(width, height, left, top, 0),
            regions,
        )
    }

    /// Builds an isolated native-layout color capture for selected rich-text regions on a page.
    pub fn capture_frame_color_regions_in(
        &mut self,
        frame: &LineDisplayFrame,
        viewport: NativeCaptureViewport,
        regions: &[NativeFrameDebugRegion],
    ) -> Result<NativeFrameCapture, NativeWindowError> {
        let width = viewport.width.max(1);
        let height = viewport.height.max(1);
        let background = [0, 0, 0, 0];
        let mut readback = self
            .capture_color_text_regions_rgba_at(frame, viewport, regions)?
            .unwrap_or_else(|| NativeRenderReadback {
                rgba: solid_rgba(width, height, background),
                diagnostics: Vec::new(),
            });
        let page_range = display_map_non_empty_page_range_at(frame, viewport.page_index)?;
        let post_process_effects = post_process_effects_for_regions(frame, &page_range, regions);
        let post_process_shaders = post_process_shaders_for_regions(frame, &page_range, regions);
        let line_label = frame.line.public_label().into_string();
        self.apply_post_process_effects(
            line_label.as_str(),
            &mut readback,
            width,
            height,
            viewport.time_seconds,
            &post_process_effects,
        );
        self.apply_post_process_shaders(
            &mut readback,
            width,
            height,
            viewport.time_seconds,
            &post_process_shaders,
        );
        let stats = native_frame_content_stats(&readback.rgba, width, height, background);
        Ok(NativeFrameCapture {
            width,
            height,
            rgba: readback.rgba,
            content_bbox: stats.content_bbox,
            content_pixels: stats.content_pixels,
            diagnostics: readback.diagnostics,
        })
    }

    fn capture_rich_text_rgba(
        &mut self,
        rich_text: &WindowRichText,
        layout: NativeRenderLayout<'_>,
        target: NativeRenderTarget,
        line_id: &str,
        post_process_effects: &[RichTextEffectDescriptor],
        post_process_shaders: &[RichTextShaderRef],
    ) -> Result<NativeFrameCapture, NativeWindowError> {
        let mut readback =
            self.render_rich_text_rgba_with_clear(rich_text, layout, target, wgpu::Color::BLACK)?;
        self.apply_post_process_effects(
            line_id,
            &mut readback,
            target.width,
            target.height,
            target.time_seconds,
            post_process_effects,
        );
        self.apply_post_process_shaders(
            &mut readback,
            target.width,
            target.height,
            target.time_seconds,
            post_process_shaders,
        );
        let stats =
            native_frame_content_stats(&readback.rgba, target.width, target.height, [0, 0, 0, 255]);
        Ok(NativeFrameCapture {
            width: target.width,
            height: target.height,
            rgba: readback.rgba,
            content_bbox: stats.content_bbox,
            content_pixels: stats.content_pixels,
            diagnostics: readback.diagnostics,
        })
    }

    fn apply_post_process_effects(
        &mut self,
        line_id: &str,
        readback: &mut NativeRenderReadback,
        width: u32,
        height: u32,
        time_seconds: f32,
        effects: &[RichTextEffectDescriptor],
    ) {
        if effects.is_empty() {
            return;
        }
        let mut execution = NativeEffectExecution::new(
            Some(&mut self.effect_registry),
            None,
            None,
            &mut self.effect_state,
        );
        execution.apply_effect_post_processes(
            line_id,
            effects.iter(),
            width,
            height,
            time_seconds,
            &mut readback.rgba,
        );
        readback.diagnostics.extend(execution.into_diagnostics());
    }

    fn apply_post_process_shaders(
        &mut self,
        readback: &mut NativeRenderReadback,
        width: u32,
        height: u32,
        time_seconds: f32,
        shaders: &[RichTextShaderRef],
    ) {
        if shaders.is_empty() {
            return;
        }
        let mut effects = NativeEffectExecution::new(
            None,
            Some(&mut self.shader_registry),
            None,
            &mut self.effect_state,
        );
        effects.apply_shader_post_processes(
            shaders.iter(),
            width,
            height,
            time_seconds,
            &mut readback.rgba,
        );
        readback.diagnostics.extend(effects.into_diagnostics());
    }

    fn capture_debug_text_regions_rgba_at(
        &mut self,
        frame: &LineDisplayFrame,
        viewport: NativeCaptureViewport,
        regions: &[NativeFrameDebugRegion],
    ) -> Result<Option<NativeRenderReadback>, NativeWindowError> {
        let width = viewport.width.max(1);
        let height = viewport.height.max(1);
        let origin = NativeTextOrigin {
            left: viewport.left,
            top: viewport.top,
        };
        let page_range = display_map_non_empty_page_range_at(frame, viewport.page_index)?;
        let selected_text = debug_selected_text_ranges(frame, &page_range, regions)
            .into_iter()
            .map(|(range, _)| range)
            .collect::<Vec<_>>();
        let page_layout = layout_page_range_with_selected_text(
            frame,
            page_range.clone(),
            native_text_layout_config_at(
                width,
                height,
                origin.left,
                origin.top,
                viewport.time_seconds,
            ),
            &selected_text,
        )?;
        let Some(page) = page_from_display_map_range(frame, page_range.clone()) else {
            return Err(NativeWindowError::EmptyPages);
        };
        let Some(rich_text) =
            debug_rich_text_for_regions(frame, &page_range, &page.rich_text, regions)
        else {
            return Ok(None);
        };
        let mut readback = self.render_rich_text_rgba_with_clear(
            &rich_text,
            NativeRenderLayout::glyph_area(&page_layout.layout),
            NativeRenderTarget {
                width,
                height,
                origin,
                time_seconds: viewport.time_seconds,
                force_alpha_mask: true,
            },
            wgpu::Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.0,
            },
        )?;
        clear_transparent_rgb(&mut readback.rgba);
        Ok(Some(readback))
    }

    fn capture_color_text_regions_rgba_at(
        &mut self,
        frame: &LineDisplayFrame,
        viewport: NativeCaptureViewport,
        regions: &[NativeFrameDebugRegion],
    ) -> Result<Option<NativeRenderReadback>, NativeWindowError> {
        let width = viewport.width.max(1);
        let height = viewport.height.max(1);
        let origin = NativeTextOrigin {
            left: viewport.left,
            top: viewport.top,
        };
        let page_range = display_map_non_empty_page_range_at(frame, viewport.page_index)?;
        let selected_text = color_selected_text_ranges(frame, &page_range, regions);
        let page_layout = layout_page_range_with_selected_text(
            frame,
            page_range.clone(),
            native_text_layout_config_at(
                width,
                height,
                origin.left,
                origin.top,
                viewport.time_seconds,
            ),
            &selected_text,
        )?;
        let Some(page) = page_from_display_map_range(frame, page_range.clone()) else {
            return Err(NativeWindowError::EmptyPages);
        };
        let Some(rich_text) =
            color_rich_text_for_regions(frame, &page_range, &page.rich_text, regions)
        else {
            return Ok(None);
        };
        let mut readback = self.render_rich_text_rgba_with_clear(
            &rich_text,
            NativeRenderLayout::glyph_area(&page_layout.layout),
            NativeRenderTarget {
                width,
                height,
                origin,
                time_seconds: viewport.time_seconds,
                force_alpha_mask: false,
            },
            wgpu::Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.0,
            },
        )?;
        clear_transparent_rgb(&mut readback.rgba);
        Ok(Some(readback))
    }

    fn render_rich_text_rgba_with_clear(
        &mut self,
        rich_text: &WindowRichText,
        layout: NativeRenderLayout<'_>,
        target: NativeRenderTarget,
        clear: wgpu::Color,
    ) -> Result<NativeRenderReadback, NativeWindowError> {
        let mut effects = NativeEffectExecution::new(
            Some(&mut self.effect_registry),
            Some(&mut self.shader_registry),
            Some(&mut self.motion_registry),
            &mut self.effect_state,
        );
        self.renderer.prepare(
            &self.device,
            &self.queue,
            rich_text,
            layout,
            target,
            Some(&mut effects),
        )?;
        let texture = self.renderer.render_texture_with_clear(
            &self.device,
            &self.queue,
            target.width,
            target.height,
            self.format,
            clear,
        )?;
        let rgba = readback_texture_rgba(
            &self.device,
            &self.queue,
            &texture,
            target.width,
            target.height,
        )?;
        Ok(NativeRenderReadback {
            rgba,
            diagnostics: effects.into_diagnostics(),
        })
    }
}
