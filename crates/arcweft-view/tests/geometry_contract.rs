use arcweft_view::ViewMountId;
use arcweft_view::geometry::{
    ViewAvailableGeometrySize, ViewContainingBlockDependency, ViewFinalGeometryKey,
    ViewGeometryClip, ViewGeometryClipAxis, ViewGeometryConsumer, ViewGeometryError,
    ViewGeometryField, ViewGeometryMeasureStyleRevision, ViewGeometryOperation,
    ViewGeometryPlaceStyleRevision, ViewGeometryPoint, ViewGeometryRect, ViewGeometrySize,
    ViewGeometrySpan, ViewGeometryTransform, ViewIntrinsicMeasure, ViewIntrinsicMeasureRevision,
    ViewMeasuredGeometryKey, ViewPaintOutsets, ViewPlacedGeometryKey, ViewPlacedGeometryRevision,
    ViewPointerCoordinateErrorKind, ViewScrollCapability, ViewScrollStateInput,
    ViewScrollStateRevision, ViewViewportGeometryInput, ViewViewportGeometryRevision,
    consumer_geometry, first_flow_border_start, first_reverse_flow_border_start,
    flow_intrinsic_size, measure_box, milli_from_logical_pointer, next_flow_border_start,
    next_reverse_flow_border_start, outer_size, place_box, scroll_axis_geometry,
    scroll_into_view_nearest, transform_chain, transform_rect, validate_supported_properties,
};
use arcweft_view::style::{
    ViewBoxAxisMode, ViewLengthMilli, ViewOverflow, ViewPhysicalAxis, ViewPhysicalBoxStyle,
    ViewPhysicalContainerStyle, ViewPhysicalEdges, ViewPhysicalFlow, ViewPosition,
    ViewPropertyKind, ViewScalarMilli, ViewStyleNodeKey,
};

fn node(instruction: u32) -> ViewStyleNodeKey {
    ViewStyleNodeKey::new(ViewMountId::from_raw(7), vec![2, 5], instruction)
}

fn intrinsic(width_milli: u32, height_milli: u32) -> ViewIntrinsicMeasure {
    ViewIntrinsicMeasure {
        content_size: ViewGeometrySize::new(width_milli, height_milli),
        revision: ViewIntrinsicMeasureRevision::new(1),
    }
}

#[test]
fn bx_001_to_017_border_box_measurement_is_checked() {
    let style = ViewPhysicalBoxStyle {
        min_width: Some(ViewLengthMilli::new(60_000)),
        max_height: Some(ViewLengthMilli::new(20_000)),
        padding: ViewPhysicalEdges::new(
            ViewLengthMilli::new(2_000),
            ViewLengthMilli::new(5_000),
            ViewLengthMilli::new(3_000),
            ViewLengthMilli::new(7_000),
        ),
        border: ViewPhysicalEdges::all(ViewLengthMilli::new(1_000)),
        ..ViewPhysicalBoxStyle::default()
    };
    let measured = measure_box(&node(1), &style, intrinsic(40_000, 10_000)).unwrap();
    assert_eq!(
        measured.border_size(),
        ViewGeometrySize::new(60_000, 17_000)
    );
    assert_eq!(measured.content_size, ViewGeometrySize::new(46_000, 10_000));
    assert_eq!(measured.x.natural_border_extent_milli, 54_000);

    let invalid = ViewPhysicalBoxStyle {
        width: Some(ViewLengthMilli::new(30_000)),
        padding: ViewPhysicalEdges::new(
            ViewLengthMilli::new(0),
            ViewLengthMilli::new(20_000),
            ViewLengthMilli::new(0),
            ViewLengthMilli::new(20_000),
        ),
        border: ViewPhysicalEdges::all(ViewLengthMilli::new(1_000)),
        ..ViewPhysicalBoxStyle::default()
    };
    assert_eq!(
        measure_box(&node(2), &invalid, intrinsic(0, 0)),
        Err(ViewGeometryError::EdgesExceedUsedBorderBox {
            node: node(2),
            axis: ViewPhysicalAxis::X,
            used_milli: 30_000,
            edges_milli: 42_000,
        })
    );
}

