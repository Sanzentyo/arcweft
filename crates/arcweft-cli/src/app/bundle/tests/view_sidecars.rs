use super::super::{ViewProgramResource, ViewStyleResource, collect_bundle_view_sidecars};
use arcweft_bundle::container::{BundleDigest, SectionId, SectionKindCode};
use arcweft_bundle::resource_codec::view::{
    ViewActionButtonActionResource, ViewActionButtonResource, ViewElementKind,
    ViewProgramInstruction, ViewRuntimeButtonBounds, ViewRuntimeSurfaceBounds, ViewStylePatch,
    ViewStylePatchId, ViewStyleProgram, ViewStyleSheet, ViewStyleSheetId, ViewSurfaceResource,
    ViewTextBlockBounds, ViewTextBlockResource,
};
use arcweft_bundle::resource_codec::{
    CrossSectionRef, ProductSourceRef, SourceMapSection, SourceRangeRef,
};
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};
use std::fs;

fn unique_root(label: &str) -> std::path::PathBuf {
    let unique = format!(
        "arcweft-view-sidecar-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock follows UNIX epoch")
            .as_nanos()
    );
    std::env::temp_dir().join(unique)
}

fn assert_direct_json_sidecar_is_rejected(
    label: &str,
    file_name: &str,
    transcript: &serde_json::Value,
) {
    let root = unique_root(label);
    fs::create_dir_all(&root).expect("sidecar fixture root creates");
    fs::write(
        root.join(file_name),
        serde_json::to_vec(transcript).expect("invalid sidecar transcript encodes"),
    )
    .expect("invalid sidecar writes");

    assert!(
        collect_bundle_view_sidecars(&root).is_err(),
        "direct JSON ingestion must reject {label}"
    );

    let _ = fs::remove_dir_all(root);
}

fn strict_style_resource_fixture() -> ViewStyleResource {
    let document = SourceDocument::try_new(
        SourceDocumentId::try_new("strict-style.arcw").expect("source ID"),
        SourceName::path("strict-style.arcw"),
        "x",
    )
    .expect("source document");
    let source_map = SourceMapSection::try_from_documents(&[&document]).expect("source map");
    let source_refs = source_map
        .documents()
        .map(ProductSourceRef::from_document)
        .collect::<Vec<_>>();
    let source_range =
        SourceRangeRef::try_for_source(&source_refs, &source_refs[0], 0, 1).expect("source range");
    ViewStyleResource {
        style_program_id: "view.style.strict".to_owned(),
        source_refs,
        program: ViewStyleProgram::try_new(
            vec![
                ViewStyleSheet::new(
                    ViewStyleSheetId::try_new("style.strict").expect("sheet ID"),
                    Vec::new(),
                    Vec::new(),
                )
                .expect("empty native sheet is valid"),
            ],
            vec![ViewStylePatch::new(ViewStylePatchId::new(0), Vec::new())],
        )
        .expect("strict native Style program is valid"),
        source_map_refs: vec![source_range],
        adapter_requirements: vec![CrossSectionRef {
            section_kind: SectionKindCode::new(0x5354_594C),
            section_id: SectionId::from_bytes([7; 16]),
            content_digest: BundleDigest::of(b"strict Style adapter"),
            public_id: None,
        }],
    }
}

