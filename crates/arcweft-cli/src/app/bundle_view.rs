use arcweft_bundle::{
    BundleImageObject, BundleImageObjectBounds,
    container::BundleDigest,
    resource_codec::{
        ViewActionButtonActionResource, ViewActionButtonResource, ViewActionPayloadResource,
        ViewActionTextControlPayloadField, ViewAwaitBranchSpan, ViewFocusDirection,
        ViewFocusGroupPolicy, ViewFocusGroupResource, ViewFocusInitialPolicy,
        ViewFocusNavigationEdge, ViewFocusNavigationResource, ViewFocusSkipPolicy,
        ViewFocusTargetResolution, ViewFocusWrapPolicy, ViewInputResource,
        ViewLayoutBoundsResource, ViewLogicalRect, ViewPartStyleRule, ViewProgramResource,
        ViewRuntimeButtonBounds, ViewRuntimeTextBlockBounds, ViewScrollAxis,
        ViewScrollIndicatorsPolicy, ViewScrollOverflowPolicy, ViewScrollOverscrollPolicy,
        ViewScrollRegionResource, ViewStyleResource, ViewSurfaceResource, ViewTextBlockResource,
        ViewTextResource,
        types::DigestRef,
        view::{
            CompositionOnBlurPolicy, EnterKeyHint, StyleAssignOp, StyleSourceIdentity,
            StyleSourceRef, StyleSyntax, TextAssistPolicy, TextCapitalization, ViewElementKind,
            ViewFocusAutoScrollPolicy, ViewInputKind, ViewInputOptions, ViewInputPurpose,
            ViewProgramInstruction, ViewSecureInputPolicy, ViewSemanticTarget, ViewStyleApplyRef,
            ViewStyleDeclaration, ViewStyleSelector, ViewStyleSelectorPart, ViewStyleValue,
            ViewTextSelectionPolicy, ViewTextShortcutPolicy, ViewTextSourceKind,
            ViewTextSourceRecord, ViewTextTabPolicy, ViewTextVerticalNavigationPolicy,
        },
    },
};
use arcweft_lang_syntax::{
    ast::{
        ids::{EntityRef, EntityRefSyntax},
        items::EntityDeclItem,
        pattern::Pattern,
        view::{
            ViewAction, ViewActionPayload, ViewArg, ViewAwait, ViewAwaitBranchKind, ViewBody,
            ViewButton, ViewButtonLabel, ViewElement, ViewExpr, ViewForEach, ViewIf, ViewImage,
            ViewLet, ViewMatch, ViewMatchArm, ViewModifier, ViewNavigationDirection,
            ViewNavigationInitial, ViewNavigationTarget, ViewNavigationTrap, ViewStyleModifier,
            ViewText, ViewTextControlPayloadField, ViewTextField, ViewTextFieldMode,
        },
    },
    expr::{CallArg, Expr, Literal},
};
use thiserror::Error;

use super::bundle_view_layout::{
    VIEW_LAYOUT_GAP_MILLI, VIEW_LAYOUT_SCROLL_VIEWPORT_HEIGHT_MILLI,
    VIEW_LAYOUT_TEXT_CONTROL_WIDTH_MILLI, ViewLayoutCursor, ViewLayoutFrame, button_bounds,
    modifier_layout_length_u32, named_arg, named_layout_length_u32, parse_px_milli,
    text_block_frame, u32_to_i32_saturating,
};
use super::bundle_view_overflow::validate_interactive_overflow_modifiers;

#[derive(Clone, Debug, Default)]
pub(in crate::app) struct ViewBundleSidecars {
    pub(in crate::app) program: Option<ViewProgramResource>,
    pub(in crate::app) style: Option<ViewStyleResource>,
    pub(in crate::app) text: Option<ViewTextResource>,
    pub(in crate::app) input: Option<ViewInputResource>,
    pub(in crate::app) image_objects: Vec<BundleImageObject>,
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub(in crate::app) enum ViewSidecarError {
    #[error(
        "error[AWF0617 view::interactive_overflow_requires_scroll]: `{element}` cannot use `{property}: {value}` as an interactive overflow container; wrap the content in `Scroll {{ ... }}` or move `{property}` to the Scroll element"
    )]
    InteractiveOverflowRequiresScroll {
        element: String,
        property: String,
        value: String,
    },
    #[error(
        "error[AWF0618 view::scroll_axis_both_unsupported]: `{element}` cannot use `{value}` as a Scroll axis in this cut; use `.vertical` or `.horizontal` and keep two-axis scrolling behind a future typed contract"
    )]
    UnsupportedScrollBothAxis { element: String, value: String },
}

#[derive(Default)]
struct ViewLoweringState {
    instructions: Vec<ViewProgramInstruction>,
    text_sources: Vec<ViewTextSourceRecord>,
    input_options: Vec<ViewInputOptions>,
    semantic_targets: Vec<ViewSemanticTarget>,
    layout_bounds: Vec<ViewLayoutBoundsResource>,
    scroll_regions: Vec<ViewScrollRegionResource>,
    surfaces: Vec<ViewSurfaceResource>,
    scroll_stack: Vec<String>,
    text_blocks: Vec<ViewTextBlockResource>,
    action_buttons: Vec<ViewActionButtonResource>,
    focus_groups: Vec<ViewFocusGroupResource>,
    focus_navigation: Vec<ViewFocusNavigationResource>,
    focus_group_stack: Vec<String>,
    style_resource: Option<ViewStyleResource>,
    inline_arcweft_sources: Vec<StyleSourceIdentity>,
    inline_css_sources: Vec<StyleSourceIdentity>,
    inline_part_rules: Vec<ViewPartStyleRule>,
    input_handle_bindings: Vec<InputHandleBinding>,
    source_image_objects: Vec<BundleImageObject>,
    image_objects: Vec<BundleImageObject>,
    text_counter: u32,
    input_counter: u32,
    button_counter: u32,
    scroll_counter: u32,
    text_block_counter: u32,
    image_counter: u32,
    group_counter: u32,
    handler_counter: u32,
    patch_counter: u32,
}

