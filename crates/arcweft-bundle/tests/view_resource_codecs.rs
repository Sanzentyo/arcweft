use arcweft_bundle::container::{BundleDigest, BundleSectionKind};
use arcweft_bundle::patch::PatchCompatibility;
use arcweft_bundle::resource_codec::view::{
    CompositionOnBlurPolicy, DialogueTextProjection, EnterKeyHint, SystemColorOverride,
    TextAssistPolicy, TextCapitalization, ViewAwaitBranchSpan, ViewCallArgumentBindingRef,
    ViewDefinitionResource, ViewElementKind, ViewExportedPart, ViewFocusAutoScrollPolicy,
    ViewFxArgumentBindingRef, ViewHandlerRef, ViewInputKind, ViewInputOptions, ViewInputPurpose,
    ViewInputResource, ViewInstructionSpan, ViewLayoutBoundsResource, ViewLocalizedTextResource,
    ViewLogicalRect, ViewObserveClassification, ViewParameterResource, ViewProgramInstruction,
    ViewProgramResource, ViewProgramStyleResources, ViewResourceBudget, ViewResourceCompatibility,
    ViewScrollAxis, ViewScrollIndicatorsPolicy, ViewScrollOverflowPolicy,
    ViewScrollOverscrollPolicy, ViewScrollRegionResource, ViewSecureInputPolicy,
    ViewSecureRedactionMetadata, ViewSemanticTarget, ViewStyleApplicationTarget, ViewStyleAssignOp,
    ViewStyleDeclaration, ViewStylePatch, ViewStylePatchId, ViewStyleProgram, ViewStyleResource,
    ViewStyleRule, ViewStyleSheet, ViewStyleSheetId, ViewStyleSourceId, ViewStyleToken,
    ViewStyleTokenId, ViewTextBlockBounds, ViewTextBlockResource, ViewTextResource,
    ViewTextSelectionPolicy, ViewTextShortcutPolicy, ViewTextSourceKind, ViewTextSourceRecord,
    ViewTextTabPolicy, ViewTextVerticalNavigationPolicy, ViewThemeEnvironmentDefaults,
    ViewThemeResource, ViewValueInputNamespace, ViewValueInputResource, ViewValueInputSource,
    migrated_view_section_compatibility,
};
use arcweft_render_text::{RichTextDocument, RichTextNode};

use arcweft_bundle::resource_codec::{
    FieldId, ProductResourceEnvelope, ProductSectionCodecKind, PublicIdRef, PublicIdTable,
    ResourceField, ResourceWireType, SectionCodecBudget, SourceRangeRef,
};
use arcweft_presentation::appearance::{
    ColorSchemePreference, ContrastPreference, PresentationColor, SystemColor,
};
use arcweft_presentation::fx::{
    FiniteF32, FxId, FxRuntimeType, FxRuntimeValue, ValueInstruction, ValueProgramSchema,
};
use arcweft_view::style::{
    ViewColorValue, ViewElementState, ViewPropertyKind, ViewSpecifiedValue, ViewStylePredicate,
    ViewStyleSelector, ViewStyleSelectorSequence, ViewStyleValueKind,
};
use arcweft_view::{ViewValueProgram, ViewValueProgramId};

const MALFORMED_RESOURCE_IDENTITIES: [&str; 4] =
    ["", "part shared", "part.\u{0007}shared", "#part.shared"];

#[test]
fn view_element_inventory_owns_codec_tags_and_round_trips() {
    for element in ViewElementKind::ALL {
        let authoritative: arcweft_view::ViewElementKind = element;
        let encoded = serde_json::to_string(&element).expect("element tag encodes");
        assert_eq!(encoded, format!("\"{}\"", authoritative.runtime_label()));
        assert_eq!(
            serde_json::from_str::<ViewElementKind>(&encoded).expect("element tag decodes"),
            element
        );
    }
}

#[test]
fn view_resource_compact_sections_round_trip_with_deterministic_bytes() {
    let program = fixture_program();
    assert_round_trip(
        ProductSectionCodecKind::ViewProgram,
        &program.encode_canonical_section().expect("program encodes"),
        ViewProgramResource::decode_canonical_section,
        &program,
    );

    let style = fixture_style();
    assert_round_trip(
        ProductSectionCodecKind::ViewStyle,
        &style.encode_canonical_section().expect("style encodes"),
        ViewStyleResource::decode_canonical_section,
        &style,
    );

    let text = fixture_text();
    assert_round_trip(
        ProductSectionCodecKind::ViewText,
        &text.encode_canonical_section().expect("text encodes"),
        ViewTextResource::decode_canonical_section,
        &text,
    );

    let input = fixture_input(ViewSecureInputPolicy::Plain);
    assert_round_trip(
        ProductSectionCodecKind::ViewInput,
        &input.encode_canonical_section().expect("input encodes"),
        ViewInputResource::decode_canonical_section,
        &input,
    );

    let theme = fixture_theme(PresentationColor::rgb(0x25, 0x63, 0xEB));
    assert_round_trip(
        ProductSectionCodecKind::ViewTheme,
        &theme.encode_canonical_section().expect("theme encodes"),
        ViewThemeResource::decode_canonical_section,
        &theme,
    );
}

#[test]
fn emit_text_requires_a_one_to_one_owned_text_block_graph() {
    let mut missing = fixture_program();
    let ViewProgramInstruction::EmitText { text_block, .. } = &mut missing.instructions[1] else {
        panic!("fixture instruction 1 emits text");
    };
    *text_block = "text.block.missing".to_owned();
    assert_eq!(
        missing
            .encode_canonical_section()
            .expect_err("EmitText must reference an existing text block"),
        arcweft_bundle::resource_codec::SectionCodecError::NonCanonicalTable(
            "view_emit_text_block_refs",
        ),
    );

    let mut duplicate = fixture_program();
    duplicate.instructions[2] = duplicate.instructions[1].clone();
    assert_eq!(
        duplicate
            .encode_canonical_section()
            .expect_err("one text block cannot be bound by multiple EmitText instructions"),
        arcweft_bundle::resource_codec::SectionCodecError::NonCanonicalTable(
            "view_emit_text_block_duplicate_refs",
        ),
    );

    let mut unreferenced = fixture_program();
    unreferenced.text_blocks.push(ViewTextBlockResource::new(
        "text.block.dialogue.unreferenced",
        Some("view.dialogue".to_owned()),
        None,
        "text.dialogue.unreferenced",
        ViewTextBlockBounds::from_px(0, 0, 100, 20),
    ));
    assert_eq!(
        unreferenced
            .encode_canonical_section()
            .expect_err("every text block must have one EmitText owner"),
        arcweft_bundle::resource_codec::SectionCodecError::NonCanonicalTable(
            "view_emit_text_block_coverage",
        ),
    );

    let mut wrong_source = fixture_program();
    wrong_source.text_blocks[0].text_source = "text.dialogue.other".to_owned();
    assert_eq!(
        wrong_source
            .encode_canonical_section()
            .expect_err("EmitText and its text block must share one text source"),
        arcweft_bundle::resource_codec::SectionCodecError::NonCanonicalTable(
            "view_emit_text_block_sources",
        ),
    );

    let mut wrong_owner = fixture_program();
    wrong_owner.text_blocks[0].view = Some("view.other".to_owned());
    assert_eq!(
        wrong_owner
            .encode_canonical_section()
            .expect_err("EmitText and its text block must share one View owner"),
        arcweft_bundle::resource_codec::SectionCodecError::NonCanonicalTable(
            "view_emit_text_block_owners",
        ),
    );
}

