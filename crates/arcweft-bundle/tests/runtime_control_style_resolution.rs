use arcweft_bundle::resource_codec::ViewTextResource;
use arcweft_bundle::resource_codec::view::{
    CompositionOnBlurPolicy, EnterKeyHint, RgbaColor, StyleAssignOp, TextAssistPolicy,
    TextCapitalization, ViewElementKind, ViewElementState, ViewInputKind, ViewInputPurpose,
    ViewInteractionState, ViewPartStyleRule, ViewProgramInstruction, ViewProgramResource,
    ViewRuntimeActionButton, ViewRuntimeActionButtonAction, ViewRuntimeButtonBounds,
    ViewRuntimeControlCornerFrameStyle, ViewRuntimeControlCornerRadius, ViewRuntimeControlFilter,
    ViewRuntimeControlRadii, ViewRuntimeControlState, ViewRuntimeControlStyle,
    ViewRuntimeControlStyleDiagnosticReason, ViewRuntimeControlStyleResolution,
    ViewRuntimeSurfaceBounds, ViewRuntimeTextBlockBounds, ViewRuntimeTextControl,
    ViewRuntimeTextControlBounds, ViewRuntimeTextControlHandlers, ViewRuntimeTextControlOptions,
    ViewRuntimeTextSelection, ViewSecureInputPolicy, ViewStyleApplyRef, ViewStyleDeclaration,
    ViewStyleResource, ViewStyleRule, ViewStyleSelector, ViewStyleSelectorPart, ViewStyleToken,
    ViewStyleValue, ViewSurfaceResource, ViewTextBlockResource, ViewTextSelectionPolicy,
    ViewTextShortcutPolicy, ViewTextSourceKind, ViewTextSourceRecord, ViewTextTabPolicy,
    ViewTextVerticalNavigationPolicy,
};

#[test]
fn text_control_resolves_authored_background_alpha_and_border_color() {
    let style = ViewStyleResource {
        rules: vec![rule(
            ViewStyleSelectorPart::Element(ViewElementKind::TextField),
            vec![
                decl("background-color", rgba(16, 24, 32, 180)),
                decl("border-color", rgba(80, 112, 96, 255)),
                decl("border-width", ViewStyleValue::Milli(2_000)),
                decl("opacity", ViewStyleValue::Milli(720)),
                decl("z-index", ViewStyleValue::Milli(2_500)),
            ],
        )],
        ..ViewStyleResource::default()
    };

    let resolved =
        resolve_text_control_style_for_test(&style, "input.feedback", ViewInputKind::TextField);
    let normal = resolved
        .style
        .visual_for_state(ViewRuntimeControlState::Normal);

    assert_eq!(normal.fill, Some(RgbaColor::rgba(16, 24, 32, 180)));
    assert_eq!(normal.opacity_milli, Some(720));
    assert_eq!(normal.depth_milli, Some(2_500));
    assert_eq!(
        normal.border.expect("border style").color,
        RgbaColor::rgb(80, 112, 96)
    );
    assert!(resolved.diagnostics.is_empty());
}

#[test]
fn runtime_control_depth_overlays_for_interaction_state() {
    let style = ViewStyleResource {
        rules: vec![
            rule(
                ViewStyleSelectorPart::Element(ViewElementKind::Button),
                vec![decl("depth", ViewStyleValue::Text("1000".to_owned()))],
            ),
            state_rule(
                ViewInteractionState::Hover,
                decl("z-index", ViewStyleValue::Text("3000".to_owned())),
            ),
        ],
        ..ViewStyleResource::default()
    };

    let resolved = resolve_button_style_for_test(&style, "button.submit_feedback");

    assert_eq!(
        resolved
            .style
            .visual_for_state(ViewRuntimeControlState::Normal)
            .depth_milli,
        Some(1_000)
    );
    assert_eq!(
        resolved
            .style
            .visual_for_state(ViewRuntimeControlState::Hover)
            .depth_milli,
        Some(3_000)
    );
    assert!(resolved.diagnostics.is_empty());
}