struct AuthoredTextControl {
    public_id: String,
    value: String,
    label: Option<String>,
    placeholder: Option<String>,
    purpose: ViewInputPurpose,
    enter_key: EnterKeyHint,
    multiline: bool,
    selection_policy: ViewTextSelectionPolicy,
    shortcut_policy: ViewTextShortcutPolicy,
    tab_policy: ViewTextTabPolicy,
    vertical_navigation_policy: ViewTextVerticalNavigationPolicy,
    secure_policy: ViewSecureInputPolicy,
    submit_handler: Option<String>,
    change_handler: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct InputHandleBinding {
    name: String,
    public_id: String,
    initial_value: String,
}

pub(in crate::app) fn view_sidecars(
    views: &[&EntityDeclItem],
    style_resource: Option<&ViewStyleResource>,
    source_image_objects: &[BundleImageObject],
) -> Result<ViewBundleSidecars, ViewSidecarError> {
    let mut state = ViewLoweringState {
        style_resource: style_resource.cloned(),
        source_image_objects: source_image_objects.to_vec(),
        ..ViewLoweringState::default()
    };
    let Some(first) = views.first() else {
        return Ok(ViewBundleSidecars::default());
    };
    for view in views {
        if let Some(body) = view.view_body().and_then(|body| body.view()) {
            lower_view_body(view.id(), body, &mut state)?;
        }
    }
    assign_action_button_bounds(&mut state);
    if state.instructions.is_empty()
        && state.text_sources.is_empty()
        && state.input_options.is_empty()
        && state.layout_bounds.is_empty()
        && state.scroll_regions.is_empty()
        && state.surfaces.is_empty()
        && state.text_blocks.is_empty()
        && state.action_buttons.is_empty()
        && state.focus_groups.is_empty()
        && state.focus_navigation.is_empty()
        && state.inline_arcweft_sources.is_empty()
        && state.inline_css_sources.is_empty()
        && state.inline_part_rules.is_empty()
        && state.image_objects.is_empty()
    {
        return Ok(ViewBundleSidecars::default());
    }
    Ok(ViewBundleSidecars {
        program: Some(ViewProgramResource {
            program_id: format!("view.program.{}", first.id().body()),
            root_view: first.id().body().to_owned(),
            instructions: state.instructions,
            child_spans: Vec::new(),
            handlers: Vec::new(),
            state_schema_hashes: Vec::new(),
            exported_parts: Vec::new(),
            semantic_targets: state.semantic_targets,
            layout_bounds: state.layout_bounds,
            scroll_regions: state.scroll_regions,
            surfaces: state.surfaces,
            text_blocks: state.text_blocks,
            action_buttons: state.action_buttons,
            focus_groups: state.focus_groups,
            focus_navigation: state.focus_navigation,
            adapter_requirements: Vec::new(),
        }),
        style: (!state.inline_arcweft_sources.is_empty() || !state.inline_css_sources.is_empty())
            .then(|| ViewStyleResource {
                style_program_id: "view.style.inline.view".to_owned(),
                arcweft_sources: state.inline_arcweft_sources,
                css_sources: state.inline_css_sources,
                tokens: Vec::new(),
                rules: Vec::new(),
                part_rules: state.inline_part_rules,
                environment_predicates: Vec::new(),
                source_map_refs: Vec::new(),
                external_css_descriptors: Vec::new(),
                adapter_requirements: Vec::new(),
            }),
        text: (!state.text_sources.is_empty()).then(|| ViewTextResource {
            sources: state.text_sources,
            ..ViewTextResource::default()
        }),
        input: (!state.input_options.is_empty()).then(|| ViewInputResource {
            options: state.input_options,
            adapter_requirements: Vec::new(),
        }),
        image_objects: state.image_objects,
    })
}

fn lower_view_body(
    view_id: &EntityRef,
    body: &ViewBody,
    state: &mut ViewLoweringState,
) -> Result<(), ViewSidecarError> {
    let mut layout = ViewLayoutCursor::root();
    lower_view_expr(view_id.body(), body.value(), state, &mut layout)?;
    Ok(())
}

fn view_resource_id(view_id: &str) -> String {
    if view_id.starts_with("view.") {
        view_id.to_owned()
    } else {
        format!("view.{view_id}")
    }
}

fn lower_view_expr(
    view_id: &str,
    expr: &ViewExpr,
    state: &mut ViewLoweringState,
    layout: &mut ViewLayoutCursor,
) -> Result<ViewLayoutFrame, ViewSidecarError> {
    Ok(match expr {
        ViewExpr::Element(element) => lower_element(view_id, element, state, layout)?,
        ViewExpr::Text(text) => lower_text(view_id, text, state, *layout)?,
        ViewExpr::TextField(field) => lower_text_field(view_id, field, state, layout)?,
        ViewExpr::Button(button) => lower_button(view_id, button, state, *layout)?,
        ViewExpr::Image(image) => lower_image(view_id, image, state, *layout)?,
        ViewExpr::Let(view_let) => lower_view_let(view_let, state),
        ViewExpr::Fragment(children) => lower_layout_column(view_id, children, state, *layout)?,
        ViewExpr::ViewCall(call) => {
            state.instructions.push(ViewProgramInstruction::CallView {
                view: expr_source(call.view()),
                child_span: 0,
                props_schema: None,
                style: None,
                part: first_part(call.modifiers()),
                key: None,
                source: None,
            });
            lower_modifiers(view_id, call.modifiers(), state);
            ViewLayoutFrame::zero()
        }
        ViewExpr::Raw(raw) => {
            state.instructions.push(ViewProgramInstruction::EmitCustom {
                element: raw.clone(),
                style: None,
                part: None,
                source: None,
            });
            ViewLayoutFrame::zero()
        }
        ViewExpr::If(branch) => lower_view_if(view_id, branch, state, layout)?,
        ViewExpr::Match(view_match) => lower_view_match(view_id, view_match, state, layout)?,
        ViewExpr::ForEach(view_for_each) => {
            lower_view_for_each(view_id, view_for_each, state, layout)?
        }
        ViewExpr::Await(view_await) => lower_view_await(view_id, view_await, state, layout)?,
        ViewExpr::Expr(_) => ViewLayoutFrame::zero(),
    })
}

fn lower_view_let(view_let: &ViewLet, state: &mut ViewLoweringState) -> ViewLayoutFrame {
    state.instructions.push(ViewProgramInstruction::BindLocal {
        pattern_schema: schema_ref_for_source(&pattern_schema_source(view_let.pattern())),
        value_schema: expr_schema_ref(view_let.value()),
        source: None,
    });
    register_input_handle_binding(view_let, state);
    ViewLayoutFrame::zero()
}

fn lower_view_if(
    view_id: &str,
    branch: &ViewIf,
    state: &mut ViewLoweringState,
    layout: &mut ViewLayoutCursor,
) -> Result<ViewLayoutFrame, ViewSidecarError> {
    let branch_index = state.instructions.len();
    state.instructions.push(ViewProgramInstruction::Branch {
        condition_schema: expr_schema_ref(branch.condition()),
        then_span: 0,
        else_span: None,
        source: None,
    });

    let then_start = state.instructions.len();
    let mut then_layout = *layout;
    let then_frame = lower_view_expr(view_id, branch.then_branch(), state, &mut then_layout)?;
    let then_span = usize_to_u32_saturating(state.instructions.len().saturating_sub(then_start));

    let (else_frame, else_span) = if let Some(branch) = branch.else_branch() {
        let else_start = state.instructions.len();
        let mut else_layout = *layout;
        let frame = lower_view_expr(view_id, branch, state, &mut else_layout)?;
        let span = usize_to_u32_saturating(state.instructions.len().saturating_sub(else_start));
        (frame, Some(span))
    } else {
        (ViewLayoutFrame::zero(), None)
    };

    state.instructions[branch_index] = ViewProgramInstruction::Branch {
        condition_schema: expr_schema_ref(branch.condition()),
        then_span,
        else_span,
        source: None,
    };
    Ok(ViewLayoutFrame::new(
        then_frame.width_milli.max(else_frame.width_milli),
        then_frame.height_milli.max(else_frame.height_milli),
    ))
}

fn lower_view_match(
    view_id: &str,
    view_match: &ViewMatch,
    state: &mut ViewLoweringState,
    layout: &mut ViewLayoutCursor,
) -> Result<ViewLayoutFrame, ViewSidecarError> {
    lower_view_match_arms(
        view_id,
        view_match.scrutinee(),
        view_match.arms(),
        state,
        layout,
    )
}

fn lower_view_match_arms(
    view_id: &str,
    scrutinee: &Expr,
    arms: &[ViewMatchArm],
    state: &mut ViewLoweringState,
    layout: &mut ViewLayoutCursor,
) -> Result<ViewLayoutFrame, ViewSidecarError> {
    let Some((arm, remaining)) = arms.split_first() else {
        return Ok(ViewLayoutFrame::zero());
    };
    let branch_index = state.instructions.len();
    state.instructions.push(ViewProgramInstruction::Branch {
        condition_schema: match_arm_schema_ref(scrutinee, arm),
        then_span: 0,
        else_span: None,
        source: None,
    });

    let then_start = state.instructions.len();
    let mut then_layout = *layout;
    let then_frame = lower_view_expr(view_id, arm.value(), state, &mut then_layout)?;
    let then_span = usize_to_u32_saturating(state.instructions.len().saturating_sub(then_start));

    let (else_frame, else_span) = if remaining.is_empty() {
        (ViewLayoutFrame::zero(), None)
    } else {
        let else_start = state.instructions.len();
        let frame = lower_view_match_arms(view_id, scrutinee, remaining, state, layout)?;
        let span = usize_to_u32_saturating(state.instructions.len().saturating_sub(else_start));
        (frame, Some(span))
    };

    state.instructions[branch_index] = ViewProgramInstruction::Branch {
        condition_schema: match_arm_schema_ref(scrutinee, arm),
        then_span,
        else_span,
        source: None,
    };
    Ok(ViewLayoutFrame::new(
        then_frame.width_milli.max(else_frame.width_milli),
        then_frame.height_milli.max(else_frame.height_milli),
    ))
}

fn lower_view_for_each(
    view_id: &str,
    view_for_each: &ViewForEach,
    state: &mut ViewLoweringState,
    layout: &mut ViewLayoutCursor,
) -> Result<ViewLayoutFrame, ViewSidecarError> {
    let repeat_index = state.instructions.len();
    state
        .instructions
        .push(ViewProgramInstruction::RepeatKeyed {
            source_schema: expr_schema_ref(view_for_each.source()),
            key_schema: repeat_key_schema_ref(view_for_each),
            body_span: 0,
            source: None,
        });
    let body_start = state.instructions.len();
    let body_frame = lower_view_expr(view_id, view_for_each.body(), state, layout)?;
    let body_span = usize_to_u32_saturating(state.instructions.len().saturating_sub(body_start));
    state.instructions[repeat_index] = ViewProgramInstruction::RepeatKeyed {
        source_schema: expr_schema_ref(view_for_each.source()),
        key_schema: repeat_key_schema_ref(view_for_each),
        body_span,
        source: None,
    };
    Ok(body_frame)
}

fn lower_view_await(
    view_id: &str,
    view_await: &ViewAwait,
    state: &mut ViewLoweringState,
    layout: &mut ViewLayoutCursor,
) -> Result<ViewLayoutFrame, ViewSidecarError> {
    let await_index = state.instructions.len();
    state.instructions.push(ViewProgramInstruction::Await {
        source_schema: expr_schema_ref(view_await.source()),
        pending_branch: None,
        ready_branch: None,
        error_branch: None,
        denied_branch: None,
        source: None,
    });

    let mut pending_branch = None;
    let mut ready_branch = None;
    let mut error_branch = None;
    let mut denied_branch = None;
    let mut frame = ViewLayoutFrame::zero();

    for branch in view_await.branches() {
        let start = state.instructions.len();
        let mut branch_layout = *layout;
        let branch_frame = lower_view_expr(view_id, branch.value(), state, &mut branch_layout)?;
        let branch_span = ViewAwaitBranchSpan {
            pattern_schema: schema_ref_for_source(&pattern_schema_source(branch.pattern())),
            body_span: usize_to_u32_saturating(state.instructions.len().saturating_sub(start)),
        };
        match branch.kind() {
            ViewAwaitBranchKind::Pending => pending_branch = Some(branch_span),
            ViewAwaitBranchKind::Ready => ready_branch = Some(branch_span),
            ViewAwaitBranchKind::Error => error_branch = Some(branch_span),
            ViewAwaitBranchKind::Denied => denied_branch = Some(branch_span),
        }
        frame = ViewLayoutFrame::new(
            frame.width_milli.max(branch_frame.width_milli),
            frame.height_milli.max(branch_frame.height_milli),
        );
    }

    state.instructions[await_index] = ViewProgramInstruction::Await {
        source_schema: expr_schema_ref(view_await.source()),
        pending_branch,
        ready_branch,
        error_branch,
        denied_branch,
        source: None,
    };
    Ok(frame)
}

fn lower_element(
    view_id: &str,
    element: &ViewElement,
    state: &mut ViewLoweringState,
    layout: &mut ViewLayoutCursor,
) -> Result<ViewLayoutFrame, ViewSidecarError> {
    if let Some(kind) = view_element_kind(element.callee()) {
        validate_interactive_overflow_modifiers(
            element.callee(),
            element.modifiers(),
            kind == ViewElementKind::Scroll,
        )?;
        state
            .instructions
            .push(ViewProgramInstruction::OpenElement {
                element: kind,
                target: None,
                style: None,
                part: first_part(element.modifiers()),
                key: None,
                source: None,
            });
        let pushed_group = lower_navigation_group(view_id, element, state);
        lower_modifiers(view_id, element.modifiers(), state);
        let frame = match kind {
            ViewElementKind::Row | ViewElementKind::LazyRow => {
                lower_layout_row(view_id, element.children(), state, *layout)?
            }
            ViewElementKind::Column | ViewElementKind::LazyColumn => {
                lower_layout_column(view_id, element.children(), state, *layout)?
            }
            ViewElementKind::Scroll => lower_scroll_region(view_id, element, state, *layout)?,
            ViewElementKind::Panel | ViewElementKind::Box | ViewElementKind::Stack => {
                lower_layout_stack(view_id, element.children(), state, *layout)?
            }
            ViewElementKind::Button => ViewLayoutFrame::action_button(),
            ViewElementKind::TextField
            | ViewElementKind::TextArea
            | ViewElementKind::SecureField => kind
                .text_input_kind()
                .map_or(ViewLayoutFrame::zero(), ViewLayoutFrame::text_control),
        };
        if pushed_group {
            state.focus_group_stack.pop();
        }
        state
            .instructions
            .push(ViewProgramInstruction::CloseElement);
        Ok(frame)
    } else {
        state.instructions.push(ViewProgramInstruction::EmitCustom {
            element: element.callee().to_owned(),
            style: None,
            part: first_part(element.modifiers()),
            source: None,
        });
        lower_modifiers(view_id, element.modifiers(), state);
        Ok(ViewLayoutFrame::zero())
    }
}

fn lower_layout_column(
    view_id: &str,
    children: &[ViewExpr],
    state: &mut ViewLoweringState,
    origin: ViewLayoutCursor,
) -> Result<ViewLayoutFrame, ViewSidecarError> {
    let mut cursor = origin;
    let mut width_milli = 0_u32;
    let mut height_milli = 0_u32;
    let mut placed = false;
    for child in children {
        if placed {
            cursor.y_milli = cursor.y_milli.saturating_add(VIEW_LAYOUT_GAP_MILLI);
        }
        let frame = lower_view_expr(view_id, child, state, &mut cursor)?;
        if frame.is_empty() {
            continue;
        }
        width_milli = width_milli.max(frame.width_milli);
        height_milli = height_milli
            .saturating_add(if placed {
                VIEW_LAYOUT_GAP_MILLI as u32
            } else {
                0
            })
            .saturating_add(frame.height_milli);
        cursor.y_milli = cursor
            .y_milli
            .saturating_add(u32_to_i32_saturating(frame.height_milli));
        placed = true;
    }
    Ok(ViewLayoutFrame::new(width_milli, height_milli))
}

fn lower_scroll_region(
    view_id: &str,
    element: &ViewElement,
    state: &mut ViewLoweringState,
    origin: ViewLayoutCursor,
) -> Result<ViewLayoutFrame, ViewSidecarError> {
    let options = scroll_region_options(
        view_id,
        element,
        state.scroll_counter,
        state.style_resource.as_ref(),
    )?;
    let scroll_id = options.public_id.clone();
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
    style_resource: Option<&ViewStyleResource>,
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
    if let Some(style_resource) = style_resource {
        apply_scroll_style_rules(&mut options, style_resource, element);
    }
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
            ViewModifier::Style(
                ViewStyleModifier::InlineArcweft(source) | ViewStyleModifier::InlineCss(source),
            ) => {
                for (name, value) in inline_style_properties(source) {
                    if normalize_property_name(&name) == "axis"
                        && ViewScrollAxis::is_unsupported_dual_axis_symbol(&value)
                    {
                        return Err(ViewSidecarError::UnsupportedScrollBothAxis {
                            element: element.callee().to_owned(),
                            value,
                        });
                    }
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn apply_scroll_style_rules(
    options: &mut ScrollRegionOptions,
    style_resource: &ViewStyleResource,
    element: &ViewElement,
) {
    let explicit_styles = explicit_style_refs(element.modifiers());
    let part = first_part(element.modifiers());
    for rule in &style_resource.rules {
        if scroll_style_selector_matches(
            &rule.selector.parts,
            &options.public_id,
            part.as_deref(),
            &explicit_styles,
        ) {
            apply_scroll_style_declarations(options, style_resource, &rule.declarations);
        }
    }
    for rule in &style_resource.part_rules {
        if scroll_style_part_matches(
            &rule.part,
            &options.public_id,
            part.as_deref(),
            &explicit_styles,
        ) && scroll_style_selector_matches(
            &rule.selector.parts,
            &options.public_id,
            part.as_deref(),
            &explicit_styles,
        ) {
            apply_scroll_style_declarations(options, style_resource, &rule.declarations);
        }
    }
}

fn scroll_style_selector_matches(
    parts: &[ViewStyleSelectorPart],
    target_id: &str,
    authored_part: Option<&str>,
    explicit_styles: &[String],
) -> bool {
    let mut has_positive_match = parts.is_empty();
    for part in parts {
        match part {
            ViewStyleSelectorPart::Element(ViewElementKind::Scroll) => {
                has_positive_match = true;
            }
            ViewStyleSelectorPart::Part(candidate)
                if scroll_style_part_matches(
                    candidate,
                    target_id,
                    authored_part,
                    explicit_styles,
                ) =>
            {
                has_positive_match = true;
            }
            ViewStyleSelectorPart::Element(_)
            | ViewStyleSelectorPart::Part(_)
            | ViewStyleSelectorPart::State(_)
            | ViewStyleSelectorPart::Interaction(_)
            | ViewStyleSelectorPart::Environment(_)
            | ViewStyleSelectorPart::Descendant
            | ViewStyleSelectorPart::Child => return false,
        }
    }
    has_positive_match
}

fn scroll_style_part_matches(
    candidate: &str,
    target_id: &str,
    authored_part: Option<&str>,
    explicit_styles: &[String],
) -> bool {
    id_or_tail_matches(candidate, target_id)
        || authored_part.is_some_and(|part| id_or_tail_matches(candidate, part))
        || explicit_styles
            .iter()
            .any(|style| id_or_tail_matches(candidate, style))
}

fn id_or_tail_matches(candidate: &str, target: &str) -> bool {
    let candidate = candidate.trim().trim_start_matches('.');
    let target = target.trim().trim_start_matches('@');
    candidate == target
        || target
            .rsplit('.')
            .next()
            .is_some_and(|tail| candidate == tail)
}

fn explicit_style_refs(modifiers: &[ViewModifier]) -> Vec<String> {
    modifiers
        .iter()
        .filter_map(|modifier| match modifier {
            ViewModifier::Style(ViewStyleModifier::Named(reference)) => {
                Some(normalize_style_ref(reference))
            }
            _ => None,
        })
        .collect()
}

fn apply_scroll_style_declarations(
    options: &mut ScrollRegionOptions,
    style_resource: &ViewStyleResource,
    declarations: &[ViewStyleDeclaration],
) {
    for declaration in declarations {
        if declaration.op != StyleAssignOp::Replace {
            continue;
        }
        match normalize_property_name(&declaration.property).as_str() {
            "width" | "w" => {
                if let Some(width) = scroll_style_length_milli(style_resource, &declaration.value) {
                    options.width_milli = Some(width);
                }
            }
            "height" | "h" => {
                if let Some(height) = scroll_style_length_milli(style_resource, &declaration.value)
                {
                    options.height_milli = Some(height);
                }
            }
            "overflow" | "overflow-y" => {
                if let Some(overflow) = scroll_style_overflow(style_resource, &declaration.value) {
                    options.overflow = overflow;
                }
            }
            "overflow-x" => {
                if let Some(overflow) = scroll_style_overflow(style_resource, &declaration.value) {
                    options.axis = ViewScrollAxis::Horizontal;
                    options.overflow = overflow;
                }
            }
            "axis" => {
                if let Some(axis) = scroll_style_axis(style_resource, &declaration.value) {
                    options.axis = axis;
                }
            }
            "indicators" | "scroll-indicators" => {
                if let Some(indicators) =
                    scroll_style_indicators(style_resource, &declaration.value)
                {
                    options.indicators = indicators;
                }
            }
            "overscroll" | "overscroll-behavior" => {
                if let Some(overscroll) =
                    scroll_style_overscroll(style_resource, &declaration.value)
                {
                    options.overscroll = overscroll;
                }
            }
            "auto-scroll-focus" | "auto-focus-scroll" => {
                if let Some(policy) = scroll_style_auto_focus(style_resource, &declaration.value) {
                    options.auto_scroll_focus = policy;
                }
            }
            "clip" => {
                if let Some(clip) = scroll_style_bool(style_resource, &declaration.value) {
                    options.overflow = if clip {
                        ViewScrollOverflowPolicy::Auto
                    } else {
                        ViewScrollOverflowPolicy::Hidden
                    };
                }
            }
            _ => {}
        }
    }
}

fn scroll_style_length_milli(
    style_resource: &ViewStyleResource,
    value: &ViewStyleValue,
) -> Option<u32> {
    scroll_style_length_milli_inner(style_resource, value, 0)
}

fn scroll_style_length_milli_inner(
    style_resource: &ViewStyleResource,
    value: &ViewStyleValue,
    depth: u8,
) -> Option<u32> {
    if depth > 8 {
        return None;
    }
    match value {
        ViewStyleValue::Milli(value) => u32::try_from((*value).max(1)).ok(),
        ViewStyleValue::Text(value) | ViewStyleValue::Resource(value) => {
            style_layout_length_u32(value)
        }
        ViewStyleValue::Token(token) => style_token_value(style_resource, token)
            .and_then(|value| scroll_style_length_milli_inner(style_resource, value, depth + 1)),
        ViewStyleValue::SystemColor(_)
        | ViewStyleValue::Rgba(_)
        | ViewStyleValue::List(_)
        | ViewStyleValue::Digest(_) => None,
    }
}

fn scroll_style_overflow(
    style_resource: &ViewStyleResource,
    value: &ViewStyleValue,
) -> Option<ViewScrollOverflowPolicy> {
    scroll_style_overflow_inner(style_resource, value, 0)
}

fn scroll_style_overflow_inner(
    style_resource: &ViewStyleResource,
    value: &ViewStyleValue,
    depth: u8,
) -> Option<ViewScrollOverflowPolicy> {
    if depth > 8 {
        return None;
    }
    match value {
        ViewStyleValue::Text(value) | ViewStyleValue::Resource(value) => {
            scroll_overflow_symbol(value)
        }
        ViewStyleValue::Token(token) => style_token_value(style_resource, token)
            .and_then(|value| scroll_style_overflow_inner(style_resource, value, depth + 1)),
        ViewStyleValue::Milli(_)
        | ViewStyleValue::SystemColor(_)
        | ViewStyleValue::Rgba(_)
        | ViewStyleValue::List(_)
        | ViewStyleValue::Digest(_) => None,
    }
}

fn scroll_style_axis(
    style_resource: &ViewStyleResource,
    value: &ViewStyleValue,
) -> Option<ViewScrollAxis> {
    scroll_style_axis_inner(style_resource, value, 0)
}

fn scroll_style_axis_inner(
    style_resource: &ViewStyleResource,
    value: &ViewStyleValue,
    depth: u8,
) -> Option<ViewScrollAxis> {
    if depth > 8 {
        return None;
    }
    match value {
        ViewStyleValue::Text(value) | ViewStyleValue::Resource(value) => scroll_axis_symbol(value),
        ViewStyleValue::Token(token) => style_token_value(style_resource, token)
            .and_then(|value| scroll_style_axis_inner(style_resource, value, depth + 1)),
        ViewStyleValue::Milli(_)
        | ViewStyleValue::SystemColor(_)
        | ViewStyleValue::Rgba(_)
        | ViewStyleValue::List(_)
        | ViewStyleValue::Digest(_) => None,
    }
}

fn scroll_style_indicators(
    style_resource: &ViewStyleResource,
    value: &ViewStyleValue,
) -> Option<ViewScrollIndicatorsPolicy> {
    scroll_style_policy_inner(
        style_resource,
        value,
        0,
        ViewScrollIndicatorsPolicy::from_author_symbol,
    )
}

fn scroll_style_overscroll(
    style_resource: &ViewStyleResource,
    value: &ViewStyleValue,
) -> Option<ViewScrollOverscrollPolicy> {
    scroll_style_policy_inner(
        style_resource,
        value,
        0,
        ViewScrollOverscrollPolicy::from_author_symbol,
    )
}

fn scroll_style_auto_focus(
    style_resource: &ViewStyleResource,
    value: &ViewStyleValue,
) -> Option<ViewFocusAutoScrollPolicy> {
    scroll_style_policy_inner(
        style_resource,
        value,
        0,
        ViewFocusAutoScrollPolicy::from_author_symbol,
    )
}

fn scroll_style_policy_inner<T>(
    style_resource: &ViewStyleResource,
    value: &ViewStyleValue,
    depth: u8,
    parse: fn(&str) -> Option<T>,
) -> Option<T> {
    if depth > 8 {
        return None;
    }
    match value {
        ViewStyleValue::Text(value) | ViewStyleValue::Resource(value) => parse(value),
        ViewStyleValue::Token(token) => style_token_value(style_resource, token)
            .and_then(|value| scroll_style_policy_inner(style_resource, value, depth + 1, parse)),
        ViewStyleValue::Milli(_)
        | ViewStyleValue::SystemColor(_)
        | ViewStyleValue::Rgba(_)
        | ViewStyleValue::List(_)
        | ViewStyleValue::Digest(_) => None,
    }
}

fn scroll_style_bool(style_resource: &ViewStyleResource, value: &ViewStyleValue) -> Option<bool> {
    scroll_style_bool_inner(style_resource, value, 0)
}

fn scroll_style_bool_inner(
    style_resource: &ViewStyleResource,
    value: &ViewStyleValue,
    depth: u8,
) -> Option<bool> {
    if depth > 8 {
        return None;
    }
    match value {
        ViewStyleValue::Text(value) | ViewStyleValue::Resource(value) => parse_bool_like(value),
        ViewStyleValue::Token(token) => style_token_value(style_resource, token)
            .and_then(|value| scroll_style_bool_inner(style_resource, value, depth + 1)),
        ViewStyleValue::Milli(_)
        | ViewStyleValue::SystemColor(_)
        | ViewStyleValue::Rgba(_)
        | ViewStyleValue::List(_)
        | ViewStyleValue::Digest(_) => None,
    }
}

fn style_token_value<'a>(
    style_resource: &'a ViewStyleResource,
    token: &str,
) -> Option<&'a ViewStyleValue> {
    let token = token.trim();
    style_resource
        .tokens
        .iter()
        .find(|candidate| id_or_tail_matches(token, &candidate.public_id))
        .map(|candidate| &candidate.value)
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
        match modifier {
            ViewModifier::Property { name, value } => {
                apply_scroll_property(options, name, &expr_source(value));
            }
            ViewModifier::Style(
                ViewStyleModifier::InlineArcweft(source) | ViewStyleModifier::InlineCss(source),
            ) => {
                for (name, value) in inline_style_properties(source) {
                    apply_scroll_property(options, &name, &value);
                }
            }
            _ => {}
        }
    }
}

fn apply_scroll_property(options: &mut ScrollRegionOptions, name: &str, value: &str) {
    match normalize_property_name(name).as_str() {
        "width" | "w" => {
            if let Some(width) = style_layout_length_u32(value) {
                options.width_milli = Some(width);
            }
        }
        "height" | "h" => {
            if let Some(height) = style_layout_length_u32(value) {
                options.height_milli = Some(height);
            }
        }
        "overflow" | "overflow-y" => {
            if let Some(overflow) = scroll_overflow_symbol(value) {
                options.overflow = overflow;
            }
        }
        "overflow-x" => {
            if let Some(overflow) = scroll_overflow_symbol(value) {
                options.axis = ViewScrollAxis::Horizontal;
                options.overflow = overflow;
            }
        }
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
        "clip" => {
            if let Some(clip) = parse_bool_like(value) {
                options.overflow = if clip {
                    ViewScrollOverflowPolicy::Auto
                } else {
                    ViewScrollOverflowPolicy::Hidden
                };
            }
        }
        _ => {}
    }
}

pub(in crate::app) fn normalize_property_name(value: &str) -> String {
    value.trim().replace('_', "-").to_ascii_lowercase()
}

pub(in crate::app) fn inline_style_properties(
    source: &str,
) -> impl Iterator<Item = (String, String)> + '_ {
    source.lines().filter_map(|line| {
        let line = line.trim().trim_end_matches(';').trim();
        if line.is_empty() {
            return None;
        }
        let (name, value) = line.split_once('=').or_else(|| line.split_once(':'))?;
        Some((name.trim().to_owned(), value.trim().to_owned()))
    })
}

