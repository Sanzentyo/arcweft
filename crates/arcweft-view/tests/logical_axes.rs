use arcweft_view::style::{
    ViewAxisSign, ViewBoxAxisMode, ViewBoxAxisModeError, ViewLengthMilli, ViewPhysicalAxis,
    ViewPhysicalSide, ViewPropertyKind, ViewPropertyValueTransform, ViewResolvedAxis,
    ViewResolvedBoxAxes, ViewSpecifiedValue, ViewStyleAssignOp, ViewStyleDeclaration,
    ViewStyleModelError, ViewStyleSourceId, ViewStyleValueKind,
};

#[test]
fn closed_axis_inventory_round_trips_source_and_product_spelling() {
    for mode in ViewBoxAxisMode::ALL {
        assert_eq!(
            ViewBoxAxisMode::from_source_name(mode.source_name()),
            Some(*mode)
        );
        assert_eq!(mode.resolved().mode(), *mode);
    }
    assert_eq!(ViewBoxAxisMode::from_source_name("horizontal_ltr"), None);
    assert_eq!(
        serde_json::to_string(&ViewPropertyKind::BoxAxes).unwrap(),
        "\"box_axes\""
    );
    assert_eq!(
        serde_json::to_string(&ViewSpecifiedValue::BoxAxes {
            value: ViewBoxAxisMode::VerticalRl,
        })
        .unwrap(),
        r#"{"kind":"box_axes","value":"vertical_rl"}"#
    );
    assert!(
        serde_json::from_str::<ViewSpecifiedValue>(
            r#"{"kind":"box_axes","value":"vertical_unknown"}"#
        )
        .is_err()
    );
    assert_eq!(ViewStyleValueKind::BoxAxes.source_name(), "BoxAxes");
}

#[test]
fn resolved_axis_constructor_rejects_non_closed_progressions() {
    let x = ViewResolvedAxis::new(
        ViewPhysicalAxis::X,
        ViewPhysicalSide::Left,
        ViewPhysicalSide::Right,
        ViewAxisSign::Positive,
    );
    assert_eq!(
        ViewResolvedBoxAxes::try_new(x, x),
        Err(ViewBoxAxisModeError::NonOrthogonal)
    );
    let invalid_y = ViewResolvedAxis::new(
        ViewPhysicalAxis::Y,
        ViewPhysicalSide::Left,
        ViewPhysicalSide::Right,
        ViewAxisSign::Positive,
    );
    assert_eq!(
        ViewResolvedBoxAxes::try_new(x, invalid_y),
        Err(ViewBoxAxisModeError::InvalidSides)
    );
    let bottom_to_top = ViewResolvedAxis::new(
        ViewPhysicalAxis::Y,
        ViewPhysicalSide::Bottom,
        ViewPhysicalSide::Top,
        ViewAxisSign::Negative,
    );
    assert_eq!(
        ViewResolvedBoxAxes::try_new(x, bottom_to_top),
        Err(ViewBoxAxisModeError::UnsupportedProgression)
    );
}

