//! Agent geometry for prepared text owned by authored or Rust-backed Views.

use super::*;

pub(in super::super) fn agent_view_prepared_text_objects(
    step: usize,
    prepared: &PreparedFrame,
    viewport: &AgentViewport,
) -> Vec<AgentObservedObject> {
    prepared
        .prepared_text_owners()
        .iter()
        .filter(|owner| matches!(owner.kind, PreparedTextOwnerKind::View { .. }))
        .filter_map(|owner| {
            prepared
                .text
                .get(owner.text)
                .and_then(|item| view_text_objects(step, owner, item, viewport))
        })
        .flatten()
        .collect()
}

fn view_text_objects(
    step: usize,
    owner: &PreparedTextOwner,
    item: &PreparedTextItem,
    viewport: &AgentViewport,
) -> Option<Vec<AgentObservedObject>> {
    let root = view_text_root(step, owner, item, viewport)?;
    let context = ViewProjection {
        step,
        owner,
        item,
        viewport,
        root: &root,
    };
    let mut objects = vec![root.clone()];
    objects.extend(view_line_objects(&context));
    objects.extend(view_run_objects(&context));
    objects.extend(view_ruby_objects(&context));
    objects.extend(view_glyph_objects(&context));
    Some(dedupe_objects(objects))
}

fn view_text_root(
    step: usize,
    owner: &PreparedTextOwner,
    item: &PreparedTextItem,
    viewport: &AgentViewport,
) -> Option<AgentObservedObject> {
    let bbox = agent_bbox_from_hit_rect(owner.object_bounds, viewport)?;
    let root_id = format!(
        "object.text.{}",
        agent_uri_component(owner.semantic_id.as_str())
    );
    let parent_id = owner.parent_id.as_ref().map(ToString::to_string);
    let source = AgentCaptureSourceIdentity::Object {
        id: root_id.clone(),
        parent_id: parent_id.clone(),
        entity: Some(owner.semantic_id.to_string()),
        layer: "view.text".to_owned(),
        role: "text".to_owned(),
        object_layer: None,
        object_depth: None,
        rich_text: None,
    };
    Some(AgentObservedObject {
        id: root_id.clone(),
        parent_id,
        entity: Some(owner.semantic_id.to_string()),
        layer: "view.text".to_owned(),
        role: "text".to_owned(),
        visible: true,
        enabled: true,
        bbox: bbox.clone(),
        polygon: bbox.polygon(),
        capture_refs: agent_object_capture_refs_with_source(
            "cli", step, &root_id, &bbox, 0, source,
        ),
        object_layer: None,
        object_depth: None,
        text: Some(item.interaction.text.clone()),
        rich_text_ref: None,
        content: AgentObservedObjectContent::Custom {
            object_type: "prepared_text".to_owned(),
        },
    })
}

struct ViewProjection<'a> {
    step: usize,
    owner: &'a PreparedTextOwner,
    item: &'a PreparedTextItem,
    viewport: &'a AgentViewport,
    root: &'a AgentObservedObject,
}

struct ViewChildSpec {
    parent_id: String,
    id: String,
    role: &'static str,
    text: String,
    bbox: AgentBBox,
    reference: AgentRichTextElementRef,
}

fn view_line_objects(context: &ViewProjection<'_>) -> Vec<AgentObservedObject> {
    context
        .item
        .layout
        .lines
        .iter()
        .enumerate()
        .filter_map(|(line_index, line)| {
            let range = global_range_unbounded(context.owner, line.source_range)?;
            let bbox = agent_bbox_from_layout(line.bounds, context.viewport)?;
            Some(view_child_object(
                context.step,
                context.root,
                ViewChildSpec {
                    parent_id: context.root.id.clone(),
                    id: format!("{}.line.{line_index}", context.root.id),
                    role: "text_line",
                    text: context
                        .item
                        .interaction
                        .text
                        .get(line.source_range.start..line.source_range.end)
                        .unwrap_or_default()
                        .to_owned(),
                    bbox: bbox.clone(),
                    reference: AgentRichTextElementRef {
                        kind: AgentRichTextElementKind::TextLine,
                        index: line_index,
                        page: 0,
                        range,
                        node_index: 0,
                        source: None,
                        ruby: None,
                        presentation: None,
                        orientation: None,
                        vertical_form: None,
                        ruby_base_bbox: None,
                        ruby_annotation_bbox: None,
                        object_layer: None,
                        object_depth: None,
                        hit_test: false,
                        hit_regions: vec![agent_hit_region(
                            AgentHitRegionKind::TextLine,
                            &bbox,
                            range,
                        )],
                    },
                },
            ))
        })
        .collect()
}

