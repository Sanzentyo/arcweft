use arcweft_bundle::container::{BundleDigest, BundleSectionKind};
use arcweft_bundle::patch::PatchCompatibility;
use arcweft_bundle::resource_codec::ui::{
    ColorSchemeDefault, CompositionOnBlurPolicy, ContrastPreference, EnterKeyHint,
    ExternalCssDescriptorRef, ExternalCssIdentity, RgbaColor, StyleAssignOp, StyleSourceIdentity,
    StyleSourceRef, StyleSyntax, SystemColor, SystemColorOverride, TextAssistPolicy,
    TextCapitalization, UiChildSpan, UiElementKind, UiElementState, UiHandlerRef, UiInputKind,
    UiInputOptions, UiInputPurpose, UiInputResource, UiLayoutBoundsResource, UiLogicalRect,
    UiObserveClassification, UiProgramInstruction, UiProgramResource, UiResourceBudget,
    UiResourceCompatibility, UiSecureInputPolicy, UiSecureRedactionMetadata, UiSemanticTarget,
    UiStateSchemaHashRef, UiStyleDeclaration, UiStyleResource, UiStyleRule, UiStyleSelector,
    UiStyleSelectorPart, UiStyleToken, UiStyleValue, UiTextResource, UiTextSelectionPolicy,
    UiTextShortcutPolicy, UiTextSourceKind, UiTextSourceRecord, UiTextTabPolicy,
    UiTextVerticalNavigationPolicy, UiThemeEnvironmentDefaults, UiThemeResource,
    migrated_ui_section_compatibility,
};
use arcweft_bundle::resource_codec::{
    FieldId, ProductResourceEnvelope, ProductSectionCodecKind, ResourceField, ResourceWireType,
    SectionCodecBudget,
};
use arcweft_bundle::{BundleVirtualFileRef, BundleVirtualFileSpace};

#[test]
fn ui_resource_compact_sections_round_trip_with_deterministic_bytes() {
    let program = fixture_program();
    assert_round_trip(
        ProductSectionCodecKind::UiProgram,
        &program.encode_canonical_section().expect("program encodes"),
        UiProgramResource::decode_canonical_section,
        &program,
    );

    let style = fixture_style();
    assert_round_trip(
        ProductSectionCodecKind::UiStyle,
        &style.encode_canonical_section().expect("style encodes"),
        UiStyleResource::decode_canonical_section,
        &style,
    );

    let text = fixture_text();
    assert_round_trip(
        ProductSectionCodecKind::UiText,
        &text.encode_canonical_section().expect("text encodes"),
        UiTextResource::decode_canonical_section,
        &text,
    );

    let input = fixture_input(UiSecureInputPolicy::Plain);
    assert_round_trip(
        ProductSectionCodecKind::UiInput,
        &input.encode_canonical_section().expect("input encodes"),
        UiInputResource::decode_canonical_section,
        &input,
    );

    let theme = fixture_theme(RgbaColor::rgb(0x25, 0x63, 0xEB));
    assert_round_trip(
        ProductSectionCodecKind::UiTheme,
        &theme.encode_canonical_section().expect("theme encodes"),
        UiThemeResource::decode_canonical_section,
        &theme,
    );
}

#[test]
fn ui_resource_unknown_optional_fields_skip_and_unknown_required_reject() {
    let style = fixture_style();
    let bytes = style.encode_canonical_section().expect("style encodes");
    let envelope = ProductResourceEnvelope::decode_all_fields(
        &bytes,
        ProductSectionCodecKind::UiStyle,
        SectionCodecBudget::default(),
    )
    .expect("envelope decodes");

    let optional_bytes = envelope_with_extra_field(
        &envelope,
        ResourceField::optional(FieldId(30_000), ResourceWireType::Bytes, b"future-ui-style"),
    );
    assert_eq!(
        UiStyleResource::decode_canonical_section(&optional_bytes)
            .expect("unknown optional field skips"),
        style,
    );

    let required_bytes = envelope_with_extra_field(
        &envelope,
        ResourceField::required(FieldId(30_001), ResourceWireType::Bytes, b"future-ui-style"),
    );
    assert!(
        UiStyleResource::decode_canonical_section(&required_bytes).is_err(),
        "unknown required fields must reject for migrated UI resources",
    );
}

