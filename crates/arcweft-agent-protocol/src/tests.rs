use crate::action::{AgentActionDispatch, AgentActionKind, AgentActionTarget};
use crate::diagnostic::{AgentDiagnostic, AgentDiagnosticSeverity};
use crate::geometry::{AgentBBox, AgentCoordinateSpace, AgentRgbaColor, AgentViewport};
use crate::hit_test::AgentHitTestHit;
use crate::image::{
    AgentImageAlignment, AgentImageComposition, AgentImageContentBBox, AgentImageCropOrigin,
    AgentImageFit, AgentImageKind, AgentImageMetadata, AgentImageObjectParam, AgentImageObjectRef,
    AgentImageRenderer, AgentImageResource, AgentImageScope, AgentImageTransform,
    AgentLayerCaptureRef, AgentLayerCaptureRefs, AgentObjectCaptureRef, AgentObjectCaptureRefs,
};
use crate::object::{
    AgentObservedImageContent, AgentObservedLayer, AgentObservedObject, AgentObservedObjectContent,
};
use crate::observation::AgentObservationReport;
use crate::presentation::AgentPresentationTree;
use crate::proxy::AgentPresentationObjectProxyRef;
use crate::resource::{
    AgentBinaryEncoding, AgentBinaryResourceBody, AgentResource, AgentResourceBody,
    AgentResourceKind,
};
use crate::rich_text::{
    AgentHitRegion, AgentHitRegionKind, AgentRichTextElementKind, AgentRichTextElementRef,
};
use crate::session::{AgentAssignment, AgentAudioState};
use crate::ui::AgentUiTree;
use arcweft_core::plan::RuntimeLineId;
use arcweft_render_text::{
    LineDisplayFrame, RichTextAssignOp, RichTextCascadeLayer, RichTextEffectDescriptor,
    RichTextEffectPhase, RichTextEffectTarget, RichTextObjectProxyDeclaration, RichTextParam,
    RichTextPresentation, RichTextRange, RichTextSettingSource, RichTextStateScope,
    RichTextStyleContribution, RichTextTextSource,
};
use std::collections::BTreeMap;

fn test_capture_refs() -> AgentObjectCaptureRefs {
    AgentObjectCaptureRefs {
        object_id_color: AgentRgbaColor {
            red: 120,
            green: 130,
            blue: 140,
            alpha: 255,
        },
        captures: vec![AgentObjectCaptureRef {
            kind: AgentImageKind::Mask,
            uri: "arcweft://session/cli/frame/1/object.object.dialogue.0.0.mask.png".to_owned(),
            mime_type: "image/png".to_owned(),
            page: 0,
            width: 3,
            height: 4,
        }],
    }
}

fn test_layer_capture_refs() -> AgentLayerCaptureRefs {
    AgentLayerCaptureRefs {
        captures: vec![AgentLayerCaptureRef {
            kind: AgentImageKind::Color,
            uri: "arcweft://session/cli/frame/1/layer.dialogue.png".to_owned(),
            mime_type: "image/png".to_owned(),
            page: 0,
            width: 10,
            height: 20,
        }],
    }
}

fn test_line_display_frame() -> LineDisplayFrame {
    LineDisplayFrame {
        line: RuntimeLineId("say.test.001".to_owned()),
        callee: "alice".to_owned(),
        text: "Hello".to_owned(),
        base_styles: Vec::new(),
        default_inline_failure_policy: None,
        style_contributions: vec![RichTextStyleContribution {
            path: "rich_text.ruby.size".to_owned(),
            layer: RichTextCascadeLayer::DialogueDefaults,
            source: RichTextSettingSource::EngineDefault {
                key: "dialogue.rich_text.ruby.size".to_owned(),
            },
            op: RichTextAssignOp::Replace,
            value: "14".to_owned(),
            style_index: None,
            active: true,
            shadowed_by: None,
        }],
        nodes: Vec::new(),
        display_map: arcweft_render_text::RichTextDisplayMap::default(),
        host_events: Vec::new(),
        inline_failures: Vec::new(),
        unresolved: Vec::new(),
    }
}

