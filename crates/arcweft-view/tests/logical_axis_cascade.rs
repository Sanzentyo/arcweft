use arcweft_presentation::appearance::{
    ColorScheme, ContrastPreference, PresentationEnvironment, PresentationEnvironmentValues,
    TextScaleMilli,
};
use arcweft_view::geometry::{
    ViewGeometryConsumer, ViewGeometryError, ViewGeometryField, ViewRepresentedGeometryFeature,
};
use arcweft_view::style::{
    ComputedViewStyle, ViewAxisProviderParticipation, ViewAxisUsageSet, ViewBoxAxisHostSeed,
    ViewBoxAxisMode, ViewBoxAxisSeedGeneration, ViewBoxAxisSource, ViewInheritedBoxAxes,
    ViewLengthMilli, ViewOverflow, ViewPropertyKind, ViewSpecifiedValue, ViewStyleApplication,
    ViewStyleApplicationTarget, ViewStyleAssignOp, ViewStyleBoundaryFacts, ViewStyleDeclaration,
    ViewStyleModelError, ViewStyleNodeFacts, ViewStyleNodeKey, ViewStylePatch, ViewStylePatchId,
    ViewStyleProgram, ViewStyleResolveContext, ViewStyleResolveError, ViewStyleResolver,
    ViewStyleRevisionSet, ViewStyleScopeId, ViewStyleSheet, ViewStyleSheetId, ViewStyleSourceId,
    ViewStyleToken, ViewStyleTokenId, ViewStyleTraceMode, ViewStyleTransition, ViewStyleValueKind,
};
use arcweft_view::{
    ViewDisplay, ViewElementKind, ViewFlexDirection, ViewFlexWrap, ViewMountId, ViewPhysicalFlow,
    ViewPosition, ViewScalarMilli,
};

fn environment(color_scheme: ColorScheme) -> PresentationEnvironment {
    PresentationEnvironment::initial(PresentationEnvironmentValues::new(
        color_scheme,
        ContrastPreference::Standard,
        false,
        TextScaleMilli::ONE,
    ))
}

fn declaration(
    property: ViewPropertyKind,
    value: ViewSpecifiedValue,
    source: u32,
) -> ViewStyleDeclaration {
    ViewStyleDeclaration::new(
        property,
        value,
        ViewStyleAssignOp::Replace,
        ViewStyleSourceId::new(source),
    )
    .unwrap()
}

fn length(property: ViewPropertyKind, value: i32, source: u32) -> ViewStyleDeclaration {
    declaration(
        property,
        ViewSpecifiedValue::Length {
            value: ViewLengthMilli::new(value),
        },
        source,
    )
}

fn resolve(
    declarations: Vec<ViewStyleDeclaration>,
    parent: Option<&ComputedViewStyle>,
) -> Result<ComputedViewStyle, ViewStyleResolveError> {
    let patch_id = ViewStylePatchId::new(1);
    let program = ViewStyleProgram::try_new(
        Vec::new(),
        vec![ViewStylePatch::new(patch_id, declarations)],
    )
    .unwrap();
    resolve_program(&program, patch_id, parent)
}

fn resolve_program(
    program: &ViewStyleProgram,
    patch_id: ViewStylePatchId,
    parent: Option<&ComputedViewStyle>,
) -> Result<ComputedViewStyle, ViewStyleResolveError> {
    let application = ViewStyleApplication::new(
        ViewStyleApplicationTarget::inline(patch_id),
        ViewStyleScopeId::new(1),
        1,
        1,
        ViewStyleBoundaryFacts::SAME_VIEW,
    );
    let applications = [application];
    let node = ViewStyleNodeFacts::new(Some(ViewElementKind::Panel));
    let key = ViewStyleNodeKey::new(ViewMountId::from_raw(1), vec![1], 1);
    let parent_key = parent.map(|_| ViewStyleNodeKey::new(ViewMountId::from_raw(1), Vec::new(), 0));
    let environment = environment(ColorScheme::Light);
    ViewStyleResolver::default()
        .resolve(
            program,
            &ViewStyleResolveContext {
                node_key: &key,
                node: &node,
                ancestors: &[],
                applications: &applications,
                parent,
                parent_node_key: parent_key.as_ref(),
                inherited_axes: parent.map_or_else(
                    || {
                        ViewInheritedBoxAxes::for_host_seed(
                            key.mount(),
                            ViewBoxAxisSeedGeneration::INITIAL,
                            ViewBoxAxisHostSeed::Default,
                        )
                    },
                    |parent| parent.axes().inherited_snapshot(),
                ),
                axis_provider_participation: ViewAxisProviderParticipation::ProjectionOnly,
                environment: &environment,
                revisions: ViewStyleRevisionSet::default(),
                trace: ViewStyleTraceMode::Off,
            },
        )
        .map(|result| std::sync::Arc::unwrap_or_clone(result.into_computed()))
}

