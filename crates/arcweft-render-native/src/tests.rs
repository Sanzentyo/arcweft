use super::*;
use crate::effects::apply_builtin_descriptor;
use crate::renderer::{
    apply_shaped_horizontal_origins_to_glyph_area, apply_text_transforms_to_glyph_area,
    cache_keys_for_layout_glyph, layout_glyph_cache_keys, native_style_for_display_range,
    native_text_bounds, padded_rgba_row_bytes, presentation_alpha_for_visibility_time,
    ruby_glyph_area_options, ruby_glyph_areas, vertical_form_cache_keys,
    vertical_form_font_features,
};
use crate::window_page::{
    NativeFontFamily, NativeTextColor, NativeTextWeight, WindowRubyAnnotation,
    native_ruby_style_from_styles, native_style_from_styles, ruby_overlay_geometry,
};
use arcweft_core::plan::{RuntimeLineId, RuntimePureHelper, RuntimePureHelperId};
use arcweft_image::{
    DecodedImage, DecodedImageFrame, ImageDimensions, ImageFormat, ImageRepetition,
};
use arcweft_render_text::{
    LineDisplaySpec, RichTextAngle, RichTextControl, RichTextDisplayMap, RichTextDocument,
    RichTextLayout, RichTextNode, RichTextObjectProxy, RichTextParam, RichTextStateScope,
    RichTextStyle, RichTextTextRun, RichTextTransform, RichTextVec2, RichTextWritingMode,
    RuntimeLineContext,
};
use arcweft_text_layout::{LayoutPoint, LayoutSize, layout_frame};
use arcweft_view::{
    FragmentKind, ImageId, LayoutLength as UiLayoutLength, LayoutPoint as UiLayoutPoint,
    LayoutResults as UiLayoutResults, LayoutSize as UiLayoutSize, LayoutTree as UiLayoutTree,
    NodeKey, StyleId, UiImageSource, UiImageSourceTable, ViewFragmentBuilder,
};

fn styled_ruby_test_frame() -> LineDisplayFrame {
    let spec = LineDisplaySpec {
        line: RuntimeLineId("say.test.001".to_owned()),
        callee: "alice".to_owned(),
        speaker_label: None,
        text_key: None,
        window: None,
        voice: None,
        look: None,
        style: None,
        base_styles: vec![RichTextStyle::from_tag("color", "#aabedc")],
        default_inline_failure_policy: None,
        style_contributions: Vec::new(),
        args: Vec::new(),
        content: RichTextDocument::new(vec![
            RichTextNode::Text {
                text: "Hello ".to_owned(),
            },
            RichTextNode::StyleStart {
                style: RichTextStyle::from_tag("color", "#80c0ff"),
            },
            RichTextNode::StyleStart {
                style: RichTextStyle::from_tag("font", "monospace"),
            },
            RichTextNode::Ruby {
                base: "夢".to_owned(),
                ruby: "ゆめ".to_owned(),
            },
            RichTextNode::StyleEnd {
                name: "font".to_owned(),
            },
            RichTextNode::StyleEnd {
                name: "color".to_owned(),
            },
        ]),
    };
    let mut frame = spec
        .resolve_frame(&RuntimeLineContext::default())
        .expect("frame resolves");
    frame.nodes.clear();
    frame
}

fn two_frame_ui_image() -> DecodedImage {
    let dimensions = ImageDimensions::new(2, 1).unwrap();
    DecodedImage::new(
        ImageFormat::Gif,
        dimensions,
        ImageRepetition::Infinite,
        vec![
            DecodedImageFrame::new(0, dimensions, 100, vec![255, 0, 0, 255, 0, 0, 255, 255])
                .unwrap(),
            DecodedImageFrame::new(1, dimensions, 100, vec![0, 255, 0, 255, 255, 255, 0, 255])
                .unwrap(),
        ],
    )
    .unwrap()
}

#[test]
fn native_image_quads_from_display_list_uses_ui_image_frame_and_fit() {
    let mut images = UiImageSourceTable::default();
    images
        .insert_with_id(ImageId(7), UiImageSource::new(two_frame_ui_image()))
        .unwrap();

    let mut builder = ViewFragmentBuilder::default();
    let image_node = builder
        .push_node(
            NodeKey(10),
            FragmentKind::Image(ImageId(7)),
            StyleId(0),
            &[],
            &[],
            None,
        )
        .unwrap();
    let fragment = builder.finish();
    let tree = UiLayoutTree::from_fragment(&fragment).unwrap();
    let mut layouts = UiLayoutResults::new(&tree);
    layouts
        .set(
            image_node,
            LayoutBox::new(
                UiLayoutPoint::new(UiLayoutLength::px(10), UiLayoutLength::px(20)),
                UiLayoutSize::new(UiLayoutLength::px(100), UiLayoutLength::px(100)),
            ),
        )
        .unwrap();
    let display = DisplayList::from_fragment(&fragment, &layouts).unwrap();

    let quads = native_image_quads_from_display_list(&display, &images, 150)
        .expect("display image resolves to native quad");

    assert_eq!(quads.len(), 1);
    assert_eq!(quads[0].width, 2);
    assert_eq!(quads[0].height, 1);
    assert_eq!(quads[0].rgba, &[0, 255, 0, 255, 255, 255, 0, 255]);
    assert_eq!(quads[0].opacity_milli, 1_000);
    assert_eq!(
        quads[0].dst,
        NativeImageRect {
            x: 10.0,
            y: 45.0,
            width: 100.0,
            height: 50.0,
        }
    );
}

#[test]
fn native_capture_renders_image_quad_pixels() {
    let rgba = vec![
        255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255,
    ];
    let quad = NativeImageQuad {
        width: 2,
        height: 2,
        rgba: &rgba,
        opacity_milli: 1_000,
        dst: NativeImageRect {
            x: 1.0,
            y: 1.0,
            width: 2.0,
            height: 2.0,
        },
        transform: NativeImageTransform::identity(),
    };

    let capture = capture_image_quads_rgba(&[quad], 4, 4).expect("image quad renders");

    assert_eq!(capture.width, 4);
    assert_eq!(capture.height, 4);
    assert_eq!(capture.content_pixels, 4);
    assert_eq!(
        capture.content_bbox,
        Some(NativeFrameContentBBox {
            x: 1,
            y: 1,
            width: 2,
            height: 2,
        })
    );
    assert_eq!(pixel_at(&capture, 1, 1), [255, 0, 0, 255]);
    assert_eq!(pixel_at(&capture, 2, 1), [0, 255, 0, 255]);
    assert_eq!(pixel_at(&capture, 1, 2), [0, 0, 255, 255]);
    assert_eq!(pixel_at(&capture, 2, 2), [255, 255, 255, 255]);
}

#[test]
fn native_capture_renders_image_debug_quad_from_alpha() {
    let rgba = vec![255, 0, 0, 255, 0, 255, 0, 0];
    let quad = NativeImageDebugQuad {
        quad: NativeImageQuad {
            width: 2,
            height: 1,
            rgba: &rgba,
            opacity_milli: 1_000,
            dst: NativeImageRect {
                x: 1.0,
                y: 1.0,
                width: 2.0,
                height: 1.0,
            },
            transform: NativeImageTransform::identity(),
        },
        color: [40, 80, 120, 255],
    };

    let capture = capture_image_debug_quads_rgba(&[quad], 4, 3).expect("debug quad renders");

    assert_eq!(capture.content_pixels, 1);
    assert_eq!(
        capture.content_bbox,
        Some(NativeFrameContentBBox {
            x: 1,
            y: 1,
            width: 1,
            height: 1,
        })
    );
    assert_eq!(pixel_at(&capture, 1, 1), [40, 80, 120, 255]);
    assert_eq!(pixel_at(&capture, 2, 1), [0, 0, 0, 0]);
}

#[test]
fn native_capture_applies_image_quad_opacity_to_color_and_debug_alpha() {
    let rgba = vec![255, 0, 0, 255];
    let quad = NativeImageQuad {
        width: 1,
        height: 1,
        rgba: &rgba,
        opacity_milli: 500,
        dst: NativeImageRect {
            x: 1.0,
            y: 1.0,
            width: 1.0,
            height: 1.0,
        },
        transform: NativeImageTransform::identity(),
    };
    let color = capture_image_quads_rgba(&[quad], 3, 3).expect("opacity quad renders");
    assert_eq!(pixel_at(&color, 1, 1), [187, 0, 0, 127]);

    let debug = capture_image_debug_quads_rgba(
        &[NativeImageDebugQuad {
            quad,
            color: [10, 20, 30, 255],
        }],
        3,
        3,
    )
    .expect("debug opacity quad renders");
    assert_eq!(pixel_at(&debug, 1, 1), [5, 11, 19, 127]);
}

#[test]
fn native_capture_applies_image_quad_transform_to_vertices() {
    let rgba = vec![255, 255, 255, 255];
    let quad = NativeImageQuad {
        width: 1,
        height: 1,
        rgba: &rgba,
        opacity_milli: 1_000,
        dst: NativeImageRect {
            x: 1.0,
            y: 1.0,
            width: 1.0,
            height: 1.0,
        },
        transform: NativeImageTransform {
            tx: 1.0,
            ty: 1.0,
            ..NativeImageTransform::identity()
        },
    };

    let capture = capture_image_quads_rgba(&[quad], 4, 4).expect("transformed quad renders");

    assert_eq!(capture.content_pixels, 1);
    assert_eq!(pixel_at(&capture, 1, 1), [0, 0, 0, 0]);
    assert_eq!(pixel_at(&capture, 2, 2), [255, 255, 255, 255]);
}

fn pixel_at(capture: &NativeFrameCapture, x: u32, y: u32) -> [u8; 4] {
    let index = usize::try_from(y)
        .unwrap()
        .saturating_mul(usize::try_from(capture.width).unwrap())
        .saturating_add(usize::try_from(x).unwrap())
        .saturating_mul(4);
    capture.rgba[index..index + 4].try_into().unwrap()
}

fn vertical_ruby_text_combine_frame(writing_mode: RichTextWritingMode) -> LineDisplayFrame {
    let spec = LineDisplaySpec {
        line: RuntimeLineId(format!(
            "say.test.vertical.{}.window.ruby.combine",
            match writing_mode {
                RichTextWritingMode::VerticalRl => "rl",
                RichTextWritingMode::VerticalLr => "lr",
                RichTextWritingMode::HorizontalTb => "horizontal",
            }
        )),
        callee: "alice".to_owned(),
        speaker_label: None,
        text_key: None,
        window: None,
        voice: None,
        look: None,
        style: None,
        base_styles: Vec::new(),
        default_inline_failure_policy: None,
        style_contributions: Vec::new(),
        args: Vec::new(),
        content: RichTextDocument::new(vec![
            RichTextNode::StyleStart {
                style: RichTextStyle::Layout {
                    layout: RichTextLayout {
                        writing_mode,
                        ..RichTextLayout::default()
                    },
                },
            },
            RichTextNode::Text {
                text: "天地".to_owned(),
            },
            RichTextNode::Ruby {
                base: "夢".to_owned(),
                ruby: "ゆめ".to_owned(),
            },
            RichTextNode::Text {
                text: "2026Z".to_owned(),
            },
            RichTextNode::StyleEnd {
                name: "layout".to_owned(),
            },
        ]),
    };
    spec.resolve_frame(&RuntimeLineContext::default())
        .expect("frame resolves")
}

#[test]
fn window_rich_text_uses_display_map_for_style_spans_and_ruby_hint() {
    let frame = styled_ruby_test_frame();
    let pages = WindowPage::from_frame(&frame);
    assert!(
        pages[0].layout_frame.is_some(),
        "display-map pages retain page-local layout source for window GlyphArea rendering"
    );
    let rich_text = &pages[0].rich_text;

    assert_eq!(rich_text.text, "Hello 夢");
    assert_eq!(
        rich_text.ruby_annotations,
        vec![WindowRubyAnnotation {
            base_range: "Hello ".len().."Hello 夢".len(),
            ruby: "ゆめ".to_owned(),
            style: NativeTextStyle {
                color: NativeTextColor::new(170, 190, 220),
                family: NativeFontFamily::Monospace,
                weight: NativeTextWeight::Regular,
                italic: false,
                size: Some(14),
            },
            presentation: RichTextPresentation::default(),
        }]
    );
    assert!(rich_text.spans.iter().any(|span| {
        &rich_text.text[span.range.clone()] == "夢"
            && span.style.color == NativeTextColor::new(128, 192, 255)
            && span.style.family == NativeFontFamily::Monospace
    }));

    let mut font_system = FontSystem::new();
    let mut buffer = Buffer::new(&mut font_system, Metrics::new(30.0, 42.0));
    buffer.set_size(&mut font_system, Some(800.0), Some(600.0));
    let default_style = NativeTextStyle::default();
    let default_attrs = default_style.attrs();
    let spans = rich_text
        .spans
        .iter()
        .map(|span| (&rich_text.text[span.range.clone()], span.style.attrs()));
    buffer.set_rich_text(
        &mut font_system,
        spans,
        &default_attrs,
        Shaping::Advanced,
        None,
    );
    buffer.shape_until_scroll(&mut font_system, false);
    let measured = ruby_overlay_geometry(
        &buffer,
        rich_text,
        &rich_text.ruby_annotations[0].base_range,
        NativeTextOrigin::default(),
    )
    .expect("ruby base has shaped glyph geometry");
    assert!(measured.2 > 1.0);

    assert_ruby_glyph_areas_use_absolute_glypharea(&mut font_system, &buffer, rich_text, &pages);
}

#[test]
fn native_text_features_disable_standard_ligatures_for_cluster_mapping() {
    let features = native_text_font_features();

    assert!(
        features
            .features
            .iter()
            .any(|feature| feature.tag == FeatureTag::new(b"liga") && feature.value == 0)
    );
    assert!(
        features
            .features
            .iter()
            .any(|feature| feature.tag == FeatureTag::new(b"clig") && feature.value == 0)
    );
}

#[test]
fn soft_glow_shader_adds_native_glyph_passes() {
    let frame = shader_test_frame("soft_glow", RichTextEffectPhase::RunOffscreenPass);
    let page = WindowPage::from_frame(&frame)
        .into_iter()
        .next()
        .expect("window page");
    let page_layout_frame = page.layout_frame.as_ref().expect("layout frame");
    let layout = layout_frame(
        page_layout_frame,
        native_text_layout_config(800, 600, 96.0, 572.0),
    )
    .expect("layout resolves");
    let mut font_system = FontSystem::new();
    let mut buffer = Buffer::new(&mut font_system, Metrics::new(30.0, 42.0));
    prepare_window_text_buffers(&mut font_system, &mut buffer, &page.rich_text, 800, 600);
    let cache_keys = layout_glyph_cache_keys(&mut font_system, &buffer, &page.rich_text, &layout);
    let glyph_area = glyph_area_from_layout(
        &layout,
        GlyphonAreaOptions {
            bounds: native_text_bounds(800, 600),
            origin_offset: Vector::new(0.0, NATIVE_GLYPHAREA_BASELINE_OFFSET),
            ..GlyphonAreaOptions::default()
        },
        |index, glyph| cache_keys_for_layout_glyph(index, glyph.range, &cache_keys),
    )
    .expect("glyph area resolves");

    let mut shader_registry = native_default_shader_registry();
    let mut state = RichTextStateStore::default();
    let mut effects =
        NativeEffectExecution::new(None, Some(&mut shader_registry), None, &mut state);

    let glow_areas = shader_glyph_areas_for_text(&glyph_area, &layout, &mut effects);

    assert_eq!(glow_areas.len(), 4);
    assert_eq!(glow_areas[0].len(), glyph_area.len());
    assert!(
        glow_areas[0].glyphs()[0].origin.x > glyph_area.glyphs()[0].origin.x + 5.5,
        "forward glow pass should offset along shader dir"
    );
    assert_eq!(
        glow_areas[0].glyphs()[0].color,
        Some(Color::rgba(155, 205, 255, 72))
    );
}

#[test]
fn soft_glow_shader_glyph_color_tints_native_glyphs() {
    let frame = shader_test_frame("soft_glow", RichTextEffectPhase::GlyphColor);
    let plan = visual_plan_from_frame_for_test(&frame, 0.0);

    assert_eq!(
        plan.diagnostics,
        Vec::<NativeVisualDiagnostic>::new(),
        "registered glyph_color shaders should execute instead of warning"
    );
    assert!(
        plan.pages
            .iter()
            .flat_map(|page| page.glyphs.iter())
            .any(|glyph| glyph.color == Some([155, 205, 255, 255])),
        "soft_glow glyph_color should tint main glyph placements: {plan:#?}"
    );

    let capture =
        capture_frame_rgba_at(&frame, 800, 600, 96.0, 572.0).expect("glyph_color shader capture");
    assert!(capture.diagnostics.is_empty());
    assert!(
        capture
            .rgba
            .chunks_exact(4)
            .any(|pixel| pixel[2] > pixel[0] && pixel[2] > pixel[1] && pixel[3] > 0),
        "soft_glow glyph_color should produce blue-tinted glyph pixels"
    );
}

#[test]
fn color_region_capture_strips_unselected_shader_passes() {
    let frame = shader_selection_test_frame();
    let mut session = NativeOffscreenCaptureSession::new().expect("capture session");
    let bounds = session
        .measure_frame_elements_at(&frame, 800, 600, 96.0, 572.0)
        .expect("bounds resolve");
    let glow_bbox = bounds
        .iter()
        .find_map(|bounds| {
            matches!(bounds.element, NativeFrameElement::TextRun { index: 0 })
                .then_some(bounds.bbox)
        })
        .expect("glow run bbox resolves");
    let selected_bbox = bounds
        .iter()
        .find_map(|bounds| {
            matches!(bounds.element, NativeFrameElement::TextRun { index: 1 })
                .then_some(bounds.bbox)
        })
        .expect("selected run bbox resolves");
    let capture = session
        .capture_frame_color_regions_at(
            &frame,
            800,
            600,
            96.0,
            572.0,
            &[NativeFrameDebugRegion {
                element: Some(NativeFrameElement::TextRun { index: 1 }),
                fallback_bbox: selected_bbox,
                color: [255, 255, 255, 255],
            }],
        )
        .expect("selected color capture resolves");

    assert_eq!(capture.diagnostics, Vec::<NativeVisualDiagnostic>::new());
    assert_eq!(
        content_pixels_in_bbox(&capture, glow_bbox),
        0,
        "unselected shader passes should not leak into isolated color capture"
    );
    assert!(
        content_pixels_in_bbox(&capture, selected_bbox) > 0,
        "selected run should still render in isolated color capture"
    );
}