#[test]
fn emit_text_transcript_requires_the_text_block_reference() {
    let program = fixture_program();
    let mut value = serde_json::to_value(&program.instructions[1]).expect("instruction encodes");
    value
        .get_mut("emit_text")
        .and_then(serde_json::Value::as_object_mut)
        .expect("EmitText uses the canonical tagged payload")
        .remove("text_block");

    assert!(
        serde_json::from_value::<ViewProgramInstruction>(value).is_err(),
        "the unreleased canonical model has no missing-field default or dual reader",
    );
}

#[test]
fn view_program_rejects_removed_string_style_fields() {
    let bytes = arcweft_bundle::standard_view::dialogue_program()
        .encode_canonical_section()
        .expect("standard View program encodes");
    let envelope = ProductResourceEnvelope::decode_all_fields(
        &bytes,
        ProductSectionCodecKind::ViewProgram,
        SectionCodecBudget::default(),
    )
    .expect("View program envelope decodes");
    let transcript = envelope
        .fields
        .iter()
        .find(|field| field.id == FieldId(1))
        .expect("View transcript field exists");
    let canonical: serde_json::Value =
        serde_json::from_slice(&transcript.payload).expect("View transcript is JSON");

    for table in ["action_buttons", "text_blocks", "surfaces"] {
        let mut tampered = canonical.clone();
        tampered[table][0]
            .as_object_mut()
            .expect("standard View record is an object")
            .insert(
                "style".to_owned(),
                serde_json::Value::String("legacy.string.style".to_owned()),
            );
        let payload = serde_json::to_vec(&tampered).expect("tampered transcript encodes");
        let bytes = envelope_with_replaced_field_payload(&envelope, FieldId(1), &payload);

        assert_eq!(
            ViewProgramResource::decode_canonical_section(&bytes)
                .expect_err("removed string Style field must not disappear during decode"),
            arcweft_bundle::resource_codec::SectionCodecError::NonCanonicalTable("view_program"),
            "removed `{table}[0].style` must reject",
        );
    }
}

#[test]
fn view_style_codec_rejects_missing_and_ambiguous_inline_token_owners() {
    let bytes = fixture_style()
        .encode_canonical_section()
        .expect("Style encodes");
    let envelope = ProductResourceEnvelope::decode_all_fields(
        &bytes,
        ProductSectionCodecKind::ViewStyle,
        SectionCodecBudget::default(),
    )
    .expect("Style envelope decodes");
    let transcript = envelope
        .fields
        .iter()
        .find(|field| field.id == FieldId(1))
        .expect("View transcript field exists");
    let canonical: serde_json::Value =
        serde_json::from_slice(&transcript.payload).expect("View transcript is JSON");

    let mut missing = canonical.clone();
    missing["program"]["patches"][0]["declarations"][0]["value"] = serde_json::json!({
        "kind": "token",
        "token": "token.missing",
        "value_kind": "ratio",
    });

    let mut ambiguous = canonical;
    let mut second_sheet = ambiguous["program"]["sheets"][0].clone();
    second_sheet["id"] = serde_json::json!("style.secondary");
    ambiguous["program"]["sheets"]
        .as_array_mut()
        .expect("sheet inventory")
        .push(second_sheet);
    ambiguous["program"]["patches"][0]["declarations"][0]["value"] = serde_json::json!({
        "kind": "token",
        "token": "token.accent",
        "value_kind": "ratio",
    });

    for (label, tampered) in [("missing", missing), ("ambiguous", ambiguous)] {
        let payload = serde_json::to_vec(&tampered).expect("tampered transcript encodes");
        let bytes = envelope_with_replaced_field_payload(&envelope, FieldId(1), &payload);
        assert_eq!(
            ViewStyleResource::decode_canonical_section(&bytes)
                .expect_err("invalid inline token ownership must reject during decode"),
            arcweft_bundle::resource_codec::SectionCodecError::NonCanonicalTable("view_style"),
            "{label} inline token owner must reject",
        );
    }
}

#[test]
fn view_resource_merge_remaps_program_source_public_ids() {
    let merged = ViewProgramStyleResources::new(Some(sourced_program("view.z_left")), None)
        .merge(ViewProgramStyleResources::new(
            Some(sourced_program("view.a_right")),
            None,
        ))
        .expect("program resources merge atomically");
    let program = merged.program.expect("merged program is retained");
    let public_ids = program
        .public_id_table()
        .expect("merged program public IDs are canonical");
    let resolved = program
        .instructions
        .iter()
        .map(|instruction| {
            let ViewProgramInstruction::EmitCustom {
                source: Some(source),
                ..
            } = instruction
            else {
                panic!("sourced fixture contains only custom node producers");
            };
            public_ids.get(source.source)
        })
        .collect::<Result<Vec<_>, _>>()
        .expect("merged source refs stay in bounds");

    assert_eq!(resolved, vec!["view.z_left", "view.a_right"]);
}

#[test]
fn dialogue_primary_action_requires_a_dialogue_parameter_role() {
    let mut program = arcweft_bundle::standard_view::dialogue_program();
    program.definitions[0].parameters[0].role =
        arcweft_bundle::resource_codec::view::ViewParameterRole::Value;

    let error = program
        .encode_canonical_section()
        .expect_err("compact ViewProgram rejects an untyped dialogue action");

    assert_eq!(
        error,
        arcweft_bundle::resource_codec::SectionCodecError::NonCanonicalTable(
            "view_dialogue_primary_action_parameter"
        )
    );
}