#[test]
fn bx_016_explicit_zero_obeys_edge_fit_before_minimum_lifting() {
    let edges_and_min = ViewPhysicalBoxStyle {
        width: Some(ViewLengthMilli::new(0)),
        min_width: Some(ViewLengthMilli::new(50)),
        padding: ViewPhysicalEdges::new(
            ViewLengthMilli::new(0),
            ViewLengthMilli::new(7),
            ViewLengthMilli::new(0),
            ViewLengthMilli::new(5),
        ),
        ..ViewPhysicalBoxStyle::default()
    };
    assert_eq!(
        measure_box(&node(20), &edges_and_min, intrinsic(0, 0)),
        Err(ViewGeometryError::EdgesExceedUsedBorderBox {
            node: node(20),
            axis: ViewPhysicalAxis::X,
            used_milli: 0,
            edges_milli: 12,
        })
    );

    let explicit_zero = ViewPhysicalBoxStyle {
        width: Some(ViewLengthMilli::new(0)),
        ..ViewPhysicalBoxStyle::default()
    };
    let empty = measure_box(&node(21), &explicit_zero, intrinsic(0, 0)).unwrap();
    assert_eq!(empty.x.used_border_extent_milli, 0);
    assert_eq!(empty.content_size.width_milli, 0);

    let lifted = measure_box(
        &node(22),
        &ViewPhysicalBoxStyle {
            min_width: Some(ViewLengthMilli::new(50)),
            ..explicit_zero
        },
        intrinsic(0, 0),
    )
    .unwrap();
    assert_eq!(lifted.x.used_border_extent_milli, 50);
    assert_eq!(lifted.content_size.width_milli, 50);

    let auto_edges = measure_box(
        &node(23),
        &ViewPhysicalBoxStyle {
            padding: ViewPhysicalEdges::new(
                ViewLengthMilli::new(0),
                ViewLengthMilli::new(7),
                ViewLengthMilli::new(0),
                ViewLengthMilli::new(5),
            ),
            ..ViewPhysicalBoxStyle::default()
        },
        intrinsic(0, 0),
    )
    .unwrap();
    assert_eq!(auto_edges.x.used_border_extent_milli, 12);
    assert_eq!(auto_edges.content_size.width_milli, 0);
}

#[test]
fn neg_001_to_006_non_negative_fields_reject_negative_values() {
    let padding = ViewPhysicalBoxStyle {
        padding: ViewPhysicalEdges::new(
            ViewLengthMilli::new(-1),
            ViewLengthMilli::new(0),
            ViewLengthMilli::new(0),
            ViewLengthMilli::new(0),
        ),
        ..ViewPhysicalBoxStyle::default()
    };
    assert_eq!(
        measure_box(&node(3), &padding, intrinsic(0, 0)),
        Err(ViewGeometryError::NegativeNonNegativeField {
            node: node(3),
            field: ViewGeometryField::PaddingTop,
            value_milli: -1,
        })
    );

    let gap = ViewPhysicalContainerStyle {
        flow: ViewPhysicalFlow::Row,
        row_gap: ViewLengthMilli::new(0),
        column_gap: ViewLengthMilli::new(-1),
    };
    assert_eq!(
        flow_intrinsic_size(&node(3), gap, &[]),
        Err(ViewGeometryError::NegativeNonNegativeField {
            node: node(3),
            field: ViewGeometryField::ColumnGap,
            value_milli: -1,
        })
    );
}

#[test]
fn bx_012_conflicting_constraints_identify_the_axis() {
    let style = ViewPhysicalBoxStyle {
        min_width: Some(ViewLengthMilli::new(21)),
        max_width: Some(ViewLengthMilli::new(20)),
        ..ViewPhysicalBoxStyle::default()
    };
    assert_eq!(
        measure_box(&node(4), &style, intrinsic(0, 0)),
        Err(ViewGeometryError::ConflictingConstraints {
            node: node(4),
            axis: ViewPhysicalAxis::X,
            min_milli: 21,
            max_milli: 20,
        })
    );
}

