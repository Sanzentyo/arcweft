//! Agent objects projected from the same prepared layout used for rendering.

mod view;

pub(super) use view::agent_view_prepared_text_objects;

use super::image_mapping::{
    agent_ceil_viewport_f32, agent_floor_viewport_f32, agent_object_capture_refs_with_source,
    agent_union_bbox, agent_uri_component,
};
use super::*;
use arcweft_agent_protocol::rich_text::{AgentGlyphOrientation, AgentGlyphVerticalForm};
use arcweft_glyphon::PreparedTextItem;
use arcweft_presentation::hit::HitRect;
use arcweft_render_text::{
    RichTextControl, RichTextNode, RichTextObjectProxy, RichTextPresentation,
    RichTextRubyAnnotation, RichTextTextRun, RichTextTextSource,
};
use arcweft_render_wgpu::geometry::{PreparedFrame, PreparedTextOwner, PreparedTextOwnerKind};
use arcweft_text_layout::{GlyphOrientation, GlyphVerticalForm, LayoutRect, TextLayoutGlyph};

pub(super) fn agent_dialogue_prepared_text_objects(
    capture_step: usize,
    textbox: usize,
    entry: usize,
    frame: LineDisplayFrame,
    prepared: &PreparedFrame,
    viewport: &AgentViewport,
) -> Result<Vec<AgentObservedObject>, ExitCode> {
    let Some(owner) = prepared.prepared_text_owners().iter().find(|owner| {
        matches!(
            owner.kind,
            PreparedTextOwnerKind::TextBox {
                textbox: owner_textbox,
                entry: owner_entry,
                part: arcweft_render_wgpu::geometry::PreparedTextBoxPart::Body,
                ..
            } if owner_textbox == u64::try_from(textbox).unwrap_or(u64::MAX)
                && owner_entry == u64::try_from(entry).unwrap_or(u64::MAX)
        )
    }) else {
        eprintln!("error: dialogue frame is missing its prepared-text owner");
        return Err(ExitCode::FAILURE);
    };
    let Some(item) = prepared.prepared_text.get(owner.text) else {
        eprintln!(
            "error: dialogue prepared-text owner references missing item {}",
            owner.text.index()
        );
        return Err(ExitCode::FAILURE);
    };
    let textbox_object =
        dialogue_textbox_object(capture_step, textbox, entry, frame, owner, viewport)?;
    let mut objects = vec![textbox_object.clone()];
    objects.extend(dialogue_children(
        capture_step,
        textbox,
        entry,
        &textbox_object,
        owner,
        item,
        viewport,
    ));
    repair_child_parents(&textbox_object, &mut objects[1..]);
    Ok(objects)
}

fn dialogue_textbox_object(
    capture_step: usize,
    textbox: usize,
    entry: usize,
    frame: LineDisplayFrame,
    owner: &PreparedTextOwner,
    viewport: &AgentViewport,
) -> Result<AgentObservedObject, ExitCode> {
    let bbox = agent_bbox_from_hit_rect(owner.object_bounds, viewport).ok_or_else(|| {
        eprintln!("error: dialogue prepared-text owner has empty viewport geometry");
        ExitCode::FAILURE
    })?;
    let object_id = format!("object.dialogue.{textbox}.{entry}");
    let source = AgentCaptureSourceIdentity::Object {
        id: object_id.clone(),
        parent_id: None,
        entity: Some(frame.callee.clone()),
        layer: "dialogue".to_owned(),
        role: AGENT_ROLE_DIALOGUE_TEXTBOX.to_owned(),
        object_layer: None,
        object_depth: None,
        rich_text: None,
    };
    Ok(AgentObservedObject {
        id: object_id.clone(),
        parent_id: None,
        entity: Some(frame.callee.clone()),
        layer: "dialogue".to_owned(),
        role: AGENT_ROLE_DIALOGUE_TEXTBOX.to_owned(),
        visible: true,
        enabled: true,
        bbox: bbox.clone(),
        polygon: bbox.polygon(),
        capture_refs: agent_object_capture_refs_with_source(
            "cli",
            capture_step,
            &object_id,
            &bbox,
            0,
            source,
        ),
        object_layer: None,
        object_depth: None,
        text: Some(frame.text.clone()),
        rich_text_ref: None,
        content: AgentObservedObjectContent::RichText {
            frame: Box::new(frame),
        },
    })
}

