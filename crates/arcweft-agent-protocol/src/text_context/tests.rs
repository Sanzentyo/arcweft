use super::*;
use crate::{
    geometry::AgentViewport,
    hit_test::{AgentHitTestHit, AgentHitTestReport},
    image::{
        AgentImageComposition, AgentImageKind, AgentImageMetadata, AgentImageObjectRef,
        AgentImageRenderer, AgentImageResource, AgentImageScope,
    },
    observation::{AgentObjectsReport, AgentObservationReport},
    resource::{AgentBinaryEncoding, AgentBinaryResourceBody, AgentResourceBody},
};

#[test]
fn issued_catalog_selection_preserves_original_owner_ordinals_and_shares_rows() {
    let original = crate::tests::test_text_contexts(12, "object.test.a");
    let mut second = original.entries().next().unwrap().clone();
    second.owner_ordinal = 1;
    second.owner.prepared_text = 1;
    second.owner.root_id = "object.test.b".to_owned();
    second.owner.kind = PreparedTextOwnerKind::DialogueView {
        dialogue: 0,
        entry: 0,
        mount: 1,
        role: DialoguePreparedTextRole::Content,
    };
    let catalog = AgentTextContextCatalog::try_new(
        original.scope().clone(),
        [second, original.entries().next().unwrap().clone()],
    )
    .unwrap();
    assert_eq!(
        catalog
            .entries()
            .map(|entry| entry.owner_ordinal)
            .collect::<Vec<_>>(),
        [0, 1]
    );
    let reference = catalog.reference(1).unwrap();
    let subset = catalog.selected([&reference]).unwrap();
    assert_eq!(
        subset
            .entries()
            .map(|entry| entry.owner_ordinal)
            .collect::<Vec<_>>(),
        [1]
    );
    assert!(std::ptr::eq(
        catalog.resolve(&reference).unwrap(),
        subset.resolve(&reference).unwrap()
    ));
    assert!(Arc::ptr_eq(
        catalog.resolve(&reference).unwrap().content.layout(),
        subset.resolve(&reference).unwrap().content.layout()
    ));
    assert!(matches!(
        subset.resolve(&catalog.reference(0).unwrap()),
        Err(AgentTextContextError::Missing { owner: 0 })
    ));
}

#[test]
fn observation_and_objects_envelopes_are_self_contained_and_reject_missing_or_mismatched_contexts()
{
    let report = crate::tests::test_serialization_observation_report();
    let bytes = serde_json::to_vec(&report).unwrap();
    assert_eq!(AgentObservationReport::decode_json(&bytes).unwrap(), report);
    let resource = report.objects_resource().unwrap();
    let AgentResourceBody::Json(body) = resource.body else {
        panic!("Objects retains the typed JSON body wrapper");
    };
    let objects = AgentObjectsReport::decode_json(&serde_json::to_vec(&body).unwrap()).unwrap();
    assert_eq!(objects.objects, report.objects);
    assert_eq!(objects.text_contexts, report.text_contexts);
    assert!(
        body["objects"][0]["content"].get("frame").is_none(),
        "no per-child frame duplication or old Box reader"
    );
    for mutation in 0..6 {
        let mut wire = serde_json::to_value(&report).unwrap();
        match mutation {
            0 => {
                wire.as_object_mut().unwrap().remove("text_contexts");
            }
            1 => wire["text_contexts"]["entries"] = serde_json::json!([]),
            2 => wire["objects"][0]["text_context"]["owner"] = serde_json::json!(9),
            3 => wire["objects"][0]["text_context"]["scope"]["tick"] = serde_json::json!(2),
            4 => wire["objects"][0]["parent_id"] = serde_json::json!("object.other"),
            _ => wire["objects"][0]["content"]["frame"] = serde_json::json!({}),
        }
        assert!(
            AgentObservationReport::decode_json(&serde_json::to_vec(&wire).unwrap()).is_err(),
            "mutation {mutation} must be rejected"
        );
    }
}

#[test]
fn catalog_rejects_duplicate_prepared_target_root_and_owner_ordinal() {
    let original = crate::tests::test_text_contexts(12, "object.test.a");
    let first = original.entries().next().unwrap();
    for change in 0..3 {
        let mut second = first.clone();
        second.owner_ordinal = 1;
        second.owner.root_id = "object.test.b".to_owned();
        second.owner.prepared_text = 1;
        match change {
            0 => second.owner.prepared_text = first.owner.prepared_text,
            1 => second.owner.root_id = first.owner.root_id.clone(),
            _ => second.owner_ordinal = first.owner_ordinal,
        }
        let Err(error) =
            AgentTextContextCatalog::try_new(original.scope().clone(), [first.clone(), second])
        else {
            panic!("duplicate catalog ownership must be rejected");
        };
        match (change, error) {
            (0, AgentTextContextError::DuplicatePreparedText { text })
                if text == first.owner.prepared_text => {}
            (1, AgentTextContextError::DuplicateRoot { root_id })
                if root_id == first.owner.root_id => {}
            (2, AgentTextContextError::Duplicate { owner }) if owner == first.owner_ordinal => {}
            (_, error) => panic!("wrong duplicate rejection: {error}"),
        }
    }
}

#[test]
fn canonical_element_identity_rejects_cross_owner_ref_and_changed_range_metadata() {
    let report = crate::tests::test_serialization_observation_report();
    let mut other = report.text_contexts.entries().next().unwrap().clone();
    other.owner_ordinal = 1;
    other.owner.root_id = "object.other".to_owned();
    other.owner.prepared_text = 1;
    let catalog = AgentTextContextCatalog::try_new(
        report.text_contexts.scope().clone(),
        [
            report.text_contexts.entries().next().unwrap().clone(),
            other,
        ],
    )
    .unwrap();
    let mut object = report.objects[0].clone();
    object.text_context = Some(catalog.reference(1).unwrap());
    assert!(matches!(
        object.validate_text_context(&catalog),
        Err(AgentTextContextError::ObjectOwner { .. })
    ));
    for mutation in 0..8 {
        let mut object = report.objects[0].clone();
        let element = object.rich_text_ref.as_mut().unwrap();
        match mutation {
            0 => element.index = 1,
            1 => element.range = RichTextRange::new(0, 4),
            2 => element.node_index = RichTextNodeIndex::new(1),
            3 => element.page = 1,
            4 => element.orientation = Some(AgentGlyphOrientation::Upright),
            5 => element.vertical_form = Some(AgentGlyphVerticalForm::None),
            6 => element.hit_regions[0].range = RichTextRange::new(1, 4),
            _ => element.hit_regions[0].kind = AgentHitRegionKind::TextObjectProxy,
        }
        assert!(
            object.validate_text_context(&report.text_contexts).is_err(),
            "element mutation {mutation}"
        );
    }
}