#[test]
fn flow_003_to_006_margins_never_collapse_and_gap_is_additive() {
    assert_eq!(
        first_flow_border_start(&node(5), ViewPhysicalAxis::X, 0, 2_000).unwrap(),
        2_000
    );
    assert_eq!(
        next_flow_border_start(&node(5), ViewPhysicalAxis::X, 22_000, 3_000, 5_000, -4_000,)
            .unwrap(),
        26_000
    );
    assert_eq!(
        first_reverse_flow_border_start(&node(5), ViewPhysicalAxis::X, 100, 3, 20).unwrap(),
        77
    );
    assert_eq!(
        next_reverse_flow_border_start(&node(5), ViewPhysicalAxis::X, 77, 2, 5, -4, 30).unwrap(),
        44
    );
}

#[test]
fn neg_007_to_008_signed_margins_may_shrink_but_not_invert() {
    let style = ViewPhysicalBoxStyle {
        width: Some(ViewLengthMilli::new(20)),
        height: Some(ViewLengthMilli::new(10)),
        margin: ViewPhysicalEdges::new(
            ViewLengthMilli::new(3),
            ViewLengthMilli::new(6),
            ViewLengthMilli::new(5),
            ViewLengthMilli::new(-4),
        ),
        ..ViewPhysicalBoxStyle::default()
    };
    let measured = measure_box(&node(6), &style, intrinsic(0, 0)).unwrap();
    let outer = outer_size(&node(6), measured).unwrap();
    assert_eq!(outer.width_milli, 22);
    assert_eq!(outer.height_milli, 18);

    let inverted = ViewPhysicalBoxStyle {
        width: Some(ViewLengthMilli::new(10)),
        height: Some(ViewLengthMilli::new(10)),
        margin: ViewPhysicalEdges::new(
            ViewLengthMilli::new(0),
            ViewLengthMilli::new(-8),
            ViewLengthMilli::new(0),
            ViewLengthMilli::new(-8),
        ),
        ..ViewPhysicalBoxStyle::default()
    };
    let measured = measure_box(&node(7), &inverted, intrinsic(0, 0)).unwrap();
    assert!(matches!(
        outer_size(&node(7), measured),
        Err(ViewGeometryError::InvertedMarginSpan {
            axis: ViewPhysicalAxis::X,
            ..
        })
    ));
}

#[test]
fn pos_002_static_insets_are_not_ignored() {
    let style = ViewPhysicalBoxStyle {
        width: Some(ViewLengthMilli::new(10)),
        height: Some(ViewLengthMilli::new(10)),
        inset: ViewPhysicalEdges::new(None, None, None, Some(ViewLengthMilli::new(1))),
        ..ViewPhysicalBoxStyle::default()
    };
    let measured = measure_box(&node(8), &style, intrinsic(0, 0)).unwrap();
    assert_eq!(
        place_box(
            &node(8),
            &style,
            measured,
            ViewGeometryRect::new(0, 0, 100, 100).unwrap(),
            ViewGeometryPoint::new(0, 0),
        ),
        Err(ViewGeometryError::InsetOnStatic {
            node: node(8),
            axis: ViewPhysicalAxis::X,
        })
    );
}

#[test]
fn pos_004_to_010_relative_insets_move_without_changing_static_allocation() {
    let style = ViewPhysicalBoxStyle {
        position: ViewPosition::Relative,
        width: Some(ViewLengthMilli::new(20)),
        height: Some(ViewLengthMilli::new(10)),
        inset: ViewPhysicalEdges::new(
            Some(ViewLengthMilli::new(-2)),
            None,
            None,
            Some(ViewLengthMilli::new(8)),
        ),
        ..ViewPhysicalBoxStyle::default()
    };
    let measured = measure_box(&node(9), &style, intrinsic(0, 0)).unwrap();
    let placed = place_box(
        &node(9),
        &style,
        measured,
        ViewGeometryRect::new(0, 0, 500, 500).unwrap(),
        ViewGeometryPoint::new(100, 50),
    )
    .unwrap();
    assert_eq!(
        placed.border_box,
        ViewGeometryRect::new(108, 48, 128, 58).unwrap()
    );
}