#[test]
fn view_fx_bindings_are_canonical_bounded_and_unique() {
    let binding = |parameter: &str| ViewFxArgumentBindingRef {
        parameter: parameter.to_owned(),
        value_program: ViewValueProgramId(1),
    };
    let instruction = |arguments| ViewProgramInstruction::ApplyFx {
        fx: FxId::try_new("game", "ui.effects.notice").expect("valid Fx id"),
        arguments,
        key_program: None,
        application_ordinal: 0,
        source: None,
    };

    let mut first = fixture_program();
    first
        .instructions
        .push(instruction(vec![binding("speed"), binding("amplitude")]));
    first.definitions[0].body.end_instruction = first.instructions.len().try_into().unwrap();
    let mut second = fixture_program();
    second
        .instructions
        .push(instruction(vec![binding("amplitude"), binding("speed")]));
    second.definitions[0].body.end_instruction = second.instructions.len().try_into().unwrap();
    assert_eq!(
        first.encode_canonical_section().expect("first encodes"),
        second.encode_canonical_section().expect("second encodes")
    );

    let mut duplicate = fixture_program();
    duplicate
        .instructions
        .push(instruction(vec![binding("speed"), binding("speed")]));
    duplicate.definitions[0].body.end_instruction =
        duplicate.instructions.len().try_into().unwrap();
    assert!(duplicate.encode_canonical_section().is_err());

    let bytes = first
        .encode_canonical_section()
        .expect("Fx program encodes");
    let budget = ViewResourceBudget {
        fx_arguments: 1,
        ..ViewResourceBudget::default()
    };
    assert!(ViewProgramResource::decode_canonical_section_with_budget(&bytes, budget).is_err());
}

#[test]
fn nested_view_calls_are_ordinal_canonical_required_and_typed() {
    let binding = |ordinal, name: &str, value_program| ViewCallArgumentBindingRef {
        ordinal,
        name: Some(name.to_owned()),
        value_program: ViewValueProgramId(value_program),
    };
    let value_program = |id, value: FxRuntimeValue| {
        ViewValueProgram::validate(
            ViewValueProgramId(id),
            ValueProgramSchema::new(
                vec![FxRuntimeType::I32, FxRuntimeType::F32],
                vec![],
                value.value_type(),
            ),
            vec![
                ValueInstruction::Constant { value },
                ValueInstruction::Return,
            ],
        )
        .unwrap()
    };
    let program = |arguments| ViewProgramResource {
        program_id: "view.program.nested".to_owned(),
        definitions: vec![
            ViewDefinitionResource {
                public_id: "view.Caller".to_owned(),
                body: ViewInstructionSpan::new(0, 1),
                styles: Vec::new(),
                parameters: Vec::new(),
                state_schema_hash: 1,
            },
            ViewDefinitionResource {
                public_id: "view.Child".to_owned(),
                body: ViewInstructionSpan::new(1, 1),
                styles: Vec::new(),
                parameters: vec![
                    ViewParameterResource {
                        ordinal: 0,
                        name: "count".to_owned(),
                        role: arcweft_bundle::resource_codec::view::ViewParameterRole::Value,
                        value_type: Some(FxRuntimeType::I32),
                        value_slot: Some(0),
                        default_program: None,
                    },
                    ViewParameterResource {
                        ordinal: 1,
                        name: "opacity".to_owned(),
                        role: arcweft_bundle::resource_codec::view::ViewParameterRole::Value,
                        value_type: Some(FxRuntimeType::F32),
                        value_slot: Some(1),
                        default_program: None,
                    },
                ],
                state_schema_hash: 2,
            },
        ],
        value_programs: vec![
            value_program(0, FxRuntimeValue::I32(2)),
            value_program(1, FxRuntimeValue::F32(FiniteF32::try_new(0.5).unwrap())),
        ],
        value_inputs: vec![
            ViewValueInputResource {
                namespace: ViewValueInputNamespace::Parameter,
                slot: 0,
                value_type: FxRuntimeType::I32,
                source: ViewValueInputSource::DefinitionParameter {
                    view: "view.Child".to_owned(),
                    name: "count".to_owned(),
                },
            },
            ViewValueInputResource {
                namespace: ViewValueInputNamespace::Parameter,
                slot: 1,
                value_type: FxRuntimeType::F32,
                source: ViewValueInputSource::DefinitionParameter {
                    view: "view.Child".to_owned(),
                    name: "opacity".to_owned(),
                },
            },
        ],
        instructions: vec![ViewProgramInstruction::CallView {
            view: "view.Child".to_owned(),
            arguments,
            styles: Vec::new(),
            part: None,
            key: None,
            source: None,
        }],
        ..ViewProgramResource::default()
    };

    let authored_reverse = program(vec![binding(1, "opacity", 1), binding(0, "count", 0)]);
    let authored_forward = program(vec![binding(0, "count", 0), binding(1, "opacity", 1)]);
    assert_eq!(
        authored_reverse.encode_canonical_section().unwrap(),
        authored_forward.encode_canonical_section().unwrap()
    );

    let missing_required = program(vec![binding(0, "count", 0)]);
    assert!(missing_required.encode_canonical_section().is_err());

    let wrong_type = program(vec![binding(0, "count", 1), binding(1, "opacity", 1)]);
    assert!(wrong_type.encode_canonical_section().is_err());
}

#[test]
fn exported_part_identity_is_scoped_to_its_owning_view() {
    let program = exported_part_program();
    program
        .validate_style_contract(None)
        .expect("each exported part resolves inside its owning View");
    let canonical = program
        .encode_canonical_section()
        .expect("different Views may export the same local and public part names");
    let mut reversed = program.clone();
    reversed.exported_parts.reverse();
    assert_eq!(
        reversed
            .encode_canonical_section()
            .expect("reordered exported parts encode"),
        canonical,
        "canonical bytes must not depend on authored cross-View export order",
    );

    let mut duplicate_local = program.clone();
    duplicate_local.exported_parts.push(ViewExportedPart {
        view: "view.Left".to_owned(),
        part_id: "part.shared".to_owned(),
        public_name: "part.other-public".to_owned(),
    });
    assert!(
        duplicate_local.encode_canonical_section().is_err(),
        "one View may not export the same local part twice",
    );

    let mut duplicate_public = program;
    duplicate_public.exported_parts.push(ViewExportedPart {
        view: "view.Left".to_owned(),
        part_id: "part.other".to_owned(),
        public_name: "part.public".to_owned(),
    });
    assert!(
        duplicate_public.encode_canonical_section().is_err(),
        "one View may not bind one public part name to multiple local parts",
    );
}

#[test]
fn exported_part_references_are_validated_by_the_program_codec() {
    let mut missing_view = exported_part_program();
    missing_view.exported_parts[0].view = "view.Missing".to_owned();
    assert_eq!(
        missing_view
            .encode_canonical_section()
            .expect_err("an exported part must name an owning View"),
        arcweft_bundle::resource_codec::SectionCodecError::NonCanonicalTable(
            "view_exported_part_views",
        ),
    );

    let mut missing_target = exported_part_program();
    missing_target.exported_parts[0].part_id = "part.missing".to_owned();
    assert_eq!(
        missing_target
            .encode_canonical_section()
            .expect_err("an exported part must name a part inside its owning View"),
        arcweft_bundle::resource_codec::SectionCodecError::NonCanonicalTable(
            "view_exported_part_targets",
        ),
    );
}

