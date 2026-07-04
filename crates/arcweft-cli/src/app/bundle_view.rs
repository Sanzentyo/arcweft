use arcweft_bundle::{
    container::BundleDigest,
    resource_codec::{
        UiActionButtonActionResource, UiActionButtonResource, UiFocusDirection, UiFocusGroupPolicy,
        UiFocusGroupResource, UiFocusInitialPolicy, UiFocusNavigationEdge,
        UiFocusNavigationResource, UiFocusSkipPolicy, UiFocusTargetResolution, UiFocusWrapPolicy,
        UiInputResource, UiProgramResource, UiRuntimeButtonBounds, UiRuntimeTextControlBounds,
        UiStyleResource, UiTextResource,
        ui::{
            CompositionOnBlurPolicy, EnterKeyHint, StyleSourceIdentity, StyleSourceRef,
            StyleSyntax, TextAssistPolicy, TextCapitalization, UiElementKind, UiInputKind,
            UiInputOptions, UiInputPurpose, UiProgramInstruction, UiSecureInputPolicy,
            UiSemanticTarget, UiStyleApplyRef, UiTextSourceKind, UiTextSourceRecord,
            UiTextSubmitImePolicy,
        },
    },
};
use arcweft_lang_syntax::{
    ast::{
        ids::{EntityRef, EntityRefSyntax},
        items::EntityDeclItem,
        view::{
            ComponentViewBody, ViewAction, ViewArg, ViewButton, ViewButtonLabel, ViewElement,
            ViewExpr, ViewImage, ViewModifier, ViewNavigationDirection, ViewNavigationInitial,
            ViewNavigationTarget, ViewNavigationTrap, ViewStyleModifier, ViewText, ViewTextField,
            ViewTextFieldMode, ViewTextSubmitImePolicy,
        },
    },
    expr::{Expr, Literal},
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
    secure_policy: UiSecureInputPolicy,
    submit_handler: Option<String>,
    change_handler: Option<String>,
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
    lower_view_expr(component_id.body(), body.value(), state);
}

fn lower_view_expr(component_id: &str, expr: &ViewExpr, state: &mut ViewLoweringState) {
    match expr {
        ViewExpr::Element(element) => lower_element(component_id, element, state),
        ViewExpr::Text(text) => lower_text(component_id, text, state),
        ViewExpr::TextField(field) => lower_text_field(component_id, field, state),
        ViewExpr::Button(button) => lower_button(component_id, button, state),
        ViewExpr::Image(image) => lower_image(image, state),
        ViewExpr::Fragment(children) => {
            for child in children {
                lower_view_expr(component_id, child, state);
            }
        }
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
        }
        ViewExpr::Raw(raw) => state.instructions.push(UiProgramInstruction::EmitCustom {
            element: raw.clone(),
            style: None,
            part: None,
            source: None,
        }),
        ViewExpr::If(_)
        | ViewExpr::Match(_)
        | ViewExpr::ForEach(_)
        | ViewExpr::Await(_)
        | ViewExpr::Expr(_) => {}
    }
}

fn lower_element(component_id: &str, element: &ViewElement, state: &mut ViewLoweringState) {
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
        for child in element.children() {
            lower_view_expr(component_id, child, state);
        }
        if pushed_group {
            state.focus_group_stack.pop();
        }
        state.instructions.push(UiProgramInstruction::CloseElement);
    } else {
        state.instructions.push(UiProgramInstruction::EmitCustom {
            element: element.callee().to_owned(),
            style: None,
            part: first_part(element.modifiers()),
            source: None,
        });
        lower_modifiers(component_id, element.modifiers(), state);
    }
}

fn lower_text(component_id: &str, text: &ViewText, state: &mut ViewLoweringState) {
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
}

