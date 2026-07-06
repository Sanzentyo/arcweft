use arcweft_bundle::{
    container::BundleDigest,
    resource_codec::{
        UiActionButtonActionResource, UiActionButtonResource, UiActionPayloadResource,
        UiActionTextControlPayloadField, UiFocusDirection, UiFocusGroupPolicy,
        UiFocusGroupResource, UiFocusInitialPolicy, UiFocusNavigationEdge,
        UiFocusNavigationResource, UiFocusSkipPolicy, UiFocusTargetResolution, UiFocusWrapPolicy,
        UiInputResource, UiLayoutBoundsResource, UiLogicalRect, UiProgramResource,
        UiRuntimeButtonBounds, UiRuntimeTextControlBounds, UiStyleResource, UiTextResource,
        types::DigestRef,
        ui::{
            CompositionOnBlurPolicy, EnterKeyHint, StyleSourceIdentity, StyleSourceRef,
            StyleSyntax, TextAssistPolicy, TextCapitalization, UiElementKind, UiInputKind,
            UiInputOptions, UiInputPurpose, UiProgramInstruction, UiSecureInputPolicy,
            UiSemanticTarget, UiStyleApplyRef, UiTextSelectionPolicy, UiTextShortcutPolicy,
            UiTextSourceKind, UiTextSourceRecord, UiTextSubmitImePolicy, UiTextTabPolicy,
            UiTextVerticalNavigationPolicy,
        },
    },
};
use arcweft_lang_syntax::{
    ast::{
        ids::{EntityRef, EntityRefSyntax},
        items::EntityDeclItem,
        pattern::Pattern,
        view::{
            ComponentViewBody, ViewAction, ViewActionPayload, ViewArg, ViewButton, ViewButtonLabel,
            ViewElement, ViewExpr, ViewForEach, ViewIf, ViewImage, ViewMatch, ViewMatchArm,
            ViewModifier, ViewNavigationDirection, ViewNavigationInitial, ViewNavigationTarget,
            ViewNavigationTrap, ViewStyleModifier, ViewText, ViewTextControlPayloadField,
            ViewTextField, ViewTextFieldMode, ViewTextSubmitImePolicy,
        },
    },
    expr::{Expr, Literal, UnitNumberSuffix},
};

#[derive(Clone, Debug, Default)]
pub(in crate::app) struct ComponentViewBundleSidecars {
    pub(in crate::app) program: Option<UiProgramResource>,
    pub(in crate::app) style: Option<UiStyleResource>,
    pub(in crate::app) text: Option<UiTextResource>,
    pub(in crate::app) input: Option<UiInputResource>,
}

#[derive(Default)]
struct ViewLoweringState {
    instructions: Vec<UiProgramInstruction>,
    text_sources: Vec<UiTextSourceRecord>,
    input_options: Vec<UiInputOptions>,
    semantic_targets: Vec<UiSemanticTarget>,
    layout_bounds: Vec<UiLayoutBoundsResource>,
    action_buttons: Vec<UiActionButtonResource>,
    focus_groups: Vec<UiFocusGroupResource>,
    focus_navigation: Vec<UiFocusNavigationResource>,
    focus_group_stack: Vec<String>,
    inline_arcweft_sources: Vec<StyleSourceIdentity>,
    inline_css_sources: Vec<StyleSourceIdentity>,
    text_counter: u32,
    input_counter: u32,
    button_counter: u32,
    group_counter: u32,
    handler_counter: u32,
    patch_counter: u32,
}

struct AuthoredTextControl {
    public_id: String,
    value: String,
    label: Option<String>,
    placeholder: Option<String>,
    purpose: UiInputPurpose,
    enter_key: EnterKeyHint,
    multiline: bool,
    selection_policy: UiTextSelectionPolicy,
    shortcut_policy: UiTextShortcutPolicy,
    tab_policy: UiTextTabPolicy,
    vertical_navigation_policy: UiTextVerticalNavigationPolicy,
    secure_policy: UiSecureInputPolicy,
    submit_handler: Option<String>,
    change_handler: Option<String>,
}

