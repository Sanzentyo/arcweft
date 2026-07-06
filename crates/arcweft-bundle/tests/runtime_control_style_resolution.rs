use arcweft_bundle::resource_codec::ui::{
    RgbaColor, StyleAssignOp, UiElementKind, UiElementState, UiInputKind, UiInteractionState,
    UiRuntimeControlCornerFrameStyle, UiRuntimeControlCornerRadius, UiRuntimeControlFilter,
    UiRuntimeControlRadii, UiRuntimeControlState, UiRuntimeControlStyleDiagnosticReason,
    UiStyleDeclaration, UiStyleResource, UiStyleRule, UiStyleSelector, UiStyleSelectorPart,
    UiStyleValue,
};

#[test]
fn text_control_resolves_authored_background_alpha_and_border_color() {
    let style = UiStyleResource {
        rules: vec![rule(
            UiStyleSelectorPart::Element(UiElementKind::TextField),
            vec![
                decl("background-color", rgba(16, 24, 32, 180)),
                decl("border-color", rgba(80, 112, 96, 255)),
                decl("border-width", UiStyleValue::Milli(2_000)),
                decl("opacity", UiStyleValue::Milli(720)),
                decl("z-index", UiStyleValue::Milli(2_500)),
            ],
        )],
        ..UiStyleResource::default()
    };

    let resolved = style.runtime_text_control_style("input.feedback", UiInputKind::TextField);
    let normal = resolved
        .style
        .visual_for_state(UiRuntimeControlState::Normal);

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
    let style = UiStyleResource {
        rules: vec![
            rule(
                UiStyleSelectorPart::Element(UiElementKind::Button),
                vec![decl("depth", UiStyleValue::Text("1000".to_owned()))],
            ),
            state_rule(
                UiInteractionState::Hover,
                decl("z-index", UiStyleValue::Text("3000".to_owned())),
            ),
        ],
        ..UiStyleResource::default()
    };

    let resolved = style
        .resolve_runtime_control_style_for_test("button.submit_feedback", UiElementKind::Button);

    assert_eq!(
        resolved
            .style
            .visual_for_state(UiRuntimeControlState::Normal)
            .depth_milli,
        Some(1_000)
    );
    assert_eq!(
        resolved
            .style
            .visual_for_state(UiRuntimeControlState::Hover)
            .depth_milli,
        Some(3_000)
    );
    assert!(resolved.diagnostics.is_empty());
}

#[test]
fn text_control_resolves_selection_and_caret_colors() {
    let style = UiStyleResource {
        rules: vec![rule(
            UiStyleSelectorPart::Element(UiElementKind::TextArea),
            vec![
                decl("selection-color", rgba(64, 128, 200, 160)),
                decl("caret-color", rgba(240, 220, 90, 255)),
            ],
        )],
        ..UiStyleResource::default()
    };

    let resolved = style.runtime_text_control_style("input.message", UiInputKind::TextArea);
    let normal = resolved
        .style
        .visual_for_state(UiRuntimeControlState::Normal);

    assert_eq!(normal.selection, Some(RgbaColor::rgba(64, 128, 200, 160)));
    assert_eq!(normal.caret, Some(RgbaColor::rgb(240, 220, 90)));
    assert!(resolved.diagnostics.is_empty());
}