#[test]
fn native_visual_plan_reports_missing_shader_registry_entry() {
    let frame = shader_test_frame("missing_glow", RichTextEffectPhase::RunOffscreenPass);

    let plan = visual_plan_from_frame_for_test(&frame, 0.0);

    assert_eq!(plan.diagnostics.len(), 1);
    assert_eq!(
        plan.diagnostics[0].severity,
        NativeVisualDiagnosticSeverity::Warning
    );
    assert_eq!(plan.diagnostics[0].code, "missing_shader");
    assert_eq!(
        plan.diagnostics[0].effect_id.as_deref(),
        Some("missing_glow")
    );
}

#[test]
fn native_capture_uses_custom_shader_registry_for_submitted_glyph_passes() {
    let frame = shader_test_frame("rose_glow", RichTextEffectPhase::RunOffscreenPass);
    let mut session = NativeOffscreenCaptureSession::new().expect("capture session");
    session
        .shader_registry_mut()
        .insert_lambda("rose_glow", |_ctx| {
            vec![NativeShaderGlyphPass {
                offset: [18.0, -2.0],
                color: [255, 48, 24, 255],
            }]
        });

    let capture = session
        .capture_frame_rgba_at(&frame, 800, 600, 96.0, 572.0)
        .expect("custom shader capture");

    assert!(capture.diagnostics.is_empty());
    assert!(capture.content_pixels > 0);
    assert!(
        capture.rgba.chunks_exact(4).any(|pixel| {
            pixel[0] > pixel[1].saturating_add(90)
                && pixel[0] > pixel[2].saturating_add(90)
                && pixel[3] > 0
        }),
        "registered custom shader should emit red-tinted glyph pass pixels"
    );
}

#[test]
fn native_capture_applies_default_post_process_shader() {
    let frame = shader_test_frame("screen_tint", RichTextEffectPhase::PostProcess);
    let capture =
        capture_frame_rgba_at(&frame, 800, 600, 96.0, 572.0).expect("post-process shader capture");

    assert!(capture.diagnostics.is_empty());
    assert!(capture.content_pixels > 0);
    assert!(
        capture
            .rgba
            .chunks_exact(4)
            .any(|pixel| pixel[2] > pixel[0] && pixel[2] > pixel[1] && pixel[3] > 0),
        "default screen_tint post-process should blue-tint rendered glyph pixels"
    );
}

#[test]
fn native_capture_uses_custom_post_process_shader_registry() {
    let frame = shader_test_frame("rose_screen", RichTextEffectPhase::PostProcess);
    let mut session = NativeOffscreenCaptureSession::new().expect("capture session");
    session
        .shader_registry_mut()
        .insert_post_process_lambda("rose_screen", |_ctx, rgba| {
            for pixel in rgba.chunks_exact_mut(4) {
                if pixel[3] == 0 || (pixel[0] == 0 && pixel[1] == 0 && pixel[2] == 0) {
                    continue;
                }
                pixel[0] = 255;
                pixel[1] = 24;
                pixel[2] = 16;
            }
        });

    let capture = session
        .capture_frame_rgba_at(&frame, 800, 600, 96.0, 572.0)
        .expect("custom post-process shader capture");

    assert!(capture.diagnostics.is_empty());
    assert!(
        capture.rgba.chunks_exact(4).any(|pixel| {
            pixel[0] > pixel[1].saturating_add(120)
                && pixel[0] > pixel[2].saturating_add(120)
                && pixel[3] > 0
        }),
        "registered post-process shader should alter rendered glyph pixels"
    );
}

#[test]
fn native_measure_reports_text_object_proxy_element_bounds() {
    let spec = LineDisplaySpec {
        line: RuntimeLineId("say.test.text.object.proxy".to_owned()),
        callee: "alice".to_owned(),
        speaker_label: None,
        text_key: None,
        window: None,
        voice: None,
        look: None,
        style: None,
        base_styles: Vec::new(),
        default_inline_failure_policy: None,
        style_contributions: Vec::new(),
        args: Vec::new(),
        content: RichTextDocument::new(vec![
            RichTextNode::Text {
                text: "A".to_owned(),
            },
            RichTextNode::StyleStart {
                style: RichTextStyle::Object {
                    proxy: RichTextObjectProxy {
                        id: "hotspot".to_owned(),
                        declaration: None,
                        type_name: Some("KeywordHit".to_owned()),
                        role: Some("keyword".to_owned()),
                        layer: Some("ui".to_owned()),
                        depth: Some(Milli(4000)),
                        hit_test: true,
                        params: BTreeMap::new(),
                    },
                },
            },
            RichTextNode::Text {
                text: "proxy".to_owned(),
            },
            RichTextNode::StyleEnd {
                name: "object".to_owned(),
            },
            RichTextNode::Text {
                text: "Z".to_owned(),
            },
        ]),
    };
    let frame = spec
        .resolve_frame(&RuntimeLineContext::default())
        .expect("frame resolves");
    let bounds = measure_frame_elements_at(&frame, 800, 600, 96.0, 572.0).expect("bounds resolve");
    let proxy = bounds
        .iter()
        .find(|bounds| {
            matches!(
                bounds.element,
                NativeFrameElement::TextObjectProxy {
                    run_index: 1,
                    proxy_index: 0
                }
            )
        })
        .expect("text object proxy element is addressable");
    let run = bounds
        .iter()
        .find(|bounds| matches!(bounds.element, NativeFrameElement::TextRun { index: 1 }))
        .expect("proxy text run element is addressable");

    assert_eq!(proxy.bbox, run.bbox);
    assert!(proxy.bbox.width > 0);
    assert!(proxy.bbox.height > 0);
}

fn shader_test_frame(id: &str, phase: RichTextEffectPhase) -> LineDisplayFrame {
    let spec = LineDisplaySpec {
        line: RuntimeLineId("say.test.shader.soft_glow".to_owned()),
        callee: "alice".to_owned(),
        speaker_label: None,
        text_key: None,
        window: None,
        voice: None,
        look: None,
        style: None,
        base_styles: Vec::new(),
        default_inline_failure_policy: None,
        style_contributions: Vec::new(),
        args: Vec::new(),
        content: RichTextDocument::new(vec![
            RichTextNode::StyleStart {
                style: RichTextStyle::Shader {
                    shader: RichTextShaderRef {
                        id: id.to_owned(),
                        params: BTreeMap::from([
                            (
                                "amount".to_owned(),
                                RichTextParam::Milli { value: Milli::ONE },
                            ),
                            (
                                "color".to_owned(),
                                RichTextParam::Text {
                                    value: "#40b0ff".to_owned(),
                                },
                            ),
                            (
                                "dir".to_owned(),
                                RichTextParam::Raw {
                                    value: "1,0".to_owned(),
                                },
                            ),
                        ]),
                        phase,
                    },
                },
            },
            RichTextNode::Text {
                text: "A".to_owned(),
            },
            RichTextNode::StyleEnd {
                name: "effect".to_owned(),
            },
        ]),
    };
    spec.resolve_frame(&RuntimeLineContext::default())
        .expect("frame resolves")
}

fn shader_selection_test_frame() -> LineDisplayFrame {
    LineDisplayFrame {
        line: RuntimeLineId("say.test.shader.selection".to_owned()),
        callee: "alice".to_owned(),
        speaker_label: None,
        text: "glow plain".to_owned(),
        base_styles: Vec::new(),
        default_inline_failure_policy: None,
        style_contributions: Vec::new(),
        nodes: Vec::new(),
        display_map: RichTextDisplayMap {
            text_runs: vec![
                RichTextTextRun {
                    range: RichTextRange::new(0, 4),
                    source: arcweft_render_text::RichTextTextSource::Text,
                    node_index: 0,
                    styles: Vec::new(),
                    presentation: RichTextPresentation {
                        shaders: vec![RichTextShaderRef {
                            id: "soft_glow".to_owned(),
                            params: BTreeMap::from([(
                                "amount".to_owned(),
                                RichTextParam::Milli { value: Milli::ONE },
                            )]),
                            phase: RichTextEffectPhase::RunOffscreenPass,
                        }],
                        ..RichTextPresentation::default()
                    },
                },
                RichTextTextRun {
                    range: RichTextRange::new(5, 10),
                    source: arcweft_render_text::RichTextTextSource::Text,
                    node_index: 1,
                    styles: Vec::new(),
                    presentation: RichTextPresentation::default(),
                },
            ],
            ruby_annotations: Vec::new(),
            controls: Vec::new(),
            host_events: Vec::new(),
        },
        host_events: Vec::new(),
        inline_failures: Vec::new(),
        unresolved: Vec::new(),
    }
}

fn content_pixels_in_bbox(capture: &NativeFrameCapture, bbox: NativeFrameContentBBox) -> u64 {
    let width = capture.width as usize;
    let x_end = bbox.x.saturating_add(bbox.width).min(capture.width);
    let y_end = bbox.y.saturating_add(bbox.height).min(capture.height);
    (bbox.y..y_end)
        .flat_map(|y| (bbox.x..x_end).map(move |x| (x as usize, y as usize)))
        .filter(|(x, y)| {
            let offset = (y.saturating_mul(width).saturating_add(*x)).saturating_mul(4);
            capture
                .rgba
                .get(offset..offset.saturating_add(4))
                .is_some_and(|pixel| pixel[3] > 0)
        })
        .count() as u64
}

fn motion_test_frame(function: &str) -> LineDisplayFrame {
    let spec = LineDisplaySpec {
        line: RuntimeLineId("say.test.motion.registry".to_owned()),
        callee: "alice".to_owned(),
        speaker_label: None,
        text_key: None,
        window: None,
        voice: None,
        look: None,
        style: None,
        base_styles: Vec::new(),
        default_inline_failure_policy: None,
        style_contributions: Vec::new(),
        args: Vec::new(),
        content: RichTextDocument::new(vec![
            RichTextNode::StyleStart {
                style: RichTextStyle::Effect {
                    effect: RichTextEffectDescriptor {
                        id: "motion".to_owned(),
                        params: BTreeMap::from([
                            (
                                "fn".to_owned(),
                                RichTextParam::Raw {
                                    value: function.to_owned(),
                                },
                            ),
                            (
                                "amp".to_owned(),
                                RichTextParam::Milli {
                                    value: Milli(48_000),
                                },
                            ),
                        ]),
                        target: RichTextEffectTarget::Glyph,
                        phase: RichTextEffectPhase::GlyphTransform,
                        state_scope: RichTextStateScope::Glyph,
                    },
                },
            },
            RichTextNode::Text {
                text: "A".to_owned(),
            },
            RichTextNode::StyleEnd {
                name: "/".to_owned(),
            },
        ]),
    };
    spec.resolve_frame(&RuntimeLineContext::default())
        .expect("frame resolves")
}

#[derive(Default)]
struct RuntimeTextHelperReport {
    shaders: Vec<RuntimePureHelper>,
    effects: Vec<RuntimePureHelper>,
    motions: Vec<RuntimePureHelper>,
}

fn arcweft_text_registry_candidates(source: &str) -> RuntimeTextHelperReport {
    let compiled = arcweft_compiler::source::compile_source(source)
        .expect("Arcweft text registry source compiles");
    let report = arcweft_compiler::lower::lower_source_text_pure_helper_candidates(&compiled.hir)
        .expect("Arcweft text registry source exports pure text helpers");
    RuntimeTextHelperReport {
        shaders: runtime_helpers(&report.shaders),
        effects: runtime_helpers(&report.effects),
        motions: runtime_helpers(&report.motions),
    }
}

fn runtime_helpers(
    candidates: &[arcweft_runtime_plan::pure::PureHelperCandidate],
) -> Vec<RuntimePureHelper> {
    candidates
        .iter()
        .enumerate()
        .map(|(index, candidate)| candidate.to_runtime_helper(RuntimePureHelperId(index)))
        .collect()
}

fn text_motion_context(
    effect: &RichTextEffectDescriptor,
    glyph_index: usize,
    glyph_count: usize,
) -> TextMotionContext<'_> {
    TextMotionContext {
        effect,
        function: "arc_phase",
        sample_time: 0.5,
        line_id: "say.test.arcweft.motion",
        run_index: 0,
        glyph_index,
        glyph_count,
        noise: [0.0, 0.0],
    }
}

#[test]
fn shaped_horizontal_origins_compact_latin_submission_spacing() {
    let spec = LineDisplaySpec {
        line: RuntimeLineId("say.test.shaped.latin".to_owned()),
        callee: "alice".to_owned(),
        speaker_label: None,
        text_key: None,
        window: None,
        voice: None,
        look: None,
        style: None,
        base_styles: Vec::new(),
        default_inline_failure_policy: None,
        style_contributions: Vec::new(),
        args: Vec::new(),
        content: RichTextDocument::new(vec![RichTextNode::Text {
            text: "serif".to_owned(),
        }]),
    };
    let frame = spec
        .resolve_frame(&RuntimeLineContext::default())
        .expect("frame resolves");
    let page = WindowPage::from_frame(&frame)
        .into_iter()
        .next()
        .expect("window page");
    let page_layout_frame = page.layout_frame.as_ref().expect("layout frame");
    let layout = layout_frame(
        page_layout_frame,
        native_text_layout_config(800, 600, 96.0, 572.0),
    )
    .expect("layout resolves");
    let mut font_system = FontSystem::new();
    let mut buffer = Buffer::new(&mut font_system, Metrics::new(30.0, 42.0));
    prepare_window_text_buffers(&mut font_system, &mut buffer, &page.rich_text, 800, 600);
    let cache_keys = layout_glyph_cache_keys(&mut font_system, &buffer, &page.rich_text, &layout);
    let mut glyph_area = glyph_area_from_layout(
        &layout,
        GlyphonAreaOptions {
            bounds: native_text_bounds(800, 600),
            origin_offset: Vector::new(0.0, NATIVE_GLYPHAREA_BASELINE_OFFSET),
            ..GlyphonAreaOptions::default()
        },
        |index, glyph| cache_keys_for_layout_glyph(index, glyph.range, &cache_keys),
    )
    .expect("glyph area resolves");
    let heuristic_span = layout.glyphs[4].origin.x - layout.glyphs[0].origin.x;

    apply_shaped_horizontal_origins_to_glyph_area(&mut glyph_area, &layout, &cache_keys);

    let first = glyph_area
        .glyphs()
        .iter()
        .find(|glyph| glyph.metadata == 0)
        .expect("first glyph")
        .origin
        .x;
    let last = glyph_area
        .glyphs()
        .iter()
        .find(|glyph| glyph.metadata == 4)
        .expect("last glyph")
        .origin
        .x;
    assert!(
        last - first < heuristic_span,
        "native shaped advance should compact Latin submission spacing: shaped={} heuristic={}",
        last - first,
        heuristic_span
    );
}