#[test]
fn ui_resource_budget_failures_are_reported() {
    let program_bytes = fixture_program()
        .encode_canonical_section()
        .expect("program encodes");
    assert!(
        UiProgramResource::decode_canonical_section_with_budget(
            &program_bytes,
            UiResourceBudget {
                program_instructions: 0,
                ..UiResourceBudget::default()
            },
        )
        .is_err()
    );

    let style_bytes = fixture_style()
        .encode_canonical_section()
        .expect("style encodes");
    assert!(
        UiStyleResource::decode_canonical_section_with_budget(
            &style_bytes,
            UiResourceBudget {
                selector_depth: 0,
                ..UiResourceBudget::default()
            },
        )
        .is_err()
    );
    assert!(
        UiStyleResource::decode_canonical_section_with_budget(
            &style_bytes,
            UiResourceBudget {
                style_tokens: 0,
                ..UiResourceBudget::default()
            },
        )
        .is_err()
    );

    let text_bytes = fixture_text()
        .encode_canonical_section()
        .expect("text encodes");
    assert!(
        UiTextResource::decode_canonical_section_with_budget(
            &text_bytes,
            UiResourceBudget {
                text_sources: 0,
                ..UiResourceBudget::default()
            },
        )
        .is_err()
    );
}

#[test]
fn ui_program_layout_bounds_reject_zero_size_rects() {
    let mut program = fixture_program();
    program
        .layout_bounds
        .push(UiLayoutBoundsResource::text_control(
            "input.dialogue.invalid",
            UiLogicalRect::new(48_000, 48_000, 0, 48_000),
        ));

    assert!(
        program.encode_canonical_section().is_err(),
        "zero-width layout bounds are not canonical UI resources",
    );
}

#[test]
fn ui_style_external_css_descriptor_refs_preserve_file_vs_embed_identity() {
    let style = fixture_style();
    let bytes = style.encode_canonical_section().expect("style encodes");
    let decoded = UiStyleResource::decode_canonical_section(&bytes).expect("style decodes");

    assert!(decoded.external_css_descriptors.iter().any(|descriptor| {
        matches!(descriptor.identity, ExternalCssIdentity::File { ref path } if path == "ui/dialogue.css")
    }));
    assert!(decoded.external_css_descriptors.iter().any(|descriptor| {
        matches!(descriptor.identity, ExternalCssIdentity::EmbeddedFile { ref file } if file.path == "ui/default.css")
    }));
}

#[test]
fn ui_theme_palette_changes_are_content_only() {
    let old = fixture_theme(RgbaColor::rgb(0x25, 0x63, 0xEB));
    let new = fixture_theme(RgbaColor::rgb(0x58, 0xA6, 0xFF));

    assert_eq!(
        old.compatibility_with(&new),
        UiResourceCompatibility::ContentOnly
    );
    assert_eq!(
        migrated_ui_section_compatibility(
            BundleSectionKind::UiTheme,
            &old.encode_canonical_section().expect("old encodes"),
            &new.encode_canonical_section().expect("new encodes"),
        )
        .expect("compatibility decodes"),
        Some(PatchCompatibility::ContentOnly),
    );
}

#[test]
fn ui_input_secure_policy_changes_are_restart_required() {
    let old = fixture_input(UiSecureInputPolicy::Plain);
    let new = fixture_input(UiSecureInputPolicy::Password);

    assert_eq!(
        old.compatibility_with(&new),
        UiResourceCompatibility::RestartRequired,
    );
    assert_eq!(
        migrated_ui_section_compatibility(
            BundleSectionKind::UiInput,
            &old.encode_canonical_section().expect("old encodes"),
            &new.encode_canonical_section().expect("new encodes"),
        )
        .expect("compatibility decodes"),
        Some(PatchCompatibility::RestartRequired),
    );
}