pub(in crate::app) fn style_layout_length_u32(value: &str) -> Option<u32> {
    let value = value.trim().trim_matches('"');
    let milli = value
        .strip_suffix("px")
        .map(str::trim)
        .and_then(parse_px_milli)
        .or_else(|| {
            value
                .strip_suffix("milli")
                .map(str::trim)
                .and_then(|raw| raw.parse::<i32>().ok())
        })
        .or_else(|| parse_px_milli(value))?;
    u32::try_from(milli.max(1)).ok()
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

fn lower_layout_row(
    view_id: &str,
    children: &[ViewExpr],
    state: &mut ViewLoweringState,
    origin: ViewLayoutCursor,
) -> Result<ViewLayoutFrame, ViewSidecarError> {
    let mut cursor = origin;
    let mut width_milli = 0_u32;
    let mut height_milli = 0_u32;
    let mut placed = false;
    for child in children {
        if placed {
            cursor.x_milli = cursor.x_milli.saturating_add(VIEW_LAYOUT_GAP_MILLI);
        }
        let frame = lower_view_expr(view_id, child, state, &mut cursor)?;
        if frame.is_empty() {
            continue;
        }
        width_milli = width_milli
            .saturating_add(if placed {
                VIEW_LAYOUT_GAP_MILLI as u32
            } else {
                0
            })
            .saturating_add(frame.width_milli);
        height_milli = height_milli.max(frame.height_milli);
        cursor.x_milli = cursor
            .x_milli
            .saturating_add(u32_to_i32_saturating(frame.width_milli));
        placed = true;
    }
    Ok(ViewLayoutFrame::new(width_milli, height_milli))
}

fn lower_layout_stack(
    view_id: &str,
    children: &[ViewExpr],
    state: &mut ViewLoweringState,
    origin: ViewLayoutCursor,
) -> Result<ViewLayoutFrame, ViewSidecarError> {
    let mut frame = ViewLayoutFrame::zero();
    for child in children {
        let mut cursor = origin;
        let child_frame = lower_view_expr(view_id, child, state, &mut cursor)?;
        frame = ViewLayoutFrame::new(
            frame.width_milli.max(child_frame.width_milli),
            frame.height_milli.max(child_frame.height_milli),
        );
    }
    Ok(frame)
}

fn lower_text(
    view_id: &str,
    text: &ViewText,
    state: &mut ViewLoweringState,
    layout: ViewLayoutCursor,
) -> Result<ViewLayoutFrame, ViewSidecarError> {
    validate_interactive_overflow_modifiers("Text", text.modifiers(), false)?;
    let id = next_text_source_id(view_id, state);
    let text_value = expr_source(text.source());
    state.text_sources.push(ViewTextSourceRecord {
        public_id: id.clone(),
        kind: ViewTextSourceKind::Literal {
            value: text_value.clone(),
        },
        source: None,
    });
    state.instructions.push(ViewProgramInstruction::EmitText {
        text_source: id.clone(),
        style: None,
        part: first_part(text.modifiers()),
        source: None,
    });
    lower_modifiers(view_id, text.modifiers(), state);
    let frame = text_block_frame(&text_value, text.modifiers());
    let text_block_id = next_text_block_id(view_id, state);
    let view = Some(view_resource_id(view_id));
    let scroll_region = state.scroll_stack.last().cloned();
    let mut text_block = ViewTextBlockResource::new(
        text_block_id,
        view,
        scroll_region,
        id,
        ViewRuntimeTextBlockBounds::new(
            layout.x_milli,
            layout.y_milli,
            frame.width_milli,
            frame.height_milli,
        ),
    );
    text_block.selection_policy = text_block_selection_policy(text.modifiers());
    state.text_blocks.push(text_block);
    Ok(frame)
}

fn lower_text_field(
    view_id: &str,
    field: &ViewTextField,
    state: &mut ViewLoweringState,
    layout: &mut ViewLayoutCursor,
) -> Result<ViewLayoutFrame, ViewSidecarError> {
    validate_interactive_overflow_modifiers(
        text_field_mode_callee(field.mode()),
        field.modifiers(),
        false,
    )?;
    let control = AuthoredTextControl::from_field(view_id, field, state);
    let public_id = control.public_id.clone();
    let kind = view_input_kind(field.mode());
    let rect = layout.text_control_rect(kind);
    let value_text_source = input_text_source_id("value", &control.public_id);
    state.text_sources.push(ViewTextSourceRecord {
        public_id: value_text_source.clone(),
        kind: ViewTextSourceKind::Literal {
            value: control.value,
        },
        source: None,
    });
    let label_text_source = control.label.map(|label| {
        let id = input_text_source_id("label", &control.public_id);
        state.text_sources.push(ViewTextSourceRecord {
            public_id: id.clone(),
            kind: ViewTextSourceKind::Literal { value: label },
            source: None,
        });
        id
    });
    let placeholder_text_source = control.placeholder.map(|placeholder| {
        let id = input_text_source_id("placeholder", &control.public_id);
        state.text_sources.push(ViewTextSourceRecord {
            public_id: id.clone(),
            kind: ViewTextSourceKind::Literal { value: placeholder },
            source: None,
        });
        id
    });
    state.input_options.push(ViewInputOptions {
        public_id: control.public_id.clone(),
        view: Some(view_resource_id(view_id)),
        containing_scroll_region: state.scroll_stack.last().cloned(),
        kind,
        value_text_source,
        placeholder_text_source,
        purpose: control.purpose,
        autocorrect: TextAssistPolicy::PlatformDefault,
        spellcheck: TextAssistPolicy::PlatformDefault,
        capitalization: TextCapitalization::None,
        enter_key: control.enter_key,
        multiline: control.multiline,
        selection_policy: control.selection_policy,
        shortcut_policy: control.shortcut_policy,
        tab_policy: control.tab_policy,
        vertical_navigation_policy: control.vertical_navigation_policy,
        secure_policy: control.secure_policy,
        composition_on_blur: CompositionOnBlurPolicy::Commit,
        submit_handler: control.submit_handler,
        change_handler: control.change_handler,
        adapter_requirements: Vec::new(),
    });
    state
        .layout_bounds
        .push(ViewLayoutBoundsResource::text_control(
            public_id.clone(),
            rect,
        ));
    state
        .layout_bounds
        .push(ViewLayoutBoundsResource::semantic_target(
            public_id.clone(),
            rect,
        ));
    state.semantic_targets.push(ViewSemanticTarget {
        public_id: public_id.clone(),
        target: public_id.clone(),
        view: Some(view_resource_id(view_id)),
        label_text_source,
        source: None,
    });
    let target = state
        .semantic_targets
        .last()
        .map(|target| target.public_id.clone());
    if let Some(target) = target {
        lower_navigation_target(view_id, &target, field.modifiers(), state);
    }
    state
        .instructions
        .push(ViewProgramInstruction::OpenElement {
            element: view_element_kind_for_text_field(field.mode()),
            target: Some(public_id.clone()),
            style: None,
            part: first_part(field.modifiers()),
            key: None,
            source: None,
        });
    lower_modifiers(view_id, field.modifiers(), state);
    state
        .instructions
        .push(ViewProgramInstruction::CloseElement);
    Ok(ViewLayoutFrame::text_control(kind))
}

fn lower_button(
    view_id: &str,
    button: &ViewButton,
    state: &mut ViewLoweringState,
    layout: ViewLayoutCursor,
) -> Result<ViewLayoutFrame, ViewSidecarError> {
    validate_interactive_overflow_modifiers("Button", button.modifiers(), false)?;
    let button_id = button
        .id()
        .map_or_else(|| next_button_id(view_id, state), normalize_entity_ref);
    let label_text_source = format!("text.button.label.{button_id}");
    let label = button_display_label(button, &button_id);
    state.text_sources.push(ViewTextSourceRecord {
        public_id: label_text_source.clone(),
        kind: ViewTextSourceKind::Literal {
            value: label.clone(),
        },
        source: None,
    });
    state
        .instructions
        .push(ViewProgramInstruction::OpenElement {
            element: ViewElementKind::Button,
            target: Some(button_id.clone()),
            style: None,
            part: first_part(button.modifiers()),
            key: None,
            source: None,
        });
    lower_button_modifiers(view_id, button.modifiers(), state);
    state
        .instructions
        .push(ViewProgramInstruction::CloseElement);
    lower_navigation_target(view_id, &button_id, button.modifiers(), state);

    let action = match button.activation() {
        Some(ViewAction::ActionInvoke(action)) => ViewActionButtonActionResource::ActionInvoke {
            action: normalize_entity_ref(action.action()),
            payload: action.payload().map(lower_action_payload),
        },
        Some(ViewAction::Noop) | None => ViewActionButtonActionResource::Noop,
    };
    state.action_buttons.push(ViewActionButtonResource {
        public_id: button_id.clone(),
        view: Some(view_resource_id(view_id)),
        containing_scroll_region: state.scroll_stack.last().cloned(),
        label_text_source: label_text_source.clone(),
        enabled: button_enabled(button.enabled()),
        action,
        bounds: button_bounds(button, layout),
        style: None,
        source: None,
    });
    state.semantic_targets.push(ViewSemanticTarget {
        public_id: button_id.clone(),
        target: button_id,
        view: Some(view_resource_id(view_id)),
        label_text_source: Some(label_text_source),
        source: None,
    });
    Ok(ViewLayoutFrame::action_button())
}

fn lower_image(
    view_id: &str,
    image: &ViewImage,
    state: &mut ViewLoweringState,
    layout: ViewLayoutCursor,
) -> Result<ViewLayoutFrame, ViewSidecarError> {
    validate_interactive_overflow_modifiers("Image", image.modifiers(), false)?;
    state.instructions.push(ViewProgramInstruction::EmitImage {
        image: expr_source(image.source()),
        style: None,
        part: first_part(image.modifiers()),
        source: None,
    });
    lower_modifiers(view_id, image.modifiers(), state);
    let Some(source_id) = image_source_object_id(image.source()) else {
        return Ok(ViewLayoutFrame::zero());
    };
    let Some(source) = state
        .source_image_objects
        .iter()
        .find(|object| object.id == source_id)
        .cloned()
    else {
        return Ok(ViewLayoutFrame::zero());
    };
    let width_milli = modifier_layout_length_u32(image.modifiers(), &["width", "w"])
        .unwrap_or(source.bounds.width_milli);
    let height_milli = modifier_layout_length_u32(image.modifiers(), &["height", "h"])
        .unwrap_or(source.bounds.height_milli);
    if width_milli == 0 || height_milli == 0 {
        return Ok(ViewLayoutFrame::zero());
    }
    let mut object = source;
    object.id = next_image_id(view_id, state);
    object.bounds = BundleImageObjectBounds {
        x_milli: layout.x_milli,
        y_milli: layout.y_milli,
        width_milli,
        height_milli,
    };
    object.placement = None;
    object.view = Some(view_resource_id(view_id));
    object.containing_scroll_region = state.scroll_stack.last().cloned();
    state.image_objects.push(object);
    Ok(ViewLayoutFrame::new(width_milli, height_milli))
}

fn lower_modifiers(view_id: &str, modifiers: &[ViewModifier], state: &mut ViewLoweringState) {
    for modifier in modifiers {
        match modifier {
            ViewModifier::Style(style) => {
                let style = lower_style_apply(view_id, style, state);
                state.instructions.push(ViewProgramInstruction::ApplyStyle {
                    style,
                    source: None,
                });
            }
            ViewModifier::OnEvent { name, .. } => {
                lower_event_handler_modifier(view_id, name, state);
            }
            ViewModifier::Part(_)
            | ViewModifier::Label(_)
            | ViewModifier::AgentTarget(_)
            | ViewModifier::Placeholder(_)
            | ViewModifier::Purpose(_)
            | ViewModifier::EnterKey(_)
            | ViewModifier::Enabled(_)
            | ViewModifier::Focusable(_)
            | ViewModifier::Property { .. }
            | ViewModifier::Environment(_)
            | ViewModifier::Focus(_)
            | ViewModifier::Navigation(_)
            | ViewModifier::Raw(_) => {}
        }
    }
}

fn lower_button_modifiers(
    view_id: &str,
    modifiers: &[ViewModifier],
    state: &mut ViewLoweringState,
) {
    for modifier in modifiers {
        match modifier {
            ViewModifier::Style(style) => {
                let style = lower_style_apply(view_id, style, state);
                state.instructions.push(ViewProgramInstruction::ApplyStyle {
                    style,
                    source: None,
                });
            }
            ViewModifier::Part(_)
            | ViewModifier::Label(_)
            | ViewModifier::AgentTarget(_)
            | ViewModifier::Placeholder(_)
            | ViewModifier::Purpose(_)
            | ViewModifier::EnterKey(_)
            | ViewModifier::Enabled(_)
            | ViewModifier::Focusable(_)
            | ViewModifier::Property { .. }
            | ViewModifier::Environment(_)
            | ViewModifier::Focus(_)
            | ViewModifier::Navigation(_)
            | ViewModifier::Raw(_) => {}
            ViewModifier::OnEvent { name, .. } => {
                lower_event_handler_modifier(view_id, name, state);
            }
        }
    }
}

fn lower_event_handler_modifier(view_id: &str, name: &str, state: &mut ViewLoweringState) {
    let handler = format!("{view_id}.handler.{name}.{}", state.handler_counter);
    state.handler_counter += 1;
    state
        .instructions
        .push(ViewProgramInstruction::BindHandler {
            event: name.to_owned(),
            handler,
            source: None,
        });
}

fn lower_navigation_group(
    view_id: &str,
    element: &ViewElement,
    state: &mut ViewLoweringState,
) -> bool {
    let Some(group) = element.navigation_group() else {
        return false;
    };
    let public_id = group
        .group()
        .map_or_else(|| next_focus_group_id(view_id, state), normalize_entity_ref);
    let parent = group
        .parent()
        .map(normalize_entity_ref)
        .or_else(|| state.focus_group_stack.last().cloned());
    state.focus_groups.push(ViewFocusGroupResource {
        public_id: public_id.clone(),
        view: Some(view_resource_id(view_id)),
        parent,
        policy: lower_navigation_trap(group.trap()),
        initial: lower_navigation_initial(group.initial()),
        wrap: group.wrap().map_or(ViewFocusWrapPolicy::Wrap, |wrap| {
            if wrap {
                ViewFocusWrapPolicy::Wrap
            } else {
                ViewFocusWrapPolicy::NoWrap
            }
        }),
        disabled_skip: ViewFocusSkipPolicy::Skip,
        hidden_skip: ViewFocusSkipPolicy::Skip,
        source: None,
    });
    state.focus_group_stack.push(public_id);
    true
}

fn lower_navigation_target(
    view_id: &str,
    public_id: &str,
    modifiers: &[ViewModifier],
    state: &mut ViewLoweringState,
) {
    let edges = modifiers
        .iter()
        .filter_map(|modifier| match modifier {
            ViewModifier::Navigation(navigation) => Some(navigation),
            _ => None,
        })
        .flat_map(|navigation| {
            navigation
                .edges()
                .iter()
                .map(|edge| ViewFocusNavigationEdge {
                    direction: lower_navigation_direction(edge.direction()),
                    target: lower_navigation_target_resolution(edge.target()),
                    source: None,
                })
        })
        .collect::<Vec<_>>();
    if edges.is_empty() {
        return;
    }
    state.focus_navigation.push(ViewFocusNavigationResource {
        public_id: public_id.to_owned(),
        view: Some(view_resource_id(view_id)),
        group: state.focus_group_stack.last().cloned(),
        edges,
        source: None,
    });
}

fn lower_navigation_direction(direction: ViewNavigationDirection) -> ViewFocusDirection {
    match direction {
        ViewNavigationDirection::Up => ViewFocusDirection::Up,
        ViewNavigationDirection::Down => ViewFocusDirection::Down,
        ViewNavigationDirection::Left => ViewFocusDirection::Left,
        ViewNavigationDirection::Right => ViewFocusDirection::Right,
        ViewNavigationDirection::Next => ViewFocusDirection::Next,
        ViewNavigationDirection::Previous => ViewFocusDirection::Previous,
    }
}

fn lower_navigation_target_resolution(target: &ViewNavigationTarget) -> ViewFocusTargetResolution {
    match target {
        ViewNavigationTarget::Explicit(target) => ViewFocusTargetResolution::Explicit {
            target: normalize_entity_ref(target),
        },
        ViewNavigationTarget::Auto => ViewFocusTargetResolution::Auto,
        ViewNavigationTarget::None => ViewFocusTargetResolution::None,
        ViewNavigationTarget::GroupBoundary => ViewFocusTargetResolution::GroupBoundary,
    }
}

fn lower_navigation_initial(initial: &ViewNavigationInitial) -> ViewFocusInitialPolicy {
    match initial {
        ViewNavigationInitial::Auto => ViewFocusInitialPolicy::Auto,
        ViewNavigationInitial::First => ViewFocusInitialPolicy::First,
        ViewNavigationInitial::Last => ViewFocusInitialPolicy::Last,
        ViewNavigationInitial::Explicit(target) => ViewFocusInitialPolicy::Explicit {
            target: normalize_entity_ref(target),
        },
        ViewNavigationInitial::None => ViewFocusInitialPolicy::None,
    }
}

fn lower_navigation_trap(trap: ViewNavigationTrap) -> ViewFocusGroupPolicy {
    match trap {
        ViewNavigationTrap::Normal => ViewFocusGroupPolicy::Normal,
        ViewNavigationTrap::Trap => ViewFocusGroupPolicy::Trap,
        ViewNavigationTrap::Modal => ViewFocusGroupPolicy::Modal,
    }
}

fn lower_style_apply(
    view_id: &str,
    style: &ViewStyleModifier,
    state: &mut ViewLoweringState,
) -> ViewStyleApplyRef {
    match style {
        ViewStyleModifier::Named(reference) => {
            ViewStyleApplyRef::Named(normalize_style_ref(reference))
        }
        ViewStyleModifier::InlineArcweft(source) => {
            let patch_id = next_patch_id(view_id, source, StyleSyntax::Arcweft, state);
            ViewStyleApplyRef::InlineArcweft { patch_id }
        }
        ViewStyleModifier::InlineCss(source) => {
            let patch_id = next_patch_id(view_id, source, StyleSyntax::Css, state);
            ViewStyleApplyRef::InlineCss { patch_id }
        }
    }
}

fn next_patch_id(
    view_id: &str,
    source: &str,
    syntax: StyleSyntax,
    state: &mut ViewLoweringState,
) -> u32 {
    let patch_id = state.patch_counter;
    state.patch_counter += 1;
    let source_digest = BundleDigest::of(source.as_bytes());
    let identity = StyleSourceIdentity {
        public_id: format!("style.inline.{view_id}.{patch_id}"),
        syntax,
        identity: StyleSourceRef::Inline { source_digest },
        content_digest: Some(source_digest),
    };
    match syntax {
        StyleSyntax::Arcweft => state.inline_arcweft_sources.push(identity),
        StyleSyntax::Css => state.inline_css_sources.push(identity),
    }
    let declarations = inline_style_declarations(source);
    if !declarations.is_empty() {
        state.inline_part_rules.push(ViewPartStyleRule {
            part: ViewStyleApplyRef::inline_patch_part(patch_id),
            selector: ViewStyleSelector::default(),
            declarations,
            source: None,
        });
    }
    patch_id
}

fn inline_style_declarations(source: &str) -> Vec<ViewStyleDeclaration> {
    inline_style_properties(source)
        .map(|(property, value)| ViewStyleDeclaration {
            property,
            value: inline_style_value(&value),
            op: StyleAssignOp::Replace,
        })
        .collect()
}

fn inline_style_value(raw: &str) -> ViewStyleValue {
    let value = raw.trim().trim_end_matches(';').trim();
    if let Some(argument) = style_function_argument(value, "text") {
        return ViewStyleValue::Text(unquote_style_argument(argument));
    }
    if let Some(argument) = style_function_argument(value, "token") {
        return ViewStyleValue::Token(unquote_style_argument(argument));
    }
    if let Some(argument) = style_function_argument(value, "resource") {
        return ViewStyleValue::Resource(unquote_style_argument(argument));
    }
    if let Some(argument) = style_function_argument(value, "milli")
        && let Ok(milli) = argument.trim().parse::<i32>()
    {
        return ViewStyleValue::Milli(milli);
    }
    if let Some(raw_milli) = value.strip_suffix("milli")
        && let Ok(milli) = raw_milli.trim().parse::<i32>()
    {
        return ViewStyleValue::Milli(milli);
    }
    ViewStyleValue::Text(unquote_style_argument(value))
}

fn style_function_argument<'a>(value: &'a str, name: &str) -> Option<&'a str> {
    value
        .strip_prefix(name)?
        .trim_start()
        .strip_prefix('(')?
        .strip_suffix(')')
        .map(str::trim)
}