fn test_raw_mask_image_resource() -> AgentImageResource {
    AgentImageResource {
        kind: AgentImageKind::Mask,
        renderer: AgentImageRenderer::Native,
        scope: AgentImageScope::Object {
            id: "object.dialogue.0.0".to_owned(),
        },
        composition: AgentImageComposition::MaskAttachment,
        page: 0,
        capture_step: 0,
        capture_time_millis: 0,
        uri: "arcweft://session/cli/frame/7/object.object.dialogue.0.0.mask.rgba".to_owned(),
        mime_type: "application/octet-stream".to_owned(),
        width: 3,
        height: 4,
        hash: "raw-hash".to_owned(),
        crop_origin: Some(AgentImageCropOrigin {
            space: AgentCoordinateSpace::Viewport,
            x: 96,
            y: 548,
        }),
        content_bbox: Some(AgentImageContentBBox {
            x: 0,
            y: 0,
            width: 3,
            height: 4,
        }),
        content_viewport_bbox: Some(AgentImageContentBBox {
            x: 96,
            y: 548,
            width: 3,
            height: 4,
        }),
        content_pixels: Some(12),
        object: None,
        diagnostics: Vec::new(),
        written: None,
    }
}

fn assert_image_metadata(resource: &AgentResource, expected: AgentImageMetadata) {
    assert_eq!(resource.image, Some(expected));
}

fn test_mcp_observation_report() -> AgentObservationReport {
    AgentObservationReport {
        status: "ok".to_owned(),
        session_id: "cli".to_owned(),
        tick: 7,
        frame_id: "frame.7".to_owned(),
        state_hash: "state-hash".to_owned(),
        render_hash: "render-hash".to_owned(),
        source: "game.arcw".to_owned(),
        viewport: AgentViewport {
            width: 1280,
            height: 720,
            scale: 1.0,
        },
        images: vec![AgentImageResource {
            kind: AgentImageKind::Color,
            renderer: AgentImageRenderer::Native,
            scope: AgentImageScope::Viewport,
            composition: AgentImageComposition::Framebuffer,
            page: 0,
            capture_step: 0,
            capture_time_millis: 0,
            uri: "arcweft://session/cli/frame/7/color.png".to_owned(),
            mime_type: "image/png".to_owned(),
            width: 1280,
            height: 720,
            hash: "image-hash".to_owned(),
            crop_origin: None,
            content_bbox: None,
            content_viewport_bbox: None,
            content_pixels: None,
            object: None,
            diagnostics: Vec::new(),
            written: None,
        }],
        layers: Vec::new(),
        objects: Vec::new(),
        presentation_tree: AgentPresentationTree::from_layers_and_objects(&[], &[]),
        actions: Vec::new(),
        ui_tree: AgentUiTree {
            root: "ui.root".to_owned(),
            children: Vec::new(),
        },
        scene_graph: Vec::new(),
        audio_state: AgentAudioState {
            active_voices: Vec::new(),
            pending_events: Vec::new(),
        },
        logs: Vec::new(),
        signals: vec![AgentAssignment {
            name: "signal.ready".to_owned(),
            value: "true".to_owned(),
        }],
        metrics: Vec::new(),
        events: Vec::new(),
        diagnostics: Vec::new(),
        steps: 1,
        capture_time_millis: None,
        task_requests: 0,
        final_status: "done Return(\"ok\")".to_owned(),
        overlay_svg: Some("<svg/>".to_owned()),
    }
}