#[test]
fn vertical_rl_resolves_aliases_shorthands_signs_and_transitions_once() {
    let transition = ViewStyleTransition::new(ViewPropertyKind::TranslateInline, 120, 30).unwrap();
    let computed = resolve(
        vec![
            declaration(
                ViewPropertyKind::BoxAxes,
                ViewSpecifiedValue::BoxAxes {
                    value: ViewBoxAxisMode::VerticalRl,
                },
                1,
            ),
            length(ViewPropertyKind::Width, 5, 2),
            length(ViewPropertyKind::BlockSize, 20, 3),
            length(ViewPropertyKind::InlineSize, 10, 4),
            length(ViewPropertyKind::Padding, 3, 5),
            length(ViewPropertyKind::PaddingBlockStart, 9, 6),
            length(ViewPropertyKind::TranslateBlock, 7, 7),
            declaration(
                ViewPropertyKind::OverflowInline,
                ViewSpecifiedValue::Overflow {
                    value: ViewOverflow::Hidden,
                },
                8,
            ),
            declaration(
                ViewPropertyKind::Transition,
                ViewSpecifiedValue::Transition {
                    value: vec![transition],
                },
                9,
            ),
        ],
        None,
    )
    .unwrap();

    assert_eq!(computed.axes().mode(), ViewBoxAxisMode::VerticalRl);
    assert!(matches!(
        computed.axes().source(),
        ViewBoxAxisSource::Style { .. }
    ));
    assert_eq!(computed.value(ViewPropertyKind::BlockSize), None);
    assert_eq!(
        computed
            .value(ViewPropertyKind::Width)
            .and_then(|value| match value {
                ViewSpecifiedValue::Length { value } => Some(value.value()),
                _ => None,
            }),
        Some(20)
    );
    let width = computed.property(ViewPropertyKind::Width).unwrap();
    assert_eq!(width.authored_property(), ViewPropertyKind::BlockSize);
    assert_eq!(width.expanded_property(), ViewPropertyKind::BlockSize);
    assert_eq!(
        width.resolved_property().as_property(),
        ViewPropertyKind::Width
    );

    let physical = computed.physical_box();
    assert_eq!(physical.axes, ViewBoxAxisMode::VerticalRl);
    assert_eq!(physical.width.map(ViewLengthMilli::value), Some(20));
    assert_eq!(physical.height.map(ViewLengthMilli::value), Some(10));
    assert_eq!(physical.padding.top.value(), 3);
    assert_eq!(physical.padding.right.value(), 9);
    assert_eq!(physical.padding.bottom.value(), 3);
    assert_eq!(physical.padding.left.value(), 3);
    assert_eq!(physical.translate_x.value(), -7);
    assert_eq!(physical.translate_y.value(), 0);
    assert_eq!(physical.overflow_x, ViewOverflow::Visible);
    assert_eq!(physical.overflow_y, ViewOverflow::Hidden);
    assert_eq!(computed.transitions().len(), 1);
    assert_eq!(
        computed.transitions()[0].resolved_property().as_property(),
        ViewPropertyKind::TranslateY
    );
    assert_eq!(
        computed.transitions()[0].axis_snapshot(),
        ViewBoxAxisMode::VerticalRl
    );
    assert!(computed.axis_usage().contains(ViewAxisUsageSet::SIZE));
    assert!(
        computed
            .axis_usage()
            .contains(ViewAxisUsageSet::TRANSLATION)
    );
    assert!(
        computed
            .axis_usage()
            .contains(ViewAxisUsageSet::TRANSITION_TARGET)
    );
}