#[test]
fn axis_context_metadata_and_append_rejection_are_closed() {
    let source = ViewStyleSourceId::new(17);
    assert!(ViewPropertyKind::BoxAxes.is_inherited());
    assert!(!ViewPropertyKind::BoxAxes.is_appendable());
    assert!(!ViewPropertyKind::BoxAxes.is_transitionable());
    assert_eq!(
        ViewStyleDeclaration::new(
            ViewPropertyKind::BoxAxes,
            ViewSpecifiedValue::BoxAxes {
                value: ViewBoxAxisMode::VerticalRl,
            },
            ViewStyleAssignOp::Append,
            source,
        ),
        Err(ViewStyleModelError::AxisContextAppend {
            style_source: source,
        })
    );
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one exhaustive table keeps all 22 normative aliases auditable across four modes"
)]
fn every_logical_alias_maps_to_the_normative_physical_slot() {
    use ViewBoxAxisMode::{HorizontalLtr, HorizontalRtl, VerticalLr, VerticalRl};
    use ViewPropertyKind as P;

    let mappings = [
        (P::InlineSize, [P::Width, P::Width, P::Height, P::Height]),
        (P::BlockSize, [P::Height, P::Height, P::Width, P::Width]),
        (
            P::MinInlineSize,
            [P::MinWidth, P::MinWidth, P::MinHeight, P::MinHeight],
        ),
        (
            P::MinBlockSize,
            [P::MinHeight, P::MinHeight, P::MinWidth, P::MinWidth],
        ),
        (
            P::MaxInlineSize,
            [P::MaxWidth, P::MaxWidth, P::MaxHeight, P::MaxHeight],
        ),
        (
            P::MaxBlockSize,
            [P::MaxHeight, P::MaxHeight, P::MaxWidth, P::MaxWidth],
        ),
        (
            P::PaddingInlineStart,
            [
                P::PaddingLeft,
                P::PaddingRight,
                P::PaddingTop,
                P::PaddingTop,
            ],
        ),
        (
            P::PaddingInlineEnd,
            [
                P::PaddingRight,
                P::PaddingLeft,
                P::PaddingBottom,
                P::PaddingBottom,
            ],
        ),
        (
            P::PaddingBlockStart,
            [
                P::PaddingTop,
                P::PaddingTop,
                P::PaddingRight,
                P::PaddingLeft,
            ],
        ),
        (
            P::PaddingBlockEnd,
            [
                P::PaddingBottom,
                P::PaddingBottom,
                P::PaddingLeft,
                P::PaddingRight,
            ],
        ),
        (
            P::MarginInlineStart,
            [P::MarginLeft, P::MarginRight, P::MarginTop, P::MarginTop],
        ),
        (
            P::MarginInlineEnd,
            [
                P::MarginRight,
                P::MarginLeft,
                P::MarginBottom,
                P::MarginBottom,
            ],
        ),
        (
            P::MarginBlockStart,
            [P::MarginTop, P::MarginTop, P::MarginRight, P::MarginLeft],
        ),
        (
            P::MarginBlockEnd,
            [
                P::MarginBottom,
                P::MarginBottom,
                P::MarginLeft,
                P::MarginRight,
            ],
        ),
        (P::InsetInlineStart, [P::Left, P::Right, P::Top, P::Top]),
        (P::InsetInlineEnd, [P::Right, P::Left, P::Bottom, P::Bottom]),
        (P::InsetBlockStart, [P::Top, P::Top, P::Right, P::Left]),
        (P::InsetBlockEnd, [P::Bottom, P::Bottom, P::Left, P::Right]),
        (
            P::TranslateInline,
            [P::TranslateX, P::TranslateX, P::TranslateY, P::TranslateY],
        ),
        (
            P::TranslateBlock,
            [P::TranslateY, P::TranslateY, P::TranslateX, P::TranslateX],
        ),
        (
            P::OverflowInline,
            [P::OverflowX, P::OverflowX, P::OverflowY, P::OverflowY],
        ),
        (
            P::OverflowBlock,
            [P::OverflowY, P::OverflowY, P::OverflowX, P::OverflowX],
        ),
    ];
    let modes = [HorizontalLtr, HorizontalRtl, VerticalRl, VerticalLr];
    for (logical, expected) in mappings {
        for (mode, expected) in modes.into_iter().zip(expected) {
            assert_eq!(
                logical.resolve_for_axes(mode).resolved().as_property(),
                expected,
                "{} in {mode:?}",
                logical.source_name()
            );
        }
    }
}

#[test]
fn logical_translation_sign_is_checked_before_mapping() {
    use ViewBoxAxisMode::{HorizontalLtr, HorizontalRtl, VerticalLr, VerticalRl};

    let expected = [
        ViewAxisSign::Positive,
        ViewAxisSign::Negative,
        ViewAxisSign::Positive,
        ViewAxisSign::Positive,
    ];
    for (mode, expected) in ViewBoxAxisMode::ALL.iter().copied().zip(expected) {
        assert_eq!(
            ViewPropertyKind::TranslateInline
                .resolve_for_axes(mode)
                .value_transform(),
            ViewPropertyValueTransform::SignedLength(expected)
        );
    }
    assert_eq!(
        ViewLengthMilli::new(12)
            .checked_apply_axis_sign(ViewAxisSign::Negative)
            .unwrap()
            .value(),
        -12
    );
    assert!(
        ViewLengthMilli::new(i32::MIN)
            .checked_apply_axis_sign(ViewAxisSign::Positive)
            .is_err()
    );

    let _ = (HorizontalLtr, HorizontalRtl, VerticalRl, VerticalLr);
}
