use super::*;
use arcweft_agent_protocol::geometry::AgentViewport;
use arcweft_presentation::fx::{
    FxApplication, FxApplicationResolver, FxDiagnostic, FxEvaluationBinding,
};
use arcweft_render_text::{ResolvedTextStyle, TextFontFamily};
use arcweft_render_wgpu::geometry::{
    ChoiceScroll, InteractionVisualState, PreparedRichTextStageRequest, RenderPreferences,
    RenderScene, RenderViewport, SharedFramePlanContext,
};
use arcweft_text_model::{
    DialoguePreparedTextRole, DialogueRevealElapsed, DialogueRevealPolicy, ResolvedRichTextNode,
    RichTextControl, RichTextControlMarker, RichTextDisplayMap, RichTextNodeCount,
    RichTextNodeIndex, RichTextNodeRange, RichTextPresentation, RichTextRange,
    RichTextRubyAnnotation, RichTextTextRun, RichTextTextRunRange, RichTextTextSource,
};

use super::super::super::prepared_text_observation::{
    agent_dialogue_prepared_text_objects, agent_prepared_text_context_catalog,
};

struct NoFx;
impl FxApplicationResolver for NoFx {
    fn resolve<'a>(
        &'a self,
        _: &FxApplication,
    ) -> Result<FxEvaluationBinding<'a>, Box<FxDiagnostic>> {
        panic!("the admitted fixture has no Fx applications")
    }
}

fn scope() -> AgentTextContextScope {
    AgentTextContextScope {
        session_id: "cli".to_owned(),
        frame_id: "frame.12".to_owned(),
        tick: 12,
        capture_time_millis: Some(500),
    }
}

fn prepared_fixture() -> PreparedFrame {
    let (mut frame, owner) = prepared_fixture_parts();
    frame.push_prepared_text_owner(owner).unwrap();
    frame
}

fn prepared_fixture_parts() -> (PreparedFrame, PreparedTextOwner) {
    let mut display = super::super::super::tests::test_line_display_frame();
    display.text = "旧夢".to_owned();
    display.nodes = vec![
        ResolvedRichTextNode::Ruby {
            body: vec![ResolvedRichTextNode::Text {
                text: "旧".to_owned(),
            }],
            ruby: "きゅう".to_owned(),
        },
        ResolvedRichTextNode::Control {
            control: RichTextControl::Clear,
        },
        ResolvedRichTextNode::Ruby {
            body: vec![ResolvedRichTextNode::Text {
                text: "夢".to_owned(),
            }],
            ruby: "ゆめ".to_owned(),
        },
    ];
    display.display_map = RichTextDisplayMap {
        source_node_count: RichTextNodeCount::new(5),
        text_runs: vec![
            RichTextTextRun {
                range: RichTextRange::new(0, 3),
                source: RichTextTextSource::Text,
                node_index: RichTextNodeIndex::new(1),
                styles: Vec::new(),
                presentation: RichTextPresentation::default(),
            },
            RichTextTextRun {
                range: RichTextRange::new(3, 6),
                source: RichTextTextSource::Text,
                node_index: RichTextNodeIndex::new(4),
                styles: Vec::new(),
                presentation: RichTextPresentation::default(),
            },
        ],
        ruby_annotations: vec![
            RichTextRubyAnnotation {
                owner_node: RichTextNodeIndex::new(0),
                body_nodes: RichTextNodeRange::new(
                    RichTextNodeIndex::new(1),
                    RichTextNodeIndex::new(2),
                ),
                base_runs: RichTextTextRunRange::new(0, 1),
                base_range: RichTextRange::new(0, 3),
                ruby: "きゅう".to_owned(),
                styles: Vec::new(),
                presentation: RichTextPresentation::default(),
            },
            RichTextRubyAnnotation {
                owner_node: RichTextNodeIndex::new(3),
                body_nodes: RichTextNodeRange::new(
                    RichTextNodeIndex::new(4),
                    RichTextNodeIndex::new(5),
                ),
                base_runs: RichTextTextRunRange::new(1, 2),
                base_range: RichTextRange::new(3, 6),
                ruby: "ゆめ".to_owned(),
                styles: Vec::new(),
                presentation: RichTextPresentation::default(),
            },
        ],
        controls: vec![RichTextControlMarker {
            node_index: RichTextNodeIndex::new(2),
            text_offset: 3,
            control: RichTextControl::Clear,
            range: None,
        }],
        ..RichTextDisplayMap::default()
    };
    prepared_display_fixture_parts(display, HitRect::new(20.0, 30.0, 180.0, 80.0))
}

