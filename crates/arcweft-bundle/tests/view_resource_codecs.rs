use arcweft_bundle::container::{BundleDigest, BundleSectionKind};
use arcweft_bundle::patch::PatchCompatibility;
use arcweft_bundle::resource_codec::view::{
    ColorSchemeDefault, CompositionOnBlurPolicy, ContrastPreference, EnterKeyHint,
    ExternalCssDescriptorRef, ExternalCssIdentity, RgbaColor, StyleAssignOp, StyleSourceIdentity,
    StyleSourceRef, StyleSyntax, SystemColor, SystemColorOverride, TextAssistPolicy,
    TextCapitalization, ViewAwaitBranchSpan, ViewChildSpan, ViewElementKind, ViewElementState,
    ViewHandlerRef, ViewInputKind, ViewInputOptions, ViewInputPurpose, ViewInputResource,
    ViewLayoutBoundsResource, ViewLogicalRect, ViewObserveClassification, ViewProgramInstruction,
    ViewProgramResource, ViewResourceBudget, ViewResourceCompatibility, ViewRuntimeTextBlockBounds,
    ViewScrollAxis, ViewScrollOverflowPolicy, ViewScrollRegionResource, ViewSecureInputPolicy,
    ViewSecureRedactionMetadata, ViewSemanticTarget, ViewStateSchemaHashRef, ViewStyleDeclaration,
    ViewStyleResource, ViewStyleRule, ViewStyleSelector, ViewStyleSelectorPart, ViewStyleToken,
    ViewStyleValue, ViewTextBlockResource, ViewTextResource, ViewTextSelectionPolicy,
    ViewTextShortcutPolicy, ViewTextSourceKind, ViewTextSourceRecord, ViewTextTabPolicy,
    ViewTextVerticalNavigationPolicy, ViewThemeEnvironmentDefaults, ViewThemeResource,
    migrated_view_section_compatibility,
};
use arcweft_bundle::resource_codec::{
    DigestRef, FieldId, ProductResourceEnvelope, ProductSectionCodecKind, ResourceField,
    ResourceWireType, SectionCodecBudget,
};
use arcweft_bundle::{BundleVirtualFileRef, BundleVirtualFileSpace};

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

    let theme = fixture_theme(RgbaColor::rgb(0x25, 0x63, 0xEB));
    assert_round_trip(
        ProductSectionCodecKind::ViewTheme,
        &theme.encode_canonical_section().expect("theme encodes"),
        ViewThemeResource::decode_canonical_section,
        &theme,
    );
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
fn view_style_external_css_descriptor_refs_preserve_file_vs_embed_identity() {
    let style = fixture_style();
    let bytes = style.encode_canonical_section().expect("style encodes");
    let decoded = ViewStyleResource::decode_canonical_section(&bytes).expect("style decodes");

    assert!(decoded.external_css_descriptors.iter().any(|descriptor| {
        matches!(descriptor.identity, ExternalCssIdentity::File { ref path } if path == "view/dialogue.css")
    }));
    assert!(decoded.external_css_descriptors.iter().any(|descriptor| {
        matches!(descriptor.identity, ExternalCssIdentity::EmbeddedFile { ref file } if file.path == "view/default.css")
    }));
}

