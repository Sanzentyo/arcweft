use super::*;
use arcweft_agent_protocol::{
    presentation::AgentPresentationTree, session::AgentAudioState, view::AgentViewTree,
};

#[test]
fn session_info_reuses_the_published_latest_capture() {
    let capture = test_latest_capture_resource();
    let source_uri = capture.uri.clone();
    let mut state = AgentMcpState {
        content_policy_mode: AgentContentPolicyMode::LocalDev,
        report: Some(test_observation_report()),
        capture_resources: vec![capture],
        ..AgentMcpState::default()
    };

    let result =
        agent_mcp_call_session_info(&mut state).expect("session info publishes latest capture");
    let McpContentBlock::Text { text } = &result.content[0] else {
        panic!("session info must return JSON text");
    };
    let info: serde_json::Value = serde_json::from_str(text).expect("session info JSON parses");
    let expected = state
        .published_resources
        .get_by_source_uri(&source_uri)
        .cloned()
        .expect("latest capture publication is cached by source URI");
    let expected_image = expected
        .resource()
        .image
        .as_ref()
        .expect("published color capture keeps image metadata");
    let selected_scope = &expected_image
        .selected_capture
        .as_ref()
        .expect("published capture keeps selected-capture metadata")
        .scope;

    assert_eq!(
        selected_scope,
        &arcweft_agent_protocol::image::AgentSelectedCaptureScope::from(&expected_image.scope)
    );
    assert!(matches!(
        &expected_image.scope,
        AgentImageScope::Layer { id } if id.starts_with("layer.") && id != "dialogue"
    ));

    assert_eq!(
        info["latest_capture"],
        serde_json::to_value(expected_image).expect("published image metadata serializes")
    );
    assert_eq!(
        info["latest_capture_uri"],
        serde_json::json!(expected.resource().uri)
    );
    assert_eq!(
        info["latest_capture_resource"],
        serde_json::to_value(resource_descriptor(&expected))
            .expect("published resource descriptor serializes")
    );
    let expected_descriptor =
        serde_json::to_value(resource_descriptor(&expected)).expect("descriptor serializes");
    assert!(
        info["resources"]
            .as_array()
            .expect("session resources are an array")
            .contains(&expected_descriptor),
        "latest capture must be one of the already-published session resources"
    );

    let read = agent_mcp_resource_read(
        &serde_json::json!({
            "uri": expected.resource().uri.as_str(),
            "max_privacy": "sensitive"
        }),
        &mut state,
    )
    .expect("cached latest capture reads back through its public URI");
    assert_eq!(
        read["contents"][0]["uri"],
        serde_json::json!(expected.resource().uri.as_str())
    );
}

fn test_latest_capture_resource() -> AgentResource {
    let source_scope = AgentCaptureScope::Layer("dialogue".to_owned());
    let bbox = AgentBBox {
        space: AgentCoordinateSpace::Viewport,
        x: 0,
        y: 0,
        width: 1,
        height: 1,
    };
    let selected_capture =
        agent_selected_capture_metadata_for_ref(AgentSelectedCaptureMetadataSpec {
            scope: &source_scope,
            kind: AgentImageKind::Color,
            composition: AgentImageKind::Color.default_capture_composition(),
            unclipped: &bbox,
            clipped: &bbox,
            source: AgentCaptureSourceIdentity::Layer {
                id: "dialogue".to_owned(),
                object_count: 0,
            },
            mask: None,
            viewport: None,
        });
    AgentResource {
        uri: AgentResourceUri::new("arcweft://session/test/frame/0/layer.dialogue.color.rgba")
            .expect("test capture URI is nonempty"),
        kind: AgentResourceKind::Image,
        mime_type: "application/octet-stream".to_owned(),
        hash: "blake3:raw-rgba-test".to_owned(),
        image: Some(AgentImageMetadata {
            kind: AgentImageKind::Color,
            renderer: AgentImageRenderer::Native,
            scope: AgentImageScope::Layer {
                id: "dialogue".to_owned(),
            },
            composition: AgentImageKind::Color.default_capture_composition(),
            page: 0,
            capture_step: 0,
            capture_time_millis: 0,
            width: 1,
            height: 1,
            crop_origin: None,
            pixel_format: Some("rgba8_unorm".to_owned()),
            row_stride_bytes: Some(4),
            content_bbox: None,
            content_viewport_bbox: None,
            content_pixels: None,
            object: None,
            view: None,
            selected_capture: Some(selected_capture),
            diagnostics: Vec::new(),
        }),
        body: AgentResourceBody::BytesBase64(
            arcweft_agent_protocol::resource::AgentBinaryResourceBody {
                encoding: AgentBinaryEncoding::Base64,
                data: "IECA/w==".to_owned(),
            },
        ),
    }
}

fn test_observation_report() -> AgentObservationReport {
    AgentObservationReport {
        status: "ok".to_owned(),
        session_id: "test".to_owned(),
        tick: 0,
        frame_id: "frame.0".to_owned(),
        state_hash: "state".to_owned(),
        render_hash: "render".to_owned(),
        source: "test.arcw".to_owned(),
        viewport: AgentViewport {
            width: 1,
            height: 1,
            scale: 1.0,
        },
        images: Vec::new(),
        layers: Vec::new(),
        views: Vec::new(),
        objects: Vec::new(),
        presentation_tree: AgentPresentationTree::from_layers_and_objects(&[], &[]),
        actions: Vec::new(),
        scroll_regions: Vec::new(),
        virtual_lists: Vec::new(),
        view_tree: AgentViewTree {
            root: "view.root".to_owned(),
            children: Vec::new(),
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
        diagnostics: Vec::new(),
        steps: 0,
        capture_time_millis: None,
        task_requests: 0,
        final_status: "done".to_owned(),
        overlay_svg: None,
    }
}