fn prepared_display_fixture_parts(
    display: arcweft_text_model::LineDisplayFrame,
    bounds: HitRect,
) -> (PreparedFrame, PreparedTextOwner) {
    display.validate().unwrap();
    let mut planner = SharedFramePlanContext::new();
    planner
        .register_font_bytes(
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../web/assets/noto-sans-jp-vf.ttf"
            ))
            .to_vec(),
        )
        .unwrap();
    let mut frame = planner
        .prepare(&RenderScene {
            content_avoidance_regions: Vec::new(),
            choices: Vec::new(),
            text_inputs: Vec::new(),
            action_buttons: Vec::new(),
            focus_groups: Vec::new(),
            focus_navigation: Vec::new(),
            images: Vec::new(),
            viewport: RenderViewport {
                logical_width: 256.0,
                logical_height: 128.0,
                physical_width: 256,
                physical_height: 128,
                scale_factor: 1.0,
            },
            visual_time_millis: 500,
            preferences: RenderPreferences::default(),
            interaction: InteractionVisualState::default(),
            choice_scroll: ChoiceScroll::default(),
            scroll_regions: Vec::new(),
        })
        .unwrap();
    let prepared = planner
        .push_prepared_rich_text_stage(
            &mut frame,
            display.stage(0).unwrap(),
            &PreparedRichTextStageRequest {
                bounds,
                default_style: ResolvedTextStyle::new(
                    vec![TextFontFamily::SansSerif],
                    24_000,
                    32_000,
                )
                .unwrap(),
                reveal_elapsed: DialogueRevealElapsed::from_nanos(500_000_000),
                reveal_policy: DialogueRevealPolicy {
                    complete_stage: true,
                    instant_characters: false,
                },
            },
            &NoFx,
        )
        .unwrap();
    let owner = PreparedTextOwner::new(
        prepared.text,
        arcweft_id::PublicId::try_new("view.fixture.content").unwrap(),
        PreparedTextOwnerKind::DialogueView {
            dialogue: 7,
            entry: 9,
            mount: 17,
            role: DialoguePreparedTextRole::Content,
        },
        prepared.context,
        bounds,
    )
    .with_parent(arcweft_id::PublicId::try_new("view.fixture").unwrap());
    (frame, owner)
}

fn observed_fixture(
    frame: &PreparedFrame,
    catalog: &AgentTextContextCatalog,
) -> Vec<AgentObservedObject> {
    observed_fixture_for_owner(frame, catalog, 0)
}

fn observed_fixture_for_owner(
    frame: &PreparedFrame,
    catalog: &AgentTextContextCatalog,
    index: usize,
) -> Vec<AgentObservedObject> {
    let owner = PreparedTextObservationOwner::new(&frame.prepared_text_owners()[index]);
    let geometry = frame.prepared_text_geometries().unwrap();
    agent_dialogue_prepared_text_objects(
        12,
        &owner,
        catalog.reference(u32::try_from(index).unwrap()).unwrap(),
        catalog,
        frame,
        geometry.get(&owner.prepared().text).unwrap(),
        &AgentViewport {
            width: 256,
            height: 128,
            scale: 1.0,
        },
    )
    .unwrap()
}

fn mixed_owner_fixture() -> (PreparedFrame, [PreparedTextOwner; 3]) {
    let (mut frame, content) = prepared_fixture_parts();
    let item = frame.text.get(content.text).unwrap().clone();
    let speaker_text = frame.text.push(item.clone()).unwrap();
    let second_text = frame.text.push(item.clone()).unwrap();
    let mut speaker = content.clone();
    speaker.text = speaker_text;
    speaker.semantic_id = arcweft_id::PublicId::try_new("view.fixture.speaker").unwrap();
    speaker.kind = PreparedTextOwnerKind::DialogueView {
        dialogue: 7,
        entry: 9,
        mount: 17,
        role: DialoguePreparedTextRole::CharacterDisplayName,
    };
    speaker.context = PreparedTextContext::document(
        content.context.ranges().prepared_origin(),
        item.interaction.text.len(),
    )
    .unwrap();
    let mut second = content.clone();
    second.text = second_text;
    second.semantic_id = arcweft_id::PublicId::try_new("view.fixture.second_content").unwrap();
    second.kind = PreparedTextOwnerKind::DialogueView {
        dialogue: 7,
        entry: 9,
        mount: 18,
        role: DialoguePreparedTextRole::Content,
    };
    (frame, [content, speaker, second])
}