#[test]
fn native_measurement_uses_shaped_horizontal_latin_spacing() {
    let spec = LineDisplaySpec {
        line: RuntimeLineId("say.test.shaped.latin.measure".to_owned()),
        callee: "alice".to_owned(),
        speaker_label: None,
        text_key: None,
        window: None,
        voice: None,
        look: None,
        style: None,
        base_styles: Vec::new(),
        default_inline_failure_policy: None,
        style_contributions: Vec::new(),
        args: Vec::new(),
        content: RichTextDocument::new(vec![RichTextNode::Text {
            text: "serif".to_owned(),
        }]),
    };
    let frame = spec
        .resolve_frame(&RuntimeLineContext::default())
        .expect("frame resolves");
    let page_range = display_map_non_empty_page_range_at(&frame, 0).expect("page range");
    let page_layout = layout_page_range(
        &frame,
        page_range,
        native_text_layout_config(800, 600, 96.0, 572.0),
    )
    .expect("layout resolves");
    let heuristic_span =
        page_layout.layout.glyphs[4].origin.x - page_layout.layout.glyphs[0].origin.x;

    let bounds =
        measure_frame_elements_at(&frame, 800, 600, 96.0, 572.0).expect("native bounds resolve");
    let first = bounds
        .iter()
        .find(|bounds| {
            matches!(
                bounds.element,
                NativeFrameElement::GlyphCluster {
                    index: 0,
                    range_start: 0,
                    range_end: 1
                }
            )
        })
        .expect("first glyph cluster");
    let last = bounds
        .iter()
        .find(|bounds| {
            matches!(
                bounds.element,
                NativeFrameElement::GlyphCluster {
                    index: 4,
                    range_start: 4,
                    range_end: 5
                }
            )
        })
        .expect("last glyph cluster");

    let measured_span = last.bbox.x.saturating_sub(first.bbox.x);
    assert!(
        f32::from(u16::try_from(measured_span).unwrap_or(u16::MAX)) < heuristic_span,
        "measurement should be tighter than Sans I/O heuristic: measured={measured_span}px heuristic={heuristic_span}"
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn native_glyph_area_applies_transform_affine_and_builtin_translation() {
    let spec = LineDisplaySpec {
        line: RuntimeLineId("say.test.transform.glyph-area".to_owned()),
        callee: "alice".to_owned(),
        speaker_label: None,
        text_key: None,
        window: None,
        voice: None,
        look: None,
        style: None,
        base_styles: Vec::new(),
        default_inline_failure_policy: None,
        style_contributions: Vec::new(),
        args: Vec::new(),
        content: RichTextDocument::new(vec![
            RichTextNode::StyleStart {
                style: RichTextStyle::Transform {
                    transform: RichTextTransform {
                        translate: RichTextVec2::new(Milli(5000), Milli(-2000)),
                        rotate: RichTextAngle {
                            degrees: Milli(10000),
                        },
                        scale: RichTextVec2::new(Milli(1200), Milli(900)),
                        skew: RichTextVec2::new(Milli(10000), Milli::ZERO),
                        origin: RichTextTransformOrigin::Center,
                        target: RichTextEffectTarget::TextBox,
                    },
                },
            },
            RichTextNode::StyleStart {
                style: RichTextStyle::Effect {
                    effect: RichTextEffectDescriptor {
                        id: "wave".to_owned(),
                        params: BTreeMap::from([
                            (
                                "amp".to_owned(),
                                RichTextParam::Milli { value: Milli(3000) },
                            ),
                            (
                                "dir".to_owned(),
                                RichTextParam::Vec2 {
                                    value: RichTextVec2::new(Milli::ONE, Milli::ZERO),
                                },
                            ),
                            (
                                "phase".to_owned(),
                                RichTextParam::Milli { value: Milli(250) },
                            ),
                        ]),
                        target: RichTextEffectTarget::TextBox,
                        phase: RichTextEffectPhase::GlyphTransform,
                        state_scope: arcweft_render_text::RichTextStateScope::Run,
                    },
                },
            },
            RichTextNode::Text {
                text: "揺".to_owned(),
            },
            RichTextNode::StyleEnd {
                name: "effect".to_owned(),
            },
            RichTextNode::StyleEnd {
                name: "transform".to_owned(),
            },
        ]),
    };
    let frame = spec
        .resolve_frame(&RuntimeLineContext::default())
        .expect("frame resolves");
    let pages = WindowPage::from_frame(&frame);
    let page = &pages[0];
    let page_layout_frame = page.layout_frame.as_ref().expect("layout frame");
    let layout = layout_frame(
        page_layout_frame,
        native_text_layout_config(800, 600, 96.0, 572.0),
    )
    .expect("layout resolves");
    let mut font_system = FontSystem::new();
    let mut buffer = Buffer::new(&mut font_system, Metrics::new(30.0, 42.0));
    prepare_window_text_buffers(&mut font_system, &mut buffer, &page.rich_text, 800, 600);
    let cache_keys = layout_glyph_cache_keys(&mut font_system, &buffer, &page.rich_text, &layout);
    let mut glyph_area = glyph_area_from_layout(
        &layout,
        GlyphonAreaOptions {
            bounds: native_text_bounds(800, 600),
            origin_offset: Vector::new(0.0, NATIVE_GLYPHAREA_BASELINE_OFFSET),
            ..GlyphonAreaOptions::default()
        },
        |index, glyph| cache_keys_for_layout_glyph(index, glyph.range, &cache_keys),
    )
    .expect("glyph area resolves");
    let before = glyph_area.glyphs()[0].origin;
    let layout_bbox_x = layout.glyphs[0].bounds.x;

    apply_text_transforms_to_glyph_area(&mut glyph_area, &page.rich_text.text, &layout, 0.0);

    let after = &glyph_area.glyphs()[0];
    assert!(
        after.origin.x > before.x + 7.5 && after.origin.x < before.x + 8.5,
        "translate x plus wave should move the glyph by about 8px: {before:?} -> {:?}",
        after.origin
    );
    assert!(
        after.origin.y < before.y - 1.5 && after.origin.y > before.y - 2.5,
        "translate y should move the glyph by about -2px: {before:?} -> {:?}",
        after.origin
    );
    let GlyphTransform::Affine(affine) = after.transform else {
        panic!("presentation transform should become a glyph affine: {after:?}");
    };
    assert!(
        (affine.values[0] - 1.0).abs() > 0.01
            || affine.values[1].abs() > 0.01
            || affine.values[2].abs() > 0.01
            || (affine.values[3] - 1.0).abs() > 0.01
            || affine.values[4].abs() > 0.01
            || affine.values[5].abs() > 0.01,
        "skew/rotation/scale/origin should produce a non-identity affine: {affine:?}"
    );

    let page_layout = NativePageLayout {
        frame: page_layout_frame.clone(),
        page_start: 0,
        config: native_text_layout_config(800, 600, 96.0, 572.0),
        layout: layout.clone(),
        text_run_indices: (0..layout.runs.len()).collect(),
        ruby_indices: Vec::new(),
    };
    let bounds = native_element_bounds_from_layout_at(&page_layout, 800, 600, 0.0, None);
    let glyph_bounds = bounds
        .iter()
        .find(|bounds| matches!(bounds.element, NativeFrameElement::GlyphCluster { .. }))
        .expect("glyph cluster bounds are reported");
    assert!(
        f64::from(glyph_bounds.bbox.x) > f64::from(layout_bbox_x + 4.0),
        "observed glyph bbox should follow transformed glyph placement: {glyph_bounds:?}"
    );
}

#[test]
fn native_builtin_effect_respects_phase_and_state_scope() {
    let run_scope = RichTextEffectDescriptor {
        id: "shake".to_owned(),
        params: BTreeMap::from([
            (
                "amp".to_owned(),
                RichTextParam::Milli { value: Milli(1000) },
            ),
            (
                "speed".to_owned(),
                RichTextParam::Milli { value: Milli::ZERO },
            ),
            ("seed".to_owned(), RichTextParam::Int { value: 7 }),
        ]),
        target: RichTextEffectTarget::TextBox,
        phase: RichTextEffectPhase::GlyphTransform,
        state_scope: RichTextStateScope::Run,
    };
    let mut first = test_native_glyph_placement(0, 0);
    let mut second = test_native_glyph_placement(0, 1);
    apply_builtin_descriptor("line.scope", &run_scope, 2, 1.0, &mut first);
    apply_builtin_descriptor("line.scope", &run_scope, 2, 1.0, &mut second);
    assert_eq!(
        (first.x, first.y),
        (second.x, second.y),
        "run-scoped shake should move glyphs in the same run together"
    );

    let glyph_scope = RichTextEffectDescriptor {
        state_scope: RichTextStateScope::Glyph,
        ..run_scope.clone()
    };
    let mut first = test_native_glyph_placement(0, 0);
    let mut second = test_native_glyph_placement(0, 1);
    apply_builtin_descriptor("line.scope", &glyph_scope, 2, 1.0, &mut first);
    apply_builtin_descriptor("line.scope", &glyph_scope, 2, 1.0, &mut second);
    assert_ne!(
        (first.x, first.y),
        (second.x, second.y),
        "glyph-scoped shake should keep per-glyph jitter"
    );

    let run_target_wave = RichTextEffectDescriptor {
        id: "wave".to_owned(),
        params: BTreeMap::from([
            (
                "amp".to_owned(),
                RichTextParam::Milli { value: Milli(1000) },
            ),
            (
                "period".to_owned(),
                RichTextParam::Milli { value: Milli(4000) },
            ),
        ]),
        target: RichTextEffectTarget::Run,
        phase: RichTextEffectPhase::GlyphTransform,
        state_scope: RichTextStateScope::Run,
    };
    let mut first = test_native_glyph_placement(0, 0);
    let mut second = test_native_glyph_placement(0, 1);
    apply_builtin_descriptor("line.scope", &run_target_wave, 2, 0.0, &mut first);
    apply_builtin_descriptor("line.scope", &run_target_wave, 2, 0.0, &mut second);
    assert_eq!(
        (first.x, first.y),
        (second.x, second.y),
        "run-targeted wave should move a run as one target"
    );

    let glyph_target_wave = RichTextEffectDescriptor {
        target: RichTextEffectTarget::Glyph,
        ..run_target_wave
    };
    let mut first = test_native_glyph_placement(0, 0);
    let mut second = test_native_glyph_placement(0, 1);
    apply_builtin_descriptor("line.scope", &glyph_target_wave, 2, 0.0, &mut first);
    apply_builtin_descriptor("line.scope", &glyph_target_wave, 2, 0.0, &mut second);
    assert_ne!(
        (first.x, first.y),
        (second.x, second.y),
        "glyph-targeted wave should evaluate per glyph"
    );

    let post_process_phase = RichTextEffectDescriptor {
        phase: RichTextEffectPhase::PostProcess,
        ..run_scope
    };
    let mut placement = test_native_glyph_placement(0, 0);
    apply_builtin_descriptor("line.scope", &post_process_phase, 1, 1.0, &mut placement);
    assert_eq!(
        (placement.x, placement.y),
        (0.0, 0.0),
        "post_process phase should not apply glyph placement"
    );
}

#[test]
fn native_builtin_spin_and_pulse_animate_affine() {
    let spin = RichTextEffectDescriptor {
        id: "spin".to_owned(),
        params: BTreeMap::from([
            (
                "angle".to_owned(),
                RichTextParam::Milli { value: Milli(8000) },
            ),
            (
                "speed".to_owned(),
                RichTextParam::Milli { value: Milli::ONE },
            ),
            (
                "origin".to_owned(),
                RichTextParam::Text {
                    value: "center".to_owned(),
                },
            ),
        ]),
        target: RichTextEffectTarget::Run,
        phase: RichTextEffectPhase::GlyphTransform,
        state_scope: RichTextStateScope::Run,
    };
    let mut spin_early = test_native_glyph_placement(0, 0);
    let mut spin_late = test_native_glyph_placement(0, 0);
    apply_builtin_descriptor("line.scope", &spin, 1, 0.125, &mut spin_early);
    apply_builtin_descriptor("line.scope", &spin, 1, 0.625, &mut spin_late);
    assert!(
        (spin_early.rotate_degrees - spin_late.rotate_degrees).abs() > 0.5,
        "spin should rotate over effect time"
    );
    assert_eq!(
        spin_early.affine_origin,
        Some(RichTextTransformOrigin::Center)
    );
    assert_eq!(spin_early.affine_target, Some(RichTextEffectTarget::Run));

    let pulse = RichTextEffectDescriptor {
        id: "pulse".to_owned(),
        params: BTreeMap::from([
            ("amp".to_owned(), RichTextParam::Milli { value: Milli(160) }),
            (
                "speed".to_owned(),
                RichTextParam::Milli { value: Milli::ONE },
            ),
        ]),
        target: RichTextEffectTarget::Run,
        phase: RichTextEffectPhase::GlyphTransform,
        state_scope: RichTextStateScope::Run,
    };
    let mut pulse_early = test_native_glyph_placement(0, 0);
    let mut pulse_late = test_native_glyph_placement(0, 0);
    apply_builtin_descriptor("line.scope", &pulse, 1, 0.25, &mut pulse_early);
    apply_builtin_descriptor("line.scope", &pulse, 1, 0.75, &mut pulse_late);
    assert!(
        (pulse_early.scale_x - pulse_late.scale_x).abs() > 0.05,
        "pulse should scale over effect time"
    );
    assert_eq!(
        pulse_early.affine_origin,
        Some(RichTextTransformOrigin::Center)
    );
    assert_eq!(pulse_early.affine_target, Some(RichTextEffectTarget::Run));
}

#[test]
fn native_builtin_motion_uses_animation_function_id() {
    let motion = RichTextEffectDescriptor {
        id: "motion".to_owned(),
        params: BTreeMap::from([
            (
                "fn".to_owned(),
                RichTextParam::Raw {
                    value: "breath_orbit".to_owned(),
                },
            ),
            (
                "amp".to_owned(),
                RichTextParam::Milli { value: Milli(5000) },
            ),
            (
                "angle".to_owned(),
                RichTextParam::Milli { value: Milli(9000) },
            ),
            (
                "scale".to_owned(),
                RichTextParam::Milli { value: Milli(140) },
            ),
            (
                "speed".to_owned(),
                RichTextParam::Milli { value: Milli(750) },
            ),
            (
                "seed".to_owned(),
                RichTextParam::Raw {
                    value: "breath".to_owned(),
                },
            ),
        ]),
        target: RichTextEffectTarget::Glyph,
        phase: RichTextEffectPhase::GlyphTransform,
        state_scope: RichTextStateScope::Glyph,
    };
    let mut early = test_native_glyph_placement(0, 2);
    let mut late = test_native_glyph_placement(0, 2);
    apply_builtin_descriptor("line.motion", &motion, 4, 0.15, &mut early);
    apply_builtin_descriptor("line.motion", &motion, 4, 0.65, &mut late);

    assert!(
        (early.x - late.x).abs() > 0.5 || (early.y - late.y).abs() > 0.5,
        "motion should translate over effect time: early={early:?} late={late:?}"
    );
    assert!(
        (early.rotate_degrees - late.rotate_degrees).abs() > 0.25,
        "motion should rotate over effect time: early={early:?} late={late:?}"
    );
    assert!(
        (early.scale_x - late.scale_x).abs() > 0.01,
        "motion should scale over effect time: early={early:?} late={late:?}"
    );
    assert_eq!(
        early.affine_origin,
        Some(RichTextTransformOrigin::GlyphCenter)
    );
    assert_eq!(early.affine_target, Some(RichTextEffectTarget::Glyph));
}

#[test]
fn native_visual_plan_reports_missing_motion_function() {
    let frame = motion_test_frame("snap_rise");

    let plan = visual_plan_from_frame_for_test(&frame, 0.0);

    assert_eq!(plan.diagnostics.len(), 1);
    assert_eq!(
        plan.diagnostics[0].severity,
        NativeVisualDiagnosticSeverity::Warning
    );
    assert_eq!(plan.diagnostics[0].code, "missing_motion_function");
    assert_eq!(plan.diagnostics[0].effect_id.as_deref(), Some("motion"));
}

#[test]
fn native_capture_uses_custom_motion_registry_for_submitted_glyphs() {
    let frame = motion_test_frame("snap_rise");
    let mut baseline = NativeOffscreenCaptureSession::new().expect("baseline session");
    let baseline_capture = baseline
        .capture_frame_rgba_at(&frame, 800, 600, 96.0, 572.0)
        .expect("baseline capture");

    let mut shifted = NativeOffscreenCaptureSession::new().expect("shifted session");
    shifted
        .motion_registry_mut()
        .insert_lambda("snap_rise", |_ctx| NativeAnimationSample {
            translate: [1.0, -0.25],
            rotate: 0.0,
            scale: 0.0,
        });
    let shifted_capture = shifted
        .capture_frame_rgba_at(&frame, 800, 600, 96.0, 572.0)
        .expect("shifted capture");

    let baseline_bbox = baseline_capture.content_bbox.expect("baseline content");
    let shifted_bbox = shifted_capture.content_bbox.expect("shifted content");
    assert!(
        baseline_capture
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "missing_motion_function")
    );
    assert!(shifted_capture.diagnostics.is_empty());
    assert!(
        shifted_bbox.x > baseline_bbox.x + 32,
        "custom motion registry should move submitted glyph pixels: {baseline_bbox:?} -> {shifted_bbox:?}"
    );
}

#[test]
fn native_motion_registry_exports_arcweft_pure_text_motion_function() {
    let candidates = arcweft_text_registry_candidates(
        r"
#[text_motion]
#[pure]
fn arc_phase(t: f32, glyph: f32, seed: f32) -> f32 {
return t + glyph * 0.125f32 + seed * 0.001f32
}
",
    );
    let mut registry = RichTextMotionRegistry::default();

    let exported = register_arcweft_pure_text_motions(&mut registry, &candidates.motions)
        .expect("motion exports");

    assert_eq!(exported, 1);
    assert!(registry.contains("arc_phase"));
    let effect = RichTextEffectDescriptor {
        id: "motion".to_owned(),
        params: BTreeMap::new(),
        target: RichTextEffectTarget::Glyph,
        phase: RichTextEffectPhase::GlyphTransform,
        state_scope: RichTextStateScope::Glyph,
    };
    let first = registry
        .sample("arc_phase", &text_motion_context(&effect, 0, 4))
        .expect("first pure text motion sample");
    let later_glyph = registry
        .sample("arc_phase", &text_motion_context(&effect, 2, 4))
        .expect("glyph-dependent pure text motion sample");
    assert_ne!(
        first, later_glyph,
        "Arcweft pure text motion body should affect native motion sampling"
    );
}

#[test]
fn native_capture_uses_arcweft_pure_text_motion_registry_export() {
    let candidates = arcweft_text_registry_candidates(
        r"
#[text_motion]
#[pure]
fn snap_arc(t: f32, glyph: f32, seed: f32) -> f32 {
return t + 0.25f32 + glyph * 0.05f32 + seed * 0.001f32
}
",
    );
    let frame = motion_test_frame("snap_arc");
    let mut baseline = NativeOffscreenCaptureSession::new().expect("baseline session");
    let baseline_capture = baseline
        .capture_frame_rgba_at(&frame, 800, 600, 96.0, 572.0)
        .expect("baseline capture");

    let mut exported = NativeOffscreenCaptureSession::new().expect("exported session");
    let export_count =
        register_arcweft_pure_text_motions(exported.motion_registry_mut(), &candidates.motions)
            .expect("register Arcweft pure text motion");
    let exported_capture = exported
        .capture_frame_rgba_at(&frame, 800, 600, 96.0, 572.0)
        .expect("exported capture");

    assert_eq!(export_count, 1);
    assert!(
        baseline_capture
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "missing_motion_function")
    );
    assert!(exported_capture.diagnostics.is_empty());
    assert_ne!(
        baseline_capture.rgba, exported_capture.rgba,
        "Arcweft pure text motion export should alter captured native glyph pixels"
    );
}

#[test]
fn native_effect_registry_exports_arcweft_pure_text_effect_function() {
    let candidates = arcweft_text_registry_candidates(
        r"
#[text_effect]
#[pure]
fn source_drift(t: f32, glyph: f32, seed: f32) -> f32 {
return t + glyph * 0.125f32 + seed * 0.001f32
}
",
    );
    let mut registry = RichTextEffectRegistry::default();

    let exported = register_arcweft_pure_text_effects(&mut registry, &candidates.effects)
        .expect("effect exports");

    assert_eq!(exported, 1);
    assert!(registry.contains("source_drift"));
    assert!(registry.supports_phase("source_drift", RichTextEffectPhase::GlyphColor));
    assert!(registry.supports_phase("source_drift", RichTextEffectPhase::PostProcess));
}

#[test]
fn native_capture_uses_arcweft_pure_text_effect_registry_export() {
    let candidates = arcweft_text_registry_candidates(
        r"
#[text_effect]
#[pure]
fn source_drift(t: f32, glyph: f32, seed: f32) -> f32 {
return t + 0.25f32 + glyph * 0.05f32 + seed * 0.001f32
}
",
    );
    let frame = custom_effect_test_frame_with_params(
        "source_drift",
        BTreeMap::from([
            (
                "amp".to_owned(),
                RichTextParam::Milli {
                    value: Milli(48_000),
                },
            ),
            (
                "angle".to_owned(),
                RichTextParam::Milli {
                    value: Milli(8_000),
                },
            ),
            (
                "scale".to_owned(),
                RichTextParam::Milli { value: Milli(120) },
            ),
            (
                "seed".to_owned(),
                RichTextParam::Raw {
                    value: "source-effect".to_owned(),
                },
            ),
        ]),
    );
    let mut baseline = NativeOffscreenCaptureSession::new().expect("baseline session");
    let baseline_capture = baseline
        .capture_frame_rgba_at(&frame, 800, 600, 96.0, 572.0)
        .expect("baseline capture");

    let mut exported = NativeOffscreenCaptureSession::new().expect("exported session");
    let export_count =
        register_arcweft_pure_text_effects(exported.effect_registry_mut(), &candidates.effects)
            .expect("register Arcweft pure text effect");
    let exported_capture = exported
        .capture_frame_rgba_at(&frame, 800, 600, 96.0, 572.0)
        .expect("exported capture");

    assert_eq!(export_count, 1);
    assert!(
        baseline_capture
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "missing_custom_effect")
    );
    assert!(exported_capture.diagnostics.is_empty());
    assert_ne!(
        baseline_capture.rgba, exported_capture.rgba,
        "Arcweft pure text effect export should alter captured native glyph pixels"
    );
}