#[test]
fn pos_015_absolute_auto_size_stretches_between_both_insets() {
    let style = ViewPhysicalBoxStyle {
        position: ViewPosition::Absolute,
        height: Some(ViewLengthMilli::new(10)),
        inset: ViewPhysicalEdges::new(
            Some(ViewLengthMilli::new(5)),
            Some(ViewLengthMilli::new(20)),
            None,
            Some(ViewLengthMilli::new(10)),
        ),
        padding: ViewPhysicalEdges::new(
            ViewLengthMilli::new(0),
            ViewLengthMilli::new(5),
            ViewLengthMilli::new(0),
            ViewLengthMilli::new(5),
        ),
        border: ViewPhysicalEdges::all(ViewLengthMilli::new(1)),
        ..ViewPhysicalBoxStyle::default()
    };
    let measured = measure_box(&node(10), &style, intrinsic(10, 0)).unwrap();
    let placed = place_box(
        &node(10),
        &style,
        measured,
        ViewGeometryRect::new(10, 20, 210, 120).unwrap(),
        ViewGeometryPoint::new(0, 0),
    )
    .unwrap();
    assert_eq!(
        placed.border_box,
        ViewGeometryRect::new(20, 25, 190, 35).unwrap()
    );
    assert_eq!(placed.content_box.size(), ViewGeometrySize::new(158, 8));
}

#[test]
fn pos_017_definite_size_with_both_insets_is_typed_error() {
    let style = ViewPhysicalBoxStyle {
        position: ViewPosition::Absolute,
        width: Some(ViewLengthMilli::new(50)),
        height: Some(ViewLengthMilli::new(10)),
        inset: ViewPhysicalEdges::new(
            None,
            Some(ViewLengthMilli::new(20)),
            None,
            Some(ViewLengthMilli::new(10)),
        ),
        ..ViewPhysicalBoxStyle::default()
    };
    let measured = measure_box(&node(11), &style, intrinsic(0, 0)).unwrap();
    assert!(matches!(
        place_box(
            &node(11),
            &style,
            measured,
            ViewGeometryRect::new(0, 0, 200, 100).unwrap(),
            ViewGeometryPoint::new(0, 0),
        ),
        Err(ViewGeometryError::OverConstrainedPositionedAxis {
            axis: ViewPhysicalAxis::X,
            ..
        })
    ));
}

#[test]
fn xfm_001_to_011_scale_rounds_outward_after_translation() {
    let rect = ViewGeometryRect::new(10, 0, 15, 10).unwrap();
    let transformed = transform_rect(
        &node(12),
        rect,
        ViewGeometryTransform {
            border_box: rect,
            translate: ViewGeometryPoint::new(0, 0),
            scale: ViewScalarMilli::new(1_500),
        },
    )
    .unwrap();
    assert_eq!(transformed, ViewGeometryRect::new(8, -3, 17, 13).unwrap());

    let zero = transform_rect(
        &node(12),
        rect,
        ViewGeometryTransform {
            border_box: rect,
            translate: ViewGeometryPoint::new(10, -10),
            scale: ViewScalarMilli::ZERO,
        },
    )
    .unwrap();
    assert_eq!(zero, ViewGeometryRect::new(22, -5, 22, -5).unwrap());
}

#[test]
fn xfm_007_to_009_transform_chain_is_inner_to_outer() {
    let rect = ViewGeometryRect::new(0, 0, 10, 10).unwrap();
    let transforms = [
        ViewGeometryTransform {
            border_box: rect,
            translate: ViewGeometryPoint::new(10, 0),
            scale: ViewScalarMilli::ONE,
        },
        ViewGeometryTransform {
            border_box: ViewGeometryRect::new(0, 0, 20, 20).unwrap(),
            translate: ViewGeometryPoint::new(0, 0),
            scale: ViewScalarMilli::new(2_000),
        },
    ];
    assert_eq!(
        transform_chain(&node(13), rect, &transforms).unwrap(),
        ViewGeometryRect::new(10, -10, 30, 10).unwrap()
    );
}