#[test]
fn text_control_resolves_selection_and_caret_colors() {
    let style = ViewStyleResource {
        rules: vec![rule(
            ViewStyleSelectorPart::Element(ViewElementKind::TextArea),
            vec![
                decl("selection-color", rgba(64, 128, 200, 160)),
                decl("caret-color", rgba(240, 220, 90, 255)),
            ],
        )],
        ..ViewStyleResource::default()
    };

    let resolved =
        resolve_text_control_style_for_test(&style, "input.message", ViewInputKind::TextArea);
    let normal = resolved
        .style
        .visual_for_state(ViewRuntimeControlState::Normal);

    assert_eq!(normal.selection, Some(RgbaColor::rgba(64, 128, 200, 160)));
    assert_eq!(normal.caret, Some(RgbaColor::rgb(240, 220, 90)));
    assert!(resolved.diagnostics.is_empty());
}

#[test]
fn text_control_inherits_panel_font_family() {
    let style = ViewStyleResource {
        tokens: vec![ViewStyleToken {
            public_id: "font.view_stack".to_owned(),
            value: ViewStyleValue::List(vec![
                ViewStyleValue::Text("Arcweft Demo".to_owned()),
                ViewStyleValue::Text("Yu Gothic View".to_owned()),
                ViewStyleValue::Text("Yu Gothic".to_owned()),
                ViewStyleValue::Text("Meiryo".to_owned()),
                ViewStyleValue::Text("Noto Sans JP".to_owned()),
                ViewStyleValue::Text("system-view".to_owned()),
            ]),
        }],
        rules: vec![rule(
            ViewStyleSelectorPart::Element(ViewElementKind::Panel),
            vec![decl(
                "font-family",
                ViewStyleValue::Token("font.view_stack".to_owned()),
            )],
        )],
        ..ViewStyleResource::default()
    };

    let resolved =
        resolve_text_control_style_for_test(&style, "input.message", ViewInputKind::TextArea);
    let normal = resolved
        .style
        .visual_for_state(ViewRuntimeControlState::Normal);

    assert_eq!(
        normal.font_family.as_deref(),
        Some("Arcweft Demo, Yu Gothic View, Yu Gothic, Meiryo, Noto Sans JP, system-view")
    );
    assert!(resolved.diagnostics.is_empty());
}

#[test]
fn text_control_font_family_overrides_panel_inheritance() {
    let style = ViewStyleResource {
        rules: vec![
            rule(
                ViewStyleSelectorPart::Element(ViewElementKind::Panel),
                vec![decl(
                    "font-family",
                    ViewStyleValue::Text("Yu Gothic".to_owned()),
                )],
            ),
            rule(
                ViewStyleSelectorPart::Element(ViewElementKind::TextArea),
                vec![decl(
                    "font-family",
                    ViewStyleValue::Text("Noto Sans JP".to_owned()),
                )],
            ),
        ],
        ..ViewStyleResource::default()
    };

    let resolved =
        resolve_text_control_style_for_test(&style, "input.message", ViewInputKind::TextArea);
    let normal = resolved
        .style
        .visual_for_state(ViewRuntimeControlState::Normal);

    assert_eq!(normal.font_family.as_deref(), Some("Noto Sans JP"));
    assert!(resolved.diagnostics.is_empty());
}