fn view_run_objects(context: &ViewProjection<'_>) -> Vec<AgentObservedObject> {
    context
        .item
        .layout
        .runs
        .iter()
        .filter_map(|run| {
            let range = global_range_unbounded(context.owner, run.source_range)?;
            let run_index = usize::try_from(run.run_index).ok()?;
            let bbox = agent_bbox_from_layout(run.bounds, context.viewport)?;
            Some(view_child_object(
                context.step,
                context.root,
                ViewChildSpec {
                    parent_id: context.root.id.clone(),
                    id: format!("{}.run.{run_index}", context.root.id),
                    role: "text_run",
                    text: context
                        .item
                        .interaction
                        .text
                        .get(run.source_range.start..run.source_range.end)
                        .unwrap_or_default()
                        .to_owned(),
                    bbox: bbox.clone(),
                    reference: AgentRichTextElementRef {
                        kind: AgentRichTextElementKind::TextRun,
                        index: run_index,
                        page: 0,
                        range,
                        node_index: 0,
                        source: None,
                        ruby: None,
                        presentation: Some(run.presentation.clone()),
                        orientation: None,
                        vertical_form: None,
                        ruby_base_bbox: None,
                        ruby_annotation_bbox: None,
                        object_layer: object_layer(&run.presentation),
                        object_depth: object_depth(&run.presentation),
                        hit_test: presentation_has_hit_test_proxy(&run.presentation),
                        hit_regions: text_hit_regions(
                            AgentHitRegionKind::TextRun,
                            &bbox,
                            range,
                            &run.presentation,
                        ),
                    },
                },
            ))
        })
        .collect()
}

