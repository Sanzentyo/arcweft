use arcweft_bundle::container::{BundleDigest, SectionId, SectionKindCode};
use arcweft_bundle::resource_codec::view::{
    ViewDefinitionResource, ViewElementKind, ViewInstructionSpan, ViewProgramResource,
    ViewProgramStyleResources, ViewStyleApplicationTarget, ViewStyleContractError,
    ViewStyleResource,
};
use arcweft_bundle::resource_codec::{
    CrossSectionRef, ProductSourceRef, SourceMapSection, SourceRangeRef,
};
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};
use arcweft_view::style::{
    ViewBoxAxisMode, ViewPropertyKind, ViewRatioMilli, ViewSpecifiedValue, ViewStyleAssignOp,
    ViewStyleDeclaration, ViewStylePatch, ViewStylePatchId, ViewStyleProgram, ViewStyleRule,
    ViewStyleSelector, ViewStyleSelectorSequence, ViewStyleSheet, ViewStyleSheetId,
    ViewStyleSourceId,
};

#[test]
fn native_style_program_round_trips_with_sheet_and_patch_ownership() {
    let style = style_resource("view.style.alpha", "style.alpha", 0);
    let bytes = style.encode_canonical_section().expect("Style encodes");
    let decoded = ViewStyleResource::decode_canonical_section(&bytes).expect("Style decodes");

    assert_eq!(decoded, style);
    assert_eq!(decoded.program.sheets()[0].id(), &sheet_id("style.alpha"));
    assert_eq!(decoded.program.sheets()[0].rules()[0].source_order(), 0);
    assert_eq!(decoded.program.patches()[0].id(), ViewStylePatchId::new(0));
    assert_eq!(
        decoded
            .encode_canonical_section()
            .expect("decoded Style re-encodes"),
        bytes
    );

    let mut json = serde_json::to_value(&style).expect("Style serializes");
    json.as_object_mut()
        .expect("Style is an object")
        .insert("legacy_field".to_owned(), serde_json::json!(true));
    assert!(serde_json::from_value::<ViewStyleResource>(json).is_err());
}

#[test]
fn style_program_contract_rejects_dangling_targets_and_inline_definition_roots() {
    let style = style_resource("view.style.alpha", "style.alpha", 0);
    let mut program = ViewProgramResource::default();
    program.definitions.push(ViewDefinitionResource {
        public_id: arcweft_bundle::resource_codec::view::ViewDefinitionRef::try_new("view.main")
            .unwrap(),
        body: ViewInstructionSpan::new(0, 0),
        styles: vec![ViewStyleApplicationTarget::named(sheet_id("style.missing"))],
        parameters: Vec::new(),
        state_schema_hash: 0,
    });
    assert!(matches!(
        program.validate_style_contract(Some(&style)),
        Err(ViewStyleContractError::UnknownSheet { .. })
    ));

    program.definitions[0].styles =
        vec![ViewStyleApplicationTarget::inline(ViewStylePatchId::new(0))];
    assert!(matches!(
        program.validate_style_contract(Some(&style)),
        Err(ViewStyleContractError::InlineDefinitionPatch { .. })
    ));
}

#[test]
fn style_merge_rebases_patch_and_source_ids_as_one_canonical_program() {
    let left = style_resource("view.style.left", "style.left", 0);
    let right = style_resource("view.style.right", "style.right", 0);
    let right_patch_source = right.program.patches()[0].declarations()[0].source();
    let expected_range = right.source_map_refs[right_patch_source.value() as usize];
    let expected_source = right.source_refs[expected_range.source().value() as usize].clone();
    let merged = ViewProgramStyleResources::new(None, Some(left))
        .merge(ViewProgramStyleResources::new(None, Some(right)))
        .expect("native Style resources merge");
    let style = merged.style.expect("merged Style exists");

    assert_eq!(
        style
            .program
            .patches()
            .iter()
            .map(ViewStylePatch::id)
            .collect::<Vec<_>>(),
        [ViewStylePatchId::new(0), ViewStylePatchId::new(1)]
    );
    let merged_patch_source = style.program.patches()[1].declarations()[0].source();
    let merged_range = style.source_map_refs[merged_patch_source.value() as usize];
    assert_eq!(
        style.source_refs[merged_range.source().value() as usize],
        expected_source
    );
    assert_eq!(merged_range.start_byte(), expected_range.start_byte());
    assert_eq!(merged_range.end_byte(), expected_range.end_byte());
    style
        .encode_canonical_section()
        .expect("merged Style remains canonical");
}

#[test]
fn style_metadata_inventory_order_does_not_change_canonical_bytes() {
    let mut canonical = style_resource("view.style.alpha", "style.alpha", 0);
    canonical.adapter_requirements = vec![adapter_requirement(1), adapter_requirement(2)];
    let mut reversed =
        style_resource_with_source_inventory("view.style.alpha", "style.alpha", 0, true);
    reversed.adapter_requirements = vec![adapter_requirement(2), adapter_requirement(1)];

    let canonical_bytes = canonical.encode_canonical_section().expect("Style encodes");
    assert_eq!(
        reversed.encode_canonical_section().expect("Style encodes"),
        canonical_bytes,
    );

    let decoded =
        ViewStyleResource::decode_canonical_section(&canonical_bytes).expect("Style decodes");
    assert!(decoded.source_map_refs.windows(2).all(|pair| {
        (pair[0].source(), pair[0].start_byte(), pair[0].end_byte())
            <= (pair[1].source(), pair[1].start_byte(), pair[1].end_byte())
    }));
    assert_eq!(
        decoded.adapter_requirements,
        [adapter_requirement(1), adapter_requirement(2)]
    );
}