#[test]
fn glyph_orientation_and_hit_regions_are_bound_to_the_issued_layout() {
    let report = crate::tests::test_serialization_observation_report();
    let mut object = report.objects[0].clone();
    let element = object.rich_text_ref.as_mut().unwrap();
    element.kind = AgentRichTextElementKind::TextGlyph;
    element.orientation = Some(AgentGlyphOrientation::Upright);
    element.vertical_form = Some(AgentGlyphVerticalForm::None);
    element.hit_regions[0].kind = AgentHitRegionKind::TextGlyph;
    let entry = report.text_contexts.entries().next().unwrap();
    object.id = entry.element_id(element).unwrap();
    object.parent_id = entry.element_parent_id(Some(element)).unwrap();
    object.capture_refs.captures.clear();
    object.validate_text_context(&report.text_contexts).unwrap();
    let mut changed = object.clone();
    changed.rich_text_ref.as_mut().unwrap().orientation = Some(AgentGlyphOrientation::SidewaysCw);
    assert!(matches!(
        changed.validate_text_context(&report.text_contexts),
        Err(AgentTextContextError::ObjectRange { .. })
    ));
    let mut changed = object;
    changed.rich_text_ref.as_mut().unwrap().hit_regions[0].range = RichTextRange::new(0, 4);
    assert!(matches!(
        changed.validate_text_context(&report.text_contexts),
        Err(AgentTextContextError::ObjectRange { .. })
    ));
}

#[test]
fn hit_and_standalone_image_envelopes_preserve_issued_context_and_reject_detached_evidence() {
    let mut report = crate::tests::test_serialization_observation_report();
    let mut object = report.objects.remove(0);
    let element = object.rich_text_ref.as_mut().unwrap();
    element.kind = AgentRichTextElementKind::TextPage;
    element.source = None;
    element.hit_test = true;
    element.hit_regions[0].kind = crate::rich_text::AgentHitRegionKind::TextPage;
    let entry = report.text_contexts.entries().next().unwrap();
    object.id = entry
        .element_id(object.rich_text_ref.as_ref().unwrap())
        .unwrap();
    object.parent_id = entry
        .element_parent_id(object.rich_text_ref.as_ref())
        .unwrap();
    object.capture_refs.captures.clear();
    object.validate_text_context(&report.text_contexts).unwrap();
    let hit = AgentHitTestHit {
        rank: 0,
        object_id: object.id.clone(),
        object: AgentImageObjectRef::from_observed(&object),
        layer: object.layer.clone(),
        role: object.role.clone(),
        text: object.text.clone(),
        bbox: object.bbox.clone(),
        polygon: object.polygon.clone(),
        capture_refs: object.capture_refs.clone(),
        region: object.rich_text_ref.as_ref().unwrap().hit_regions[0].clone(),
        rich_text_ref: object.rich_text_ref.clone(),
        depth: None,
    };
    let hits = AgentHitTestReport {
        status: "ok".to_owned(),
        session_id: report.session_id.clone(),
        frame_id: report.frame_id.clone(),
        source: report.source.clone(),
        viewport: AgentViewport {
            width: 10,
            height: 20,
            scale: 1.0,
        },
        x: 2,
        y: 3,
        top_object_id: Some(object.id.clone()),
        hits: vec![hit],
        text_contexts: report.text_contexts.clone(),
    };
    assert_eq!(
        AgentHitTestReport::decode_json(&serde_json::to_vec(&hits).unwrap()).unwrap(),
        hits
    );
    let mut detached = serde_json::to_value(&hits).unwrap();
    detached["hits"][0]["text"] = serde_json::json!("changed");
    assert!(AgentHitTestReport::decode_json(&serde_json::to_vec(&detached).unwrap()).is_err());
    let image = AgentImageResource {
        kind: AgentImageKind::Mask,
        renderer: AgentImageRenderer::Native,
        scope: AgentImageScope::Object {
            id: object.id.clone(),
        },
        composition: AgentImageComposition::MaskAttachment,
        page: 0,
        capture_step: report.tick + 1,
        capture_time_millis: 0,
        uri: format!(
            "arcweft://session/{}/frame/{}/object.{}.mask.rgba",
            report.session_id, report.tick, object.id
        ),
        mime_type: "application/octet-stream".to_owned(),
        pixel_format: Some(crate::image::AgentImagePixelFormat::Rgba8Unorm),
        width: object.bbox.width,
        height: object.bbox.height,
        hash: "image.fixture".to_owned(),
        crop_origin: None,
        content_bbox: None,
        content_viewport_bbox: None,
        content_pixels: Some(1),
        object: Some(AgentImageObjectRef::from_observed(&object)),
        view: None,
        selected_capture: None,
        diagnostics: Vec::new(),
        written: None,
    };
    let resource = report
        .image_resource(
            &image,
            AgentBinaryResourceBody {
                encoding: AgentBinaryEncoding::Base64,
                data: "AAAAAA==".to_owned(),
            },
        )
        .unwrap();
    let metadata = resource.image.unwrap();
    assert_eq!(metadata.text_contexts.as_ref().unwrap().entries().len(), 1);
    assert_eq!(
        AgentImageMetadata::decode_json(&serde_json::to_vec(&metadata).unwrap()).unwrap(),
        metadata
    );
    let mut wire = serde_json::to_value(&metadata).unwrap();
    wire["capture_step"] = serde_json::json!(999);
    assert!(AgentImageMetadata::decode_json(&serde_json::to_vec(&wire).unwrap()).is_err());
    for keep_catalog in [false, true] {
        let mut detached = metadata.clone();
        let descriptor = detached.object.as_mut().unwrap();
        descriptor.text_context = None;
        descriptor.rich_text_ref = None;
        if !keep_catalog {
            detached.text_contexts = None;
        }
        assert!(matches!(
            detached.validate_text_contexts(),
            Err(AgentTextContextError::MissingObjectContext { .. })
        ));
        let mut wire = serde_json::to_value(&metadata).unwrap();
        wire["object"] = serde_json::to_value(&detached.object).unwrap();
        if !keep_catalog {
            wire.as_object_mut().unwrap().remove("text_contexts");
        }
        assert!(AgentImageMetadata::decode_json(&serde_json::to_vec(&wire).unwrap()).is_err());
    }
    for change in 0..2 {
        let mut detached = metadata.clone();
        let descriptor = detached.object.as_mut().unwrap();
        descriptor.content = if change == 0 {
            crate::object::AgentObservedObjectContent::ViewText(
                crate::object::AgentTextObjectContent::default(),
            )
        } else {
            crate::object::AgentObservedObjectContent::Custom {
                object_type: "forged".to_owned(),
            }
        };
        match (change, detached.validate_text_contexts()) {
            (0, Err(AgentTextContextError::ContentFamily { .. }))
            | (1, Err(AgentTextContextError::ObjectOwner { .. })) => {}
            (change, result) => panic!("standalone descriptor family {change}: {result:?}"),
        }
        let mut wire = serde_json::to_value(&metadata).unwrap();
        wire["object"] = serde_json::to_value(&detached.object).unwrap();
        assert!(AgentImageMetadata::decode_json(&serde_json::to_vec(&wire).unwrap()).is_err());
    }
    let mut scrubbed = metadata;
    scrubbed.scrub_for_external_publication("opaque");
    assert!(scrubbed.text_contexts.is_none());
    assert!(scrubbed.object.is_none());
    serde_json::to_vec(&scrubbed).unwrap();
}