fn dialogue_children(
    capture_step: usize,
    textbox_id: usize,
    entry: usize,
    textbox_object: &AgentObservedObject,
    owner: &PreparedTextOwner,
    item: &PreparedTextItem,
    viewport: &AgentViewport,
) -> Vec<AgentObservedObject> {
    let Some(frame) = textbox_object.rich_text_frame() else {
        return Vec::new();
    };
    let context = DialogueProjection {
        capture_step,
        textbox_id,
        entry,
        textbox_object,
        frame,
        owner,
        item,
        viewport,
        run_geometry: dialogue_run_geometry(frame, owner, item, viewport),
    };
    let mut children = dialogue_page_objects(&context);
    children.extend(dialogue_line_objects(&context));
    children.extend(dialogue_run_objects(&context));
    children.extend(dialogue_ruby_objects(&context));
    children.extend(dialogue_glyph_objects(&context));
    dedupe_objects(children)
}

struct DialogueProjection<'a> {
    capture_step: usize,
    textbox_id: usize,
    entry: usize,
    textbox_object: &'a AgentObservedObject,
    frame: &'a LineDisplayFrame,
    owner: &'a PreparedTextOwner,
    item: &'a PreparedTextItem,
    viewport: &'a AgentViewport,
    run_geometry: Vec<(usize, &'a RichTextTextRun, RichTextRange, AgentBBox)>,
}

fn dialogue_page_objects(context: &DialogueProjection<'_>) -> Vec<AgentObservedObject> {
    let visible_range = RichTextRange::new(
        context.owner.source_origin,
        context
            .owner
            .source_origin
            .saturating_add(context.item.interaction.text.len())
            .min(context.frame.text.len()),
    );
    let page = rich_text_page_for_range(context.frame, visible_range);
    let Some(text) = text_for_range(context.frame, visible_range) else {
        return Vec::new();
    };
    if text.trim().is_empty() {
        return Vec::new();
    }
    let mut hit_regions = vec![agent_hit_region(
        AgentHitRegionKind::TextPage,
        &context.textbox_object.bbox,
        visible_range,
    )];
    hit_regions.extend(proxy_hit_regions_for_range(
        visible_range,
        &context.run_geometry,
    ));
    let id = page_object_id(context.textbox_id, context.entry, page);
    vec![dialogue_child_object(
        context.capture_step,
        context.textbox_object,
        DialogueChildSpec {
            id: &id,
            parent_id: Some(context.textbox_object.id.clone()),
            role: "rich_text_page",
            text: text.to_owned(),
            bbox: &context.textbox_object.bbox,
            reference: AgentRichTextElementRef {
                kind: AgentRichTextElementKind::TextPage,
                index: page,
                page,
                range: visible_range,
                node_index: range_node_index(context.frame, visible_range),
                source: None,
                ruby: None,
                presentation: presentation_for_range(context.frame, visible_range),
                orientation: None,
                vertical_form: None,
                ruby_base_bbox: None,
                ruby_annotation_bbox: None,
                object_layer: object_layer_for_range(context.frame, visible_range),
                object_depth: object_depth_for_range(context.frame, visible_range),
                hit_test: true,
                hit_regions,
            },
            page,
        },
    )]
}

fn dialogue_line_objects(context: &DialogueProjection<'_>) -> Vec<AgentObservedObject> {
    let mut children = Vec::new();
    for (line_index, line) in context.item.layout.lines.iter().enumerate() {
        let Some(range) = global_range(context.owner, line.source_range, context.frame.text.len())
        else {
            continue;
        };
        let Some(text) =
            text_for_range(context.frame, range).filter(|text| !text.trim().is_empty())
        else {
            continue;
        };
        let Some(bbox) = agent_bbox_from_layout(line.bounds, context.viewport) else {
            continue;
        };
        let page = rich_text_page_for_range(context.frame, range);
        let id = line_object_id(context.textbox_id, context.entry, line_index);
        children.push(dialogue_child_object(
            context.capture_step,
            context.textbox_object,
            DialogueChildSpec {
                id: &id,
                parent_id: Some(page_object_id(context.textbox_id, context.entry, page)),
                role: "rich_text_line",
                text: text.to_owned(),
                bbox: &bbox,
                reference: AgentRichTextElementRef {
                    kind: AgentRichTextElementKind::TextLine,
                    index: line_index,
                    page,
                    range,
                    node_index: range_node_index(context.frame, range),
                    source: None,
                    ruby: None,
                    presentation: presentation_for_range(context.frame, range),
                    orientation: None,
                    vertical_form: None,
                    ruby_base_bbox: None,
                    ruby_annotation_bbox: None,
                    object_layer: object_layer_for_range(context.frame, range),
                    object_depth: object_depth_for_range(context.frame, range),
                    hit_test: true,
                    hit_regions: vec![agent_hit_region(AgentHitRegionKind::TextLine, &bbox, range)],
                },
                page,
            },
        ));
    }
    children
}

