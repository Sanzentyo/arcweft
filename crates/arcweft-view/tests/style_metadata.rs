use arcweft_id::PublicId;
use arcweft_view::geometry::{ViewGeometryPropertySupport, ViewRepresentedGeometryFeature};
use arcweft_view::{
    ViewAlignment, ViewBlendMode, ViewDisplay, ViewElementKind, ViewElementState,
    ViewFlexDirection, ViewFlexWrap, ViewFontFamily, ViewFontFamilyList, ViewFontStyle,
    ViewFontWeight, ViewInteractionSelector, ViewLocalPartName, ViewOverflow, ViewPartName,
    ViewPosition, ViewPropertyExpansion, ViewPropertyKind, ViewRatioMilli, ViewScalarMilli,
    ViewSpecifiedValue, ViewStyleApplication, ViewStyleApplicationTarget, ViewStyleBoundaryFacts,
    ViewStyleCombinator,
    ViewStyleInvalidationSet, ViewStylePatchId, ViewStylePredicate, ViewStyleScopeId,
    ViewStyleSelector, ViewStyleSelectorSequence, ViewStyleSheetId, ViewStyleTokenId,
    ViewStyleValueKind, ViewSystemFontFamily,
};
use std::collections::BTreeSet;

#[test]
fn canonical_property_names_are_unique_and_round_trip() {
    let names = ViewPropertyKind::ALL
        .iter()
        .copied()
        .map(ViewPropertyKind::source_name)
        .collect::<BTreeSet<_>>();

    assert_eq!(names.len(), ViewPropertyKind::ALL.len());
    for property in ViewPropertyKind::ALL {
        assert_eq!(
            ViewPropertyKind::from_source_name(property.source_name()),
            Some(*property)
        );
    }

    assert_eq!(ViewPropertyKind::from_source_name("background_color"), None);
    assert_eq!(ViewPropertyKind::from_source_name("semantic-label"), None);
    assert_eq!(ViewPropertyKind::from_source_name("custom"), None);
}

#[test]
fn canonical_selector_names_are_exact_and_round_trip() {
    for selector in ViewInteractionSelector::ALL {
        assert_eq!(
            ViewInteractionSelector::from_source_name(selector.source_name()),
            Some(*selector)
        );
    }
    for state in ViewElementState::ALL {
        assert_eq!(
            ViewElementState::from_source_name(state.source_name()),
            Some(*state)
        );
    }

    assert_eq!(ViewInteractionSelector::from_source_name("pressed"), None);
    assert_eq!(ViewElementState::from_source_name("focus_visible"), None);
}

#[test]
fn canonical_value_names_are_exact_and_round_trip() {
    for kind in ViewStyleValueKind::ALL {
        assert_eq!(
            ViewStyleValueKind::from_source_name(kind.source_name()),
            Some(*kind)
        );
    }
    for family in ViewSystemFontFamily::ALL {
        assert_eq!(
            ViewSystemFontFamily::from_source_name(family.source_name()),
            Some(*family)
        );
    }
    for display in ViewDisplay::ALL {
        assert_eq!(
            ViewDisplay::from_source_name(display.source_name()),
            Some(*display)
        );
    }
    for position in ViewPosition::ALL {
        assert_eq!(
            ViewPosition::from_source_name(position.source_name()),
            Some(*position)
        );
    }
    for overflow in ViewOverflow::ALL {
        assert_eq!(
            ViewOverflow::from_source_name(overflow.source_name()),
            Some(*overflow)
        );
    }
    for direction in ViewFlexDirection::ALL {
        assert_eq!(
            ViewFlexDirection::from_source_name(direction.source_name()),
            Some(*direction)
        );
    }
    for wrap in ViewFlexWrap::ALL {
        assert_eq!(
            ViewFlexWrap::from_source_name(wrap.source_name()),
            Some(*wrap)
        );
    }
    for style in ViewFontStyle::ALL {
        assert_eq!(
            ViewFontStyle::from_source_name(style.source_name()),
            Some(*style)
        );
    }
    for alignment in ViewAlignment::ALL {
        assert_eq!(
            ViewAlignment::from_source_name(alignment.source_name()),
            Some(*alignment)
        );
    }
    for mode in ViewBlendMode::ALL {
        assert_eq!(
            ViewBlendMode::from_source_name(mode.source_name()),
            Some(*mode)
        );
    }

    assert_eq!(ViewDisplay::from_source_name("flex"), None);
    assert_eq!(ViewAlignment::from_source_name("space-between"), None);
    assert_eq!(ViewSystemFontFamily::from_source_name("ui"), None);
}