#[allow(clippy::too_many_lines)]
fn test_serialization_observation_report() -> AgentObservationReport {
    let bbox = AgentBBox {
        space: AgentCoordinateSpace::Viewport,
        x: 1,
        y: 2,
        width: 3,
        height: 4,
    };
    let layers = vec![AgentObservedLayer {
        id: "dialogue".to_owned(),
        visible: true,
        bbox: bbox.clone(),
        object_count: 1,
        capture_refs: test_layer_capture_refs(),
    }];
    let objects = vec![AgentObservedObject {
        id: "object.dialogue.0.0".to_owned(),
        parent_id: None,
        entity: Some("alice".to_owned()),
        layer: "dialogue".to_owned(),
        role: "dialogue_textbox".to_owned(),
        visible: true,
        enabled: true,
        polygon: bbox.polygon(),
        bbox: bbox.clone(),
        capture_refs: test_capture_refs(),
        object_layer: None,
        object_depth: None,
        text: Some("Hello".to_owned()),
        rich_text_ref: Some(test_rich_text_ref(&bbox)),
        content: AgentObservedObjectContent::RichText {
            frame: Box::new(test_line_display_frame()),
        },
    }];
    let presentation_tree = AgentPresentationTree::from_layers_and_objects(&layers, &objects);
    AgentObservationReport {
        status: "ok".to_owned(),
        session_id: "cli".to_owned(),
        tick: 1,
        frame_id: "frame.1".to_owned(),
        state_hash: "state".to_owned(),
        render_hash: "render".to_owned(),
        source: "game.arcw".to_owned(),
        viewport: AgentViewport {
            width: 1280,
            height: 720,
            scale: 1.0,
        },
        images: vec![AgentImageResource {
            kind: AgentImageKind::OverlaySvg,
            renderer: AgentImageRenderer::Native,
            scope: AgentImageScope::Viewport,
            composition: AgentImageComposition::OverlayVector,
            page: 0,
            capture_step: 0,
            capture_time_millis: 0,
            uri: "arcweft://session/cli/frame/1/overlay.svg".to_owned(),
            mime_type: "image/svg+xml".to_owned(),
            width: 1280,
            height: 720,
            hash: "render".to_owned(),
            crop_origin: None,
            content_bbox: None,
            content_viewport_bbox: None,
            content_pixels: None,
            object: None,
            diagnostics: Vec::new(),
            written: None,
        }],
        layers,
        objects,
        presentation_tree,
        actions: vec![AgentActionTarget {
            id: "action.advance_text.object.dialogue.0.0".to_owned(),
            target: "object.dialogue.0.0".to_owned(),
            action: AgentActionKind::AdvanceText,
            kind: AgentActionDispatch::Semantic,
            enabled: true,
        }],
        ui_tree: AgentUiTree {
            root: "ui.root".to_owned(),
            children: vec!["dialogue.layer".to_owned()],
        },
        scene_graph: Vec::new(),
        audio_state: AgentAudioState {
            active_voices: Vec::new(),
            pending_events: Vec::new(),
        },
        logs: Vec::new(),
        signals: Vec::new(),
        metrics: Vec::new(),
        events: Vec::new(),
        diagnostics: vec![AgentDiagnostic {
            step: 0,
            severity: AgentDiagnosticSeverity::Info,
            source: None,
            code: None,
            effect_id: None,
            message: "ready".to_owned(),
        }],
        steps: 1,
        capture_time_millis: None,
        task_requests: 0,
        final_status: "done Return(\"ok\")".to_owned(),
        overlay_svg: None,
    }
}

fn test_rich_text_ref(bbox: &AgentBBox) -> AgentRichTextElementRef {
    AgentRichTextElementRef {
        kind: AgentRichTextElementKind::TextRun,
        index: 0,
        page: 0,
        range: RichTextRange::new(0, 5),
        node_index: 0,
        source: Some(RichTextTextSource::Text),
        ruby: None,
        presentation: Some(RichTextPresentation {
            effects: vec![RichTextEffectDescriptor {
                id: "shake".to_owned(),
                params: BTreeMap::default(),
                target: RichTextEffectTarget::default(),
                phase: RichTextEffectPhase::GlyphTransform,
                state_scope: RichTextStateScope::default(),
            }],
            ..RichTextPresentation::default()
        }),
        orientation: None,
        vertical_form: None,
        ruby_base_bbox: None,
        ruby_annotation_bbox: None,
        object_layer: None,
        object_depth: None,
        hit_test: false,
        hit_regions: vec![AgentHitRegion {
            kind: AgentHitRegionKind::TextRun,
            bbox: bbox.clone(),
            range: RichTextRange::new(0, 5),
            proxy_id: None,
            proxy_type: None,
            proxy_declaration: None,
            proxy_role: None,
            proxy_layer: None,
            depth: None,
            proxy_params: BTreeMap::new(),
        }],
    }
}