#[test]
fn view_theme_palette_changes_are_content_only() {
    let old = fixture_theme(RgbaColor::rgb(0x25, 0x63, 0xEB));
    let new = fixture_theme(RgbaColor::rgb(0x58, 0xA6, 0xFF));

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
fn view_resource_source_gate_rejects_json_fallback() {
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
    T: EncodeAgain + Eq + std::fmt::Debug + PartialEq,
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
    let fields: Vec<ResourceField> = envelope
        .fields
        .iter()
        .map(|field| {
            if field.id == FieldId(1) {
                ResourceField::new(
                    field.id,
                    field.requirement,
                    field.wire_type,
                    field.nesting_depth,
                    field.reference_count,
                    updated_transcript.clone(),
                )
            } else {
                field.clone()
            }
        })
        .collect();
    ProductResourceEnvelope::new(
        envelope.header.codec,
        envelope.strings,
        envelope.public_ids,
        envelope.enums,
        fields,
        envelope.header.record_count,
    )
    .expect("envelope rebuilds")
    .encode_canonical()
    .expect("envelope re-encodes")
}

fn fixture_program() -> ViewProgramResource {
    ViewProgramResource {
        program_id: "view.program.dialogue".to_owned(),
        root_view: "view.dialogue".to_owned(),
        instructions: vec![
            ViewProgramInstruction::OpenElement {
                element: ViewElementKind::Column,
                target: None,
                style: Some("style.dialogue".to_owned()),
                part: Some("part.root".to_owned()),
                key: Some(7),
                source: None,
            },
            ViewProgramInstruction::EmitText {
                text_source: "text.dialogue.title".to_owned(),
                style: Some("style.dialogue.title".to_owned()),
                part: Some("part.title".to_owned()),
                source: None,
            },
            ViewProgramInstruction::Await {
                source_schema: DigestRef {
                    digest: BundleDigest::of(b"avatar-need-schema"),
                },
                pending_branch: Some(ViewAwaitBranchSpan {
                    pattern_schema: DigestRef {
                        digest: BundleDigest::of(b"pending-pattern"),
                    },
                    body_span: 1,
                }),
                ready_branch: Some(ViewAwaitBranchSpan {
                    pattern_schema: DigestRef {
                        digest: BundleDigest::of(b"ready-pattern"),
                    },
                    body_span: 1,
                }),
                error_branch: None,
                denied_branch: None,
                source: None,
            },
            ViewProgramInstruction::CloseElement,
        ],
        child_spans: vec![ViewChildSpan::new(1, 2)],
        handlers: vec![ViewHandlerRef {
            handler_id: "handler.dialogue.submit".to_owned(),
            event: "submit".to_owned(),
            awbc_function_index: 2,
            handler_abi: BundleDigest::of(b"handler-abi"),
            function_binding: None,
        }],
        state_schema_hashes: vec![ViewStateSchemaHashRef {
            public_id: Some("state.dialogue".to_owned()),
            hash: BundleDigest::of(b"state-schema"),
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
            ViewRuntimeTextBlockBounds::from_px(48, 112, 420, 24),
        )],
        action_buttons: Vec::new(),
        focus_groups: Vec::new(),
        focus_navigation: Vec::new(),
        adapter_requirements: vec![],
    }
}

fn fixture_style() -> ViewStyleResource {
    ViewStyleResource {
        style_program_id: "style.dialogue".to_owned(),
        arcweft_sources: vec![StyleSourceIdentity {
            public_id: "style.dialogue.arcw".to_owned(),
            syntax: StyleSyntax::Arcweft,
            identity: StyleSourceRef::Inline {
                source_digest: BundleDigest::of(b"opacity: 1"),
            },
            content_digest: None,
        }],
        css_sources: vec![StyleSourceIdentity {
            public_id: "style.dialogue.css".to_owned(),
            syntax: StyleSyntax::Css,
            identity: StyleSourceRef::File {
                path: "view/dialogue.css".to_owned(),
            },
            content_digest: Some(BundleDigest::of(b"dialogue-css")),
        }],
        tokens: vec![ViewStyleToken {
            public_id: "token.accent".to_owned(),
            value: ViewStyleValue::SystemColor(SystemColor::Accent),
        }],
        rules: vec![ViewStyleRule {
            selector: ViewStyleSelector {
                parts: vec![
                    ViewStyleSelectorPart::Element(ViewElementKind::Button),
                    ViewStyleSelectorPart::State(ViewElementState::FocusVisible),
                ],
            },
            declarations: vec![ViewStyleDeclaration {
                property: "border_color".to_owned(),
                value: ViewStyleValue::Token("token.accent".to_owned()),
                op: StyleAssignOp::Replace,
            }],
            source: None,
        }],
        part_rules: vec![],
        environment_predicates: vec![],
        source_map_refs: vec![],
        external_css_descriptors: vec![
            ExternalCssDescriptorRef {
                public_id: "css.embed.default".to_owned(),
                identity: ExternalCssIdentity::EmbeddedFile {
                    file: BundleVirtualFileRef {
                        space: BundleVirtualFileSpace::Asset,
                        path: "view/default.css".to_owned(),
                    },
                },
                source_map: None,
            },
            ExternalCssDescriptorRef {
                public_id: "css.file.dialogue".to_owned(),
                identity: ExternalCssIdentity::File {
                    path: "view/dialogue.css".to_owned(),
                },
                source_map: None,
            },
        ],
        adapter_requirements: vec![],
    }
}

fn fixture_text() -> ViewTextResource {
    ViewTextResource {
        sources: vec![
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
        display_frame_refs: vec![],
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

fn fixture_theme(accent: RgbaColor) -> ViewThemeResource {
    ViewThemeResource {
        palette_overrides: vec![SystemColorOverride {
            color: SystemColor::Accent,
            light: Some(accent),
            dark: Some(RgbaColor::rgb(0x58, 0xA6, 0xFF)),
            source: None,
        }],
        defaults: ViewThemeEnvironmentDefaults {
            color_scheme: ColorSchemeDefault::default(),
            contrast: ContrastPreference::Standard,
            reduce_motion: false,
            text_scale_milli: 1_000,
        },
        dark_mode_visual_golden_ids: vec!["golden.view.dialogue.dark".to_owned()],
    }
}