fn dialogue_run_objects(context: &DialogueProjection<'_>) -> Vec<AgentObservedObject> {
    let mut children = Vec::new();
    for (original_index, run, range, bbox) in &context.run_geometry {
        let Some(text) =
            text_for_range(context.frame, *range).filter(|text| !text.trim().is_empty())
        else {
            continue;
        };
        if matches!(
            run.source,
            RichTextTextSource::ControlHardBreak | RichTextTextSource::ControlRaw
        ) {
            continue;
        }
        let page = rich_text_page_for_range(context.frame, *range);
        let id = run_object_id(context.textbox_id, context.entry, *original_index);
        let parent_id = layout_line_for_range(context.owner, context.item, *range).map_or_else(
            || page_object_id(context.textbox_id, context.entry, page),
            |line| line_object_id(context.textbox_id, context.entry, line),
        );
        children.push(dialogue_child_object(
            context.capture_step,
            context.textbox_object,
            DialogueChildSpec {
                id: &id,
                parent_id: Some(parent_id),
                role: "rich_text_run",
                text: text.to_owned(),
                bbox,
                reference: run_reference(
                    AgentRichTextElementKind::TextRun,
                    *original_index,
                    page,
                    *range,
                    run,
                    bbox,
                    AgentHitRegionKind::TextRun,
                ),
                page,
            },
        ));
        children.extend(dialogue_proxy_objects(
            context,
            &DialogueRunProjection {
                id: &id,
                original_index: *original_index,
                run,
                range: *range,
                bbox,
                page,
                text,
            },
        ));
    }
    children
}

struct DialogueRunProjection<'a> {
    id: &'a str,
    original_index: usize,
    run: &'a RichTextTextRun,
    range: RichTextRange,
    bbox: &'a AgentBBox,
    page: usize,
    text: &'a str,
}

fn dialogue_proxy_objects(
    context: &DialogueProjection<'_>,
    projection: &DialogueRunProjection<'_>,
) -> Vec<AgentObservedObject> {
    projection
        .run
        .presentation
        .object_proxies
        .iter()
        .enumerate()
        .map(|(proxy_index, proxy)| {
            let proxy_id = format!(
                "object.dialogue.{}.{}.proxy.{}.{}",
                context.textbox_id, context.entry, projection.original_index, proxy_index
            );
            let presentation = proxy_presentation(&projection.run.presentation, proxy);
            dialogue_child_object(
                context.capture_step,
                context.textbox_object,
                DialogueChildSpec {
                    id: &proxy_id,
                    parent_id: Some(projection.id.to_owned()),
                    role: "rich_text_proxy",
                    text: projection.text.to_owned(),
                    bbox: projection.bbox,
                    reference: AgentRichTextElementRef {
                        kind: AgentRichTextElementKind::TextObjectProxy,
                        index: proxy_index,
                        page: projection.page,
                        range: projection.range,
                        node_index: projection.run.node_index,
                        source: Some(projection.run.source),
                        ruby: None,
                        presentation: Some(presentation),
                        orientation: None,
                        vertical_form: None,
                        ruby_base_bbox: None,
                        ruby_annotation_bbox: None,
                        object_layer: proxy
                            .layer
                            .clone()
                            .or_else(|| projection.run.presentation.layer.clone()),
                        object_depth: proxy.depth.map(|depth| depth.0).or_else(|| {
                            (projection.run.presentation.z_index != 0)
                                .then_some(i32::from(projection.run.presentation.z_index) * 1_000)
                        }),
                        hit_test: proxy.hit_test,
                        hit_regions: proxy_hit_regions(
                            projection.bbox,
                            projection.range,
                            &projection.run.presentation,
                            proxy,
                        ),
                    },
                    page: projection.page,
                },
            )
        })
        .collect()
}