#[test]
fn mixed_dialogue_owners_resolve_each_child_and_capture_target_for_every_prepared_order() {
    let (base, owners) = mixed_owner_fixture();
    let viewport = AgentViewport {
        width: 256,
        height: 128,
        scale: 1.0,
    };
    for order in [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ] {
        let mut frame = base.clone();
        for index in order {
            frame
                .push_prepared_text_owner(owners[index].clone())
                .unwrap();
        }
        let catalog = agent_prepared_text_context_catalog(&frame, scope()).unwrap();
        let contexts = PlayerTextCaptureContexts::bind(&frame, &catalog, &scope()).unwrap();
        let geometry = frame.prepared_text_geometries().unwrap();
        let mut objects =
            super::super::super::prepared_text_observation::agent_view_prepared_text_objects(
                12, &frame, &catalog, &geometry, &viewport,
            );
        for (ordinal, prepared) in frame.prepared_text_owners().iter().enumerate() {
            if prepared.context.stage().is_some() {
                let owner = PreparedTextObservationOwner::new(prepared);
                objects.extend(
                    agent_dialogue_prepared_text_objects(
                        12,
                        &owner,
                        catalog.reference(u32::try_from(ordinal).unwrap()).unwrap(),
                        &catalog,
                        &frame,
                        geometry.get(&prepared.text).unwrap(),
                        &viewport,
                    )
                    .unwrap(),
                );
            }
        }
        assert_eq!(
            objects
                .iter()
                .filter(|object| object.rich_text_ref.is_none())
                .count(),
            3
        );
        assert_eq!(catalog.entries().len(), 3);
        for object in &objects {
            object.validate_text_context(&catalog).unwrap();
            let reference = object.text_context.as_ref().unwrap();
            let ordinal = usize::try_from(reference.owner()).unwrap();
            let actual = &frame.prepared_text_owners()[ordinal];
            let entry = catalog.resolve(reference).unwrap();
            assert_eq!(entry.owner.prepared_text, actual.text.index());
            assert_eq!(entry.owner.kind, actual.kind);
            assert!(std::ptr::eq(
                contexts.owner(object).unwrap().unwrap().prepared,
                actual
            ));
            assert!(Arc::ptr_eq(
                entry.content.layout(),
                &frame.text.get(actual.text).unwrap().layout
            ));
            if object.rich_text_ref.as_ref().is_some_and(|element| {
                matches!(
                    element.kind,
                    AgentRichTextElementKind::Ruby | AgentRichTextElementKind::GlyphCluster
                )
            }) {
                let selected = player_prepared_text_capture_selection(&contexts, object)
                    .unwrap()
                    .unwrap();
                assert_eq!(selected.text(), actual.text);
                assert!(!selected.glyph_indices().is_empty());
            }
        }
    }
}

#[test]
fn distinct_dialogue_content_targets_share_context_while_duplicate_prepared_ownership_rejects() {
    let (mut frame, owners) = mixed_owner_fixture();
    frame.push_prepared_text_owner(owners[0].clone()).unwrap();
    frame.push_prepared_text_owner(owners[2].clone()).unwrap();
    let catalog = agent_prepared_text_context_catalog(&frame, scope()).unwrap();
    let first = catalog.resolve(&catalog.reference(0).unwrap()).unwrap();
    let second = catalog.resolve(&catalog.reference(1).unwrap()).unwrap();
    assert_ne!(first.owner.root_id, second.owner.root_id);
    assert_ne!(first.owner.prepared_text, second.owner.prepared_text);
    assert!(Arc::ptr_eq(
        first.content.display_stage().unwrap(),
        second.content.display_stage().unwrap()
    ));
    assert!(Arc::ptr_eq(first.content.layout(), second.content.layout()));
    let mut duplicate = owners[0].clone();
    duplicate.semantic_id = arcweft_id::PublicId::try_new("view.fixture.alias").unwrap();
    duplicate.kind = PreparedTextOwnerKind::DialogueView {
        dialogue: 7,
        entry: 9,
        mount: 19,
        role: DialoguePreparedTextRole::Content,
    };
    assert!(
        matches!(frame.push_prepared_text_owner(duplicate), Err(arcweft_render_wgpu::geometry::FramePlanError::DuplicatePreparedTextOwner { index }) if index == owners[0].text.index())
    );
    assert_eq!(frame.prepared_text_owners().len(), 2);
    assert_eq!(
        agent_prepared_text_context_catalog(&frame, scope()).unwrap(),
        catalog
    );
    PlayerTextCaptureContexts::bind(&frame, &catalog, &scope()).unwrap();
}

