//! Agent objects projected from the same prepared layout used for rendering.

mod owner;
mod view;

pub(super) use owner::PreparedTextObservationOwner;
pub(super) use view::agent_view_prepared_text_objects;

use super::image_mapping::{
    agent_ceil_viewport_f32, agent_floor_viewport_f32, agent_object_capture_refs_with_source,
    agent_union_bbox, agent_uri_component,
};
use super::*;
use arcweft_agent_protocol::rich_text::{AgentGlyphOrientation, AgentGlyphVerticalForm};
use arcweft_agent_protocol::text_context::{AgentTextContextCatalog, AgentTextContextEntry};
use arcweft_glyphon::{PreparedTextGeometry, PreparedTextItem};
use arcweft_presentation::hit::HitRect;
use arcweft_render_wgpu::geometry::{PreparedFrame, PreparedTextOwner};
use arcweft_text_layout::{GlyphOrientation, GlyphVerticalForm, LayoutRect, TextLayoutGlyph};
use arcweft_text_model::{
    LineDisplayStageProjection, RichTextNodeIndex, RichTextObjectProxy, RichTextPresentation,
    RichTextTextRun, RichTextTextSource,
};

pub(super) fn agent_prepared_text_context_catalog(
    prepared: &PreparedFrame,
    scope: arcweft_agent_protocol::text_context::AgentTextContextScope,
) -> Result<arcweft_agent_protocol::text_context::AgentTextContextCatalog, ExitCode> {
    use arcweft_agent_protocol::text_context::{
        AgentTextContextCatalog, AgentTextContextContent, AgentTextContextEntry,
        AgentTextContextOwner,
    };
    let mut entries = Vec::new();
    for (ordinal, prepared_owner) in prepared.prepared_text_owners().iter().enumerate() {
        let owner = PreparedTextObservationOwner::new(prepared_owner);
        let item = prepared
            .text
            .get(prepared_owner.text)
            .expect("admitted owner has an actual prepared item");
        let content = match &prepared_owner.context {
            arcweft_text_model::PreparedTextContext::DisplayStage { context } => {
                AgentTextContextContent::DisplayStage {
                    context: std::sync::Arc::clone(context),
                    layout: std::sync::Arc::clone(&item.layout),
                }
            }
            arcweft_text_model::PreparedTextContext::Document { context } => {
                AgentTextContextContent::PreparedDocument {
                    context: *context,
                    text: item.interaction.text.clone(),
                    layout: std::sync::Arc::clone(&item.layout),
                }
            }
        };
        entries.push(AgentTextContextEntry {
            owner_ordinal: u32::try_from(ordinal).map_err(|error| {
                eprintln!("error: prepared owner ordinal: {error}");
                ExitCode::FAILURE
            })?,
            owner: AgentTextContextOwner {
                root_id: owner.root_id().to_owned(),
                parent_id: prepared_owner.parent_id.as_ref().map(ToString::to_string),
                semantic_id: prepared_owner.semantic_id.to_string(),
                prepared_text: prepared_owner.text.index(),
                kind: prepared_owner.kind,
            },
            content,
        });
    }
    AgentTextContextCatalog::try_new(scope, entries).map_err(|error| {
        eprintln!("error: prepared text catalog: {error}");
        ExitCode::FAILURE
    })
}

pub(super) fn agent_dialogue_prepared_text_objects(
    capture_step: usize,
    owner: &PreparedTextObservationOwner<'_>,
    reference: arcweft_agent_protocol::text_context::AgentTextContextReference,
    catalog: &AgentTextContextCatalog,
    prepared: &PreparedFrame,
    geometry: &PreparedTextGeometry,
    viewport: &AgentViewport,
) -> Result<Vec<AgentObservedObject>, ExitCode> {
    let item = prepared
        .text
        .get(owner.prepared().text)
        .expect("admitted text owner has its item");
    let stage = owner
        .prepared()
        .context
        .stage()
        .expect("stage observer receives an admitted stage context");
    let frame = stage.frame();
    let entry = catalog
        .resolve(&reference)
        .expect("issued reference resolves to the actual prepared owner");
    let root = dialogue_view_object(
        capture_step,
        frame,
        owner,
        reference,
        geometry,
        entry,
        viewport,
    )?;
    let mut objects = vec![root.clone()];
    objects.extend(dialogue_children(
        capture_step,
        &root,
        frame,
        owner.prepared(),
        item,
        geometry,
        entry,
        viewport,
    ));
    Ok(objects)
}