#[test]
fn view_program_identities_reject_malformed_resource_ids() {
    for malformed in MALFORMED_RESOURCE_IDENTITIES {
        let mut invalid_program = exported_part_program();
        invalid_program.program_id = malformed.to_owned();
        assert_eq!(
            invalid_program
                .encode_canonical_section()
                .expect_err("a View program ID must follow resource identity rules"),
            arcweft_bundle::resource_codec::SectionCodecError::NonCanonicalTable(
                "view_program_identities",
            ),
        );

        let mut invalid_view = exported_part_program();
        invalid_view.definitions[0].public_id = malformed.to_owned();
        invalid_view.exported_parts[0].view = malformed.to_owned();
        assert_eq!(
            invalid_view
                .encode_canonical_section()
                .expect_err("a View ID must follow resource identity rules"),
            arcweft_bundle::resource_codec::SectionCodecError::NonCanonicalTable(
                "view_program_identities",
            ),
        );
    }
}

#[test]
fn exported_part_identities_reject_malformed_resource_ids() {
    for malformed in MALFORMED_RESOURCE_IDENTITIES {
        let mut invalid_view = exported_part_program();
        invalid_view.exported_parts[0].view = malformed.to_owned();
        assert_eq!(
            invalid_view
                .encode_canonical_section()
                .expect_err("an exported-part View must follow resource identity rules"),
            arcweft_bundle::resource_codec::SectionCodecError::NonCanonicalTable(
                "view_exported_part_identities",
            ),
        );

        let mut invalid_part = exported_part_program();
        invalid_part.exported_parts[0].part_id = malformed.to_owned();
        let ViewProgramInstruction::EmitCustom { part, .. } = &mut invalid_part.instructions[0]
        else {
            unreachable!("exported-part fixture begins with EmitCustom")
        };
        *part = Some(malformed.to_owned());
        assert_eq!(
            invalid_part
                .encode_canonical_section()
                .expect_err("an exported part ID must follow resource identity rules"),
            arcweft_bundle::resource_codec::SectionCodecError::NonCanonicalTable(
                "view_exported_part_identities",
            ),
        );

        let mut invalid_public_name = exported_part_program();
        invalid_public_name.exported_parts[0].public_name = malformed.to_owned();
        assert_eq!(
            invalid_public_name
                .encode_canonical_section()
                .expect_err("an exported part public name must follow resource identity rules"),
            arcweft_bundle::resource_codec::SectionCodecError::NonCanonicalTable(
                "view_exported_part_identities",
            ),
        );
    }
}

#[test]
fn instruction_parts_reject_malformed_resource_ids() {
    for malformed in MALFORMED_RESOURCE_IDENTITIES {
        let instructions = [
            (
                "open_element",
                ViewProgramInstruction::OpenElement {
                    element: ViewElementKind::Panel,
                    target: None,
                    styles: Vec::new(),
                    part: Some(malformed.to_owned()),
                    key: None,
                    source: None,
                },
            ),
            (
                "emit_text",
                ViewProgramInstruction::EmitText {
                    text_source: "text.body".to_owned(),
                    text_block: "text.block.body".to_owned(),
                    styles: Vec::new(),
                    part: Some(malformed.to_owned()),
                    source: None,
                },
            ),
            (
                "emit_image",
                ViewProgramInstruction::EmitImage {
                    image: "image.hero".to_owned(),
                    target: None,
                    styles: Vec::new(),
                    part: Some(malformed.to_owned()),
                    source: None,
                },
            ),
            (
                "emit_custom",
                ViewProgramInstruction::EmitCustom {
                    element: "element.custom".to_owned(),
                    styles: Vec::new(),
                    part: Some(malformed.to_owned()),
                    source: None,
                },
            ),
            (
                "call_view",
                ViewProgramInstruction::CallView {
                    view: "view.Left".to_owned(),
                    arguments: Vec::new(),
                    styles: Vec::new(),
                    part: Some(malformed.to_owned()),
                    key: None,
                    source: None,
                },
            ),
        ];

        for (kind, instruction) in instructions {
            let mut program = exported_part_program();
            program.exported_parts.clear();
            program.instructions[0] = instruction;
            let error = program
                .encode_canonical_section()
                .expect_err("an instruction part must follow resource identity rules");
            assert_eq!(
                error,
                arcweft_bundle::resource_codec::SectionCodecError::NonCanonicalTable(
                    "view_instruction_parts",
                ),
                "{kind} accepted malformed part identity {malformed:?}",
            );
        }
    }
}

#[test]
fn view_value_program_references_require_existing_programs_and_result_types() {
    let mut missing = fixture_program();
    missing.instructions.push(ViewProgramInstruction::Branch {
        condition_program: ViewValueProgramId(99),
        then_span: 0,
        else_span: None,
        source: None,
    });
    missing.definitions[0].body.end_instruction = missing.instructions.len().try_into().unwrap();
    assert!(missing.encode_canonical_section().is_err());

    let mut wrong_type = fixture_program();
    wrong_type
        .instructions
        .push(ViewProgramInstruction::Branch {
            condition_program: ViewValueProgramId(0),
            then_span: 0,
            else_span: None,
            source: None,
        });
    wrong_type.definitions[0].body.end_instruction =
        wrong_type.instructions.len().try_into().unwrap();
    assert!(wrong_type.encode_canonical_section().is_err());

    let mut malformed_input = fixture_program();
    malformed_input.value_inputs.push(
        arcweft_bundle::resource_codec::view::ViewValueInputResource {
            namespace: arcweft_bundle::resource_codec::view::ViewValueInputNamespace::State,
            slot: 0,
            value_type: arcweft_presentation::fx::FxRuntimeType::I32,
            source: arcweft_bundle::resource_codec::view::ViewValueInputSource::Projection {
                path: vec!["state".to_owned(), String::new()],
            },
        },
    );
    assert!(malformed_input.encode_canonical_section().is_err());
}

#[test]
fn view_resource_unknown_optional_fields_skip_and_unknown_required_reject() {
    let style = fixture_style();
    let bytes = style.encode_canonical_section().expect("style encodes");
    let envelope = ProductResourceEnvelope::decode_all_fields(
        &bytes,
        ProductSectionCodecKind::ViewStyle,
        SectionCodecBudget::default(),
    )
    .expect("envelope decodes");

    let optional_bytes = envelope_with_extra_field(
        &envelope,
        ResourceField::optional(
            FieldId(30_000),
            ResourceWireType::Bytes,
            b"future-view-style",
        ),
    );
    assert_eq!(
        ViewStyleResource::decode_canonical_section(&optional_bytes)
            .expect("unknown optional field skips"),
        style,
    );

    let required_bytes = envelope_with_extra_field(
        &envelope,
        ResourceField::required(
            FieldId(30_001),
            ResourceWireType::Bytes,
            b"future-view-style",
        ),
    );
    assert!(
        ViewStyleResource::decode_canonical_section(&required_bytes).is_err(),
        "unknown required fields must reject for migrated View resources",
    );
}