#[test]
fn cleared_ruby_and_body_selection_bind_the_actual_context_and_prepared_ordinals() {
    let frame = prepared_fixture();
    let catalog = agent_prepared_text_context_catalog(&frame, scope()).unwrap();
    let contexts = PlayerTextCaptureContexts::bind(&frame, &catalog, &scope()).unwrap();
    let objects = observed_fixture(&frame, &catalog);
    for object in &objects {
        object.validate_text_context(&catalog).unwrap();
    }
    let ruby = objects
        .iter()
        .find(|object| {
            object
                .rich_text_ref
                .as_ref()
                .is_some_and(|element| element.kind == AgentRichTextElementKind::Ruby)
        })
        .unwrap();
    assert_eq!(ruby.text.as_deref(), Some("夢 (ゆめ)"));
    assert_eq!(
        ruby.rich_text_ref.as_ref().unwrap().index,
        1,
        "display annotation ordinal retains the cleared annotation"
    );
    let selection = player_prepared_text_capture_selection(&contexts, ruby)
        .unwrap()
        .unwrap();
    let item = frame.text.get(selection.text()).unwrap();
    assert!(
        selection.glyph_indices().iter().any(|index| matches!(
            item.glyphs[usize::try_from(*index).unwrap()].source,
            PreparedGlyphSource::Ruby { ruby_index: 0, .. }
        )),
        "capture selects the retained renderer annotation, not display ordinal 1"
    );
    assert_eq!(selection.glyph_indices().len(), item.glyphs.len());
    let cluster = objects
        .iter()
        .find(|object| {
            object
                .rich_text_ref
                .as_ref()
                .is_some_and(|element| element.kind == AgentRichTextElementKind::GlyphCluster)
        })
        .unwrap();
    assert_eq!(
        cluster.rich_text_ref.as_ref().unwrap().range,
        RichTextRange::new(3, 6)
    );
    let selection = player_prepared_text_capture_selection(&contexts, cluster)
        .unwrap()
        .unwrap();
    assert!(!selection.glyph_indices().is_empty());
    for index in selection.glyph_indices() {
        let glyph = &item.glyphs[usize::try_from(*index).unwrap()];
        assert!(matches!(glyph.source, PreparedGlyphSource::Body { .. }));
        assert_eq!(glyph.source_range, RichTextRange::new(3, 6));
    }
}

#[test]
fn transported_equal_context_does_not_admit_a_prepared_frame() {
    let frame = prepared_fixture();
    let catalog = agent_prepared_text_context_catalog(&frame, scope()).unwrap();
    let decoded =
        AgentTextContextCatalog::decode_json(&serde_json::to_vec(&catalog).unwrap()).unwrap();
    assert_eq!(catalog, decoded);
    assert!(matches!(
        PlayerTextCaptureContexts::bind(&frame, &decoded, &scope()),
        Err(PlayerTextCaptureSelectionError::PreparedOwnerMismatch { owner: 0 })
    ));
    let selected = catalog.selected([&catalog.reference(0).unwrap()]).unwrap();
    assert!(
        PlayerTextCaptureContexts::bind(&frame, &selected, &scope()).is_ok(),
        "issued subsets share actual context and layout allocations"
    );
}

#[test]
fn capture_binding_rejects_other_owner_mount_parent_and_sample() {
    let frame = prepared_fixture();
    let catalog = agent_prepared_text_context_catalog(&frame, scope()).unwrap();
    for change in 0..3 {
        let mut entry = catalog.entries().next().unwrap().clone();
        match change {
            0 => {
                entry.owner.kind = PreparedTextOwnerKind::DialogueView {
                    dialogue: 7,
                    entry: 9,
                    mount: 18,
                    role: DialoguePreparedTextRole::Content,
                }
            }
            1 => entry.owner.parent_id = Some("view.other".to_owned()),
            _ => entry.owner_ordinal = 1,
        }
        let changed = AgentTextContextCatalog::try_new(scope(), [entry]).unwrap();
        assert!(matches!(
            PlayerTextCaptureContexts::bind(&frame, &changed, &scope()),
            Err(PlayerTextCaptureSelectionError::PreparedOwnerMismatch { .. })
        ));
    }
    let mut other_scope = scope();
    other_scope.capture_time_millis = Some(501);
    assert!(matches!(
        PlayerTextCaptureContexts::bind(&frame, &catalog, &other_scope),
        Err(PlayerTextCaptureSelectionError::Context(
            AgentTextContextError::ScopeMismatch
        ))
    ));
}

