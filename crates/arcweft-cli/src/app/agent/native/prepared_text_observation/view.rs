//! Agent geometry for prepared text owned by authored or Rust-backed Views.

use super::*;

pub(in super::super) fn agent_view_prepared_text_objects(
    step: usize,
    prepared: &PreparedFrame,
    catalog: &arcweft_agent_protocol::text_context::AgentTextContextCatalog,
    geometries: &BTreeMap<arcweft_glyphon::PreparedTextId, PreparedTextGeometry>,
    viewport: &AgentViewport,
) -> Vec<AgentObservedObject> {
    prepared
        .prepared_text_owners()
        .iter()
        .enumerate()
        .filter(|(_, owner)| {
            matches!(
                owner.context,
                arcweft_text_model::PreparedTextContext::Document { .. }
            )
        })
        .filter_map(|(ordinal, prepared_owner)| {
            let owner = PreparedTextObservationOwner::new(prepared_owner);
            let reference = catalog
                .reference(u32::try_from(ordinal).expect("admitted catalog ordinal"))
                .expect("catalog contains every admitted owner");
            prepared.text.get(prepared_owner.text).and_then(|item| {
                view_text_objects(
                    step,
                    &owner,
                    item,
                    reference.clone(),
                    catalog
                        .resolve(&reference)
                        .expect("issued document context"),
                    geometries
                        .get(&prepared_owner.text)
                        .expect("admitted item geometry"),
                    viewport,
                )
            })
        })
        .flatten()
        .collect()
}
fn view_text_objects(
    step: usize,
    owner: &PreparedTextObservationOwner<'_>,
    item: &PreparedTextItem,
    reference: arcweft_agent_protocol::text_context::AgentTextContextReference,
    entry: &AgentTextContextEntry,
    geometry: &PreparedTextGeometry,
    viewport: &AgentViewport,
) -> Option<Vec<AgentObservedObject>> {
    let root = view_text_root(step, owner, item, reference, geometry, entry, viewport)?;
    let context = ViewProjection {
        step,
        owner: owner.prepared(),
        item,
        geometry,
        entry,
        viewport,
        root: &root,
    };
    let mut objects = vec![root.clone()];
    objects.extend(view_line_objects(&context));
    objects.extend(view_run_objects(&context));
    objects.extend(view_proxy_objects(&context));
    objects.extend(view_ruby_objects(&context));
    objects.extend(view_glyph_objects(&context));
    Some(objects)
}

fn view_text_root(
    step: usize,
    owner: &PreparedTextObservationOwner<'_>,
    item: &PreparedTextItem,
    reference: arcweft_agent_protocol::text_context::AgentTextContextReference,
    geometry: &PreparedTextGeometry,
    entry: &AgentTextContextEntry,
    viewport: &AgentViewport,
) -> Option<AgentObservedObject> {
    let root_id = owner.root_id().to_owned();
    let owner = owner.prepared();
    let bounds = owner.object_bounds;
    let bounds = LayoutRect::new(bounds.x, bounds.y, bounds.width, bounds.height);
    let bbox = agent_bbox_from_layout(
        geometry
            .bounds()
            .map_or(bounds, |painted| bounds.union(painted)),
        viewport,
    )?;
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
        capture_refs: agent_text_object_capture_refs(step, &root_id, &bbox, source, entry),
        object_layer: None,
        object_depth: None,
        text: Some(item.interaction.text.clone()),
        text_context: Some(reference),
        rich_text_ref: None,
        content: AgentObservedObjectContent::ViewText(
            arcweft_agent_protocol::object::AgentTextObjectContent::default(),
        ),
    })
}

struct ViewProjection<'a> {
    step: usize,
    owner: &'a PreparedTextOwner,
    item: &'a PreparedTextItem,
    geometry: &'a PreparedTextGeometry,
    entry: &'a AgentTextContextEntry,
    viewport: &'a AgentViewport,
    root: &'a AgentObservedObject,
}