fn unquote_style_argument(value: &str) -> String {
    let value = value.trim();
    value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .or_else(|| {
            value
                .strip_prefix('\'')
                .and_then(|value| value.strip_suffix('\''))
        })
        .unwrap_or(value)
        .to_owned()
}

fn expr_schema_ref(expr: &Expr) -> DigestRef {
    schema_ref_for_source(&expr_source(expr))
}

fn match_arm_schema_ref(scrutinee: &Expr, arm: &ViewMatchArm) -> DigestRef {
    let guard = arm.guard().map(expr_source).unwrap_or_default();
    schema_ref_for_source(&format!(
        "match:{}=>{} when {}",
        expr_source(scrutinee),
        pattern_schema_source(arm.pattern()),
        guard
    ))
}

fn repeat_key_schema_ref(view_for_each: &ViewForEach) -> DigestRef {
    view_for_each.key().map_or_else(
        || {
            schema_ref_for_source(&format!(
                "source_order:{} in {}",
                pattern_schema_source(view_for_each.pattern()),
                expr_source(view_for_each.source())
            ))
        },
        expr_schema_ref,
    )
}

fn schema_ref_for_source(source: &str) -> DigestRef {
    DigestRef {
        digest: BundleDigest::of(source.as_bytes()),
    }
}