#[test]
fn text_control_styles_bind_by_program_target_not_resource_order() {
    let style = ViewStyleResource {
        rules: vec![
            rule(
                ViewStyleSelectorPart::Element(ViewElementKind::TextField),
                vec![decl("background-color", rgba(10, 20, 30, 255))],
            ),
            rule(
                ViewStyleSelectorPart::Element(ViewElementKind::TextArea),
                vec![
                    decl("color", rgba(240, 245, 250, 255)),
                    decl("corner-frame-color", rgba(158, 245, 231, 210)),
                    decl("corner-frame-width", ViewStyleValue::Milli(3_000)),
                ],
            ),
        ],
        ..ViewStyleResource::default()
    };
    let program = ViewProgramResource {
        instructions: vec![
            ViewProgramInstruction::OpenElement {
                element: ViewElementKind::Panel,
                target: None,
                style: None,
                part: None,
                key: None,
                source: None,
            },
            ViewProgramInstruction::OpenElement {
                element: ViewElementKind::TextField,
                target: Some("input.visitor_name".to_owned()),
                style: None,
                part: None,
                key: None,
                source: None,
            },
            ViewProgramInstruction::CloseElement,
            ViewProgramInstruction::OpenElement {
                element: ViewElementKind::TextArea,
                target: Some("input.product_brief".to_owned()),
                style: None,
                part: None,
                key: None,
                source: None,
            },
            ViewProgramInstruction::CloseElement,
            ViewProgramInstruction::CloseElement,
        ],
        ..ViewProgramResource::default()
    };
    let mut controls = vec![
        runtime_text_control("input.product_brief", ViewInputKind::TextArea),
        runtime_text_control("input.visitor_name", ViewInputKind::TextField),
    ];
    let mut buttons = Vec::new();
    let mut text_blocks = Vec::new();

    let diagnostics =
        program.apply_runtime_styles(&style, &mut controls, &mut buttons, &mut text_blocks);

    let product_brief = controls
        .iter()
        .find(|control| control.public_id == "input.product_brief")
        .expect("product brief control");
    let product_visual = product_brief
        .style
        .visual_for_state(ViewRuntimeControlState::Normal);
    assert_eq!(product_visual.text, Some(RgbaColor::rgb(240, 245, 250)));
    assert_eq!(
        product_visual.corner_frame.expect("corner frame").color,
        RgbaColor::rgba(158, 245, 231, 210)
    );

    let visitor_name = controls
        .iter()
        .find(|control| control.public_id == "input.visitor_name")
        .expect("visitor name control");
    let visitor_visual = visitor_name
        .style
        .visual_for_state(ViewRuntimeControlState::Normal);
    assert_eq!(visitor_visual.fill, Some(RgbaColor::rgb(10, 20, 30)));
    assert!(diagnostics.is_empty());
}

#[test]
fn view_program_cascade_styles_text_blocks_with_child_and_inline_rules() {
    let text = ViewTextResource {
        sources: vec![ViewTextSourceRecord {
            public_id: "text.title".to_owned(),
            kind: ViewTextSourceKind::Literal {
                value: "Control deck".to_owned(),
            },
            source: None,
        }],
        ..ViewTextResource::default()
    };
    let program = ViewProgramResource {
        instructions: vec![
            ViewProgramInstruction::OpenElement {
                element: ViewElementKind::Panel,
                target: None,
                style: None,
                part: None,
                key: None,
                source: None,
            },
            ViewProgramInstruction::EmitText {
                text_source: "text.title".to_owned(),
                style: None,
                part: Some("headline".to_owned()),
                source: None,
            },
            ViewProgramInstruction::ApplyStyle {
                style: ViewStyleApplyRef::InlineArcweft { patch_id: 0 },
                source: None,
            },
            ViewProgramInstruction::CloseElement,
        ],
        text_blocks: vec![ViewTextBlockResource::new(
            "text.block.title",
            None,
            None,
            "text.title",
            ViewRuntimeTextBlockBounds::from_px(0, 0, 320, 48),
        )],
        ..ViewProgramResource::default()
    };
    let style = ViewStyleResource {
        rules: vec![
            rule(
                ViewStyleSelectorPart::Element(ViewElementKind::Panel),
                vec![decl(
                    "font-family",
                    ViewStyleValue::Text("Yu Gothic View".to_owned()),
                )],
            ),
            ViewStyleRule {
                selector: ViewStyleSelector {
                    parts: vec![
                        ViewStyleSelectorPart::Element(ViewElementKind::Panel),
                        ViewStyleSelectorPart::Child,
                        ViewStyleSelectorPart::Part("headline".to_owned()),
                    ],
                },
                declarations: vec![decl("color", rgba(244, 247, 251, 255))],
                source: None,
            },
        ],
        part_rules: vec![ViewPartStyleRule {
            part: ViewStyleApplyRef::inline_patch_part(0),
            selector: ViewStyleSelector::default(),
            declarations: vec![
                decl("font-size", ViewStyleValue::Milli(36_000)),
                decl("font-weight", ViewStyleValue::Text("720".to_owned())),
            ],
            source: None,
        }],
        ..ViewStyleResource::default()
    };

    let blocks = program.runtime_text_blocks_with_style(Some(&text), Some(&style));
    let normal = blocks.controls[0]
        .style
        .visual_for_state(ViewRuntimeControlState::Normal);

    assert_eq!(blocks.controls[0].text, "Control deck");
    assert_eq!(normal.font_family.as_deref(), Some("Yu Gothic View"));
    assert_eq!(normal.text, Some(RgbaColor::rgb(244, 247, 251)));
    assert_eq!(normal.font_size_milli, Some(36_000));
    assert_eq!(normal.font_weight, Some(720));
    assert!(blocks.diagnostics.is_empty());
}