#[test]
fn view_envelope_declared_inventory_must_match_the_typed_transcript() {
    let bytes = fixture_style()
        .encode_canonical_section()
        .expect("fixture Style encodes");
    let envelope = ProductResourceEnvelope::decode_all_fields(
        &bytes,
        ProductSectionCodecKind::ViewStyle,
        SectionCodecBudget::default(),
    )
    .expect("Style envelope decodes");

    let mut tampered_ids = envelope.public_ids.values().to_vec();
    tampered_ids[0].push_str(".tampered");
    let tampered_public_ids =
        PublicIdTable::new(tampered_ids).expect("tampered IDs remain canonical");
    let public_id_bytes = ProductResourceEnvelope::new(
        envelope.header.codec,
        envelope.strings.clone(),
        tampered_public_ids,
        envelope.enums.clone(),
        envelope.fields.clone(),
        envelope.header.record_count,
    )
    .expect("tampered envelope rebuilds")
    .encode_canonical()
    .expect("tampered envelope encodes");
    assert_eq!(
        ViewStyleResource::decode_canonical_section(&public_id_bytes)
            .expect_err("declared public IDs must match the transcript"),
        arcweft_bundle::resource_codec::SectionCodecError::NonCanonicalTable(
            "view_envelope_public_ids",
        ),
    );

    let record_count_bytes = ProductResourceEnvelope::new(
        envelope.header.codec,
        envelope.strings,
        envelope.public_ids,
        envelope.enums,
        envelope.fields,
        envelope.header.record_count.saturating_add(1),
    )
    .expect("record-count envelope rebuilds")
    .encode_canonical()
    .expect("record-count envelope encodes");
    assert_eq!(
        ViewStyleResource::decode_canonical_section(&record_count_bytes)
            .expect_err("declared record count must match the transcript"),
        arcweft_bundle::resource_codec::SectionCodecError::NonCanonicalTable(
            "view_envelope_record_count",
        ),
    );
}

#[test]
fn view_resource_budget_failures_are_reported() {
    let program_bytes = fixture_program()
        .encode_canonical_section()
        .expect("program encodes");
    assert!(
        ViewProgramResource::decode_canonical_section_with_budget(
            &program_bytes,
            ViewResourceBudget {
                program_instructions: 0,
                ..ViewResourceBudget::default()
            },
        )
        .is_err()
    );
    assert!(
        ViewProgramResource::decode_canonical_section_with_budget(
            &program_bytes,
            ViewResourceBudget {
                text_blocks: 0,
                ..ViewResourceBudget::default()
            },
        )
        .is_err()
    );

    let style_bytes = fixture_style()
        .encode_canonical_section()
        .expect("style encodes");
    assert!(
        ViewStyleResource::decode_canonical_section_with_budget(
            &style_bytes,
            ViewResourceBudget {
                selector_depth: 0,
                ..ViewResourceBudget::default()
            },
        )
        .is_err()
    );
    for budget in [
        ViewResourceBudget {
            style_sheets: 0,
            ..ViewResourceBudget::default()
        },
        ViewResourceBudget {
            style_patches: 0,
            ..ViewResourceBudget::default()
        },
        ViewResourceBudget {
            style_declarations: 1,
            ..ViewResourceBudget::default()
        },
        ViewResourceBudget {
            style_token_depth: 0,
            ..ViewResourceBudget::default()
        },
    ] {
        assert!(
            ViewStyleResource::decode_canonical_section_with_budget(&style_bytes, budget).is_err(),
            "each owned Style inventory must enforce its decode budget",
        );
    }
    assert!(
        ViewStyleResource::decode_canonical_section_with_budget(
            &style_bytes,
            ViewResourceBudget {
                style_tokens: 0,
                ..ViewResourceBudget::default()
            },
        )
        .is_err()
    );

    let text_bytes = fixture_text()
        .encode_canonical_section()
        .expect("text encodes");
    assert!(
        ViewTextResource::decode_canonical_section_with_budget(
            &text_bytes,
            ViewResourceBudget {
                text_sources: 0,
                ..ViewResourceBudget::default()
            },
        )
        .is_err()
    );
}

#[test]
fn view_program_layout_bounds_reject_zero_size_rects() {
    let mut program = fixture_program();
    program
        .layout_bounds
        .push(ViewLayoutBoundsResource::text_control(
            "input.dialogue.invalid",
            ViewLogicalRect::new(48_000, 48_000, 0, 48_000),
        ));

    assert!(
        program.encode_canonical_section().is_err(),
        "zero-width layout bounds are not canonical View resources",
    );
}

#[test]
fn view_program_scroll_regions_reject_pre_axis_payload_shape() {
    for removed_field in ["content_width_milli", "axis"] {
        let bytes = view_program_bytes_without_scroll_region_field(removed_field);
        assert!(
            ViewProgramResource::decode_canonical_section(&bytes).is_err(),
            "scroll regions must reject payloads missing `{removed_field}`",
        );
    }
}

#[test]
fn view_program_scroll_region_policy_defaults_are_round_tripped() {
    let bytes = fixture_program()
        .encode_canonical_section()
        .expect("program encodes");
    let decoded = ViewProgramResource::decode_canonical_section(&bytes).expect("program decodes");
    let region = decoded.scroll_regions.first().expect("scroll region");
    let runtime = region.runtime_scroll_region();

    assert_eq!(region.indicators, ViewScrollIndicatorsPolicy::Auto);
    assert_eq!(region.overscroll, ViewScrollOverscrollPolicy::Clamp);
    assert_eq!(region.auto_scroll_focus, ViewFocusAutoScrollPolicy::Nearest);
    assert_eq!(runtime.indicators, ViewScrollIndicatorsPolicy::Auto);
    assert_eq!(runtime.overscroll, ViewScrollOverscrollPolicy::Clamp);
    assert_eq!(
        runtime.auto_scroll_focus,
        ViewFocusAutoScrollPolicy::Nearest
    );
}

#[test]
fn view_program_scroll_region_policy_values_are_preserved() {
    let mut program = fixture_program();
    program.scroll_regions[0] = program.scroll_regions[0]
        .clone()
        .with_indicators(ViewScrollIndicatorsPolicy::Visible)
        .with_overscroll(ViewScrollOverscrollPolicy::Elastic)
        .with_auto_scroll_focus(ViewFocusAutoScrollPolicy::End);

    let bytes = program.encode_canonical_section().expect("program encodes");
    let decoded = ViewProgramResource::decode_canonical_section(&bytes).expect("program decodes");
    let region = decoded.scroll_regions.first().expect("scroll region");
    let runtime = region.runtime_scroll_region();

    assert_eq!(region.indicators, ViewScrollIndicatorsPolicy::Visible);
    assert_eq!(region.overscroll, ViewScrollOverscrollPolicy::Elastic);
    assert_eq!(region.auto_scroll_focus, ViewFocusAutoScrollPolicy::End);
    assert_eq!(runtime.indicators, ViewScrollIndicatorsPolicy::Visible);
    assert_eq!(runtime.overscroll, ViewScrollOverscrollPolicy::Elastic);
    assert_eq!(runtime.auto_scroll_focus, ViewFocusAutoScrollPolicy::End);
}

