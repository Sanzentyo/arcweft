use arcweft_presentation::hit::HitRect;
use arcweft_render_wgpu::ui_compositor::{
    UiCompositor, UiCompositorError, UiCompositorFrame, UiCompositorTarget,
    UiDirectPrimitiveRenderer, UiNoMaskTextures,
};
use arcweft_render_wgpu::ui_effects::UiTextureExtent;
use arcweft_render_wgpu::ui_scene::{
    UiAffine2D, UiBoxShadow, UiBoxShadowList, UiColorRgba8, UiCompositingEffects,
    UiCompositingGroup, UiPaintNode, UiPrimitiveRange, UiScene, UiSceneContext,
};

struct NoopDirectRenderer;

impl UiDirectPrimitiveRenderer for NoopDirectRenderer {
    fn render_direct_range(
        &mut self,
        _device: &wgpu::Device,
        _queue: &wgpu::Queue,
        _encoder: &mut wgpu::CommandEncoder,
        _scene: &UiScene,
        _context: &UiSceneContext,
        _target: UiCompositorTarget<'_>,
    ) -> Result<(), UiCompositorError> {
        Ok(())
    }
}

fn rgba(red: u8, green: u8, blue: u8, alpha: u8) -> UiColorRgba8 {
    UiColorRgba8 {
        red,
        green,
        blue,
        alpha,
    }
}

fn direct(start: u32, end: u32) -> UiPaintNode {
    UiPaintNode::Direct(UiSceneContext {
        transform: UiAffine2D::IDENTITY,
        opacity: 1.0,
        clip: None,
        primitive_range: UiPrimitiveRange { start, end },
    })
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
        label: Some("arcweft-ui-box-shadow-gpu-smoke"),
        required_features: wgpu::Features::empty(),
        required_limits: wgpu::Limits::downlevel_defaults(),
        experimental_features: wgpu::ExperimentalFeatures::disabled(),
        memory_hints: wgpu::MemoryHints::Performance,
        trace: wgpu::Trace::Off,
    }))
    .ok()
}

fn smoke_scene() -> UiScene {
    let mut scene = UiScene::new(320.0, 180.0);

    scene.push_paint_node(UiPaintNode::Group(
        UiCompositingGroup::new(
            HitRect::new(24.0, 24.0, 112.0, 72.0),
            UiCompositingEffects {
                box_shadows: UiBoxShadowList::new([UiBoxShadow::inset(
                    0.0,
                    3.0,
                    12.0,
                    2.0,
                    14.0,
                    rgba(0, 0, 0, 144),
                )]),
                ..UiCompositingEffects::default()
            },
        )
        .with_children(vec![direct(0, 0)]),
    ));

    scene.push_paint_node(UiPaintNode::Group(
        UiCompositingGroup::new(
            HitRect::new(176.0, 40.0, 112.0, 72.0),
            UiCompositingEffects {
                box_shadows: UiBoxShadowList::new([
                    UiBoxShadow::outer(0.0, 10.0, 18.0, 2.0, 16.0, rgba(0, 0, 0, 96)),
                    UiBoxShadow::inset(0.0, -2.0, 10.0, 1.0, 16.0, rgba(255, 255, 255, 88)),
                ]),
                ..UiCompositingEffects::default()
            },
        )
        .with_children(vec![direct(0, 0)]),
    ));

    scene
}

#[test]
#[ignore = "requires a local wgpu adapter; exact PNG promotion remains manual"]
fn rounded_inset_and_mixed_shadow_cards_execute_gpu_compositor_path() {
    let Some((device, queue)) = gpu_context() else {
        eprintln!("no compatible wgpu adapter available for seq06.13e smoke");
        return;
    };

    let format = wgpu::TextureFormat::Rgba8Unorm;
    let extent = UiTextureExtent::new(320, 180);
    let final_texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("arcweft-ui-box-shadow-smoke-target"),
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
        label: Some("arcweft-ui-box-shadow-smoke-encoder"),
    });
    let scene = smoke_scene();
    let mut direct_renderer = NoopDirectRenderer;
    let mut mask_textures = UiNoMaskTextures;
    let mut compositor = UiCompositor::new(&device, &queue, format);
    let mut frame = UiCompositorFrame {
        device: &device,
        queue: &queue,
        encoder: &mut encoder,
        final_target: &final_view,
        scene: &scene,
        target_extent: extent,
        direct_renderer: &mut direct_renderer,
        mask_textures: &mut mask_textures,
    };

    let stats = compositor
        .render_scene(&mut frame)
        .expect("seq06.13e inset box-shadow smoke should render");
    queue.submit([encoder.finish()]);

    assert_eq!(stats.box_shadow_passes, 3);
    assert!(stats.shader_passes >= 6);
}