#[test]
fn milli_function_and_suffix_lengths_resolve_equally() {
    let function_style = ViewStyleResource {
        rules: vec![rule(
            ViewStyleSelectorPart::Element(ViewElementKind::TextField),
            vec![decl(
                "font-size",
                ViewStyleValue::Text("milli(36000)".to_owned()),
            )],
        )],
        ..ViewStyleResource::default()
    };
    let suffix_style = ViewStyleResource {
        rules: vec![rule(
            ViewStyleSelectorPart::Element(ViewElementKind::TextField),
            vec![decl(
                "font-size",
                ViewStyleValue::Text("36000milli".to_owned()),
            )],
        )],
        ..ViewStyleResource::default()
    };

    let function_visual = resolve_text_control_style_for_test(
        &function_style,
        "input.feedback",
        ViewInputKind::TextField,
    )
    .style
    .visual_for_state(ViewRuntimeControlState::Normal);
    let suffix_visual = resolve_text_control_style_for_test(
        &suffix_style,
        "input.feedback",
        ViewInputKind::TextField,
    )
    .style
    .visual_for_state(ViewRuntimeControlState::Normal);

    assert_eq!(function_visual.font_size_milli, Some(36_000));
    assert_eq!(
        function_visual.font_size_milli,
        suffix_visual.font_size_milli
    );
}

#[test]
fn text_control_resolves_corner_frame_decoration() {
    let style = ViewStyleResource {
        rules: vec![rule(
            ViewStyleSelectorPart::Element(ViewElementKind::TextArea),
            vec![
                decl("corner-frame-color", rgba(94, 234, 212, 220)),
                decl("corner-frame-width", ViewStyleValue::Milli(3_000)),
                decl("corner-frame-length", ViewStyleValue::Milli(24_000)),
                decl("corner-frame-offset", ViewStyleValue::Milli(2_000)),
            ],
        )],
        ..ViewStyleResource::default()
    };

    let resolved =
        resolve_text_control_style_for_test(&style, "input.message", ViewInputKind::TextArea);
    let normal = resolved
        .style
        .visual_for_state(ViewRuntimeControlState::Normal);

    assert_eq!(
        normal.corner_frame,
        Some(ViewRuntimeControlCornerFrameStyle {
            color: RgbaColor::rgba(94, 234, 212, 220),
            width_milli: 3_000,
            length_milli: 24_000,
            offset_milli: 2_000,
        })
    );
    assert!(resolved.diagnostics.is_empty());
}

#[test]
fn button_hover_pressed_and_disabled_selectors_resolve_deterministically() {
    let style = ViewStyleResource {
        rules: vec![
            rule(
                ViewStyleSelectorPart::Element(ViewElementKind::Button),
                vec![decl("background-color", rgba(20, 30, 40, 255))],
            ),
            state_rule(
                ViewInteractionState::Hover,
                decl("background-color", rgba(40, 60, 80, 255)),
            ),
            state_rule(
                ViewInteractionState::Active,
                decl("background-color", rgba(70, 90, 110, 255)),
            ),
            state_rule(
                ViewInteractionState::Disabled,
                decl("background-color", rgba(12, 12, 12, 160)),
            ),
        ],
        ..ViewStyleResource::default()
    };

    let resolved = resolve_button_style_for_test(&style, "button.submit_feedback");

    assert_eq!(
        resolved
            .style
            .visual_for_state(ViewRuntimeControlState::Hover)
            .fill,
        Some(RgbaColor::rgb(40, 60, 80))
    );
    assert_eq!(
        resolved
            .style
            .visual_for_state(ViewRuntimeControlState::Pressed)
            .fill,
        Some(RgbaColor::rgb(70, 90, 110))
    );
    assert_eq!(
        resolved
            .style
            .visual_for_state(ViewRuntimeControlState::Disabled)
            .fill,
        Some(RgbaColor::rgba(12, 12, 12, 160))
    );
}

