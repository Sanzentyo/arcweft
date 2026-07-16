use arcweft_presentation::appearance::{ColorScheme, PresentationEnvironment};
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
    ViewDisplay, ViewElementKind, ViewMountId, ViewPhysicalFlow, ViewPosition, ViewScalarMilli,
};

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
    let environment = PresentationEnvironment::new(ColorScheme::Light);
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
        .map(arcweft_view::style::ViewStyleResolution::into_computed)
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
            length(ViewPropertyKind::Gap, 4, 5),
            length(ViewPropertyKind::RowGap, 6, 6),
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

    let container = computed
        .physical_container(ViewElementKind::Panel)
        .expect("flex container geometry is executable")
        .expect("display Flex retains a geometry container");
    assert_eq!(container.flow, ViewPhysicalFlow::Row);
    assert_eq!(container.row_gap.value(), 6);
    assert_eq!(container.column_gap.value(), 4);
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
        ViewStyleSheetId::try_new("axis.tokens").unwrap(),
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