#[test]
fn view_theme_palette_changes_are_content_only() {
    let old = fixture_theme(PresentationColor::rgb(0x25, 0x63, 0xEB));
    let new = fixture_theme(PresentationColor::rgb(0x58, 0xA6, 0xFF));

    assert_eq!(
        old.compatibility_with(&new),
        ViewResourceCompatibility::ContentOnly
    );
    assert_eq!(
        migrated_view_section_compatibility(
            BundleSectionKind::ViewTheme,
            &old.encode_canonical_section().expect("old encodes"),
            &new.encode_canonical_section().expect("new encodes"),
        )
        .expect("compatibility decodes"),
        Some(PatchCompatibility::ContentOnly),
    );
}

#[test]
fn view_input_secure_policy_changes_are_restart_required() {
    let old = fixture_input(ViewSecureInputPolicy::Plain);
    let new = fixture_input(ViewSecureInputPolicy::Password);

    assert_eq!(
        old.compatibility_with(&new),
        ViewResourceCompatibility::RestartRequired,
    );
    assert_eq!(
        migrated_view_section_compatibility(
            BundleSectionKind::ViewInput,
            &old.encode_canonical_section().expect("old encodes"),
            &new.encode_canonical_section().expect("new encodes"),
        )
        .expect("compatibility decodes"),
        Some(PatchCompatibility::RestartRequired),
    );
}

#[test]
fn view_resource_rejects_json_fallback() {
    let json = br#"{"style_program_id":"style.dialogue","tokens":[]}"#;

    assert!(
        ViewStyleResource::decode_canonical_section(json).is_err(),
        "migrated View resource decode must require compact AWFB section magic",
    );
}

fn assert_round_trip<T>(
    codec: ProductSectionCodecKind,
    bytes: &[u8],
    decode: fn(&[u8]) -> Result<T, arcweft_bundle::resource_codec::SectionCodecError>,
    expected: &T,
) where
    T: EncodeAgain + std::fmt::Debug + PartialEq,
{
    assert_ne!(bytes.first(), Some(&b'{'), "{codec:?} must not be JSON");
    assert_eq!(bytes[..8], codec.magic(), "{codec:?} compact magic");
    let decoded = decode(bytes).expect("compact section decodes");
    assert_eq!(&decoded, expected);
    assert_eq!(
        bytes,
        decoded.encode_again(codec).as_slice(),
        "{codec:?} bytes must be deterministic",
    );
}

trait EncodeAgain {
    fn encode_again(&self, codec: ProductSectionCodecKind) -> Vec<u8>;
}

impl EncodeAgain for ViewProgramResource {
    fn encode_again(&self, codec: ProductSectionCodecKind) -> Vec<u8> {
        assert_eq!(codec, ProductSectionCodecKind::ViewProgram);
        self.encode_canonical_section().expect("program re-encodes")
    }
}

impl EncodeAgain for ViewStyleResource {
    fn encode_again(&self, codec: ProductSectionCodecKind) -> Vec<u8> {
        assert_eq!(codec, ProductSectionCodecKind::ViewStyle);
        self.encode_canonical_section().expect("style re-encodes")
    }
}

impl EncodeAgain for ViewTextResource {
    fn encode_again(&self, codec: ProductSectionCodecKind) -> Vec<u8> {
        assert_eq!(codec, ProductSectionCodecKind::ViewText);
        self.encode_canonical_section().expect("text re-encodes")
    }
}

impl EncodeAgain for ViewInputResource {
    fn encode_again(&self, codec: ProductSectionCodecKind) -> Vec<u8> {
        assert_eq!(codec, ProductSectionCodecKind::ViewInput);
        self.encode_canonical_section().expect("input re-encodes")
    }
}

impl EncodeAgain for ViewThemeResource {
    fn encode_again(&self, codec: ProductSectionCodecKind) -> Vec<u8> {
        assert_eq!(codec, ProductSectionCodecKind::ViewTheme);
        self.encode_canonical_section().expect("theme re-encodes")
    }
}

fn envelope_with_extra_field(envelope: &ProductResourceEnvelope, field: ResourceField) -> Vec<u8> {
    let mut fields = envelope.fields.clone();
    fields.push(field);
    ProductResourceEnvelope::new(
        envelope.header.codec,
        envelope.strings.clone(),
        envelope.public_ids.clone(),
        envelope.enums.clone(),
        fields,
        envelope.header.record_count,
    )
    .expect("envelope rebuilds")
    .encode_canonical()
    .expect("envelope re-encodes")
}

fn envelope_with_replaced_field_payload(
    envelope: &ProductResourceEnvelope,
    field_id: FieldId,
    payload: &[u8],
) -> Vec<u8> {
    let fields: Vec<ResourceField> = envelope
        .fields
        .iter()
        .map(|field| {
            if field.id == field_id {
                ResourceField::new(
                    field.id,
                    field.requirement,
                    field.wire_type,
                    field.nesting_depth,
                    field.reference_count,
                    payload.to_vec(),
                )
            } else {
                field.clone()
            }
        })
        .collect();
    ProductResourceEnvelope::new(
        envelope.header.codec,
        envelope.strings.clone(),
        envelope.public_ids.clone(),
        envelope.enums.clone(),
        fields,
        envelope.header.record_count,
    )
    .expect("envelope rebuilds")
    .encode_canonical()
    .expect("envelope re-encodes")
}

fn view_program_bytes_without_scroll_region_field(field_name: &str) -> Vec<u8> {
    let bytes = fixture_program()
        .encode_canonical_section()
        .expect("program encodes");
    let envelope = ProductResourceEnvelope::decode_all_fields(
        &bytes,
        ProductSectionCodecKind::ViewProgram,
        SectionCodecBudget::default(),
    )
    .expect("envelope decodes");
    let transcript = envelope
        .fields
        .iter()
        .find(|field| field.id == FieldId(1))
        .expect("view transcript field exists");
    let mut json: serde_json::Value =
        serde_json::from_slice(&transcript.payload).expect("transcript is JSON");
    json["scroll_regions"][0]
        .as_object_mut()
        .expect("scroll region transcript is an object")
        .remove(field_name);
    let updated_transcript = serde_json::to_vec(&json).expect("updated transcript encodes");
    envelope_with_replaced_field_payload(&envelope, FieldId(1), &updated_transcript)
}