#[test]
fn focus_visible_ring_and_supported_box_shadow_are_typed() {
    let style = ViewStyleResource {
        rules: vec![
            rule(
                ViewStyleSelectorPart::Element(ViewElementKind::TextArea),
                vec![
                    decl("border-radius", ViewStyleValue::Milli(12_000)),
                    decl(
                        "box-shadow",
                        ViewStyleValue::Text("0px 8px 20px 0px rgba(0,0,0,0.35)".to_owned()),
                    ),
                ],
            ),
            ViewStyleRule {
                selector: ViewStyleSelector {
                    parts: vec![
                        ViewStyleSelectorPart::Element(ViewElementKind::TextArea),
                        ViewStyleSelectorPart::State(ViewElementState::FocusVisible),
                    ],
                },
                declarations: vec![
                    decl("focus-ring-color", rgba(226, 233, 98, 255)),
                    decl("focus-ring-width", ViewStyleValue::Milli(3_000)),
                ],
                source: None,
            },
        ],
        ..ViewStyleResource::default()
    };

    let resolved =
        resolve_text_control_style_for_test(&style, "input.message", ViewInputKind::TextArea);
    let normal = resolved
        .style
        .visual_for_state(ViewRuntimeControlState::Normal);
    let focused = resolved
        .style
        .visual_for_state(ViewRuntimeControlState::FocusVisible);

    assert_eq!(normal.radius_milli, Some(12_000));
    assert_eq!(normal.shadows.len(), 1);
    assert_eq!(normal.shadows[0].blur_milli, 20_000);
    assert_eq!(focused.focus_ring.expect("focus ring").width_milli, 3_000);
}

#[test]
fn surface_style_resolves_radius_fill_and_box_shadow() {
    let style = ViewStyleResource {
        rules: vec![ViewStyleRule {
            selector: ViewStyleSelector {
                parts: vec![
                    ViewStyleSelectorPart::Element(ViewElementKind::Panel),
                    ViewStyleSelectorPart::Part("card.feedback".to_owned()),
                ],
            },
            declarations: vec![
                decl("background-color", rgba(36, 42, 54, 255)),
                decl("border-radius", ViewStyleValue::Text("16px".to_owned())),
                decl(
                    "box-shadow",
                    ViewStyleValue::Text("inset 0px 3px 14px 2px rgba(0,0,0,0.38)".to_owned()),
                ),
            ],
            source: None,
        }],
        ..ViewStyleResource::default()
    };

    let program = ViewProgramResource {
        instructions: vec![
            ViewProgramInstruction::OpenElement {
                element: ViewElementKind::Panel,
                target: None,
                style: None,
                part: Some("card.feedback".to_owned()),
                key: None,
                source: None,
            },
            ViewProgramInstruction::CloseElement,
        ],
        ..ViewProgramResource::default()
    };

    let resolved = program.runtime_element_styles_with_style(&style);
    let panel = resolved
        .controls
        .iter()
        .find(|element| element.part.as_deref() == Some("card.feedback"))
        .expect("panel part style");
    let visual = panel
        .style
        .visual_for_state(ViewRuntimeControlState::Normal);

    assert_eq!(visual.fill, Some(RgbaColor::rgb(36, 42, 54)));
    assert_eq!(visual.radius_milli, Some(16_000));
    assert_eq!(visual.shadows.len(), 1);
    assert_eq!(visual.shadows[0].radius_milli, 16_000);
    assert!(resolved.diagnostics.is_empty());
    assert_eq!(panel.element, ViewElementKind::Panel);
}