#[test]
fn equivalent_reversed_style_merges_have_deterministic_bytes() {
    let mut style_a = style_resource("view.style.program", "style.a", 0);
    style_a.adapter_requirements = vec![adapter_requirement(2)];
    let mut style_z = style_resource("view.style.program", "style.z", 0);
    style_z.adapter_requirements = vec![adapter_requirement(1)];

    let forward = ViewProgramStyleResources::new(None, Some(style_z.clone()))
        .merge(ViewProgramStyleResources::new(None, Some(style_a.clone())))
        .expect("forward merge succeeds")
        .style
        .expect("merged Style exists");
    let reversed = ViewProgramStyleResources::new(None, Some(style_a))
        .merge(ViewProgramStyleResources::new(None, Some(style_z)))
        .expect("reversed merge succeeds")
        .style
        .expect("merged Style exists");

    assert_eq!(
        forward.encode_canonical_section().expect("Style encodes"),
        reversed.encode_canonical_section().expect("Style encodes"),
    );
}

fn style_resource(program_id: &str, sheet_name: &str, patch_id: u32) -> ViewStyleResource {
    style_resource_with_source_inventory(program_id, sheet_name, patch_id, false)
}

fn style_resource_with_source_inventory(
    program_id: &str,
    sheet_name: &str,
    patch_id: u32,
    reversed_sources: bool,
) -> ViewStyleResource {
    let (sheet_source, patch_source) = if reversed_sources {
        (ViewStyleSourceId::new(1), ViewStyleSourceId::new(0))
    } else {
        (ViewStyleSourceId::new(0), ViewStyleSourceId::new(1))
    };
    let sheet_id = sheet_id(sheet_name);
    let declaration = ViewStyleDeclaration::new(
        ViewPropertyKind::Opacity,
        ViewSpecifiedValue::Ratio {
            value: ViewRatioMilli::new(900).expect("valid ratio"),
        },
        ViewStyleAssignOp::Replace,
        sheet_source,
    )
    .expect("valid declaration");
    let selector = ViewStyleSelector::new(vec![
        ViewStyleSelectorSequence::new(None, Some(ViewElementKind::Button), None, Vec::new())
            .expect("valid selector sequence"),
    ])
    .expect("valid selector");
    let rule =
        ViewStyleRule::new(selector, None, vec![declaration], 0, sheet_source).expect("valid rule");
    let sheet = ViewStyleSheet::new(sheet_id.clone(), Vec::new(), vec![rule]).expect("valid sheet");
    let patch = ViewStylePatch::new(
        ViewStylePatchId::new(patch_id),
        vec![
            ViewStyleDeclaration::new(
                ViewPropertyKind::Opacity,
                ViewSpecifiedValue::Ratio {
                    value: ViewRatioMilli::new(700).expect("valid ratio"),
                },
                ViewStyleAssignOp::Replace,
                patch_source,
            )
            .expect("valid patch declaration"),
            ViewStyleDeclaration::new(
                ViewPropertyKind::BoxAxes,
                ViewSpecifiedValue::BoxAxes {
                    value: ViewBoxAxisMode::VerticalRl,
                },
                ViewStyleAssignOp::Replace,
                patch_source,
            )
            .expect("valid axis declaration"),
        ],
    );
    let sheet_document = source_document(&format!("style-sheet:{sheet_name}"));
    let patch_document = source_document(&format!("style-patch:{program_id}"));
    let section = SourceMapSection::try_from_documents(&[&sheet_document, &patch_document])
        .expect("source map");
    let sheet_ref = ProductSourceRef::from_document(
        section
            .documents()
            .find(|document| document.document_id() == sheet_document.identity().id())
            .expect("sheet source"),
    );
    let patch_ref = ProductSourceRef::from_document(
        section
            .documents()
            .find(|document| document.document_id() == patch_document.identity().id())
            .expect("patch source"),
    );
    let mut source_refs = vec![sheet_ref.clone(), patch_ref.clone()];
    if reversed_sources {
        source_refs.reverse();
    }
    let mut source_map_refs = vec![
        SourceRangeRef::try_for_source(&source_refs, &sheet_ref, 0, 1).expect("sheet range"),
        SourceRangeRef::try_for_source(&source_refs, &patch_ref, 2, 3).expect("patch range"),
    ];
    if reversed_sources {
        source_map_refs.reverse();
    }
    ViewStyleResource {
        style_program_id: program_id.to_owned(),
        program: ViewStyleProgram::try_new(vec![sheet], vec![patch]).expect("valid Style program"),
        source_refs,
        source_map_refs,
        adapter_requirements: Vec::new(),
    }
}

fn source_document(id: &str) -> SourceDocument {
    SourceDocument::try_new(
        SourceDocumentId::try_new(id).expect("source ID"),
        SourceName::Memory,
        "a b",
    )
    .expect("source document")
}

fn adapter_requirement(seed: u8) -> CrossSectionRef {
    CrossSectionRef {
        section_kind: SectionKindCode::new(u32::from(seed)),
        section_id: SectionId::from_bytes([seed; 16]),
        content_digest: BundleDigest::from_bytes([seed; 32]),
        public_id: None,
    }
}

fn sheet_id(value: &str) -> ViewStyleSheetId {
    ViewStyleSheetId::try_new(value).expect("valid sheet ID")
}