#[test]
fn property_metadata_encodes_value_and_element_constraints() {
    assert_eq!(
        ViewPropertyKind::BackgroundColor.value_kind(),
        ViewStyleValueKind::Color
    );
    assert_eq!(
        ViewPropertyKind::FontFamily.value_kind(),
        ViewStyleValueKind::FontFamilyList
    );
    assert!(ViewPropertyKind::FontFamily.is_inherited());
    assert!(ViewPropertyKind::BoxShadow.is_appendable());
    assert!(!ViewPropertyKind::Opacity.is_appendable());
    assert!(ViewPropertyKind::Opacity.is_transitionable());

    assert!(ViewPropertyKind::PlaceholderColor.applies_to(ViewElementKind::TextField));
    assert!(ViewPropertyKind::PlaceholderColor.applies_to(ViewElementKind::SecureField));
    assert!(!ViewPropertyKind::PlaceholderColor.applies_to(ViewElementKind::Button));
    assert!(ViewPropertyKind::BackgroundColor.applies_to(ViewElementKind::Panel));
    assert!(!ViewPropertyKind::FlexDirection.applies_to(ViewElementKind::Button));
    assert!(ViewPropertyKind::FlexDirection.applies_to(ViewElementKind::Row));

    assert!(ViewAlignment::SpaceBetween.applies_to(ViewPropertyKind::JustifyContent));
    assert!(!ViewAlignment::SpaceBetween.applies_to(ViewPropertyKind::AlignSelf));
    assert!(ViewAlignment::Stretch.applies_to(ViewPropertyKind::AlignItems));
    assert!(!ViewAlignment::Stretch.applies_to(ViewPropertyKind::TextAlign));
    assert!(ViewAlignment::Center.applies_to(ViewPropertyKind::TextAlign));
    assert!(!ViewAlignment::Center.applies_to(ViewPropertyKind::Opacity));
}

#[test]
fn typed_ids_and_applications_preserve_scope_order_and_boundaries() {
    let sheet = ViewStyleSheetId::try_new("style.controls").expect("valid sheet id");
    let encoded = serde_json::to_string(&sheet).expect("sheet id serializes");
    assert_eq!(encoded, "\"style.controls\"");
    assert_eq!(
        serde_json::from_str::<ViewStyleSheetId>(&encoded)
            .expect("sheet id deserializes")
            .public_id()
            .as_str(),
        "style.controls"
    );
    assert!(serde_json::from_str::<ViewStyleSheetId>("\"#style.bad\"").is_err());

    let boundary = ViewStyleBoundaryFacts::nested_view(1, true, false);
    let application = ViewStyleApplication::new(
        ViewStyleApplicationTarget::named(sheet),
        ViewStyleScopeId::new(7),
        3,
        11,
        boundary,
    );
    assert_eq!(application.scope().value(), 7);
    assert_eq!(application.scope_depth(), 3);
    assert_eq!(application.application_order(), 11);
    assert!(application.boundary().allows_selector_traversal());
    assert!(!application.boundary().allows_inherited_root());
    let private = ViewLocalPartName::try_new("part.private").unwrap();
    let public = ViewPartName::try_new("part.public").unwrap();
    assert!(
        application
            .boundary()
            .matches_part(&public, Some(&private), Some(&public))
    );

    let transitive_export = ViewStyleBoundaryFacts::nested_view(2, true, false);
    assert_eq!(transitive_export.crossed_view_boundaries(), 2);
    assert!(!transitive_export.allows_selector_traversal());
    assert!(!transitive_export.matches_part(&public, Some(&private), Some(&public)));

    let inline = ViewStyleApplicationTarget::inline(ViewStylePatchId::new(9));
    assert!(matches!(
        inline,
        ViewStyleApplicationTarget::Inline { patch } if patch.value() == 9
    ));
}

#[test]
fn selector_sequences_validate_relations_and_compute_specificity() {
    let first =
        ViewStyleSelectorSequence::new(None, Some(ViewElementKind::Panel), None, Vec::new())
            .expect("element sequence");
    let second = ViewStyleSelectorSequence::new(
        Some(ViewStyleCombinator::Child),
        Some(ViewElementKind::Button),
        Some(ViewPartName::try_new("part.action").expect("valid part")),
        vec![ViewStylePredicate::Interaction(
            ViewInteractionSelector::Hovered,
        )],
    )
    .expect("child sequence");
    let selector = ViewStyleSelector::new(vec![first, second]).expect("valid selector");
    let specificity = selector.specificity().unwrap();
    assert_eq!(specificity.predicates(), 2);
    assert_eq!(specificity.elements(), 2);
    assert_eq!(selector.max_depth(), 2);

    let leading_combinator = ViewStyleSelectorSequence::new(
        Some(ViewStyleCombinator::Descendant),
        Some(ViewElementKind::Button),
        None,
        Vec::new(),
    )
    .expect("non-empty sequence");
    assert!(ViewStyleSelector::new(vec![leading_combinator]).is_none());
}