#[test]
fn issued_capture_uri_keeps_its_context_and_rejects_another_retained_sample() {
    use super::super::super::capture::{agent_capture_request_from_uri, agent_capture_resource};
    use arcweft_agent_protocol::image::AgentImageMetadata;
    let frame = prepared_fixture();
    let catalog = agent_prepared_text_context_catalog(&frame, scope()).unwrap();
    let objects = observed_fixture(&frame, &catalog);
    let ruby = objects
        .iter()
        .find(|object| {
            object
                .rich_text_ref
                .as_ref()
                .is_some_and(|element| element.kind == AgentRichTextElementKind::Ruby)
        })
        .unwrap();
    let uri = ruby
        .capture_refs
        .captures
        .iter()
        .find(|capture| {
            capture.kind == arcweft_agent_protocol::image::AgentImageKind::Mask
                && capture.mime_type == "application/octet-stream"
        })
        .unwrap()
        .uri
        .clone();
    let mut report = super::super::super::tests::test_agent_observation_report(Some(500));
    report.tick = 12;
    report.frame_id = "frame.12".to_owned();
    report.steps = 13;
    report.viewport = AgentViewport {
        width: 256,
        height: 128,
        scale: 1.0,
    };
    report.objects = objects;
    report.text_contexts = catalog;
    let request = agent_capture_request_from_uri(&report, &uri).unwrap();
    assert_eq!(request.capture_step, 13);
    let frames = AgentImageFrameStore::from_full_attachments(
        scope(),
        256,
        128,
        vec![0; 256 * 128 * 4],
        vec![0; 256 * 128 * 4],
        vec![0; 256 * 128 * 4],
    )
    .unwrap();
    let resource = agent_capture_resource(&report, &request, &frames).unwrap();
    let metadata = resource.image.unwrap();
    assert_eq!(
        metadata
            .object
            .as_ref()
            .unwrap()
            .rich_text_ref
            .as_ref()
            .unwrap()
            .index,
        1
    );
    assert_eq!(metadata.text_contexts.as_ref().unwrap().entries().len(), 1);
    AgentImageMetadata::decode_json(&serde_json::to_vec(&metadata).unwrap()).unwrap();
    let mut other_scope = scope();
    other_scope.capture_time_millis = Some(501);
    let other = AgentImageFrameStore::from_full_attachments(
        other_scope,
        256,
        128,
        vec![0; 256 * 128 * 4],
        vec![0; 256 * 128 * 4],
        vec![0; 256 * 128 * 4],
    )
    .unwrap();
    assert!(agent_capture_resource(&report, &request, &other).is_err());
    let mut wrong_page = request.clone();
    wrong_page.page = 1;
    assert!(agent_capture_resource(&report, &wrong_page, &frames).is_err());
    let mut wrong_time = request;
    wrong_time.capture_time_seconds = 0.501;
    assert!(agent_capture_resource(&report, &wrong_time, &frames).is_err());
}

fn paired_body_capture_fixture() -> PreparedFrame {
    body_capture_text_fixture("――")
}

fn body_capture_text_fixture(text: &str) -> PreparedFrame {
    let mut display = super::super::super::tests::test_line_display_frame();
    display.text = text.to_owned();
    display.nodes = vec![ResolvedRichTextNode::Text {
        text: display.text.clone(),
    }];
    // ResolvedTextStyle consumes the authored style stack. Presentation is
    // retained metadata derived from that same stack, not a second flow input.
    let styles = vec![arcweft_text_model::RichTextStyle::Layout {
        layout: arcweft_text_model::RichTextLayout {
            writing_mode: arcweft_text_model::RichTextWritingMode::VerticalRl,
            ..Default::default()
        },
    }];
    let presentation = arcweft_text_model::presentation_from_styles(styles.iter());
    display.display_map = RichTextDisplayMap {
        source_node_count: RichTextNodeCount::new(1),
        text_runs: vec![RichTextTextRun {
            range: RichTextRange::new(0, display.text.len()),
            source: RichTextTextSource::Text,
            node_index: RichTextNodeIndex::new(0),
            styles,
            presentation,
        }],
        ..Default::default()
    };
    let (mut frame, owner) =
        prepared_display_fixture_parts(display, HitRect::new(20.0, 30.0, 180.0, 80.0));
    let item = frame
        .text
        .get(owner.text)
        .expect("real prepared text owner");
    assert_eq!(item.layout.runs.len(), 1);
    assert_eq!(
        item.layout.runs[0].style.writing_mode(),
        arcweft_text_model::RichTextWritingMode::VerticalRl,
        "the actual resolved style enters the vertical layout owner"
    );
    assert_eq!(
        item.layout.runs[0].writing_mode,
        arcweft_text_model::RichTextWritingMode::VerticalRl
    );
    frame.push_prepared_text_owner(owner).unwrap();
    frame
}