#[test]
#[allow(clippy::too_many_lines)]
fn observation_report_serializes_stable_snake_case_enums() {
    let report = test_serialization_observation_report();

    let json = serde_json::to_value(&report).expect("report serializes");

    assert_eq!(json["images"][0]["kind"], "overlay_svg");
    assert_eq!(json["images"][0]["renderer"], "native");
    assert_eq!(json["images"][0]["scope"]["kind"], "viewport");
    assert_eq!(json["images"][0]["composition"], "overlay_vector");
    assert_eq!(
        serde_json::to_value(AgentImageComposition::MaskedFramebufferCrop)
            .expect("composition serializes"),
        "masked_framebuffer_crop"
    );
    assert_eq!(
        serde_json::to_value(AgentImageComposition::ObjectIdAttachment)
            .expect("composition serializes"),
        "object_id_attachment"
    );
    assert_eq!(
        serde_json::to_value(AgentImageComposition::MaskAttachment)
            .expect("composition serializes"),
        "mask_attachment"
    );
    assert_eq!(
        json["layers"][0]["capture_refs"]["captures"][0]["kind"],
        "color"
    );
    assert_eq!(json["objects"][0]["bbox"]["space"], "viewport");
    assert_eq!(
        json["objects"][0]["capture_refs"]["captures"][0]["kind"],
        "mask"
    );
    assert_eq!(
        json["objects"][0]["capture_refs"]["object_id_color"]["alpha"],
        255
    );
    assert_eq!(json["objects"][0]["rich_text_ref"]["kind"], "text_run");
    assert_eq!(
        serde_json::to_value(AgentRichTextElementKind::TextPage)
            .expect("rich-text element kind serializes"),
        "text_page"
    );
    assert_eq!(
        serde_json::to_value(AgentRichTextElementKind::TextLine)
            .expect("rich-text element kind serializes"),
        "text_line"
    );
    assert_eq!(
        serde_json::to_value(AgentRichTextElementKind::TextGlyph)
            .expect("rich-text element kind serializes"),
        "text_glyph"
    );
    assert_eq!(json["objects"][0]["rich_text_ref"]["source"], "text");
    assert_eq!(
        json["objects"][0]["rich_text_ref"]["presentation"]["effects"][0]["id"],
        "shake"
    );
    assert_eq!(
        json["objects"][0]["rich_text_ref"]["presentation"]["effects"][0]["phase"],
        "glyph_transform"
    );
    assert_eq!(json["objects"][0]["content"]["kind"], "rich_text");
    assert_eq!(
        json["objects"][0]["content"]["frame"]["style_contributions"][0]["path"],
        "rich_text.ruby.size"
    );
    assert_eq!(
        json["objects"][0]["content"]["frame"]["style_contributions"][0]["layer"],
        "dialogue_defaults"
    );
    assert_eq!(
        json["objects"][0]["rich_text_ref"]["hit_regions"][0]["kind"],
        "text_run"
    );
    assert_eq!(json["presentation_tree"]["root"], "presentation.root");
    assert_eq!(json["presentation_tree"]["nodes"][0]["kind"], "root");
    assert_eq!(
        json["presentation_tree"]["nodes"][0]["children"][0],
        "presentation.layer.dialogue"
    );
    assert_eq!(json["presentation_tree"]["nodes"][1]["kind"], "layer");
    assert_eq!(
        json["presentation_tree"]["nodes"][1]["layer_id"],
        "dialogue"
    );
    assert_eq!(
        json["presentation_tree"]["nodes"][1]["children"][0],
        "object.dialogue.0.0"
    );
    assert_eq!(json["presentation_tree"]["nodes"][2]["kind"], "object");
    assert_eq!(
        json["presentation_tree"]["nodes"][2]["object_id"],
        "object.dialogue.0.0"
    );
    assert_eq!(
        json["presentation_tree"]["nodes"][2]["role"],
        "dialogue_textbox"
    );
    assert_eq!(
        json["presentation_tree"]["nodes"][2]["rich_text_kind"],
        "text_run"
    );
    assert_eq!(
        json["presentation_tree"]["nodes"][2]["effects"][0]["id"],
        "shake"
    );
    assert_eq!(
        json["presentation_tree"]["nodes"][2]["effects"][0]["phase"],
        "glyph_transform"
    );
    assert_eq!(
        serde_json::to_value(AgentHitRegionKind::Object).expect("hit-region kind serializes"),
        "object"
    );
    assert_eq!(
        serde_json::to_value(AgentHitRegionKind::ObjectProxy).expect("hit-region kind serializes"),
        "object_proxy"
    );
    assert_eq!(
        serde_json::to_value(AgentHitRegionKind::TextPage).expect("hit-region kind serializes"),
        "text_page"
    );
    assert_eq!(
        serde_json::to_value(AgentHitRegionKind::TextLine).expect("hit-region kind serializes"),
        "text_line"
    );
    assert_eq!(
        serde_json::to_value(AgentHitRegionKind::TextGlyph).expect("hit-region kind serializes"),
        "text_glyph"
    );
    assert_eq!(
        serde_json::to_value(AgentHitRegionKind::RubyAnnotation)
            .expect("hit-region kind serializes"),
        "ruby_annotation"
    );
    assert_eq!(json["actions"][0]["action"], "advance_text");
    assert_eq!(json["actions"][0]["kind"], "semantic");
    assert_eq!(json["diagnostics"][0]["severity"], "info");
}