fn pattern_schema_source(pattern: &Pattern) -> String {
    match pattern {
        Pattern::Ident(name) => name.clone(),
        Pattern::MutIdent(name) => format!("mut {name}"),
        Pattern::Literal(expr) => expr_source(expr),
        Pattern::Entity(entity) => entity.body().to_owned(),
        Pattern::Variant {
            path,
            name,
            payload,
        } => format!(
            "{}{}{}",
            path.as_ref().map_or("", String::as_str),
            name,
            payload
                .as_ref()
                .map_or_else(String::new, variant_pattern_payload_source)
        ),
        Pattern::Discard => "_".to_owned(),
        Pattern::Tuple(items) => format!(
            "({})",
            items
                .iter()
                .map(pattern_schema_source)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Pattern::Record { path, fields, rest } => {
            let mut fields = fields
                .iter()
                .map(|field| {
                    format!(
                        "{}: {}",
                        field.name(),
                        pattern_schema_source(field.pattern())
                    )
                })
                .collect::<Vec<_>>();
            if *rest {
                fields.push("..".to_owned());
            }
            format!(
                "{}{{{}}}",
                path.as_ref().map_or("", String::as_str),
                fields.join(", ")
            )
        }
        Pattern::BracketSeq { items, rest } => {
            let mut items = items.iter().map(pattern_schema_source).collect::<Vec<_>>();
            if let Some(rest) = rest {
                items.push(format!("..{rest}"));
            }
            format!("[{}]", items.join(", "))
        }
        Pattern::Whole { name, pattern } => format!("{name} @ {}", pattern_schema_source(pattern)),
        Pattern::Typed { name, ty } => format!("{name}: {ty:?}"),
        Pattern::Raw(source) => source.clone(),
    }
}

fn variant_pattern_payload_source(
    payload: &arcweft_lang_syntax::ast::pattern::VariantPatternPayload,
) -> String {
    match payload {
        arcweft_lang_syntax::ast::pattern::VariantPatternPayload::Tuple(items) => format!(
            "({})",
            items
                .iter()
                .map(pattern_schema_source)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        arcweft_lang_syntax::ast::pattern::VariantPatternPayload::Record { fields, rest } => {
            let mut fields = fields
                .iter()
                .map(|field| {
                    format!(
                        "{}: {}",
                        field.name(),
                        pattern_schema_source(field.pattern())
                    )
                })
                .collect::<Vec<_>>();
            if *rest {
                fields.push("..".to_owned());
            }
            format!("{{{}}}", fields.join(", "))
        }
    }
}

fn usize_to_u32_saturating(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

fn first_part(modifiers: &[ViewModifier]) -> Option<String> {
    modifiers.iter().find_map(|modifier| match modifier {
        ViewModifier::Part(part) => Some(part.clone()),
        _ => None,
    })
}

fn next_text_source_id(view_id: &str, state: &mut ViewLoweringState) -> String {
    let id = format!("text.{view_id}.{}", state.text_counter);
    state.text_counter += 1;
    id
}

fn next_text_block_id(view_id: &str, state: &mut ViewLoweringState) -> String {
    let id = format!("text.block.{view_id}.{}", state.text_block_counter);
    state.text_block_counter += 1;
    id
}

fn next_input_id(view_id: &str, mode: ViewTextFieldMode, state: &mut ViewLoweringState) -> String {
    let id = format!(
        "input.{view_id}.{}.{}",
        text_field_mode_label(mode),
        state.input_counter
    );
    state.input_counter += 1;
    id
}

fn next_button_id(view_id: &str, state: &mut ViewLoweringState) -> String {
    let id = format!("button.{view_id}.{}", state.button_counter);
    state.button_counter += 1;
    id
}

fn next_image_id(view_id: &str, state: &mut ViewLoweringState) -> String {
    let id = format!("image.{view_id}.{}", state.image_counter);
    state.image_counter += 1;
    id
}

fn next_focus_group_id(view_id: &str, state: &mut ViewLoweringState) -> String {
    let id = format!("group.{view_id}.{}", state.group_counter);
    state.group_counter += 1;
    id
}

fn normalize_style_ref(reference: &EntityRefSyntax) -> String {
    normalize_entity_ref(reference)
}

fn normalize_entity_ref(reference: &EntityRefSyntax) -> String {
    reference.canonical_body()
}

fn image_source_object_id(expr: &Expr) -> Option<String> {
    match expr {
        Expr::EntityRef(reference) => {
            let id = normalize_entity_ref(reference);
            id.starts_with("image.").then_some(id)
        }
        Expr::Literal(Literal::String(value)) | Expr::Raw(value) => {
            let id = value.trim().trim_matches('"').trim_matches('\'');
            id.starts_with("image.").then(|| id.to_owned())
        }
        Expr::Path(value) => {
            let id = value.as_label();
            id.starts_with("image.").then(|| id.to_owned())
        }
        _ => None,
    }
}

fn lower_action_payload(payload: &ViewActionPayload) -> ViewActionPayloadResource {
    match payload {
        ViewActionPayload::LiteralString(value) => ViewActionPayloadResource::LiteralString {
            value: value.clone(),
        },
        ViewActionPayload::TextControlProjection { input, field } => {
            ViewActionPayloadResource::TextControlProjection {
                input: normalize_input_payload_ref(input),
                field: lower_text_control_payload_field(*field),
            }
        }
    }
}

fn normalize_input_payload_ref(input: &str) -> String {
    let input = input.trim().strip_prefix('@').unwrap_or(input.trim());
    let input = input.strip_prefix("input:.").unwrap_or(input);
    if input.starts_with("input.") {
        input.to_owned()
    } else {
        format!("input.{input}")
    }
}

fn lower_text_control_payload_field(
    field: ViewTextControlPayloadField,
) -> ViewActionTextControlPayloadField {
    match field {
        ViewTextControlPayloadField::Text => ViewActionTextControlPayloadField::Text,
        ViewTextControlPayloadField::Value => ViewActionTextControlPayloadField::Value,
    }
}

fn view_element_kind(value: &str) -> Option<ViewElementKind> {
    Some(match value {
        "Panel" => ViewElementKind::Panel,
        "Box" => ViewElementKind::Box,
        "Scroll" => ViewElementKind::Scroll,
        "Row" => ViewElementKind::Row,
        "Column" => ViewElementKind::Column,
        "LazyRow" => ViewElementKind::LazyRow,
        "LazyColumn" => ViewElementKind::LazyColumn,
        "Stack" => ViewElementKind::Stack,
        "Button" => ViewElementKind::Button,
        "TextField" => ViewElementKind::TextField,
        "TextArea" => ViewElementKind::TextArea,
        "SecureField" => ViewElementKind::SecureField,
        _ => return None,
    })
}

fn view_element_kind_for_text_field(mode: ViewTextFieldMode) -> ViewElementKind {
    match mode {
        ViewTextFieldMode::TextField => ViewElementKind::TextField,
        ViewTextFieldMode::TextArea => ViewElementKind::TextArea,
        ViewTextFieldMode::SecureField => ViewElementKind::SecureField,
    }
}

fn text_field_mode_callee(mode: ViewTextFieldMode) -> &'static str {
    match mode {
        ViewTextFieldMode::TextField => "TextField",
        ViewTextFieldMode::TextArea => "TextArea",
        ViewTextFieldMode::SecureField => "SecureField",
    }
}

fn view_input_kind(mode: ViewTextFieldMode) -> ViewInputKind {
    match mode {
        ViewTextFieldMode::TextField => ViewInputKind::TextField,
        ViewTextFieldMode::TextArea => ViewInputKind::TextArea,
        ViewTextFieldMode::SecureField => ViewInputKind::SecureField,
    }
}

fn text_field_mode_label(mode: ViewTextFieldMode) -> &'static str {
    match mode {
        ViewTextFieldMode::TextField => "text_field",
        ViewTextFieldMode::TextArea => "text_area",
        ViewTextFieldMode::SecureField => "secure_field",
    }
}

pub(in crate::app) fn expr_source(expr: &Expr) -> String {
    match expr {
        Expr::Literal(Literal::String(value)) | Expr::Raw(value) => value.clone(),
        Expr::Path(value) => value.as_label().to_owned(),
        Expr::ShortVariant(value) => format!(".{value}"),
        Expr::EntityRef(reference) => normalize_style_ref(reference),
        other => format!("{other:?}"),
    }
}

fn button_label_text(label: &ViewButtonLabel) -> String {
    match label {
        ViewButtonLabel::Literal(value) => value.clone(),
        ViewButtonLabel::Expr(expr) => expr_source(expr),
        ViewButtonLabel::Empty => String::new(),
    }
}

fn button_display_label(button: &ViewButton, button_id: &str) -> String {
    modifier_label(button.modifiers())
        .or_else(|| {
            let value = button_label_text(button.label());
            (!value.is_empty()).then_some(value)
        })
        .unwrap_or_else(|| fallback_label_from_public_id(button_id))
}

fn fallback_label_from_public_id(public_id: &str) -> String {
    public_id
        .rsplit('.')
        .next()
        .unwrap_or(public_id)
        .split('_')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().chain(chars).collect::<String>(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn button_enabled(enabled: Option<&Expr>) -> bool {
    match enabled {
        Some(Expr::Literal(Literal::Bool(value))) => *value,
        Some(_) | None => true,
    }
}

fn text_block_selection_policy(modifiers: &[ViewModifier]) -> ViewTextSelectionPolicy {
    modifiers
        .iter()
        .find_map(|modifier| match modifier {
            ViewModifier::Property { name, value }
                if matches!(
                    name.as_str(),
                    "selection" | "selection_policy" | "selectionPolicy"
                ) =>
            {
                symbol_expr_name(value)
                    .as_deref()
                    .map(|value| text_control_selection_policy(Some(value)))
            }
            ViewModifier::Property { name, value }
                if matches!(name.as_str(), "selectable" | "user_select" | "userSelect") =>
            {
                match value {
                    Expr::Literal(Literal::Bool(true)) => Some(ViewTextSelectionPolicy::Enabled),
                    Expr::Literal(Literal::Bool(false)) => Some(ViewTextSelectionPolicy::Disabled),
                    _ => symbol_expr_name(value)
                        .as_deref()
                        .map(|value| text_control_selection_policy(Some(value))),
                }
            }
            _ => None,
        })
        .unwrap_or(ViewTextSelectionPolicy::Disabled)
}

fn assign_action_button_bounds(state: &mut ViewLoweringState) {
    if state.action_buttons.is_empty() {
        return;
    }
    for (fallback_index, button) in state.action_buttons.iter_mut().enumerate() {
        if button.bounds.width_milli == 0 || button.bounds.height_milli == 0 {
            button.bounds = ViewRuntimeButtonBounds::default_slot(fallback_index);
        }
    }
}

fn register_input_handle_binding(view_let: &ViewLet, state: &mut ViewLoweringState) {
    let Some(name) = view_let.pattern().simple_binding_name() else {
        return;
    };
    let Some(binding) = input_handle_binding(name, view_let.value()) else {
        return;
    };
    if let Some(existing) = state
        .input_handle_bindings
        .iter_mut()
        .find(|existing| existing.name == binding.name)
    {
        *existing = binding;
    } else {
        state.input_handle_bindings.push(binding);
    }
}

fn input_handle_binding(name: &str, value: &Expr) -> Option<InputHandleBinding> {
    let args = match value {
        Expr::Call { callee, args }
            if expr_path_matches(callee, &["input", "text"])
                || expr_path_matches(callee, &["input", "secure"]) =>
        {
            args
        }
        _ => return None,
    };
    let input = first_positional_entity_arg(args)?;
    let initial_value = named_call_arg(args, &["initial", "value"])
        .map(input_handle_initial_value)
        .unwrap_or_default();
    Some(InputHandleBinding {
        name: name.to_owned(),
        public_id: normalize_input_payload_ref(&input.canonical_body()),
        initial_value,
    })
}

fn text_field_bound_input<'a>(
    field: &ViewTextField,
    state: &'a ViewLoweringState,
) -> Option<&'a InputHandleBinding> {
    if field.input().is_some() {
        return None;
    }
    let name = simple_path_name(field.value())?;
    state
        .input_handle_bindings
        .iter()
        .rev()
        .find(|binding| binding.name == name)
}

fn simple_path_name(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::Path(path) if path.segments().len() == 1 => Some(path.as_label()),
        _ => None,
    }
}