#[test]
fn surface_resource_receives_panel_part_runtime_style() {
    let style = ViewStyleResource {
        rules: vec![ViewStyleRule {
            selector: ViewStyleSelector {
                parts: vec![
                    ViewStyleSelectorPart::Element(ViewElementKind::Panel),
                    ViewStyleSelectorPart::Part("card.feedback".to_owned()),
                ],
            },
            declarations: vec![
                decl("background-color", rgba(36, 42, 54, 255)),
                decl("border-radius", ViewStyleValue::Text("16px".to_owned())),
                decl(
                    "box-shadow",
                    ViewStyleValue::Text("inset 0px 3px 14px 2px rgba(0,0,0,0.38)".to_owned()),
                ),
            ],
            source: None,
        }],
        ..ViewStyleResource::default()
    };
    let program = ViewProgramResource {
        instructions: vec![
            ViewProgramInstruction::OpenElement {
                element: ViewElementKind::Panel,
                target: None,
                style: None,
                part: Some("card.feedback".to_owned()),
                key: None,
                source: None,
            },
            ViewProgramInstruction::CloseElement,
        ],
        surfaces: vec![ViewSurfaceResource::new(
            "card.feedback",
            Some("view.Feedback".to_owned()),
            None,
            ViewElementKind::Panel,
            ViewRuntimeSurfaceBounds::from_px(24, 32, 112, 72),
        )],
        ..ViewProgramResource::default()
    };

    let resolved = program.runtime_surfaces_with_style(Some(&style));
    assert!(resolved.diagnostics.is_empty());
    let surface = resolved.controls.first().expect("surface");
    let visual = surface
        .style
        .visual_for_state(ViewRuntimeControlState::Normal);

    assert_eq!(surface.public_id, "card.feedback");
    assert_eq!(visual.fill, Some(RgbaColor::rgb(36, 42, 54)));
    assert_eq!(visual.radius_milli, Some(16_000));
    assert_eq!(visual.shadows.len(), 1);
    assert_eq!(visual.shadows[0].radius_milli, 16_000);
}

#[test]
fn border_radius_shorthand_resolves_four_corners_and_elliptical_axes() {
    let style = ViewStyleResource {
        rules: vec![rule(
            ViewStyleSelectorPart::Element(ViewElementKind::TextArea),
            vec![decl(
                "border-radius",
                ViewStyleValue::Text("12px 10px 8px 6px / 5px 4px 3px 2px".to_owned()),
            )],
        )],
        ..ViewStyleResource::default()
    };

    let resolved =
        resolve_text_control_style_for_test(&style, "input.message", ViewInputKind::TextArea);
    let normal = resolved
        .style
        .visual_for_state(ViewRuntimeControlState::Normal);

    assert_eq!(normal.radius_milli, None);
    assert_eq!(
        normal.radii_milli,
        Some(ViewRuntimeControlRadii::new(
            ViewRuntimeControlCornerRadius::new(12_000, 5_000),
            ViewRuntimeControlCornerRadius::new(10_000, 4_000),
            ViewRuntimeControlCornerRadius::new(8_000, 3_000),
            ViewRuntimeControlCornerRadius::new(6_000, 2_000),
        ))
    );
    assert!(resolved.diagnostics.is_empty());
}

#[test]
fn backdrop_filter_blur_resolves_to_typed_runtime_control_effect() {
    let style = ViewStyleResource {
        rules: vec![rule(
            ViewStyleSelectorPart::Element(ViewElementKind::TextField),
            vec![decl(
                "backdrop-filter",
                ViewStyleValue::Text("blur(12px)".to_owned()),
            )],
        )],
        ..ViewStyleResource::default()
    };

    let resolved =
        resolve_text_control_style_for_test(&style, "input.feedback", ViewInputKind::TextField);
    let normal = resolved
        .style
        .visual_for_state(ViewRuntimeControlState::Normal);

    assert_eq!(
        normal
            .backdrop_filters
            .as_ref()
            .expect("backdrop filter")
            .filters
            .as_slice(),
        &[ViewRuntimeControlFilter::Blur {
            radius_milli: 12_000,
        }]
    );
    assert!(resolved.diagnostics.is_empty());
}

