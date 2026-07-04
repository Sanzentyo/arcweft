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
use js_sys::{Object, Reflect, Uint8Array};
use std::fmt::Write as _;
use wasm_bindgen::prelude::*;

const WIDTH: u32 = 320;
const HEIGHT: u32 = 180;
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// Captures the seq06.13e.1 inset box-shadow exact fixture through the portable
/// Arcweft WGPU compositor in a browser WebGPU runtime.
///
/// The returned object contains raw RGBA pixels, observe JSON, and adapter
/// evidence. JavaScript owns PNG encoding and filesystem writes so the browser
/// side never uses DOM/CSS screenshots, SVG filters, Canvas 2D, or CPU raster
/// replacement as the visual source.
#[wasm_bindgen]
pub async fn capture_seq06_13e1_inset_box_shadow_exact_png() -> Result<JsValue, JsValue> {
    capture_js_value()
        .await
        .map_err(|error| JsValue::from_str(&error))
}

async fn capture_js_value() -> Result<JsValue, String> {
    let capture = render_capture().await?;
    let object = Object::new();
    set_property(&object, "width", JsValue::from_f64(f64::from(WIDTH)))?;
    set_property(&object, "height", JsValue::from_f64(f64::from(HEIGHT)))?;
    set_property(&object, "format", JsValue::from_str("rgba8unorm"))?;
    set_property(
        &object,
        "rgba",
        Uint8Array::from(capture.rgba.as_slice()).into(),
    )?;
    set_property(
        &object,
        "observe_json",
        JsValue::from_str(&observe_json(&capture)),
    )?;
    set_property(
        &object,
        "adapter_info_json",
        JsValue::from_str(&adapter_info_json(&capture.adapter)),
    )?;
    Ok(object.into())
}

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

struct CaptureOutput {
    rgba: Vec<u8>,
    stats: arcweft_render_wgpu::ui_compositor::UiCompositorStats,
    adapter: wgpu::AdapterInfo,
    content: ContentStats,
}

#[derive(Clone, Copy, Debug, Default)]
struct ContentStats {
    non_transparent_pixels: u32,
    max_alpha: u8,
    max_channel: u8,
}

async fn render_capture() -> Result<CaptureOutput, String> {
    let instance = wgpu::Instance::default();
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: None,
            force_fallback_adapter: false,
        })
        .await
        .map_err(|error| format!("request browser WebGPU adapter: {error}"))?;
    let adapter_info = adapter.get_info();
    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("arcweft-seq06-13e1-web-exact-device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            ..Default::default()
        })
        .await
        .map_err(|error| format!("request browser WebGPU device: {error}"))?;

    let extent = UiTextureExtent::new(WIDTH, HEIGHT);
    let final_texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("arcweft-seq06-13e1-web-exact-target"),
        size: wgpu::Extent3d {
            width: extent.width,
            height: extent.height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let final_view = final_texture.create_view(&wgpu::TextureViewDescriptor::default());
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("arcweft-seq06-13e1-web-exact-render-encoder"),
    });
    clear_target(&mut encoder, &final_view);

    let scene = smoke_scene();
    let mut direct_renderer = NoopDirectRenderer;
    let mut mask_textures = UiNoMaskTextures;
    let mut compositor = UiCompositor::new(&device, &queue, FORMAT);
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
        .map_err(|error| format!("render seq06.13e.1 browser compositor scene: {error}"))?;
    let padded_row_bytes = padded_rgba_row_bytes(WIDTH);
    let readback = create_readback_buffer(&device, HEIGHT, padded_row_bytes);
    copy_texture_to_readback(
        &mut encoder,
        &final_texture,
        &readback,
        WIDTH,
        HEIGHT,
        padded_row_bytes,
    );
    queue.submit([encoder.finish()]);
    let rgba = map_readback_buffer(&readback, WIDTH, HEIGHT, padded_row_bytes).await?;
    let content = content_stats(&rgba);
    if content.non_transparent_pixels == 0 {
        return Err(String::from(
            "browser WebGPU readback produced a fully transparent seq06.13e.1 candidate",
        ));
    }
    Ok(CaptureOutput {
        rgba,
        stats,
        adapter: adapter_info,
        content,
    })
}