fn dialogue_ruby_objects(context: &DialogueProjection<'_>) -> Vec<AgentObservedObject> {
    let mut children = Vec::new();
    for layout_ruby in &context.item.layout.ruby {
        let Some(range) = global_range(
            context.owner,
            layout_ruby.base_range,
            context.frame.text.len(),
        ) else {
            continue;
        };
        let Some((ruby_index, ruby)) = find_ruby(context.frame, range, &layout_ruby.text) else {
            continue;
        };
        let Some(base_bbox) = agent_bbox_from_layout(layout_ruby.base_bounds, context.viewport)
        else {
            continue;
        };
        let Some(annotation_bbox) =
            agent_bbox_from_layout(layout_ruby.ruby_bounds, context.viewport)
        else {
            continue;
        };
        let bbox = agent_union_bbox(&base_bbox, &annotation_bbox);
        let page = rich_text_page_for_range(context.frame, range);
        let id = format!(
            "object.dialogue.{}.{}.ruby.{ruby_index}",
            context.textbox_id, context.entry
        );
        let parent_id = layout_line_for_range(context.owner, context.item, range).map_or_else(
            || page_object_id(context.textbox_id, context.entry, page),
            |line| line_object_id(context.textbox_id, context.entry, line),
        );
        let base_text = text_for_range(context.frame, range).unwrap_or_default();
        children.push(dialogue_child_object(
            context.capture_step,
            context.textbox_object,
            DialogueChildSpec {
                id: &id,
                parent_id: Some(parent_id),
                role: "rich_text_ruby",
                text: format!("{base_text} ({})", ruby.ruby),
                bbox: &bbox,
                reference: AgentRichTextElementRef {
                    kind: AgentRichTextElementKind::Ruby,
                    index: ruby_index,
                    page,
                    range,
                    node_index: ruby.node_index,
                    source: None,
                    ruby: Some(ruby.ruby.clone()),
                    presentation: Some(ruby.presentation.clone()),
                    orientation: None,
                    vertical_form: None,
                    ruby_base_bbox: Some(base_bbox.clone()),
                    ruby_annotation_bbox: Some(annotation_bbox.clone()),
                    object_layer: object_layer(&ruby.presentation),
                    object_depth: object_depth(&ruby.presentation),
                    hit_test: presentation_has_hit_test_proxy(&ruby.presentation),
                    hit_regions: vec![
                        agent_hit_region(AgentHitRegionKind::RubyObject, &bbox, range),
                        agent_hit_region(AgentHitRegionKind::RubyBase, &base_bbox, range),
                        agent_hit_region(
                            AgentHitRegionKind::RubyAnnotation,
                            &annotation_bbox,
                            range,
                        ),
                    ],
                },
                page,
            },
        ));
    }
    children
}

fn dialogue_glyph_objects(context: &DialogueProjection<'_>) -> Vec<AgentObservedObject> {
    let mut children = Vec::new();
    for (glyph_index, glyph) in context.item.layout.glyphs.iter().enumerate() {
        let Some(range) = global_range(context.owner, glyph.source_range, context.frame.text.len())
        else {
            continue;
        };
        let Some((run_index, run)) = find_run(context.frame, range) else {
            continue;
        };
        let Some(text) =
            text_for_range(context.frame, range).filter(|text| !text.trim().is_empty())
        else {
            continue;
        };
        let Some(bbox) = agent_bbox_from_layout(glyph.layout_bounds, context.viewport) else {
            continue;
        };
        let page = rich_text_page_for_range(context.frame, range);
        let parent_id = run_object_id(context.textbox_id, context.entry, run_index);
        let glyph_id = format!(
            "object.dialogue.{}.{}.glyph.{glyph_index}.{}.{}",
            context.textbox_id, context.entry, range.start, range.end
        );
        children.push(dialogue_child_object(
            context.capture_step,
            context.textbox_object,
            DialogueChildSpec {
                id: &glyph_id,
                parent_id: Some(parent_id.clone()),
                role: "rich_text_glyph",
                text: text.to_owned(),
                bbox: &bbox,
                reference: glyph_reference(
                    AgentRichTextElementKind::TextGlyph,
                    glyph_index,
                    page,
                    range,
                    run,
                    glyph,
                    &bbox,
                    AgentHitRegionKind::TextGlyph,
                ),
                page,
            },
        ));
        let cluster_index = usize::try_from(glyph.cluster_index).unwrap_or(usize::MAX);
        let cluster_id = format!(
            "object.dialogue.{}.{}.cluster.{cluster_index}.{}.{}",
            context.textbox_id, context.entry, range.start, range.end
        );
        children.push(dialogue_child_object(
            context.capture_step,
            context.textbox_object,
            DialogueChildSpec {
                id: &cluster_id,
                parent_id: Some(parent_id),
                role: "rich_text_cluster",
                text: text.to_owned(),
                bbox: &bbox,
                reference: glyph_reference(
                    AgentRichTextElementKind::GlyphCluster,
                    cluster_index,
                    page,
                    range,
                    run,
                    glyph,
                    &bbox,
                    AgentHitRegionKind::GlyphCluster,
                ),
                page,
            },
        ));
    }
    children
}