fn expr_path_matches(expr: &Expr, segments: &[&str]) -> bool {
    expr.dotted_selector_label()
        .is_some_and(|label| label.split('.').eq(segments.iter().copied()))
}

fn first_positional_entity_arg(args: &[CallArg]) -> Option<&EntityRefSyntax> {
    args.iter().find_map(|arg| match arg {
        CallArg::Positional(Expr::EntityRef(reference)) => Some(reference),
        CallArg::Positional(_) | CallArg::Named { .. } | CallArg::Spread { .. } => None,
    })
}

fn named_call_arg<'a>(args: &'a [CallArg], names: &[&str]) -> Option<&'a Expr> {
    args.iter().find_map(|arg| match arg {
        CallArg::Named { name, value } if names.contains(&name.as_str()) => Some(value.as_ref()),
        CallArg::Positional(_) | CallArg::Named { .. } | CallArg::Spread { .. } => None,
    })
}

fn input_handle_initial_value(expr: &Expr) -> String {
    match expr {
        Expr::Literal(Literal::String(value)) => value.clone(),
        _ => expr_source(expr),
    }
}

impl AuthoredTextControl {
    fn from_field(view_id: &str, field: &ViewTextField, state: &mut ViewLoweringState) -> Self {
        let binding = text_field_bound_input(field, state).cloned();
        let public_id = field
            .input()
            .map(normalize_entity_ref)
            .or_else(|| binding.as_ref().map(|binding| binding.public_id.clone()))
            .unwrap_or_else(|| next_input_id(view_id, field.mode(), state));
        let value = binding.as_ref().map_or_else(
            || expr_source(field.value()),
            |binding| binding.initial_value.clone(),
        );
        let purpose = text_control_symbol_arg(field.args(), &["purpose"])
            .or_else(|| modifier_symbol(field.modifiers(), modifier_purpose_expr));
        let enter_key = text_control_symbol_arg(field.args(), &["enter_key", "enterKey"])
            .or_else(|| modifier_symbol(field.modifiers(), modifier_enter_key_expr));
        let secure_policy =
            text_control_symbol_arg(field.args(), &["secure_policy", "securePolicy"]);
        let selection_policy = text_control_symbol_arg(
            field.args(),
            &["selection", "selection_policy", "selectionPolicy"],
        );
        let shortcut_policy = text_control_symbol_arg(
            field.args(),
            &["shortcuts", "shortcut_policy", "shortcutPolicy"],
        );
        let tab_policy = text_control_symbol_arg(field.args(), &["tab", "tab_policy", "tabPolicy"]);
        let vertical_navigation_policy = text_control_symbol_arg(
            field.args(),
            &[
                "vertical_navigation",
                "vertical_navigation_policy",
                "verticalNavigation",
                "verticalNavigationPolicy",
            ],
        );
        Self {
            public_id: public_id.clone(),
            value,
            label: text_control_text_arg(field.args(), &["label"])
                .or_else(|| modifier_label(field.modifiers())),
            placeholder: text_control_text_arg(field.args(), &["placeholder"])
                .or_else(|| modifier_placeholder(field.modifiers())),
            purpose: text_control_purpose(purpose.as_deref(), field.mode()),
            enter_key: text_control_enter_key(enter_key.as_deref()),
            multiline: text_control_bool_arg(field.args(), "multiline")
                .unwrap_or(field.mode() == ViewTextFieldMode::TextArea),
            selection_policy: text_control_selection_policy(selection_policy.as_deref()),
            shortcut_policy: text_control_shortcut_policy(shortcut_policy.as_deref()),
            tab_policy: text_control_tab_policy(tab_policy.as_deref()),
            vertical_navigation_policy: text_control_vertical_navigation_policy(
                vertical_navigation_policy.as_deref(),
            ),
            secure_policy: text_control_secure_policy(secure_policy.as_deref(), field.mode()),
            submit_handler: text_control_submit_action_handler(field)
                .or_else(|| text_control_handler_arg(field.args(), "submit"))
                .or_else(|| Some(public_id.clone())),
            change_handler: text_control_handler_arg(field.args(), "change")
                .or_else(|| Some(public_id.clone())),
        }
    }
}

