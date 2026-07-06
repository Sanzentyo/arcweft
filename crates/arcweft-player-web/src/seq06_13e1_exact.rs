use arcweft_bundle::resource_codec::ui::{
    RgbaColor, StyleAssignOp, StyleSourceIdentity, StyleSourceRef, StyleSyntax,
    UiRuntimeControlState, UiRuntimeControlVisualStyle, UiRuntimeShadow, UiRuntimeShadowKind,
    UiStyleResource, ViewElementKind, ViewStyleDeclaration, ViewStyleRule, ViewStyleSelector,
    ViewStyleSelectorPart, ViewStyleValue,
};
use arcweft_presentation::hit::HitRect;
use arcweft_render_wgpu::geometry::PreparedUiSceneResources;
use arcweft_render_wgpu::ui_compositor::{UiCompositor, UiCompositorFrame, UiNoMaskTextures};
use arcweft_render_wgpu::ui_direct_renderer::WgpuUiDirectPrimitiveRenderer;
use arcweft_render_wgpu::ui_effects::UiTextureExtent;
use arcweft_render_wgpu::ui_scene::{
    UiAffine2D, UiBoxShadow, UiBoxShadowList, UiColorRgba8, UiCompositingEffects,
    UiCompositingGroup, UiPaintNode, UiPrimitive, UiPrimitiveRange, UiRoundedRect, UiScene,
    UiSceneContext,
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
    let validation_scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
    clear_target(&mut encoder, &final_view);

    let scene = smoke_scene()?;
    let direct_renderer_backend = WgpuUiDirectPrimitiveRenderer::new(&device, FORMAT);
    let resources = PreparedUiSceneResources::default();
    let mut direct_renderer = direct_renderer_backend.for_resources(&resources);
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
    wait_for_submitted_work(&queue).await?;
    if let Some(error) = validation_scope.pop().await {
        return Err(format!(
            "browser WebGPU validation error during seq06.13e.1 exact capture: {error}"
        ));
    }
    let rgba = map_readback_buffer(&readback, WIDTH, HEIGHT, padded_row_bytes).await?;
    let content = content_stats(&rgba);
    if content.non_transparent_pixels == 0 {
        return Err(format!(
            "browser WebGPU readback produced a fully transparent seq06.13e.1 candidate; adapter={} backend={:?} driver={}; direct_ranges={}; offscreen_targets={}; shader_passes={}; box_shadow_passes={}; max_channel={}",
            adapter_info.name,
            adapter_info.backend,
            adapter_info.driver,
            stats.direct_ranges,
            stats.offscreen_targets,
            stats.shader_passes,
            stats.box_shadow_passes,
            content.max_channel,
        ));
    }
    Ok(CaptureOutput {
        rgba,
        stats,
        adapter: adapter_info,
        content,
    })
}

fn smoke_scene() -> Result<UiScene, String> {
    let mut scene = UiScene::new(WIDTH as f32, HEIGHT as f32);
    let style = exact_style_resource();

    push_styled_card(
        &mut scene,
        &style,
        "rounded_inset_shadow_card",
        HitRect::new(24.0, 24.0, 112.0, 72.0),
    )?;
    push_styled_card(
        &mut scene,
        &style,
        "mixed_outer_inset_shadow_card",
        HitRect::new(176.0, 40.0, 112.0, 72.0),
    )?;

    Ok(scene)
}

fn exact_style_resource() -> UiStyleResource {
    UiStyleResource {
        style_program_id: "style.seq06_13e1_inset_box_shadow_exact".to_owned(),
        css_sources: vec![StyleSourceIdentity {
            public_id: "style.source.seq06_13e_inset_box_shadow_card_css".to_owned(),
            syntax: StyleSyntax::Css,
            identity: StyleSourceRef::File {
                path: "docs/fixtures/css/seq06.13e-inset-box-shadow-card.css".to_owned(),
            },
            content_digest: None,
        }],
        rules: vec![
            surface_rule(
                "rounded_inset_shadow_card",
                [
                    decl("background-color", style_rgba(36, 42, 54, 255)),
                    decl("border-radius", ViewStyleValue::Text("14px".to_owned())),
                    decl(
                        "box-shadow",
                        ViewStyleValue::Text(
                            "inset 0px 3px 12px 2px rgba(0,0,0,0.56)".to_owned(),
                        ),
                    ),
                ],
            ),
            surface_rule(
                "mixed_outer_inset_shadow_card",
                [
                    decl("background-color", style_rgba(255, 255, 255, 255)),
                    decl("border-radius", ViewStyleValue::Text("16px".to_owned())),
                    decl(
                        "box-shadow",
                        ViewStyleValue::Text(
                            "0px 10px 18px 2px rgba(0,0,0,0.38), inset 0px -2px 10px 1px rgba(255,255,255,0.35)"
                                .to_owned(),
                        ),
                    ),
                ],
            ),
        ],
        ..UiStyleResource::default()
    }
}