#[test]
fn ui_resource_source_gate_rejects_json_fallback() {
    let json = br#"{"style_program_id":"style.dialogue","tokens":[]}"#;

    assert!(
        UiStyleResource::decode_canonical_section(json).is_err(),
        "migrated UI resource decode must require compact AWFB section magic",
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

impl EncodeAgain for UiProgramResource {
    fn encode_again(&self, codec: ProductSectionCodecKind) -> Vec<u8> {
        assert_eq!(codec, ProductSectionCodecKind::UiProgram);
        self.encode_canonical_section().expect("program re-encodes")
    }
}

impl EncodeAgain for UiStyleResource {
    fn encode_again(&self, codec: ProductSectionCodecKind) -> Vec<u8> {
        assert_eq!(codec, ProductSectionCodecKind::UiStyle);
        self.encode_canonical_section().expect("style re-encodes")
    }
}

impl EncodeAgain for UiTextResource {
    fn encode_again(&self, codec: ProductSectionCodecKind) -> Vec<u8> {
        assert_eq!(codec, ProductSectionCodecKind::UiText);
        self.encode_canonical_section().expect("text re-encodes")
    }
}

impl EncodeAgain for UiInputResource {
    fn encode_again(&self, codec: ProductSectionCodecKind) -> Vec<u8> {
        assert_eq!(codec, ProductSectionCodecKind::UiInput);
        self.encode_canonical_section().expect("input re-encodes")
    }
}

impl EncodeAgain for UiThemeResource {
    fn encode_again(&self, codec: ProductSectionCodecKind) -> Vec<u8> {
        assert_eq!(codec, ProductSectionCodecKind::UiTheme);
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

fn fixture_program() -> UiProgramResource {
    UiProgramResource {
        program_id: "ui.program.dialogue".to_owned(),
        root_component: "component.dialogue".to_owned(),
        instructions: vec![
            UiProgramInstruction::OpenElement {
                element: UiElementKind::Column,
                style: Some("style.dialogue".to_owned()),
                part: Some("part.root".to_owned()),
                key: Some(7),
                source: None,
            },
            UiProgramInstruction::EmitText {
                text_source: "text.dialogue.title".to_owned(),
                style: Some("style.dialogue.title".to_owned()),
                part: Some("part.title".to_owned()),
                source: None,
            },
            UiProgramInstruction::CloseElement,
        ],
        child_spans: vec![UiChildSpan::new(1, 2)],
        handlers: vec![UiHandlerRef {
            handler_id: "handler.dialogue.submit".to_owned(),
            event: "submit".to_owned(),
            awbc_function_index: 2,
            handler_abi: BundleDigest::of(b"handler-abi"),
            function_binding: None,
        }],
        state_schema_hashes: vec![UiStateSchemaHashRef {
            public_id: Some("state.dialogue".to_owned()),
            hash: BundleDigest::of(b"state-schema"),
        }],
        exported_parts: vec![],
        semantic_targets: vec![UiSemanticTarget {
            public_id: "semantic.dialogue.title".to_owned(),
            target: "heading".to_owned(),
            label_text_source: Some("text.dialogue.title".to_owned()),
            source: None,
        }],
        layout_bounds: vec![
            UiLayoutBoundsResource::text_control(
                "input.dialogue.name",
                UiLogicalRect::from_px(48, 48, 420, 48),
            ),
            UiLayoutBoundsResource::semantic_target(
                "input.dialogue.name",
                UiLogicalRect::from_px(48, 48, 420, 48),
            ),
        ],
        action_buttons: Vec::new(),
        focus_groups: Vec::new(),
        focus_navigation: Vec::new(),
        adapter_requirements: vec![],
    }
}

fn fixture_style() -> UiStyleResource {
    UiStyleResource {
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
                path: "ui/dialogue.css".to_owned(),
            },
            content_digest: Some(BundleDigest::of(b"dialogue-css")),
        }],
        tokens: vec![UiStyleToken {
            public_id: "token.accent".to_owned(),
            value: UiStyleValue::SystemColor(SystemColor::Accent),
        }],
        rules: vec![UiStyleRule {
            selector: UiStyleSelector {
                parts: vec![
                    UiStyleSelectorPart::Element(UiElementKind::Button),
                    UiStyleSelectorPart::State(UiElementState::FocusVisible),
                ],
            },
            declarations: vec![UiStyleDeclaration {
                property: "border_color".to_owned(),
                value: UiStyleValue::Token("token.accent".to_owned()),
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
                        path: "ui/default.css".to_owned(),
                    },
                },
                source_map: None,
            },
            ExternalCssDescriptorRef {
                public_id: "css.file.dialogue".to_owned(),
                identity: ExternalCssIdentity::File {
                    path: "ui/dialogue.css".to_owned(),
                },
                source_map: None,
            },
        ],
        adapter_requirements: vec![],
    }
}

fn fixture_text() -> UiTextResource {
    UiTextResource {
        sources: vec![
            UiTextSourceRecord {
                public_id: "text.dialogue.name".to_owned(),
                kind: UiTextSourceKind::Localized {
                    key: "ui.dialogue.name".to_owned(),
                    locale: Some("en-US".to_owned()),
                },
                source: None,
            },
            UiTextSourceRecord {
                public_id: "text.dialogue.title".to_owned(),
                kind: UiTextSourceKind::Literal {
                    value: "Hello".to_owned(),
                },
                source: None,
            },
        ],
        display_frame_refs: vec![],
        source_ranges: vec![],
        reveal_policies: vec![],
        cursor_policies: vec![],
        redactions: vec![UiSecureRedactionMetadata {
            text_source: "text.dialogue.name".to_owned(),
            classification: UiObserveClassification::AgentMasked,
            replacement: Some("[redacted]".to_owned()),
        }],
    }
}

fn fixture_input(secure_policy: UiSecureInputPolicy) -> UiInputResource {
    UiInputResource {
        options: vec![UiInputOptions {
            public_id: "input.dialogue.name".to_owned(),
            kind: UiInputKind::TextField,
            value_text_source: "text.dialogue.name".to_owned(),
            placeholder_text_source: Some("text.dialogue.placeholder".to_owned()),
            purpose: UiInputPurpose::Name,
            autocorrect: TextAssistPolicy::Enabled,
            spellcheck: TextAssistPolicy::Enabled,
            capitalization: TextCapitalization::Words,
            enter_key: EnterKeyHint::Done,
            multiline: false,
            selection_policy: UiTextSelectionPolicy::Enabled,
            shortcut_policy: UiTextShortcutPolicy::Enabled,
            tab_policy: UiTextTabPolicy::FocusNavigation,
            vertical_navigation_policy: UiTextVerticalNavigationPolicy::LogicalLine,
            secure_policy,
            composition_on_blur: CompositionOnBlurPolicy::Commit,
            submit_handler: Some("handler.dialogue.submit".to_owned()),
            change_handler: Some("handler.dialogue.change".to_owned()),
            adapter_requirements: vec![],
        }],
        adapter_requirements: vec![],
    }
}

fn fixture_theme(accent: RgbaColor) -> UiThemeResource {
    UiThemeResource {
        palette_overrides: vec![SystemColorOverride {
            color: SystemColor::Accent,
            light: Some(accent),
            dark: Some(RgbaColor::rgb(0x58, 0xA6, 0xFF)),
            source: None,
        }],
        defaults: UiThemeEnvironmentDefaults {
            color_scheme: ColorSchemeDefault::default(),
            contrast: ContrastPreference::Standard,
            reduce_motion: false,
            text_scale_milli: 1_000,
        },
        dark_mode_visual_golden_ids: vec!["golden.ui.dialogue.dark".to_owned()],
    }
}