fn exported_part_program() -> ViewProgramResource {
    ViewProgramResource {
        program_id: "program.exported-parts".to_owned(),
        definitions: vec![
            ViewDefinitionResource {
                public_id: "view.Left".to_owned(),
                body: ViewInstructionSpan::new(0, 2),
                styles: Vec::new(),
                parameters: Vec::new(),
                state_schema_hash: 0,
            },
            ViewDefinitionResource {
                public_id: "view.Right".to_owned(),
                body: ViewInstructionSpan::new(2, 3),
                styles: Vec::new(),
                parameters: Vec::new(),
                state_schema_hash: 0,
            },
        ],
        instructions: vec![
            ViewProgramInstruction::EmitCustom {
                element: "element.left.shared".to_owned(),
                styles: Vec::new(),
                part: Some("part.shared".to_owned()),
                source: None,
            },
            ViewProgramInstruction::EmitCustom {
                element: "element.left.other".to_owned(),
                styles: Vec::new(),
                part: Some("part.other".to_owned()),
                source: None,
            },
            ViewProgramInstruction::EmitCustom {
                element: "element.right.shared".to_owned(),
                styles: Vec::new(),
                part: Some("part.shared".to_owned()),
                source: None,
            },
        ],
        exported_parts: vec![
            ViewExportedPart {
                view: "view.Left".to_owned(),
                part_id: "part.shared".to_owned(),
                public_name: "part.public".to_owned(),
            },
            ViewExportedPart {
                view: "view.Right".to_owned(),
                part_id: "part.shared".to_owned(),
                public_name: "part.public".to_owned(),
            },
        ],
        ..ViewProgramResource::default()
    }
}

fn sourced_program(view_id: &str) -> ViewProgramResource {
    let mut program = ViewProgramResource {
        program_id: format!("program.{view_id}"),
        definitions: vec![ViewDefinitionResource {
            public_id: view_id.to_owned(),
            body: ViewInstructionSpan::new(0, 1),
            styles: Vec::new(),
            parameters: Vec::new(),
            state_schema_hash: 0,
        }],
        instructions: vec![ViewProgramInstruction::EmitCustom {
            element: format!("{view_id}.element"),
            styles: Vec::new(),
            part: None,
            source: None,
        }],
        ..ViewProgramResource::default()
    };
    let source = program
        .public_id_table()
        .expect("fixture program public IDs are canonical")
        .id_for(view_id)
        .expect("definition ID is retained in the program public-ID table");
    let ViewProgramInstruction::EmitCustom {
        source: instruction_source,
        ..
    } = &mut program.instructions[0]
    else {
        unreachable!("fixture instruction was constructed as EmitCustom");
    };
    *instruction_source = Some(SourceRangeRef {
        source,
        start_byte: 0,
        end_byte: 1,
    });
    program
}

fn fixture_program() -> ViewProgramResource {
    ViewProgramResource {
        program_id: "view.program.dialogue".to_owned(),
        definitions: vec![ViewDefinitionResource {
            public_id: "view.dialogue".to_owned(),
            body: ViewInstructionSpan::new(0, 4),
            styles: vec![named_style("style.dialogue")],
            parameters: Vec::new(),
            state_schema_hash: 0xD1A1_06A0_0000_0001,
        }],
        value_programs: vec![
            constant_value_program(ViewValueProgramId(0), FxRuntimeValue::I32(1)),
            constant_value_program(
                ViewValueProgramId(1),
                FxRuntimeValue::F32(FiniteF32::try_new(1.0).unwrap()),
            ),
        ],
        value_inputs: vec![],
        instructions: vec![
            ViewProgramInstruction::OpenElement {
                element: ViewElementKind::Column,
                target: None,
                styles: vec![ViewStyleApplicationTarget::inline(ViewStylePatchId::new(7))],
                part: Some("part.root".to_owned()),
                key: Some(7),
                source: None,
            },
            ViewProgramInstruction::EmitText {
                text_source: "text.dialogue.title".to_owned(),
                text_block: "text.block.dialogue.title".to_owned(),
                styles: vec![named_style("style.dialogue.secondary")],
                part: Some("part.title".to_owned()),
                source: None,
            },
            ViewProgramInstruction::Await {
                source_program: ViewValueProgramId(0),
                pending_branch: Some(ViewAwaitBranchSpan {
                    start_offset: 0,
                    body_span: 0,
                }),
                ready_branch: Some(ViewAwaitBranchSpan {
                    start_offset: 0,
                    body_span: 0,
                }),
                error_branch: None,
                denied_branch: None,
                source: None,
            },
            ViewProgramInstruction::CloseElement,
        ],
        handlers: vec![ViewHandlerRef {
            handler_id: "handler.dialogue.submit".to_owned(),
            event: "submit".to_owned(),
            awbc_function_index: 2,
            handler_abi: BundleDigest::of(b"handler-abi"),
            function_binding: None,
        }],
        exported_parts: vec![],
        semantic_targets: vec![ViewSemanticTarget {
            public_id: "semantic.dialogue.title".to_owned(),
            target: "heading".to_owned(),
            view: None,
            label_text_source: Some("text.dialogue.title".to_owned()),
            source: None,
        }],
        layout_bounds: vec![
            ViewLayoutBoundsResource::text_control(
                "input.dialogue.name",
                ViewLogicalRect::from_px(48, 48, 420, 48),
            ),
            ViewLayoutBoundsResource::semantic_target(
                "input.dialogue.name",
                ViewLogicalRect::from_px(48, 48, 420, 48),
            ),
        ],
        scroll_regions: vec![
            ViewScrollRegionResource::new(
                "scroll.dialogue.body",
                Some("view.dialogue".to_owned()),
                ViewLogicalRect::from_px(48, 112, 420, 180),
                640_000,
                360_000,
                ViewScrollAxis::Horizontal,
            )
            .with_overflow(ViewScrollOverflowPolicy::Hidden),
        ],
        surfaces: Vec::new(),
        text_blocks: vec![ViewTextBlockResource::new(
            "text.block.dialogue.title",
            Some("view.dialogue".to_owned()),
            Some("scroll.dialogue.body".to_owned()),
            "text.dialogue.title",
            ViewTextBlockBounds::from_px(48, 112, 420, 24),
        )],
        action_buttons: Vec::new(),
        focus_groups: Vec::new(),
        focus_navigation: Vec::new(),
        adapter_requirements: vec![],
    }
}

fn constant_value_program(id: ViewValueProgramId, value: FxRuntimeValue) -> ViewValueProgram {
    ViewValueProgram::validate(
        id,
        ValueProgramSchema::new(vec![], vec![], value.value_type()),
        vec![
            ValueInstruction::Constant { value },
            ValueInstruction::Return,
        ],
    )
    .unwrap()
}