fn body_capture_report(
    catalog: AgentTextContextCatalog,
    objects: Vec<AgentObservedObject>,
) -> arcweft_agent_protocol::observation::AgentObservationReport {
    let mut report = super::super::super::tests::test_agent_observation_report(Some(500));
    report.tick = 12;
    report.frame_id = "frame.12".to_owned();
    report.steps = 13;
    report.capture_time_millis = Some(500);
    report.viewport = AgentViewport {
        width: 256,
        height: 128,
        scale: 1.0,
    };
    report.objects = objects;
    report.text_contexts = catalog;
    report
}

/// This fixture supplies attachment pixels to exercise scope/identity joins;
/// actual renderer alpha remains covered by the existing CLI crop regressions.
fn body_capture_attachment_store(pixels: &[(u32, u32, &str)]) -> AgentImageFrameStore {
    let mut object_ids = vec![0; 256 * 128 * 4];
    let mut mask = vec![0; 256 * 128 * 4];
    for &(x, y, id) in pixels {
        let offset = (usize::try_from(y).unwrap() * 256 + usize::try_from(x).unwrap()) * 4;
        object_ids[offset..offset + 4].copy_from_slice(&agent_object_id_color(id));
        mask[offset..offset + 4].copy_from_slice(&[255; 4]);
    }
    AgentImageFrameStore::from_full_attachments(
        scope(),
        256,
        128,
        vec![80; 256 * 128 * 4],
        object_ids,
        mask,
    )
    .unwrap()
}

fn body_capture_result(
    report: &arcweft_agent_protocol::observation::AgentObservationReport,
    object: &AgentObservedObject,
    frames: &AgentImageFrameStore,
) -> (arcweft_agent_protocol::image::AgentImageMetadata, Vec<u8>) {
    use super::super::super::capture::{agent_capture_image, agent_capture_request_from_uri};
    let uri = &object
        .capture_refs
        .captures
        .iter()
        .find(|capture| {
            capture.kind == arcweft_agent_protocol::image::AgentImageKind::Mask
                && capture.mime_type == "application/octet-stream"
        })
        .unwrap()
        .uri;
    let request = agent_capture_request_from_uri(report, uri).unwrap();
    let result = agent_capture_image(report, &request, frames).unwrap();
    let resource = report
        .image_resource(
            &result.image,
            super::super::super::agent_binary_resource_body(&result.bytes),
        )
        .unwrap();
    (resource.image.unwrap(), result.bytes)
}

fn assert_body_capture_geometry(
    result: &(arcweft_agent_protocol::image::AgentImageMetadata, Vec<u8>),
    object: &AgentObservedObject,
    pixels: u64,
) {
    use arcweft_agent_protocol::image::{AgentImageComposition, AgentImageCropOrigin};
    let (image, bytes) = result;
    assert_eq!(
        image.object.as_ref().unwrap(),
        &arcweft_agent_protocol::image::AgentImageObjectRef::from_observed(object)
    );
    assert_eq!(image.width, object.bbox.width);
    assert_eq!(image.height, object.bbox.height);
    assert_eq!(
        image.crop_origin,
        Some(AgentImageCropOrigin {
            space: object.bbox.space,
            x: object.bbox.x,
            y: object.bbox.y,
        })
    );
    assert_eq!(image.composition, AgentImageComposition::MaskAttachment);
    assert_eq!(image.content_pixels, Some(pixels));
    assert_eq!(
        bytes.chunks_exact(4).filter(|pixel| pixel[3] != 0).count(),
        usize::try_from(pixels).unwrap()
    );
    arcweft_agent_protocol::image::AgentImageMetadata::decode_json(
        &serde_json::to_vec(image).unwrap(),
    )
    .unwrap();
}