#[test]
fn native_capture_uses_arcweft_pure_text_effect_post_process_export() {
    let candidates = arcweft_text_registry_candidates(
        r"
#[text_effect]
#[pure]
fn source_drift(t: f32, glyph: f32, seed: f32) -> f32 {
return t + 0.25f32 + glyph * 0.05f32 + seed * 0.001f32
}
",
    );
    let frame = custom_effect_test_frame_with_params_and_phase(
        "source_drift",
        BTreeMap::from([
            (
                "amount".to_owned(),
                RichTextParam::Milli { value: Milli::ONE },
            ),
            (
                "seed".to_owned(),
                RichTextParam::Raw {
                    value: "source-post-effect".to_owned(),
                },
            ),
        ]),
        RichTextEffectPhase::PostProcess,
    );
    let mut baseline = NativeOffscreenCaptureSession::new().expect("baseline session");
    let baseline_capture = baseline
        .capture_frame_rgba_at(&frame, 800, 600, 96.0, 572.0)
        .expect("baseline capture");

    let mut exported = NativeOffscreenCaptureSession::new().expect("exported session");
    let export_count =
        register_arcweft_pure_text_effects(exported.effect_registry_mut(), &candidates.effects)
            .expect("register Arcweft pure text effect");
    let exported_capture = exported
        .capture_frame_rgba_at(&frame, 800, 600, 96.0, 572.0)
        .expect("exported capture");

    assert_eq!(export_count, 1);
    assert!(
        baseline_capture
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "missing_custom_effect")
    );
    assert!(exported_capture.diagnostics.is_empty());
    assert_ne!(
        baseline_capture.rgba, exported_capture.rgba,
        "Arcweft pure text effect post-process export should alter captured framebuffer pixels"
    );
}

#[test]
fn native_shader_registry_exports_arcweft_pure_text_shader_function() {
    let candidates = arcweft_text_registry_candidates(
        r"
#[text_shader]
#[pure]
fn source_glow(t: f32, glyph: f32, seed: f32) -> f32 {
return t + glyph * 0.125f32 + seed * 0.001f32
}
",
    );
    let mut registry = RichTextShaderRegistry::default();

    let exported = register_arcweft_pure_text_shaders(&mut registry, &candidates.shaders)
        .expect("shader exports");

    assert_eq!(exported, 1);
    assert!(registry.contains("source_glow"));
    assert!(registry.supports_phase("source_glow", RichTextEffectPhase::GlyphColor));
    assert!(registry.supports_phase("source_glow", RichTextEffectPhase::PostProcess));
}

#[test]
fn native_capture_uses_arcweft_pure_text_shader_registry_export() {
    let candidates = arcweft_text_registry_candidates(
        r"
#[text_shader]
#[pure]
fn source_glow(t: f32, glyph: f32, seed: f32) -> f32 {
return t + 0.25f32 + glyph * 0.05f32 + seed * 0.001f32
}
",
    );
    let frame = shader_test_frame("source_glow", RichTextEffectPhase::GlyphColor);
    let mut baseline = NativeOffscreenCaptureSession::new().expect("baseline session");
    let baseline_capture = baseline
        .capture_frame_rgba_at(&frame, 800, 600, 96.0, 572.0)
        .expect("baseline capture");

    let mut exported = NativeOffscreenCaptureSession::new().expect("exported session");
    let export_count =
        register_arcweft_pure_text_shaders(exported.shader_registry_mut(), &candidates.shaders)
            .expect("register Arcweft pure text shader");
    let exported_capture = exported
        .capture_frame_rgba_at(&frame, 800, 600, 96.0, 572.0)
        .expect("exported capture");

    assert_eq!(export_count, 1);
    assert!(
        baseline_capture
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "missing_shader")
    );
    assert!(exported_capture.diagnostics.is_empty());
    assert_ne!(
        baseline_capture.rgba, exported_capture.rgba,
        "Arcweft pure text shader export should alter captured native glyph pixels"
    );
}

#[test]
fn native_capture_uses_arcweft_pure_text_shader_post_process_export() {
    let candidates = arcweft_text_registry_candidates(
        r"
#[text_shader]
#[pure]
fn source_glow(t: f32, glyph: f32, seed: f32) -> f32 {
return t + 0.25f32 + glyph * 0.05f32 + seed * 0.001f32
}
",
    );
    let frame = shader_test_frame("source_glow", RichTextEffectPhase::PostProcess);
    let mut baseline = NativeOffscreenCaptureSession::new().expect("baseline session");
    let baseline_capture = baseline
        .capture_frame_rgba_at(&frame, 800, 600, 96.0, 572.0)
        .expect("baseline capture");

    let mut exported = NativeOffscreenCaptureSession::new().expect("exported session");
    let export_count =
        register_arcweft_pure_text_shaders(exported.shader_registry_mut(), &candidates.shaders)
            .expect("register Arcweft pure text shader");
    let exported_capture = exported
        .capture_frame_rgba_at(&frame, 800, 600, 96.0, 572.0)
        .expect("exported capture");

    assert_eq!(export_count, 1);
    assert!(
        baseline_capture
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "missing_shader")
    );
    assert!(exported_capture.diagnostics.is_empty());
    assert_ne!(
        baseline_capture.rgba, exported_capture.rgba,
        "Arcweft pure text shader post-process export should alter captured framebuffer pixels"
    );
}

#[test]
fn glyph_presentation_affine_uses_transform_target_bounds_for_center_pivot() {
    let transform = RichTextTransform {
        rotate: RichTextAngle {
            degrees: Milli(180_000),
        },
        origin: RichTextTransformOrigin::Center,
        target: RichTextEffectTarget::TextBox,
        ..RichTextTransform::default()
    };
    let presentation = RichTextPresentation {
        transform: Some(transform),
        ..RichTextPresentation::default()
    };
    let glyph = LaidOutGlyph {
        run_index: 0,
        range: RichTextRange::new(0, 1),
        text: "A".to_owned(),
        origin: LayoutPoint::new(0.0, 0.0),
        advance: LayoutSize::new(10.0, 0.0),
        bounds: LayoutRect::new(0.0, 0.0, 10.0, 10.0),
        writing_mode: RichTextWritingMode::HorizontalTb,
        orientation: GlyphOrientation::Upright,
        vertical_form: GlyphVerticalForm::None,
        presentation,
    };
    let layout = LaidOutText {
        glyphs: vec![glyph.clone()],
        runs: vec![arcweft_text_layout::LaidOutRun {
            run_index: 0,
            range: RichTextRange::new(0, 1),
            bounds: LayoutRect::new(0.0, 0.0, 10.0, 10.0),
            writing_mode: RichTextWritingMode::HorizontalTb,
            presentation: glyph.presentation.clone(),
        }],
        ruby: Vec::new(),
        bounds: Some(LayoutRect::new(0.0, 0.0, 30.0, 10.0)),
    };
    let mut placement = test_native_glyph_placement(0, 0);
    placement.rotate_degrees = 180.0;

    let affine = glyph_presentation_affine(&placement, &glyph, &layout).expect("affine resolves");

    assert!(
        affine[4] > 29.5 && affine[4] < 30.5,
        "textbox center pivot should translate around x=15, not glyph-local center: {affine:?}"
    );
}

fn test_native_glyph_placement(run_index: usize, glyph_index: usize) -> NativeGlyphPlacement {
    NativeGlyphPlacement {
        run_index,
        glyph_index,
        range: glyph_index..glyph_index + 1,
        x: 0.0,
        y: 0.0,
        rotate_degrees: 0.0,
        skew_x_degrees: 0.0,
        skew_y_degrees: 0.0,
        affine_origin: None,
        affine_target: None,
        vertical_form: GlyphVerticalForm::None,
        scale_x: 1.0,
        scale_y: 1.0,
        opacity: 1.0,
        color: None,
    }
}

#[test]
fn ruby_buffers_without_layout_require_shaped_base_geometry() {
    let frame = styled_ruby_test_frame();
    let pages = WindowPage::from_frame(&frame);
    let rich_text = &pages[0].rich_text;
    let mut font_system = FontSystem::new();
    let empty_buffer = Buffer::new(&mut font_system, Metrics::new(30.0, 42.0));

    let ruby_buffers = build_ruby_buffers(
        &mut font_system,
        &empty_buffer,
        rich_text,
        None,
        800,
        600,
        NativeTextOrigin::default(),
    );

    assert!(
        ruby_buffers.is_empty(),
        "ruby buffers without layout should not fall back to estimated positions"
    );
}

fn assert_ruby_glyph_areas_use_absolute_glypharea(
    font_system: &mut FontSystem,
    text_buffer: &Buffer,
    rich_text: &WindowRichText,
    pages: &[WindowPage],
) {
    let layout = layout_frame(
        pages[0].layout_frame.as_ref().expect("layout frame"),
        native_text_layout_config(800, 600, 0.0, 0.0),
    )
    .expect("layout resolves");
    let ruby_buffers = build_ruby_buffers(
        font_system,
        text_buffer,
        rich_text,
        Some(&layout),
        800,
        600,
        NativeTextOrigin::default(),
    );
    let ruby_glyph_areas =
        ruby_glyph_areas(&ruby_buffers, &rich_text.text, 800, 600, 60.0, false, None);
    assert_eq!(ruby_glyph_areas.len(), 1);
    assert!(!ruby_glyph_areas[0].is_empty());
    assert!((ruby_glyph_areas[0].as_glyph_area().left - 0.0).abs() < f32::EPSILON);
    assert!((ruby_glyph_areas[0].as_glyph_area().top - 0.0).abs() < f32::EPSILON);
    assert!(ruby_glyph_areas[0].glyphs()[0].origin.x >= layout.ruby[0].ruby_bounds.x.floor());
    if matches!(
        layout.ruby[0].writing_mode,
        RichTextWritingMode::HorizontalTb
    ) {
        let glyph_y = ruby_glyph_areas[0].glyphs()[0].origin.y;
        assert!(
            glyph_y >= layout.ruby[0].ruby_bounds.y && glyph_y < layout.ruby[0].base_bounds.y,
            "horizontal ruby glyph y should stay in the annotation track toward the base: glyph={:?}, ruby={:?}, base={:?}",
            ruby_glyph_areas[0].glyphs()[0].origin,
            layout.ruby[0].ruby_bounds,
            layout.ruby[0].base_bounds,
        );
    }
}

#[test]
fn window_pages_keep_vertical_layout_source_for_glyph_area_rendering() {
    let spec = LineDisplaySpec {
        line: RuntimeLineId("say.test.vertical.window".to_owned()),
        callee: "alice".to_owned(),
        speaker_label: None,
        text_key: None,
        window: None,
        voice: None,
        look: None,
        style: None,
        base_styles: Vec::new(),
        default_inline_failure_policy: None,
        style_contributions: Vec::new(),
        args: Vec::new(),
        content: RichTextDocument::new(vec![
            RichTextNode::StyleStart {
                style: RichTextStyle::Layout {
                    layout: RichTextLayout {
                        writing_mode: RichTextWritingMode::VerticalRl,
                        ..RichTextLayout::default()
                    },
                },
            },
            RichTextNode::Text {
                text: "縦Ａ。ー".to_owned(),
            },
            RichTextNode::StyleEnd {
                name: "/".to_owned(),
            },
        ]),
    };
    let frame = spec
        .resolve_frame(&RuntimeLineContext::default())
        .expect("frame resolves");
    let page = WindowPage::from_frame(&frame)
        .into_iter()
        .next()
        .expect("page exists");
    let layout_frame = page
        .layout_frame
        .as_ref()
        .expect("page keeps layout source");
    let layout = layout_frame
        .display_map
        .text_runs
        .iter()
        .find_map(|run| run.presentation.layout.as_ref())
        .expect("layout presentation is preserved");

    assert_eq!(page.rich_text.text, "縦Ａ。ー");
    assert_eq!(layout.writing_mode, RichTextWritingMode::VerticalRl);

    let plan = visual_plan_from_frame_for_test(&frame, 0.0);
    let visual_page = plan.pages.first().expect("visual page exists");
    let vertical_form_for = |text: &str| {
        visual_page
            .glyphs
            .iter()
            .find(|glyph| visual_page.text.get(glyph.range.clone()) == Some(text))
            .map(|glyph| glyph.vertical_form)
            .expect("glyph placement exists")
    };

    assert_eq!(vertical_form_for("。"), GlyphVerticalForm::UprightAlternate);
    assert_eq!(vertical_form_for("ー"), GlyphVerticalForm::RotatedAlternate);
}

#[test]
fn window_pages_keep_vertical_ruby_text_combine_source_for_glyph_area_rendering() {
    for writing_mode in [
        RichTextWritingMode::VerticalRl,
        RichTextWritingMode::VerticalLr,
    ] {
        let frame = vertical_ruby_text_combine_frame(writing_mode);
        let page = WindowPage::from_frame(&frame)
            .into_iter()
            .next()
            .expect("page exists");

        assert_eq!(page.rich_text.text, "天地夢2026Z");
        assert_eq!(page.rich_text.ruby_annotations.len(), 1);
        assert_eq!(page.rich_text.ruby_annotations[0].base_range, 6..9);

        let page_layout_frame = page
            .layout_frame
            .as_ref()
            .expect("window page keeps page-local layout source");
        let layout_presentation = page_layout_frame
            .display_map
            .text_runs
            .iter()
            .find_map(|run| run.presentation.layout.as_ref())
            .expect("layout presentation is preserved");
        assert_eq!(layout_presentation.writing_mode, writing_mode);

        let layout = layout_frame(
            page_layout_frame,
            native_text_layout_config(800, 600, 96.0, 572.0),
        )
        .expect("layout resolves");
        let combine_index = layout
            .glyphs
            .iter()
            .position(|glyph| glyph.text == "2026")
            .expect("text-combine glyph exists");
        assert_eq!(
            layout.glyphs[combine_index].orientation,
            GlyphOrientation::TextCombineUpright
        );

        let mut font_system = FontSystem::new();
        let mut buffer = Buffer::new(&mut font_system, Metrics::new(30.0, 42.0));
        prepare_window_text_buffers(&mut font_system, &mut buffer, &page.rich_text, 800, 600);
        let cache_keys =
            layout_glyph_cache_keys(&mut font_system, &buffer, &page.rich_text, &layout);
        let glyph_area = glyph_area_from_layout(
            &layout,
            GlyphonAreaOptions {
                bounds: native_text_bounds(800, 600),
                origin_offset: Vector::new(0.0, NATIVE_GLYPHAREA_BASELINE_OFFSET),
                ..GlyphonAreaOptions::default()
            },
            |index, glyph| cache_keys_for_layout_glyph(index, glyph.range, &cache_keys),
        )
        .expect("window layout source adapts to glyph area");
        assert_eq!(
            glyph_area
                .glyphs()
                .iter()
                .filter(|glyph| glyph.metadata == combine_index)
                .count(),
            4,
            "{writing_mode:?} text-combine cluster should expand to one glyph instance per digit"
        );

        let ruby_buffers = build_ruby_buffers(
            &mut font_system,
            &buffer,
            &page.rich_text,
            Some(&layout),
            800,
            600,
            NativeTextOrigin::default(),
        );
        let ruby_glyph_areas = ruby_glyph_areas(
            &ruby_buffers,
            &page.rich_text.text,
            800,
            600,
            60.0,
            false,
            None,
        );
        assert_vertical_ruby_glyph_areas_align_toward_base(
            writing_mode,
            &layout,
            &ruby_buffers,
            &ruby_glyph_areas,
        );
    }
}

fn assert_vertical_ruby_glyph_areas_align_toward_base(
    writing_mode: RichTextWritingMode,
    layout: &LaidOutText,
    ruby_buffers: &[WindowRubyBuffer],
    ruby_glyph_areas: &[OwnedGlyphArea],
) {
    assert_eq!(ruby_buffers.len(), 1);
    assert!(matches!(
        ruby_buffers[0].placement,
        RubyGlyphPlacement::Vertical { .. }
    ));
    assert_eq!(layout.ruby[0].writing_mode, writing_mode);
    let ruby_center = layout.ruby[0].ruby_bounds.x + layout.ruby[0].ruby_bounds.width * 0.5;
    let base_center = layout.ruby[0].base_bounds.x + layout.ruby[0].base_bounds.width * 0.5;
    match writing_mode {
        RichTextWritingMode::VerticalRl => {
            assert!(
                ruby_center > base_center,
                "vertical_rl ruby should render on the right annotation track"
            );
            assert!(matches!(
                ruby_buffers[0].placement,
                RubyGlyphPlacement::Vertical {
                    horizontal_align: VerticalGlyphHorizontalAlign::Start,
                    ..
                }
            ));
        }
        RichTextWritingMode::VerticalLr => {
            assert!(
                ruby_center < base_center,
                "vertical_lr ruby should render on the left annotation track"
            );
            assert!(matches!(
                ruby_buffers[0].placement,
                RubyGlyphPlacement::Vertical {
                    horizontal_align: VerticalGlyphHorizontalAlign::End,
                    ..
                }
            ));
        }
        RichTextWritingMode::HorizontalTb => unreachable!("test uses vertical modes"),
    }
    assert_eq!(ruby_glyph_areas.len(), 1);
    assert!(
        ruby_glyph_areas[0]
            .glyphs()
            .iter()
            .all(|glyph| glyph.origin.x >= layout.ruby[0].ruby_bounds.x.floor())
    );
    let first_ruby_glyph = &ruby_glyph_areas[0].glyphs()[0];
    match writing_mode {
        RichTextWritingMode::VerticalRl => assert!(
            (first_ruby_glyph.origin.x - layout.ruby[0].ruby_bounds.x).abs() <= 1.0,
            "vertical_rl ruby glyph ink should align toward the base-side track edge"
        ),
        RichTextWritingMode::VerticalLr => assert!(
            (first_ruby_glyph.origin.x + first_ruby_glyph.ink_bounds.width()
                - layout.ruby[0].ruby_bounds.right())
            .abs()
                <= 1.0,
            "vertical_lr ruby glyph ink should align toward the base-side track edge"
        ),
        RichTextWritingMode::HorizontalTb => unreachable!("test uses vertical modes"),
    }
}

#[test]
fn overheight_vertical_ruby_segments_render_as_multiple_glyph_areas() {
    for (writing_mode, continuation_moves_right) in [
        (RichTextWritingMode::VerticalRl, true),
        (RichTextWritingMode::VerticalLr, false),
    ] {
        assert_overheight_vertical_ruby_glyph_areas(writing_mode, continuation_moves_right);
    }
}