fn issued_root_hit_fixture(document: bool) -> AgentHitTestReport {
    let mut report = crate::tests::test_serialization_observation_report();
    let mut row = report.text_contexts.entries().next().unwrap().clone();
    if document {
        row.owner.kind = PreparedTextOwnerKind::DialogueView {
            dialogue: 0,
            entry: 0,
            mount: 0,
            role: DialoguePreparedTextRole::CharacterDisplayName,
        };
        row.content = AgentTextContextContent::PreparedDocument {
            context: PreparedTextDocumentContext::try_new(0, 5).unwrap(),
            text: "Hello".to_owned(),
            layout: Arc::clone(row.content.layout()),
        };
    }
    let mut other = row.clone();
    other.owner_ordinal = 1;
    other.owner.root_id = "object.dialogue.other".to_owned();
    other.owner.prepared_text = 1;
    other.owner.kind = PreparedTextOwnerKind::DialogueView {
        dialogue: 0,
        entry: 0,
        mount: 1,
        role: if document {
            DialoguePreparedTextRole::CharacterDisplayName
        } else {
            DialoguePreparedTextRole::Content
        },
    };
    let catalog = AgentTextContextCatalog::try_new(
        report.text_contexts.scope().clone(),
        [row.clone(), other],
    )
    .unwrap();
    let mut object = report.objects.remove(0);
    object.id = row.owner.root_id;
    object.parent_id = row.owner.parent_id;
    object.text_context = Some(catalog.reference(0).unwrap());
    object.rich_text_ref = None;
    object.object_layer = Some("hud.foreground".to_owned());
    object.object_depth = Some(2500);
    object.capture_refs.captures.clear();
    if document {
        object.content = crate::object::AgentObservedObjectContent::ViewText(
            crate::object::AgentTextObjectContent::default(),
        );
    }
    object.validate_text_context(&catalog).unwrap();
    let hit = AgentHitTestHit {
        rank: 0,
        object_id: object.id.clone(),
        object: AgentImageObjectRef::from_observed(&object),
        layer: object.resolved_object_layer().unwrap(),
        role: object.role.clone(),
        text: object.text.clone(),
        bbox: object.bbox.clone(),
        polygon: object.polygon.clone(),
        capture_refs: object.capture_refs.clone(),
        region: AgentHitRegion::for_object(
            &object.bbox,
            object.object_layer.as_deref(),
            object.object_depth,
        ),
        rich_text_ref: None,
        depth: object.object_depth,
    };
    AgentHitTestReport {
        status: "ok".to_owned(),
        session_id: report.session_id,
        frame_id: report.frame_id,
        source: report.source,
        viewport: report.viewport,
        x: 2,
        y: 3,
        top_object_id: Some(object.id),
        hits: vec![hit],
        text_contexts: catalog,
    }
}

#[test]
fn issued_document_and_dialogue_roots_accept_exact_object_hits_and_reject_forged_evidence() {
    for document in [false, true] {
        let report = issued_root_hit_fixture(document);
        let wire = serde_json::to_value(&report).unwrap();
        assert_eq!(
            AgentHitTestReport::decode_json(&serde_json::to_vec(&wire).unwrap()).unwrap(),
            report
        );
        assert!(report.hits[0].rich_text_ref.is_none());
        for change in 0..14 {
            let mut forged = report.clone();
            let hit = &mut forged.hits[0];
            match change {
                0 => hit.object_id = "object.foreign".to_owned(),
                1 => hit.region.kind = AgentHitRegionKind::TextRun,
                2 => hit.region.bbox.x += 1,
                3 => hit.region.range = RichTextRange::new(0, 1),
                4 => hit.region.proxy_id = Some("forged.proxy".to_owned()),
                5 => hit.object.text_context = Some(report.text_contexts.reference(1).unwrap()),
                6 => {
                    let mut reference = serde_json::to_value(&hit.object.text_context).unwrap();
                    reference["scope"]["tick"] = serde_json::json!(2);
                    hit.object.text_context = serde_json::from_value(reference).unwrap();
                }
                7 => {
                    hit.object.text = Some("changed".to_owned());
                    hit.text = hit.object.text.clone();
                }
                8 => {
                    hit.polygon.pop();
                }
                9 => hit.capture_refs.object_id_color.red ^= 1,
                10 => hit.depth = Some(2501),
                11 => hit.object.id.push_str(".run.0"),
                12 => hit.object.parent_id = Some("view.foreign".to_owned()),
                _ => hit.object.text_context = None,
            }
            match (change, forged.validate_text_contexts()) {
                (6, Err(AgentTextContextError::ScopeMismatch))
                | (7, Err(AgentTextContextError::ObjectRange { .. })) => {}
                (13, Err(AgentTextContextError::MissingObjectContext { .. })) => {}
                (0..=5 | 8..=12, Err(AgentTextContextError::ObjectOwner { .. })) => {}
                (change, result) => panic!("root hit mutation {change}: {result:?}"),
            }
            let mut changed_wire = wire.clone();
            changed_wire["hits"] = serde_json::to_value(&forged.hits).unwrap();
            assert!(
                AgentHitTestReport::decode_json(&serde_json::to_vec(&changed_wire).unwrap())
                    .is_err(),
                "root hit mutation {change} must fail checked decoding"
            );
        }
        for old_shape in [false, true] {
            let mut detached = wire.clone();
            let descriptor = detached["hits"][0]["object"].as_object_mut().unwrap();
            if old_shape {
                descriptor.insert("image_ref".to_owned(), serde_json::Value::Null);
            } else {
                descriptor.remove("content");
            }
            assert!(
                AgentHitTestReport::decode_json(&serde_json::to_vec(&detached).unwrap()).is_err()
            );
        }
    }
}