#[test]
fn clip_002_to_003_overflow_axes_are_independent() {
    let viewport = ViewGeometryRect::new(0, 0, 320, 180).unwrap();
    let padding = ViewGeometryRect::new(24, 32, 144, 80).unwrap();
    let child = ViewGeometryRect::new(-10, -10, 200, 200).unwrap();
    let clip = ViewGeometryClip::from_rect(viewport).with_overflow(
        padding,
        ViewOverflow::Hidden,
        ViewOverflow::Visible,
    );
    assert_eq!(
        clip.clip_rect(child),
        Some(ViewGeometryRect::new(24, 0, 144, 180).unwrap())
    );
}

#[test]
fn con_001_to_009_consumers_share_one_visible_border_box() {
    let viewport = ViewGeometryRect::new(0, 0, 100, 100).unwrap();
    let border = ViewGeometryRect::new(-10, 20, 80, 120).unwrap();
    let paint = ViewGeometryRect::new(-15, 15, 85, 125).unwrap();
    let consumers = consumer_geometry(border, paint, ViewGeometryClip::from_rect(viewport));
    let visible = Some(ViewGeometryRect::new(0, 20, 80, 100).unwrap());
    assert_eq!(consumers.visible_border_box, visible);
    assert_eq!(consumers.hit_bounds, visible);
    assert_eq!(consumers.focus_target_bounds, visible);
    assert_eq!(consumers.avoidance_bounds, visible);
    assert_eq!(consumers.scroll_target_bounds, visible);
    assert_eq!(
        consumers.paint_bounds,
        Some(ViewGeometryRect::new(0, 15, 85, 100).unwrap())
    );
}

#[test]
fn scr_002_to_018_scroll_range_preserves_leading_and_trailing_overflow() {
    let geometry = scroll_axis_geometry(
        &node(14),
        ViewPhysicalAxis::X,
        ViewOverflow::Auto,
        ViewGeometrySpan::new(0, 100).unwrap(),
        ViewGeometrySpan::new(-20, 150).unwrap(),
        0,
    )
    .unwrap();
    assert_eq!(geometry.min_offset_milli, -20);
    assert_eq!(geometry.max_offset_milli, 50);
    assert_eq!(
        geometry.capability,
        ViewScrollCapability::UserAndProgrammatic
    );
    assert_eq!(
        scroll_into_view_nearest(
            &node(14),
            ViewPhysicalAxis::X,
            geometry,
            ViewGeometrySpan::new(120, 140).unwrap(),
        )
        .unwrap(),
        40
    );
}

#[test]
fn num_003_coordinate_overflow_is_not_saturated() {
    assert_eq!(
        ViewGeometryRect::from_origin_size(
            &node(15),
            ViewGeometryPoint::new(i32::MAX, 0),
            ViewGeometrySize::new(1, 0),
        ),
        Err(ViewGeometryError::ArithmeticOverflow {
            node: node(15),
            axis: Some(ViewPhysicalAxis::X),
            operation: ViewGeometryOperation::Add,
        })
    );
}

#[test]
fn num_014_edge_touch_is_not_visible() {
    let left = ViewGeometryRect::new(0, 0, 10, 10).unwrap();
    let right = ViewGeometryRect::new(10, 0, 20, 10).unwrap();
    assert_eq!(left.intersection(right), None);
}

#[test]
fn clip_empty_bounded_unbounded_and_intersection_are_closed() {
    let zero = ViewGeometrySpan::new(10, 10).unwrap();
    assert_eq!(
        ViewGeometryClip::from_axes(
            ViewGeometryClipAxis::bounded(zero),
            ViewGeometryClipAxis::unbounded(),
        ),
        ViewGeometryClip::Empty
    );

    let left = ViewGeometryClip::from_rect(ViewGeometryRect::new(0, 0, 10, 10).unwrap());
    let edge_touch = ViewGeometryClip::from_rect(ViewGeometryRect::new(10, 0, 20, 10).unwrap());
    assert_eq!(left.intersect(edge_touch), ViewGeometryClip::Empty);
    assert_eq!(left.intersect(ViewGeometryClip::unbounded()), left);

    let mixed = ViewGeometryClip::from_axes(
        ViewGeometryClipAxis::bounded(ViewGeometrySpan::new(2, 8).unwrap()),
        ViewGeometryClipAxis::unbounded(),
    );
    let axes = mixed.axes().expect("mixed clip remains non-empty");
    assert_eq!(
        axes.x(),
        ViewGeometryClipAxis::bounded(ViewGeometrySpan::new(2, 8).unwrap())
    );
    assert_eq!(axes.y(), ViewGeometryClipAxis::unbounded());
    assert_eq!(
        mixed.clip_rect(ViewGeometryRect::new(0, -5, 10, 5).unwrap()),
        Some(ViewGeometryRect::new(2, -5, 8, 5).unwrap())
    );
}