fn assert_overheight_vertical_ruby_glyph_areas(
    writing_mode: RichTextWritingMode,
    continuation_moves_right: bool,
) {
    let spec = LineDisplaySpec {
        line: RuntimeLineId(format!("say.test.vertical.ruby.split.{writing_mode:?}")),
        callee: "alice".to_owned(),
        speaker_label: None,
        text_key: None,
        window: None,
        voice: None,
        look: None,
        style: None,
        base_styles: Vec::new(),
        default_inline_failure_policy: None,
        style_contributions: Vec::new(),
        args: Vec::new(),
        content: RichTextDocument::new(vec![
            RichTextNode::StyleStart {
                style: RichTextStyle::Layout {
                    layout: RichTextLayout {
                        writing_mode,
                        ..RichTextLayout::default()
                    },
                },
            },
            RichTextNode::Ruby {
                base: "夢".to_owned(),
                ruby: "あいうえお".to_owned(),
            },
            RichTextNode::StyleEnd {
                name: "/".to_owned(),
            },
        ]),
    };
    let frame = spec
        .resolve_frame(&RuntimeLineContext::default())
        .expect("frame resolves");
    let page = WindowPage::from_frame(&frame)
        .into_iter()
        .next()
        .expect("page exists");
    let layout = layout_frame(
        page.layout_frame.as_ref().expect("layout frame"),
        TextLayoutConfig {
            size: LayoutSize::new(160.0, 42.0),
            ruby_font_size: 14.0,
            ..native_text_layout_config(800, 600, 0.0, 0.0)
        },
    )
    .expect("layout resolves");
    assert_eq!(layout.ruby.len(), 2);
    let mut font_system = FontSystem::new();
    let mut buffer = Buffer::new(&mut font_system, Metrics::new(30.0, 42.0));
    prepare_window_text_buffers(&mut font_system, &mut buffer, &page.rich_text, 800, 600);
    let ruby_buffers = build_ruby_buffers(
        &mut font_system,
        &buffer,
        &page.rich_text,
        Some(&layout),
        800,
        600,
        NativeTextOrigin::default(),
    );
    let ruby_glyph_areas = ruby_glyph_areas(
        &ruby_buffers,
        &page.rich_text.text,
        800,
        600,
        60.0,
        false,
        None,
    );
    assert_ruby_continuation_track(&ruby_buffers, continuation_moves_right);
    assert_vertical_ruby_glyph_placement(&ruby_buffers[0].placement, &layout);
    assert_eq!(ruby_glyph_areas.len(), 2);
    assert_split_ruby_glyph_area_geometry(&ruby_glyph_areas, &layout, continuation_moves_right);
}

fn assert_ruby_continuation_track(
    ruby_buffers: &[WindowRubyBuffer],
    continuation_moves_right: bool,
) {
    if continuation_moves_right {
        assert!(ruby_buffers[1].left > ruby_buffers[0].left);
    } else {
        assert!(ruby_buffers[1].left < ruby_buffers[0].left);
    }
}

fn assert_vertical_ruby_glyph_placement(placement: &RubyGlyphPlacement, layout: &LaidOutText) {
    let RubyGlyphPlacement::Vertical {
        cell_width: w,
        vertical_advance: advance,
        horizontal_align: _,
    } = *placement
    else {
        panic!("vertical layout ruby should use vertical glyph placement");
    };
    assert!((w - layout.ruby[0].ruby_bounds.width).abs() < f32::EPSILON);
    assert!((advance - layout.ruby[0].ruby_bounds.height / 3.0).abs() < 0.0001);
}

fn assert_split_ruby_glyph_area_geometry(
    ruby_glyph_areas: &[OwnedGlyphArea],
    layout: &LaidOutText,
    continuation_moves_right: bool,
) {
    assert!(
        ruby_glyph_areas[0].glyphs()[1].origin.y > ruby_glyph_areas[0].glyphs()[0].origin.y,
        "vertical ruby glyphs should advance downward inside each segment"
    );
    assert!(
        (ruby_glyph_areas[0].glyphs()[1].origin.x - ruby_glyph_areas[0].glyphs()[0].origin.x).abs()
            <= layout.ruby[0].ruby_bounds.width,
        "vertical ruby glyphs should remain in the same annotation track"
    );
    if continuation_moves_right {
        assert!(
            ruby_glyph_areas[1].glyphs()[0].origin.x > ruby_glyph_areas[0].glyphs()[0].origin.x
        );
    } else {
        assert!(
            ruby_glyph_areas[1].glyphs()[0].origin.x < ruby_glyph_areas[0].glyphs()[0].origin.x
        );
    }
}