#[test]
fn native_capture_cluster_ordinal_keeps_atomic_source_siblings_outside_crop() {
    let frame = paired_body_capture_fixture();
    let catalog = agent_prepared_text_context_catalog(&frame, scope()).unwrap();
    let objects = observed_fixture(&frame, &catalog);
    let mut clusters = objects
        .iter()
        .filter(|object| {
            object
                .rich_text_ref
                .as_ref()
                .is_some_and(|element| element.kind == AgentRichTextElementKind::GlyphCluster)
        })
        .collect::<Vec<_>>();
    clusters.sort_by_key(|object| object.rich_text_ref.as_ref().unwrap().index);
    assert_eq!(
        clusters.len(),
        2,
        "the real vertical layout allocates two dash cells"
    );
    let left = (*clusters[0]).clone();
    let right = (*clusters[1]).clone();
    assert_eq!(
        left.rich_text_ref.as_ref().unwrap().range,
        right.rich_text_ref.as_ref().unwrap().range
    );
    assert_ne!(
        left.rich_text_ref.as_ref().unwrap().index,
        right.rich_text_ref.as_ref().unwrap().index
    );
    assert_ne!(left.bbox, right.bbox);
    assert!(
        !catalog
            .element_capture(&right)
            .and_then(|capture| capture.contains(&left))
            .unwrap()
    );
    let pixels = [
        (left.bbox.x + 1, left.bbox.y + 1, left.id.as_str()),
        (right.bbox.x + 1, right.bbox.y + 1, right.id.as_str()),
    ];
    let frames = body_capture_attachment_store(&pixels);
    let report = body_capture_report(catalog, objects);
    let result = body_capture_result(&report, &right, &frames);
    assert_body_capture_geometry(&result, &right, 1);
}

#[test]
fn native_capture_ruby_base_keeps_actual_body_partitions_and_annotation_order() {
    let frame = prepared_fixture();
    let catalog = agent_prepared_text_context_catalog(&frame, scope()).unwrap();
    let contexts = PlayerTextCaptureContexts::bind(&frame, &catalog, &scope()).unwrap();
    let objects = observed_fixture(&frame, &catalog);
    let ruby = objects
        .iter()
        .find(|object| {
            object
                .rich_text_ref
                .as_ref()
                .is_some_and(|element| element.kind == AgentRichTextElementKind::Ruby)
        })
        .unwrap()
        .clone();
    let cluster = objects
        .iter()
        .find(|object| {
            object
                .rich_text_ref
                .as_ref()
                .is_some_and(|element| element.kind == AgentRichTextElementKind::GlyphCluster)
        })
        .unwrap()
        .clone();
    assert_eq!(
        ruby.rich_text_ref.as_ref().unwrap().index,
        1,
        "original cleared-stage ordinal"
    );
    assert!(
        catalog
            .element_capture(&ruby)
            .and_then(|capture| capture.contains(&cluster))
            .unwrap()
    );
    assert!(
        !catalog
            .element_capture(&cluster)
            .and_then(|capture| capture.contains(&ruby))
            .unwrap()
    );
    for reversed in [false, true] {
        let mut ordered = objects.clone();
        if reversed {
            ordered.reverse();
        }
        let regions = player_capture_regions(&frame, &contexts, &ordered).unwrap();
        let ruby_index = regions
            .iter()
            .position(|region| region.id.as_str() == ruby.id)
            .unwrap();
        let cluster_index = regions
            .iter()
            .position(|region| region.id.as_str() == cluster.id)
            .unwrap();
        assert!(
            ruby_index < cluster_index,
            "annotation identity survives, base gets exact cluster identity"
        );
    }
    let annotation = ruby
        .rich_text_ref
        .as_ref()
        .unwrap()
        .ruby_annotation_bbox
        .as_ref()
        .unwrap();
    let base_point = (
        cluster.bbox.x + cluster.bbox.width / 2,
        cluster.bbox.y + cluster.bbox.height / 2,
    );
    let annotation_point = (
        annotation.x + annotation.width / 2,
        annotation.y + annotation.height / 2,
    );
    assert!(
        annotation_point.0 < cluster.bbox.x
            || annotation_point.0 >= cluster.bbox.x + cluster.bbox.width
            || annotation_point.1 < cluster.bbox.y
            || annotation_point.1 >= cluster.bbox.y + cluster.bbox.height
    );
    let frames = body_capture_attachment_store(&[
        (base_point.0, base_point.1, cluster.id.as_str()),
        (annotation_point.0, annotation_point.1, ruby.id.as_str()),
    ]);
    let report = body_capture_report(catalog, objects);
    assert_body_capture_geometry(&body_capture_result(&report, &ruby, &frames), &ruby, 2);
    assert_body_capture_geometry(
        &body_capture_result(&report, &cluster, &frames),
        &cluster,
        1,
    );
}