#[test]
fn hit_region_serializes_proxy_params_when_present() {
    let region = AgentHitRegion {
        kind: AgentHitRegionKind::TextObjectProxy,
        bbox: AgentBBox {
            space: AgentCoordinateSpace::Viewport,
            x: 1,
            y: 2,
            width: 3,
            height: 4,
        },
        range: RichTextRange::new(0, 3),
        proxy_id: Some("hotspot".to_owned()),
        proxy_type: Some("KeywordHit".to_owned()),
        proxy_declaration: Some(RichTextObjectProxyDeclaration {
            struct_name: "KeywordHit".to_owned(),
            attribute: "text_proxy".to_owned(),
        }),
        proxy_role: Some("keyword".to_owned()),
        proxy_layer: None,
        depth: Some(4000),
        proxy_params: BTreeMap::from([(
            "channel".to_owned(),
            RichTextParam::Selector {
                value: "choice".to_owned(),
            },
        )]),
    };

    let json = serde_json::to_value(&region).expect("hit region serializes");

    assert_eq!(json["kind"], "text_object_proxy");
    assert_eq!(json["proxy_declaration"]["struct_name"], "KeywordHit");
    assert_eq!(json["proxy_declaration"]["attribute"], "text_proxy");
    assert_eq!(json["proxy_params"]["channel"]["value"], "choice");
}

#[test]
fn hit_test_hit_serializes_capture_refs() {
    let bbox = AgentBBox {
        space: AgentCoordinateSpace::Viewport,
        x: 10,
        y: 20,
        width: 30,
        height: 40,
    };
    let hit = AgentHitTestHit {
        rank: 0,
        object_id: "object.dialogue.0.0.proxy.0.0".to_owned(),
        object: AgentImageObjectRef {
            id: "object.dialogue.0.0.proxy.0.0".to_owned(),
            parent_id: Some("object.dialogue.0.0".to_owned()),
            entity: Some("character.alice".to_owned()),
            layer: "dialogue.rich_text".to_owned(),
            role: "rich_text_proxy".to_owned(),
            bbox: bbox.clone(),
            polygon: bbox.polygon(),
            capture_refs: test_capture_refs(),
            object_layer: Some("ui".to_owned()),
            object_depth: Some(4000),
            text: Some("Hit".to_owned()),
            rich_text_ref: None,
            image_ref: None,
        },
        layer: "ui".to_owned(),
        role: "rich_text_proxy".to_owned(),
        text: Some("Hit".to_owned()),
        bbox: bbox.clone(),
        polygon: bbox.polygon(),
        capture_refs: test_capture_refs(),
        region: AgentHitRegion {
            kind: AgentHitRegionKind::TextObjectProxy,
            bbox,
            range: RichTextRange::new(0, 3),
            proxy_id: Some("hotspot".to_owned()),
            proxy_type: Some("KeywordHit".to_owned()),
            proxy_declaration: Some(RichTextObjectProxyDeclaration {
                struct_name: "KeywordHit".to_owned(),
                attribute: "text_proxy".to_owned(),
            }),
            proxy_role: Some("keyword".to_owned()),
            proxy_layer: Some("ui".to_owned()),
            depth: Some(4000),
            proxy_params: BTreeMap::new(),
        },
        rich_text_ref: None,
        depth: Some(4000),
    };

    let json = serde_json::to_value(&hit).expect("hit serializes");

    assert_eq!(json["object"]["layer"], "dialogue.rich_text");
    assert_eq!(json["object"]["object_layer"], "ui");
    assert_eq!(json["layer"], "ui");
    assert_eq!(json["polygon"].as_array().unwrap().len(), 4);
    assert_eq!(json["capture_refs"]["object_id_color"]["alpha"], 255);
    assert_eq!(json["capture_refs"]["captures"][0]["kind"], "mask");
}