fn input_text_source_id(kind: &str, public_id: &str) -> String {
    format!("text.{kind}.{public_id}")
}

fn text_control_text_arg(args: &[ViewArg], names: &[&str]) -> Option<String> {
    names
        .iter()
        .find_map(|name| text_control_arg(args, name))
        .map(expr_source)
}

fn text_control_symbol_arg(args: &[ViewArg], names: &[&str]) -> Option<String> {
    names
        .iter()
        .find_map(|name| text_control_arg(args, name))
        .and_then(symbol_expr_name)
}

fn modifier_symbol(
    modifiers: &[ViewModifier],
    select: impl Fn(&ViewModifier) -> Option<&Expr>,
) -> Option<String> {
    modifiers.iter().find_map(select).and_then(symbol_expr_name)
}

fn modifier_purpose_expr(modifier: &ViewModifier) -> Option<&Expr> {
    match modifier {
        ViewModifier::Purpose(expr) => Some(expr),
        _ => None,
    }
}

fn modifier_enter_key_expr(modifier: &ViewModifier) -> Option<&Expr> {
    match modifier {
        ViewModifier::EnterKey(expr) => Some(expr),
        _ => None,
    }
}

fn text_control_handler_arg(args: &[ViewArg], name: &str) -> Option<String> {
    text_control_arg(args, name).map(|expr| match expr {
        Expr::EntityRef(reference) => normalize_entity_ref(reference),
        expr => expr_source(expr),
    })
}