fn smoke_scene() -> UiScene {
    let mut scene = UiScene::new(WIDTH as f32, HEIGHT as f32);

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

fn clear_target(encoder: &mut wgpu::CommandEncoder, target: &wgpu::TextureView) {
    let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("arcweft-seq06-13e1-web-exact-clear"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: target,
            resolve_target: None,
            depth_slice: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
}

fn create_readback_buffer(
    device: &wgpu::Device,
    height: u32,
    padded_row_bytes: u32,
) -> wgpu::Buffer {
    let buffer_size = u64::from(padded_row_bytes).saturating_mul(u64::from(height));
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("arcweft-seq06-13e1-web-exact-readback"),
        size: buffer_size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    })
}

fn copy_texture_to_readback(
    encoder: &mut wgpu::CommandEncoder,
    texture: &wgpu::Texture,
    readback: &wgpu::Buffer,
    width: u32,
    height: u32,
    padded_row_bytes: u32,
) {
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded_row_bytes),
                rows_per_image: Some(height),
            },
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
}

async fn map_readback_buffer(
    readback: &wgpu::Buffer,
    width: u32,
    height: u32,
    padded_row_bytes: u32,
) -> Result<Vec<u8>, String> {
    let slice = readback.slice(..);
    let (sender, receiver) = futures_channel::oneshot::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = sender.send(result.map_err(|error| error.to_string()));
    });
    receiver
        .await
        .map_err(|_| String::from("readback map callback was dropped"))?
        .map_err(|error| format!("map browser WebGPU readback buffer: {error}"))?;

    let mapped = slice.get_mapped_range();
    let rgba = unpad_rgba_rows(&mapped, width, height, padded_row_bytes);
    drop(mapped);
    readback.unmap();
    Ok(rgba)
}

fn padded_rgba_row_bytes(width: u32) -> u32 {
    let row_bytes = width.saturating_mul(4);
    row_bytes.saturating_add(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT - 1)
        / wgpu::COPY_BYTES_PER_ROW_ALIGNMENT
        * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT
}

fn unpad_rgba_rows(mapped: &[u8], width: u32, height: u32, padded_row_bytes: u32) -> Vec<u8> {
    let row_bytes = usize::try_from(width.saturating_mul(4)).unwrap_or(0);
    let padded = usize::try_from(padded_row_bytes).unwrap_or(row_bytes);
    (0..usize::try_from(height).unwrap_or(0))
        .flat_map(|row| {
            let start = row.saturating_mul(padded);
            let end = start.saturating_add(row_bytes).min(mapped.len());
            mapped[start..end].iter().copied()
        })
        .collect()
}