#[test]
fn image_resource_metadata_preserves_observed_object_ref() {
    let report = test_mcp_observation_report();
    let bbox = AgentBBox {
        space: AgentCoordinateSpace::Viewport,
        x: 96,
        y: 548,
        width: 3,
        height: 4,
    };
    let mut rich_text_ref = test_rich_text_ref(&bbox);
    rich_text_ref.object_layer = Some("ui".to_owned());
    rich_text_ref.object_depth = Some(7000);
    let mut image = test_raw_mask_image_resource();
    image.object = Some(AgentImageObjectRef {
        id: "object.dialogue.0.0.run.0".to_owned(),
        parent_id: Some("object.dialogue.0.0".to_owned()),
        entity: Some("character.alice".to_owned()),
        layer: "dialogue".to_owned(),
        role: "rich_text_run".to_owned(),
        bbox: bbox.clone(),
        polygon: bbox.polygon(),
        capture_refs: test_capture_refs(),
        object_layer: Some("ui".to_owned()),
        object_depth: Some(7000),
        text: Some("Hello".to_owned()),
        rich_text_ref: Some(rich_text_ref.clone()),
        image_ref: None,
    });

    let resource = report.image_resource(&image, &[255; 48]);

    assert_eq!(
        resource
            .image
            .as_ref()
            .and_then(|image| image.object.as_ref()),
        image.object.as_ref()
    );
    assert_eq!(
        resource
            .image
            .as_ref()
            .and_then(|image| image.object.as_ref())
            .and_then(|object| object.rich_text_ref.as_ref()),
        Some(&rich_text_ref)
    );
    let json = serde_json::to_value(&resource).expect("resource serializes");
    assert_eq!(json["image"]["object"]["layer"], "dialogue");
    assert_eq!(json["image"]["object"]["bbox"]["x"], 96);
    assert_eq!(
        json["image"]["object"]["polygon"].as_array().unwrap().len(),
        4
    );
    assert_eq!(
        json["image"]["object"]["capture_refs"]["object_id_color"]["alpha"],
        255
    );
    assert_eq!(
        json["image"]["object"]["capture_refs"]["captures"][0]["kind"],
        "mask"
    );
    assert_eq!(json["image"]["object"]["object_layer"], "ui");
    assert_eq!(json["image"]["object"]["object_depth"], 7000);
}

