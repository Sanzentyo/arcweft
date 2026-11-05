//! Positive raw attachments belong to the local Native capture boundary.
//! MCP's separate mandatory publication policy can withhold Mask/ObjectId.

use super::*;
use arcweft_agent_protocol::{
    action::AgentActionKind, resource::AgentResourceBody, text_context::AgentTextContextContent,
};
use base64::Engine as _;

#[test]
fn progressed_full_grammar_frame_keeps_issued_cluster_mask_and_object_id_readback() {
    let options = super::super::mcp_resources::agent_mcp_observe_options(&serde_json::json!({
        "source": Path::new(env!("CARGO_MANIFEST_DIR")).join("../../samples/rich-text-full-grammar.arcw"),
        "entry":"entry.rich_text_full_grammar", "steps":4, "max_ops":128,
        "viewport_width":1280, "viewport_height":720,
    })).unwrap();
    let mut runtime = native_player_runtime_state_for_options(
        &options,
        &[crate::app::desktop_native_adapter_registrar],
    )
    .unwrap();
    let request = NativeFrameObservationRequest::for_progress(&options);
    let mut observed =
        observe_native_player_runtime(&mut runtime, &request, BundleStepInput::default()).unwrap();
    let target =
        arcweft_core::plan::RuntimeLineId::from_runtime_line_value("say.full.005").unwrap();
    for _ in 0..80 {
        assert!(observed.report.diagnostics.is_empty());
        if observed.report.text_contexts.entries().any(|entry| {
            matches!(&entry.content, AgentTextContextContent::DisplayStage {context, ..} if context.frame().line == target)
        }) {
            break;
        }
        let input = if observed
            .report
            .actions
            .iter()
            .any(|action| action.enabled && action.action == AgentActionKind::AdvanceText)
        {
            super::super::observe::native_agent_action_step_input(
                &observed.report,
                runtime.session.presentation(),
                arcweft_agent_protocol::protocol::AgentAction::AdvanceText,
            )
            .unwrap()
        } else {
            BundleStepInput::default()
        };
        observed = observe_native_player_runtime(&mut runtime, &request, input).unwrap();
    }
    let report =
        AgentObservationReport::decode_json(&serde_json::to_vec(&observed.report).unwrap())
            .unwrap();
    assert_eq!(report.tick, 55);
    for (text, start, end, kinds) in [
        ("吾", 28, 31, &["mask"][..]),
        ("縦", 67, 70, &["mask", "object_id"][..]),
    ] {
        let object = report
            .objects
            .iter()
            .find(|object| {
                object.role == "rich_text_cluster"
                    && object.text.as_deref() == Some(text)
                    && object.rich_text_ref.as_ref().is_some_and(|element| {
                        element.range == arcweft_text_model::RichTextRange::new(start, end)
                    })
            })
            .unwrap();
        let entry = report
            .text_contexts
            .resolve(object.text_context.as_ref().unwrap())
            .unwrap();
        assert!(
            matches!(&entry.content, AgentTextContextContent::DisplayStage {context, ..} if context.frame().line == target)
        );
        assert_eq!((object.bbox.width, object.bbox.height), (26, 26));
        let refs = serde_json::to_value(&object.capture_refs).unwrap();
        for kind in kinds {
            let issued = refs["captures"]
                .as_array()
                .unwrap()
                .iter()
                .find(|capture| {
                    capture["kind"] == *kind && capture["mime_type"] == "application/octet-stream"
                })
                .unwrap();
            let uri = issued["uri"].as_str().unwrap();
            let resource =
                super::super::agent_observe_resource_by_uri_with_page_and_time_and_frame_store(
                    &report,
                    uri,
                    None,
                    super::super::agent_report_capture_time_seconds(&report),
                    &observed.image_frames,
                )
                .unwrap();
            assert_eq!(resource.uri.as_str(), uri);
            assert_eq!(resource.mime_type, "application/octet-stream");
            let metadata = resource.image.unwrap();
            let image = serde_json::to_value(&metadata).unwrap();
            assert_eq!(image["renderer"], "native");
            assert_eq!(image["kind"], *kind);
            assert_eq!(image["scope"]["kind"], "object");
            assert_eq!(image["scope"]["id"], object.id);
            assert_eq!(image["object"]["id"], object.id);
            assert_eq!((metadata.width, metadata.height), (26, 26));
            arcweft_agent_protocol::image::AgentImageMetadata::decode_json(
                &serde_json::to_vec(&metadata).unwrap(),
            )
            .unwrap();
            let AgentResourceBody::BytesBase64(body) = resource.body else {
                panic!("actual raw attachment")
            };
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(body.data)
                .unwrap();
            assert_eq!(bytes.len(), 26 * 26 * 4);
            let nonzero = bytes
                .chunks_exact(4)
                .filter(|pixel| pixel[3] > 0)
                .collect::<Vec<_>>();
            assert!(!nonzero.is_empty());
            assert_eq!(
                u64::try_from(nonzero.len()).unwrap(),
                metadata.content_pixels.unwrap()
            );
            if *kind == "object_id" {
                let expected = super::super::agent_object_id_color(&object.id);
                assert!(nonzero.iter().all(|pixel| **pixel == expected));
            }
        }
    }
}