#[test]
fn cap_007_raster_rounding_is_outward_for_negative_and_positive_edges() {
    assert_eq!(
        ViewGeometryRect::new(-1, -1_001, 1, 1_001)
            .unwrap()
            .outward_raster_rect(),
        arcweft_view::geometry::ViewGeometryRasterRect {
            left_px: -1,
            top_px: -2,
            right_px: 1,
            bottom_px: 2,
        }
    );
}

#[test]
fn con_pointer_ingress_floors_to_milli_and_rejects_non_finite_values() {
    assert_eq!(milli_from_logical_pointer(-0.0001).unwrap(), -1);
    assert_eq!(
        milli_from_logical_pointer(f64::NAN),
        Err(ViewGeometryError::InvalidPointerCoordinate {
            value_bits: f64::NAN.to_bits(),
            kind: ViewPointerCoordinateErrorKind::NonFinite,
        })
    );
    assert_eq!(
        milli_from_logical_pointer(f64::MAX),
        Err(ViewGeometryError::InvalidPointerCoordinate {
            value_bits: f64::MAX.to_bits(),
            kind: ViewPointerCoordinateErrorKind::OutsideMilliRange,
        })
    );
}

#[test]
fn axis_001_to_011_axis_metadata_does_not_change_physical_geometry_or_revision() {
    let baseline = ViewPhysicalBoxStyle {
        axes: ViewBoxAxisMode::HorizontalLtr,
        width: Some(ViewLengthMilli::new(50)),
        height: Some(ViewLengthMilli::new(20)),
        padding: ViewPhysicalEdges::new(
            ViewLengthMilli::new(1),
            ViewLengthMilli::new(2),
            ViewLengthMilli::new(3),
            ViewLengthMilli::new(4),
        ),
        ..ViewPhysicalBoxStyle::default()
    };
    let expected = measure_box(&node(16), &baseline, intrinsic(10, 5)).unwrap();
    for axes in [
        ViewBoxAxisMode::HorizontalRtl,
        ViewBoxAxisMode::VerticalRl,
        ViewBoxAxisMode::VerticalLr,
    ] {
        let current = measure_box(
            &node(16),
            &ViewPhysicalBoxStyle { axes, ..baseline },
            intrinsic(10, 5),
        )
        .unwrap();
        assert_eq!(current.border_size(), expected.border_size());
        assert_eq!(current.content_size, expected.content_size);
        assert_eq!(current.revision, expected.revision);
    }
}