#[test]
fn issued_child_hit_cannot_bypass_its_element_reference_with_a_root_region() {
    let mut report = issued_root_hit_fixture(false);
    let mut source = crate::tests::test_serialization_observation_report();
    let mut object = source.objects.remove(0);
    object.rich_text_ref.as_mut().unwrap().hit_test = true;
    object.capture_refs.captures.clear();
    let hit = AgentHitTestHit {
        rank: 0,
        object_id: object.id.clone(),
        object: AgentImageObjectRef::from_observed(&object),
        layer: object.layer.clone(),
        role: object.role.clone(),
        text: object.text.clone(),
        bbox: object.bbox.clone(),
        polygon: object.polygon.clone(),
        capture_refs: object.capture_refs.clone(),
        region: object.rich_text_ref.as_ref().unwrap().hit_regions[0].clone(),
        rich_text_ref: object.rich_text_ref.clone(),
        depth: None,
    };
    report.top_object_id = Some(object.id);
    report.hits = vec![hit];
    let wire = serde_json::to_value(&report).unwrap();
    assert_eq!(
        AgentHitTestReport::decode_json(&serde_json::to_vec(&wire).unwrap()).unwrap(),
        report
    );
    for remove_references in 0..3 {
        let mut forged = report.clone();
        let hit = &mut forged.hits[0];
        hit.rich_text_ref = None;
        hit.region = AgentHitRegion::for_object(
            &hit.object.bbox,
            hit.object.object_layer.as_deref(),
            hit.object.object_depth,
        );
        if remove_references > 0 {
            hit.object.rich_text_ref = None;
        }
        if remove_references == 2 {
            hit.object.text_context = None;
        }
        match (remove_references, forged.validate_text_contexts()) {
            (0 | 1, Err(AgentTextContextError::ObjectOwner { .. }))
            | (2, Err(AgentTextContextError::MissingObjectContext { .. })) => {}
            (change, result) => panic!("child reference removal {change}: {result:?}"),
        }
        let mut changed_wire = wire.clone();
        changed_wire["hits"] = serde_json::to_value(&forged.hits).unwrap();
        assert!(
            AgentHitTestReport::decode_json(&serde_json::to_vec(&changed_wire).unwrap()).is_err()
        );
    }
    let mut disabled = report;
    disabled.hits[0].rich_text_ref.as_mut().unwrap().hit_test = false;
    disabled.hits[0]
        .object
        .rich_text_ref
        .as_mut()
        .unwrap()
        .hit_test = false;
    assert!(matches!(
        disabled.validate_text_contexts(),
        Err(AgentTextContextError::ObjectOwner { .. })
    ));
    let mut changed_wire = wire;
    changed_wire["hits"] = serde_json::to_value(&disabled.hits).unwrap();
    assert!(AgentHitTestReport::decode_json(&serde_json::to_vec(&changed_wire).unwrap()).is_err());
}

#[test]
fn catalog_rejects_scope_only_canonical_wire_overflow_before_publication() {
    let scope = AgentTextContextScope {
        session_id: "s".repeat(MAX_AGENT_TEXT_CONTEXT_WIRE_BYTES),
        frame_id: "f".to_owned(),
        tick: 0,
        capture_time_millis: None,
    };
    assert!(matches!(
        AgentTextContextCatalog::empty(scope),
        Err(AgentTextContextError::Limit {
            what: "wire bytes",
            limit: MAX_AGENT_TEXT_CONTEXT_WIRE_BYTES,
        })
    ));
}

#[test]
fn catalog_canonical_wire_budget_counts_escaping_and_full_envelope_overhead() {
    let base = AgentTextContextCatalog::empty(AgentTextContextScope {
        session_id: "s".to_owned(),
        frame_id: "f".to_owned(),
        tick: 0,
        capture_time_millis: None,
    })
    .unwrap();
    let base_bytes = serde_json::to_vec(&base).unwrap();
    let session_wire_bytes = MAX_AGENT_TEXT_CONTEXT_WIRE_BYTES - (base_bytes.len() - 1);
    let mut scope = base.scope().clone();
    scope.session_id = format!(
        "{}{}",
        "\0".repeat(session_wire_bytes / 6),
        "s".repeat(session_wire_bytes % 6)
    );
    assert!(scope.session_id.len() < MAX_AGENT_TEXT_CONTEXT_WIRE_BYTES);
    let catalog = AgentTextContextCatalog::empty(scope).unwrap();
    let bytes = serde_json::to_vec(&catalog).unwrap();
    assert_eq!(bytes.len(), MAX_AGENT_TEXT_CONTEXT_WIRE_BYTES);
    let decoded = AgentTextContextCatalog::decode_json(&bytes).unwrap();
    assert_eq!(decoded, catalog);
    drop(decoded);
    drop(bytes);

    let mut larger = catalog.scope().clone();
    larger.session_id.push('s');
    assert!(matches!(
        AgentTextContextCatalog::empty(larger),
        Err(AgentTextContextError::Limit {
            what: "wire bytes",
            limit: MAX_AGENT_TEXT_CONTEXT_WIRE_BYTES,
        })
    ));

    // A catalogue at its exact boundary is legal on its own. Nesting it adds
    // record overhead, so the Objects envelope must reject before publication.
    let objects = AgentObjectsReport {
        text_contexts: catalog,
        objects: Vec::new(),
    };
    assert!(matches!(
        objects.validate_text_contexts(),
        Err(AgentTextContextError::Limit {
            what: "wire bytes",
            limit: MAX_AGENT_TEXT_CONTEXT_WIRE_BYTES,
        })
    ));
    let mut output = Vec::new();
    assert!(serde_json::to_writer(&mut output, &objects).is_err());
    assert!(
        output.is_empty(),
        "failed admission must not emit a JSON prefix"
    );
}

#[test]
fn canonical_wire_quota_counts_all_outer_records_before_serialization() {
    fn rejects_without_emitting_bytes(value: &impl Serialize) {
        let mut output = Vec::new();
        let error = serde_json::to_writer(&mut output, value).unwrap_err();
        assert!(error.to_string().contains("wire bytes"));
        assert!(
            output.is_empty(),
            "failed admission must not emit a JSON prefix"
        );
    }

    {
        let mut report = crate::tests::test_serialization_observation_report();
        report.source = "s".repeat(MAX_AGENT_TEXT_CONTEXT_WIRE_BYTES);
        assert!(matches!(
            report.validate_text_contexts(),
            Err(AgentTextContextError::Limit {
                what: "wire bytes",
                limit: MAX_AGENT_TEXT_CONTEXT_WIRE_BYTES,
            })
        ));
        rejects_without_emitting_bytes(&report);
        assert_eq!(report.source.len(), MAX_AGENT_TEXT_CONTEXT_WIRE_BYTES);
    }
    {
        let source = crate::tests::test_serialization_observation_report();
        let mut objects = AgentObjectsReport {
            text_contexts: source.text_contexts,
            objects: source.objects,
        };
        objects.objects[0].entity = Some("e".repeat(MAX_AGENT_TEXT_CONTEXT_WIRE_BYTES));
        assert!(matches!(
            objects.validate_text_contexts(),
            Err(AgentTextContextError::Limit {
                what: "wire bytes",
                limit: MAX_AGENT_TEXT_CONTEXT_WIRE_BYTES,
            })
        ));
        rejects_without_emitting_bytes(&objects);
        assert_eq!(
            objects.objects[0].entity.as_ref().unwrap().len(),
            MAX_AGENT_TEXT_CONTEXT_WIRE_BYTES
        );
    }
    {
        let mut hits = issued_root_hit_fixture(false);
        hits.source = "s".repeat(MAX_AGENT_TEXT_CONTEXT_WIRE_BYTES);
        assert!(matches!(
            hits.validate_text_contexts(),
            Err(AgentTextContextError::Limit {
                what: "wire bytes",
                limit: MAX_AGENT_TEXT_CONTEXT_WIRE_BYTES,
            })
        ));
        rejects_without_emitting_bytes(&hits);
        assert_eq!(hits.source.len(), MAX_AGENT_TEXT_CONTEXT_WIRE_BYTES);
    }
    {
        let report = crate::tests::test_serialization_observation_report();
        let mut metadata =
            AgentImageMetadata::from_image_resource(&report.images[0], None).unwrap();
        metadata
            .diagnostics
            .push(crate::diagnostic::AgentDiagnostic {
                step: 0,
                severity: crate::diagnostic::AgentDiagnosticSeverity::Warning,
                source: None,
                code: None,
                effect_id: None,
                message: "m".repeat(MAX_AGENT_TEXT_CONTEXT_WIRE_BYTES),
            });
        assert!(matches!(
            metadata.validate_text_contexts(),
            Err(AgentTextContextError::Limit {
                what: "wire bytes",
                limit: MAX_AGENT_TEXT_CONTEXT_WIRE_BYTES,
            })
        ));
        rejects_without_emitting_bytes(&metadata);
        assert_eq!(
            metadata.diagnostics[0].message.len(),
            MAX_AGENT_TEXT_CONTEXT_WIRE_BYTES
        );
    }
}