#[test]
fn generic_image_object_metadata_does_not_require_rich_text_ref() {
    let bbox = AgentBBox {
        space: AgentCoordinateSpace::Viewport,
        x: 20,
        y: 30,
        width: 40,
        height: 50,
    };
    let object = AgentObservedObject {
        id: "object.image.logo".to_owned(),
        parent_id: None,
        entity: Some("asset.logo.webp".to_owned()),
        layer: "hud".to_owned(),
        role: "image".to_owned(),
        visible: true,
        enabled: true,
        bbox: bbox.clone(),
        polygon: bbox.polygon(),
        capture_refs: test_capture_refs(),
        object_layer: Some("hud.foreground".to_owned()),
        object_depth: Some(2500),
        text: None,
        rich_text_ref: None,
        content: AgentObservedObjectContent::Image(Box::new(test_observed_image_content())),
    };

    let image_object = AgentImageObjectRef::from_observed(&object);
    assert_eq!(image_object.object_layer.as_deref(), Some("hud.foreground"));
    assert_eq!(image_object.object_depth, Some(2500));
    assert!(image_object.rich_text_ref.is_none());
    let image_ref = image_object
        .image_ref
        .as_ref()
        .expect("image object metadata preserves active image payload");
    assert_eq!(image_ref.source, "ui.image.7");
    assert_eq!(image_ref.asset.as_deref(), Some("asset.logo.webp"));
    assert_eq!(image_ref.frame_index, Some(1));
    assert_eq!(image_ref.local_time_millis, Some(250));
    assert_eq!(image_ref.opacity_milli, Some(750));
    assert_eq!(image_ref.fit, Some(AgentImageFit::Intrinsic));
    assert_eq!(
        image_ref.alignment,
        Some(AgentImageAlignment {
            x_milli: 1_000,
            y_milli: 0,
        })
    );
    assert_eq!(image_ref.intrinsic_width, Some(64));
    assert_eq!(image_ref.intrinsic_height, Some(32));
    assert_eq!(image_ref.actions, vec!["action.inspect".to_owned()]);
    assert_eq!(image_ref.proxies[0].id, "proxy.logo.hotspot");

    let object_json = serde_json::to_value(&object).expect("image object serializes");
    assert_eq!(
        object_json["content"]["opacity_milli"],
        serde_json::json!(750)
    );
    assert_eq!(object_json["content"]["fit"], "intrinsic");
    assert_eq!(object_json["content"]["alignment"]["x_milli"], 1_000);
    assert_eq!(object_json["enabled"], serde_json::json!(true));
    assert_eq!(
        object_json["content"]["transform"]["tx_milli"],
        serde_json::json!(12000)
    );

    let tree = AgentPresentationTree::from_layers_and_objects(
        &[AgentObservedLayer {
            id: "hud".to_owned(),
            visible: true,
            bbox,
            object_count: 1,
            capture_refs: test_layer_capture_refs(),
        }],
        &[object],
    );
    let image_node = tree
        .nodes
        .iter()
        .find(|node| node.object_id.as_deref() == Some("object.image.logo"))
        .expect("image object appears in presentation tree");
    assert_eq!(image_node.role.as_deref(), Some("image"));
    assert_eq!(image_node.object_layer.as_deref(), Some("hud.foreground"));
    assert_eq!(image_node.object_depth, Some(2500));
    assert_eq!(
        image_node.object_proxy_ids,
        vec!["proxy.logo.hotspot".to_owned()]
    );
    assert_eq!(
        image_node.object_proxies[0].type_name.as_deref(),
        Some("LogoHotspot")
    );

    let json = serde_json::to_value(&tree).expect("presentation tree serializes");
    assert_eq!(json["nodes"][2]["object_layer"], "hud.foreground");
    assert_eq!(json["nodes"][2]["object_proxies"][0]["hit_test"], true);
}

fn test_observed_image_content() -> AgentObservedImageContent {
    AgentObservedImageContent {
        source: "ui.image.7".to_owned(),
        object: Some("image.logo".to_owned()),
        target: Some("target.logo".to_owned()),
        asset: Some("asset.logo.webp".to_owned()),
        frame_index: Some(1),
        local_time_millis: Some(250),
        opacity_milli: Some(750),
        fit: Some(AgentImageFit::Intrinsic),
        alignment: Some(AgentImageAlignment {
            x_milli: 1_000,
            y_milli: 0,
        }),
        transform: Some(AgentImageTransform {
            m11_milli: 1_000,
            m12_milli: 0,
            m21_milli: 0,
            m22_milli: 1_000,
            tx_milli: 12_000,
            ty_milli: 8_000,
        }),
        intrinsic_width: Some(64),
        intrinsic_height: Some(32),
        actions: vec!["action.inspect".to_owned()],
        params: BTreeMap::from([(
            "param.role".to_owned(),
            AgentImageObjectParam::Text {
                value: "title-logo".to_owned(),
            },
        )]),
        proxies: vec![AgentPresentationObjectProxyRef {
            id: "proxy.logo.hotspot".to_owned(),
            type_name: Some("LogoHotspot".to_owned()),
            role: Some("inspect".to_owned()),
            layer: Some("hud.hit".to_owned()),
            depth: Some(2600),
            declaration: None,
            hit_test: true,
            params: BTreeMap::from([(
                "param.channel".to_owned(),
                RichTextParam::Text {
                    value: "preview".to_owned(),
                },
            )]),
        }],
    }
}

