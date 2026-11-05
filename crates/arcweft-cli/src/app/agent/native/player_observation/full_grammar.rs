//! Positive raw attachments belong to the local Native capture boundary.
//! MCP's separate mandatory publication policy can withhold Mask/ObjectId.

use super::*;
use arcweft_agent_protocol::{
    action::AgentActionKind, resource::AgentResourceBody, text_context::AgentTextContextContent,
};
use base64::Engine as _;

fn full_grammar_frame(line: &str, sample: Option<f32>) -> NativePlayerObservedFrame {
    let mut arguments = serde_json::json!({
        "source": Path::new(env!("CARGO_MANIFEST_DIR")).join("../../samples/rich-text-full-grammar.arcw"),
        "entry":"entry.rich_text_full_grammar", "steps":4, "max_ops":128,
        "viewport_width":1280, "viewport_height":720,
    });
    if let Some(sample) = sample {
        arguments["capture_time"] = serde_json::json!(sample);
    }
    let options = super::super::mcp_resources::agent_mcp_observe_options(&arguments).unwrap();
    let mut runtime = native_player_runtime_state_for_options(
        &options,
        &[crate::app::desktop_native_adapter_registrar],
    )
    .unwrap();
    let request = NativeFrameObservationRequest::for_progress(&options);
    let mut observed =
        observe_native_player_runtime(&mut runtime, &request, BundleStepInput::default()).unwrap();
    let target = arcweft_core::plan::RuntimeLineId::from_runtime_line_value(line).unwrap();
    for _ in 0..80 {
        assert!(
            observed.report.diagnostics.is_empty(),
            "{:?}",
            observed.report.diagnostics
        );
        if observed.report.text_contexts.entries().any(|entry| {
            matches!(&entry.content, AgentTextContextContent::DisplayStage {context, ..} if context.frame().line == target)
        }) {
            AgentObservationReport::decode_json(&serde_json::to_vec(&observed.report).unwrap())
                .unwrap();
            return observed;
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
    panic!("bounded Native progression did not reach {line}")
}

/// Read only an attachment issued by this retained frame, with its complete
/// object/context metadata. No replay of a foreign frame's opaque URI or ID.
fn raw_attachment(
    frame: &NativePlayerObservedFrame,
    object: &AgentObservedObject,
    kind: &str,
) -> (arcweft_agent_protocol::image::AgentImageMetadata, Vec<u8>) {
    let report = &frame.report;
    let context = object.text_context.as_ref().unwrap();
    let element = object.rich_text_ref.as_ref().unwrap();
    report
        .text_contexts
        .validate_element(&object.id, context, Some(element), object.text.as_deref())
        .unwrap();
    let entry = report.text_contexts.resolve(context).unwrap();
    assert_eq!(
        entry.element_parent_id(Some(element)).unwrap(),
        object.parent_id
    );
    let refs = serde_json::to_value(&object.capture_refs).unwrap();
    let issued = refs["captures"]
        .as_array()
        .unwrap()
        .iter()
        .find(|capture| {
            capture["kind"] == kind && capture["mime_type"] == "application/octet-stream"
        })
        .unwrap();
    let uri = issued["uri"].as_str().unwrap();
    let resource = super::super::agent_observe_resource_by_uri_with_page_and_time_and_frame_store(
        report,
        uri,
        None,
        super::super::agent_report_capture_time_seconds(report),
        &frame.image_frames,
    )
    .unwrap();
    assert_eq!(resource.uri.as_str(), uri);
    assert_eq!(resource.mime_type, "application/octet-stream");
    let metadata = resource.image.unwrap();
    arcweft_agent_protocol::image::AgentImageMetadata::decode_json(
        &serde_json::to_vec(&metadata).unwrap(),
    )
    .unwrap();
    let image = serde_json::to_value(&metadata).unwrap();
    assert_eq!(image["renderer"], "native");
    assert_eq!(image["kind"], kind);
    assert_eq!(image["scope"]["kind"], "object");
    assert_eq!(image["scope"]["id"], object.id);
    assert_eq!(
        metadata.object.as_ref().unwrap(),
        &arcweft_agent_protocol::image::AgentImageObjectRef::from_observed(object)
    );
    assert_eq!(metadata.capture_step, report.steps);
    assert_eq!(issued["width"], metadata.width);
    assert_eq!(issued["height"], metadata.height);
    let AgentResourceBody::BytesBase64(body) = resource.body else {
        panic!("actual raw attachment")
    };
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(body.data)
        .unwrap();
    assert_eq!(
        bytes.len() as u64,
        u64::from(metadata.width) * u64::from(metadata.height) * 4
    );
    let nonzero = bytes
        .chunks_exact(4)
        .filter(|pixel| pixel[3] > 0)
        .collect::<Vec<_>>();
    assert_eq!(
        u64::try_from(nonzero.len()).unwrap(),
        metadata.content_pixels.unwrap()
    );
    if kind == "object_id" {
        let expected = super::super::agent_object_id_color(&object.id);
        assert!(nonzero.iter().all(|pixel| **pixel == expected));
    }
    (metadata, bytes)
}

fn run_object<'a>(frame: &'a NativePlayerObservedFrame, text: &str) -> &'a AgentObservedObject {
    frame
        .report
        .objects
        .iter()
        .find(|object| object.role == "rich_text_run" && object.text.as_deref() == Some(text))
        .unwrap()
}