#[test]
fn compact_canonical_envelopes_roundtrip_checked_contexts_and_typed_values() {
    let report = crate::tests::test_serialization_observation_report();
    let catalog = report.text_contexts.clone();
    let reference = catalog.reference(0).unwrap();
    let selected = catalog.selected([&reference]).unwrap();
    let bytes = serde_json::to_vec(&selected).unwrap();
    assert_eq!(
        AgentTextContextCatalog::decode_json(&bytes).unwrap(),
        selected
    );
    let bytes = serde_json::to_vec(&report).unwrap();
    assert_eq!(AgentObservationReport::decode_json(&bytes).unwrap(), report);

    let objects = AgentObjectsReport {
        text_contexts: catalog,
        objects: report.objects.clone(),
    };
    let bytes = serde_json::to_vec(&objects).unwrap();
    assert_eq!(AgentObjectsReport::decode_json(&bytes).unwrap(), objects);
    let hits = issued_root_hit_fixture(false);
    let bytes = serde_json::to_vec(&hits).unwrap();
    assert_eq!(AgentHitTestReport::decode_json(&bytes).unwrap(), hits);
    let metadata = AgentImageMetadata::from_image_resource(&report.images[0], None).unwrap();
    let bytes = serde_json::to_vec(&metadata).unwrap();
    assert_eq!(AgentImageMetadata::decode_json(&bytes).unwrap(), metadata);
}

#[test]
fn catalog_decode_is_bounded_and_accepts_only_version_one_checked_layouts() {
    let catalog = crate::tests::test_text_contexts(1, "object.fixture");
    let mut wire = serde_json::to_value(&catalog).unwrap();
    wire["version"] = serde_json::json!(2);
    assert!(AgentTextContextCatalog::decode_json(&serde_json::to_vec(&wire).unwrap()).is_err());
    let mut wire = serde_json::to_value(&catalog).unwrap();
    wire["entries"][0]["content"]["layout"]["glyphs"][0]["source_range"]["end"] =
        serde_json::json!(99);
    assert!(AgentTextContextCatalog::decode_json(&serde_json::to_vec(&wire).unwrap()).is_err());
    let mut row = catalog.entries().next().unwrap().clone();
    row.owner_ordinal = u32::try_from(MAX_AGENT_TEXT_CONTEXTS).unwrap();
    assert!(matches!(
        AgentTextContextCatalog::try_new(catalog.scope().clone(), [row]),
        Err(AgentTextContextError::Limit {
            what: "context count",
            ..
        })
    ));
    let bytes = vec![b' '; MAX_AGENT_TEXT_CONTEXT_WIRE_BYTES + 1];
    assert!(matches!(
        AgentTextContextCatalog::decode_json(&bytes),
        Err(AgentTextContextError::Limit {
            what: "wire bytes",
            ..
        })
    ));
    assert!(matches!(
        AgentObjectsReport::decode_json(&bytes),
        Err(AgentTextContextError::Limit {
            what: "wire bytes",
            ..
        })
    ));
    assert!(matches!(
        AgentHitTestReport::decode_json(&bytes),
        Err(AgentTextContextError::Limit {
            what: "wire bytes",
            ..
        })
    ));
    assert!(matches!(
        AgentImageMetadata::decode_json(&bytes),
        Err(AgentTextContextError::Limit {
            what: "wire bytes",
            ..
        })
    ));
}

#[test]
fn catalog_admission_bounds_aggregate_text_and_elements_before_projection() {
    let catalog = crate::tests::test_text_contexts(1, "object.fixture");
    let mut row = catalog.entries().next().unwrap().clone();
    let text = "x".repeat(MAX_AGENT_TEXT_CONTEXT_TEXT_BYTES + 1);
    row.owner.kind = PreparedTextOwnerKind::View { mount: 1 };
    row.content = AgentTextContextContent::PreparedDocument {
        context: PreparedTextDocumentContext::try_new(0, text.len()).unwrap(),
        text,
        layout: Arc::clone(row.content.layout()),
    };
    assert!(matches!(
        AgentTextContextCatalog::try_new(catalog.scope().clone(), [row]),
        Err(AgentTextContextError::Limit {
            what: "text bytes",
            ..
        })
    ));
    let mut row = catalog.entries().next().unwrap().clone();
    let mut layout = row.content.layout().as_ref().clone();
    layout.lines = vec![layout.lines[0].clone(); MAX_AGENT_TEXT_CONTEXT_ELEMENTS + 1];
    match &mut row.content {
        AgentTextContextContent::DisplayStage { layout: owned, .. } => *owned = Arc::new(layout),
        AgentTextContextContent::PreparedDocument { .. } => unreachable!(),
    }
    assert!(matches!(
        AgentTextContextCatalog::try_new(catalog.scope().clone(), [row]),
        Err(AgentTextContextError::Limit {
            what: "elements",
            ..
        })
    ));
}