#[test]
fn backdrop_filter_color_matrix_functions_resolve_to_typed_runtime_control_effects() {
    let style = ViewStyleResource {
        rules: vec![rule(
            ViewStyleSelectorPart::Element(ViewElementKind::TextField),
            vec![decl(
                "backdrop-filter",
                ViewStyleValue::Text(
                    "brightness(120%) contrast(0.9) saturate(140%) hue-rotate(12deg) opacity(85%)"
                        .to_owned(),
                ),
            )],
        )],
        ..ViewStyleResource::default()
    };

    let resolved =
        resolve_text_control_style_for_test(&style, "input.feedback", ViewInputKind::TextField);
    let normal = resolved
        .style
        .visual_for_state(ViewRuntimeControlState::Normal);

    assert_eq!(
        normal
            .backdrop_filters
            .as_ref()
            .expect("backdrop filter")
            .filters
            .as_slice(),
        &[
            ViewRuntimeControlFilter::Brightness {
                factor_milli: 1_200,
            },
            ViewRuntimeControlFilter::Contrast { factor_milli: 900 },
            ViewRuntimeControlFilter::Saturate {
                factor_milli: 1_400,
            },
            ViewRuntimeControlFilter::HueRotate {
                degrees_milli: 12_000,
            },
            ViewRuntimeControlFilter::Opacity { amount_milli: 850 },
        ]
    );
    assert!(resolved.diagnostics.is_empty());
}

#[test]
fn foreground_filter_blur_resolves_to_typed_runtime_control_effect() {
    let style = ViewStyleResource {
        rules: vec![rule(
            ViewStyleSelectorPart::Element(ViewElementKind::Button),
            vec![decl(
                "filter",
                ViewStyleValue::Text("blur(2.5px)".to_owned()),
            )],
        )],
        ..ViewStyleResource::default()
    };

    let resolved = resolve_button_style_for_test(&style, "button.submit_feedback");
    let normal = resolved
        .style
        .visual_for_state(ViewRuntimeControlState::Normal);

    assert_eq!(
        normal
            .filters
            .as_ref()
            .expect("foreground filter")
            .filters
            .as_slice(),
        &[ViewRuntimeControlFilter::Blur {
            radius_milli: 2_500,
        }]
    );
    assert!(resolved.diagnostics.is_empty());
}

#[test]
fn unsupported_filter_function_produces_structured_diagnostic() {
    let style = ViewStyleResource {
        rules: vec![rule(
            ViewStyleSelectorPart::Element(ViewElementKind::TextField),
            vec![decl(
                "backdrop-filter",
                ViewStyleValue::Text("drop-shadow(0px 4px 8px rgba(0,0,0,0.4))".to_owned()),
            )],
        )],
        ..ViewStyleResource::default()
    };

    let resolved =
        resolve_text_control_style_for_test(&style, "input.feedback", ViewInputKind::TextField);

    assert_eq!(resolved.diagnostics.diagnostics.len(), 1);
    assert_eq!(
        resolved.diagnostics.diagnostics[0].reason,
        ViewRuntimeControlStyleDiagnosticReason::UnsupportedValue
    );
    assert_eq!(
        resolved.diagnostics.diagnostics[0].property,
        "backdrop-filter"
    );
}

#[test]
fn unsupported_style_property_produces_structured_diagnostic() {
    let style = ViewStyleResource {
        rules: vec![rule(
            ViewStyleSelectorPart::Element(ViewElementKind::TextField),
            vec![decl(
                "transform",
                ViewStyleValue::Text("translateX(8px)".to_owned()),
            )],
        )],
        ..ViewStyleResource::default()
    };

    let resolved =
        resolve_text_control_style_for_test(&style, "input.feedback", ViewInputKind::TextField);

    assert_eq!(resolved.diagnostics.diagnostics.len(), 1);
    assert_eq!(
        resolved.diagnostics.diagnostics[0].reason,
        ViewRuntimeControlStyleDiagnosticReason::UnsupportedProperty
    );
    assert_eq!(resolved.diagnostics.diagnostics[0].property, "transform");
}

fn rule(part: ViewStyleSelectorPart, declarations: Vec<ViewStyleDeclaration>) -> ViewStyleRule {
    ViewStyleRule {
        selector: ViewStyleSelector { parts: vec![part] },
        declarations,
        source: None,
    }
}

fn state_rule(state: ViewInteractionState, declaration: ViewStyleDeclaration) -> ViewStyleRule {
    ViewStyleRule {
        selector: ViewStyleSelector {
            parts: vec![
                ViewStyleSelectorPart::Element(ViewElementKind::Button),
                ViewStyleSelectorPart::Interaction(state),
            ],
        },
        declarations: vec![declaration],
        source: None,
    }
}