fn view_ruby_objects(context: &ViewProjection<'_>) -> Vec<AgentObservedObject> {
    context
        .item
        .layout
        .ruby
        .iter()
        .filter_map(|ruby| {
            let range = global_range_unbounded(context.owner, ruby.base_range)?;
            let base_bbox = agent_bbox_from_layout(ruby.base_bounds, context.viewport)?;
            let annotation_bbox = agent_bbox_from_layout(ruby.ruby_bounds, context.viewport)?;
            let bbox = agent_union_bbox(&base_bbox, &annotation_bbox);
            let ruby_index = usize::try_from(ruby.ruby_index).ok()?;
            let base_text = context
                .item
                .interaction
                .text
                .get(ruby.base_range.start..ruby.base_range.end)
                .unwrap_or_default();
            Some(view_child_object(
                context.step,
                context.root,
                ViewChildSpec {
                    parent_id: context.root.id.clone(),
                    id: format!("{}.ruby.{ruby_index}", context.root.id),
                    role: "text_ruby",
                    text: format!("{base_text} ({})", ruby.text),
                    bbox: bbox.clone(),
                    reference: AgentRichTextElementRef {
                        kind: AgentRichTextElementKind::Ruby,
                        index: ruby_index,
                        page: 0,
                        range,
                        node_index: 0,
                        source: None,
                        ruby: Some(ruby.text.clone()),
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
                },
            ))
        })
        .collect()
}

fn view_glyph_objects(context: &ViewProjection<'_>) -> Vec<AgentObservedObject> {
    context
        .item
        .layout
        .glyphs
        .iter()
        .enumerate()
        .flat_map(|(glyph_index, glyph)| {
            let Some(range) = global_range_unbounded(context.owner, glyph.source_range) else {
                return Vec::new();
            };
            let Some(bbox) = agent_bbox_from_layout(glyph.layout_bounds, context.viewport) else {
                return Vec::new();
            };
            let Some(run) = context
                .item
                .layout
                .runs
                .iter()
                .find(|run| run.run_index == glyph.run_index)
            else {
                return Vec::new();
            };
            let Ok(run_index) = usize::try_from(glyph.run_index) else {
                return Vec::new();
            };
            let Ok(cluster_index) = usize::try_from(glyph.cluster_index) else {
                return Vec::new();
            };
            let parent_id = format!("{}.run.{run_index}", context.root.id);
            let text = context
                .item
                .interaction
                .text
                .get(glyph.source_range.start..glyph.source_range.end)
                .unwrap_or_default()
                .to_owned();
            [
                (
                    AgentRichTextElementKind::TextGlyph,
                    glyph_index,
                    "text_glyph",
                    format!(
                        "{}.glyph.{glyph_index}.{}.{}",
                        context.root.id, range.start, range.end
                    ),
                    AgentHitRegionKind::TextGlyph,
                ),
                (
                    AgentRichTextElementKind::GlyphCluster,
                    cluster_index,
                    "text_cluster",
                    format!(
                        "{}.cluster.{cluster_index}.{}.{}",
                        context.root.id, range.start, range.end
                    ),
                    AgentHitRegionKind::GlyphCluster,
                ),
            ]
            .into_iter()
            .map(|(kind, index, role, id, hit_kind)| {
                view_child_object(
                    context.step,
                    context.root,
                    ViewChildSpec {
                        parent_id: parent_id.clone(),
                        id,
                        role,
                        text: text.clone(),
                        bbox: bbox.clone(),
                        reference: view_glyph_reference(
                            kind,
                            index,
                            range,
                            glyph,
                            &run.presentation,
                            &bbox,
                            hit_kind,
                        ),
                    },
                )
            })
            .collect::<Vec<_>>()
        })
        .collect()
}

fn view_glyph_reference(
    kind: AgentRichTextElementKind,
    index: usize,
    range: RichTextRange,
    glyph: &TextLayoutGlyph,
    presentation: &RichTextPresentation,
    bbox: &AgentBBox,
    hit_kind: AgentHitRegionKind,
) -> AgentRichTextElementRef {
    AgentRichTextElementRef {
        kind,
        index,
        page: 0,
        range,
        node_index: 0,
        source: None,
        ruby: None,
        presentation: Some(presentation.clone()),
        orientation: Some(agent_glyph_orientation(glyph.orientation)),
        vertical_form: Some(agent_glyph_vertical_form(glyph.vertical_form)),
        ruby_base_bbox: None,
        ruby_annotation_bbox: None,
        object_layer: object_layer(presentation),
        object_depth: object_depth(presentation),
        hit_test: presentation_has_hit_test_proxy(presentation),
        hit_regions: text_hit_regions(hit_kind, bbox, range, presentation),
    }
}

fn view_child_object(
    step: usize,
    root: &AgentObservedObject,
    spec: ViewChildSpec,
) -> AgentObservedObject {
    let source = AgentCaptureSourceIdentity::Object {
        id: spec.id.clone(),
        parent_id: Some(spec.parent_id.clone()),
        entity: root.entity.clone(),
        layer: "view.rich_text".to_owned(),
        role: spec.role.to_owned(),
        object_layer: spec.reference.object_layer.clone(),
        object_depth: spec.reference.object_depth,
        rich_text: Some((&spec.reference).into()),
    };
    AgentObservedObject {
        id: spec.id.clone(),
        parent_id: Some(spec.parent_id),
        entity: root.entity.clone(),
        layer: "view.rich_text".to_owned(),
        role: spec.role.to_owned(),
        visible: root.visible,
        enabled: root.enabled,
        bbox: spec.bbox.clone(),
        polygon: spec.bbox.polygon(),
        capture_refs: agent_object_capture_refs_with_source(
            "cli", step, &spec.id, &spec.bbox, 0, source,
        ),
        object_layer: spec.reference.object_layer.clone(),
        object_depth: spec.reference.object_depth,
        text: Some(spec.text),
        rich_text_ref: Some(spec.reference),
        content: AgentObservedObjectContent::Custom {
            object_type: "prepared_text".to_owned(),
        },
    }
}