fn dialogue_view_object(
    capture_step: usize,
    frame: &LineDisplayStageProjection,
    owner: &PreparedTextObservationOwner<'_>,
    reference: arcweft_agent_protocol::text_context::AgentTextContextReference,
    geometry: &PreparedTextGeometry,
    entry: &AgentTextContextEntry,
    viewport: &AgentViewport,
) -> Result<AgentObservedObject, ExitCode> {
    let owner_bounds = owner.prepared().object_bounds;
    let bounds = LayoutRect::new(
        owner_bounds.x,
        owner_bounds.y,
        owner_bounds.width,
        owner_bounds.height,
    );
    let bounds = geometry
        .bounds()
        .map_or(bounds, |painted| bounds.union(painted));
    let bbox = agent_bbox_from_layout(bounds, viewport).ok_or_else(|| {
        eprintln!("error: prepared-text owner has empty viewport geometry");
        ExitCode::FAILURE
    })?;
    let object_id = owner.root_id().to_owned();
    let parent_id = owner.prepared().parent_id.as_ref().map(ToString::to_string);
    let dialogue = matches!(
        owner.prepared().kind,
        arcweft_text_model::PreparedTextOwnerKind::DialogueView {
            role: arcweft_text_model::DialoguePreparedTextRole::Content,
            ..
        }
    );
    let layer = if dialogue { "dialogue" } else { "view.text" };
    let role = if dialogue {
        AGENT_ROLE_DIALOGUE_VIEW
    } else {
        "rich_text_view"
    };
    let source = AgentCaptureSourceIdentity::Object {
        id: object_id.clone(),
        parent_id: parent_id.clone(),
        entity: Some(frame.character.id.as_str().to_owned()),
        layer: layer.to_owned(),
        role: role.to_owned(),
        object_layer: None,
        object_depth: None,
        rich_text: None,
    };
    Ok(AgentObservedObject {
        id: object_id.clone(),
        parent_id,
        entity: Some(frame.character.id.as_str().to_owned()),
        layer: layer.to_owned(),
        role: role.to_owned(),
        visible: true,
        enabled: true,
        bbox: bbox.clone(),
        polygon: bbox.polygon(),
        capture_refs: agent_text_object_capture_refs(
            capture_step,
            &object_id,
            &bbox,
            source,
            entry,
        ),
        object_layer: None,
        object_depth: None,
        text: Some(frame.text.clone()),
        text_context: Some(reference),
        rich_text_ref: None,
        content: AgentObservedObjectContent::RichText(
            arcweft_agent_protocol::object::AgentTextObjectContent::default(),
        ),
    })
}

fn dialogue_children(
    capture_step: usize,
    dialogue_object: &AgentObservedObject,
    frame: &LineDisplayStageProjection,
    owner: &PreparedTextOwner,
    item: &PreparedTextItem,
    geometry: &PreparedTextGeometry,
    entry: &AgentTextContextEntry,
    viewport: &AgentViewport,
) -> Vec<AgentObservedObject> {
    let context = DialogueProjection {
        capture_step,
        dialogue_object,
        frame,
        page_index: frame.page_index,
        owner,
        item,
        geometry,
        entry,
        viewport,
        run_geometry: dialogue_run_geometry(frame, owner, item, geometry, viewport),
    };
    let mut children = dialogue_page_objects(&context);
    children.extend(dialogue_line_objects(&context));
    children.extend(dialogue_run_objects(&context));
    children.extend(dialogue_ruby_objects(&context));
    children.extend(dialogue_glyph_objects(&context));
    children
}
struct DialogueProjection<'a> {
    capture_step: usize,
    dialogue_object: &'a AgentObservedObject,
    frame: &'a LineDisplayStageProjection,
    page_index: usize,
    owner: &'a PreparedTextOwner,
    item: &'a PreparedTextItem,
    geometry: &'a PreparedTextGeometry,
    entry: &'a AgentTextContextEntry,
    viewport: &'a AgentViewport,
    run_geometry: Vec<(usize, &'a RichTextTextRun, RichTextRange, AgentBBox)>,
}

struct PreparedBodyCluster<'a> {
    index: u32,
    run_index: u32,
    source_range: RichTextRange,
    bounds: LayoutRect,
    representative: &'a TextLayoutGlyph,
}