fn fixture_style() -> ViewStyleResource {
    let sheet_source = ViewStyleSourceId::new(0);
    let patch_source = ViewStyleSourceId::new(1);
    let sheet_id = ViewStyleSheetId::try_new("style.dialogue").expect("valid sheet ID");
    let token_id = ViewStyleTokenId::try_new("token.accent").expect("valid token ID");
    let token = ViewStyleToken::new(
        token_id.clone(),
        ViewStyleValueKind::Color,
        ViewSpecifiedValue::Color {
            value: ViewColorValue::Literal {
                color: PresentationColor::rgb(0x25, 0x63, 0xEB),
            },
        },
        sheet_source,
    )
    .expect("valid token");
    let selector = ViewStyleSelector::new(vec![
        ViewStyleSelectorSequence::new(None, Some(ViewElementKind::Column), None, Vec::new())
            .expect("valid ancestor selector"),
        ViewStyleSelectorSequence::new(
            Some(arcweft_view::ViewStyleCombinator::Child),
            Some(ViewElementKind::Button),
            None,
            vec![ViewStylePredicate::ElementState(
                ViewElementState::FocusVisible,
            )],
        )
        .expect("valid target selector"),
    ])
    .expect("valid selector");
    let declaration = ViewStyleDeclaration::new(
        ViewPropertyKind::BackgroundColor,
        ViewSpecifiedValue::Token {
            token: token_id,
            value_kind: ViewStyleValueKind::Color,
        },
        ViewStyleAssignOp::Replace,
        sheet_source,
    )
    .expect("valid declaration");
    let rule =
        ViewStyleRule::new(selector, vec![declaration], 0, sheet_source).expect("valid rule");
    let sheet =
        ViewStyleSheet::new(sheet_id.clone(), vec![token], vec![rule]).expect("valid sheet");
    let patch = ViewStylePatch::new(
        ViewStylePatchId::new(7),
        vec![
            ViewStyleDeclaration::new(
                ViewPropertyKind::Opacity,
                ViewSpecifiedValue::Ratio {
                    value: arcweft_view::ViewRatioMilli::new(750).expect("valid ratio"),
                },
                ViewStyleAssignOp::Replace,
                patch_source,
            )
            .expect("valid patch declaration"),
        ],
    );
    let mut resource = ViewStyleResource {
        style_program_id: "view.style.program".to_owned(),
        program: ViewStyleProgram::try_new(vec![sheet], vec![patch]).expect("valid Style program"),
        source_map_refs: vec![
            SourceRangeRef {
                source: PublicIdRef::default(),
                start_byte: 0,
                end_byte: 1,
            },
            SourceRangeRef {
                source: PublicIdRef::default(),
                start_byte: 2,
                end_byte: 3,
            },
        ],
        adapter_requirements: Vec::new(),
    };
    let public_ids = resource.public_id_table().expect("valid public IDs");
    resource.source_map_refs[0].source = public_ids
        .id_for(sheet_id.public_id().as_str())
        .expect("sheet owner exists");
    resource.source_map_refs[1].source = public_ids
        .id_for(&resource.style_program_id)
        .expect("program owner exists");
    resource
}

fn named_style(id: &str) -> ViewStyleApplicationTarget {
    ViewStyleApplicationTarget::named(
        ViewStyleSheetId::try_new(id).expect("valid fixture sheet ID"),
    )
}

fn fixture_text() -> ViewTextResource {
    ViewTextResource {
        sources: vec![
            ViewTextSourceRecord {
                public_id: "text.dialogue.content".to_owned(),
                kind: ViewTextSourceKind::Dialogue {
                    parameter: "dialogue".to_owned(),
                    projection: DialogueTextProjection::Content,
                },
                source: None,
            },
            ViewTextSourceRecord {
                public_id: "text.dialogue.name".to_owned(),
                kind: ViewTextSourceKind::Localized {
                    key: "view.dialogue.name".to_owned(),
                    locale: Some("en-US".to_owned()),
                },
                source: None,
            },
            ViewTextSourceRecord {
                public_id: "text.dialogue.title".to_owned(),
                kind: ViewTextSourceKind::Literal {
                    value: "Hello".to_owned(),
                },
                source: None,
            },
        ],
        localized: vec![ViewLocalizedTextResource {
            key: "view.dialogue.name".to_owned(),
            locale: Some("en-US".to_owned()),
            document: RichTextDocument::new(vec![RichTextNode::Text {
                text: "Guest".to_owned(),
            }]),
        }],
        rich_text_documents: vec![],
        display_frames: vec![],
        source_ranges: vec![],
        reveal_policies: vec![],
        cursor_policies: vec![],
        redactions: vec![ViewSecureRedactionMetadata {
            text_source: "text.dialogue.name".to_owned(),
            classification: ViewObserveClassification::AgentMasked,
            replacement: Some("[redacted]".to_owned()),
        }],
    }
}

fn fixture_input(secure_policy: ViewSecureInputPolicy) -> ViewInputResource {
    ViewInputResource {
        options: vec![ViewInputOptions {
            public_id: "input.dialogue.name".to_owned(),
            view: None,
            containing_scroll_region: None,
            kind: ViewInputKind::TextField,
            value_text_source: "text.dialogue.name".to_owned(),
            placeholder_text_source: Some("text.dialogue.placeholder".to_owned()),
            purpose: ViewInputPurpose::Name,
            autocorrect: TextAssistPolicy::Enabled,
            spellcheck: TextAssistPolicy::Enabled,
            capitalization: TextCapitalization::Words,
            enter_key: EnterKeyHint::Done,
            multiline: false,
            selection_policy: ViewTextSelectionPolicy::Enabled,
            shortcut_policy: ViewTextShortcutPolicy::Enabled,
            tab_policy: ViewTextTabPolicy::FocusNavigation,
            vertical_navigation_policy: ViewTextVerticalNavigationPolicy::LogicalLine,
            secure_policy,
            composition_on_blur: CompositionOnBlurPolicy::Commit,
            submit_handler: Some("handler.dialogue.submit".to_owned()),
            change_handler: Some("handler.dialogue.change".to_owned()),
            adapter_requirements: vec![],
        }],
        adapter_requirements: vec![],
    }
}

fn fixture_theme(accent: PresentationColor) -> ViewThemeResource {
    ViewThemeResource {
        palette_overrides: vec![SystemColorOverride {
            color: SystemColor::Accent,
            light: Some(accent),
            dark: Some(PresentationColor::rgb(0x58, 0xA6, 0xFF)),
            source: None,
        }],
        defaults: ViewThemeEnvironmentDefaults {
            color_scheme: ColorSchemePreference::default(),
            contrast: ContrastPreference::Standard,
            reduce_motion: false,
            text_scale_milli: 1_000,
        },
        dark_mode_visual_golden_ids: vec!["golden.view.dialogue.dark".to_owned()],
    }
}