fn lower_text_field(component_id: &str, field: &ViewTextField, state: &mut ViewLoweringState) {
    let control = AuthoredTextControl::from_field(component_id, field, state);
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
        kind: ui_input_kind(field.mode()),
        value_text_source,
        placeholder_text_source,
        purpose: control.purpose,
        autocorrect: TextAssistPolicy::PlatformDefault,
        spellcheck: TextAssistPolicy::PlatformDefault,
        capitalization: TextCapitalization::None,
        enter_key: control.enter_key,
        multiline: control.multiline,
        secure_policy: control.secure_policy,
        composition_on_blur: CompositionOnBlurPolicy::Commit,
        submit_handler: control.submit_handler,
        change_handler: control.change_handler,
        adapter_requirements: Vec::new(),
    });
    state.semantic_targets.push(UiSemanticTarget {
        public_id: control.public_id.clone(),
        target: control.public_id,
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
}

fn lower_button(component_id: &str, button: &ViewButton, state: &mut ViewLoweringState) {
    let button_id = button
        .id()
        .map_or_else(|| next_button_id(component_id, state), normalize_entity_ref);
    let label_text_source = format!("text.button.label.{button_id}");
    let label = button_label_text(button.label());
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

    let Some(ViewAction::TextSubmit(action)) = button.activation() else {
        return;
    };
    let input = normalize_entity_ref(action.input());
    state.action_buttons.push(UiActionButtonResource {
        public_id: button_id.clone(),
        label_text_source: label_text_source.clone(),
        enabled: button_enabled(button.enabled()),
        action: UiActionButtonActionResource::TextInputSubmit {
            input,
            ime_policy: lower_ime_policy(action.ime_policy()),
        },
        bounds: UiRuntimeButtonBounds::default_slot(state.action_buttons.len()),
        source: None,
    });
    state.semantic_targets.push(UiSemanticTarget {
        public_id: button_id.clone(),
        target: button_id,
        label_text_source: Some(label_text_source),
        source: None,
    });
}

fn lower_image(image: &ViewImage, state: &mut ViewLoweringState) {
    state.instructions.push(UiProgramInstruction::EmitImage {
        image: expr_source(image.source()),
        style: None,
        part: first_part(image.modifiers()),
        source: None,
    });
    lower_modifiers("", image.modifiers(), state);
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

fn ui_element_kind(value: &str) -> Option<UiElementKind> {
    Some(match value {
        "Surface" => UiElementKind::Surface,
        "Row" | "HStack" => UiElementKind::Row,
        "Column" | "VStack" => UiElementKind::Column,
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
        Expr::Literal(Literal::String(value)) | Expr::Path(value) | Expr::Raw(value) => {
            value.clone()
        }
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

fn button_enabled(enabled: Option<&Expr>) -> bool {
    match enabled {
        Some(Expr::Literal(Literal::Bool(value))) => *value,
        Some(_) | None => true,
    }
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
    let input_bounds = UiRuntimeTextControlBounds::default_stacked_slots(
        state.input_options.iter().map(|option| option.kind),
    );
    let mut submit_counts_by_input: Vec<(String, usize)> = Vec::new();
    for (fallback_index, button) in state.action_buttons.iter_mut().enumerate() {
        let UiActionButtonActionResource::TextInputSubmit { input, .. } = &button.action;
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
        button.bounds = UiRuntimeButtonBounds::default_submit_slot(
            input_bounds[input_index],
            input_option.kind,
            ordinal,
        );
    }
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
        Self {
            public_id,
            value: expr_source(field.value()),
            label: text_control_text_arg(field.args(), &["label"]),
            placeholder: text_control_text_arg(field.args(), &["placeholder"])
                .or_else(|| modifier_placeholder(field.modifiers())),
            purpose: text_control_purpose(purpose.as_deref(), field.mode()),
            enter_key: text_control_enter_key(enter_key.as_deref()),
            multiline: text_control_bool_arg(field.args(), "multiline")
                .unwrap_or(field.mode() == ViewTextFieldMode::TextArea),
            secure_policy: text_control_secure_policy(secure_policy.as_deref(), field.mode()),
            submit_handler: text_control_handler_arg(field.args(), "submit"),
            change_handler: text_control_handler_arg(field.args(), "change"),
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