#[test]
fn text_control_resolves_corner_frame_decoration() {
    let style = UiStyleResource {
        rules: vec![rule(
            UiStyleSelectorPart::Element(UiElementKind::TextArea),
            vec![
                decl("corner-frame-color", rgba(94, 234, 212, 220)),
                decl("corner-frame-width", UiStyleValue::Milli(3_000)),
                decl("corner-frame-length", UiStyleValue::Milli(24_000)),
                decl("corner-frame-offset", UiStyleValue::Milli(2_000)),
            ],
        )],
        ..UiStyleResource::default()
    };

    let resolved = style.runtime_text_control_style("input.message", UiInputKind::TextArea);
    let normal = resolved
        .style
        .visual_for_state(UiRuntimeControlState::Normal);

    assert_eq!(
        normal.corner_frame,
        Some(UiRuntimeControlCornerFrameStyle {
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
    let style = UiStyleResource {
        rules: vec![
            rule(
                UiStyleSelectorPart::Element(UiElementKind::Button),
                vec![decl("background-color", rgba(20, 30, 40, 255))],
            ),
            state_rule(
                UiInteractionState::Hover,
                decl("background-color", rgba(40, 60, 80, 255)),
            ),
            state_rule(
                UiInteractionState::Active,
                decl("background-color", rgba(70, 90, 110, 255)),
            ),
            state_rule(
                UiInteractionState::Disabled,
                decl("background-color", rgba(12, 12, 12, 160)),
            ),
        ],
        ..UiStyleResource::default()
    };

    let resolved = style
        .resolve_runtime_control_style_for_test("button.submit_feedback", UiElementKind::Button);

    assert_eq!(
        resolved
            .style
            .visual_for_state(UiRuntimeControlState::Hover)
            .fill,
        Some(RgbaColor::rgb(40, 60, 80))
    );
    assert_eq!(
        resolved
            .style
            .visual_for_state(UiRuntimeControlState::Pressed)
            .fill,
        Some(RgbaColor::rgb(70, 90, 110))
    );
    assert_eq!(
        resolved
            .style
            .visual_for_state(UiRuntimeControlState::Disabled)
            .fill,
        Some(RgbaColor::rgba(12, 12, 12, 160))
    );
}

#[test]
fn focus_visible_ring_and_supported_box_shadow_are_typed() {
    let style = UiStyleResource {
        rules: vec![
            rule(
                UiStyleSelectorPart::Element(UiElementKind::TextArea),
                vec![
                    decl("border-radius", UiStyleValue::Milli(12_000)),
                    decl(
                        "box-shadow",
                        UiStyleValue::Text("0px 8px 20px 0px rgba(0,0,0,0.35)".to_owned()),
                    ),
                ],
            ),
            UiStyleRule {
                selector: UiStyleSelector {
                    parts: vec![
                        UiStyleSelectorPart::Element(UiElementKind::TextArea),
                        UiStyleSelectorPart::State(UiElementState::FocusVisible),
                    ],
                },
                declarations: vec![
                    decl("focus-ring-color", rgba(226, 233, 98, 255)),
                    decl("focus-ring-width", UiStyleValue::Milli(3_000)),
                ],
                source: None,
            },
        ],
        ..UiStyleResource::default()
    };

    let resolved = style.runtime_text_control_style("input.message", UiInputKind::TextArea);
    let normal = resolved
        .style
        .visual_for_state(UiRuntimeControlState::Normal);
    let focused = resolved
        .style
        .visual_for_state(UiRuntimeControlState::FocusVisible);

    assert_eq!(normal.radius_milli, Some(12_000));
    assert_eq!(normal.shadows.len(), 1);
    assert_eq!(normal.shadows[0].blur_milli, 20_000);
    assert_eq!(focused.focus_ring.expect("focus ring").width_milli, 3_000);
}

#[test]
fn surface_style_resolves_radius_fill_and_box_shadow() {
    let style = UiStyleResource {
        rules: vec![UiStyleRule {
            selector: UiStyleSelector {
                parts: vec![
                    UiStyleSelectorPart::Element(UiElementKind::Surface),
                    UiStyleSelectorPart::Part("card.feedback".to_owned()),
                ],
            },
            declarations: vec![
                decl("background-color", rgba(36, 42, 54, 255)),
                decl("border-radius", UiStyleValue::Text("16px".to_owned())),
                decl(
                    "box-shadow",
                    UiStyleValue::Text("inset 0px 3px 14px 2px rgba(0,0,0,0.38)".to_owned()),
                ),
            ],
            source: None,
        }],
        ..UiStyleResource::default()
    };

    let resolved = style.runtime_surface_style("card.feedback");
    let visual = resolved
        .style
        .visual_for_state(UiRuntimeControlState::Normal);

    assert_eq!(visual.fill, Some(RgbaColor::rgb(36, 42, 54)));
    assert_eq!(visual.radius_milli, Some(16_000));
    assert_eq!(visual.shadows.len(), 1);
    assert_eq!(visual.shadows[0].radius_milli, 16_000);
    assert!(resolved.diagnostics.is_empty());
}

#[test]
fn border_radius_shorthand_resolves_four_corners_and_elliptical_axes() {
    let style = UiStyleResource {
        rules: vec![rule(
            UiStyleSelectorPart::Element(UiElementKind::TextArea),
            vec![decl(
                "border-radius",
                UiStyleValue::Text("12px 10px 8px 6px / 5px 4px 3px 2px".to_owned()),
            )],
        )],
        ..UiStyleResource::default()
    };

    let resolved = style.runtime_text_control_style("input.message", UiInputKind::TextArea);
    let normal = resolved
        .style
        .visual_for_state(UiRuntimeControlState::Normal);

    assert_eq!(normal.radius_milli, None);
    assert_eq!(
        normal.radii_milli,
        Some(UiRuntimeControlRadii::new(
            UiRuntimeControlCornerRadius::new(12_000, 5_000),
            UiRuntimeControlCornerRadius::new(10_000, 4_000),
            UiRuntimeControlCornerRadius::new(8_000, 3_000),
            UiRuntimeControlCornerRadius::new(6_000, 2_000),
        ))
    );
    assert!(resolved.diagnostics.is_empty());
}

#[test]
fn backdrop_filter_blur_resolves_to_typed_runtime_control_effect() {
    let style = UiStyleResource {
        rules: vec![rule(
            UiStyleSelectorPart::Element(UiElementKind::TextField),
            vec![decl(
                "backdrop-filter",
                UiStyleValue::Text("blur(12px)".to_owned()),
            )],
        )],
        ..UiStyleResource::default()
    };

    let resolved = style.runtime_text_control_style("input.feedback", UiInputKind::TextField);
    let normal = resolved
        .style
        .visual_for_state(UiRuntimeControlState::Normal);

    assert_eq!(
        normal
            .backdrop_filters
            .as_ref()
            .expect("backdrop filter")
            .filters
            .as_slice(),
        &[UiRuntimeControlFilter::Blur {
            radius_milli: 12_000,
        }]
    );
    assert!(resolved.diagnostics.is_empty());
}

#[test]
fn backdrop_filter_color_matrix_functions_resolve_to_typed_runtime_control_effects() {
    let style = UiStyleResource {
        rules: vec![rule(
            UiStyleSelectorPart::Element(UiElementKind::TextField),
            vec![decl(
                "backdrop-filter",
                UiStyleValue::Text(
                    "brightness(120%) contrast(0.9) saturate(140%) hue-rotate(12deg) opacity(85%)"
                        .to_owned(),
                ),
            )],
        )],
        ..UiStyleResource::default()
    };

    let resolved = style.runtime_text_control_style("input.feedback", UiInputKind::TextField);
    let normal = resolved
        .style
        .visual_for_state(UiRuntimeControlState::Normal);

    assert_eq!(
        normal
            .backdrop_filters
            .as_ref()
            .expect("backdrop filter")
            .filters
            .as_slice(),
        &[
            UiRuntimeControlFilter::Brightness {
                factor_milli: 1_200,
            },
            UiRuntimeControlFilter::Contrast { factor_milli: 900 },
            UiRuntimeControlFilter::Saturate {
                factor_milli: 1_400,
            },
            UiRuntimeControlFilter::HueRotate {
                degrees_milli: 12_000,
            },
            UiRuntimeControlFilter::Opacity { amount_milli: 850 },
        ]
    );
    assert!(resolved.diagnostics.is_empty());
}

#[test]
fn foreground_filter_blur_resolves_to_typed_runtime_control_effect() {
    let style = UiStyleResource {
        rules: vec![rule(
            UiStyleSelectorPart::Element(UiElementKind::Button),
            vec![decl("filter", UiStyleValue::Text("blur(2.5px)".to_owned()))],
        )],
        ..UiStyleResource::default()
    };

    let resolved = style
        .resolve_runtime_control_style_for_test("button.submit_feedback", UiElementKind::Button);
    let normal = resolved
        .style
        .visual_for_state(UiRuntimeControlState::Normal);

    assert_eq!(
        normal
            .filters
            .as_ref()
            .expect("foreground filter")
            .filters
            .as_slice(),
        &[UiRuntimeControlFilter::Blur {
            radius_milli: 2_500,
        }]
    );
    assert!(resolved.diagnostics.is_empty());
}

#[test]
fn unsupported_filter_function_produces_structured_diagnostic() {
    let style = UiStyleResource {
        rules: vec![rule(
            UiStyleSelectorPart::Element(UiElementKind::TextField),
            vec![decl(
                "backdrop-filter",
                UiStyleValue::Text("drop-shadow(0px 4px 8px rgba(0,0,0,0.4))".to_owned()),
            )],
        )],
        ..UiStyleResource::default()
    };

    let resolved = style.runtime_text_control_style("input.feedback", UiInputKind::TextField);

    assert_eq!(resolved.diagnostics.diagnostics.len(), 1);
    assert_eq!(
        resolved.diagnostics.diagnostics[0].reason,
        UiRuntimeControlStyleDiagnosticReason::UnsupportedValue
    );
    assert_eq!(
        resolved.diagnostics.diagnostics[0].property,
        "backdrop-filter"
    );
}

#[test]
fn unsupported_style_property_produces_structured_diagnostic() {
    let style = UiStyleResource {
        rules: vec![rule(
            UiStyleSelectorPart::Element(UiElementKind::TextField),
            vec![decl(
                "transform",
                UiStyleValue::Text("translateX(8px)".to_owned()),
            )],
        )],
        ..UiStyleResource::default()
    };

    let resolved = style.runtime_text_control_style("input.feedback", UiInputKind::TextField);

    assert_eq!(resolved.diagnostics.diagnostics.len(), 1);
    assert_eq!(
        resolved.diagnostics.diagnostics[0].reason,
        UiRuntimeControlStyleDiagnosticReason::UnsupportedProperty
    );
    assert_eq!(resolved.diagnostics.diagnostics[0].property, "transform");
}

fn rule(part: UiStyleSelectorPart, declarations: Vec<UiStyleDeclaration>) -> UiStyleRule {
    UiStyleRule {
        selector: UiStyleSelector { parts: vec![part] },
        declarations,
        source: None,
    }
}

fn state_rule(state: UiInteractionState, declaration: UiStyleDeclaration) -> UiStyleRule {
    UiStyleRule {
        selector: UiStyleSelector {
            parts: vec![
                UiStyleSelectorPart::Element(UiElementKind::Button),
                UiStyleSelectorPart::Interaction(state),
            ],
        },
        declarations: vec![declaration],
        source: None,
    }
}

fn decl(property: &str, value: UiStyleValue) -> UiStyleDeclaration {
    UiStyleDeclaration {
        property: property.to_owned(),
        value,
        op: StyleAssignOp::Replace,
    }
}

fn rgba(red: u8, green: u8, blue: u8, alpha: u8) -> UiStyleValue {
    UiStyleValue::Rgba(RgbaColor::rgba(red, green, blue, alpha))
}

trait RuntimeControlStyleTestExt {
    fn resolve_runtime_control_style_for_test(
        &self,
        target: &str,
        element: UiElementKind,
    ) -> arcweft_bundle::resource_codec::ui::UiRuntimeControlStyleResolution;
}

impl RuntimeControlStyleTestExt for UiStyleResource {
    fn resolve_runtime_control_style_for_test(
        &self,
        target: &str,
        element: UiElementKind,
    ) -> arcweft_bundle::resource_codec::ui::UiRuntimeControlStyleResolution {
        match element {
            UiElementKind::Button => {
                use arcweft_bundle::resource_codec::ui::{
                    UiActionButtonActionResource, UiActionButtonResource, UiRuntimeButtonBounds,
                    UiTextSubmitImePolicy,
                };
                self.runtime_action_button_style(&UiActionButtonResource {
                    public_id: target.to_owned(),
                    component: None,
                    label_text_source: "text.submit".to_owned(),
                    enabled: true,
                    action: UiActionButtonActionResource::TextInputSubmit {
                        input: "input.feedback".to_owned(),
                        ime_policy: UiTextSubmitImePolicy::Commit,
                    },
                    bounds: UiRuntimeButtonBounds::new(0, 0, 100_000, 40_000),
                    style: None,
                    source: None,
                })
            }
            UiElementKind::TextField => {
                self.runtime_text_control_style(target, UiInputKind::TextField)
            }
            UiElementKind::TextArea => {
                self.runtime_text_control_style(target, UiInputKind::TextArea)
            }
            UiElementKind::SecureField => {
                self.runtime_text_control_style(target, UiInputKind::SecureField)
            }
            UiElementKind::Surface
            | UiElementKind::Box
            | UiElementKind::Scroll
            | UiElementKind::Row
            | UiElementKind::Column
            | UiElementKind::Stack => {
                panic!("unsupported test element")
            }
        }
    }
}
