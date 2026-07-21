use arcweft_presentation::appearance::PresentationColor;
use arcweft_view::{
    ViewAlignment, ViewBorderRadii, ViewColorValue, ViewContainerAxis, ViewContainerComparison,
    ViewContainerPredicate, ViewElementKind, ViewFilter, ViewFontFamily, ViewFontFamilyList,
    ViewLengthMilli, ViewPropertyKind, ViewRatioMilli, ViewShadow, ViewSpecifiedValue,
    ViewStyleApplicationTarget, ViewStyleAssignOp, ViewStyleCombinator, ViewStyleDeclaration,
    ViewStyleModelError, ViewStylePatch, ViewStylePatchId, ViewStylePredicate, ViewStyleProgram,
    ViewStyleRule, ViewStyleSelector, ViewStyleSelectorSequence, ViewStyleSheet, ViewStyleSheetId,
    ViewStyleSheetIdError, ViewStyleSourceId, ViewStyleToken, ViewStyleTokenId,
    ViewStyleTransition, ViewStyleValueKind,
};
use serde::de::DeserializeOwned;

fn token_id(value: &str) -> ViewStyleTokenId {
    ViewStyleTokenId::try_new(value).expect("valid token ID")
}

#[test]
fn sheet_identity_owns_authored_and_engine_family_invariants() {
    assert_eq!(
        ViewStyleSheetId::try_new("style.dialogue")
            .unwrap()
            .public_id()
            .as_str(),
        "style.dialogue"
    );
    assert_eq!(
        ViewStyleSheetId::try_new_engine_owned("std.style.dialogue")
            .unwrap()
            .public_id()
            .as_str(),
        "std.style.dialogue"
    );
    assert!(matches!(
        ViewStyleSheetId::parse_public("view.dialogue"),
        Err(ViewStyleSheetIdError::WrongFamily { .. })
    ));
    assert!(matches!(
        ViewStyleSheetId::parse_public("@style.dialogue"),
        Err(ViewStyleSheetIdError::Invalid(_))
    ));
    assert!(ViewStyleSheetId::try_new("std.style.dialogue").is_err());
    assert!(ViewStyleSheetId::try_new_engine_owned("style.dialogue").is_err());
    assert!(ViewStyleSheetId::try_new("style.").is_err());
    assert_eq!(
        serde_json::from_str::<ViewStyleSheetId>(r#""std.style.dialogue""#)
            .unwrap()
            .public_id()
            .as_str(),
        "std.style.dialogue"
    );
    assert!(serde_json::from_str::<ViewStyleSheetId>(r#""view.dialogue""#).is_err());
}

fn button_selector() -> ViewStyleSelector {
    ViewStyleSelector::new(vec![
        ViewStyleSelectorSequence::new(None, Some(ViewElementKind::Button), None, Vec::new())
            .expect("non-empty selector sequence"),
    ])
    .expect("valid selector")
}

fn color(value: PresentationColor) -> ViewSpecifiedValue {
    ViewSpecifiedValue::Color {
        value: ViewColorValue::Literal { color: value },
    }
}

fn assert_unknown_json_field_is_rejected<T: DeserializeOwned>(
    label: &str,
    mut encoded: serde_json::Value,
    object_pointer: &str,
) {
    encoded
        .pointer_mut(object_pointer)
        .unwrap_or_else(|| panic!("missing JSON object for {label}: {object_pointer}"))
        .as_object_mut()
        .unwrap_or_else(|| panic!("JSON target for {label} is not an object"))
        .insert("unexpected_style_field".to_owned(), serde_json::json!(true));
    assert!(
        serde_json::from_value::<T>(encoded).is_err(),
        "{label} must reject an unknown JSON field"
    );
}

#[test]
fn checked_sheet_owns_id_tokens_rules_and_round_trips() {
    let accent_id = token_id("token.accent");
    let token = ViewStyleToken::new(
        accent_id.clone(),
        ViewStyleValueKind::Color,
        color(PresentationColor::rgb(12, 34, 56)),
        ViewStyleSourceId::new(1),
    )
    .expect("checked token");
    let declaration = ViewStyleDeclaration::new(
        ViewPropertyKind::BackgroundColor,
        ViewSpecifiedValue::Token {
            token: accent_id,
            value_kind: ViewStyleValueKind::Color,
        },
        ViewStyleAssignOp::Replace,
        ViewStyleSourceId::new(3),
    )
    .expect("checked declaration");
    let rule = ViewStyleRule::new(
        button_selector(),
        None,
        vec![declaration],
        7,
        ViewStyleSourceId::new(2),
    )
    .expect("checked rule");
    let sheet = ViewStyleSheet::new(
        ViewStyleSheetId::try_new("style.controls").expect("sheet ID"),
        vec![token],
        vec![rule],
    )
    .expect("checked sheet");

    let encoded = serde_json::to_string(&sheet).expect("sheet encodes");
    assert_eq!(encoded.matches("style.controls").count(), 1);
    assert_eq!(
        serde_json::from_str::<ViewStyleSheet>(&encoded).expect("sheet decodes"),
        sheet
    );
    assert_eq!(sheet.rules()[0].source_order(), 7);
    assert_eq!(sheet.tokens()[0].source().value(), 1);
}

#[test]
fn sheet_rejects_duplicate_missing_mismatched_and_cyclic_tokens() {
    let literal = |id: ViewStyleTokenId| {
        ViewStyleToken::new(
            id,
            ViewStyleValueKind::Color,
            color(PresentationColor::rgb(1, 2, 3)),
            ViewStyleSourceId::new(0),
        )
        .expect("checked token")
    };
    let duplicate_id = token_id("token.duplicate");
    let duplicate = ViewStyleSheet::new(
        ViewStyleSheetId::try_new("style.duplicate").expect("sheet ID"),
        vec![literal(duplicate_id.clone()), literal(duplicate_id.clone())],
        Vec::new(),
    )
    .expect_err("duplicate token must fail");
    assert_eq!(duplicate, ViewStyleModelError::DuplicateToken(duplicate_id));

    let missing_id = token_id("token.missing");
    let owner_id = token_id("token.owner");
    let missing = ViewStyleToken::new(
        owner_id.clone(),
        ViewStyleValueKind::Color,
        ViewSpecifiedValue::Token {
            token: missing_id.clone(),
            value_kind: ViewStyleValueKind::Color,
        },
        ViewStyleSourceId::new(0),
    )
    .expect("locally checked token");
    assert_eq!(
        ViewStyleSheet::new(
            ViewStyleSheetId::try_new("style.missing").expect("sheet ID"),
            vec![missing],
            Vec::new(),
        )
        .expect_err("missing reference must fail"),
        ViewStyleModelError::UnknownTokenReference {
            owner: owner_id,
            referenced: missing_id,
        }
    );

    let color_id = token_id("token.color");
    let length_id = token_id("token.length");
    let referencing_color = ViewStyleToken::new(
        color_id.clone(),
        ViewStyleValueKind::Color,
        ViewSpecifiedValue::Token {
            token: length_id.clone(),
            value_kind: ViewStyleValueKind::Color,
        },
        ViewStyleSourceId::new(0),
    )
    .expect("locally checked token");
    let length = ViewStyleToken::new(
        length_id.clone(),
        ViewStyleValueKind::Length,
        ViewSpecifiedValue::Length {
            value: arcweft_view::ViewLengthMilli::new(1_000),
        },
        ViewStyleSourceId::new(0),
    )
    .expect("length token");
    assert!(matches!(
        ViewStyleSheet::new(
            ViewStyleSheetId::try_new("style.kind").expect("sheet ID"),
            vec![referencing_color, length],
            Vec::new(),
        ),
        Err(ViewStyleModelError::TokenReferenceKindMismatch {
            owner,
            referenced,
            expected: ViewStyleValueKind::Color,
            actual: ViewStyleValueKind::Length,
        }) if owner == color_id && referenced == length_id
    ));

    let a_id = token_id("token.a");
    let b_id = token_id("token.b");
    let alias = |id: ViewStyleTokenId, target: ViewStyleTokenId| {
        ViewStyleToken::new(
            id,
            ViewStyleValueKind::Color,
            ViewSpecifiedValue::Token {
                token: target,
                value_kind: ViewStyleValueKind::Color,
            },
            ViewStyleSourceId::new(0),
        )
        .expect("locally checked alias")
    };
    assert!(matches!(
        ViewStyleSheet::new(
            ViewStyleSheetId::try_new("style.cycle").expect("sheet ID"),
            vec![alias(a_id.clone(), b_id.clone()), alias(b_id, a_id)],
            Vec::new(),
        ),
        Err(ViewStyleModelError::TokenCycle(_))
    ));
}

#[test]
fn rule_token_references_must_resolve_in_the_owning_sheet_with_the_same_kind() {
    let remote_id = token_id("token.remote");
    let remote_token = ViewStyleToken::new(
        remote_id.clone(),
        ViewStyleValueKind::Color,
        color(PresentationColor::rgb(1, 2, 3)),
        ViewStyleSourceId::new(0),
    )
    .expect("checked token");
    let _remote_sheet = ViewStyleSheet::new(
        ViewStyleSheetId::try_new("style.remote").expect("sheet ID"),
        vec![remote_token],
        Vec::new(),
    )
    .expect("remote owner is valid");
    let remote_declaration = ViewStyleDeclaration::new(
        ViewPropertyKind::BackgroundColor,
        ViewSpecifiedValue::Token {
            token: remote_id.clone(),
            value_kind: ViewStyleValueKind::Color,
        },
        ViewStyleAssignOp::Replace,
        ViewStyleSourceId::new(0),
    )
    .expect("locally checked declaration");
    let remote_rule = ViewStyleRule::new(
        button_selector(),
        None,
        vec![remote_declaration],
        3,
        ViewStyleSourceId::new(0),
    )
    .expect("locally checked rule");
    assert_eq!(
        ViewStyleSheet::new(
            ViewStyleSheetId::try_new("style.local").expect("sheet ID"),
            Vec::new(),
            vec![remote_rule],
        )
        .expect_err("a rule may not borrow another sheet's token"),
        ViewStyleModelError::UnknownRuleTokenReference {
            source_order: 3,
            property: ViewPropertyKind::BackgroundColor,
            referenced: remote_id,
        }
    );

    let length_id = token_id("token.length");
    let length_token = ViewStyleToken::new(
        length_id.clone(),
        ViewStyleValueKind::Length,
        ViewSpecifiedValue::Length {
            value: arcweft_view::ViewLengthMilli::new(1_000),
        },
        ViewStyleSourceId::new(0),
    )
    .expect("checked length token");
    let mismatched_declaration = ViewStyleDeclaration::new(
        ViewPropertyKind::BackgroundColor,
        ViewSpecifiedValue::Token {
            token: length_id.clone(),
            value_kind: ViewStyleValueKind::Color,
        },
        ViewStyleAssignOp::Replace,
        ViewStyleSourceId::new(0),
    )
    .expect("the declaration's annotated kind matches its property");
    let mismatched_rule = ViewStyleRule::new(
        button_selector(),
        None,
        vec![mismatched_declaration],
        4,
        ViewStyleSourceId::new(0),
    )
    .expect("locally checked rule");
    assert_eq!(
        ViewStyleSheet::new(
            ViewStyleSheetId::try_new("style.kind-mismatch").expect("sheet ID"),
            vec![length_token],
            vec![mismatched_rule],
        )
        .expect_err("the referenced token's actual kind must match"),
        ViewStyleModelError::RuleTokenReferenceKindMismatch {
            source_order: 4,
            property: ViewPropertyKind::BackgroundColor,
            referenced: length_id,
            expected: ViewStyleValueKind::Color,
            actual: ViewStyleValueKind::Length,
        }
    );
}

#[test]
fn sheet_rejects_token_reference_depth_above_the_canonical_limit() {
    let tokens = (0..=ViewStyleSheet::MAX_TOKEN_REFERENCE_DEPTH)
        .map(|index| {
            let id = token_id(&format!("token.{index:03}"));
            let value = if index == ViewStyleSheet::MAX_TOKEN_REFERENCE_DEPTH {
                color(PresentationColor::rgb(1, 2, 3))
            } else {
                ViewSpecifiedValue::Token {
                    token: token_id(&format!("token.{:03}", index + 1)),
                    value_kind: ViewStyleValueKind::Color,
                }
            };
            ViewStyleToken::new(
                id,
                ViewStyleValueKind::Color,
                value,
                ViewStyleSourceId::new(0),
            )
            .expect("locally checked token")
        })
        .collect();

    assert_eq!(
        ViewStyleSheet::new(
            ViewStyleSheetId::try_new("style.too-deep").expect("sheet ID"),
            tokens,
            Vec::new(),
        )
        .expect_err("an over-deep token chain must fail during sheet validation"),
        ViewStyleModelError::TokenReferenceDepthExceeded {
            token: token_id("token.000"),
            depth: ViewStyleSheet::MAX_TOKEN_REFERENCE_DEPTH + 1,
            max_depth: ViewStyleSheet::MAX_TOKEN_REFERENCE_DEPTH,
        }
    );
}

#[test]
fn declaration_and_rule_validation_rejects_malformed_combinations() {
    assert!(matches!(
        ViewStyleDeclaration::new(
            ViewPropertyKind::Opacity,
            color(PresentationColor::rgb(1, 2, 3)),
            ViewStyleAssignOp::Replace,
            ViewStyleSourceId::new(0),
        ),
        Err(ViewStyleModelError::DeclarationValueKindMismatch { .. })
    ));
    assert_eq!(
        ViewStyleDeclaration::new(
            ViewPropertyKind::Opacity,
            ViewSpecifiedValue::Ratio {
                value: ViewRatioMilli::ONE,
            },
            ViewStyleAssignOp::Append,
            ViewStyleSourceId::new(0),
        )
        .expect_err("opacity is not appendable"),
        ViewStyleModelError::InvalidAppend {
            property: ViewPropertyKind::Opacity,
        }
    );
    assert!(matches!(
        ViewStyleDeclaration::new(
            ViewPropertyKind::TextAlign,
            ViewSpecifiedValue::Alignment {
                value: ViewAlignment::SpaceBetween,
            },
            ViewStyleAssignOp::Replace,
            ViewStyleSourceId::new(0),
        ),
        Err(ViewStyleModelError::InvalidAlignment { .. })
    ));
    assert_eq!(
        ViewStyleRule::new(
            button_selector(),
            None,
            Vec::new(),
            9,
            ViewStyleSourceId::new(0),
        )
        .expect_err("empty rule must fail"),
        ViewStyleModelError::EmptyRule { source_order: 9 }
    );

    let not_for_button = ViewStyleDeclaration::new(
        ViewPropertyKind::FlexDirection,
        ViewSpecifiedValue::FlexDirection {
            value: arcweft_view::ViewFlexDirection::Row,
        },
        ViewStyleAssignOp::Replace,
        ViewStyleSourceId::new(0),
    )
    .expect("property/value pair is locally valid");
    let rule = ViewStyleRule::new(
        button_selector(),
        None,
        vec![not_for_button],
        0,
        ViewStyleSourceId::new(0),
    )
    .expect("locally checked rule");
    assert!(matches!(
        ViewStyleSheet::new(
            ViewStyleSheetId::try_new("style.applicability").expect("sheet ID"),
            Vec::new(),
            vec![rule],
        ),
        Err(ViewStyleModelError::PropertyNotApplicable {
            property: ViewPropertyKind::FlexDirection,
            element: ViewElementKind::Button,
            source_order: 0,
        })
    ));
}

#[test]
fn serde_decode_rechecks_nested_values_selectors_and_patch_declarations() {
    assert!(serde_json::from_str::<ViewRatioMilli>("1001").is_err());

    let leading_combinator = serde_json::json!({
        "sequences": [{
            "relation_to_previous": "descendant",
            "element": "button",
            "part": null,
            "predicates": []
        }]
    });
    assert!(serde_json::from_value::<ViewStyleSelector>(leading_combinator).is_err());

    let declaration = ViewStyleDeclaration::new(
        ViewPropertyKind::Opacity,
        ViewSpecifiedValue::Ratio {
            value: ViewRatioMilli::ONE,
        },
        ViewStyleAssignOp::Replace,
        ViewStyleSourceId::new(4),
    )
    .expect("checked declaration");
    let patch = ViewStylePatch::new(ViewStylePatchId::new(8), vec![declaration]);
    let encoded = serde_json::to_value(&patch).expect("patch encodes");
    assert_eq!(
        serde_json::from_value::<ViewStylePatch>(encoded.clone()).expect("patch decodes"),
        patch
    );

    let mut malformed = encoded;
    malformed["declarations"][0]["property"] = serde_json::json!("background_color");
    assert!(serde_json::from_value::<ViewStylePatch>(malformed).is_err());
}

#[test]
fn checked_style_records_reject_unknown_fields_at_every_nested_boundary() {
    let accent_id = token_id("token.strict.accent");
    let token = ViewStyleToken::new(
        accent_id.clone(),
        ViewStyleValueKind::Color,
        color(PresentationColor::rgb(12, 34, 56)),
        ViewStyleSourceId::new(1),
    )
    .expect("checked token");
    let declaration = ViewStyleDeclaration::new(
        ViewPropertyKind::BackgroundColor,
        ViewSpecifiedValue::Token {
            token: accent_id,
            value_kind: ViewStyleValueKind::Color,
        },
        ViewStyleAssignOp::Replace,
        ViewStyleSourceId::new(3),
    )
    .expect("checked declaration");
    let selector = ViewStyleSelector::new(vec![
        ViewStyleSelectorSequence::new(
            None,
            Some(ViewElementKind::Button),
            None,
            vec![ViewStylePredicate::Container(ViewContainerPredicate::new(
                ViewContainerAxis::InlineSize,
                ViewContainerComparison::GreaterOrEqual,
                ViewLengthMilli::new(1_000),
            ))],
        )
        .expect("non-empty selector sequence"),
    ])
    .expect("valid selector");
    let rule = ViewStyleRule::new(
        selector,
        None,
        vec![declaration],
        7,
        ViewStyleSourceId::new(2),
    )
    .expect("checked rule");
    let sheet = ViewStyleSheet::new(
        ViewStyleSheetId::try_new("style.strict").expect("sheet ID"),
        vec![token],
        vec![rule],
    )
    .expect("checked sheet");
    let encoded_sheet = serde_json::to_value(&sheet).expect("sheet encodes");

    for (label, pointer) in [
        ("sheet", ""),
        ("token", "/tokens/0"),
        ("token value", "/tokens/0/value"),
        ("nested color value", "/tokens/0/value/value"),
        (
            "nested literal presentation color",
            "/tokens/0/value/value/color",
        ),
        ("rule", "/rules/0"),
        ("selector", "/rules/0/selector"),
        ("selector sequence", "/rules/0/selector/sequences/0"),
        (
            "container predicate",
            "/rules/0/selector/sequences/0/predicates/0/container",
        ),
        ("declaration", "/rules/0/declarations/0"),
        ("declaration value", "/rules/0/declarations/0/value"),
    ] {
        assert_unknown_json_field_is_rejected::<ViewStyleSheet>(
            label,
            encoded_sheet.clone(),
            pointer,
        );
    }

    let patch_declaration = ViewStyleDeclaration::new(
        ViewPropertyKind::Opacity,
        ViewSpecifiedValue::Ratio {
            value: ViewRatioMilli::new(500).expect("bounded ratio"),
        },
        ViewStyleAssignOp::Replace,
        ViewStyleSourceId::new(4),
    )
    .expect("checked patch declaration");
    let patch = ViewStylePatch::new(ViewStylePatchId::new(8), vec![patch_declaration]);
    let encoded_patch = serde_json::to_value(&patch).expect("patch encodes");
    assert_unknown_json_field_is_rejected::<ViewStylePatch>("patch", encoded_patch.clone(), "");
    assert_unknown_json_field_is_rejected::<ViewStylePatch>(
        "patch declaration",
        encoded_patch,
        "/declarations/0",
    );
}

#[test]
fn style_value_and_application_object_codecs_reject_unknown_fields() {
    let family_list = ViewFontFamilyList::new(vec![
        ViewFontFamily::named("Arcweft Sans").expect("non-blank family"),
    ])
    .expect("non-empty family list");
    assert_unknown_json_field_is_rejected::<ViewFontFamilyList>(
        "font family list",
        serde_json::to_value(family_list).expect("font family list encodes"),
        "",
    );

    let transition = ViewStyleTransition::new(ViewPropertyKind::Opacity, 150, 25)
        .expect("opacity is transitionable");
    assert_unknown_json_field_is_rejected::<ViewStyleTransition>(
        "transition",
        serde_json::to_value(transition).expect("transition encodes"),
        "",
    );

    let radii = ViewBorderRadii {
        top_left: ViewLengthMilli::new(1),
        top_right: ViewLengthMilli::new(2),
        bottom_right: ViewLengthMilli::new(3),
        bottom_left: ViewLengthMilli::new(4),
    };
    assert_unknown_json_field_is_rejected::<ViewBorderRadii>(
        "border radii",
        serde_json::to_value(radii).expect("border radii encode"),
        "",
    );

    let shadow = ViewShadow {
        x: ViewLengthMilli::new(1),
        y: ViewLengthMilli::new(2),
        blur: ViewLengthMilli::new(3),
        spread: ViewLengthMilli::new(4),
        color: ViewColorValue::Literal {
            color: PresentationColor::rgb(12, 34, 56),
        },
        inset: false,
    };
    assert_unknown_json_field_is_rejected::<ViewShadow>(
        "shadow",
        serde_json::to_value(shadow).expect("shadow encodes"),
        "",
    );
    assert_unknown_json_field_is_rejected::<ViewShadow>(
        "shadow presentation color",
        serde_json::to_value(shadow).expect("shadow encodes"),
        "/color/color",
    );

    let filter = ViewFilter::Blur {
        radius: ViewLengthMilli::new(8),
    };
    assert_unknown_json_field_is_rejected::<ViewFilter>(
        "filter variant",
        serde_json::to_value(filter).expect("filter encodes"),
        "",
    );

    let application = ViewStyleApplicationTarget::named(
        ViewStyleSheetId::try_new("style.strict.application").expect("sheet ID"),
    );
    assert_unknown_json_field_is_rejected::<ViewStyleApplicationTarget>(
        "Style application variant",
        serde_json::to_value(application).expect("application encodes"),
        "",
    );
}

#[test]
fn duplicate_rule_source_order_is_rejected() {
    let declaration = || {
        ViewStyleDeclaration::new(
            ViewPropertyKind::Opacity,
            ViewSpecifiedValue::Ratio {
                value: ViewRatioMilli::ONE,
            },
            ViewStyleAssignOp::Replace,
            ViewStyleSourceId::new(0),
        )
        .expect("checked declaration")
    };
    let rule = || {
        ViewStyleRule::new(
            button_selector(),
            None,
            vec![declaration()],
            2,
            ViewStyleSourceId::new(0),
        )
        .expect("checked rule")
    };
    assert_eq!(
        ViewStyleSheet::new(
            ViewStyleSheetId::try_new("style.order").expect("sheet ID"),
            Vec::new(),
            vec![rule(), rule()],
        )
        .expect_err("duplicate rule order must fail"),
        ViewStyleModelError::DuplicateRuleSourceOrder(2)
    );
}

#[test]
fn sheet_constructor_canonicalizes_token_and_rule_order() {
    let token = |id: &str, channel: u8| {
        ViewStyleToken::new(
            token_id(id),
            ViewStyleValueKind::Color,
            color(PresentationColor::rgb(channel, channel, channel)),
            ViewStyleSourceId::new(0),
        )
        .expect("checked token")
    };
    let rule = |source_order| {
        ViewStyleRule::new(
            button_selector(),
            None,
            vec![
                ViewStyleDeclaration::new(
                    ViewPropertyKind::Opacity,
                    ViewSpecifiedValue::Ratio {
                        value: ViewRatioMilli::ONE,
                    },
                    ViewStyleAssignOp::Replace,
                    ViewStyleSourceId::new(0),
                )
                .expect("checked declaration"),
            ],
            source_order,
            ViewStyleSourceId::new(0),
        )
        .expect("checked rule")
    };
    let sheet = ViewStyleSheet::new(
        ViewStyleSheetId::try_new("style.canonical").expect("sheet ID"),
        vec![token("token.z", 2), token("token.a", 1)],
        vec![rule(7), rule(3)],
    )
    .expect("checked sheet");

    assert_eq!(sheet.tokens()[0].id(), &token_id("token.a"));
    assert_eq!(sheet.tokens()[1].id(), &token_id("token.z"));
    assert_eq!(sheet.rules()[0].source_order(), 3);
    assert_eq!(sheet.rules()[1].source_order(), 7);

    let reversed = ViewStyleSheet::new(
        ViewStyleSheetId::try_new("style.canonical").expect("sheet ID"),
        vec![token("token.a", 1), token("token.z", 2)],
        vec![rule(3), rule(7)],
    )
    .expect("checked sheet");
    assert_eq!(sheet, reversed);
    assert_eq!(
        serde_json::to_vec(&sheet).expect("sheet encodes"),
        serde_json::to_vec(&reversed).expect("sheet encodes")
    );
}

#[test]
fn canonical_sheet_decode_rejects_unsorted_token_and_rule_arrays() {
    let token = |id: &str, channel: u8| {
        ViewStyleToken::new(
            token_id(id),
            ViewStyleValueKind::Color,
            color(PresentationColor::rgb(channel, channel, channel)),
            ViewStyleSourceId::new(0),
        )
        .expect("checked token")
    };
    let rule = |source_order| {
        ViewStyleRule::new(
            button_selector(),
            None,
            vec![
                ViewStyleDeclaration::new(
                    ViewPropertyKind::Opacity,
                    ViewSpecifiedValue::Ratio {
                        value: ViewRatioMilli::ONE,
                    },
                    ViewStyleAssignOp::Replace,
                    ViewStyleSourceId::new(0),
                )
                .expect("checked declaration"),
            ],
            source_order,
            ViewStyleSourceId::new(0),
        )
        .expect("checked rule")
    };
    let sheet = ViewStyleSheet::new(
        ViewStyleSheetId::try_new("style.strict-canonical-decode").expect("sheet ID"),
        vec![token("token.a", 1), token("token.z", 2)],
        vec![rule(3), rule(7)],
    )
    .expect("checked sheet");

    let mut unsorted_tokens = serde_json::to_value(&sheet).expect("sheet encodes");
    unsorted_tokens["tokens"]
        .as_array_mut()
        .expect("token array")
        .reverse();
    let token_error = serde_json::from_value::<ViewStyleSheet>(unsorted_tokens)
        .expect_err("canonical decode must not sort an unsorted token array")
        .to_string();
    assert!(
        token_error.contains("Style tokens are not in canonical order"),
        "unexpected decode error: {token_error}"
    );

    let mut unsorted_rules = serde_json::to_value(&sheet).expect("sheet encodes");
    unsorted_rules["rules"]
        .as_array_mut()
        .expect("rule array")
        .reverse();
    let rule_error = serde_json::from_value::<ViewStyleSheet>(unsorted_rules)
        .expect_err("canonical decode must not sort an unsorted rule array")
        .to_string();
    assert!(
        rule_error.contains("Style rules are not in canonical source order"),
        "unexpected decode error: {rule_error}"
    );
}

#[test]
fn selector_depth_matches_combinator_chain_length() {
    let root = ViewStyleSelectorSequence::new(None, Some(ViewElementKind::Panel), None, Vec::new())
        .expect("root selector");
    let child = ViewStyleSelectorSequence::new(
        Some(ViewStyleCombinator::Child),
        Some(ViewElementKind::Button),
        None,
        Vec::new(),
    )
    .expect("child selector");
    assert_eq!(
        ViewStyleSelector::new(vec![root, child])
            .expect("selector")
            .max_depth(),
        2
    );
}

#[test]
fn style_program_canonicalizes_owned_inventories_and_rejects_noncanonical_decode() {
    let sheet_a = ViewStyleSheet::new(
        ViewStyleSheetId::try_new("style.a").expect("sheet ID"),
        Vec::new(),
        Vec::new(),
    )
    .expect("checked sheet");
    let sheet_z = ViewStyleSheet::new(
        ViewStyleSheetId::try_new("style.z").expect("sheet ID"),
        Vec::new(),
        Vec::new(),
    )
    .expect("checked sheet");
    let patch_0 = ViewStylePatch::new(ViewStylePatchId::new(0), Vec::new());
    let patch_2 = ViewStylePatch::new(ViewStylePatchId::new(2), Vec::new());
    let program = ViewStyleProgram::try_new(
        vec![sheet_z.clone(), sheet_a.clone()],
        vec![patch_2.clone(), patch_0.clone()],
    )
    .expect("canonical program");

    assert_eq!(program.sheets(), [sheet_a.clone(), sheet_z.clone()]);
    assert_eq!(program.patches(), [patch_0.clone(), patch_2.clone()]);
    assert_eq!(
        program.sheet(sheet_z.id()).map(ViewStyleSheet::id),
        Some(sheet_z.id())
    );
    assert_eq!(
        program
            .patch(ViewStylePatchId::new(2))
            .map(ViewStylePatch::id),
        Some(ViewStylePatchId::new(2))
    );
    assert!(matches!(
        ViewStyleProgram::try_new(vec![sheet_a.clone(), sheet_a], Vec::new()),
        Err(ViewStyleModelError::DuplicateSheet(_))
    ));
    assert!(matches!(
        ViewStyleProgram::try_new(Vec::new(), vec![patch_0.clone(), patch_0]),
        Err(ViewStyleModelError::DuplicatePatch(_))
    ));

    let mut noncanonical = serde_json::to_value(&program).expect("program encodes");
    noncanonical["sheets"]
        .as_array_mut()
        .expect("sheet array")
        .reverse();
    assert!(serde_json::from_value::<ViewStyleProgram>(noncanonical).is_err());

    let mut unknown = serde_json::to_value(&program).expect("program encodes");
    unknown
        .as_object_mut()
        .expect("program object")
        .insert("syntax".to_owned(), serde_json::json!("removed"));
    assert!(serde_json::from_value::<ViewStyleProgram>(unknown).is_err());
}

#[test]
fn style_program_owns_inline_patch_token_validation() {
    let referenced = token_id("token.inline-accent");
    let declaration = ViewStyleDeclaration::new(
        ViewPropertyKind::BackgroundColor,
        ViewSpecifiedValue::Token {
            token: referenced.clone(),
            value_kind: ViewStyleValueKind::Color,
        },
        ViewStyleAssignOp::Replace,
        ViewStyleSourceId::new(0),
    )
    .expect("checked declaration");
    let patch = ViewStylePatch::new(ViewStylePatchId::new(0), vec![declaration]);
    assert!(matches!(
        ViewStyleProgram::try_new(Vec::new(), vec![patch.clone()]),
        Err(ViewStyleModelError::MissingInlineToken { .. })
    ));

    let color_sheet = |id: &str, channel: u8| {
        ViewStyleSheet::new(
            ViewStyleSheetId::try_new(id).expect("sheet ID"),
            vec![
                ViewStyleToken::new(
                    referenced.clone(),
                    ViewStyleValueKind::Color,
                    color(PresentationColor::rgb(channel, channel, channel)),
                    ViewStyleSourceId::new(0),
                )
                .expect("checked token"),
            ],
            Vec::new(),
        )
        .expect("checked sheet")
    };
    assert!(matches!(
        ViewStyleProgram::try_new(
            vec![color_sheet("style.a", 1), color_sheet("style.b", 2)],
            vec![patch.clone()],
        ),
        Err(ViewStyleModelError::AmbiguousInlineToken { sheet_count: 2, .. })
    ));

    let length_sheet = ViewStyleSheet::new(
        ViewStyleSheetId::try_new("style.length").expect("sheet ID"),
        vec![
            ViewStyleToken::new(
                referenced,
                ViewStyleValueKind::Length,
                ViewSpecifiedValue::Length {
                    value: ViewLengthMilli::new(1_000),
                },
                ViewStyleSourceId::new(0),
            )
            .expect("checked token"),
        ],
        Vec::new(),
    )
    .expect("checked sheet");
    assert!(matches!(
        ViewStyleProgram::try_new(vec![length_sheet], vec![patch]),
        Err(ViewStyleModelError::InlineTokenKindMismatch { .. })
    ));
}