#[test]
fn image_resource_metadata_preserves_capture_diagnostics() {
    let report = test_mcp_observation_report();
    let mut image = test_raw_mask_image_resource();
    image.diagnostics = vec![AgentDiagnostic {
        step: 7,
        severity: AgentDiagnosticSeverity::Warning,
        source: Some("native_rich_text".to_owned()),
        code: Some("missing_shader".to_owned()),
        effect_id: Some("ghost_glow".to_owned()),
        message: "native rich-text missing_shader: ghost_glow".to_owned(),
    }];

    let resource = report.image_resource(&image, &[255; 48]);
    let metadata = resource.image.expect("image metadata is attached");

    assert_eq!(metadata.diagnostics, image.diagnostics);
}

#[test]
fn observation_report_builds_mcp_style_resources() {
    let report = test_mcp_observation_report();

    let latest = report
        .observation_resource()
        .expect("latest resource serializes");
    let objects = report.objects_resource().expect("objects serialize");
    let presentation_tree = report
        .presentation_tree_resource()
        .expect("presentation tree serializes");
    let overlay = report.overlay_svg_resource().expect("overlay exists");
    let image = report.image_resource(&report.images[0], b"\x89PNG\r\n\x1a\n");
    let raw_image = report.image_resource(&test_raw_mask_image_resource(), &[255; 48]);
    let signals = report.signals_resource().expect("signals serialize");

    assert_eq!(latest.uri, "arcweft://session/cli/observation/latest.json");
    assert_eq!(latest.kind, AgentResourceKind::ObservationLatest);
    assert_eq!(objects.uri, "arcweft://session/cli/frame/7/objects.json");
    assert_eq!(
        presentation_tree.uri,
        "arcweft://session/cli/frame/7/presentation-tree.json"
    );
    assert_eq!(presentation_tree.kind, AgentResourceKind::PresentationTree);
    assert!(matches!(presentation_tree.body, AgentResourceBody::Json(_)));
    assert_eq!(overlay.uri, "arcweft://session/cli/frame/7/overlay.svg");
    assert_eq!(overlay.mime_type, "image/svg+xml");
    assert_eq!(image.uri, "arcweft://session/cli/frame/7/color.png");
    assert_eq!(image.kind, AgentResourceKind::Image);
    assert_eq!(image.mime_type, "image/png");
    assert_image_metadata(
        &image,
        AgentImageMetadata {
            kind: AgentImageKind::Color,
            renderer: AgentImageRenderer::Native,
            scope: AgentImageScope::Viewport,
            composition: AgentImageComposition::Framebuffer,
            page: 0,
            capture_step: 0,
            capture_time_millis: 0,
            width: 1280,
            height: 720,
            crop_origin: None,
            pixel_format: None,
            row_stride_bytes: None,
            content_bbox: None,
            content_viewport_bbox: None,
            content_pixels: None,
            object: None,
            diagnostics: Vec::new(),
        },
    );
    assert_image_metadata(
        &raw_image,
        AgentImageMetadata {
            kind: AgentImageKind::Mask,
            renderer: AgentImageRenderer::Native,
            scope: AgentImageScope::Object {
                id: "object.dialogue.0.0".to_owned(),
            },
            composition: AgentImageComposition::MaskAttachment,
            page: 0,
            capture_step: 0,
            capture_time_millis: 0,
            width: 3,
            height: 4,
            crop_origin: Some(AgentImageCropOrigin {
                space: AgentCoordinateSpace::Viewport,
                x: 96,
                y: 548,
            }),
            pixel_format: Some("rgba8_unorm".to_owned()),
            row_stride_bytes: Some(12),
            content_bbox: Some(AgentImageContentBBox {
                x: 0,
                y: 0,
                width: 3,
                height: 4,
            }),
            content_viewport_bbox: Some(AgentImageContentBBox {
                x: 96,
                y: 548,
                width: 3,
                height: 4,
            }),
            content_pixels: Some(12),
            object: None,
            diagnostics: Vec::new(),
        },
    );
    assert_eq!(signals.uri, "arcweft://session/cli/signals.json");
    assert!(matches!(overlay.body, AgentResourceBody::Text(_)));
    assert!(matches!(
        image.body,
        AgentResourceBody::BytesBase64(AgentBinaryResourceBody {
            encoding: AgentBinaryEncoding::Base64,
            ..
        })
    ));
}