fn prepared_body_clusters<'a>(
    item: &'a PreparedTextItem,
    geometry: &PreparedTextGeometry,
) -> Vec<PreparedBodyCluster<'a>> {
    let mut clusters = BTreeMap::<u32, PreparedBodyCluster<'_>>::new();
    for (index, glyph) in item.layout.glyphs.iter().enumerate() {
        let bounds = geometry
            .glyph(index)
            .expect("admitted body glyph geometry")
            .logical_bounds;
        clusters
            .entry(glyph.cluster_index)
            .and_modify(|cluster| {
                debug_assert_eq!(cluster.run_index, glyph.run_index);
                debug_assert_eq!(cluster.representative.orientation, glyph.orientation);
                debug_assert_eq!(cluster.representative.vertical_form, glyph.vertical_form);
                cluster.source_range = RichTextRange::new(
                    cluster.source_range.start.min(glyph.source_range.start),
                    cluster.source_range.end.max(glyph.source_range.end),
                );
                cluster.bounds = cluster.bounds.union(bounds);
            })
            .or_insert(PreparedBodyCluster {
                index: glyph.cluster_index,
                run_index: glyph.run_index,
                source_range: glyph.source_range,
                bounds,
                representative: glyph,
            });
    }
    clusters.into_values().collect()
}