#[test]
fn physical_geometry_projection_includes_box_and_container_inputs() {
    let computed = resolve(
        vec![
            declaration(
                ViewPropertyKind::Display,
                ViewSpecifiedValue::Display {
                    value: ViewDisplay::Flex,
                },
                1,
            ),
            declaration(
                ViewPropertyKind::Position,
                ViewSpecifiedValue::Position {
                    value: ViewPosition::Absolute,
                },
                2,
            ),
            length(ViewPropertyKind::BorderWidth, 2, 3),
            declaration(
                ViewPropertyKind::Scale,
                ViewSpecifiedValue::Scalar {
                    value: ViewScalarMilli::new(1_250),
                },
                4,
            ),
            length(ViewPropertyKind::ColumnGap, 4, 5),
        ],
        None,
    )
    .unwrap();

    let physical = computed.physical_box();
    assert_eq!(physical.display, Some(ViewDisplay::Flex));
    assert_eq!(physical.position, ViewPosition::Absolute);
    assert_eq!(physical.border.top.value(), 2);
    assert_eq!(physical.border.right.value(), 2);
    assert_eq!(physical.border.bottom.value(), 2);
    assert_eq!(physical.border.left.value(), 2);
    assert_eq!(physical.scale.value(), 1_250);

    let geometry_node = ViewStyleNodeKey::new(ViewMountId::from_raw(1), vec![1], 1);
    let container = computed
        .physical_container(&geometry_node, ViewElementKind::Panel)
        .expect("flex container geometry is executable")
        .expect("display Flex retains a geometry container");
    assert_eq!(container.flow, ViewPhysicalFlow::Row);
    assert_eq!(container.row_gap.value(), 0);
    assert_eq!(container.column_gap.value(), 4);
}

#[test]
fn physical_container_enforces_element_display_and_gap_ownership() {
    let node = ViewStyleNodeKey::new(ViewMountId::from_raw(1), vec![2], 2);
    let defaults = ComputedViewStyle::default();
    for (element, expected) in [
        (ViewElementKind::Panel, Some(ViewPhysicalFlow::Overlay)),
        (ViewElementKind::Box, Some(ViewPhysicalFlow::Overlay)),
        (ViewElementKind::Scroll, Some(ViewPhysicalFlow::Overlay)),
        (ViewElementKind::Row, Some(ViewPhysicalFlow::Row)),
        (ViewElementKind::Column, Some(ViewPhysicalFlow::Column)),
        (ViewElementKind::Stack, Some(ViewPhysicalFlow::Overlay)),
        (ViewElementKind::Button, None),
        (ViewElementKind::TextField, None),
        (ViewElementKind::TextArea, None),
        (ViewElementKind::SecureField, None),
    ] {
        assert_eq!(element.default_physical_flow(), expected);
        assert_eq!(
            defaults
                .physical_container(&node, element)
                .unwrap()
                .map(|container| container.flow),
            expected
        );
    }

    let leaf_container_property = resolve(
        vec![
            declaration(
                ViewPropertyKind::Display,
                ViewSpecifiedValue::Display {
                    value: ViewDisplay::None,
                },
                1,
            ),
            declaration(
                ViewPropertyKind::FlexDirection,
                ViewSpecifiedValue::FlexDirection {
                    value: ViewFlexDirection::Column,
                },
                2,
            ),
        ],
        None,
    )
    .unwrap();
    assert_eq!(
        leaf_container_property.physical_container(&node, ViewElementKind::Button),
        Err(ViewGeometryError::ContainerStyleOnLeaf {
            node: node.clone(),
            element: ViewElementKind::Button,
            property: ViewPropertyKind::FlexDirection,
        })
    );

    let stack = resolve(
        vec![declaration(
            ViewPropertyKind::Display,
            ViewSpecifiedValue::Display {
                value: ViewDisplay::Stack,
            },
            1,
        )],
        None,
    )
    .unwrap();
    assert_eq!(
        stack.physical_container(&node, ViewElementKind::TextField),
        Err(ViewGeometryError::DisplayRequiresContainer {
            node: node.clone(),
            element: ViewElementKind::TextField,
            display: ViewDisplay::Stack,
        })
    );
}