fn surface_rule<const N: usize>(
    public_id: &str,
    declarations: [ViewStyleDeclaration; N],
) -> ViewStyleRule {
    ViewStyleRule {
        selector: ViewStyleSelector {
            parts: vec![
                ViewStyleSelectorPart::Element(ViewElementKind::Panel),
                ViewStyleSelectorPart::Part(public_id.to_owned()),
            ],
        },
        declarations: declarations.into(),
        source: None,
    }
}

fn decl(property: &str, value: ViewStyleValue) -> ViewStyleDeclaration {
    ViewStyleDeclaration {
        property: property.to_owned(),
        value,
        op: StyleAssignOp::Replace,
    }
}

fn style_rgba(red: u8, green: u8, blue: u8, alpha: u8) -> ViewStyleValue {
    ViewStyleValue::Rgba(RgbaColor::rgba(red, green, blue, alpha))
}

fn push_styled_card(
    scene: &mut UiScene,
    style: &UiStyleResource,
    public_id: &str,
    bounds: HitRect,
) -> Result<(), String> {
    let resolved = style.runtime_surface_style(public_id);
    if !resolved.diagnostics.is_empty() {
        let diagnostics = resolved
            .diagnostics
            .diagnostics
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("; ");
        return Err(format!(
            "resolve seq06.13e.1 exact surface style `{public_id}`: {diagnostics}"
        ));
    }

    let visual = resolved
        .style
        .visual_for_state(UiRuntimeControlState::Normal);
    let range = push_style_fill(scene, bounds, &visual, public_id)?;
    scene.push_paint_node(UiPaintNode::Group(
        UiCompositingGroup::new(bounds, compositing_effects_from_style(&visual))
            .with_children(vec![direct(range.start, range.end)]),
    ));
    Ok(())
}

fn push_style_fill(
    scene: &mut UiScene,
    bounds: HitRect,
    visual: &UiRuntimeControlVisualStyle,
    public_id: &str,
) -> Result<UiPrimitiveRange, String> {
    let fill = visual
        .fill
        .ok_or_else(|| format!("seq06.13e.1 exact surface style `{public_id}` missing fill"))?;
    let radius = visual
        .radius_milli
        .map(milli_u32_to_f32)
        .unwrap_or_default();
    let start = u32::try_from(scene.primitives().len()).unwrap_or(u32::MAX);
    scene.push_primitive(UiPrimitive::RoundedRect(UiRoundedRect {
        bounds,
        radius,
        color: ui_rgba(fill),
    }));
    let end = u32::try_from(scene.primitives().len()).unwrap_or(u32::MAX);
    Ok(UiPrimitiveRange { start, end })
}

fn compositing_effects_from_style(visual: &UiRuntimeControlVisualStyle) -> UiCompositingEffects {
    UiCompositingEffects {
        box_shadows: UiBoxShadowList::new(
            visual
                .shadows
                .iter()
                .copied()
                .map(ui_box_shadow_from_runtime),
        ),
        ..UiCompositingEffects::default()
    }
}

fn ui_box_shadow_from_runtime(shadow: UiRuntimeShadow) -> UiBoxShadow {
    let offset_x = milli_i32_to_f32(shadow.offset_x_milli);
    let offset_y = milli_i32_to_f32(shadow.offset_y_milli);
    let blur = milli_u32_to_f32(shadow.blur_milli);
    let spread = milli_i32_to_f32(shadow.spread_milli);
    let radius = milli_u32_to_f32(shadow.radius_milli);
    let color = ui_rgba(shadow.color);
    match shadow.kind {
        UiRuntimeShadowKind::Outer => {
            UiBoxShadow::outer(offset_x, offset_y, blur, spread, radius, color)
        }
        UiRuntimeShadowKind::Inset => {
            UiBoxShadow::inset(offset_x, offset_y, blur, spread, radius, color)
        }
    }
}

fn ui_rgba(color: RgbaColor) -> UiColorRgba8 {
    UiColorRgba8 {
        red: color.red,
        green: color.green,
        blue: color.blue,
        alpha: color.alpha,
    }
}

fn milli_i32_to_f32(value: i32) -> f32 {
    value as f32 / 1_000.0
}

fn milli_u32_to_f32(value: u32) -> f32 {
    value as f32 / 1_000.0
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

async fn wait_for_submitted_work(queue: &wgpu::Queue) -> Result<(), String> {
    let (sender, receiver) = futures_channel::oneshot::channel();
    queue.on_submitted_work_done(move || {
        let _ = sender.send(());
    });
    receiver
        .await
        .map_err(|_| String::from("queue submitted-work callback was dropped"))
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
        "UiStyleResource::runtime_surface_style",
        "UiRuntimeControlVisualStyle fill/radius/shadows",
        "UiRoundedRect direct primitive from style fill and border-radius",
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
    writeln!(
        &mut json,
        "    \"style_source\": \"UiStyleResource typed CSS-lowered surface rules\","
    )
    .unwrap();
    writeln!(&mut json, "    \"rounded_rect_fill\": true,").unwrap();
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