fn dialogue_page_objects(context: &DialogueProjection<'_>) -> Vec<AgentObservedObject> {
    let visible_range = context
        .owner
        .context
        .ranges()
        .prepared_to_display(context.owner.context.ranges().prepared_range())
        .expect("admitted prepared range has its display extent");
    let page = context.page_index;
    let Some(text) = text_for_range(context.frame, visible_range) else {
        return Vec::new();
    };
    if text.trim().is_empty() {
        return Vec::new();
    }
    let mut hit_regions = vec![agent_hit_region(
        AgentHitRegionKind::TextPage,
        &context.dialogue_object.bbox,
        visible_range,
    )];
    hit_regions.extend(proxy_hit_regions_for_range(
        visible_range,
        &context.run_geometry,
    ));
    vec![dialogue_child_object(
        context.capture_step,
        context.dialogue_object,
        context.entry,
        DialogueChildSpec {
            role: "rich_text_page",
            text: text.to_owned(),
            bbox: &context.dialogue_object.bbox,
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
        let Some(bbox) = context
            .geometry
            .line(line_index)
            .and_then(|bounds| agent_bbox_from_layout(bounds, context.viewport))
        else {
            continue;
        };
        let page = context.page_index;
        children.push(dialogue_child_object(
            context.capture_step,
            context.dialogue_object,
            context.entry,
            DialogueChildSpec {
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
                    hit_regions: {
                        let mut regions =
                            vec![agent_hit_region(AgentHitRegionKind::TextLine, &bbox, range)];
                        regions.extend(proxy_hit_regions_for_range(range, &context.run_geometry));
                        regions
                    },
                },
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
        if matches!(run.source, RichTextTextSource::ControlHardBreak) {
            continue;
        }
        let page = context.page_index;
        children.push(dialogue_child_object(
            context.capture_step,
            context.dialogue_object,
            context.entry,
            DialogueChildSpec {
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
            },
        ));
        children.extend(dialogue_proxy_objects(
            context,
            &DialogueRunProjection {
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
            let presentation = proxy_presentation(&projection.run.presentation, proxy);
            dialogue_child_object(
                context.capture_step,
                context.dialogue_object,
                context.entry,
                DialogueChildSpec {
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
                },
            )
        })
        .collect()
}

fn dialogue_ruby_objects(context: &DialogueProjection<'_>) -> Vec<AgentObservedObject> {
    let mut children = Vec::new();
    let annotations = context
        .frame
        .display_map
        .ruby_annotations
        .iter()
        .enumerate()
        .map(|(index, ruby)| {
            (
                (
                    ruby.owner_node.get(),
                    ruby.base_range.start,
                    ruby.base_range.end,
                    ruby.ruby.as_str(),
                ),
                (index, ruby),
            )
        })
        .collect::<BTreeMap<_, _>>();
    for layout_ruby in &context.item.layout.ruby {
        let Some(range) = global_range(
            context.owner,
            layout_ruby.base_range,
            context.frame.text.len(),
        ) else {
            continue;
        };
        let Some(&(ruby_index, ruby)) = annotations.get(&(
            layout_ruby.owner_node.get(),
            range.start,
            range.end,
            layout_ruby.text.as_str(),
        )) else {
            continue;
        };
        let Some(geometry) = context.geometry.ruby(layout_ruby.ruby_index) else {
            continue;
        };
        let Some(base_bbox) = agent_bbox_from_layout(geometry.base_bounds, context.viewport) else {
            continue;
        };
        let Some(annotation_bbox) =
            agent_bbox_from_layout(geometry.annotation_bounds, context.viewport)
        else {
            continue;
        };
        let bbox = agent_union_bbox(&base_bbox, &annotation_bbox);
        let page = context.page_index;
        let base_text = text_for_range(context.frame, range).unwrap_or_default();
        children.push(dialogue_child_object(
            context.capture_step,
            context.dialogue_object,
            context.entry,
            DialogueChildSpec {
                role: "rich_text_ruby",
                text: format!("{base_text} ({})", ruby.ruby),
                bbox: &bbox,
                reference: AgentRichTextElementRef {
                    kind: AgentRichTextElementKind::Ruby,
                    index: ruby_index,
                    page,
                    range,
                    node_index: ruby.owner_node,
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
        let Some((_, run)) = find_run(context.frame, range) else {
            continue;
        };
        let Some(text) =
            text_for_range(context.frame, range).filter(|text| !text.trim().is_empty())
        else {
            continue;
        };
        let Some(bbox) = context
            .geometry
            .glyph(glyph_index)
            .and_then(|glyph| agent_bbox_from_layout(glyph.logical_bounds, context.viewport))
        else {
            continue;
        };
        let page = context.page_index;
        children.push(dialogue_child_object(
            context.capture_step,
            context.dialogue_object,
            context.entry,
            DialogueChildSpec {
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
            },
        ));
    }
    for cluster in prepared_body_clusters(context.item, context.geometry) {
        let Some(range) = global_range(
            context.owner,
            cluster.source_range,
            context.frame.text.len(),
        ) else {
            continue;
        };
        let Some((_, run)) = find_run(context.frame, range) else {
            continue;
        };
        let Some(text) =
            text_for_range(context.frame, range).filter(|text| !text.trim().is_empty())
        else {
            continue;
        };
        let Some(bbox) = agent_bbox_from_layout(cluster.bounds, context.viewport) else {
            continue;
        };
        let page = context.page_index;
        let cluster_index = usize::try_from(cluster.index).unwrap_or(usize::MAX);
        children.push(dialogue_child_object(
            context.capture_step,
            context.dialogue_object,
            context.entry,
            DialogueChildSpec {
                role: "rich_text_cluster",
                text: text.to_owned(),
                bbox: &bbox,
                reference: glyph_reference(
                    AgentRichTextElementKind::GlyphCluster,
                    cluster_index,
                    page,
                    range,
                    run,
                    cluster.representative,
                    &bbox,
                    AgentHitRegionKind::GlyphCluster,
                ),
            },
        ));
    }
    children
}

fn dialogue_run_geometry<'a>(
    frame: &'a LineDisplayStageProjection,
    owner: &PreparedTextOwner,
    item: &PreparedTextItem,
    geometry: &PreparedTextGeometry,
    viewport: &AgentViewport,
) -> Vec<(usize, &'a RichTextTextRun, RichTextRange, AgentBBox)> {
    let mut by_run = BTreeMap::<usize, (RichTextRange, AgentBBox)>::new();
    for (layout_index, run) in item.layout.runs.iter().enumerate() {
        let Some(range) = global_range(owner, run.source_range, frame.text.len()) else {
            continue;
        };
        let Some((index, _)) = find_run(frame, range) else {
            continue;
        };
        let Some(bbox) = geometry
            .run(layout_index)
            .and_then(|bounds| agent_bbox_from_layout(bounds, viewport))
        else {
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
    role: &'a str,
    text: String,
    bbox: &'a AgentBBox,
    reference: AgentRichTextElementRef,
}

fn dialogue_child_object(
    step: usize,
    dialogue: &AgentObservedObject,
    entry: &AgentTextContextEntry,
    spec: DialogueChildSpec<'_>,
) -> AgentObservedObject {
    let id = entry
        .element_id(&spec.reference)
        .expect("observed element belongs to its admitted context");
    let parent_id = entry
        .element_parent_id(Some(&spec.reference))
        .expect("admitted element has its owning parent");
    let layer = format!("{}.rich_text", dialogue.layer);
    let source = AgentCaptureSourceIdentity::Object {
        id: id.clone(),
        parent_id: parent_id.clone(),
        entity: dialogue.entity.clone(),
        layer: layer.clone(),
        role: spec.role.to_owned(),
        object_layer: spec.reference.object_layer.clone(),
        object_depth: spec.reference.object_depth,
        rich_text: Some((&spec.reference).into()),
    };
    AgentObservedObject {
        id: id.clone(),
        parent_id,
        entity: dialogue.entity.clone(),
        layer,
        role: spec.role.to_owned(),
        visible: dialogue.visible,
        enabled: dialogue.enabled,
        bbox: spec.bbox.clone(),
        polygon: spec.bbox.polygon(),
        capture_refs: agent_text_object_capture_refs(step, &id, spec.bbox, source, entry),
        object_layer: spec.reference.object_layer.clone(),
        object_depth: spec.reference.object_depth,
        text: Some(spec.text.clone()),
        text_context: dialogue.text_context.clone(),
        rich_text_ref: Some(spec.reference),
        content: dialogue.content.clone(),
    }
}

fn agent_text_object_capture_refs(
    step: usize,
    id: &str,
    bbox: &AgentBBox,
    source: AgentCaptureSourceIdentity,
    entry: &AgentTextContextEntry,
) -> AgentObjectCaptureRefs {
    let page = entry
        .content
        .display_stage()
        .map_or(0, |stage| stage.frame().page_index);
    let mut refs = agent_object_capture_refs_with_source("cli", step, id, bbox, source);
    for capture in &mut refs.captures {
        capture.page = page;
        if page != 0 {
            capture.uri.push_str(&format!("?page={page}"));
        }
    }
    refs
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
    owner.context.ranges().source_to_display(range).ok()
}

fn valid_range(range: RichTextRange, text_len: usize) -> Option<std::ops::Range<usize>> {
    (range.start <= range.end && range.end <= text_len).then_some(range.start..range.end)
}

fn text_for_range(frame: &LineDisplayStageProjection, range: RichTextRange) -> Option<&str> {
    frame.text.get(valid_range(range, frame.text.len())?)
}

fn find_run(
    frame: &LineDisplayStageProjection,
    range: RichTextRange,
) -> Option<(usize, &RichTextTextRun)> {
    let runs = &frame.display_map.text_runs;
    let index = runs.partition_point(|run| run.range.end <= range.start);
    runs.get(index)
        .filter(|run| range.start >= run.range.start && range.end <= run.range.end)
        .map(|run| (index, run))
}

fn ranges_overlap(left: RichTextRange, right: RichTextRange) -> bool {
    left.start < right.end && right.start < left.end
}

fn range_node_index(frame: &LineDisplayStageProjection, range: RichTextRange) -> RichTextNodeIndex {
    frame
        .display_map
        .text_runs
        .iter()
        .find(|run| ranges_overlap(run.range, range))
        .map_or(RichTextNodeIndex::new(0), |run| run.node_index)
}

fn presentation_for_range(
    frame: &LineDisplayStageProjection,
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

fn object_depth_for_range(frame: &LineDisplayStageProjection, range: RichTextRange) -> Option<i32> {
    frame
        .display_map
        .text_runs
        .iter()
        .filter(|run| ranges_overlap(run.range, range))
        .filter_map(|run| object_depth(&run.presentation))
        .max()
}

fn object_layer_for_range(
    frame: &LineDisplayStageProjection,
    range: RichTextRange,
) -> Option<String> {
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
        proxy_schema: None,
        proxy_fields: Vec::new(),
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
        proxy_schema: proxy.schema.clone(),
        proxy_fields: proxy.fields.clone(),
        proxy_role: proxy.role.clone(),
        proxy_layer: proxy.layer.clone().or_else(|| presentation.layer.clone()),
        depth: proxy.depth.map(|depth| depth.0),
        proxy_params: BTreeMap::new(),
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

fn prepared_text_for_source<'a>(
    owner: &PreparedTextOwner,
    item: &'a PreparedTextItem,
    source: RichTextRange,
) -> Option<&'a str> {
    let range = owner.context.ranges().source_to_prepared(source).ok()?;
    item.interaction.text.get(range.start..range.end)
}