fn dialogue_run_geometry<'a>(
    frame: &'a LineDisplayFrame,
    owner: &PreparedTextOwner,
    item: &PreparedTextItem,
    viewport: &AgentViewport,
) -> Vec<(usize, &'a RichTextTextRun, RichTextRange, AgentBBox)> {
    let mut by_run = BTreeMap::<usize, (RichTextRange, AgentBBox)>::new();
    for run in &item.layout.runs {
        let Some(range) = global_range(owner, run.source_range, frame.text.len()) else {
            continue;
        };
        let Some((index, _)) = find_run(frame, range) else {
            continue;
        };
        let Some(bbox) = agent_bbox_from_layout(run.bounds, viewport) else {
            continue;
        };
        by_run
            .entry(index)
            .and_modify(|(existing_range, existing_bbox)| {
                existing_range.start = existing_range.start.min(range.start);
                existing_range.end = existing_range.end.max(range.end);
                *existing_bbox = agent_union_bbox(existing_bbox, &bbox);
            })
            .or_insert((range, bbox));
    }
    by_run
        .into_iter()
        .filter_map(|(index, (range, bbox))| {
            frame
                .display_map
                .text_runs
                .get(index)
                .map(|run| (index, run, range, bbox))
        })
        .collect()
}

fn run_reference(
    kind: AgentRichTextElementKind,
    index: usize,
    page: usize,
    range: RichTextRange,
    run: &RichTextTextRun,
    bbox: &AgentBBox,
    hit_kind: AgentHitRegionKind,
) -> AgentRichTextElementRef {
    AgentRichTextElementRef {
        kind,
        index,
        page,
        range,
        node_index: run.node_index,
        source: Some(run.source),
        ruby: None,
        presentation: Some(run.presentation.clone()),
        orientation: None,
        vertical_form: None,
        ruby_base_bbox: None,
        ruby_annotation_bbox: None,
        object_layer: object_layer(&run.presentation),
        object_depth: object_depth(&run.presentation),
        hit_test: presentation_has_hit_test_proxy(&run.presentation),
        hit_regions: text_hit_regions(hit_kind, bbox, range, &run.presentation),
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "protocol glyph reference carries exact typed evidence"
)]
fn glyph_reference(
    kind: AgentRichTextElementKind,
    index: usize,
    page: usize,
    range: RichTextRange,
    run: &RichTextTextRun,
    glyph: &TextLayoutGlyph,
    bbox: &AgentBBox,
    hit_kind: AgentHitRegionKind,
) -> AgentRichTextElementRef {
    AgentRichTextElementRef {
        kind,
        index,
        page,
        range,
        node_index: run.node_index,
        source: Some(run.source),
        ruby: None,
        presentation: Some(run.presentation.clone()),
        orientation: Some(agent_glyph_orientation(glyph.orientation)),
        vertical_form: Some(agent_glyph_vertical_form(glyph.vertical_form)),
        ruby_base_bbox: None,
        ruby_annotation_bbox: None,
        object_layer: object_layer(&run.presentation),
        object_depth: object_depth(&run.presentation),
        hit_test: presentation_has_hit_test_proxy(&run.presentation),
        hit_regions: text_hit_regions(hit_kind, bbox, range, &run.presentation),
    }
}

struct DialogueChildSpec<'a> {
    id: &'a str,
    parent_id: Option<String>,
    role: &'a str,
    text: String,
    bbox: &'a AgentBBox,
    reference: AgentRichTextElementRef,
    page: usize,
}

fn dialogue_child_object(
    step: usize,
    textbox: &AgentObservedObject,
    spec: DialogueChildSpec<'_>,
) -> AgentObservedObject {
    let source = AgentCaptureSourceIdentity::Object {
        id: spec.id.to_owned(),
        parent_id: spec.parent_id.clone().or_else(|| Some(textbox.id.clone())),
        entity: textbox.entity.clone(),
        layer: "dialogue.rich_text".to_owned(),
        role: spec.role.to_owned(),
        object_layer: spec.reference.object_layer.clone(),
        object_depth: spec.reference.object_depth,
        rich_text: Some((&spec.reference).into()),
    };
    AgentObservedObject {
        id: spec.id.to_owned(),
        parent_id: spec.parent_id.or_else(|| Some(textbox.id.clone())),
        entity: textbox.entity.clone(),
        layer: "dialogue.rich_text".to_owned(),
        role: spec.role.to_owned(),
        visible: textbox.visible,
        enabled: textbox.enabled,
        bbox: spec.bbox.clone(),
        polygon: spec.bbox.polygon(),
        capture_refs: agent_object_capture_refs_with_source(
            "cli", step, spec.id, spec.bbox, spec.page, source,
        ),
        object_layer: spec.reference.object_layer.clone(),
        object_depth: spec.reference.object_depth,
        text: Some(spec.text.clone()),
        rich_text_ref: Some(spec.reference),
        content: AgentObservedObjectContent::RichText {
            frame: Box::new(child_frame(
                textbox
                    .rich_text_frame()
                    .expect("dialogue child keeps its parent frame"),
                spec.text,
            )),
        },
    }
}

