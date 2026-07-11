//! Orchestrates view control-flow and layout lowering across responsibility modules.

mod content;
mod modifiers;
mod scroll;
mod text_controls;

use self::content::{assign_action_button_bounds, lower_button, lower_image, lower_text};
use self::modifiers::{
    lower_button_modifiers, lower_modifiers, lower_navigation_group, lower_navigation_target,
};
use self::scroll::lower_scroll_region;
pub(in crate::app) use self::scroll::{
    inline_style_properties, normalize_property_name, style_layout_length_u32,
};
use self::text_controls::{
    InputHandleBinding, lower_text_control_payload_field, lower_text_field, modifier_label,
    normalize_input_payload_ref, register_input_handle_binding, symbol_expr_name,
    text_control_selection_policy,
};

use arcweft_bundle::{
    BundleImageObject, BundleImageObjectBounds,
    container::BundleDigest,
    resource_codec::{
        ViewActionButtonActionResource, ViewActionButtonResource, ViewActionPayloadResource,
        ViewActionTextControlPayloadField, ViewAwaitBranchSpan, ViewFocusDirection,
        ViewFocusGroupPolicy, ViewFocusGroupResource, ViewFocusInitialPolicy,
        ViewFocusNavigationEdge, ViewFocusNavigationResource, ViewFocusSkipPolicy,
        ViewFocusTargetResolution, ViewFocusWrapPolicy, ViewFxArgumentBindingRef,
        ViewInputResource, ViewLayoutBoundsResource, ViewLogicalRect, ViewPartStyleRule,
        ViewProgramResource, ViewRuntimeButtonBounds, ViewRuntimeTextBlockBounds, ViewScrollAxis,
        ViewScrollIndicatorsPolicy, ViewScrollOverflowPolicy, ViewScrollOverscrollPolicy,
        ViewScrollRegionResource, ViewStyleResource, ViewSurfaceResource, ViewTextBlockResource,
        ViewTextResource,
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
        view::{
            ViewAction, ViewActionPayload, ViewArg, ViewAwait, ViewAwaitBranchKind, ViewBody,
            ViewButton, ViewButtonLabel, ViewElement, ViewExpr, ViewForEach, ViewFxApplication,
            ViewIf, ViewImage, ViewLet, ViewMatch, ViewMatchArm, ViewModifier,
            ViewNavigationDirection, ViewNavigationInitial, ViewNavigationTarget,
            ViewNavigationTrap, ViewStyleModifier, ViewText, ViewTextControlPayloadField,
            ViewTextField, ViewTextFieldMode,
        },
    },
    expr::{CallArg, Expr, Literal},
};
use arcweft_presentation::fx::FxId;
use arcweft_view::ViewElementLayoutKind;
use std::collections::BTreeMap;
use thiserror::Error;

use super::super::bundle_view_layout::{
    VIEW_LAYOUT_GAP_MILLI, VIEW_LAYOUT_SCROLL_VIEWPORT_HEIGHT_MILLI,
    VIEW_LAYOUT_TEXT_CONTROL_WIDTH_MILLI, ViewLayoutCursor, ViewLayoutFrame, button_bounds,
    modifier_layout_length_u32, named_arg, named_layout_length_u32, parse_px_milli,
    text_block_frame, u32_to_i32_saturating,
};
use super::super::bundle_view_overflow::validate_interactive_overflow_modifiers;
use super::super::bundle_view_schema::{
    expr_schema_ref, match_arm_schema_ref, pattern_schema_source, repeat_key_schema_ref,
    schema_ref_for_source,
};

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
    fx_ids: BTreeMap<String, FxId>,
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

pub(in crate::app) fn view_sidecars(
    views: &[&EntityDeclItem],
    style_resource: Option<&ViewStyleResource>,
    source_image_objects: &[BundleImageObject],
    fx_ids: BTreeMap<String, FxId>,
) -> Result<ViewBundleSidecars, ViewSidecarError> {
    let mut state = ViewLoweringState {
        fx_ids,
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
    if let Some(kind) = ViewElementKind::from_source_name(element.callee()) {
        validate_interactive_overflow_modifiers(
            element.callee(),
            element.modifiers(),
            kind.layout_kind() == Some(ViewElementLayoutKind::Scroll),
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
        let frame = match kind.layout_kind() {
            Some(ViewElementLayoutKind::Row) => {
                lower_layout_row(view_id, element.children(), state, *layout)?
            }
            Some(ViewElementLayoutKind::Column) => {
                lower_layout_column(view_id, element.children(), state, *layout)?
            }
            Some(ViewElementLayoutKind::Scroll) => {
                lower_scroll_region(view_id, element, state, *layout)?
            }
            Some(ViewElementLayoutKind::Stack) => {
                lower_layout_stack(view_id, element.children(), state, *layout)?
            }
            None if kind.is_action_control() => ViewLayoutFrame::action_button(),
            None => ViewInputKind::from_element(kind)
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

fn usize_to_u32_saturating(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

fn first_part(modifiers: &[ViewModifier]) -> Option<String> {
    modifiers.iter().find_map(|modifier| match modifier {
        ViewModifier::Part(part) => Some(part.clone()),
        _ => None,
    })
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

pub(in crate::app) fn expr_source(expr: &Expr) -> String {
    match expr {
        Expr::Literal(Literal::String(value)) | Expr::Raw(value) => value.clone(),
        Expr::Path(value) => value.as_label().to_owned(),
        Expr::ShortVariant(value) => format!(".{value}"),
        Expr::EntityRef(reference) => normalize_style_ref(reference),
        other => format!("{other:?}"),
    }
}

fn lower_fx_application(
    application: &ViewFxApplication,
    fx_ids: &BTreeMap<String, FxId>,
) -> Option<ViewProgramInstruction> {
    let Expr::Call { callee, args } = application.call() else {
        return None;
    };
    let function = callee.dotted_selector_label()?;
    let name = function.rsplit('.').next().unwrap_or(&function);
    let fx = fx_ids.get(name)?.clone();
    let arguments = args
        .iter()
        .filter_map(|argument| match argument {
            CallArg::Named { name, value } => Some(ViewFxArgumentBindingRef {
                parameter: name.clone(),
                value_schema: expr_schema_ref(value),
            }),
            CallArg::Positional(_) | CallArg::Spread { .. } => None,
        })
        .collect();
    Some(ViewProgramInstruction::ApplyFx {
        fx,
        arguments,
        key_schema: application.key().map(expr_schema_ref),
        application_ordinal: application.ordinal().get(),
        source: None,
    })
}