const VIEW_LAYOUT_ROOT_X_MILLI: i32 = 48_000;
const VIEW_LAYOUT_ROOT_Y_MILLI: i32 = 48_000;
const VIEW_LAYOUT_GAP_MILLI: i32 = 16_000;
const VIEW_LAYOUT_TEXT_CONTROL_WIDTH_MILLI: u32 = 420_000;
const VIEW_LAYOUT_TEXT_LINE_HEIGHT_MILLI: u32 = 24_000;
const VIEW_LAYOUT_BUTTON_WIDTH_MILLI: u32 = 180_000;
const VIEW_LAYOUT_BUTTON_HEIGHT_MILLI: u32 = 44_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ViewLayoutCursor {
    x_milli: i32,
    y_milli: i32,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct ViewLayoutFrame {
    width_milli: u32,
    height_milli: u32,
}

impl ViewLayoutCursor {
    const fn root() -> Self {
        Self {
            x_milli: VIEW_LAYOUT_ROOT_X_MILLI,
            y_milli: VIEW_LAYOUT_ROOT_Y_MILLI,
        }
    }

    const fn text_control_rect(self, kind: UiInputKind) -> UiLogicalRect {
        UiLogicalRect::new(
            self.x_milli,
            self.y_milli,
            VIEW_LAYOUT_TEXT_CONTROL_WIDTH_MILLI,
            kind.default_text_control_height_milli(),
        )
    }
}

impl ViewLayoutFrame {
    const fn zero() -> Self {
        Self {
            width_milli: 0,
            height_milli: 0,
        }
    }

    const fn new(width_milli: u32, height_milli: u32) -> Self {
        Self {
            width_milli,
            height_milli,
        }
    }

    const fn text_line() -> Self {
        Self::new(
            VIEW_LAYOUT_TEXT_CONTROL_WIDTH_MILLI,
            VIEW_LAYOUT_TEXT_LINE_HEIGHT_MILLI,
        )
    }

    const fn text_control(kind: UiInputKind) -> Self {
        Self::new(
            VIEW_LAYOUT_TEXT_CONTROL_WIDTH_MILLI,
            kind.default_text_control_height_milli(),
        )
    }

    const fn action_button() -> Self {
        Self::new(
            VIEW_LAYOUT_BUTTON_WIDTH_MILLI,
            VIEW_LAYOUT_BUTTON_HEIGHT_MILLI,
        )
    }

    const fn is_empty(self) -> bool {
        self.width_milli == 0 || self.height_milli == 0
    }
}

pub(in crate::app) fn component_view_sidecars(
    components: &[&EntityDeclItem],
) -> ComponentViewBundleSidecars {
    let mut state = ViewLoweringState::default();
    let Some(first) = components.first() else {
        return ComponentViewBundleSidecars::default();
    };
    for component in components {
        if let Some(body) = component.component_body().and_then(|body| body.view()) {
            lower_component_view(component.id(), body, &mut state);
        }
    }
    assign_action_button_bounds(&mut state);
    if state.instructions.is_empty()
        && state.text_sources.is_empty()
        && state.input_options.is_empty()
        && state.layout_bounds.is_empty()
        && state.action_buttons.is_empty()
        && state.focus_groups.is_empty()
        && state.focus_navigation.is_empty()
        && state.inline_arcweft_sources.is_empty()
        && state.inline_css_sources.is_empty()
    {
        return ComponentViewBundleSidecars::default();
    }
    ComponentViewBundleSidecars {
        program: Some(UiProgramResource {
            program_id: format!("ui.program.{}", first.id().body()),
            root_component: first.id().body().to_owned(),
            instructions: state.instructions,
            child_spans: Vec::new(),
            handlers: Vec::new(),
            state_schema_hashes: Vec::new(),
            exported_parts: Vec::new(),
            semantic_targets: state.semantic_targets,
            layout_bounds: state.layout_bounds,
            action_buttons: state.action_buttons,
            focus_groups: state.focus_groups,
            focus_navigation: state.focus_navigation,
            adapter_requirements: Vec::new(),
        }),
        style: (!state.inline_arcweft_sources.is_empty() || !state.inline_css_sources.is_empty())
            .then(|| UiStyleResource {
                style_program_id: "ui.style.inline.component_view".to_owned(),
                arcweft_sources: state.inline_arcweft_sources,
                css_sources: state.inline_css_sources,
                tokens: Vec::new(),
                rules: Vec::new(),
                part_rules: Vec::new(),
                environment_predicates: Vec::new(),
                source_map_refs: Vec::new(),
                external_css_descriptors: Vec::new(),
                adapter_requirements: Vec::new(),
            }),
        text: (!state.text_sources.is_empty()).then(|| UiTextResource {
            sources: state.text_sources,
            ..UiTextResource::default()
        }),
        input: (!state.input_options.is_empty()).then(|| UiInputResource {
            options: state.input_options,
            adapter_requirements: Vec::new(),
        }),
    }
}

fn lower_component_view(
    component_id: &EntityRef,
    body: &ComponentViewBody,
    state: &mut ViewLoweringState,
) {
    let mut layout = ViewLayoutCursor::root();
    lower_view_expr(component_id.body(), body.value(), state, &mut layout);
}

fn component_resource_id(component_id: &str) -> String {
    if component_id.starts_with("component.") {
        component_id.to_owned()
    } else {
        format!("component.{component_id}")
    }
}

fn lower_view_expr(
    component_id: &str,
    expr: &ViewExpr,
    state: &mut ViewLoweringState,
    layout: &mut ViewLayoutCursor,
) -> ViewLayoutFrame {
    match expr {
        ViewExpr::Element(element) => lower_element(component_id, element, state, layout),
        ViewExpr::Text(text) => lower_text(component_id, text, state),
        ViewExpr::TextField(field) => lower_text_field(component_id, field, state, layout),
        ViewExpr::Button(button) => lower_button(component_id, button, state, *layout),
        ViewExpr::Image(image) => lower_image(image, state),
        ViewExpr::Fragment(children) => lower_layout_column(component_id, children, state, *layout),
        ViewExpr::ComponentCall(call) => {
            state
                .instructions
                .push(UiProgramInstruction::CallComponent {
                    component: expr_source(call.component()),
                    child_span: 0,
                    props_schema: None,
                    style: None,
                    part: first_part(call.modifiers()),
                    key: None,
                    source: None,
                });
            lower_modifiers(component_id, call.modifiers(), state);
            ViewLayoutFrame::zero()
        }
        ViewExpr::Raw(raw) => {
            state.instructions.push(UiProgramInstruction::EmitCustom {
                element: raw.clone(),
                style: None,
                part: None,
                source: None,
            });
            ViewLayoutFrame::zero()
        }
        ViewExpr::If(view_if) => lower_view_if(component_id, view_if, state, layout),
        ViewExpr::Match(view_match) => lower_view_match(component_id, view_match, state, layout),
        ViewExpr::ForEach(view_for_each) => {
            lower_view_for_each(component_id, view_for_each, state, layout)
        }
        ViewExpr::Await(_) | ViewExpr::Expr(_) => ViewLayoutFrame::zero(),
    }
}

fn lower_view_if(
    component_id: &str,
    view_if: &ViewIf,
    state: &mut ViewLoweringState,
    layout: &mut ViewLayoutCursor,
) -> ViewLayoutFrame {
    let branch_index = state.instructions.len();
    state.instructions.push(UiProgramInstruction::Branch {
        condition_schema: expr_schema_ref(view_if.condition()),
        then_span: 0,
        else_span: None,
        source: None,
    });

    let then_start = state.instructions.len();
    let mut then_layout = *layout;
    let then_frame = lower_view_expr(component_id, view_if.then_branch(), state, &mut then_layout);
    let then_span = usize_to_u32_saturating(state.instructions.len().saturating_sub(then_start));

    let (else_frame, else_span) =
        view_if
            .else_branch()
            .map_or((ViewLayoutFrame::zero(), None), |branch| {
                let else_start = state.instructions.len();
                let mut else_layout = *layout;
                let frame = lower_view_expr(component_id, branch, state, &mut else_layout);
                let span =
                    usize_to_u32_saturating(state.instructions.len().saturating_sub(else_start));
                (frame, Some(span))
            });

    state.instructions[branch_index] = UiProgramInstruction::Branch {
        condition_schema: expr_schema_ref(view_if.condition()),
        then_span,
        else_span,
        source: None,
    };
    ViewLayoutFrame::new(
        then_frame.width_milli.max(else_frame.width_milli),
        then_frame.height_milli.max(else_frame.height_milli),
    )
}

fn lower_view_match(
    component_id: &str,
    view_match: &ViewMatch,
    state: &mut ViewLoweringState,
    layout: &mut ViewLayoutCursor,
) -> ViewLayoutFrame {
    lower_view_match_arms(
        component_id,
        view_match.scrutinee(),
        view_match.arms(),
        state,
        layout,
    )
}

fn lower_view_match_arms(
    component_id: &str,
    scrutinee: &Expr,
    arms: &[ViewMatchArm],
    state: &mut ViewLoweringState,
    layout: &mut ViewLayoutCursor,
) -> ViewLayoutFrame {
    let Some((arm, remaining)) = arms.split_first() else {
        return ViewLayoutFrame::zero();
    };
    let branch_index = state.instructions.len();
    state.instructions.push(UiProgramInstruction::Branch {
        condition_schema: match_arm_schema_ref(scrutinee, arm),
        then_span: 0,
        else_span: None,
        source: None,
    });

    let then_start = state.instructions.len();
    let mut then_layout = *layout;
    let then_frame = lower_view_expr(component_id, arm.value(), state, &mut then_layout);
    let then_span = usize_to_u32_saturating(state.instructions.len().saturating_sub(then_start));

    let (else_frame, else_span) = if remaining.is_empty() {
        (ViewLayoutFrame::zero(), None)
    } else {
        let else_start = state.instructions.len();
        let frame = lower_view_match_arms(component_id, scrutinee, remaining, state, layout);
        let span = usize_to_u32_saturating(state.instructions.len().saturating_sub(else_start));
        (frame, Some(span))
    };

    state.instructions[branch_index] = UiProgramInstruction::Branch {
        condition_schema: match_arm_schema_ref(scrutinee, arm),
        then_span,
        else_span,
        source: None,
    };
    ViewLayoutFrame::new(
        then_frame.width_milli.max(else_frame.width_milli),
        then_frame.height_milli.max(else_frame.height_milli),
    )
}

fn lower_view_for_each(
    component_id: &str,
    view_for_each: &ViewForEach,
    state: &mut ViewLoweringState,
    layout: &mut ViewLayoutCursor,
) -> ViewLayoutFrame {
    let repeat_index = state.instructions.len();
    state.instructions.push(UiProgramInstruction::RepeatKeyed {
        source_schema: expr_schema_ref(view_for_each.source()),
        key_schema: repeat_key_schema_ref(view_for_each),
        body_span: 0,
        source: None,
    });
    let body_start = state.instructions.len();
    let body_frame = lower_view_expr(component_id, view_for_each.body(), state, layout);
    let body_span = usize_to_u32_saturating(state.instructions.len().saturating_sub(body_start));
    state.instructions[repeat_index] = UiProgramInstruction::RepeatKeyed {
        source_schema: expr_schema_ref(view_for_each.source()),
        key_schema: repeat_key_schema_ref(view_for_each),
        body_span,
        source: None,
    };
    body_frame
}

fn lower_element(
    component_id: &str,
    element: &ViewElement,
    state: &mut ViewLoweringState,
    layout: &mut ViewLayoutCursor,
) -> ViewLayoutFrame {
    if let Some(kind) = ui_element_kind(element.callee()) {
        state.instructions.push(UiProgramInstruction::OpenElement {
            element: kind,
            style: None,
            part: first_part(element.modifiers()),
            key: None,
            source: None,
        });
        let pushed_group = lower_navigation_group(component_id, element, state);
        lower_modifiers(component_id, element.modifiers(), state);
        let frame = match kind {
            UiElementKind::Row => {
                lower_layout_row(component_id, element.children(), state, *layout)
            }
            UiElementKind::Column | UiElementKind::Scroll => {
                lower_layout_column(component_id, element.children(), state, *layout)
            }
            UiElementKind::Surface | UiElementKind::Box | UiElementKind::Stack => {
                lower_layout_stack(component_id, element.children(), state, *layout)
            }
            UiElementKind::Button => ViewLayoutFrame::action_button(),
            UiElementKind::TextField | UiElementKind::TextArea | UiElementKind::SecureField => kind
                .text_input_kind()
                .map_or(ViewLayoutFrame::zero(), ViewLayoutFrame::text_control),
        };
        if pushed_group {
            state.focus_group_stack.pop();
        }
        state.instructions.push(UiProgramInstruction::CloseElement);
        frame
    } else {
        state.instructions.push(UiProgramInstruction::EmitCustom {
            element: element.callee().to_owned(),
            style: None,
            part: first_part(element.modifiers()),
            source: None,
        });
        lower_modifiers(component_id, element.modifiers(), state);
        ViewLayoutFrame::zero()
    }
}

fn lower_layout_column(
    component_id: &str,
    children: &[ViewExpr],
    state: &mut ViewLoweringState,
    origin: ViewLayoutCursor,
) -> ViewLayoutFrame {
    let mut cursor = origin;
    let mut width_milli = 0_u32;
    let mut height_milli = 0_u32;
    let mut placed = false;
    for child in children {
        if placed {
            cursor.y_milli = cursor.y_milli.saturating_add(VIEW_LAYOUT_GAP_MILLI);
        }
        let frame = lower_view_expr(component_id, child, state, &mut cursor);
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
    ViewLayoutFrame::new(width_milli, height_milli)
}

fn lower_layout_row(
    component_id: &str,
    children: &[ViewExpr],
    state: &mut ViewLoweringState,
    origin: ViewLayoutCursor,
) -> ViewLayoutFrame {
    let mut cursor = origin;
    let mut width_milli = 0_u32;
    let mut height_milli = 0_u32;
    let mut placed = false;
    for child in children {
        if placed {
            cursor.x_milli = cursor.x_milli.saturating_add(VIEW_LAYOUT_GAP_MILLI);
        }
        let frame = lower_view_expr(component_id, child, state, &mut cursor);
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
    ViewLayoutFrame::new(width_milli, height_milli)
}

fn lower_layout_stack(
    component_id: &str,
    children: &[ViewExpr],
    state: &mut ViewLoweringState,
    origin: ViewLayoutCursor,
) -> ViewLayoutFrame {
    children
        .iter()
        .fold(ViewLayoutFrame::zero(), |frame, child| {
            let mut cursor = origin;
            let child_frame = lower_view_expr(component_id, child, state, &mut cursor);
            ViewLayoutFrame::new(
                frame.width_milli.max(child_frame.width_milli),
                frame.height_milli.max(child_frame.height_milli),
            )
        })
}

fn lower_text(
    component_id: &str,
    text: &ViewText,
    state: &mut ViewLoweringState,
) -> ViewLayoutFrame {
    let id = next_text_source_id(component_id, state);
    state.text_sources.push(UiTextSourceRecord {
        public_id: id.clone(),
        kind: UiTextSourceKind::Literal {
            value: expr_source(text.source()),
        },
        source: None,
    });
    state.instructions.push(UiProgramInstruction::EmitText {
        text_source: id,
        style: None,
        part: first_part(text.modifiers()),
        source: None,
    });
    lower_modifiers(component_id, text.modifiers(), state);
    ViewLayoutFrame::text_line()
}

fn lower_text_field(
    component_id: &str,
    field: &ViewTextField,
    state: &mut ViewLoweringState,
    layout: &mut ViewLayoutCursor,
) -> ViewLayoutFrame {
    let control = AuthoredTextControl::from_field(component_id, field, state);
    let public_id = control.public_id.clone();
    let kind = ui_input_kind(field.mode());
    let rect = layout.text_control_rect(kind);
    let value_text_source = input_text_source_id("value", &control.public_id);
    state.text_sources.push(UiTextSourceRecord {
        public_id: value_text_source.clone(),
        kind: UiTextSourceKind::Literal {
            value: control.value,
        },
        source: None,
    });
    let label_text_source = control.label.map(|label| {
        let id = input_text_source_id("label", &control.public_id);
        state.text_sources.push(UiTextSourceRecord {
            public_id: id.clone(),
            kind: UiTextSourceKind::Literal { value: label },
            source: None,
        });
        id
    });
    let placeholder_text_source = control.placeholder.map(|placeholder| {
        let id = input_text_source_id("placeholder", &control.public_id);
        state.text_sources.push(UiTextSourceRecord {
            public_id: id.clone(),
            kind: UiTextSourceKind::Literal { value: placeholder },
            source: None,
        });
        id
    });
    state.input_options.push(UiInputOptions {
        public_id: control.public_id.clone(),
        component: Some(component_resource_id(component_id)),
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
        .push(UiLayoutBoundsResource::text_control(
            public_id.clone(),
            rect,
        ));
    state
        .layout_bounds
        .push(UiLayoutBoundsResource::semantic_target(
            public_id.clone(),
            rect,
        ));
    state.semantic_targets.push(UiSemanticTarget {
        public_id: public_id.clone(),
        target: public_id,
        component: Some(component_resource_id(component_id)),
        label_text_source,
        source: None,
    });
    let target = state
        .semantic_targets
        .last()
        .map(|target| target.public_id.clone());
    if let Some(target) = target {
        lower_navigation_target(&target, field.modifiers(), state);
    }
    state.instructions.push(UiProgramInstruction::OpenElement {
        element: ui_element_kind_for_text_field(field.mode()),
        style: None,
        part: first_part(field.modifiers()),
        key: None,
        source: None,
    });
    lower_modifiers(component_id, field.modifiers(), state);
    state.instructions.push(UiProgramInstruction::CloseElement);
    ViewLayoutFrame::text_control(kind)
}

fn lower_button(
    component_id: &str,
    button: &ViewButton,
    state: &mut ViewLoweringState,
    layout: ViewLayoutCursor,
) -> ViewLayoutFrame {
    let button_id = button
        .id()
        .map_or_else(|| next_button_id(component_id, state), normalize_entity_ref);
    let label_text_source = format!("text.button.label.{button_id}");
    let label = button_display_label(button, &button_id);
    state.text_sources.push(UiTextSourceRecord {
        public_id: label_text_source.clone(),
        kind: UiTextSourceKind::Literal {
            value: label.clone(),
        },
        source: None,
    });
    state.instructions.push(UiProgramInstruction::OpenElement {
        element: UiElementKind::Button,
        style: None,
        part: first_part(button.modifiers()),
        key: None,
        source: None,
    });
    lower_button_modifiers(component_id, button.modifiers(), state);
    state.instructions.push(UiProgramInstruction::CloseElement);
    lower_navigation_target(&button_id, button.modifiers(), state);

    let action = match button.activation() {
        Some(ViewAction::TextSubmit(action)) => UiActionButtonActionResource::TextInputSubmit {
            input: normalize_entity_ref(action.input()),
            ime_policy: lower_ime_policy(action.ime_policy()),
        },
        Some(ViewAction::ActionInvoke(action)) => UiActionButtonActionResource::ActionInvoke {
            action: normalize_entity_ref(action.action()),
            payload: action.payload().map(lower_action_payload),
        },
        Some(ViewAction::Noop) | None => UiActionButtonActionResource::Noop,
    };
    state.action_buttons.push(UiActionButtonResource {
        public_id: button_id.clone(),
        component: Some(component_resource_id(component_id)),
        label_text_source: label_text_source.clone(),
        enabled: button_enabled(button.enabled()),
        action,
        bounds: button_bounds(button, layout),
        style: None,
        source: None,
    });
    state.semantic_targets.push(UiSemanticTarget {
        public_id: button_id.clone(),
        target: button_id,
        component: Some(component_resource_id(component_id)),
        label_text_source: Some(label_text_source),
        source: None,
    });
    ViewLayoutFrame::action_button()
}

fn lower_image(image: &ViewImage, state: &mut ViewLoweringState) -> ViewLayoutFrame {
    state.instructions.push(UiProgramInstruction::EmitImage {
        image: expr_source(image.source()),
        style: None,
        part: first_part(image.modifiers()),
        source: None,
    });
    lower_modifiers("", image.modifiers(), state);
    ViewLayoutFrame::zero()
}

fn lower_modifiers(component_id: &str, modifiers: &[ViewModifier], state: &mut ViewLoweringState) {
    for modifier in modifiers {
        match modifier {
            ViewModifier::Style(style) => {
                let style = lower_style_apply(component_id, style, state);
                state.instructions.push(UiProgramInstruction::ApplyStyle {
                    style,
                    source: None,
                });
            }
            ViewModifier::OnEvent { name, .. } => {
                let handler = format!("{component_id}.handler.{name}.{}", state.handler_counter);
                state.handler_counter += 1;
                state.instructions.push(UiProgramInstruction::BindHandler {
                    event: name.clone(),
                    handler,
                    source: None,
                });
            }
            ViewModifier::Part(_)
            | ViewModifier::Label(_)
            | ViewModifier::AgentTarget(_)
            | ViewModifier::Placeholder(_)
            | ViewModifier::SubmitAction(_)
            | ViewModifier::Enabled(_)
            | ViewModifier::Focusable(_)
            | ViewModifier::Environment(_)
            | ViewModifier::Focus(_)
            | ViewModifier::Navigation(_)
            | ViewModifier::Raw(_) => {}
        }
    }
}

fn lower_button_modifiers(
    component_id: &str,
    modifiers: &[ViewModifier],
    state: &mut ViewLoweringState,
) {
    for modifier in modifiers {
        match modifier {
            ViewModifier::Style(style) => {
                let style = lower_style_apply(component_id, style, state);
                state.instructions.push(UiProgramInstruction::ApplyStyle {
                    style,
                    source: None,
                });
            }
            ViewModifier::Part(_)
            | ViewModifier::Label(_)
            | ViewModifier::AgentTarget(_)
            | ViewModifier::Placeholder(_)
            | ViewModifier::SubmitAction(_)
            | ViewModifier::Enabled(_)
            | ViewModifier::Focusable(_)
            | ViewModifier::Environment(_)
            | ViewModifier::Focus(_)
            | ViewModifier::Navigation(_)
            | ViewModifier::Raw(_)
            | ViewModifier::OnEvent { .. } => {}
        }
    }
}

fn lower_navigation_group(
    component_id: &str,
    element: &ViewElement,
    state: &mut ViewLoweringState,
) -> bool {
    let Some(group) = element.navigation_group() else {
        return false;
    };
    let public_id = group.group().map_or_else(
        || next_focus_group_id(component_id, state),
        normalize_entity_ref,
    );
    let parent = group
        .parent()
        .map(normalize_entity_ref)
        .or_else(|| state.focus_group_stack.last().cloned());
    state.focus_groups.push(UiFocusGroupResource {
        public_id: public_id.clone(),
        parent,
        policy: lower_navigation_trap(group.trap()),
        initial: lower_navigation_initial(group.initial()),
        wrap: group.wrap().map_or(UiFocusWrapPolicy::Wrap, |wrap| {
            if wrap {
                UiFocusWrapPolicy::Wrap
            } else {
                UiFocusWrapPolicy::NoWrap
            }
        }),
        disabled_skip: UiFocusSkipPolicy::Skip,
        hidden_skip: UiFocusSkipPolicy::Skip,
        source: None,
    });
    state.focus_group_stack.push(public_id);
    true
}

fn lower_navigation_target(
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
            navigation.edges().iter().map(|edge| UiFocusNavigationEdge {
                direction: lower_navigation_direction(edge.direction()),
                target: lower_navigation_target_resolution(edge.target()),
                source: None,
            })
        })
        .collect::<Vec<_>>();
    if edges.is_empty() {
        return;
    }
    state.focus_navigation.push(UiFocusNavigationResource {
        public_id: public_id.to_owned(),
        group: state.focus_group_stack.last().cloned(),
        edges,
        source: None,
    });
}

fn lower_navigation_direction(direction: ViewNavigationDirection) -> UiFocusDirection {
    match direction {
        ViewNavigationDirection::Up => UiFocusDirection::Up,
        ViewNavigationDirection::Down => UiFocusDirection::Down,
        ViewNavigationDirection::Left => UiFocusDirection::Left,
        ViewNavigationDirection::Right => UiFocusDirection::Right,
        ViewNavigationDirection::Next => UiFocusDirection::Next,
        ViewNavigationDirection::Previous => UiFocusDirection::Previous,
    }
}

fn lower_navigation_target_resolution(target: &ViewNavigationTarget) -> UiFocusTargetResolution {
    match target {
        ViewNavigationTarget::Explicit(target) => UiFocusTargetResolution::Explicit {
            target: normalize_entity_ref(target),
        },
        ViewNavigationTarget::Auto => UiFocusTargetResolution::Auto,
        ViewNavigationTarget::None => UiFocusTargetResolution::None,
        ViewNavigationTarget::GroupBoundary => UiFocusTargetResolution::GroupBoundary,
    }
}

fn lower_navigation_initial(initial: &ViewNavigationInitial) -> UiFocusInitialPolicy {
    match initial {
        ViewNavigationInitial::Auto => UiFocusInitialPolicy::Auto,
        ViewNavigationInitial::First => UiFocusInitialPolicy::First,
        ViewNavigationInitial::Last => UiFocusInitialPolicy::Last,
        ViewNavigationInitial::Explicit(target) => UiFocusInitialPolicy::Explicit {
            target: normalize_entity_ref(target),
        },
        ViewNavigationInitial::None => UiFocusInitialPolicy::None,
    }
}

fn lower_navigation_trap(trap: ViewNavigationTrap) -> UiFocusGroupPolicy {
    match trap {
        ViewNavigationTrap::Normal => UiFocusGroupPolicy::Normal,
        ViewNavigationTrap::Trap => UiFocusGroupPolicy::Trap,
        ViewNavigationTrap::Modal => UiFocusGroupPolicy::Modal,
    }
}

fn lower_style_apply(
    component_id: &str,
    style: &ViewStyleModifier,
    state: &mut ViewLoweringState,
) -> UiStyleApplyRef {
    match style {
        ViewStyleModifier::Named(reference) => {
            UiStyleApplyRef::Named(normalize_style_ref(reference))
        }
        ViewStyleModifier::InlineArcweft(source) => {
            let patch_id = next_patch_id(component_id, source, StyleSyntax::Arcweft, state);
            UiStyleApplyRef::InlineArcweft { patch_id }
        }
        ViewStyleModifier::InlineCss(source) => {
            let patch_id = next_patch_id(component_id, source, StyleSyntax::Css, state);
            UiStyleApplyRef::InlineCss { patch_id }
        }
    }
}

fn next_patch_id(
    component_id: &str,
    source: &str,
    syntax: StyleSyntax,
    state: &mut ViewLoweringState,
) -> u32 {
    let patch_id = state.patch_counter;
    state.patch_counter += 1;
    let source_digest = BundleDigest::of(source.as_bytes());
    let identity = StyleSourceIdentity {
        public_id: format!("style.inline.{component_id}.{patch_id}"),
        syntax,
        identity: StyleSourceRef::Inline { source_digest },
        content_digest: Some(source_digest),
    };
    match syntax {
        StyleSyntax::Arcweft => state.inline_arcweft_sources.push(identity),
        StyleSyntax::Css => state.inline_css_sources.push(identity),
    }
    patch_id
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

fn next_text_source_id(component_id: &str, state: &mut ViewLoweringState) -> String {
    let id = format!("text.{component_id}.{}", state.text_counter);
    state.text_counter += 1;
    id
}

fn next_input_id(
    component_id: &str,
    mode: ViewTextFieldMode,
    state: &mut ViewLoweringState,
) -> String {
    let id = format!(
        "input.{component_id}.{}.{}",
        text_field_mode_label(mode),
        state.input_counter
    );
    state.input_counter += 1;
    id
}

fn next_button_id(component_id: &str, state: &mut ViewLoweringState) -> String {
    let id = format!("button.{component_id}.{}", state.button_counter);
    state.button_counter += 1;
    id
}

fn next_focus_group_id(component_id: &str, state: &mut ViewLoweringState) -> String {
    let id = format!("group.{component_id}.{}", state.group_counter);
    state.group_counter += 1;
    id
}

fn normalize_style_ref(reference: &EntityRefSyntax) -> String {
    normalize_entity_ref(reference)
}

fn normalize_entity_ref(reference: &EntityRefSyntax) -> String {
    reference.canonical_body()
}

fn lower_action_payload(payload: &ViewActionPayload) -> UiActionPayloadResource {
    match payload {
        ViewActionPayload::LiteralString(value) => UiActionPayloadResource::LiteralString {
            value: value.clone(),
        },
        ViewActionPayload::TextControlProjection { input, field } => {
            UiActionPayloadResource::TextControlProjection {
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
) -> UiActionTextControlPayloadField {
    match field {
        ViewTextControlPayloadField::Text => UiActionTextControlPayloadField::Text,
        ViewTextControlPayloadField::Value => UiActionTextControlPayloadField::Value,
    }
}

fn ui_element_kind(value: &str) -> Option<UiElementKind> {
    Some(match value {
        "Panel" => UiElementKind::Surface,
        "Box" => UiElementKind::Box,
        "Scroll" => UiElementKind::Scroll,
        "Row" => UiElementKind::Row,
        "Column" => UiElementKind::Column,
        "Stack" => UiElementKind::Stack,
        "Button" => UiElementKind::Button,
        "TextField" => UiElementKind::TextField,
        "TextArea" => UiElementKind::TextArea,
        "SecureField" => UiElementKind::SecureField,
        _ => return None,
    })
}

fn ui_element_kind_for_text_field(mode: ViewTextFieldMode) -> UiElementKind {
    match mode {
        ViewTextFieldMode::TextField => UiElementKind::TextField,
        ViewTextFieldMode::TextArea => UiElementKind::TextArea,
        ViewTextFieldMode::SecureField => UiElementKind::SecureField,
    }
}

fn ui_input_kind(mode: ViewTextFieldMode) -> UiInputKind {
    match mode {
        ViewTextFieldMode::TextField => UiInputKind::TextField,
        ViewTextFieldMode::TextArea => UiInputKind::TextArea,
        ViewTextFieldMode::SecureField => UiInputKind::SecureField,
    }
}

fn text_field_mode_label(mode: ViewTextFieldMode) -> &'static str {
    match mode {
        ViewTextFieldMode::TextField => "text_field",
        ViewTextFieldMode::TextArea => "text_area",
        ViewTextFieldMode::SecureField => "secure_field",
    }
}

fn expr_source(expr: &Expr) -> String {
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

fn button_bounds(button: &ViewButton, layout: ViewLayoutCursor) -> UiRuntimeButtonBounds {
    UiRuntimeButtonBounds::new(
        named_layout_length_i32(button.args(), &["x"]).unwrap_or(layout.x_milli),
        named_layout_length_i32(button.args(), &["y"]).unwrap_or(layout.y_milli),
        named_layout_length_u32(button.args(), &["width", "w"])
            .unwrap_or(VIEW_LAYOUT_BUTTON_WIDTH_MILLI),
        named_layout_length_u32(button.args(), &["height", "h"])
            .unwrap_or(VIEW_LAYOUT_BUTTON_HEIGHT_MILLI),
    )
}

fn named_layout_length_i32(args: &[ViewArg], names: &[&str]) -> Option<i32> {
    names
        .iter()
        .find_map(|name| named_arg(args, name))
        .and_then(expr_px_milli)
}

fn named_arg<'a>(args: &'a [ViewArg], name: &str) -> Option<&'a Expr> {
    args.iter().find_map(|arg| match arg {
        ViewArg::Named {
            name: actual,
            value,
        } if actual == name => Some(value),
        _ => None,
    })
}

fn named_layout_length_u32(args: &[ViewArg], names: &[&str]) -> Option<u32> {
    named_layout_length_i32(args, names).and_then(|value| u32::try_from(value.max(0)).ok())
}

fn expr_px_milli(expr: &Expr) -> Option<i32> {
    match expr {
        Expr::Literal(Literal::UnitNumber {
            raw,
            suffix: UnitNumberSuffix::Px,
        }) => parse_px_milli(raw),
        Expr::Literal(Literal::Int { value, .. }) => {
            i32::try_from(value.saturating_mul(1_000)).ok()
        }
        Expr::Raw(value) => value
            .trim()
            .strip_suffix("px")
            .map(str::trim)
            .and_then(parse_px_milli),
        Expr::Path(value) => value
            .as_label()
            .trim()
            .strip_suffix("px")
            .map(str::trim)
            .and_then(parse_px_milli),
        _ => None,
    }
}

fn parse_px_milli(raw: &str) -> Option<i32> {
    let source = raw.trim().replace('_', "");
    let (negative, unsigned) = source
        .strip_prefix('-')
        .map_or((false, source.as_str()), |rest| (true, rest));
    let unsigned = unsigned.strip_prefix('+').unwrap_or(unsigned);
    let (whole, fraction) = unsigned.split_once('.').unwrap_or((unsigned, ""));
    if whole.is_empty() && fraction.is_empty() {
        return None;
    }
    if !whole.chars().all(|ch| ch.is_ascii_digit())
        || !fraction.chars().all(|ch| ch.is_ascii_digit())
    {
        return None;
    }

    let whole_milli = if whole.is_empty() {
        0
    } else {
        whole.parse::<i64>().ok()?.checked_mul(1_000)?
    };
    let (fraction_milli, round_up) = fractional_px_milli(fraction)?;
    let magnitude = whole_milli
        .checked_add(fraction_milli)?
        .checked_add(i64::from(round_up))?;
    let signed = if negative { -magnitude } else { magnitude };
    i32::try_from(signed).ok()
}

fn fractional_px_milli(fraction: &str) -> Option<(i64, bool)> {
    let mut milli = 0_i64;
    let mut scale = 100_i64;
    for digit in fraction.chars().take(3) {
        let value = i64::from(digit.to_digit(10)?);
        milli = milli.checked_add(value.checked_mul(scale)?)?;
        scale /= 10;
    }
    let round_up = fraction
        .chars()
        .nth(3)
        .and_then(|digit| digit.to_digit(10))
        .is_some_and(|digit| digit >= 5);
    Some((milli, round_up))
}

fn lower_ime_policy(policy: ViewTextSubmitImePolicy) -> UiTextSubmitImePolicy {
    match policy {
        ViewTextSubmitImePolicy::Commit => UiTextSubmitImePolicy::Commit,
        ViewTextSubmitImePolicy::Cancel => UiTextSubmitImePolicy::Cancel,
        ViewTextSubmitImePolicy::Reject => UiTextSubmitImePolicy::Reject,
    }
}

fn assign_action_button_bounds(state: &mut ViewLoweringState) {
    if state.action_buttons.is_empty() || state.input_options.is_empty() {
        return;
    }
    let fallback_input_bounds = UiRuntimeTextControlBounds::default_stacked_slots(
        state.input_options.iter().map(|option| option.kind),
    );
    let mut submit_counts_by_input: Vec<(String, usize)> = Vec::new();
    for (fallback_index, button) in state.action_buttons.iter_mut().enumerate() {
        let UiActionButtonActionResource::TextInputSubmit { input, .. } = &button.action else {
            button.bounds = if button.bounds.width_milli == 0 || button.bounds.height_milli == 0 {
                UiRuntimeButtonBounds::default_slot(fallback_index)
            } else {
                button.bounds
            };
            continue;
        };
        let Some((input_index, input_option)) = state
            .input_options
            .iter()
            .enumerate()
            .find(|(_, option)| option.public_id == *input)
        else {
            button.bounds = UiRuntimeButtonBounds::default_slot(fallback_index);
            continue;
        };
        let ordinal = action_button_submit_ordinal(&mut submit_counts_by_input, input);
        let input_bounds = state
            .layout_bounds
            .iter()
            .find(|bounds| bounds.is_text_control_for(input))
            .map_or(
                fallback_input_bounds[input_index],
                UiLayoutBoundsResource::runtime_text_control_bounds,
            );
        button.bounds =
            UiRuntimeButtonBounds::default_submit_slot(input_bounds, input_option.kind, ordinal);
    }
}

fn u32_to_i32_saturating(value: u32) -> i32 {
    i32::try_from(value).unwrap_or(i32::MAX)
}

fn action_button_submit_ordinal(counts: &mut Vec<(String, usize)>, input: &str) -> usize {
    if let Some((_, count)) = counts.iter_mut().find(|(public_id, _)| public_id == input) {
        let ordinal = *count;
        *count = count.saturating_add(1);
        return ordinal;
    }
    counts.push((input.to_owned(), 1));
    0
}

impl AuthoredTextControl {
    fn from_field(
        component_id: &str,
        field: &ViewTextField,
        state: &mut ViewLoweringState,
    ) -> Self {
        let public_id = field.input().map_or_else(
            || next_input_id(component_id, field.mode(), state),
            normalize_entity_ref,
        );
        let purpose = text_control_symbol_arg(field.args(), &["purpose"]);
        let enter_key = text_control_symbol_arg(field.args(), &["enter_key", "enterKey"])
            .or_else(|| modifier_submit_action(field.modifiers()));
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
            value: expr_source(field.value()),
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
            submit_handler: text_control_handler_arg(field.args(), "submit")
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

fn text_control_handler_arg(args: &[ViewArg], name: &str) -> Option<String> {
    text_control_arg(args, name).map(|expr| match expr {
        Expr::EntityRef(reference) => normalize_entity_ref(reference),
        expr => expr_source(expr),
    })
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

fn modifier_submit_action(modifiers: &[ViewModifier]) -> Option<String> {
    modifiers.iter().find_map(|modifier| match modifier {
        ViewModifier::SubmitAction(expr) => symbol_expr_name(expr),
        _ => None,
    })
}

fn symbol_expr_name(expr: &Expr) -> Option<String> {
    let value = expr_source(expr);
    let value = value.trim().trim_start_matches('.');
    (!value.is_empty()).then(|| value.to_owned())
}

fn text_control_purpose(value: Option<&str>, mode: ViewTextFieldMode) -> UiInputPurpose {
    match value {
        Some("search") => UiInputPurpose::Search,
        Some("name") => UiInputPurpose::Name,
        Some("email") => UiInputPurpose::Email,
        Some("url") => UiInputPurpose::Url,
        Some("telephone" | "tel") => UiInputPurpose::Telephone,
        Some("number") => UiInputPurpose::Number,
        Some("decimal") => UiInputPurpose::Decimal,
        Some("password") => UiInputPurpose::Password,
        Some("pin") => UiInputPurpose::Pin,
        Some("terminal") => UiInputPurpose::Terminal,
        _ if mode == ViewTextFieldMode::SecureField => UiInputPurpose::Password,
        _ => UiInputPurpose::Text,
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

fn text_control_selection_policy(value: Option<&str>) -> UiTextSelectionPolicy {
    match value {
        Some("disabled" | "none" | "false") => UiTextSelectionPolicy::Disabled,
        _ => UiTextSelectionPolicy::Enabled,
    }
}

fn text_control_shortcut_policy(value: Option<&str>) -> UiTextShortcutPolicy {
    match value {
        Some("disabled" | "none" | "false") => UiTextShortcutPolicy::Disabled,
        _ => UiTextShortcutPolicy::Enabled,
    }
}

fn text_control_tab_policy(value: Option<&str>) -> UiTextTabPolicy {
    match value {
        Some("insert" | "insert_tab" | "insertTab" | "text") => UiTextTabPolicy::InsertTab,
        _ => UiTextTabPolicy::FocusNavigation,
    }
}

fn text_control_vertical_navigation_policy(value: Option<&str>) -> UiTextVerticalNavigationPolicy {
    match value {
        Some("visual" | "visual_line" | "visualLine" | "soft_wrap" | "softWrap") => {
            UiTextVerticalNavigationPolicy::VisualLine
        }
        _ => UiTextVerticalNavigationPolicy::LogicalLine,
    }
}

fn text_control_secure_policy(value: Option<&str>, mode: ViewTextFieldMode) -> UiSecureInputPolicy {
    match value {
        Some("plain") => UiSecureInputPolicy::Plain,
        Some("sensitive") => UiSecureInputPolicy::Sensitive,
        Some("password") => UiSecureInputPolicy::Password,
        Some("one_time_code" | "oneTimeCode" | "otp") => UiSecureInputPolicy::OneTimeCode,
        _ if mode == ViewTextFieldMode::SecureField => UiSecureInputPolicy::Password,
        _ => UiSecureInputPolicy::Plain,
    }
}