#[test]
fn font_families_and_closed_specified_variants_are_typed() {
    assert!(ViewFontFamily::named("  ").is_none());
    let families = ViewFontFamilyList::new(vec![
        ViewFontFamily::named("Noto Sans JP").expect("named family"),
        ViewFontFamily::system(ViewSystemFontFamily::Ui),
    ])
    .expect("non-empty family list");
    assert_eq!(families.as_slice().len(), 2);
    assert_eq!(
        ViewSpecifiedValue::FontFamilyList { value: families }.kind(),
        ViewStyleValueKind::FontFamilyList
    );
    assert_eq!(
        ViewSpecifiedValue::BlendMode {
            value: ViewBlendMode::Multiply
        }
        .kind(),
        ViewStyleValueKind::BlendMode
    );
}

#[test]
fn invalidation_and_specified_values_keep_distinct_units_and_kinds() {
    let font_invalidation = ViewPropertyKind::FontSize.default_invalidation();
    assert!(font_invalidation.contains(ViewStyleInvalidationSet::TEXT_LAYOUT));
    assert!(font_invalidation.contains(ViewStyleInvalidationSet::LAYOUT));
    assert!(font_invalidation.contains(ViewStyleInvalidationSet::PAINT));

    assert_eq!(ViewRatioMilli::new(1_001), None);
    assert_eq!(ViewScalarMilli::new(1_800).value(), 1_800);
    assert_eq!(
        ViewPropertyKind::Scale.value_kind(),
        ViewStyleValueKind::Scalar
    );
    assert_eq!(
        ViewPropertyKind::FlexGrow.value_kind(),
        ViewStyleValueKind::Scalar
    );
    assert_eq!(ViewFontWeight::new(0), None);
    assert_eq!(
        ViewFontWeight::new(720).map(ViewFontWeight::value),
        Some(720)
    );

    let token = ViewSpecifiedValue::Token {
        token: ViewStyleTokenId::from_public_id(
            PublicId::try_new("style_token.color.accent").expect("valid token id"),
        ),
        value_kind: ViewStyleValueKind::Color,
    };
    assert_eq!(token.kind(), ViewStyleValueKind::Color);
    assert_eq!(
        ViewSpecifiedValue::Ratio {
            value: ViewRatioMilli::ONE
        }
        .kind(),
        ViewStyleValueKind::Ratio
    );
    assert_eq!(
        ViewSpecifiedValue::Scalar {
            value: ViewScalarMilli::new(1_250)
        }
        .kind(),
        ViewStyleValueKind::Scalar
    );
}

#[test]
fn physical_geometry_metadata_is_exhaustive_and_distinguishes_representation() {
    assert_eq!(
        ViewPropertyKind::Width.geometry_support(),
        ViewGeometryPropertySupport::Supported
    );
    assert_eq!(
        ViewPropertyKind::FlexGrow.geometry_support(),
        ViewGeometryPropertySupport::RepresentedOnly(
            ViewRepresentedGeometryFeature::FlexDistribution,
        )
    );
    assert_eq!(
        ViewPropertyKind::Color.geometry_support(),
        ViewGeometryPropertySupport::NotGeometry
    );

    let layout = ViewPropertyKind::Width.default_invalidation();
    assert!(layout.contains(ViewStyleInvalidationSet::PHYSICAL_GEOMETRY));
    assert!(layout.contains(ViewStyleInvalidationSet::LAYOUT));
    assert!(layout.contains(ViewStyleInvalidationSet::HIT_TEST));
    assert!(layout.contains(ViewStyleInvalidationSet::SCROLL));

    let transform = ViewPropertyKind::TranslateX.default_invalidation();
    assert!(transform.contains(ViewStyleInvalidationSet::PHYSICAL_GEOMETRY));
    assert!(transform.contains(ViewStyleInvalidationSet::COMPOSITE));
    assert!(!transform.contains(ViewStyleInvalidationSet::LAYOUT));
}

#[test]
fn gap_is_a_noncanonical_two_axis_shorthand() {
    assert!(!ViewPropertyKind::Gap.is_computed_canonical());
    assert_eq!(
        ViewPropertyKind::Gap.shorthand_expansion(),
        ViewPropertyExpansion::TwoPhysicalAxes
    );
    assert_eq!(
        ViewPropertyKind::Gap.expanded_properties(),
        &[ViewPropertyKind::RowGap, ViewPropertyKind::ColumnGap]
    );
}