fn child_frame(parent: &LineDisplayFrame, text: String) -> LineDisplayFrame {
    LineDisplayFrame {
        line: parent.line.clone(),
        callee: parent.callee.clone(),
        speaker_label: parent.speaker_label.clone(),
        text: text.clone(),
        base_styles: parent.base_styles.clone(),
        default_inline_failure_policy: parent.default_inline_failure_policy.clone(),
        style_contributions: parent.style_contributions.clone(),
        nodes: vec![RichTextNode::Text { text }],
        display_map: arcweft_render_text::RichTextDisplayMap::default(),
        host_events: Vec::new(),
        inline_failures: Vec::new(),
        unresolved: Vec::new(),
    }
}

fn repair_child_parents(textbox: &AgentObservedObject, children: &mut [AgentObservedObject]) {
    let mut ids = children
        .iter()
        .map(|child| child.id.clone())
        .collect::<BTreeSet<_>>();
    ids.insert(textbox.id.clone());
    for child in children {
        if !child
            .parent_id
            .as_ref()
            .is_some_and(|parent| ids.contains(parent))
        {
            child.parent_id = Some(textbox.id.clone());
        }
    }
}

fn dedupe_objects(objects: Vec<AgentObservedObject>) -> Vec<AgentObservedObject> {
    let mut ids = BTreeSet::new();
    objects
        .into_iter()
        .filter(|object| ids.insert(object.id.clone()))
        .collect()
}

fn global_range(
    owner: &PreparedTextOwner,
    range: RichTextRange,
    text_len: usize,
) -> Option<RichTextRange> {
    let range = global_range_unbounded(owner, range)?;
    valid_range(range, text_len).map(|range| RichTextRange::new(range.start, range.end))
}

fn global_range_unbounded(
    owner: &PreparedTextOwner,
    range: RichTextRange,
) -> Option<RichTextRange> {
    Some(RichTextRange::new(
        owner.source_origin.checked_add(range.start)?,
        owner.source_origin.checked_add(range.end)?,
    ))
}

fn valid_range(range: RichTextRange, text_len: usize) -> Option<std::ops::Range<usize>> {
    (range.start <= range.end && range.end <= text_len).then_some(range.start..range.end)
}

fn text_for_range(frame: &LineDisplayFrame, range: RichTextRange) -> Option<&str> {
    frame.text.get(valid_range(range, frame.text.len())?)
}

fn find_run(frame: &LineDisplayFrame, range: RichTextRange) -> Option<(usize, &RichTextTextRun)> {
    frame
        .display_map
        .text_runs
        .iter()
        .enumerate()
        .find(|(_, run)| range.start >= run.range.start && range.end <= run.range.end)
        .or_else(|| {
            frame
                .display_map
                .text_runs
                .iter()
                .enumerate()
                .find(|(_, run)| ranges_overlap(run.range, range))
        })
}

fn find_ruby<'a>(
    frame: &'a LineDisplayFrame,
    range: RichTextRange,
    text: &str,
) -> Option<(usize, &'a RichTextRubyAnnotation)> {
    frame
        .display_map
        .ruby_annotations
        .iter()
        .enumerate()
        .find(|(_, ruby)| ruby.base_range == range && ruby.ruby == text)
        .or_else(|| {
            frame
                .display_map
                .ruby_annotations
                .iter()
                .enumerate()
                .find(|(_, ruby)| ranges_overlap(ruby.base_range, range))
        })
}

fn layout_line_for_range(
    owner: &PreparedTextOwner,
    item: &PreparedTextItem,
    range: RichTextRange,
) -> Option<usize> {
    item.layout.lines.iter().position(|line| {
        global_range_unbounded(owner, line.source_range)
            .is_some_and(|line_range| ranges_overlap(line_range, range))
    })
}

fn ranges_overlap(left: RichTextRange, right: RichTextRange) -> bool {
    left.start < right.end && right.start < left.end
}

fn rich_text_page_for_range(frame: &LineDisplayFrame, range: RichTextRange) -> usize {
    let Some(range) = valid_range(range, frame.text.len()) else {
        return 0;
    };
    page_ranges(frame)
        .into_iter()
        .filter(|page| !page.is_empty())
        .position(|page| range.start >= page.start && range.end <= page.end)
        .unwrap_or(0)
}