fn text_control_submit_action_handler(field: &ViewTextField) -> Option<String> {
    match field.submit_action()? {
        ViewAction::ActionInvoke(action) => Some(normalize_entity_ref(action.action())),
        ViewAction::Noop => None,
    }
}

fn text_control_bool_arg(args: &[ViewArg], name: &str) -> Option<bool> {
    match text_control_arg(args, name) {
        Some(Expr::Literal(Literal::Bool(value))) => Some(*value),
        _ => None,
    }
}

fn text_control_arg<'a>(args: &'a [ViewArg], name: &str) -> Option<&'a Expr> {
    args.iter().find_map(|arg| match arg {
        ViewArg::Named {
            name: actual,
            value,
        } if actual == name => Some(value),
        _ => None,
    })
}

fn modifier_placeholder(modifiers: &[ViewModifier]) -> Option<String> {
    modifiers.iter().find_map(|modifier| match modifier {
        ViewModifier::Placeholder(expr) => Some(expr_source(expr)),
        _ => None,
    })
}

fn modifier_label(modifiers: &[ViewModifier]) -> Option<String> {
    modifiers.iter().find_map(|modifier| match modifier {
        ViewModifier::Label(expr) => Some(expr_source(expr)),
        _ => None,
    })
}

fn symbol_expr_name(expr: &Expr) -> Option<String> {
    let value = expr_source(expr);
    let value = value.trim().trim_start_matches('.');
    (!value.is_empty()).then(|| value.to_owned())
}

fn text_control_purpose(value: Option<&str>, mode: ViewTextFieldMode) -> ViewInputPurpose {
    match value {
        Some("search") => ViewInputPurpose::Search,
        Some("name") => ViewInputPurpose::Name,
        Some("email") => ViewInputPurpose::Email,
        Some("url") => ViewInputPurpose::Url,
        Some("telephone" | "tel") => ViewInputPurpose::Telephone,
        Some("number") => ViewInputPurpose::Number,
        Some("decimal") => ViewInputPurpose::Decimal,
        Some("password") => ViewInputPurpose::Password,
        Some("pin") => ViewInputPurpose::Pin,
        Some("terminal") => ViewInputPurpose::Terminal,
        _ if mode == ViewTextFieldMode::SecureField => ViewInputPurpose::Password,
        _ => ViewInputPurpose::Text,
    }
}

fn text_control_enter_key(value: Option<&str>) -> EnterKeyHint {
    match value {
        Some("enter") => EnterKeyHint::Enter,
        Some("done") => EnterKeyHint::Done,
        Some("go") => EnterKeyHint::Go,
        Some("next") => EnterKeyHint::Next,
        Some("search") => EnterKeyHint::Search,
        Some("send") => EnterKeyHint::Send,
        _ => EnterKeyHint::Default,
    }
}

fn text_control_selection_policy(value: Option<&str>) -> ViewTextSelectionPolicy {
    match value {
        Some("disabled" | "none" | "false") => ViewTextSelectionPolicy::Disabled,
        _ => ViewTextSelectionPolicy::Enabled,
    }
}

fn text_control_shortcut_policy(value: Option<&str>) -> ViewTextShortcutPolicy {
    match value {
        Some("disabled" | "none" | "false") => ViewTextShortcutPolicy::Disabled,
        _ => ViewTextShortcutPolicy::Enabled,
    }
}

fn text_control_tab_policy(value: Option<&str>) -> ViewTextTabPolicy {
    match value {
        Some("insert" | "insert_tab" | "insertTab" | "text") => ViewTextTabPolicy::InsertTab,
        _ => ViewTextTabPolicy::FocusNavigation,
    }
}

fn text_control_vertical_navigation_policy(
    value: Option<&str>,
) -> ViewTextVerticalNavigationPolicy {
    match value {
        Some("visual" | "visual_line" | "visualLine" | "soft_wrap" | "softWrap") => {
            ViewTextVerticalNavigationPolicy::VisualLine
        }
        _ => ViewTextVerticalNavigationPolicy::LogicalLine,
    }
}

fn text_control_secure_policy(
    value: Option<&str>,
    mode: ViewTextFieldMode,
) -> ViewSecureInputPolicy {
    match value {
        Some("plain") => ViewSecureInputPolicy::Plain,
        Some("sensitive") => ViewSecureInputPolicy::Sensitive,
        Some("password") => ViewSecureInputPolicy::Password,
        Some("one_time_code" | "oneTimeCode" | "otp") => ViewSecureInputPolicy::OneTimeCode,
        _ if mode == ViewTextFieldMode::SecureField => ViewSecureInputPolicy::Password,
        _ => ViewSecureInputPolicy::Plain,
    }
}