// Legal transported layout: a two-glyph grapheme and two distinct vertical
// cluster ordinals sharing one atomic punctuation range.
fn body_capture_catalog() -> AgentTextContextCatalog {
    use arcweft_text_layout::{
        LayoutPoint, LayoutRect, TextLayoutBodyAllocation, TextLayoutGlyphSource, TextLayoutRuby,
        TextLayoutSourceMap,
    };
    use arcweft_text_model::{RichTextNodeRange, RichTextTextRunRange};
    let original = crate::tests::test_text_contexts(12, "object.capture.root");
    let mut row = original.entries().next().unwrap().clone();
    let mut layout = row.content.layout().as_ref().clone();
    let text = "e\u{301}――".to_owned();
    let ranges = [
        RichTextRange::new(0, 3),
        RichTextRange::new(0, 3),
        RichTextRange::new(3, 9),
        RichTextRange::new(3, 9),
    ];
    layout.glyphs = [0, 0, 1, 2]
        .into_iter()
        .enumerate()
        .map(|(index, cluster)| {
            let mut glyph = layout.glyphs[0].clone();
            glyph.source_range = ranges[index];
            glyph.cluster_index = cluster;
            glyph.logical_ordinal = cluster;
            glyph.origin =
                LayoutPoint::new(1.0 + f32::from(u16::try_from(cluster).unwrap()) * 10.0, 2.0);
            glyph.layout_bounds = LayoutRect::new(glyph.origin.x, 2.0, 10.0, 10.0);
            glyph.ink_bounds = glyph.layout_bounds;
            glyph
        })
        .collect();
    let whole = RichTextRange::new(0, text.len());
    let bounds = LayoutRect::new(1.0, 2.0, 30.0, 10.0);
    layout.lines[0].source_range = whole;
    layout.lines[0].glyph_range = 0..4;
    layout.lines[0].bounds = bounds;
    layout.runs[0].source_range = whole;
    layout.runs[0].glyph_range = 0..4;
    layout.runs[0].bounds = bounds;
    layout.runs[0].presentation = Default::default();
    layout.bounds = Some(bounds);
    layout.source_map = TextLayoutSourceMap::new(
        layout
            .glyphs
            .iter()
            .map(|glyph| TextLayoutGlyphSource {
                run_index: glyph.run_index,
                source_range: glyph.source_range,
                line_index: glyph.line_index,
                cluster_index: glyph.cluster_index,
                logical_ordinal: glyph.logical_ordinal,
            })
            .collect(),
    );
    layout.ruby = [
        (7, RichTextRange::new(0, 3), "é", 1.0),
        (8, RichTextRange::new(3, 9), "だっしゅ", 11.0),
    ]
    .into_iter()
    .map(|(index, base_range, text, x)| TextLayoutRuby {
        ruby_index: index,
        owner_node: RichTextNodeIndex::new(index),
        body_nodes: RichTextNodeRange::new(
            RichTextNodeIndex::new(index + 1),
            RichTextNodeIndex::new(index + 2),
        ),
        base_runs: RichTextTextRunRange::new(0, 1),
        base_range,
        base_allocations: if index == 7 {
            vec![TextLayoutBodyAllocation {
                glyph_range: 0..2,
                bounds: LayoutRect::new(1.0, 2.0, 10.0, 10.0),
            }]
        } else {
            vec![
                TextLayoutBodyAllocation {
                    glyph_range: 2..3,
                    bounds: LayoutRect::new(11.0, 2.0, 10.0, 10.0),
                },
                TextLayoutBodyAllocation {
                    glyph_range: 3..4,
                    bounds: LayoutRect::new(21.0, 2.0, 10.0, 10.0),
                },
            ]
        },
        text: text.to_owned(),
        base_bounds: LayoutRect::new(x, 2.0, if index == 7 { 10.0 } else { 20.0 }, 10.0),
        ruby_bounds: LayoutRect::new(x, 0.0, if index == 7 { 10.0 } else { 20.0 }, 2.0),
        glyphs: Vec::new(),
        writing_mode: arcweft_text_model::RichTextWritingMode::HorizontalTb,
        style: Default::default(),
        presentation: Default::default(),
    })
    .collect();
    row.owner.kind = PreparedTextOwnerKind::View { mount: 0 };
    row.owner.semantic_id = "view.capture.root".to_owned();
    row.content = AgentTextContextContent::PreparedDocument {
        context: PreparedTextDocumentContext::try_new(0, text.len()).unwrap(),
        text,
        layout: Arc::new(layout),
    };
    let mut other = row.clone();
    other.owner_ordinal = 1;
    other.owner.prepared_text = 1;
    other.owner.kind = PreparedTextOwnerKind::View { mount: 1 };
    other.owner.root_id = "object.capture.foreign".to_owned();
    other.owner.semantic_id = "view.capture.foreign".to_owned();
    AgentTextContextCatalog::try_new(original.scope().clone(), [row, other]).unwrap()
}

fn body_capture_element(
    catalog: &AgentTextContextCatalog,
    kind: AgentRichTextElementKind,
    index: usize,
) -> AgentRichTextElementRef {
    let row = catalog.resolve(&catalog.reference(0).unwrap()).unwrap();
    let (range, node, ruby) = match kind {
        AgentRichTextElementKind::TextGlyph => (
            row.content.layout().glyphs[index].source_range,
            RichTextNodeIndex::new(0),
            None,
        ),
        AgentRichTextElementKind::GlyphCluster => (
            row.content
                .layout()
                .cluster_glyphs(u32::try_from(index).unwrap())
                .unwrap()[0]
                .source_range,
            RichTextNodeIndex::new(0),
            None,
        ),
        AgentRichTextElementKind::Ruby => {
            let ruby = row
                .content
                .layout()
                .ruby
                .iter()
                .find(|ruby| usize::try_from(ruby.ruby_index).ok() == Some(index))
                .unwrap();
            (ruby.base_range, ruby.owner_node, Some(ruby.text.clone()))
        }
        _ => panic!("fixture admits only glyphs, clusters and Ruby"),
    };
    let body = kind != AgentRichTextElementKind::Ruby;
    AgentRichTextElementRef {
        kind,
        index,
        page: 0,
        range,
        node_index: node,
        source: None,
        ruby,
        presentation: Some(Default::default()),
        orientation: body.then_some(AgentGlyphOrientation::Upright),
        vertical_form: body.then_some(AgentGlyphVerticalForm::None),
        ruby_base_bbox: None,
        ruby_annotation_bbox: None,
        object_layer: None,
        object_depth: None,
        hit_test: false,
        hit_regions: Vec::new(),
    }
}

fn body_capture_object(
    catalog: &AgentTextContextCatalog,
    owner: u32,
    element: AgentRichTextElementRef,
) -> crate::object::AgentObservedObject {
    use crate::geometry::{AgentBBox, AgentCoordinateSpace, AgentRgbaColor};
    use crate::image::AgentObjectCaptureRefs;
    use crate::object::{AgentObservedObject, AgentObservedObjectContent, AgentTextObjectContent};
    let reference = catalog.reference(owner).unwrap();
    let row = catalog.resolve(&reference).unwrap();
    let bbox = AgentBBox {
        space: AgentCoordinateSpace::Viewport,
        x: 1,
        y: 2,
        width: 10,
        height: 10,
    };
    let base = &row.content.text()[element.range.start..element.range.end];
    let text = element
        .ruby
        .as_ref()
        .map_or_else(|| base.to_owned(), |ruby| format!("{base} ({ruby})"));
    let object = AgentObservedObject {
        id: row.element_id(&element).unwrap(),
        parent_id: row.element_parent_id(Some(&element)).unwrap(),
        entity: None,
        layer: "view.text".to_owned(),
        role: "text_element".to_owned(),
        visible: true,
        enabled: true,
        polygon: bbox.polygon(),
        bbox,
        capture_refs: AgentObjectCaptureRefs {
            object_id_color: AgentRgbaColor {
                red: 1,
                green: 2,
                blue: 3,
                alpha: 255,
            },
            captures: Vec::new(),
        },
        object_layer: None,
        object_depth: None,
        text: Some(text),
        text_context: Some(reference),
        rich_text_ref: Some(element),
        content: AgentObservedObjectContent::ViewText(AgentTextObjectContent::default()),
    };
    object.validate_text_context(catalog).unwrap();
    object
}