fn page_ranges(frame: &LineDisplayFrame) -> Vec<std::ops::Range<usize>> {
    let mut breaks = frame
        .display_map
        .controls
        .iter()
        .filter(|marker| {
            matches!(
                marker.control,
                RichTextControl::Page | RichTextControl::LineWait | RichTextControl::Clear
            )
        })
        .map(|marker| offset_before_node(frame, marker.node_index))
        .map(|offset| offset_after_ruby_base(frame, offset))
        .filter(|offset| *offset <= frame.text.len() && frame.text.is_char_boundary(*offset))
        .collect::<Vec<_>>();
    breaks.sort_unstable();
    breaks.dedup();
    let mut start = 0;
    let mut ranges = Vec::with_capacity(breaks.len().saturating_add(1));
    for end in breaks {
        if start <= end {
            ranges.push(start..end);
            start = end;
        }
    }
    ranges.push(start..frame.text.len());
    ranges
}

fn offset_after_ruby_base(frame: &LineDisplayFrame, offset: usize) -> usize {
    let mut adjusted = offset;
    loop {
        let Some(range) = frame
            .display_map
            .ruby_annotations
            .iter()
            .filter_map(|ruby| valid_range(ruby.base_range, frame.text.len()))
            .find(|range| range.start < adjusted && adjusted < range.end)
        else {
            return adjusted;
        };
        adjusted = range.end;
    }
}

fn offset_before_node(frame: &LineDisplayFrame, node_index: usize) -> usize {
    frame
        .display_map
        .text_runs
        .iter()
        .filter(|run| run.node_index < node_index)
        .map(|run| run.range.end)
        .max()
        .unwrap_or(0)
}

fn page_object_id(textbox: usize, entry: usize, page: usize) -> String {
    format!("object.dialogue.{textbox}.{entry}.page.{page}")
}

fn line_object_id(textbox: usize, entry: usize, line: usize) -> String {
    format!("object.dialogue.{textbox}.{entry}.line.{line}")
}

fn run_object_id(textbox: usize, entry: usize, run: usize) -> String {
    format!("object.dialogue.{textbox}.{entry}.run.{run}")
}

fn range_node_index(frame: &LineDisplayFrame, range: RichTextRange) -> usize {
    frame
        .display_map
        .text_runs
        .iter()
        .find(|run| ranges_overlap(run.range, range))
        .map_or(0, |run| run.node_index)
}

fn presentation_for_range(
    frame: &LineDisplayFrame,
    range: RichTextRange,
) -> Option<RichTextPresentation> {
    frame
        .display_map
        .text_runs
        .iter()
        .filter(|run| ranges_overlap(run.range, range))
        .map(|run| run.presentation.clone())
        .reduce(|mut accumulated, presentation| {
            accumulated.merge(presentation);
            accumulated
        })
}

fn object_depth_for_range(frame: &LineDisplayFrame, range: RichTextRange) -> Option<i32> {
    frame
        .display_map
        .text_runs
        .iter()
        .filter(|run| ranges_overlap(run.range, range))
        .filter_map(|run| object_depth(&run.presentation))
        .max()
}

fn object_layer_for_range(frame: &LineDisplayFrame, range: RichTextRange) -> Option<String> {
    frame
        .display_map
        .text_runs
        .iter()
        .filter(|run| ranges_overlap(run.range, range))
        .filter_map(|run| {
            object_layer(&run.presentation)
                .map(|layer| (object_depth(&run.presentation).unwrap_or(0), layer))
        })
        .max_by_key(|(depth, _)| *depth)
        .map(|(_, layer)| layer)
}

fn proxy_hit_regions_for_range(
    range: RichTextRange,
    run_geometry: &[(usize, &RichTextTextRun, RichTextRange, AgentBBox)],
) -> Vec<AgentHitRegion> {
    run_geometry
        .iter()
        .filter(|(_, _, run_range, _)| ranges_overlap(*run_range, range))
        .flat_map(|(_, run, run_range, bbox)| {
            let hit_range = RichTextRange::new(
                run_range.start.max(range.start),
                run_range.end.min(range.end),
            );
            run.presentation
                .object_proxies
                .iter()
                .filter(|proxy| proxy.hit_test)
                .map(move |proxy| proxy_hit_region(bbox, hit_range, &run.presentation, proxy))
        })
        .collect()
}

fn agent_hit_region(
    kind: AgentHitRegionKind,
    bbox: &AgentBBox,
    range: RichTextRange,
) -> AgentHitRegion {
    AgentHitRegion {
        kind,
        bbox: bbox.clone(),
        range,
        proxy_id: None,
        proxy_type: None,
        proxy_declaration: None,
        proxy_role: None,
        proxy_layer: None,
        depth: None,
        proxy_params: BTreeMap::new(),
    }
}