#[test]
fn direct_json_sidecars_accept_current_view_program_and_style_resources() {
    let root = unique_root("current-shape");
    fs::create_dir_all(&root).expect("sidecar fixture root creates");
    fs::write(
        root.join("view.program.json"),
        serde_json::to_vec(&ViewProgramResource::default()).expect("program sidecar encodes"),
    )
    .expect("program sidecar writes");
    fs::write(
        root.join("view.style.json"),
        serde_json::to_vec(&ViewStyleResource::default()).expect("Style sidecar encodes"),
    )
    .expect("Style sidecar writes");

    let sidecars = collect_bundle_view_sidecars(&root).expect("current sidecars decode");
    assert_eq!(sidecars.program, Some(ViewProgramResource::default()));
    assert_eq!(sidecars.style, Some(ViewStyleResource::default()));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn direct_json_program_sidecar_rejects_unknown_fields_in_d2_product_records() {
    let mut root_record =
        serde_json::to_value(ViewProgramResource::default()).expect("program sidecar encodes");
    root_record
        .as_object_mut()
        .expect("program resource is an object")
        .insert(
            "unexpected_program_field".to_owned(),
            serde_json::json!(true),
        );
    assert_direct_json_sidecar_is_rejected(
        "unknown ViewProgramResource field",
        "view.program.json",
        &root_record,
    );

    let mut definition_record =
        serde_json::to_value(ViewProgramResource::default()).expect("program sidecar encodes");
    definition_record["definitions"] = serde_json::json!([{
        "public_id": "view.strict",
        "body": { "start_instruction": 0, "end_instruction": 0 },
        "styles": [],
        "parameters": [],
        "state_schema_hash": 0,
        "unexpected_definition_field": true
    }]);
    assert_direct_json_sidecar_is_rejected(
        "unknown ViewDefinitionResource field",
        "view.program.json",
        &definition_record,
    );

    let mut exported_part_record =
        serde_json::to_value(ViewProgramResource::default()).expect("program sidecar encodes");
    exported_part_record["exported_parts"] = serde_json::json!([{
        "view": "view.strict",
        "part_id": "part.strict",
        "public_name": "strict",
        "unexpected_exported_part_field": true
    }]);
    assert_direct_json_sidecar_is_rejected(
        "unknown ViewExportedPart field",
        "view.program.json",
        &exported_part_record,
    );
}

#[test]
fn direct_json_program_sidecar_rejects_unknown_fields_on_every_node_producer() {
    let instructions = [
        (
            "OpenElement unknown field",
            ViewProgramInstruction::OpenElement {
                element: ViewElementKind::Panel,
                target: None,
                styles: Vec::new(),
                part: None,
                key: None,
                source: None,
            },
        ),
        (
            "EmitText unknown field",
            ViewProgramInstruction::EmitText {
                text_source: "text.strict".to_owned(),
                text_block: "text.block.strict".to_owned(),
                styles: Vec::new(),
                part: None,
                source: None,
            },
        ),
        (
            "EmitImage unknown field",
            ViewProgramInstruction::EmitImage {
                image: "image.strict".to_owned(),
                target: None,
                styles: Vec::new(),
                part: None,
                source: None,
            },
        ),
        (
            "EmitCustom unknown field",
            ViewProgramInstruction::EmitCustom {
                element: "strict-custom".to_owned(),
                styles: Vec::new(),
                part: None,
                source: None,
            },
        ),
        (
            "CallView unknown field",
            ViewProgramInstruction::CallView {
                view: arcweft_bundle::resource_codec::view::ViewDefinitionRef::try_new(
                    "view.strict.child",
                )
                .unwrap(),
                arguments: Vec::new(),
                styles: Vec::new(),
                part: None,
                key: None,
                source: None,
            },
        ),
    ];

    for (label, instruction) in instructions {
        let mut encoded_instruction =
            serde_json::to_value(instruction).expect("instruction encodes");
        encoded_instruction
            .as_object_mut()
            .expect("instruction is externally tagged")
            .values_mut()
            .next()
            .expect("instruction has one variant payload")
            .as_object_mut()
            .expect("node producer has an object payload")
            .insert(
                "unexpected_instruction_field".to_owned(),
                serde_json::json!(true),
            );

        let mut transcript =
            serde_json::to_value(ViewProgramResource::default()).expect("program sidecar encodes");
        transcript["instructions"] = serde_json::json!([encoded_instruction]);
        assert_direct_json_sidecar_is_rejected(label, "view.program.json", &transcript);
    }
}

#[test]
fn direct_json_style_sidecar_rejects_unknown_native_program_fields() {
    let encoded =
        serde_json::to_value(strict_style_resource_fixture()).expect("Style sidecar encodes");
    for (label, pointer) in [
        ("unknown ViewStyleProgram field", "/program"),
        ("unknown ViewStyleSheet field", "/program/sheets/0"),
        ("unknown ViewStylePatch field", "/program/patches/0"),
        ("unknown Style source map field", "/source_map_refs/0"),
        (
            "unknown Style adapter cross-section field",
            "/adapter_requirements/0",
        ),
    ] {
        let mut transcript = encoded.clone();
        transcript
            .pointer_mut(pointer)
            .unwrap_or_else(|| panic!("missing Style fixture object: {pointer}"))
            .as_object_mut()
            .unwrap_or_else(|| panic!("Style fixture target is not an object: {pointer}"))
            .insert("unexpected_style_field".to_owned(), serde_json::json!(true));
        assert_direct_json_sidecar_is_rejected(label, "view.style.json", &transcript);
    }
}

#[test]
fn direct_json_program_sidecar_rejects_unknown_resource_fields() {
    let action_button = ViewActionButtonResource {
        public_id: "button.strict".to_owned(),
        view: None,
        containing_scroll_region: None,
        label_text_source: "text.button.label".to_owned(),
        enabled: true,
        action: ViewActionButtonActionResource::Noop,
        bounds: ViewRuntimeButtonBounds {
            x_milli: 0,
            y_milli: 0,
            width_milli: 1_000,
            height_milli: 1_000,
        },
        source: None,
    };
    let text_block = ViewTextBlockResource::new(
        "text.strict",
        None,
        None,
        "text.body",
        ViewTextBlockBounds {
            x_milli: 0,
            y_milli: 0,
            width_milli: 1_000,
            height_milli: 1_000,
        },
    );
    let surface = ViewSurfaceResource::new(
        "surface.strict",
        None,
        None,
        ViewElementKind::Panel,
        ViewRuntimeSurfaceBounds {
            x_milli: 0,
            y_milli: 0,
            width_milli: 1_000,
            height_milli: 1_000,
        },
    );

    for (collection, resource) in [
        (
            "action_buttons",
            serde_json::to_value(action_button).expect("action button encodes"),
        ),
        (
            "text_blocks",
            serde_json::to_value(text_block).expect("text block encodes"),
        ),
        (
            "surfaces",
            serde_json::to_value(surface).expect("surface encodes"),
        ),
    ] {
        let root = unique_root(collection);
        fs::create_dir_all(&root).expect("sidecar fixture root creates");
        let mut resource = resource;
        resource
            .as_object_mut()
            .expect("View resource encodes as an object")
            .insert(
                "unexpected_resource_field".to_owned(),
                serde_json::json!(true),
            );
        let mut transcript =
            serde_json::to_value(ViewProgramResource::default()).expect("program sidecar encodes");
        transcript
            .as_object_mut()
            .expect("program resource encodes as an object")
            .insert(collection.to_owned(), serde_json::json!([resource]));
        fs::write(
            root.join("view.program.json"),
            serde_json::to_vec(&transcript).expect("program sidecar encodes"),
        )
        .expect("program sidecar writes");

        assert!(
            collect_bundle_view_sidecars(&root).is_err(),
            "direct JSON ingestion must reject unknown `{collection}` fields"
        );

        let _ = fs::remove_dir_all(root);
    }
}