#[test]
#[allow(clippy::too_many_lines)]
fn ruby_glyph_areas_apply_typewriter_visibility_alpha() {
    let spec = LineDisplaySpec {
        line: RuntimeLineId("say.test.vertical.ruby.typewriter.alpha".to_owned()),
        callee: "alice".to_owned(),
        speaker_label: None,
        text_key: None,
        window: None,
        voice: None,
        look: None,
        style: None,
        base_styles: Vec::new(),
        default_inline_failure_policy: None,
        style_contributions: Vec::new(),
        args: Vec::new(),
        content: RichTextDocument::new(vec![
            RichTextNode::StyleStart {
                style: RichTextStyle::Layout {
                    layout: RichTextLayout {
                        writing_mode: RichTextWritingMode::VerticalRl,
                        ..RichTextLayout::default()
                    },
                },
            },
            RichTextNode::StyleStart {
                style: RichTextStyle::Effect {
                    effect: RichTextEffectDescriptor {
                        id: "typewriter".to_owned(),
                        params: BTreeMap::from([(
                            "cps".to_owned(),
                            RichTextParam::Milli { value: Milli::ONE },
                        )]),
                        target: RichTextEffectTarget::Run,
                        phase: RichTextEffectPhase::GlyphMask,
                        state_scope: arcweft_render_text::RichTextStateScope::Run,
                    },
                },
            },
            RichTextNode::Ruby {
                base: "夢".to_owned(),
                ruby: "ながいよみ".to_owned(),
            },
            RichTextNode::StyleEnd {
                name: "effect".to_owned(),
            },
            RichTextNode::StyleEnd {
                name: "layout".to_owned(),
            },
        ]),
    };
    let frame = spec
        .resolve_frame(&RuntimeLineContext::default())
        .expect("frame resolves");
    let page = WindowPage::from_frame(&frame)
        .into_iter()
        .next()
        .expect("page exists");
    let layout = layout_frame(
        page.layout_frame.as_ref().expect("layout frame"),
        native_text_layout_config(800, 600, 0.0, 0.0),
    )
    .expect("layout resolves");

    let mut font_system = FontSystem::new();
    let mut buffer = Buffer::new(&mut font_system, Metrics::new(30.0, 42.0));
    prepare_window_text_buffers(&mut font_system, &mut buffer, &page.rich_text, 800, 600);
    let ruby_buffers = build_ruby_buffers(
        &mut font_system,
        &buffer,
        &page.rich_text,
        Some(&layout),
        800,
        600,
        NativeTextOrigin::default(),
    );

    let hidden = ruby_glyph_areas(
        &ruby_buffers,
        &page.rich_text.text,
        800,
        600,
        0.0,
        false,
        None,
    );
    let visible = ruby_glyph_areas(
        &ruby_buffers,
        &page.rich_text.text,
        800,
        600,
        4.0,
        false,
        None,
    );

    assert!(!hidden.is_empty());
    assert!(
        hidden
            .iter()
            .flat_map(OwnedGlyphArea::glyphs)
            .all(|glyph| glyph.color == Some(Color::rgba(170, 190, 220, 0)))
    );
    assert!(
        visible
            .iter()
            .flat_map(OwnedGlyphArea::glyphs)
            .all(|glyph| glyph.color == Some(Color::rgba(170, 190, 220, 255)))
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn ruby_glyph_areas_apply_transform_affine_and_builtin_translation() {
    let spec = LineDisplaySpec {
        line: RuntimeLineId("say.test.ruby.transform.glyph-area".to_owned()),
        callee: "alice".to_owned(),
        speaker_label: None,
        text_key: None,
        window: None,
        voice: None,
        look: None,
        style: None,
        base_styles: Vec::new(),
        default_inline_failure_policy: None,
        style_contributions: Vec::new(),
        args: Vec::new(),
        content: RichTextDocument::new(vec![
            RichTextNode::StyleStart {
                style: RichTextStyle::Transform {
                    transform: RichTextTransform {
                        translate: RichTextVec2::new(Milli(5000), Milli(-2000)),
                        rotate: RichTextAngle {
                            degrees: Milli(10000),
                        },
                        scale: RichTextVec2::new(Milli(1200), Milli(900)),
                        skew: RichTextVec2::new(Milli(10000), Milli::ZERO),
                        origin: RichTextTransformOrigin::Center,
                        target: RichTextEffectTarget::TextBox,
                    },
                },
            },
            RichTextNode::StyleStart {
                style: RichTextStyle::Effect {
                    effect: RichTextEffectDescriptor {
                        id: "wave".to_owned(),
                        params: BTreeMap::from([
                            (
                                "amp".to_owned(),
                                RichTextParam::Milli { value: Milli(3000) },
                            ),
                            (
                                "dir".to_owned(),
                                RichTextParam::Vec2 {
                                    value: RichTextVec2::new(Milli::ONE, Milli::ZERO),
                                },
                            ),
                            (
                                "phase".to_owned(),
                                RichTextParam::Milli { value: Milli(250) },
                            ),
                        ]),
                        target: RichTextEffectTarget::TextBox,
                        phase: RichTextEffectPhase::GlyphTransform,
                        state_scope: RichTextStateScope::Run,
                    },
                },
            },
            RichTextNode::Ruby {
                base: "揺".to_owned(),
                ruby: "ゆれ".to_owned(),
            },
            RichTextNode::StyleEnd {
                name: "effect".to_owned(),
            },
            RichTextNode::StyleEnd {
                name: "transform".to_owned(),
            },
        ]),
    };
    let frame = spec
        .resolve_frame(&RuntimeLineContext::default())
        .expect("frame resolves");
    let page = WindowPage::from_frame(&frame)
        .into_iter()
        .next()
        .expect("page exists");
    let layout = layout_frame(
        page.layout_frame.as_ref().expect("layout frame"),
        native_text_layout_config(800, 600, 96.0, 572.0),
    )
    .expect("layout resolves");

    let mut font_system = FontSystem::new();
    let mut buffer = Buffer::new(&mut font_system, Metrics::new(30.0, 42.0));
    prepare_window_text_buffers(&mut font_system, &mut buffer, &page.rich_text, 800, 600);
    let ruby_buffers = build_ruby_buffers(
        &mut font_system,
        &buffer,
        &page.rich_text,
        Some(&layout),
        800,
        600,
        NativeTextOrigin::default(),
    );
    assert!(!ruby_buffers.is_empty());

    let bounds = native_text_bounds(800, 600);
    let before_area = match ruby_buffers[0].placement {
        RubyGlyphPlacement::Horizontal { line_height } => horizontal_glyph_area_from_shaped_buffer(
            &ruby_buffers[0].buffer,
            ruby_glyph_area_options(bounds, ruby_buffers[0].left, ruby_buffers[0].top, false),
            line_height,
        ),
        RubyGlyphPlacement::Vertical {
            cell_width,
            vertical_advance,
            horizontal_align,
        } => vertical_glyph_area_from_shaped_buffer(
            &ruby_buffers[0].buffer,
            ruby_glyph_area_options(bounds, ruby_buffers[0].left, ruby_buffers[0].top, false),
            cell_width,
            vertical_advance,
            horizontal_align,
        ),
    };
    let after_areas = ruby_glyph_areas(
        &ruby_buffers,
        &page.rich_text.text,
        800,
        600,
        0.0,
        false,
        None,
    );

    let before = before_area.glyphs()[0].origin;
    let after = &after_areas[0].glyphs()[0];
    assert!(
        after.origin.x > before.x + 7.5 && after.origin.x < before.x + 8.5,
        "ruby translate x plus wave should move the glyph by about 8px: {before:?} -> {:?}",
        after.origin
    );
    assert!(
        after.origin.y < before.y - 1.5 && after.origin.y > before.y - 2.5,
        "ruby translate y should move the glyph by about -2px: {before:?} -> {:?}",
        after.origin
    );
    let GlyphTransform::Affine(affine) = after.transform else {
        panic!("ruby presentation transform should become a glyph affine: {after:?}");
    };
    assert!(
        (affine.values[0] - 1.0).abs() > 0.01
            || affine.values[1].abs() > 0.01
            || affine.values[2].abs() > 0.01
            || (affine.values[3] - 1.0).abs() > 0.01
            || affine.values[4].abs() > 0.01
            || affine.values[5].abs() > 0.01,
        "ruby skew/rotation/scale/origin should produce a non-identity affine: {affine:?}"
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn native_ruby_element_bounds_follow_transform_and_builtin_translation() {
    let spec = LineDisplaySpec {
        line: RuntimeLineId("say.test.ruby.transform.bounds".to_owned()),
        callee: "alice".to_owned(),
        speaker_label: None,
        text_key: None,
        window: None,
        voice: None,
        look: None,
        style: None,
        base_styles: Vec::new(),
        default_inline_failure_policy: None,
        style_contributions: Vec::new(),
        args: Vec::new(),
        content: RichTextDocument::new(vec![
            RichTextNode::StyleStart {
                style: RichTextStyle::Transform {
                    transform: RichTextTransform {
                        translate: RichTextVec2::new(Milli(5000), Milli(-2000)),
                        target: RichTextEffectTarget::TextBox,
                        ..RichTextTransform::default()
                    },
                },
            },
            RichTextNode::StyleStart {
                style: RichTextStyle::Effect {
                    effect: RichTextEffectDescriptor {
                        id: "wave".to_owned(),
                        params: BTreeMap::from([
                            (
                                "amp".to_owned(),
                                RichTextParam::Milli { value: Milli(3000) },
                            ),
                            (
                                "dir".to_owned(),
                                RichTextParam::Vec2 {
                                    value: RichTextVec2::new(Milli::ONE, Milli::ZERO),
                                },
                            ),
                            (
                                "phase".to_owned(),
                                RichTextParam::Milli { value: Milli(250) },
                            ),
                        ]),
                        target: RichTextEffectTarget::TextBox,
                        phase: RichTextEffectPhase::GlyphTransform,
                        state_scope: RichTextStateScope::Run,
                    },
                },
            },
            RichTextNode::Ruby {
                base: "揺".to_owned(),
                ruby: "ゆれ".to_owned(),
            },
            RichTextNode::StyleEnd {
                name: "effect".to_owned(),
            },
            RichTextNode::StyleEnd {
                name: "transform".to_owned(),
            },
        ]),
    };
    let frame = spec
        .resolve_frame(&RuntimeLineContext::default())
        .expect("frame resolves");
    let page_layout = layout_page_range(
        &frame,
        0.."揺".len(),
        native_text_layout_config(800, 600, 96.0, 572.0),
    )
    .expect("page layout resolves");
    let layout_annotation_x = page_layout.layout.ruby[0].ruby_bounds.x;
    let bounds = native_element_bounds_from_layout_at(&page_layout, 800, 600, 0.0, None);
    let ruby_bounds = bounds
        .iter()
        .find_map(|bounds| match bounds.element {
            NativeFrameElement::Ruby { .. } => bounds.ruby.as_ref(),
            _ => None,
        })
        .expect("ruby element bounds are reported");
    assert!(
        f64::from(ruby_bounds.annotation_bbox.x) > f64::from(layout_annotation_x + 7.0),
        "observed ruby annotation bbox should follow translate plus wave: layout x={layout_annotation_x}, observed={:?}",
        ruby_bounds.annotation_bbox
    );
}

#[test]
fn measure_frame_elements_with_time_follows_glyph_transform_effects() {
    let spec = LineDisplaySpec {
        line: RuntimeLineId("say.test.measure.time.wave".to_owned()),
        callee: "alice".to_owned(),
        speaker_label: None,
        text_key: None,
        window: None,
        voice: None,
        look: None,
        style: None,
        base_styles: Vec::new(),
        default_inline_failure_policy: None,
        style_contributions: Vec::new(),
        args: Vec::new(),
        content: RichTextDocument::new(vec![
            RichTextNode::StyleStart {
                style: RichTextStyle::Effect {
                    effect: RichTextEffectDescriptor {
                        id: "wave".to_owned(),
                        params: BTreeMap::from([
                            (
                                "amp".to_owned(),
                                RichTextParam::Milli {
                                    value: Milli(10000),
                                },
                            ),
                            (
                                "dir".to_owned(),
                                RichTextParam::Vec2 {
                                    value: RichTextVec2::new(Milli::ONE, Milli::ZERO),
                                },
                            ),
                        ]),
                        target: RichTextEffectTarget::Run,
                        phase: RichTextEffectPhase::GlyphTransform,
                        state_scope: RichTextStateScope::Run,
                    },
                },
            },
            RichTextNode::Text {
                text: "A".to_owned(),
            },
            RichTextNode::StyleEnd {
                name: "/".to_owned(),
            },
        ]),
    };
    let frame = spec
        .resolve_frame(&RuntimeLineContext::default())
        .expect("frame resolves");
    let at_zero = measure_frame_elements_at_page_with_time(&frame, 800, 600, 96.0, 572.0, 0, 0.0)
        .expect("zero-time bounds resolve");
    let at_quarter =
        measure_frame_elements_at_page_with_time(&frame, 800, 600, 96.0, 572.0, 0, 0.25)
            .expect("quarter-time bounds resolve");
    let zero_glyph = at_zero
        .iter()
        .find(|bounds| matches!(bounds.element, NativeFrameElement::GlyphCluster { .. }))
        .expect("zero glyph bounds");
    let quarter_glyph = at_quarter
        .iter()
        .find(|bounds| matches!(bounds.element, NativeFrameElement::GlyphCluster { .. }))
        .expect("quarter glyph bounds");

    assert!(
        quarter_glyph.bbox.x > zero_glyph.bbox.x + 8,
        "time-aware native measurement should follow wave placement: {zero_glyph:?} -> {quarter_glyph:?}"
    );
}

#[test]
fn native_bounds_union_overheight_ruby_segments_by_object_index() {
    for writing_mode in [
        RichTextWritingMode::VerticalRl,
        RichTextWritingMode::VerticalLr,
    ] {
        let spec = LineDisplaySpec {
            line: RuntimeLineId(format!(
                "say.test.vertical.ruby.bounds.split.{writing_mode:?}"
            )),
            callee: "alice".to_owned(),
            speaker_label: None,
            text_key: None,
            window: None,
            voice: None,
            look: None,
            style: None,
            base_styles: Vec::new(),
            default_inline_failure_policy: None,
            style_contributions: Vec::new(),
            args: Vec::new(),
            content: RichTextDocument::new(vec![
                RichTextNode::StyleStart {
                    style: RichTextStyle::Layout {
                        layout: RichTextLayout {
                            writing_mode,
                            ..RichTextLayout::default()
                        },
                    },
                },
                RichTextNode::Ruby {
                    base: "夢".to_owned(),
                    ruby: "あいうえおかきくけこ".to_owned(),
                },
                RichTextNode::StyleEnd {
                    name: "/".to_owned(),
                },
            ]),
        };
        let frame = spec
            .resolve_frame(&RuntimeLineContext::default())
            .expect("frame resolves");
        let layout = layout_page_range(
            &frame,
            0.."夢".len(),
            TextLayoutConfig {
                size: LayoutSize::new(160.0, 90.0),
                ruby_font_size: 14.0,
                ..native_text_layout_config(160, 90, 0.0, 0.0)
            },
        )
        .expect("page layout resolves");
        assert!(layout.layout.ruby.len() > 1);

        let bounds = native_element_bounds_from_layout_at(&layout, 220, 120, 0.0, None);
        let ruby_bounds = bounds
            .iter()
            .filter(|bounds| matches!(bounds.element, NativeFrameElement::Ruby { index: 0 }))
            .collect::<Vec<_>>();

        assert_eq!(ruby_bounds.len(), 1);
        assert!(
            ruby_bounds[0].bbox.width > 40,
            "{writing_mode:?} ruby object bounds should union split annotation columns"
        );
    }
}

#[test]
fn native_debug_capture_unions_overheight_ruby_segments_by_object_index() {
    let spec = LineDisplaySpec {
        line: RuntimeLineId("say.test.vertical.ruby.debug.split".to_owned()),
        callee: "alice".to_owned(),
        speaker_label: None,
        text_key: None,
        window: None,
        voice: None,
        look: None,
        style: None,
        base_styles: Vec::new(),
        default_inline_failure_policy: None,
        style_contributions: Vec::new(),
        args: Vec::new(),
        content: RichTextDocument::new(vec![
            RichTextNode::StyleStart {
                style: RichTextStyle::Layout {
                    layout: RichTextLayout {
                        writing_mode: RichTextWritingMode::VerticalLr,
                        ..RichTextLayout::default()
                    },
                },
            },
            RichTextNode::Text {
                text: "天地".to_owned(),
            },
            RichTextNode::Ruby {
                base: "夢".to_owned(),
                ruby: "あいうえおかきくけこ".to_owned(),
            },
            RichTextNode::StyleEnd {
                name: "/".to_owned(),
            },
        ]),
    };
    let frame = spec
        .resolve_frame(&RuntimeLineContext::default())
        .expect("frame resolves");
    let page_layout = layout_page_range(
        &frame,
        0.."天地夢".len(),
        native_text_layout_config(220, 120, 48.0, 0.0),
    )
    .expect("page layout resolves");
    assert!(page_layout.layout.ruby.len() > 1);
    let bounds = native_element_bounds_from_layout_at(&page_layout, 220, 120, 0.0, None);
    let ruby = bounds
        .iter()
        .find(|bounds| matches!(bounds.element, NativeFrameElement::Ruby { index: 0 }))
        .expect("ruby element has native bounds");
    let ruby_geometry = ruby.ruby.expect("ruby geometry is reported");
    assert!(
        ruby_geometry.annotation_bbox.width > 14,
        "over-height ruby bounds should union split annotation tracks"
    );
    let fallback_bbox = NativeFrameContentBBox {
        x: 1,
        y: 1,
        width: 8,
        height: 8,
    };
    let capture = capture_frame_debug_regions_at(
        &frame,
        220,
        120,
        48.0,
        0.0,
        &[NativeFrameDebugRegion {
            element: Some(NativeFrameElement::Ruby { index: 0 }),
            fallback_bbox,
            color: [255, 255, 255, 255],
        }],
    )
    .expect("over-height ruby debug capture resolves");

    let content = capture
        .content_bbox
        .expect("over-height ruby debug capture has visible content");
    assert_ne!(content, fallback_bbox);
    assert!(content.x >= ruby.bbox.x);
    assert!(content.y >= ruby.bbox.y);
    assert!(content.x.saturating_add(content.width) <= ruby.bbox.x + ruby.bbox.width);
    assert!(content.y.saturating_add(content.height) <= ruby.bbox.y + ruby.bbox.height);
    assert!(
        content.width > 14,
        "over-height ruby debug content should include split annotation columns"
    );
    assert!(capture.content_pixels > 0);
}

#[test]
fn native_text_style_metrics_follow_size_style() {
    let style = RichTextStyle::Size {
        points: Some(48),
        raw: "48".to_owned(),
    };
    let native = native_style_from_styles([&style]);
    let metrics = native.metrics();

    assert!((metrics.font_size - 48.0).abs() < f32::EPSILON);
    assert!((metrics.line_height - 64.8).abs() <= 0.0001);
}

#[test]
fn native_ruby_style_uses_tight_line_height() {
    let presentation = RichTextPresentation {
        layout: Some(RichTextLayout {
            ruby_font_size: Some(arcweft_render_text::Milli(11000)),
            ..RichTextLayout::default()
        }),
        ..RichTextPresentation::default()
    };
    let native = native_ruby_style_from_styles(&[], &presentation);
    let metrics = native.ruby_metrics();

    assert!((metrics.font_size - 11.0).abs() < f32::EPSILON);
    assert!((metrics.line_height - 11.0).abs() < f32::EPSILON);
}

#[test]
fn vertical_alternate_glyphs_use_feature_shaped_cache_keys() {
    let spec = LineDisplaySpec {
        line: RuntimeLineId("say.test.vertical.features".to_owned()),
        callee: "alice".to_owned(),
        speaker_label: None,
        text_key: None,
        window: None,
        voice: None,
        look: None,
        style: None,
        base_styles: vec![RichTextStyle::Size {
            points: Some(48),
            raw: "48".to_owned(),
        }],
        default_inline_failure_policy: None,
        style_contributions: Vec::new(),
        args: Vec::new(),
        content: RichTextDocument::new(vec![
            RichTextNode::StyleStart {
                style: RichTextStyle::Layout {
                    layout: RichTextLayout {
                        writing_mode: RichTextWritingMode::VerticalRl,
                        ..RichTextLayout::default()
                    },
                },
            },
            RichTextNode::Text {
                text: "縦Ａ。ー".to_owned(),
            },
            RichTextNode::StyleEnd {
                name: "/".to_owned(),
            },
        ]),
    };
    let frame = spec
        .resolve_frame(&RuntimeLineContext::default())
        .expect("frame resolves");
    let page = WindowPage::from_frame(&frame)
        .into_iter()
        .next()
        .expect("page exists");
    let layout = layout_frame(
        page.layout_frame.as_ref().expect("layout frame"),
        native_text_layout_config(800, 600, 0.0, 0.0),
    )
    .expect("layout resolves");
    let mut font_system = FontSystem::new();
    let mut buffer = Buffer::new(&mut font_system, Metrics::new(30.0, 42.0));
    prepare_window_text_buffers(&mut font_system, &mut buffer, &page.rich_text, 800, 600);

    let cache_keys = layout_glyph_cache_keys(&mut font_system, &buffer, &page.rich_text, &layout);
    let upright_index = layout
        .glyphs
        .iter()
        .position(|glyph| glyph.text == "。")
        .expect("upright alternate glyph exists");
    let rotated_index = layout
        .glyphs
        .iter()
        .position(|glyph| glyph.text == "ー")
        .expect("rotated alternate glyph exists");

    assert!(cache_keys.vertical_alternates.contains_key(&upright_index));
    assert!(cache_keys.vertical_alternates.contains_key(&rotated_index));
    let upright_style =
        native_style_for_display_range(&page.rich_text, layout.glyphs[upright_index].range);
    assert!((upright_style.metrics().font_size - 48.0).abs() < f32::EPSILON);
    let default_upright_keys = vertical_form_cache_keys(
        &mut font_system,
        &layout.glyphs[upright_index],
        &NativeTextStyle::default(),
    );
    let sized_upright_keys = cache_keys
        .vertical_alternates
        .get(&upright_index)
        .expect("sized upright alternate keys exist");
    assert!(
        sized_upright_keys[0].advance.x > default_upright_keys[0].advance.x,
        "vertical alternate shaping should use the rich-text size style"
    );
    assert_eq!(
        vertical_form_font_features(GlyphVerticalForm::UprightAlternate).features[0].tag,
        FeatureTag::new(b"vert")
    );
    assert_eq!(
        vertical_form_font_features(GlyphVerticalForm::RotatedAlternate).features[0].tag,
        FeatureTag::new(b"vrtr")
    );
}

#[test]
fn native_layout_reports_text_run_and_ruby_element_bounds() {
    let spec = LineDisplaySpec {
        line: RuntimeLineId("say.test.bounds".to_owned()),
        callee: "alice".to_owned(),
        speaker_label: None,
        text_key: None,
        window: None,
        voice: None,
        look: None,
        style: None,
        base_styles: Vec::new(),
        default_inline_failure_policy: None,
        style_contributions: Vec::new(),
        args: Vec::new(),
        content: RichTextDocument::new(vec![
            RichTextNode::Text {
                text: "Hello ".to_owned(),
            },
            RichTextNode::Ruby {
                base: "夢".to_owned(),
                ruby: "ゆめ".to_owned(),
            },
        ]),
    };
    let frame = spec
        .resolve_frame(&RuntimeLineContext::default())
        .expect("frame resolves");
    let page_range = display_map_non_empty_page_range_at(&frame, 0).expect("page range");
    let page_layout = layout_page_range(
        &frame,
        page_range,
        native_text_layout_config(800, 600, 96.0, 572.0),
    )
    .expect("page layout resolves");
    assert_eq!(page_layout.layout.ruby.len(), 1);
    let bounds = measure_frame_elements_at(&frame, 800, 600, 96.0, 572.0)
        .expect("native layout bounds resolve");

    assert!(bounds.iter().any(|bounds| {
        matches!(bounds.element, NativeFrameElement::TextRun { index: 0 | 1 })
            && bounds.bbox.x >= 96
            && bounds.bbox.y >= 540
    }));
    let cluster = bounds
        .iter()
        .find(|bounds| {
            matches!(
                bounds.element,
                NativeFrameElement::GlyphCluster {
                    index: 0,
                    range_start: 0,
                    range_end: 1
                }
            )
        })
        .expect("first glyph cluster has native bounds");
    assert!(cluster.bbox.x >= 96);
    assert!(cluster.bbox.y >= 540);
    assert!(cluster.bbox.width > 0);
    assert!(cluster.bbox.height > 0);
    let ruby = bounds
        .iter()
        .find(|bounds| matches!(bounds.element, NativeFrameElement::Ruby { index: 0 }))
        .expect("ruby element has native bounds");
    assert!(ruby.bbox.width < 180);
    assert!(ruby.bbox.height < 120);
}

#[test]
fn native_layout_reports_vertical_typewriter_ruby_element_bounds() {
    let spec = LineDisplaySpec {
        line: RuntimeLineId("say.test.vertical.typewriter.ruby.bounds".to_owned()),
        callee: "alice".to_owned(),
        speaker_label: None,
        text_key: None,
        window: None,
        voice: None,
        look: None,
        style: None,
        base_styles: Vec::new(),
        default_inline_failure_policy: None,
        style_contributions: Vec::new(),
        args: Vec::new(),
        content: RichTextDocument::new(vec![
            RichTextNode::StyleStart {
                style: RichTextStyle::Layout {
                    layout: RichTextLayout {
                        writing_mode: RichTextWritingMode::VerticalRl,
                        ..RichTextLayout::default()
                    },
                },
            },
            RichTextNode::Text {
                text: "天地春夏秋冬".to_owned(),
            },
            RichTextNode::StyleStart {
                style: RichTextStyle::Effect {
                    effect: RichTextEffectDescriptor {
                        id: "typewriter".to_owned(),
                        params: BTreeMap::from([(
                            "cps".to_owned(),
                            RichTextParam::Milli { value: Milli::ONE },
                        )]),
                        target: RichTextEffectTarget::Run,
                        phase: RichTextEffectPhase::GlyphMask,
                        state_scope: arcweft_render_text::RichTextStateScope::Run,
                    },
                },
            },
            RichTextNode::Ruby {
                base: "夢".to_owned(),
                ruby: "ながいながいよみ".to_owned(),
            },
            RichTextNode::Text {
                text: "人外".to_owned(),
            },
            RichTextNode::StyleEnd {
                name: "effect".to_owned(),
            },
            RichTextNode::StyleEnd {
                name: "layout".to_owned(),
            },
        ]),
    };
    let frame = spec
        .resolve_frame(&RuntimeLineContext::default())
        .expect("frame resolves");
    let bounds = measure_frame_elements_at(&frame, 1280, 720, 120.0, 572.0)
        .expect("native layout bounds resolve");

    assert!(
        bounds
            .iter()
            .any(|bounds| { matches!(bounds.element, NativeFrameElement::Ruby { index: 0 }) })
    );
}

#[test]
fn native_layout_reports_short_vertical_rl_ruby_at_viewport_edge() {
    let spec = LineDisplaySpec {
        line: RuntimeLineId("say.test.vertical.short.ruby.edge".to_owned()),
        callee: "alice".to_owned(),
        speaker_label: None,
        text_key: None,
        window: None,
        voice: None,
        look: None,
        style: None,
        base_styles: Vec::new(),
        default_inline_failure_policy: None,
        style_contributions: Vec::new(),
        args: Vec::new(),
        content: RichTextDocument::new(vec![
            RichTextNode::StyleStart {
                style: RichTextStyle::Layout {
                    layout: RichTextLayout {
                        writing_mode: RichTextWritingMode::VerticalRl,
                        ..RichTextLayout::default()
                    },
                },
            },
            RichTextNode::Text {
                text: "天地春夏秋冬".to_owned(),
            },
            RichTextNode::Ruby {
                base: "夢".to_owned(),
                ruby: "ゆめ".to_owned(),
            },
            RichTextNode::StyleEnd {
                name: "layout".to_owned(),
            },
        ]),
    };
    let frame = spec
        .resolve_frame(&RuntimeLineContext::default())
        .expect("frame resolves");
    let bounds = measure_frame_elements_at(&frame, 800, 600, 96.0, 572.0)
        .expect("native layout bounds resolve");

    let ruby = bounds
        .iter()
        .find(|bounds| matches!(bounds.element, NativeFrameElement::Ruby { index: 0 }))
        .expect("short vertical_rl ruby remains observable at the viewport edge");
    let geometry = ruby.ruby.expect("ruby geometry is reported");
    assert!(geometry.annotation_bbox.x >= geometry.base_bbox.x);
    assert!(geometry.annotation_bbox.x < 800);
}

#[test]
fn native_debug_capture_uses_layout_bounds_for_text_elements() {
    let spec = LineDisplaySpec {
        line: RuntimeLineId("say.test.debug".to_owned()),
        callee: "alice".to_owned(),
        speaker_label: None,
        text_key: None,
        window: None,
        voice: None,
        look: None,
        style: None,
        base_styles: Vec::new(),
        default_inline_failure_policy: None,
        style_contributions: Vec::new(),
        args: Vec::new(),
        content: RichTextDocument::new(vec![
            RichTextNode::Text {
                text: "Hello ".to_owned(),
            },
            RichTextNode::Ruby {
                base: "夢".to_owned(),
                ruby: "ゆめ".to_owned(),
            },
        ]),
    };
    let frame = spec
        .resolve_frame(&RuntimeLineContext::default())
        .expect("frame resolves");
    let fallback_bbox = NativeFrameContentBBox {
        x: 1,
        y: 1,
        width: 8,
        height: 8,
    };
    let capture = capture_frame_debug_regions_at(
        &frame,
        800,
        600,
        96.0,
        572.0,
        &[NativeFrameDebugRegion {
            element: Some(NativeFrameElement::Ruby { index: 0 }),
            fallback_bbox,
            color: [255, 255, 255, 255],
        }],
    )
    .expect("debug capture resolves");

    let bbox = capture
        .content_bbox
        .expect("debug capture has visible content");
    assert_ne!(bbox, fallback_bbox);
    assert!(bbox.x >= 96);
    assert!(bbox.y >= 520);
    let bbox_area = u64::from(bbox.width) * u64::from(bbox.height);
    assert!(capture.content_pixels > 0);
    assert!(capture.content_pixels < bbox_area);
}

#[test]
fn native_debug_capture_uses_glyph_area_for_vertical_clusters() {
    let spec = LineDisplaySpec {
        line: RuntimeLineId("say.test.vertical.cluster.debug".to_owned()),
        callee: "alice".to_owned(),
        speaker_label: None,
        text_key: None,
        window: None,
        voice: None,
        look: None,
        style: None,
        base_styles: Vec::new(),
        default_inline_failure_policy: None,
        style_contributions: Vec::new(),
        args: Vec::new(),
        content: RichTextDocument::new(vec![
            RichTextNode::StyleStart {
                style: RichTextStyle::Layout {
                    layout: RichTextLayout {
                        writing_mode: RichTextWritingMode::VerticalRl,
                        ..RichTextLayout::default()
                    },
                },
            },
            RichTextNode::Text {
                text: "吾輩".to_owned(),
            },
            RichTextNode::StyleEnd {
                name: "layout".to_owned(),
            },
        ]),
    };
    let frame = spec
        .resolve_frame(&RuntimeLineContext::default())
        .expect("frame resolves");
    let bounds = measure_frame_elements_at(&frame, 800, 600, 96.0, 572.0)
        .expect("native layout bounds resolve");
    let cluster = bounds
        .iter()
        .find(|bounds| {
            matches!(
                bounds.element,
                NativeFrameElement::GlyphCluster {
                    index: 0,
                    range_start: 0,
                    range_end: 3
                }
            )
        })
        .expect("first vertical glyph cluster has native bounds");
    let capture = capture_frame_debug_regions_at(
        &frame,
        800,
        600,
        96.0,
        572.0,
        &[NativeFrameDebugRegion {
            element: Some(NativeFrameElement::GlyphCluster {
                index: 0,
                range_start: 0,
                range_end: 3,
            }),
            fallback_bbox: NativeFrameContentBBox {
                x: 1,
                y: 1,
                width: 8,
                height: 8,
            },
            color: [255, 255, 255, 255],
        }],
    )
    .expect("debug capture resolves");

    let bbox = capture
        .content_bbox
        .expect("vertical glyph cluster debug capture has visible content");
    assert!(bbox.x >= cluster.bbox.x);
    assert!(bbox.y >= cluster.bbox.y);
    assert!(bbox.x.saturating_add(bbox.width) <= cluster.bbox.x + cluster.bbox.width);
    assert!(bbox.y.saturating_add(bbox.height) <= cluster.bbox.y + cluster.bbox.height);
    assert!(capture.content_pixels > 0);
}

#[test]
fn native_color_region_capture_preserves_selected_text_style() {
    let spec = LineDisplaySpec {
        line: RuntimeLineId("say.test.color.region".to_owned()),
        callee: "alice".to_owned(),
        speaker_label: None,
        text_key: None,
        window: None,
        voice: None,
        look: None,
        style: None,
        base_styles: vec![RichTextStyle::from_tag("color", "#ff0000")],
        default_inline_failure_policy: None,
        style_contributions: Vec::new(),
        args: Vec::new(),
        content: RichTextDocument::new(vec![
            RichTextNode::Text {
                text: "Red ".to_owned(),
            },
            RichTextNode::Text {
                text: "Hidden".to_owned(),
            },
        ]),
    };
    let frame = spec
        .resolve_frame(&RuntimeLineContext::default())
        .expect("frame resolves");
    let fallback_bbox = NativeFrameContentBBox {
        x: 1,
        y: 1,
        width: 8,
        height: 8,
    };
    let capture = capture_frame_color_regions_at(
        &frame,
        800,
        600,
        96.0,
        572.0,
        &[NativeFrameDebugRegion {
            element: Some(NativeFrameElement::TextRun { index: 0 }),
            fallback_bbox,
            color: [0, 0, 0, 0],
        }],
    )
    .expect("color region capture resolves");

    let bbox = capture
        .content_bbox
        .expect("color region capture has visible content");
    assert_ne!(bbox, fallback_bbox);
    assert!(bbox.x >= 96);
    assert!(bbox.y >= 540);
    assert!(capture.content_pixels > 0);
    assert!(capture.rgba.chunks_exact(4).any(|pixel| {
        pixel[0] > pixel[1].saturating_add(40)
            && pixel[0] > pixel[2].saturating_add(40)
            && pixel[3] > 0
    }));
}

#[test]
fn native_offscreen_capture_session_reuses_renderer_for_multiple_capture_modes() {
    let spec = LineDisplaySpec {
        line: RuntimeLineId("say.test.session".to_owned()),
        callee: "alice".to_owned(),
        speaker_label: None,
        text_key: None,
        window: None,
        voice: None,
        look: None,
        style: None,
        base_styles: vec![RichTextStyle::from_tag("color", "#ff0000")],
        default_inline_failure_policy: None,
        style_contributions: Vec::new(),
        args: Vec::new(),
        content: RichTextDocument::new(vec![
            RichTextNode::Text {
                text: "Red ".to_owned(),
            },
            RichTextNode::Ruby {
                base: "夢".to_owned(),
                ruby: "ゆめ".to_owned(),
            },
        ]),
    };
    let frame = spec
        .resolve_frame(&RuntimeLineContext::default())
        .expect("frame resolves");
    let fallback_bbox = NativeFrameContentBBox {
        x: 1,
        y: 1,
        width: 8,
        height: 8,
    };
    let regions = [NativeFrameDebugRegion {
        element: Some(NativeFrameElement::TextRun { index: 0 }),
        fallback_bbox,
        color: [255, 255, 255, 255],
    }];
    let mut session = NativeOffscreenCaptureSession::new().expect("capture session");

    let full = session
        .capture_frame_rgba_at(&frame, 800, 600, 96.0, 572.0)
        .expect("full capture resolves");
    let debug = session
        .capture_frame_debug_regions_at(&frame, 800, 600, 96.0, 572.0, &regions)
        .expect("debug capture resolves");
    let color = session
        .capture_frame_color_regions_at(&frame, 800, 600, 96.0, 572.0, &regions)
        .expect("color capture resolves");

    assert_eq!((full.width, full.height), (800, 600));
    assert!(full.content_pixels > 0);
    assert!(debug.content_pixels > 0);
    assert!(color.content_pixels > 0);
    assert_ne!(debug.content_bbox, Some(fallback_bbox));
    assert!(color.rgba.chunks_exact(4).any(|pixel| {
        pixel[0] > pixel[1].saturating_add(40)
            && pixel[0] > pixel[2].saturating_add(40)
            && pixel[3] > 0
    }));
}

#[test]
fn native_typewriter_capture_changes_visibility_without_relayout() {
    let spec = LineDisplaySpec {
        line: RuntimeLineId("say.test.typewriter.vertical".to_owned()),
        callee: "alice".to_owned(),
        speaker_label: None,
        text_key: None,
        window: None,
        voice: None,
        look: None,
        style: None,
        base_styles: Vec::new(),
        default_inline_failure_policy: None,
        style_contributions: Vec::new(),
        args: Vec::new(),
        content: RichTextDocument::new(vec![
            RichTextNode::StyleStart {
                style: RichTextStyle::Layout {
                    layout: RichTextLayout {
                        writing_mode: RichTextWritingMode::VerticalRl,
                        ..RichTextLayout::default()
                    },
                },
            },
            RichTextNode::StyleStart {
                style: RichTextStyle::Effect {
                    effect: RichTextEffectDescriptor {
                        id: "typewriter".to_owned(),
                        params: BTreeMap::from([(
                            "cps".to_owned(),
                            RichTextParam::Milli { value: Milli::ONE },
                        )]),
                        target: RichTextEffectTarget::Run,
                        phase: RichTextEffectPhase::GlyphMask,
                        state_scope: arcweft_render_text::RichTextStateScope::Run,
                    },
                },
            },
            RichTextNode::Text {
                text: "吾輩".to_owned(),
            },
            RichTextNode::StyleEnd {
                name: "effect".to_owned(),
            },
            RichTextNode::StyleEnd {
                name: "layout".to_owned(),
            },
        ]),
    };
    let frame = spec
        .resolve_frame(&RuntimeLineContext::default())
        .expect("frame resolves");
    let at_zero = visual_plan_from_frame_for_test(&frame, 0.0);
    let at_later = visual_plan_from_frame_for_test(&frame, 4.0);
    assert_eq!(
        at_zero.pages[0].glyphs.len(),
        at_later.pages[0].glyphs.len()
    );
    for (hidden, visible) in at_zero.pages[0]
        .glyphs
        .iter()
        .zip(&at_later.pages[0].glyphs)
    {
        assert_eq!(hidden.range, visible.range);
        assert!((hidden.x - visible.x).abs() < f32::EPSILON);
        assert!((hidden.y - visible.y).abs() < f32::EPSILON);
    }
    assert!(
        at_zero.pages[0]
            .glyphs
            .iter()
            .all(|glyph| glyph.opacity == 0.0)
    );
    assert!(
        at_later.pages[0]
            .glyphs
            .iter()
            .all(|glyph| glyph.opacity > 0.0)
    );

    let mut session = NativeOffscreenCaptureSession::new().expect("capture session");
    let hidden = session
        .capture_frame_rgba_in(
            &frame,
            NativeCaptureViewport::new(800, 600, 96.0, 572.0, 0).with_time_seconds(0.0),
        )
        .expect("hidden typewriter capture resolves");
    let visible = session
        .capture_frame_rgba_in(
            &frame,
            NativeCaptureViewport::new(800, 600, 96.0, 572.0, 0).with_time_seconds(4.0),
        )
        .expect("visible typewriter capture resolves");

    assert_eq!(hidden.content_pixels, 0);
    assert!(visible.content_pixels > 0);
}

#[test]
fn native_typewriter_delay_offsets_visibility_time() {
    let spec = LineDisplaySpec {
        line: RuntimeLineId("say.test.typewriter.delay".to_owned()),
        callee: "alice".to_owned(),
        speaker_label: None,
        text_key: None,
        window: None,
        voice: None,
        look: None,
        style: None,
        base_styles: Vec::new(),
        default_inline_failure_policy: None,
        style_contributions: Vec::new(),
        args: Vec::new(),
        content: RichTextDocument::new(vec![
            RichTextNode::StyleStart {
                style: RichTextStyle::Effect {
                    effect: RichTextEffectDescriptor {
                        id: "typewriter".to_owned(),
                        params: BTreeMap::from([
                            ("cps".to_owned(), RichTextParam::Milli { value: Milli::ONE }),
                            (
                                "delay".to_owned(),
                                RichTextParam::Raw {
                                    value: "500ms".to_owned(),
                                },
                            ),
                        ]),
                        target: RichTextEffectTarget::Run,
                        phase: RichTextEffectPhase::GlyphMask,
                        state_scope: arcweft_render_text::RichTextStateScope::Run,
                    },
                },
            },
            RichTextNode::Text {
                text: "AB".to_owned(),
            },
            RichTextNode::StyleEnd {
                name: "effect".to_owned(),
            },
        ]),
    };
    let frame = spec
        .resolve_frame(&RuntimeLineContext::default())
        .expect("frame resolves");
    let delayed = visual_plan_from_frame_for_test(&frame, 0.25);
    let visible = visual_plan_from_frame_for_test(&frame, 2.75);
    assert_eq!(delayed.pages[0].glyphs.len(), visible.pages[0].glyphs.len());
    assert!(
        delayed.pages[0]
            .glyphs
            .iter()
            .all(|glyph| glyph.opacity == 0.0),
        "delay should hide typewriter glyphs before its start time"
    );
    assert!(
        visible.pages[0]
            .glyphs
            .iter()
            .all(|glyph| glyph.opacity > 0.0),
        "typewriter glyphs should become visible after delay plus cps time"
    );

    let mut session = NativeOffscreenCaptureSession::new().expect("capture session");
    let hidden_capture = session
        .capture_frame_rgba_in(
            &frame,
            NativeCaptureViewport::new(800, 600, 96.0, 572.0, 0).with_time_seconds(0.25),
        )
        .expect("delayed typewriter capture resolves");
    let visible_capture = session
        .capture_frame_rgba_in(
            &frame,
            NativeCaptureViewport::new(800, 600, 96.0, 572.0, 0).with_time_seconds(2.75),
        )
        .expect("visible typewriter capture resolves");

    assert_eq!(hidden_capture.content_pixels, 0);
    assert!(visible_capture.content_pixels > 0);
}

#[test]
fn native_typewriter_cursor_previews_next_glyph_without_relayout() {
    let spec = LineDisplaySpec {
        line: RuntimeLineId("say.test.typewriter.cursor".to_owned()),
        callee: "alice".to_owned(),
        speaker_label: None,
        text_key: None,
        window: None,
        voice: None,
        look: None,
        style: None,
        base_styles: Vec::new(),
        default_inline_failure_policy: None,
        style_contributions: Vec::new(),
        args: Vec::new(),
        content: RichTextDocument::new(vec![
            RichTextNode::StyleStart {
                style: RichTextStyle::Effect {
                    effect: RichTextEffectDescriptor {
                        id: "typewriter".to_owned(),
                        params: BTreeMap::from([
                            ("cps".to_owned(), RichTextParam::Milli { value: Milli::ONE }),
                            ("cursor".to_owned(), RichTextParam::Bool { value: true }),
                            (
                                "cursor_alpha".to_owned(),
                                RichTextParam::Milli { value: Milli(500) },
                            ),
                        ]),
                        target: RichTextEffectTarget::Run,
                        phase: RichTextEffectPhase::GlyphMask,
                        state_scope: arcweft_render_text::RichTextStateScope::Run,
                    },
                },
            },
            RichTextNode::Text {
                text: "AB".to_owned(),
            },
            RichTextNode::StyleEnd {
                name: "effect".to_owned(),
            },
        ]),
    };
    let frame = spec
        .resolve_frame(&RuntimeLineContext::default())
        .expect("frame resolves");
    let cursor_plan = visual_plan_from_frame_for_test(&frame, 0.0);
    let later_plan = visual_plan_from_frame_for_test(&frame, 2.0);
    assert_eq!(
        cursor_plan.pages[0].glyphs.len(),
        later_plan.pages[0].glyphs.len()
    );
    assert!(
        cursor_plan.pages[0].glyphs[0].opacity > 0.45
            && cursor_plan.pages[0].glyphs[0].opacity < 0.55,
        "cursor should preview the next glyph with configured opacity: {:?}",
        cursor_plan.pages[0].glyphs[0]
    );
    assert!(cursor_plan.pages[0].glyphs[1].opacity.abs() < f32::EPSILON);
    for (cursor, later) in cursor_plan.pages[0]
        .glyphs
        .iter()
        .zip(&later_plan.pages[0].glyphs)
    {
        assert_eq!(cursor.range, later.range);
        assert!((cursor.x - later.x).abs() < f32::EPSILON);
        assert!((cursor.y - later.y).abs() < f32::EPSILON);
    }

    let mut session = NativeOffscreenCaptureSession::new().expect("capture session");
    let cursor_capture = session
        .capture_frame_rgba_in(
            &frame,
            NativeCaptureViewport::new(800, 600, 96.0, 572.0, 0).with_time_seconds(0.0),
        )
        .expect("cursor typewriter capture resolves");
    assert!(
        cursor_capture.content_pixels > 0,
        "typewriter cursor preview should be visible in framebuffer capture"
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn native_debug_ruby_capture_applies_typewriter_visibility() {
    let spec = LineDisplaySpec {
        line: RuntimeLineId("say.test.typewriter.vertical.ruby.debug".to_owned()),
        callee: "alice".to_owned(),
        speaker_label: None,
        text_key: None,
        window: None,
        voice: None,
        look: None,
        style: None,
        base_styles: Vec::new(),
        default_inline_failure_policy: None,
        style_contributions: Vec::new(),
        args: Vec::new(),
        content: RichTextDocument::new(vec![
            RichTextNode::StyleStart {
                style: RichTextStyle::Layout {
                    layout: RichTextLayout {
                        writing_mode: RichTextWritingMode::VerticalRl,
                        ..RichTextLayout::default()
                    },
                },
            },
            RichTextNode::Text {
                text: "天地春夏秋冬".to_owned(),
            },
            RichTextNode::StyleStart {
                style: RichTextStyle::Effect {
                    effect: RichTextEffectDescriptor {
                        id: "typewriter".to_owned(),
                        params: BTreeMap::from([(
                            "cps".to_owned(),
                            RichTextParam::Milli { value: Milli::ONE },
                        )]),
                        target: RichTextEffectTarget::Run,
                        phase: RichTextEffectPhase::GlyphMask,
                        state_scope: arcweft_render_text::RichTextStateScope::Run,
                    },
                },
            },
            RichTextNode::Ruby {
                base: "夢".to_owned(),
                ruby: "ながいながいよみ".to_owned(),
            },
            RichTextNode::Text {
                text: "人外".to_owned(),
            },
            RichTextNode::StyleEnd {
                name: "effect".to_owned(),
            },
            RichTextNode::StyleEnd {
                name: "layout".to_owned(),
            },
        ]),
    };
    let frame = spec
        .resolve_frame(&RuntimeLineContext::default())
        .expect("frame resolves");
    let bounds =
        measure_frame_elements_at(&frame, 1280, 720, 120.0, 572.0).expect("bounds resolve");
    let ruby = bounds
        .iter()
        .find(|bounds| matches!(bounds.element, NativeFrameElement::Ruby { index: 0 }))
        .expect("ruby element is observed");
    let region = NativeFrameDebugRegion {
        element: Some(NativeFrameElement::Ruby { index: 0 }),
        fallback_bbox: ruby.bbox,
        color: [255, 255, 255, 255],
    };
    let page_range = display_map_non_empty_page_range_at(&frame, 0).expect("page range");
    let page = page_from_display_map_range(&frame, page_range.clone()).expect("page");
    let debug_rich_text =
        debug_rich_text_for_regions(&frame, &page_range, &page.rich_text, &[region])
            .expect("debug rich text");
    assert!(
        debug_rich_text
            .spans
            .iter()
            .all(|span| span.style.color.alpha == 0)
    );
    assert_eq!(debug_rich_text.ruby_annotations.len(), 1);
    assert_eq!(
        presentation_alpha_for_visibility_time(
            &debug_rich_text.ruby_annotations[0].presentation,
            0.0
        ),
        0
    );
    let mut session = NativeOffscreenCaptureSession::new().expect("capture session");
    let hidden = session
        .capture_frame_debug_regions_in(
            &frame,
            NativeCaptureViewport::new(1280, 720, 120.0, 572.0, 0).with_time_seconds(0.0),
            &[region],
        )
        .expect("hidden ruby debug capture resolves");
    let visible = session
        .capture_frame_debug_regions_in(
            &frame,
            NativeCaptureViewport::new(1280, 720, 120.0, 572.0, 0).with_time_seconds(4.0),
            &[region],
        )
        .expect("visible ruby debug capture resolves");

    assert_eq!(hidden.content_pixels, 0);
    assert!(visible.content_pixels > 0);
}

#[test]
fn window_pages_split_on_display_map_page_line_wait_and_clear_controls() {
    let spec = LineDisplaySpec {
        line: RuntimeLineId("say.test.002".to_owned()),
        callee: "alice".to_owned(),
        speaker_label: None,
        text_key: None,
        window: None,
        voice: None,
        look: None,
        style: None,
        base_styles: vec![RichTextStyle::from_tag("font", "serif")],
        default_inline_failure_policy: None,
        style_contributions: Vec::new(),
        args: Vec::new(),
        content: RichTextDocument::new(vec![
            RichTextNode::Text {
                text: "one".to_owned(),
            },
            RichTextNode::Control {
                control: RichTextControl::Page,
            },
            RichTextNode::Text {
                text: "two".to_owned(),
            },
            RichTextNode::Control {
                control: RichTextControl::LineWait,
            },
            RichTextNode::Text {
                text: "three".to_owned(),
            },
            RichTextNode::Control {
                control: RichTextControl::Clear,
            },
            RichTextNode::Text {
                text: "four".to_owned(),
            },
        ]),
    };
    let mut frame = spec
        .resolve_frame(&RuntimeLineContext::default())
        .expect("frame resolves");
    frame.nodes.clear();

    let pages = WindowPage::from_frame(&frame);

    assert_eq!(pages.len(), 4);
    assert_eq!(pages[0].rich_text.text, "one");
    assert_eq!(pages[1].rich_text.text, "two");
    assert_eq!(pages[2].rich_text.text, "three");
    assert_eq!(pages[3].rich_text.text, "four");
    assert!(pages.iter().all(|page| {
        page.rich_text
            .spans
            .iter()
            .all(|span| span.style.family == NativeFontFamily::Serif)
    }));
}

#[test]
fn display_map_page_ranges_do_not_split_ruby_base_ranges() {
    let frame = LineDisplayFrame {
        line: RuntimeLineId("say.test.page.ruby.atomic".to_owned()),
        callee: "alice".to_owned(),
        speaker_label: None,
        text: "ABCDE".to_owned(),
        base_styles: Vec::new(),
        default_inline_failure_policy: None,
        style_contributions: Vec::new(),
        nodes: Vec::new(),
        display_map: arcweft_render_text::RichTextDisplayMap {
            text_runs: vec![
                arcweft_render_text::RichTextTextRun {
                    range: RichTextRange::new(0, 2),
                    source: arcweft_render_text::RichTextTextSource::Text,
                    node_index: 0,
                    styles: Vec::new(),
                    presentation: RichTextPresentation::default(),
                },
                arcweft_render_text::RichTextTextRun {
                    range: RichTextRange::new(2, 5),
                    source: arcweft_render_text::RichTextTextSource::Text,
                    node_index: 2,
                    styles: Vec::new(),
                    presentation: RichTextPresentation::default(),
                },
            ],
            ruby_annotations: vec![arcweft_render_text::RichTextRubyAnnotation {
                base_range: RichTextRange::new(1, 4),
                ruby: "ruby".to_owned(),
                node_index: 1,
                styles: Vec::new(),
                presentation: RichTextPresentation::default(),
            }],
            controls: vec![arcweft_render_text::RichTextControlMarker {
                node_index: 2,
                control: RichTextControl::Page,
                range: None,
            }],
            host_events: Vec::new(),
        },
        host_events: Vec::new(),
        inline_failures: Vec::new(),
        unresolved: Vec::new(),
    };

    assert_eq!(display_map_page_ranges(&frame), vec![0..4, 4..5]);
    let pages = WindowPage::from_frame(&frame);
    assert_eq!(pages.len(), 2);
    assert_eq!(pages[0].rich_text.text, "ABCD");
    assert_eq!(pages[0].rich_text.ruby_annotations.len(), 1);
    assert_eq!(pages[0].rich_text.ruby_annotations[0].base_range, 1..4);
    assert!(pages[1].rich_text.ruby_annotations.is_empty());
}

#[test]
fn native_capture_content_stats_measure_non_background_bounds() {
    let mut rgba = (0..12).flat_map(|_| [0, 0, 0, 255]).collect::<Vec<_>>();
    let width = 4;
    for (x, y) in [(1_u32, 1_u32), (2, 1), (2, 2)] {
        let index = usize::try_from(y)
            .unwrap()
            .saturating_mul(usize::try_from(width).unwrap())
            .saturating_add(usize::try_from(x).unwrap())
            .saturating_mul(4);
        rgba[index..index + 4].copy_from_slice(&[245, 245, 245, 255]);
    }

    let stats = native_frame_content_stats(&rgba, width, 3, [0, 0, 0, 255]);

    assert_eq!(
        stats.content_bbox,
        Some(NativeFrameContentBBox {
            x: 1,
            y: 1,
            width: 2,
            height: 2,
        })
    );
    assert_eq!(stats.content_pixels, 3);
}

#[test]
fn native_capture_row_padding_uses_wgpu_alignment() {
    assert_eq!(padded_rgba_row_bytes(1), COPY_BYTES_PER_ROW_ALIGNMENT);
    assert_eq!(padded_rgba_row_bytes(64), COPY_BYTES_PER_ROW_ALIGNMENT);
    assert_eq!(padded_rgba_row_bytes(65), COPY_BYTES_PER_ROW_ALIGNMENT * 2);
}

#[test]
fn native_visual_plan_reads_raw_effect_params_at_builtin_boundary() {
    let spec = LineDisplaySpec {
        line: RuntimeLineId("say.test.raw.effect".to_owned()),
        callee: "alice".to_owned(),
        speaker_label: None,
        text_key: None,
        window: None,
        voice: None,
        look: None,
        style: None,
        base_styles: Vec::new(),
        default_inline_failure_policy: None,
        style_contributions: Vec::new(),
        args: Vec::new(),
        content: RichTextDocument::new(vec![
            RichTextNode::StyleStart {
                style: RichTextStyle::Effect {
                    effect: RichTextEffectDescriptor {
                        id: "wave".to_owned(),
                        params: BTreeMap::from([
                            (
                                "amp".to_owned(),
                                RichTextParam::Raw {
                                    value: "2px".to_owned(),
                                },
                            ),
                            (
                                "dir".to_owned(),
                                RichTextParam::Raw {
                                    value: "0,1".to_owned(),
                                },
                            ),
                        ]),
                        target: RichTextEffectTarget::Run,
                        phase: RichTextEffectPhase::GlyphTransform,
                        state_scope: arcweft_render_text::RichTextStateScope::Run,
                    },
                },
            },
            RichTextNode::Text {
                text: "A".to_owned(),
            },
            RichTextNode::StyleEnd {
                name: "/".to_owned(),
            },
        ]),
    };
    let frame = spec
        .resolve_frame(&RuntimeLineContext::default())
        .expect("frame resolves");
    let plan = visual_plan_from_frame_for_test(&frame, 0.25);
    let glyph = plan.pages[0].glyphs.first().expect("glyph placement");

    assert!(glyph.x.abs() < f32::EPSILON);
    assert!(glyph.y.abs() > 0.1);
    assert_eq!(plan.pages[0].runs[0].presentation.effects[0].id, "wave");
}

#[test]
fn custom_effect_registry_updates_native_visual_plan_glyphs() {
    let frame = custom_effect_test_frame(".nudge");
    let mut registry = RichTextEffectRegistry::default();
    registry.insert_lambda(".nudge", |ctx| {
        ctx.placement.x += 11.0;
        ctx.state.set(
            RichTextStateScopeKey::Glyph {
                line: ctx.line_id.to_owned(),
                run_index: ctx.run_index,
                glyph_index: ctx.glyph_index,
            },
            "nudge.applied",
            SharedTextValue::Bool(true),
        );
    });
    let mut state = RichTextStateStore::default();

    let plan = visual_plan_from_frame_with_effect_registry(&frame, 0.0, &mut registry, &mut state);
    let glyph = plan.pages[0].glyphs.first().expect("glyph placement");

    assert!(glyph.x > 10.5 && glyph.x < 11.5);
    assert!(plan.diagnostics.is_empty());
    assert_eq!(
        state.get(
            &RichTextStateScopeKey::Glyph {
                line: "say.test.custom.effect".to_owned(),
                run_index: 0,
                glyph_index: 0,
            },
            "nudge.applied",
        ),
        Some(&SharedTextValue::Bool(true))
    );
}

#[test]
fn native_visual_plan_reports_missing_custom_effect_registry() {
    let frame = custom_effect_test_frame(".sparkle");

    let plan = visual_plan_from_frame_for_test(&frame, 0.0);

    assert_eq!(plan.diagnostics.len(), 1);
    assert_eq!(
        plan.diagnostics[0].severity,
        NativeVisualDiagnosticSeverity::Warning
    );
    assert_eq!(plan.diagnostics[0].code, "missing_custom_effect_registry");
    assert_eq!(plan.diagnostics[0].effect_id.as_deref(), Some(".sparkle"));
    assert!(
        plan.pages[0].glyphs[0].x.abs() < f32::EPSILON,
        "missing custom effects should no-op instead of being reinterpreted"
    );
}

#[test]
fn native_capture_applies_builtin_post_process_effect_phase() {
    let frame = custom_effect_test_frame_with_params_and_phase(
        "wave",
        BTreeMap::from([
            (
                "amp".to_owned(),
                RichTextParam::Raw {
                    value: "18px".to_owned(),
                },
            ),
            (
                "period".to_owned(),
                RichTextParam::Raw {
                    value: "48px".to_owned(),
                },
            ),
            (
                "dir".to_owned(),
                RichTextParam::Raw {
                    value: "1,0".to_owned(),
                },
            ),
        ]),
        RichTextEffectPhase::PostProcess,
    );
    let baseline = plain_effect_test_frame();

    let plan = visual_plan_from_frame_for_test(&frame, 0.0);

    assert!(
        plan.diagnostics.is_empty(),
        "post_process builtin effects should execute instead of warning: {:?}",
        plan.diagnostics
    );
    assert!(
        plan.pages[0].glyphs[0].x.abs() < f32::EPSILON,
        "post_process effects should not be reinterpreted as glyph placement transforms"
    );

    let baseline_capture =
        capture_frame_rgba_at(&baseline, 800, 600, 96.0, 572.0).expect("baseline capture");
    let capture = capture_frame_rgba_at(&frame, 800, 600, 96.0, 572.0).expect("wave capture");

    assert!(capture.diagnostics.is_empty());
    assert_ne!(
        capture.rgba, baseline_capture.rgba,
        "post_process wave should alter framebuffer pixels"
    );
}

#[test]
fn native_default_effect_registry_applies_sparkle_params() {
    let frame = custom_effect_test_frame_with_params(
        "sparkle",
        BTreeMap::from([
            (
                "amp".to_owned(),
                RichTextParam::Raw {
                    value: "2px".to_owned(),
                },
            ),
            (
                "seed".to_owned(),
                RichTextParam::Raw {
                    value: "custom".to_owned(),
                },
            ),
        ]),
    );
    let mut registry = native_default_effect_registry();
    let mut state = RichTextStateStore::default();

    let plan = visual_plan_from_frame_with_effect_registry(&frame, 0.25, &mut registry, &mut state);
    let glyph = plan.pages[0].glyphs.first().expect("glyph placement");

    assert!(plan.diagnostics.is_empty());
    assert!(
        glyph.y < -0.05 || (glyph.scale_x - 1.0).abs() > 0.005 || glyph.opacity < 0.99,
        "sparkle should visibly alter glyph placement or mask state: {glyph:?}"
    );
}

#[test]
fn native_capture_uses_default_sparkle_registry_without_diagnostics() {
    let frame = custom_effect_test_frame("sparkle");
    let mut session = NativeOffscreenCaptureSession::new().expect("capture session");

    let capture = session
        .capture_frame_rgba_at(&frame, 800, 600, 96.0, 572.0)
        .expect("sparkle capture");

    assert!(capture.diagnostics.is_empty());
    assert!(capture.content_pixels > 0);
}

#[test]
fn native_capture_uses_custom_effect_registry_for_submitted_glyphs() {
    let frame = custom_effect_test_frame(".nudge");
    let mut baseline = NativeOffscreenCaptureSession::new().expect("baseline session");
    let baseline_capture = baseline
        .capture_frame_rgba_at(&frame, 800, 600, 96.0, 572.0)
        .expect("baseline capture");

    let mut shifted = NativeOffscreenCaptureSession::new().expect("shifted session");
    shifted
        .effect_registry_mut()
        .insert_lambda(".nudge", |ctx| {
            ctx.placement.x += 48.0;
        });
    let shifted_capture = shifted
        .capture_frame_rgba_at(&frame, 800, 600, 96.0, 572.0)
        .expect("shifted capture");

    let baseline_bbox = baseline_capture.content_bbox.expect("baseline content");
    let shifted_bbox = shifted_capture.content_bbox.expect("shifted content");
    assert!(
        shifted_bbox.x > baseline_bbox.x + 32,
        "custom effect registry should move submitted glyph pixels: {baseline_bbox:?} -> {shifted_bbox:?}"
    );
}

#[test]
fn native_capture_uses_custom_effect_registry_for_glyph_color() {
    let frame = custom_effect_test_frame_with_phase(".scarlet", RichTextEffectPhase::GlyphColor);
    let mut session = NativeOffscreenCaptureSession::new().expect("capture session");
    session
        .effect_registry_mut()
        .insert_lambda(".scarlet", |ctx| {
            ctx.placement.color = Some([255, 32, 16, 255]);
        });

    let capture = session
        .capture_frame_rgba_at(&frame, 800, 600, 96.0, 572.0)
        .expect("glyph color capture");

    assert!(capture.diagnostics.is_empty());
    assert!(capture.content_pixels > 0);
    assert!(
        capture.rgba.chunks_exact(4).any(|pixel| {
            pixel[0] > pixel[1].saturating_add(80)
                && pixel[0] > pixel[2].saturating_add(80)
                && pixel[3] > 0
        }),
        "registered glyph_color custom effect should tint submitted glyph pixels"
    );
}

#[test]
fn native_capture_uses_custom_effect_registry_for_post_process() {
    let frame = custom_effect_test_frame_with_phase(".rose_wash", RichTextEffectPhase::PostProcess);
    let mut session = NativeOffscreenCaptureSession::new().expect("capture session");
    session
        .effect_registry_mut()
        .insert_post_process_lambda(".rose_wash", |_ctx, rgba| {
            for pixel in rgba.chunks_exact_mut(4) {
                if pixel[3] == 0 || (pixel[0] == 0 && pixel[1] == 0 && pixel[2] == 0) {
                    continue;
                }
                pixel[0] = 255;
                pixel[1] = 32;
                pixel[2] = 96;
            }
        });

    let capture = session
        .capture_frame_rgba_at(&frame, 800, 600, 96.0, 572.0)
        .expect("post-process custom effect capture");

    assert!(capture.diagnostics.is_empty());
    assert!(
        capture.rgba.chunks_exact(4).any(|pixel| {
            pixel[0] > pixel[1].saturating_add(120)
                && pixel[0] > pixel[2].saturating_add(80)
                && pixel[3] > 0
        }),
        "registered post-process custom effect should alter rendered glyph pixels"
    );
}

#[test]
fn measure_frame_elements_uses_custom_effect_registry_for_glyph_bounds() {
    let frame = custom_effect_test_frame(".nudge");
    let baseline = measure_frame_elements_at_page_with_time(&frame, 800, 600, 96.0, 572.0, 0, 0.0)
        .expect("baseline bounds");

    let mut session = NativeOffscreenCaptureSession::new().expect("capture session");
    session
        .effect_registry_mut()
        .insert_lambda(".nudge", |ctx| {
            ctx.placement.x += 40.0;
        });
    let shifted = session
        .measure_frame_elements_at(&frame, 800, 600, 96.0, 572.0)
        .expect("registry bounds");

    let baseline_glyph = baseline
        .iter()
        .find(|bounds| matches!(bounds.element, NativeFrameElement::GlyphCluster { .. }))
        .expect("baseline glyph");
    let shifted_glyph = shifted
        .iter()
        .find(|bounds| matches!(bounds.element, NativeFrameElement::GlyphCluster { .. }))
        .expect("shifted glyph");
    assert!(
        shifted_glyph.bbox.x > baseline_glyph.bbox.x + 28,
        "custom effect registry should move observed glyph bounds: {:?} -> {:?}",
        baseline_glyph.bbox,
        shifted_glyph.bbox
    );
}

#[test]
fn measure_frame_elements_uses_custom_effect_registry_for_ruby_bounds() {
    let frame = custom_effect_ruby_test_frame(".nudge");
    let baseline = measure_frame_elements_at_page_with_time(&frame, 800, 600, 96.0, 572.0, 0, 0.0)
        .expect("baseline bounds");

    let mut registry = RichTextEffectRegistry::default();
    registry.insert_lambda(".nudge", |ctx| {
        ctx.placement.y -= 32.0;
    });
    let mut state = RichTextStateStore::default();
    let shifted = measure_frame_elements_with_effect_registry(
        &frame,
        NativeCaptureViewport::new(800, 600, 96.0, 572.0, 0),
        &mut registry,
        &mut state,
    )
    .expect("registry ruby bounds");

    let baseline_ruby = baseline
        .iter()
        .find_map(|bounds| {
            matches!(bounds.element, NativeFrameElement::Ruby { .. })
                .then_some(bounds.ruby.as_ref())?
        })
        .expect("baseline ruby geometry");
    let shifted_ruby = shifted
        .iter()
        .find_map(|bounds| {
            matches!(bounds.element, NativeFrameElement::Ruby { .. })
                .then_some(bounds.ruby.as_ref())?
        })
        .expect("shifted ruby geometry");
    assert!(
        shifted_ruby.annotation_bbox.y + 20 < baseline_ruby.annotation_bbox.y,
        "custom effect registry should move observed ruby annotation bounds: {:?} -> {:?}",
        baseline_ruby.annotation_bbox,
        shifted_ruby.annotation_bbox
    );
}

fn custom_effect_test_frame(id: &str) -> LineDisplayFrame {
    custom_effect_test_frame_with_params(id, BTreeMap::new())
}

fn plain_effect_test_frame() -> LineDisplayFrame {
    let spec = LineDisplaySpec {
        line: RuntimeLineId("say.test.custom.effect".to_owned()),
        callee: "alice".to_owned(),
        speaker_label: None,
        text_key: None,
        window: None,
        voice: None,
        look: None,
        style: None,
        base_styles: Vec::new(),
        default_inline_failure_policy: None,
        style_contributions: Vec::new(),
        args: Vec::new(),
        content: RichTextDocument::new(vec![RichTextNode::Text {
            text: "A".to_owned(),
        }]),
    };
    spec.resolve_frame(&RuntimeLineContext::default())
        .expect("frame resolves")
}

fn custom_effect_test_frame_with_phase(id: &str, phase: RichTextEffectPhase) -> LineDisplayFrame {
    custom_effect_test_frame_with_params_and_phase(id, BTreeMap::new(), phase)
}

fn custom_effect_test_frame_with_params(
    id: &str,
    params: BTreeMap<String, RichTextParam>,
) -> LineDisplayFrame {
    custom_effect_test_frame_with_params_and_phase(id, params, RichTextEffectPhase::GlyphTransform)
}

fn custom_effect_test_frame_with_params_and_phase(
    id: &str,
    params: BTreeMap<String, RichTextParam>,
    phase: RichTextEffectPhase,
) -> LineDisplayFrame {
    let spec = LineDisplaySpec {
        line: RuntimeLineId("say.test.custom.effect".to_owned()),
        callee: "alice".to_owned(),
        speaker_label: None,
        text_key: None,
        window: None,
        voice: None,
        look: None,
        style: None,
        base_styles: Vec::new(),
        default_inline_failure_policy: None,
        style_contributions: Vec::new(),
        args: Vec::new(),
        content: RichTextDocument::new(vec![
            RichTextNode::StyleStart {
                style: RichTextStyle::Effect {
                    effect: RichTextEffectDescriptor {
                        id: id.to_owned(),
                        params,
                        target: RichTextEffectTarget::Run,
                        phase,
                        state_scope: RichTextStateScope::Glyph,
                    },
                },
            },
            RichTextNode::Text {
                text: "A".to_owned(),
            },
            RichTextNode::StyleEnd {
                name: "/".to_owned(),
            },
        ]),
    };
    spec.resolve_frame(&RuntimeLineContext::default())
        .expect("frame resolves")
}

fn custom_effect_ruby_test_frame(id: &str) -> LineDisplayFrame {
    let spec = LineDisplaySpec {
        line: RuntimeLineId("say.test.custom.ruby.effect".to_owned()),
        callee: "alice".to_owned(),
        speaker_label: None,
        text_key: None,
        window: None,
        voice: None,
        look: None,
        style: None,
        base_styles: Vec::new(),
        default_inline_failure_policy: None,
        style_contributions: Vec::new(),
        args: Vec::new(),
        content: RichTextDocument::new(vec![
            RichTextNode::StyleStart {
                style: RichTextStyle::Effect {
                    effect: RichTextEffectDescriptor {
                        id: id.to_owned(),
                        params: BTreeMap::new(),
                        target: RichTextEffectTarget::Run,
                        phase: RichTextEffectPhase::GlyphTransform,
                        state_scope: RichTextStateScope::Glyph,
                    },
                },
            },
            RichTextNode::Ruby {
                base: "夢".to_owned(),
                ruby: "ゆめ".to_owned(),
            },
            RichTextNode::StyleEnd {
                name: "/".to_owned(),
            },
        ]),
    };
    spec.resolve_frame(&RuntimeLineContext::default())
        .expect("frame resolves")
}
