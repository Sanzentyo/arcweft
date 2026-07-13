//! Lowers structural Scroll layout and policy authoring.

use super::{
    Expr, Literal, VIEW_LAYOUT_SCROLL_VIEWPORT_HEIGHT_MILLI, VIEW_LAYOUT_TEXT_CONTROL_WIDTH_MILLI,
    ViewArg, ViewElement, ViewFocusAutoScrollPolicy, ViewLayoutCursor, ViewLayoutFrame,
    ViewLogicalRect, ViewLoweringState, ViewModifier, ViewScrollAxis, ViewScrollIndicatorsPolicy,
    ViewScrollOverflowPolicy, ViewScrollOverscrollPolicy, ViewScrollRegionResource,
    ViewSidecarError, expr_source, lower_layout_column, named_arg, named_layout_length_u32,
    normalize_entity_ref, view_resource_id,
};

pub(super) fn lower_scroll_region(
    view_id: &str,
    element: &ViewElement,
    state: &mut ViewLoweringState,
    origin: ViewLayoutCursor,
    open_instruction: usize,
) -> Result<ViewLayoutFrame, ViewSidecarError> {
    let options = scroll_region_options(view_id, element, state.scroll_counter)?;
    let scroll_id = options.public_id.clone();
    if let Some(super::ViewProgramInstruction::OpenElement { target, .. }) =
        state.instructions.get_mut(open_instruction)
    {
        *target = Some(scroll_id.clone());
    }
    state.scroll_counter = state.scroll_counter.saturating_add(1);
    state.scroll_stack.push(scroll_id.clone());
    let content_frame = lower_layout_column(view_id, element.children(), state, origin)?;
    state.scroll_stack.pop();
    let width_milli = options.width_milli.unwrap_or_else(|| {
        content_frame
            .width_milli
            .max(VIEW_LAYOUT_TEXT_CONTROL_WIDTH_MILLI)
    });
    let viewport_height_milli = options.height_milli.unwrap_or_else(|| {
        content_frame
            .height_milli
            .clamp(1, VIEW_LAYOUT_SCROLL_VIEWPORT_HEIGHT_MILLI)
    });
    let content_width_milli = content_frame.width_milli.max(width_milli);
    let content_height_milli = content_frame.height_milli.max(viewport_height_milli);
    state.scroll_regions.push(
        ViewScrollRegionResource::new(
            scroll_id,
            Some(view_resource_id(view_id)),
            ViewLogicalRect::new(
                origin.x_milli,
                origin.y_milli,
                width_milli,
                viewport_height_milli,
            ),
            content_width_milli,
            content_height_milli,
            options.axis,
        )
        .with_overflow(options.overflow)
        .with_indicators(options.indicators)
        .with_overscroll(options.overscroll)
        .with_auto_scroll_focus(options.auto_scroll_focus),
    );
    Ok(ViewLayoutFrame::new(width_milli, viewport_height_milli))
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ScrollRegionOptions {
    public_id: String,
    width_milli: Option<u32>,
    height_milli: Option<u32>,
    axis: ViewScrollAxis,
    overflow: ViewScrollOverflowPolicy,
    indicators: ViewScrollIndicatorsPolicy,
    overscroll: ViewScrollOverscrollPolicy,
    auto_scroll_focus: ViewFocusAutoScrollPolicy,
}

fn scroll_region_options(
    view_id: &str,
    element: &ViewElement,
    fallback_index: u32,
) -> Result<ScrollRegionOptions, ViewSidecarError> {
    let mut options = ScrollRegionOptions {
        public_id: scroll_region_public_id(view_id, element, fallback_index),
        width_milli: None,
        height_milli: None,
        axis: ViewScrollAxis::Vertical,
        overflow: ViewScrollOverflowPolicy::default(),
        indicators: ViewScrollIndicatorsPolicy::default(),
        overscroll: ViewScrollOverscrollPolicy::default(),
        auto_scroll_focus: ViewFocusAutoScrollPolicy::default(),
    };
    reject_dual_axis_scroll_authoring(element)?;
    if let Some(width) = named_layout_length_u32(element.args(), &["width", "w"]) {
        options.width_milli = Some(width);
    }
    if let Some(height) = named_layout_length_u32(element.args(), &["height", "h"]) {
        options.height_milli = Some(height);
    }
    if let Some(overflow) = scroll_overflow_arg(element.args()) {
        options.overflow = overflow;
    }
    if let Some(axis) = scroll_axis_arg(element.args()) {
        options.axis = axis;
    }
    if let Some(indicators) = scroll_indicators_arg(element.args()) {
        options.indicators = indicators;
    }
    if let Some(overscroll) = scroll_overscroll_arg(element.args()) {
        options.overscroll = overscroll;
    }
    if let Some(policy) = scroll_auto_focus_arg(element.args()) {
        options.auto_scroll_focus = policy;
    }
    apply_scroll_property_modifiers(&mut options, element.modifiers());
    Ok(options)
}

fn reject_dual_axis_scroll_authoring(element: &ViewElement) -> Result<(), ViewSidecarError> {
    for arg in element.args() {
        let ViewArg::Named { name, value } = arg else {
            continue;
        };
        if normalize_property_name(name) == "axis"
            && ViewScrollAxis::is_unsupported_dual_axis_symbol(&expr_source(value))
        {
            return Err(ViewSidecarError::UnsupportedScrollBothAxis {
                element: element.callee().to_owned(),
                value: expr_source(value),
            });
        }
    }
    for modifier in element.modifiers() {
        match modifier {
            ViewModifier::Property { name, value }
                if normalize_property_name(name) == "axis"
                    && ViewScrollAxis::is_unsupported_dual_axis_symbol(&expr_source(value)) =>
            {
                return Err(ViewSidecarError::UnsupportedScrollBothAxis {
                    element: element.callee().to_owned(),
                    value: expr_source(value),
                });
            }
            _ => {}
        }
    }
    Ok(())
}

fn scroll_region_public_id(view_id: &str, element: &ViewElement, fallback_index: u32) -> String {
    named_arg(element.args(), "id")
        .and_then(scroll_id_expr)
        .unwrap_or_else(|| format!("scroll.{view_id}.{fallback_index}"))
}

fn scroll_id_expr(expr: &Expr) -> Option<String> {
    match expr {
        Expr::EntityRef(reference) => Some(normalize_scroll_id(&normalize_entity_ref(reference))),
        Expr::Literal(Literal::String(value)) | Expr::Raw(value) => {
            Some(normalize_scroll_id(value))
        }
        Expr::Path(value) => Some(normalize_scroll_id(value.as_label())),
        _ => None,
    }
}

fn normalize_scroll_id(value: &str) -> String {
    let value = value.trim().strip_prefix('@').unwrap_or(value.trim());
    let value = value.strip_prefix("scroll:.").unwrap_or(value);
    if value.starts_with("scroll.") {
        value.to_owned()
    } else {
        format!("scroll.{value}")
    }
}

fn scroll_overflow_arg(args: &[ViewArg]) -> Option<ViewScrollOverflowPolicy> {
    named_arg(args, "overflow")
        .or_else(|| named_arg(args, "overflow_y"))
        .or_else(|| named_arg(args, "overflow-y"))
        .and_then(scroll_overflow_expr)
        .or_else(|| {
            named_arg(args, "clip").and_then(expr_bool).map(|clip| {
                if clip {
                    ViewScrollOverflowPolicy::Auto
                } else {
                    ViewScrollOverflowPolicy::Hidden
                }
            })
        })
}

fn scroll_axis_arg(args: &[ViewArg]) -> Option<ViewScrollAxis> {
    named_arg(args, "axis").and_then(scroll_axis_expr)
}

fn scroll_indicators_arg(args: &[ViewArg]) -> Option<ViewScrollIndicatorsPolicy> {
    named_arg(args, "indicators")
        .or_else(|| named_arg(args, "scroll-indicators"))
        .and_then(scroll_indicators_expr)
}

fn scroll_overscroll_arg(args: &[ViewArg]) -> Option<ViewScrollOverscrollPolicy> {
    named_arg(args, "overscroll")
        .or_else(|| named_arg(args, "overscroll-behavior"))
        .and_then(scroll_overscroll_expr)
}

fn scroll_auto_focus_arg(args: &[ViewArg]) -> Option<ViewFocusAutoScrollPolicy> {
    named_arg(args, "auto_scroll_focus")
        .or_else(|| named_arg(args, "auto-scroll-focus"))
        .or_else(|| named_arg(args, "auto-focus-scroll"))
        .and_then(scroll_auto_focus_expr)
}

fn apply_scroll_property_modifiers(options: &mut ScrollRegionOptions, modifiers: &[ViewModifier]) {
    for modifier in modifiers {
        if let ViewModifier::Property { name, value } = modifier {
            apply_scroll_property(options, name, &expr_source(value));
        }
    }
}

fn apply_scroll_property(options: &mut ScrollRegionOptions, name: &str, value: &str) {
    match normalize_property_name(name).as_str() {
        "axis" => {
            if let Some(axis) = scroll_axis_symbol(value) {
                options.axis = axis;
            }
        }
        "indicators" | "scroll-indicators" => {
            if let Some(indicators) = ViewScrollIndicatorsPolicy::from_author_symbol(value) {
                options.indicators = indicators;
            }
        }
        "overscroll" | "overscroll-behavior" => {
            if let Some(overscroll) = ViewScrollOverscrollPolicy::from_author_symbol(value) {
                options.overscroll = overscroll;
            }
        }
        "auto-scroll-focus" | "auto-focus-scroll" => {
            if let Some(policy) = ViewFocusAutoScrollPolicy::from_author_symbol(value) {
                options.auto_scroll_focus = policy;
            }
        }
        _ => {}
    }
}

fn normalize_property_name(value: &str) -> String {
    value.trim().replace('_', "-").to_ascii_lowercase()
}

fn scroll_overflow_expr(expr: &Expr) -> Option<ViewScrollOverflowPolicy> {
    scroll_overflow_symbol(&expr_source(expr))
}

fn scroll_axis_expr(expr: &Expr) -> Option<ViewScrollAxis> {
    scroll_axis_symbol(&expr_source(expr))
}

fn scroll_indicators_expr(expr: &Expr) -> Option<ViewScrollIndicatorsPolicy> {
    ViewScrollIndicatorsPolicy::from_author_symbol(&expr_source(expr))
}

fn scroll_overscroll_expr(expr: &Expr) -> Option<ViewScrollOverscrollPolicy> {
    ViewScrollOverscrollPolicy::from_author_symbol(&expr_source(expr))
}

fn scroll_auto_focus_expr(expr: &Expr) -> Option<ViewFocusAutoScrollPolicy> {
    ViewFocusAutoScrollPolicy::from_author_symbol(&expr_source(expr))
}

fn expr_bool(expr: &Expr) -> Option<bool> {
    match expr {
        Expr::Literal(Literal::Bool(value)) => Some(*value),
        Expr::Raw(value) => parse_bool_like(value),
        Expr::Path(value) => parse_bool_like(value.as_label()),
        _ => None,
    }
}

fn parse_bool_like(value: &str) -> Option<bool> {
    match value.trim().trim_matches('"') {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

fn scroll_overflow_symbol(value: &str) -> Option<ViewScrollOverflowPolicy> {
    match value.trim().trim_matches('"').trim_start_matches('.') {
        "auto" => Some(ViewScrollOverflowPolicy::Auto),
        "scroll" => Some(ViewScrollOverflowPolicy::Scroll),
        "hidden" | "clip" => Some(ViewScrollOverflowPolicy::Hidden),
        _ => None,
    }
}

fn scroll_axis_symbol(value: &str) -> Option<ViewScrollAxis> {
    ViewScrollAxis::from_author_symbol(value)
}