#[test]
fn physical_container_rejects_invalid_gaps_and_features_before_suppression() {
    let node = ViewStyleNodeKey::new(ViewMountId::from_raw(1), vec![2], 2);
    let cross_axis_gap = resolve(vec![length(ViewPropertyKind::RowGap, 3, 1)], None).unwrap();
    assert_eq!(
        cross_axis_gap.physical_container(&node, ViewElementKind::Row),
        Err(ViewGeometryError::CrossAxisGapRequiresWrap {
            node: node.clone(),
            flow: ViewPhysicalFlow::Row,
            property: ViewPropertyKind::RowGap,
            value_milli: 3,
        })
    );

    let overlay_gap = resolve(vec![length(ViewPropertyKind::ColumnGap, 4, 1)], None).unwrap();
    assert_eq!(
        overlay_gap.physical_container(&node, ViewElementKind::Stack),
        Err(ViewGeometryError::GapRequiresLinearFlow {
            node: node.clone(),
            flow: ViewPhysicalFlow::Overlay,
            property: ViewPropertyKind::ColumnGap,
            value_milli: 4,
        })
    );

    let negative_gap = resolve(vec![length(ViewPropertyKind::RowGap, -1, 1)], None).unwrap();
    assert_eq!(
        negative_gap.physical_container(&node, ViewElementKind::Column),
        Err(ViewGeometryError::NegativeNonNegativeField {
            node: node.clone(),
            field: ViewGeometryField::RowGap,
            value_milli: -1,
        })
    );

    let represented_before_suppression = resolve(
        vec![
            declaration(
                ViewPropertyKind::Display,
                ViewSpecifiedValue::Display {
                    value: ViewDisplay::None,
                },
                1,
            ),
            declaration(
                ViewPropertyKind::FlexWrap,
                ViewSpecifiedValue::FlexWrap {
                    value: ViewFlexWrap::Wrap,
                },
                2,
            ),
        ],
        None,
    )
    .unwrap();
    assert_eq!(
        represented_before_suppression.physical_container(&node, ViewElementKind::Panel),
        Err(ViewGeometryError::UnsupportedConsumer {
            node,
            consumer: ViewGeometryConsumer::Layout,
            property: ViewPropertyKind::FlexWrap,
            feature: ViewRepresentedGeometryFeature::FlexWrap,
        })
    );
}

#[test]
fn child_uses_parent_axis_snapshot_without_reinterpreting_source() {
    let parent = resolve(
        vec![declaration(
            ViewPropertyKind::BoxAxes,
            ViewSpecifiedValue::BoxAxes {
                value: ViewBoxAxisMode::VerticalLr,
            },
            1,
        )],
        None,
    )
    .unwrap();
    let child = resolve(
        vec![length(ViewPropertyKind::InlineSize, 44, 1)],
        Some(&parent),
    )
    .unwrap();

    assert_eq!(child.axes().mode(), ViewBoxAxisMode::VerticalLr);
    assert!(matches!(
        child.axes().source(),
        ViewBoxAxisSource::Inherited { parent: revision }
            if *revision == parent.axes().revision()
    ));
    assert_eq!(
        child.physical_box().height.map(ViewLengthMilli::value),
        Some(44)
    );
    assert_eq!(child.physical_box().width, None);
}

#[test]
fn non_reversible_logical_translation_is_a_typed_resolver_error() {
    let source = ViewStyleSourceId::new(2);
    assert_eq!(
        ViewStyleDeclaration::new(
            ViewPropertyKind::TranslateInline,
            ViewSpecifiedValue::Length {
                value: ViewLengthMilli::new(i32::MIN),
            },
            ViewStyleAssignOp::Replace,
            source,
        ),
        Err(ViewStyleModelError::LogicalTranslationNotSignReversible {
            property: ViewPropertyKind::TranslateInline,
            style_source: source,
        })
    );

    let token_id = ViewStyleTokenId::try_new("axis.translation").unwrap();
    let token = ViewStyleToken::new(
        token_id.clone(),
        ViewStyleValueKind::Length,
        ViewSpecifiedValue::Length {
            value: ViewLengthMilli::new(i32::MIN),
        },
        ViewStyleSourceId::new(1),
    )
    .unwrap();
    let sheet = ViewStyleSheet::new(
        ViewStyleSheetId::try_new("style.axis.tokens").unwrap(),
        vec![token],
        Vec::new(),
    )
    .unwrap();
    let patch_id = ViewStylePatchId::new(1);
    let declaration = ViewStyleDeclaration::new(
        ViewPropertyKind::TranslateInline,
        ViewSpecifiedValue::Token {
            token: token_id,
            value_kind: ViewStyleValueKind::Length,
        },
        ViewStyleAssignOp::Replace,
        source,
    )
    .unwrap();
    let program = ViewStyleProgram::try_new(
        vec![sheet],
        vec![ViewStylePatch::new(patch_id, vec![declaration])],
    )
    .unwrap();
    let error = resolve_program(&program, patch_id, None).unwrap_err();

    assert!(matches!(
        error,
        ViewStyleResolveError::AxisValueOverflow {
            style_source,
            authored_property: ViewPropertyKind::TranslateInline,
            resolved_property,
            mode: ViewBoxAxisMode::HorizontalLtr,
        } if style_source == ViewStyleSourceId::new(2)
            && resolved_property.as_property() == ViewPropertyKind::TranslateX
    ));
}