#[test]
fn native_capture_foreign_cluster_cannot_contribute_or_relabel_current_owner() {
    let (mut frame, owners) = mixed_owner_fixture();
    for owner in owners {
        frame.push_prepared_text_owner(owner).unwrap();
    }
    let catalog = agent_prepared_text_context_catalog(&frame, scope()).unwrap();
    let mut objects = Vec::new();
    for index in [0, 2] {
        objects.extend(observed_fixture_for_owner(&frame, &catalog, index));
    }
    let clusters = objects
        .iter()
        .filter(|object| {
            object
                .rich_text_ref
                .as_ref()
                .is_some_and(|element| element.kind == AgentRichTextElementKind::GlyphCluster)
        })
        .collect::<Vec<_>>();
    assert_eq!(clusters.len(), 2);
    let current = (*clusters[0]).clone();
    let foreign = (*clusters[1]).clone();
    assert_eq!(
        current.rich_text_ref.as_ref().unwrap().index,
        foreign.rich_text_ref.as_ref().unwrap().index
    );
    assert_eq!(
        current.rich_text_ref.as_ref().unwrap().range,
        foreign.rich_text_ref.as_ref().unwrap().range
    );
    assert!(
        !catalog
            .element_capture(&current)
            .and_then(|capture| capture.contains(&foreign))
            .unwrap()
    );
    let mut changed = foreign.clone();
    changed.text_context = current.text_context.clone();
    assert!(matches!(
        catalog
            .element_capture(&current)
            .and_then(|capture| capture.contains(&changed)),
        Err(AgentTextContextError::ObjectOwner { .. })
    ));
    let mut stripped = current.clone();
    stripped.rich_text_ref = None;
    stripped.text_context = None;
    assert!(matches!(
        catalog
            .element_capture(&current)
            .and_then(|capture| capture.contains(&stripped)),
        Err(AgentTextContextError::MissingObjectContext { .. })
    ));
    let frames = body_capture_attachment_store(&[
        (current.bbox.x + 1, current.bbox.y + 1, current.id.as_str()),
        (current.bbox.x + 2, current.bbox.y + 1, foreign.id.as_str()),
    ]);
    let report = body_capture_report(catalog, objects);
    assert_body_capture_geometry(
        &body_capture_result(&report, &current, &frames),
        &current,
        1,
    );
}

#[test]
fn native_capture_multiglyph_word_retains_its_physical_members_and_issued_cell() {
    let frame = body_capture_text_fixture("AB");
    let catalog = agent_prepared_text_context_catalog(&frame, scope()).unwrap();
    let contexts = PlayerTextCaptureContexts::bind(&frame, &catalog, &scope()).unwrap();
    let objects = observed_fixture(&frame, &catalog);
    let clusters = objects
        .iter()
        .filter(|object| {
            object
                .rich_text_ref
                .as_ref()
                .is_some_and(|element| element.kind == AgentRichTextElementKind::GlyphCluster)
        })
        .collect::<Vec<_>>();
    assert_eq!(clusters.len(), 1);
    let cluster = (*clusters[0]).clone();
    let physical = objects
        .iter()
        .filter(|object| {
            object
                .rich_text_ref
                .as_ref()
                .is_some_and(|element| element.kind == AgentRichTextElementKind::TextGlyph)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        physical.len(),
        2,
        "the real shaper retains both Latin raster glyphs"
    );
    let first = (*physical[0]).clone();
    assert_eq!(
        first.bbox, cluster.bbox,
        "sideways word members use their issued atomic cell"
    );
    let entry = catalog
        .resolve(cluster.text_context.as_ref().unwrap())
        .unwrap();
    let element = cluster.rich_text_ref.as_ref().unwrap();
    let body = entry.capture_body(element).unwrap();
    let members = entry
        .content
        .layout()
        .cluster_glyphs(u32::try_from(element.index).unwrap())
        .unwrap();
    assert_eq!(members.len(), 2);
    let selection = player_prepared_text_capture_selection(&contexts, &cluster)
        .unwrap()
        .unwrap();
    assert_eq!(selection.glyph_indices().len(), 2);
    for index in selection.glyph_indices() {
        let item = frame.text.get(selection.text()).unwrap();
        let PreparedGlyphSource::Body { glyph_index } =
            item.glyphs[usize::try_from(*index).unwrap()].source
        else {
            panic!("word capture contains only its body glyphs");
        };
        assert!(body.contains_glyph(glyph_index));
    }
    assert!(
        catalog
            .element_capture(&first)
            .and_then(|capture| capture.contains(&cluster))
            .unwrap()
    );
    let frames = body_capture_attachment_store(&[
        (cluster.bbox.x + 1, cluster.bbox.y + 1, cluster.id.as_str()),
        (cluster.bbox.x + 2, cluster.bbox.y + 1, cluster.id.as_str()),
    ]);
    let report = body_capture_report(catalog, objects);
    assert_body_capture_geometry(
        &body_capture_result(&report, &cluster, &frames),
        &cluster,
        2,
    );
    assert_body_capture_geometry(&body_capture_result(&report, &first, &frames), &first, 2);
}