fn observe_json(capture: &CaptureOutput) -> String {
    let mut json = String::new();
    writeln!(&mut json, "{{").unwrap();
    writeln!(
        &mut json,
        "  \"schema\": \"arcweft.seq06.13e1.inset_box_shadow.web_observe.v1\","
    )
    .unwrap();
    writeln!(
        &mut json,
        "  \"generated_unix_seconds\": {},",
        unix_seconds()
    )
    .unwrap();
    writeln!(&mut json, "  \"target\": \"web\",").unwrap();
    writeln!(&mut json, "  \"fixture\": \"seq06_13e1_inset_box_shadow\",").unwrap();
    writeln!(&mut json, "  \"candidate_png\": null,").unwrap();
    writeln!(&mut json, "  \"viewport\": {{").unwrap();
    writeln!(&mut json, "    \"width\": {WIDTH},").unwrap();
    writeln!(&mut json, "    \"height\": {HEIGHT},").unwrap();
    writeln!(&mut json, "    \"device_pixel_ratio\": 1.0").unwrap();
    writeln!(&mut json, "  }},").unwrap();
    writeln!(&mut json, "  \"route\": [").unwrap();
    let route_entries = [
        "UiCompositingEffects::box_shadows",
        "UiBoxShadowPassPlan unified outer/inset pass list",
        "UiCompositor::render_group",
        "PASS_BOX_SHADOW WGSL kind flag",
        "WebAssembly-exported Arcweft WGPU texture copy/readback",
    ];
    for (index, route_entry) in route_entries.iter().enumerate() {
        let comma = if index + 1 == route_entries.len() {
            ""
        } else {
            ","
        };
        writeln!(&mut json, "    {}{comma}", json_string(route_entry)).unwrap();
    }
    writeln!(&mut json, "  ],").unwrap();
    writeln!(&mut json, "  \"scene\": {{").unwrap();
    writeln!(
        &mut json,
        "    \"cards\": [\"rounded_inset_shadow_card\", \"mixed_outer_inset_shadow_card\"],"
    )
    .unwrap();
    writeln!(&mut json, "    \"box_shadow\": \"outer and inset\",").unwrap();
    writeln!(&mut json, "    \"inset\": true").unwrap();
    writeln!(&mut json, "  }},").unwrap();
    writeln!(&mut json, "  \"readback\": {{").unwrap();
    writeln!(
        &mut json,
        "    \"source\": \"Arcweft-owned WebGPU texture\","
    )
    .unwrap();
    writeln!(&mut json, "    \"operation\": \"copy_texture_to_buffer\",").unwrap();
    writeln!(&mut json, "    \"format\": \"rgba8unorm\"").unwrap();
    writeln!(&mut json, "  }},").unwrap();
    writeln!(&mut json, "  \"stats\": {{").unwrap();
    writeln!(
        &mut json,
        "    \"direct_ranges\": {},",
        capture.stats.direct_ranges
    )
    .unwrap();
    writeln!(
        &mut json,
        "    \"offscreen_targets\": {},",
        capture.stats.offscreen_targets
    )
    .unwrap();
    writeln!(
        &mut json,
        "    \"shader_passes\": {},",
        capture.stats.shader_passes
    )
    .unwrap();
    writeln!(
        &mut json,
        "    \"backdrop_copies\": {},",
        capture.stats.backdrop_copies
    )
    .unwrap();
    writeln!(
        &mut json,
        "    \"pool_reuses\": {},",
        capture.stats.pool_reuses
    )
    .unwrap();
    writeln!(
        &mut json,
        "    \"clip_passes\": {},",
        capture.stats.clip_passes
    )
    .unwrap();
    writeln!(
        &mut json,
        "    \"box_shadow_passes\": {}",
        capture.stats.box_shadow_passes
    )
    .unwrap();
    writeln!(&mut json, "  }},").unwrap();
    writeln!(&mut json, "  \"content\": {{").unwrap();
    writeln!(
        &mut json,
        "    \"non_transparent_pixels\": {},",
        capture.content.non_transparent_pixels
    )
    .unwrap();
    writeln!(
        &mut json,
        "    \"max_alpha\": {},",
        capture.content.max_alpha
    )
    .unwrap();
    writeln!(
        &mut json,
        "    \"max_channel\": {}",
        capture.content.max_channel
    )
    .unwrap();
    writeln!(&mut json, "  }},").unwrap();
    writeln!(
        &mut json,
        "  \"adapter\": {}",
        adapter_info_json(&capture.adapter)
    )
    .unwrap();
    writeln!(&mut json, "}}").unwrap();
    json
}

fn adapter_info_json(adapter: &wgpu::AdapterInfo) -> String {
    format!(
        "{{\n    \"name\": {},\n    \"vendor\": {},\n    \"device\": {},\n    \"device_type\": {},\n    \"backend\": {},\n    \"driver\": {},\n    \"driver_info\": {}\n  }}",
        json_string(&adapter.name),
        adapter.vendor,
        adapter.device,
        json_string(&format!("{:?}", adapter.device_type)),
        json_string(&format!("{:?}", adapter.backend)),
        json_string(&adapter.driver),
        json_string(&adapter.driver_info)
    )
}

fn content_stats(rgba: &[u8]) -> ContentStats {
    rgba.chunks_exact(4)
        .fold(ContentStats::default(), |mut stats, pixel| {
            let alpha = pixel[3];
            if alpha != 0 {
                stats.non_transparent_pixels = stats.non_transparent_pixels.saturating_add(1);
            }
            stats.max_alpha = stats.max_alpha.max(alpha);
            stats.max_channel = stats.max_channel.max(pixel[0]).max(pixel[1]).max(pixel[2]);
            stats
        })
}

fn set_property(object: &Object, name: &str, value: JsValue) -> Result<(), String> {
    Reflect::set(object, &JsValue::from_str(name), &value)
        .map_err(|error| format!("set capture result property `{name}`: {error:?}"))?;
    Ok(())
}

fn unix_seconds() -> u64 {
    (js_sys::Date::now() / 1_000.0).floor().max(0.0) as u64
}

fn json_string(value: &str) -> String {
    format!("\"{}\"", escape_json(value))
}

fn escape_json(value: &str) -> String {
    let mut escaped = String::new();
    for character in value.chars() {
        match character {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            c if c.is_control() => write!(&mut escaped, "\\u{:04x}", c as u32).unwrap(),
            c => escaped.push(c),
        }
    }
    escaped
}