#[test]
fn body_capture_uses_actual_singleton_and_multiglyph_cluster_ordinals() {
    let catalog = body_capture_catalog();
    let row = catalog.resolve(&catalog.reference(0).unwrap()).unwrap();
    let multi = body_capture_element(&catalog, AgentRichTextElementKind::GlyphCluster, 0);
    let selection = row.capture_body(&multi).unwrap();
    assert!(selection.contains_glyph(0));
    assert!(selection.contains_glyph(1));
    assert!(!selection.contains_glyph(2));
    assert!(!selection.contains_glyph(3));
    assert!(selection.contains_cluster(0));
    assert!(!selection.contains_cluster(1));
    let physical = body_capture_element(&catalog, AgentRichTextElementKind::TextGlyph, 1);
    let physical_selection = row.capture_body(&physical).unwrap();
    assert!(!physical_selection.contains_glyph(0));
    assert!(physical_selection.contains_glyph(1));
    assert!(physical_selection.contains_cluster(0));
    assert!(!physical_selection.contains_cluster(1));
    let left = body_capture_element(&catalog, AgentRichTextElementKind::GlyphCluster, 1);
    let right = body_capture_element(&catalog, AgentRichTextElementKind::GlyphCluster, 2);
    assert_eq!(left.range, right.range);
    assert_ne!(
        row.element_id(&left).unwrap(),
        row.element_id(&right).unwrap()
    );
    for (element, included, excluded) in [(&left, 2, 3), (&right, 3, 2)] {
        let selection = row.capture_body(element).unwrap();
        assert!(selection.contains_glyph(included));
        assert!(!selection.contains_glyph(excluded));
        assert!(selection.contains_cluster(element.index));
        assert!(!selection.contains_cluster(if element.index == 1 { 2 } else { 1 }));
    }
    let mut forged = right;
    forged.range = RichTextRange::new(6, 9);
    assert!(matches!(
        row.capture_body(&forged),
        Err(AgentTextContextError::ObjectRange { .. })
    ));
}

#[test]
fn ruby_body_capture_joins_only_its_actual_base_partition() {
    let catalog = body_capture_catalog();
    let ruby = body_capture_object(
        &catalog,
        0,
        body_capture_element(&catalog, AgentRichTextElementKind::Ruby, 7),
    );
    let base = body_capture_object(
        &catalog,
        0,
        body_capture_element(&catalog, AgentRichTextElementKind::GlyphCluster, 0),
    );
    let outside = body_capture_object(
        &catalog,
        0,
        body_capture_element(&catalog, AgentRichTextElementKind::GlyphCluster, 1),
    );
    let other_ruby = body_capture_object(
        &catalog,
        0,
        body_capture_element(&catalog, AgentRichTextElementKind::Ruby, 8),
    );
    assert!(
        catalog
            .element_capture(&ruby)
            .and_then(|capture| capture.contains(&base))
            .unwrap()
    );
    assert!(
        !catalog
            .element_capture(&ruby)
            .and_then(|capture| capture.contains(&outside))
            .unwrap()
    );
    assert!(
        !catalog
            .element_capture(&ruby)
            .and_then(|capture| capture.contains(&other_ruby))
            .unwrap()
    );
    assert!(
        !catalog
            .element_capture(&base)
            .and_then(|capture| capture.contains(&ruby))
            .unwrap(),
        "a base cluster never inherits the whole annotation attachment"
    );
    let decoded =
        AgentTextContextCatalog::decode_json(&serde_json::to_vec(&catalog).unwrap()).unwrap();
    assert!(
        decoded
            .element_capture(&ruby)
            .and_then(|capture| capture.contains(&base))
            .unwrap()
    );
    assert!(
        !decoded
            .element_capture(&ruby)
            .and_then(|capture| capture.contains(&outside))
            .unwrap()
    );
}

#[test]
fn capture_membership_rejects_foreign_detached_and_stale_context_evidence() {
    let catalog = body_capture_catalog();
    let element = body_capture_element(&catalog, AgentRichTextElementKind::GlyphCluster, 1);
    let root = body_capture_object(&catalog, 0, element.clone());
    let other = body_capture_object(&catalog, 1, element.clone());
    assert!(
        !catalog
            .element_capture(&root)
            .and_then(|capture| capture.contains(&other))
            .unwrap()
    );
    let mut forged = other;
    forged.text_context = root.text_context.clone();
    assert!(matches!(
        catalog
            .element_capture(&root)
            .and_then(|capture| capture.contains(&forged)),
        Err(AgentTextContextError::ObjectOwner { .. })
    ));
    let mut detached = root.clone();
    detached.text_context = None;
    assert!(matches!(
        catalog
            .element_capture(&root)
            .and_then(|capture| capture.contains(&detached)),
        Err(AgentTextContextError::MissingObjectContext { .. })
    ));
    let mut stale = root.clone();
    let mut wire = serde_json::to_value(stale.text_context.as_ref().unwrap()).unwrap();
    wire["scope"]["capture_time_millis"] = serde_json::json!(501);
    stale.text_context = Some(serde_json::from_value(wire).unwrap());
    assert!(matches!(
        catalog
            .element_capture(&root)
            .and_then(|capture| capture.contains(&stale)),
        Err(AgentTextContextError::ScopeMismatch)
    ));
    let mut alias = root.clone();
    alias.rich_text_ref.as_mut().unwrap().index = 2;
    assert!(matches!(
        catalog
            .element_capture(&root)
            .and_then(|capture| capture.contains(&alias)),
        Err(AgentTextContextError::ObjectOwner { .. })
    ));
}