#[test]
fn cache_002_to_016_revision_domains_include_path_order_and_exact_dependencies() {
    let style = ViewPhysicalBoxStyle::default();
    let intrinsic = intrinsic(10, 5);
    let measured_a = measure_box(&node(17), &style, intrinsic).unwrap();
    let path_variant = ViewStyleNodeKey::new(ViewMountId::from_raw(7), vec![2, 6], 17);
    let measured_b = measure_box(&path_variant, &style, intrinsic).unwrap();
    assert_ne!(measured_a.revision, measured_b.revision);

    let measured_key = ViewMeasuredGeometryKey {
        node: node(17),
        box_style: style,
        container_style: None,
        intrinsic,
        available: ViewAvailableGeometrySize::default(),
        ordered_children: Vec::new(),
    };
    let mut path_key = measured_key.clone();
    path_key.node = path_variant;
    assert_ne!(measured_key, path_key);
    assert_ne!(measured_key.revision(), path_key.revision());

    let viewport = ViewViewportGeometryRevision::new(9);
    let root = ViewPlacedGeometryRevision::for_root_viewport(viewport);
    let viewport_rect = ViewGeometryRect::new(0, 0, 100, 100).unwrap();
    let placement = place_box(
        &node(17),
        &style,
        measured_a,
        viewport_rect,
        ViewGeometryPoint::new(0, 0),
    )
    .unwrap();
    let scroll = ViewScrollStateInput {
        x_milli: 0,
        y_milli: 0,
        revision: ViewScrollStateRevision::new(0),
    };
    let placed_key = ViewPlacedGeometryKey {
        node: node(17),
        measured: measured_a,
        box_style: style,
        containing_block: ViewContainingBlockDependency {
            node: None,
            rect: viewport_rect,
            revision: root,
        },
        static_border_origin: ViewGeometryPoint::new(0, 0),
        parent: None,
        previous_flow_sibling: None,
        viewport: ViewViewportGeometryInput {
            rect: viewport_rect,
            revision: viewport,
        },
        scroll,
    };
    let placed = placed_key.revision();
    let final_key = ViewFinalGeometryKey {
        node: node(17),
        placement,
        box_style: style,
        transform_chain: Vec::new(),
        inherited_clip: ViewGeometryClip::from_rect(viewport_rect),
        paint_outsets: ViewPaintOutsets::default(),
        scroll,
        ordered_children: Vec::new(),
    };
    let final_revision = final_key.revision();
    assert_ne!(measured_key.revision().value(), 0);
    assert_ne!(placed.value(), 0);
    assert_ne!(final_revision.value(), 0);
    assert_ne!(placed.value(), final_revision.value());
    assert_ne!(
        ViewGeometryMeasureStyleRevision::for_style(&style, None).value(),
        ViewGeometryPlaceStyleRevision::for_style(&style).value()
    );
}

#[test]
fn flow_010_to_013_physical_gaps_apply_only_between_children() {
    let children = [
        arcweft_view::geometry::ViewOuterSize {
            width_milli: 20,
            height_milli: 10,
        },
        arcweft_view::geometry::ViewOuterSize {
            width_milli: 30,
            height_milli: 15,
        },
    ];
    assert_eq!(
        flow_intrinsic_size(
            &node(18),
            ViewPhysicalContainerStyle {
                flow: ViewPhysicalFlow::Row,
                row_gap: ViewLengthMilli::new(7),
                column_gap: ViewLengthMilli::new(5),
            },
            &children,
        )
        .unwrap(),
        ViewGeometrySize::new(55, 15)
    );
}

#[test]
fn unsupported_represented_geometry_is_never_silently_dropped() {
    assert_eq!(
        validate_supported_properties(
            &node(19),
            ViewGeometryConsumer::Layout,
            &[ViewPropertyKind::Width, ViewPropertyKind::FlexGrow],
        ),
        Err(ViewGeometryError::UnsupportedConsumer {
            node: node(19),
            consumer: ViewGeometryConsumer::Layout,
            property: ViewPropertyKind::FlexGrow,
            feature: arcweft_view::geometry::ViewRepresentedGeometryFeature::FlexDistribution,
        })
    );
}

#[test]
fn paint_effect_bounds_do_not_block_layout_but_remain_fail_closed_for_paint() {
    let paint_effects = [
        ViewPropertyKind::BoxShadow,
        ViewPropertyKind::Filter,
        ViewPropertyKind::BackdropFilter,
    ];
    for consumer in [
        ViewGeometryConsumer::Measure,
        ViewGeometryConsumer::Layout,
        ViewGeometryConsumer::Clip,
        ViewGeometryConsumer::HitTest,
        ViewGeometryConsumer::Focus,
        ViewGeometryConsumer::Avoidance,
        ViewGeometryConsumer::Scroll,
        ViewGeometryConsumer::Capture,
    ] {
        assert_eq!(
            validate_supported_properties(&node(20), consumer, &paint_effects),
            Ok(())
        );
    }
    assert_eq!(
        validate_supported_properties(
            &node(21),
            ViewGeometryConsumer::Paint,
            &[ViewPropertyKind::BoxShadow],
        ),
        Err(ViewGeometryError::UnsupportedConsumer {
            node: node(21),
            consumer: ViewGeometryConsumer::Paint,
            property: ViewPropertyKind::BoxShadow,
            feature: arcweft_view::geometry::ViewRepresentedGeometryFeature::PaintEffectBounds,
        })
    );
}