#[test]
fn progressed_full_grammar_frame_keeps_issued_cluster_mask_and_object_id_readback() {
    let frame = full_grammar_frame("say.full.005", None);
    let report = &frame.report;
    let target =
        arcweft_core::plan::RuntimeLineId::from_runtime_line_value("say.full.005").unwrap();
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
        for kind in kinds {
            let (metadata, _) = raw_attachment(&frame, object, kind);
            assert_eq!((metadata.width, metadata.height), (26, 26));
            assert!(metadata.content_pixels.unwrap() > 0);
        }
    }
}

#[test]
fn progressed_full_grammar_proxies_page_line_and_shader_keep_actual_readback() {
    let frame = full_grammar_frame("say.full.006", None);
    assert_eq!(frame.report.tick, 63);
    let selected = frame
        .report
        .objects
        .iter()
        .filter(|object| {
            object.role == "rich_text_proxy"
                || object.role == "rich_text_page"
                || (object.role == "rich_text_line"
                    && object.rich_text_ref.as_ref().unwrap().index == 0)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        selected.len(),
        6,
        "four authored proxies, one page, one line"
    );
    for object in selected {
        for kind in ["mask", "object_id"] {
            let (metadata, _) = raw_attachment(&frame, object, kind);
            assert!(metadata.content_pixels.unwrap() > 0, "{kind} {}", object.id);
        }
    }
    let (metadata, bytes) = raw_attachment(&frame, run_object(&frame, "shader"), "color");
    assert!(metadata.content_pixels.unwrap() > 0);
    let blue_glow_pixels = bytes
        .chunks_exact(4)
        .filter(|pixel| {
            pixel[3] > 0
                && pixel[2] > pixel[0].saturating_add(20)
                && pixel[2] > pixel[1].saturating_add(5)
        })
        .count();
    assert!(
        blue_glow_pixels > 32,
        "soft_glow actual blue-tinted pixels: {blue_glow_pixels}"
    );
}

#[test]
fn progressed_full_grammar_effect_samples_keep_typewriter_masks_and_motion_pixels() {
    // The visual sample evaluates ordinary stage reveal as well as Fx. This
    // long authored line has not revealed its effects section at 1s. Use the
    // established 60s completed visual sample and later samples while retaining
    // the original entered frame, instead of advancing past its page boundary.
    let zero = full_grammar_frame("say.full.005", Some(0.0));
    let early = full_grammar_frame("say.full.005", Some(60.0));
    let late = full_grammar_frame("say.full.005", Some(61.0));
    for (frame, millis) in [(&zero, 0), (&early, 60_000), (&late, 61_000)] {
        assert_eq!(frame.report.tick, 55);
        assert_eq!(frame.report.capture_time_millis, Some(millis));
    }
    let (hidden, hidden_bytes) = raw_attachment(&zero, run_object(&zero, "typewriter"), "mask");
    let (visible, visible_bytes) = raw_attachment(&early, run_object(&early, "typewriter"), "mask");
    assert_eq!(hidden.content_pixels, Some(0));
    assert!(visible.content_pixels.unwrap() > 0);
    assert_ne!(hidden_bytes, visible_bytes);
    // Both real authored sparkle calls survive; the current sample names the
    // second run "sparkle", rather than the obsolete fixture's "host".
    for text in ["shake", "custom effect", "sparkle"] {
        let (a, a_bytes) = raw_attachment(&early, run_object(&early, text), "color");
        let (b, b_bytes) = raw_attachment(&late, run_object(&late, text), "color");
        assert!(a.content_pixels.unwrap() > 0 && b.content_pixels.unwrap() > 0);
        assert_ne!(
            a_bytes, b_bytes,
            "{text} changes at explicit 60s/61s samples"
        );
    }
    let wave_early = full_grammar_frame("say.full.005", Some(60.125));
    let wave_late = full_grammar_frame("say.full.005", Some(60.375));
    let (a, a_bytes) = raw_attachment(&wave_early, run_object(&wave_early, "wave"), "color");
    let (b, b_bytes) = raw_attachment(&wave_late, run_object(&wave_late, "wave"), "color");
    assert_eq!(a.capture_time_millis, 60_125);
    assert_eq!(b.capture_time_millis, 60_375);
    assert!(a.content_pixels.unwrap() > 0 && b.content_pixels.unwrap() > 0);
    assert_ne!(a_bytes, b_bytes, "wave changes at 60.125s/60.375s samples");
}
