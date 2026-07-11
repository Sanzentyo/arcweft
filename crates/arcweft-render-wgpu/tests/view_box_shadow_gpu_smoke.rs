use arcweft_presentation::hit::HitRect;
use arcweft_render_wgpu::view_compositor::{
    ViewCompositor, ViewCompositorError, ViewCompositorFrame, ViewDirectPrimitiveRenderer,
    ViewDirectRenderFrame, ViewNoMaskTextures, ViewTextRenderFrame, ViewTextRenderer,
};
use arcweft_render_wgpu::view_effects::ViewTextureExtent;
use arcweft_render_wgpu::view_scene::{
    PreparedTextId, ViewAffine2D, ViewBoxShadow, ViewBoxShadowCornerRadius, ViewBoxShadowList,
    ViewBoxShadowRadii, ViewColorRgba8, ViewCompositingEffects, ViewCompositingGroup,
    ViewPaintNode, ViewPrimitiveRange, ViewScene, ViewSceneContext,
};

struct NoopDirectRenderer;

impl ViewDirectPrimitiveRenderer for NoopDirectRenderer {
    fn render_direct_range(
        &mut self,
        _frame: &mut ViewDirectRenderFrame<'_>,
    ) -> Result<(), ViewCompositorError> {
        Ok(())
    }
}

struct NoopTextRenderer;

impl ViewTextRenderer for NoopTextRenderer {
    fn render_text(
        &mut self,
        _frame: &mut ViewTextRenderFrame<'_>,
        _text: PreparedTextId,
    ) -> Result<(), ViewCompositorError> {
        Ok(())
    }
}

fn rgba(red: u8, green: u8, blue: u8, alpha: u8) -> ViewColorRgba8 {
    ViewColorRgba8 {
        red,
        green,
        blue,
        alpha,
    }
}

fn direct(start: u32, end: u32) -> ViewPaintNode {
    ViewPaintNode::Direct(ViewSceneContext {
        transform: ViewAffine2D::IDENTITY,
        opacity: 1.0,
        clip: None,
        primitive_range: ViewPrimitiveRange { start, end },
    })
}

fn radii(
    top_left: (f32, f32),
    top_right: (f32, f32),
    bottom_right: (f32, f32),
    bottom_left: (f32, f32),
) -> ViewBoxShadowRadii {
    ViewBoxShadowRadii::from_corners(
        ViewBoxShadowCornerRadius::new(top_left.0, top_left.1),
        ViewBoxShadowCornerRadius::new(top_right.0, top_right.1),
        ViewBoxShadowCornerRadius::new(bottom_right.0, bottom_right.1),
        ViewBoxShadowCornerRadius::new(bottom_left.0, bottom_left.1),
    )
}

fn gpu_context() -> Option<(wgpu::Device, wgpu::Queue)> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface: None,
        force_fallback_adapter: false,
    }))
    .ok()?;

    pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("arcweft-view-box-shadow-gpu-smoke"),
        required_features: wgpu::Features::empty(),
        required_limits: wgpu::Limits::downlevel_defaults(),
        experimental_features: wgpu::ExperimentalFeatures::disabled(),
        memory_hints: wgpu::MemoryHints::Performance,
        trace: wgpu::Trace::Off,
    }))
    .ok()
}

fn smoke_scene() -> ViewScene {
    let mut scene = ViewScene::new(320.0, 180.0);

    scene.push_paint_node(ViewPaintNode::Group(
        ViewCompositingGroup::new(
            HitRect::new(24.0, 24.0, 112.0, 72.0),
            ViewCompositingEffects {
                box_shadows: ViewBoxShadowList::new([ViewBoxShadow::inset_with_radii(
                    0.0,
                    3.0,
                    12.0,
                    2.0,
                    radii((18.0, 7.0), (6.0, 16.0), (20.0, 9.0), (8.0, 14.0)),
                    rgba(0, 0, 0, 144),
                )]),
                ..ViewCompositingEffects::default()
            },
        )
        .with_children(vec![direct(0, 0)]),
    ));

    scene.push_paint_node(ViewPaintNode::Group(
        ViewCompositingGroup::new(
            HitRect::new(176.0, 40.0, 112.0, 72.0),
            ViewCompositingEffects {
                box_shadows: ViewBoxShadowList::new([
                    ViewBoxShadow::outer_with_radii(
                        0.0,
                        10.0,
                        18.0,
                        2.0,
                        radii((24.0, 10.0), (8.0, 20.0), (16.0, 6.0), (4.0, 14.0)),
                        rgba(0, 0, 0, 96),
                    ),
                    ViewBoxShadow::inset(0.0, -2.0, 10.0, 1.0, 16.0, rgba(255, 255, 255, 88)),
                ]),
                ..ViewCompositingEffects::default()
            },
        )
        .with_children(vec![direct(0, 0)]),
    ));

    scene
}

#[test]
#[ignore = "requires a local wgpu adapter; exact PNG promotion remains manual"]
fn per_corner_outer_and_elliptical_inset_shadow_cards_execute_gpu_compositor_path() {
    let Some((device, queue)) = gpu_context() else {
        eprintln!("no compatible wgpu adapter available for seq06.13e smoke");
        return;
    };

    let format = wgpu::TextureFormat::Rgba8Unorm;
    let extent = ViewTextureExtent::new(320, 180);
    let final_texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("arcweft-view-box-shadow-smoke-target"),
        size: wgpu::Extent3d {
            width: extent.width,
            height: extent.height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let final_view = final_texture.create_view(&wgpu::TextureViewDescriptor::default());
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("arcweft-view-box-shadow-smoke-encoder"),
    });
    let scene = smoke_scene();
    let mut direct_renderer = NoopDirectRenderer;
    let mut text_renderer = NoopTextRenderer;
    let mut mask_textures = ViewNoMaskTextures;
    let mut compositor = ViewCompositor::new(&device, &queue, format);
    let mut frame = ViewCompositorFrame {
        device: &device,
        queue: &queue,
        encoder: &mut encoder,
        final_target: &final_view,
        scene: &scene,
        target_extent: extent,
        device_pixel_ratio: 1.0,
        direct_renderer: &mut direct_renderer,
        text_renderer: &mut text_renderer,
        mask_textures: &mut mask_textures,
    };

    let stats = compositor
        .render_scene(&mut frame)
        .expect("seq06.13e inset box-shadow smoke should render");
    queue.submit([encoder.finish()]);

    assert_eq!(stats.box_shadow_passes, 3);
    assert!(stats.shader_passes >= 6);
}
