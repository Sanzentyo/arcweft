//! Lowers text-control authoring, bindings, input policy, and semantic metadata.

use super::{
    CallArg, CompositionOnBlurPolicy, EnterKeyHint, EntityRefSyntax, Expr, Literal,
    TextAssistPolicy, TextCapitalization, ViewAction, ViewActionTextControlPayloadField, ViewArg,
    ViewElementKind, ViewInputKind, ViewInputOptions, ViewInputPurpose, ViewLayoutBoundsResource,
    ViewLayoutCursor, ViewLayoutFrame, ViewLet, ViewLoweringState, ViewModifier,
    ViewProgramInstruction, ViewSecureInputPolicy, ViewSemanticTarget, ViewSidecarError,
    ViewTextControlPayloadField, ViewTextField, ViewTextFieldMode, ViewTextSelectionPolicy,
    ViewTextShortcutPolicy, ViewTextSourceKind, ViewTextSourceRecord, ViewTextTabPolicy,
    ViewTextVerticalNavigationPolicy, expr_source, first_part, lower_modifiers,
    lower_navigation_target, normalize_entity_ref, view_resource_id,
};

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
pub(super) struct InputHandleBinding {
    name: String,
    public_id: String,
    initial_value: String,
}

pub(super) fn lower_text_field(
    view_id: &str,
    field: &ViewTextField,
    state: &mut ViewLoweringState,
    layout: &mut ViewLayoutCursor,
) -> Result<ViewLayoutFrame, ViewSidecarError> {
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
    let styles = state.producer_styles(field.range());
    state
        .instructions
        .push(ViewProgramInstruction::OpenElement {
            element: view_element_kind_for_text_field(field.mode()),
            target: Some(public_id.clone()),
            styles,
            part: first_part(field.modifiers()),
            key: None,
            source: None,
        });
    lower_modifiers(view_id, field.modifiers(), state)?;
    state
        .instructions
        .push(ViewProgramInstruction::CloseElement);
    Ok(ViewLayoutFrame::text_control(kind))
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

pub(super) fn normalize_input_payload_ref(input: &str) -> String {
    let input = input.trim().strip_prefix('@').unwrap_or(input.trim());
    let input = input.strip_prefix("input:.").unwrap_or(input);
    if input.starts_with("input.") {
        input.to_owned()
    } else {
        format!("input.{input}")
    }
}

pub(super) fn lower_text_control_payload_field(
    field: ViewTextControlPayloadField,
) -> ViewActionTextControlPayloadField {
    match field {
        ViewTextControlPayloadField::Text => ViewActionTextControlPayloadField::Text,
        ViewTextControlPayloadField::Value => ViewActionTextControlPayloadField::Value,
    }
}

fn view_element_kind_for_text_field(mode: ViewTextFieldMode) -> ViewElementKind {
    match mode {
        ViewTextFieldMode::TextField => ViewElementKind::TextField,
        ViewTextFieldMode::TextArea => ViewElementKind::TextArea,
        ViewTextFieldMode::SecureField => ViewElementKind::SecureField,
    }
}

fn view_input_kind(mode: ViewTextFieldMode) -> ViewInputKind {
    ViewInputKind::from_element(view_element_kind_for_text_field(mode))
        .expect("every text-field mode owns a text-input View element")
}

fn text_field_mode_label(mode: ViewTextFieldMode) -> &'static str {
    view_element_kind_for_text_field(mode).runtime_label()
}

pub(super) fn register_input_handle_binding(view_let: &ViewLet, state: &mut ViewLoweringState) {
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
        ViewAction::Noop | ViewAction::Projection(_) => None,
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

pub(super) fn modifier_label(modifiers: &[ViewModifier]) -> Option<String> {
    modifiers.iter().find_map(|modifier| match modifier {
        ViewModifier::Label(expr) => Some(expr_source(expr)),
        _ => None,
    })
}

pub(super) fn symbol_expr_name(expr: &Expr) -> Option<String> {
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

pub(super) fn text_control_selection_policy(value: Option<&str>) -> ViewTextSelectionPolicy {
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