#[test]
fn ruby_allocation_catalog_rejects_split_atomic_clusters_and_foreign_body_indices() {
    let original = body_capture_catalog();
    for defect in 0..3 {
        let mut rows = original.entries().cloned().collect::<Vec<_>>();
        let AgentTextContextContent::PreparedDocument { layout, .. } = &mut rows[0].content else {
            panic!("owning fixture is an admitted document");
        };
        let layout = Arc::make_mut(layout);
        match defect {
            0 => {
                let allocation = layout.ruby[0].base_allocations[0].clone();
                layout.ruby[0].base_allocations = vec![
                    arcweft_text_layout::TextLayoutBodyAllocation {
                        glyph_range: 0..1,
                        bounds: allocation.bounds,
                    },
                    arcweft_text_layout::TextLayoutBodyAllocation {
                        glyph_range: 1..2,
                        bounds: allocation.bounds,
                    },
                ];
            }
            1 => layout.ruby[0].base_allocations[0].glyph_range = 2..3,
            2 => {
                let duplicate = layout.ruby[0].base_allocations[0].clone();
                layout.ruby[0].base_allocations.push(duplicate);
            }
            _ => unreachable!(),
        }
        let error = AgentTextContextCatalog::try_new(original.scope().clone(), rows).unwrap_err();
        let AgentTextContextError::PreparedDocument { owner, reason } = error else {
            panic!("exact allocation admission failure expected");
        };
        assert_eq!(owner, 0);
        assert_eq!(
            reason,
            arcweft_text_layout::TextLayoutContextError::RubyAllocation { index: 0 }.to_string()
        );
    }
}

#[test]
fn ruby_allocation_wire_is_required_and_charged_to_the_existing_context_budget() {
    let original = body_capture_catalog();
    let mut missing = serde_json::to_value(&original).unwrap();
    missing["entries"][0]["content"]["layout"]["ruby"][0]
        .as_object_mut()
        .unwrap()
        .remove("base_allocations");
    let AgentTextContextError::Json(error) =
        AgentTextContextCatalog::decode_json(&serde_json::to_vec(&missing).unwrap()).unwrap_err()
    else {
        panic!("missing required allocation metadata is a codec error");
    };
    assert!(
        error
            .to_string()
            .contains("missing field \u{60}base_allocations\u{60}")
    );

    let mut row = original.entries().next().unwrap().clone();
    let AgentTextContextContent::PreparedDocument { layout, .. } = &mut row.content else {
        panic!("owning fixture is a document");
    };
    let layout = Arc::make_mut(layout);
    layout.ruby[0].base_allocations =
        vec![layout.ruby[0].base_allocations[0].clone(); MAX_AGENT_TEXT_CONTEXT_ELEMENTS];
    let limit = AgentTextContextError::Limit {
        what: "elements",
        limit: MAX_AGENT_TEXT_CONTEXT_ELEMENTS,
    };
    assert!(matches!(
        AgentTextContextCatalog::try_new(original.scope().clone(), [row.clone()]),
        Err(AgentTextContextError::Limit {
            what: "elements",
            limit: MAX_AGENT_TEXT_CONTEXT_ELEMENTS
        })
    ));
    let wire = serde_json::json!({
        "version": AGENT_TEXT_CONTEXT_CATALOG_VERSION,
        "scope": original.scope(),
        "entries": [row],
    });
    let bytes = serde_json::to_vec(&wire).unwrap();
    assert!(
        bytes.len() < MAX_AGENT_TEXT_CONTEXT_WIRE_BYTES,
        "element quota must reject before the envelope cap"
    );
    let AgentTextContextError::Json(error) =
        AgentTextContextCatalog::decode_json(&bytes).unwrap_err()
    else {
        panic!("wire allocation quota is an owning codec admission failure");
    };
    assert!(error.to_string().contains(&limit.to_string()));
}

#[test]
fn objects_envelope_rejects_duplicate_ids_and_preserves_distinct_shared_context_elements() {
    let catalog = body_capture_catalog();
    let first = body_capture_object(
        &catalog,
        0,
        body_capture_element(&catalog, AgentRichTextElementKind::GlyphCluster, 1),
    );
    let second = body_capture_object(
        &catalog,
        0,
        body_capture_element(&catalog, AgentRichTextElementKind::GlyphCluster, 2),
    );
    assert_ne!(first.id, second.id);
    assert_eq!(first.text, second.text);
    assert_eq!(first.text_context, second.text_context);
    assert_eq!(
        first.rich_text_ref.as_ref().unwrap().range,
        second.rich_text_ref.as_ref().unwrap().range
    );
    let report = AgentObjectsReport {
        text_contexts: catalog,
        objects: vec![first, second],
    };
    report
        .text_contexts
        .validate_objects(&report.objects)
        .unwrap();
    let bytes = serde_json::to_vec(&report).unwrap();
    assert_eq!(AgentObjectsReport::decode_json(&bytes).unwrap(), report);

    let duplicate = report.objects[0].clone();
    let expected = AgentTextContextError::ObjectOwner {
        object_id: duplicate.id.clone(),
    };
    let mut invalid = report.clone();
    invalid.objects.push(duplicate);
    assert!(matches!(
        invalid.text_contexts.validate_objects(&invalid.objects),
        Err(AgentTextContextError::ObjectOwner { object_id }) if object_id == invalid.objects[0].id,
    ));
    let error = serde_json::to_vec(&invalid).unwrap_err();
    assert!(error.to_string().contains(&expected.to_string()));
    let mut wire = serde_json::to_value(&report).unwrap();
    let duplicate = wire["objects"][0].clone();
    wire["objects"].as_array_mut().unwrap().push(duplicate);
    let AgentTextContextError::Json(error) =
        AgentObjectsReport::decode_json(&serde_json::to_vec(&wire).unwrap()).unwrap_err()
    else {
        panic!("duplicate object is refused by owning JSON admission");
    };
    assert!(error.to_string().contains(&expected.to_string()));
}

#[test]
fn observation_and_objects_resources_use_the_same_duplicate_identity_admission() {
    let valid = crate::tests::test_serialization_observation_report();
    let bytes = serde_json::to_vec(&valid).unwrap();
    assert_eq!(AgentObservationReport::decode_json(&bytes).unwrap(), valid);
    let mut invalid = valid.clone();
    let duplicate = invalid.objects[0].clone();
    let expected = AgentTextContextError::ObjectOwner {
        object_id: duplicate.id.clone(),
    };
    invalid.objects.push(duplicate);
    assert!(matches!(
        invalid.validate_text_contexts(),
        Err(AgentTextContextError::ObjectOwner { object_id }) if object_id == invalid.objects[0].id,
    ));
    assert!(
        serde_json::to_vec(&invalid)
            .unwrap_err()
            .to_string()
            .contains(&expected.to_string())
    );
    assert!(
        invalid
            .objects_resource()
            .unwrap_err()
            .to_string()
            .contains(&expected.to_string())
    );

    let mut wire = serde_json::to_value(&valid).unwrap();
    let duplicate = wire["objects"][0].clone();
    wire["objects"].as_array_mut().unwrap().push(duplicate);
    let AgentTextContextError::Json(error) =
        AgentObservationReport::decode_json(&serde_json::to_vec(&wire).unwrap()).unwrap_err()
    else {
        panic!("duplicate object is refused by owning JSON admission");
    };
    assert!(error.to_string().contains(&expected.to_string()));
}