fn text_hit_regions(
    kind: AgentHitRegionKind,
    bbox: &AgentBBox,
    range: RichTextRange,
    presentation: &RichTextPresentation,
) -> Vec<AgentHitRegion> {
    let mut regions = vec![agent_hit_region(kind, bbox, range)];
    regions.extend(
        presentation
            .object_proxies
            .iter()
            .filter(|proxy| proxy.hit_test)
            .map(|proxy| proxy_hit_region(bbox, range, presentation, proxy)),
    );
    regions
}

fn proxy_presentation(
    presentation: &RichTextPresentation,
    proxy: &RichTextObjectProxy,
) -> RichTextPresentation {
    let mut proxy_presentation = presentation.clone();
    proxy_presentation.object_proxies = vec![proxy.clone()];
    proxy_presentation
}

fn proxy_hit_regions(
    bbox: &AgentBBox,
    range: RichTextRange,
    presentation: &RichTextPresentation,
    proxy: &RichTextObjectProxy,
) -> Vec<AgentHitRegion> {
    proxy
        .hit_test
        .then(|| proxy_hit_region(bbox, range, presentation, proxy))
        .into_iter()
        .collect()
}

fn proxy_hit_region(
    bbox: &AgentBBox,
    range: RichTextRange,
    presentation: &RichTextPresentation,
    proxy: &RichTextObjectProxy,
) -> AgentHitRegion {
    AgentHitRegion {
        kind: AgentHitRegionKind::TextObjectProxy,
        bbox: bbox.clone(),
        range,
        proxy_id: Some(proxy.id.clone()),
        proxy_type: proxy.type_name.clone(),
        proxy_declaration: proxy.declaration.clone(),
        proxy_role: proxy.role.clone(),
        proxy_layer: proxy.layer.clone().or_else(|| presentation.layer.clone()),
        depth: proxy.depth.map(|depth| depth.0),
        proxy_params: proxy.params.clone(),
    }
}

fn object_layer(presentation: &RichTextPresentation) -> Option<String> {
    presentation
        .object_proxies
        .iter()
        .filter_map(|proxy| {
            proxy
                .layer
                .as_ref()
                .map(|layer| (proxy.depth.map_or(0, |depth| depth.0), layer))
        })
        .max_by_key(|(depth, _)| *depth)
        .map(|(_, layer)| layer.clone())
        .or_else(|| presentation.layer.clone())
}

fn object_depth(presentation: &RichTextPresentation) -> Option<i32> {
    presentation
        .object_proxies
        .iter()
        .filter_map(|proxy| proxy.depth.map(|depth| depth.0))
        .max()
        .or_else(|| (presentation.z_index != 0).then_some(i32::from(presentation.z_index) * 1_000))
}

fn presentation_has_hit_test_proxy(presentation: &RichTextPresentation) -> bool {
    presentation
        .object_proxies
        .iter()
        .any(|proxy| proxy.hit_test)
}

fn agent_glyph_orientation(value: GlyphOrientation) -> AgentGlyphOrientation {
    match value {
        GlyphOrientation::Upright => AgentGlyphOrientation::Upright,
        GlyphOrientation::SidewaysCw => AgentGlyphOrientation::SidewaysCw,
        GlyphOrientation::TextCombineUpright => AgentGlyphOrientation::TextCombineUpright,
    }
}

fn agent_glyph_vertical_form(value: GlyphVerticalForm) -> AgentGlyphVerticalForm {
    match value {
        GlyphVerticalForm::None => AgentGlyphVerticalForm::None,
        GlyphVerticalForm::UprightAlternate => AgentGlyphVerticalForm::UprightAlternate,
        GlyphVerticalForm::RotatedAlternate => AgentGlyphVerticalForm::RotatedAlternate,
    }
}

fn agent_bbox_from_hit_rect(rect: HitRect, viewport: &AgentViewport) -> Option<AgentBBox> {
    agent_bbox_from_layout(
        LayoutRect::new(rect.x, rect.y, rect.width, rect.height),
        viewport,
    )
}

fn agent_bbox_from_layout(rect: LayoutRect, viewport: &AgentViewport) -> Option<AgentBBox> {
    if !rect.x.is_finite()
        || !rect.y.is_finite()
        || !rect.width.is_finite()
        || !rect.height.is_finite()
        || rect.width <= 0.0
        || rect.height <= 0.0
    {
        return None;
    }
    let x = agent_floor_viewport_f32(rect.x, viewport.width);
    let y = agent_floor_viewport_f32(rect.y, viewport.height);
    let right = agent_ceil_viewport_f32(rect.x + rect.width, viewport.width);
    let bottom = agent_ceil_viewport_f32(rect.y + rect.height, viewport.height);
    Some(AgentBBox {
        space: AgentCoordinateSpace::Viewport,
        x,
        y,
        width: right.saturating_sub(x).max(1),
        height: bottom.saturating_sub(y).max(1),
    })
}