struct ViewChildSpec {
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
            let bbox =
                agent_bbox_from_layout(context.geometry.line(line_index)?, context.viewport)?;
            Some(view_child_object(
                context.step,
                context.root,
                context.entry,
                ViewChildSpec {
                    role: "text_line",
                    text: prepared_text_for_source(context.owner, context.item, line.source_range)
                        .unwrap_or_default()
                        .to_owned(),
                    bbox: bbox.clone(),
                    reference: AgentRichTextElementRef {
                        kind: AgentRichTextElementKind::TextLine,
                        index: line_index,
                        page: 0,
                        range,
                        node_index: RichTextNodeIndex::new(0),
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
        .enumerate()
        .filter_map(|(layout_index, run)| {
            let range = global_range_unbounded(context.owner, run.source_range)?;
            let run_index = usize::try_from(run.run_index).ok()?;
            let bbox =
                agent_bbox_from_layout(context.geometry.run(layout_index)?, context.viewport)?;
            Some(view_child_object(
                context.step,
                context.root,
                context.entry,
                ViewChildSpec {
                    role: "text_run",
                    text: prepared_text_for_source(context.owner, context.item, run.source_range)
                        .unwrap_or_default()
                        .to_owned(),
                    bbox: bbox.clone(),
                    reference: AgentRichTextElementRef {
                        kind: AgentRichTextElementKind::TextRun,
                        index: run_index,
                        page: 0,
                        range,
                        node_index: RichTextNodeIndex::new(0),
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
            let geometry = context.geometry.ruby(ruby.ruby_index)?;
            let base_bbox = agent_bbox_from_layout(geometry.base_bounds, context.viewport)?;
            let annotation_bbox =
                agent_bbox_from_layout(geometry.annotation_bounds, context.viewport)?;
            let bbox = agent_union_bbox(&base_bbox, &annotation_bbox);
            let ruby_index = usize::try_from(ruby.ruby_index).ok()?;
            let base_text = prepared_text_for_source(context.owner, context.item, ruby.base_range)
                .unwrap_or_default();
            Some(view_child_object(
                context.step,
                context.root,
                context.entry,
                ViewChildSpec {
                    role: "text_ruby",
                    text: format!("{base_text} ({})", ruby.text),
                    bbox: bbox.clone(),
                    reference: AgentRichTextElementRef {
                        kind: AgentRichTextElementKind::Ruby,
                        index: ruby_index,
                        page: 0,
                        range,
                        node_index: ruby.owner_node,
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

fn view_proxy_objects(context: &ViewProjection<'_>) -> Vec<AgentObservedObject> {
    let mut objects = Vec::new();
    for (layout_index, run) in context.item.layout.runs.iter().enumerate() {
        let Some(range) = global_range_unbounded(context.owner, run.source_range) else {
            continue;
        };
        let Some(bbox) = context
            .geometry
            .run(layout_index)
            .and_then(|bounds| agent_bbox_from_layout(bounds, context.viewport))
        else {
            continue;
        };
        for (proxy_index, proxy) in run.presentation.object_proxies.iter().enumerate() {
            let text = prepared_text_for_source(context.owner, context.item, run.source_range)
                .expect("admitted document proxy range preserves UTF-8")
                .to_owned();
            objects.push(view_child_object(
                context.step,
                context.root,
                context.entry,
                ViewChildSpec {
                    role: "text_proxy",
                    text,
                    bbox: bbox.clone(),
                    reference: AgentRichTextElementRef {
                        kind: AgentRichTextElementKind::TextObjectProxy,
                        index: proxy_index,
                        page: 0,
                        range,
                        node_index: RichTextNodeIndex::new(0),
                        source: None,
                        ruby: None,
                        presentation: Some(proxy_presentation(&run.presentation, proxy)),
                        orientation: None,
                        vertical_form: None,
                        ruby_base_bbox: None,
                        ruby_annotation_bbox: None,
                        object_layer: proxy
                            .layer
                            .clone()
                            .or_else(|| run.presentation.layer.clone()),
                        object_depth: proxy.depth.map(|depth| depth.0).or_else(|| {
                            (run.presentation.z_index != 0)
                                .then_some(i32::from(run.presentation.z_index) * 1_000)
                        }),
                        hit_test: proxy.hit_test,
                        hit_regions: proxy_hit_regions(&bbox, range, &run.presentation, proxy),
                    },
                },
            ));
        }
    }
    objects
}

fn view_glyph_objects(context: &ViewProjection<'_>) -> Vec<AgentObservedObject> {
    let mut objects = view_body_glyph_objects(context);
    objects.extend(view_body_cluster_objects(context));
    objects
}

fn view_body_glyph_objects(context: &ViewProjection<'_>) -> Vec<AgentObservedObject> {
    context
        .item
        .layout
        .glyphs
        .iter()
        .enumerate()
        .filter_map(|(glyph_index, glyph)| {
            let range = global_range_unbounded(context.owner, glyph.source_range)?;
            let bbox = agent_bbox_from_layout(
                context.geometry.glyph(glyph_index)?.logical_bounds,
                context.viewport,
            )?;
            let run = context
                .item
                .layout
                .runs
                .iter()
                .find(|run| run.run_index == glyph.run_index)?;
            let text = prepared_text_for_source(context.owner, context.item, glyph.source_range)
                .unwrap_or_default()
                .to_owned();
            Some(view_child_object(
                context.step,
                context.root,
                context.entry,
                ViewChildSpec {
                    role: "text_glyph",
                    text,
                    bbox: bbox.clone(),
                    reference: view_glyph_reference(
                        AgentRichTextElementKind::TextGlyph,
                        glyph_index,
                        range,
                        glyph,
                        &run.presentation,
                        &bbox,
                        AgentHitRegionKind::TextGlyph,
                    ),
                },
            ))
        })
        .collect()
}

fn view_body_cluster_objects(context: &ViewProjection<'_>) -> Vec<AgentObservedObject> {
    prepared_body_clusters(context.item, context.geometry)
        .into_iter()
        .filter_map(|cluster| {
            let range = global_range_unbounded(context.owner, cluster.source_range)?;
            let bbox = agent_bbox_from_layout(cluster.bounds, context.viewport)?;
            let run = context
                .item
                .layout
                .runs
                .iter()
                .find(|run| run.run_index == cluster.run_index)?;
            let cluster_index = usize::try_from(cluster.index).ok()?;
            let text = prepared_text_for_source(context.owner, context.item, cluster.source_range)
                .unwrap_or_default()
                .to_owned();
            Some(view_child_object(
                context.step,
                context.root,
                context.entry,
                ViewChildSpec {
                    role: "text_cluster",
                    text,
                    bbox: bbox.clone(),
                    reference: view_glyph_reference(
                        AgentRichTextElementKind::GlyphCluster,
                        cluster_index,
                        range,
                        cluster.representative,
                        &run.presentation,
                        &bbox,
                        AgentHitRegionKind::GlyphCluster,
                    ),
                },
            ))
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
        node_index: RichTextNodeIndex::new(0),
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
    entry: &AgentTextContextEntry,
    spec: ViewChildSpec,
) -> AgentObservedObject {
    let id = entry
        .element_id(&spec.reference)
        .expect("document element belongs to its admitted context");
    let parent_id = entry
        .element_parent_id(Some(&spec.reference))
        .expect("admitted document element parent");
    let source = AgentCaptureSourceIdentity::Object {
        id: id.clone(),
        parent_id: parent_id.clone(),
        entity: root.entity.clone(),
        layer: "view.rich_text".to_owned(),
        role: spec.role.to_owned(),
        object_layer: spec.reference.object_layer.clone(),
        object_depth: spec.reference.object_depth,
        rich_text: Some((&spec.reference).into()),
    };
    AgentObservedObject {
        id: id.clone(),
        parent_id,
        entity: root.entity.clone(),
        layer: "view.rich_text".to_owned(),
        role: spec.role.to_owned(),
        visible: root.visible,
        enabled: root.enabled,
        bbox: spec.bbox.clone(),
        polygon: spec.bbox.polygon(),
        capture_refs: agent_text_object_capture_refs(step, &id, &spec.bbox, source, entry),
        object_layer: spec.reference.object_layer.clone(),
        object_depth: spec.reference.object_depth,
        text: Some(spec.text),
        text_context: root.text_context.clone(),
        rich_text_ref: Some(spec.reference),
        content: AgentObservedObjectContent::ViewText(
            arcweft_agent_protocol::object::AgentTextObjectContent::default(),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn owner(semantic_id: &str, mount: u64) -> PreparedTextOwner {
        PreparedTextOwner::new(
            arcweft_render_wgpu::view_scene::PreparedTextId::from_index(0),
            arcweft_id::PublicId::try_new(semantic_id).expect("test semantic id"),
            arcweft_text_model::PreparedTextOwnerKind::View { mount },
            arcweft_text_model::PreparedTextContext::document(0, 0).unwrap(),
            HitRect::new(0.0, 0.0, 10.0, 10.0),
        )
    }

    #[test]
    fn prepared_view_text_root_identity_includes_mount_occurrence() {
        let first = PreparedTextObservationOwner::new(&owner("view.shared.text", 17))
            .root_id()
            .to_owned();
        let second = PreparedTextObservationOwner::new(&owner("view.shared.text", 18))
            .root_id()
            .to_owned();

        assert_ne!(first, second);
        assert!(first.ends_with(".mount.17.text.0"));
        assert!(second.ends_with(".mount.18.text.0"));
    }

    #[test]
    fn prepared_view_text_root_identity_encodes_semantic_ids_injectively() {
        let slash = PreparedTextObservationOwner::new(&owner("view/text", 17))
            .root_id()
            .to_owned();
        let underscore = PreparedTextObservationOwner::new(&owner("view_text", 17))
            .root_id()
            .to_owned();

        assert_ne!(slash, underscore);
        assert!(slash.contains("view%2Ftext"));
        assert!(underscore.contains("view_text"));
    }
}