fn decl(property: &str, value: ViewStyleValue) -> ViewStyleDeclaration {
    ViewStyleDeclaration {
        property: property.to_owned(),
        value,
        op: StyleAssignOp::Replace,
    }
}

fn rgba(red: u8, green: u8, blue: u8, alpha: u8) -> ViewStyleValue {
    ViewStyleValue::Rgba(RgbaColor::rgba(red, green, blue, alpha))
}

fn runtime_text_control(target: &str, kind: ViewInputKind) -> ViewRuntimeTextControl {
    ViewRuntimeTextControl {
        public_id: target.to_owned(),
        target: target.to_owned(),
        view: None,
        containing_scroll_region: None,
        session: 1,
        value: String::new(),
        selection: ViewRuntimeTextSelection::default(),
        options: ViewRuntimeTextControlOptions {
            purpose: ViewInputPurpose::Text,
            autocorrect: TextAssistPolicy::PlatformDefault,
            spellcheck: TextAssistPolicy::PlatformDefault,
            capitalization: TextCapitalization::None,
            enter_key: EnterKeyHint::Default,
            multiline: kind.is_multiline(),
            selection_policy: ViewTextSelectionPolicy::Enabled,
            shortcut_policy: ViewTextShortcutPolicy::Enabled,
            tab_policy: ViewTextTabPolicy::FocusNavigation,
            vertical_navigation_policy: ViewTextVerticalNavigationPolicy::LogicalLine,
            secure_policy: ViewSecureInputPolicy::Plain,
            composition_on_blur: CompositionOnBlurPolicy::Commit,
        },
        kind,
        bounds: ViewRuntimeTextControlBounds::new(0, 0, 100_000, 40_000),
        label: None,
        handlers: ViewRuntimeTextControlHandlers::default(),
        style: ViewRuntimeControlStyle::default(),
    }
}

fn resolve_text_control_style_for_test(
    style: &ViewStyleResource,
    target: &str,
    kind: ViewInputKind,
) -> ViewRuntimeControlStyleResolution {
    let program = panel_wrapped_program(kind.runtime_control_element());
    let mut controls = vec![runtime_text_control(target, kind)];
    let mut buttons = Vec::new();
    let mut text_blocks = Vec::new();
    let diagnostics =
        program.apply_runtime_styles(style, &mut controls, &mut buttons, &mut text_blocks);
    ViewRuntimeControlStyleResolution {
        style: controls.remove(0).style,
        diagnostics,
    }
}

fn resolve_button_style_for_test(
    style: &ViewStyleResource,
    target: &str,
) -> ViewRuntimeControlStyleResolution {
    let program = panel_wrapped_program(ViewElementKind::Button);
    let mut controls = Vec::new();
    let mut buttons = vec![ViewRuntimeActionButton {
        public_id: target.to_owned(),
        target: target.to_owned(),
        view: None,
        containing_scroll_region: None,
        label: "Submit".to_owned(),
        enabled: true,
        bounds: ViewRuntimeButtonBounds::new(0, 0, 100_000, 40_000),
        action: ViewRuntimeActionButtonAction::Noop,
        style: ViewRuntimeControlStyle::default(),
    }];
    let mut text_blocks = Vec::new();
    let diagnostics =
        program.apply_runtime_styles(style, &mut controls, &mut buttons, &mut text_blocks);
    ViewRuntimeControlStyleResolution {
        style: buttons.remove(0).style,
        diagnostics,
    }
}

fn panel_wrapped_program(element: ViewElementKind) -> ViewProgramResource {
    ViewProgramResource {
        instructions: vec![
            ViewProgramInstruction::OpenElement {
                element: ViewElementKind::Panel,
                target: None,
                style: None,
                part: None,
                key: None,
                source: None,
            },
            ViewProgramInstruction::OpenElement {
                element,
                target: None,
                style: None,
                part: None,
                key: None,
                source: None,
            },
            ViewProgramInstruction::CloseElement,
            ViewProgramInstruction::CloseElement,
        ],
        ..ViewProgramResource::default()
    }
}
