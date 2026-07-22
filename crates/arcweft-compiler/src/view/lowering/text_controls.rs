//! Lowers text-control authoring, bindings, input policy, and semantic metadata.

use super::{
    CallArg, CompositionOnBlurPolicy, EnterKeyHint, EntityRefSyntax, Expr, Literal,
    TextAssistPolicy, TextCapitalization, ViewAction, ViewActionTextControlPayloadField, ViewArg,
    ViewElementKind, ViewInputKind, ViewInputOptions, ViewInputPurpose, ViewLayoutBoundsResource,
    ViewLayoutCursor, ViewLayoutFrame, ViewLet, ViewLoweringState, ViewModifier,
    ViewProgramInstruction, ViewSecureInputPolicy, ViewSemanticTarget, ViewSidecarError,
    ViewTextControlPayloadField, ViewTextField, ViewTextFieldMode, ViewTextSelectionPolicy,
    ViewTextShortcutPolicy, ViewTextSourceKind, ViewTextSourceRecord, ViewTextTabPolicy,
    ViewTextVerticalNavigationPolicy, first_part, literal_text_source, lower_navigation_target,
    lower_text_control_modifiers, normalize_entity_ref, static_symbol_source, view_resource_id,
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

struct AuthoredTextControlPolicies {
    purpose: ViewInputPurpose,
    enter_key: EnterKeyHint,
    selection: ViewTextSelectionPolicy,
    shortcuts: ViewTextShortcutPolicy,
    tab: ViewTextTabPolicy,
    vertical_navigation: ViewTextVerticalNavigationPolicy,
    secure: ViewSecureInputPolicy,
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
    let control = AuthoredTextControl::from_field(view_id, field, state)?;
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
            part: first_part(field.modifiers())?,
            key: None,
            source: None,
        });
    lower_text_control_modifiers(
        view_id,
        field.modifiers(),
        field.submit_action().is_some(),
        state,
    )?;
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

pub(super) fn register_input_handle_binding(
    view_let: &ViewLet,
    state: &mut ViewLoweringState,
) -> Result<bool, ViewSidecarError> {
    let Some(name) = view_let.pattern().simple_binding_name() else {
        return Ok(false);
    };
    let Some(binding) = input_handle_binding(name, view_let.value())? else {
        return Ok(false);
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
    Ok(true)
}

fn input_handle_binding(
    name: &str,
    value: &Expr,
) -> Result<Option<InputHandleBinding>, ViewSidecarError> {
    let args = match value {
        Expr::Call(call)
            if expr_path_matches(call.callee(), &["input", "text"])
                || expr_path_matches(call.callee(), &["input", "secure"]) =>
        {
            call.args()
        }
        _ => return Ok(None),
    };
    let Some(input) = first_positional_entity_arg(args) else {
        return Ok(None);
    };
    let initial_value = named_call_arg(args, &["initial", "value"])
        .map(input_handle_initial_value)
        .transpose()?
        .unwrap_or_default();
    Ok(Some(InputHandleBinding {
        name: name.to_owned(),
        public_id: normalize_input_payload_ref(&input.canonical_body()),
        initial_value,
    }))
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
        CallArg::Positional(value) => match value.as_ref() {
            Expr::EntityRef(reference) => Some(reference),
            _ => None,
        },
        CallArg::Named { .. } | CallArg::Spread { .. } => None,
    })
}

fn named_call_arg<'a>(args: &'a [CallArg], names: &[&str]) -> Option<&'a Expr> {
    args.iter().find_map(|arg| match arg {
        CallArg::Named { name, value } if names.contains(&name.as_str()) => Some(value.as_ref()),
        CallArg::Positional(_) | CallArg::Named { .. } | CallArg::Spread { .. } => None,
    })
}

fn input_handle_initial_value(expr: &Expr) -> Result<String, ViewSidecarError> {
    match expr {
        Expr::Literal(Literal::String(value)) => Ok(value.clone()),
        _ => literal_text_source(expr, "text input initial value"),
    }
}

impl AuthoredTextControl {
    fn from_field(
        view_id: &str,
        field: &ViewTextField,
        state: &mut ViewLoweringState,
    ) -> Result<Self, ViewSidecarError> {
        let binding = text_field_bound_input(field, state).cloned();
        let public_id = field
            .input()
            .map(normalize_entity_ref)
            .or_else(|| binding.as_ref().map(|binding| binding.public_id.clone()))
            .unwrap_or_else(|| next_input_id(view_id, field.mode(), state));
        let value = match binding.as_ref() {
            Some(binding) => binding.initial_value.clone(),
            None => literal_text_source(field.value(), "text input value")?,
        };
        let policies = AuthoredTextControlPolicies::from_field(view_id, field)?;
        Ok(Self {
            public_id: public_id.clone(),
            value,
            label: match text_control_text_arg(field.args(), &["label"])? {
                Some(label) => Some(label),
                None => modifier_label(field.modifiers())?,
            },
            placeholder: match text_control_text_arg(field.args(), &["placeholder"])? {
                Some(placeholder) => Some(placeholder),
                None => modifier_placeholder(field.modifiers())?,
            },
            purpose: policies.purpose,
            enter_key: policies.enter_key,
            multiline: text_control_bool_arg(field.args(), "multiline")?
                .unwrap_or(field.mode() == ViewTextFieldMode::TextArea),
            selection_policy: policies.selection,
            shortcut_policy: policies.shortcuts,
            tab_policy: policies.tab,
            vertical_navigation_policy: policies.vertical_navigation,
            secure_policy: policies.secure,
            submit_handler: match text_control_submit_action_handler(field) {
                Some(handler) => Some(handler),
                None => text_control_handler_arg(field.args(), "submit")?
                    .or_else(|| Some(public_id.clone())),
            },
            change_handler: text_control_handler_arg(field.args(), "change")?
                .or_else(|| Some(public_id.clone())),
        })
    }
}

impl AuthoredTextControlPolicies {
    fn from_field(view_id: &str, field: &ViewTextField) -> Result<Self, ViewSidecarError> {
        let purpose = text_control_symbol_arg(field.args(), &["purpose"])?
            .or(modifier_symbol(field.modifiers(), modifier_purpose_expr)?);
        let enter_key = text_control_symbol_arg(field.args(), &["enter_key", "enterKey"])?
            .or(modifier_symbol(field.modifiers(), modifier_enter_key_expr)?);
        let secure = field_policy_symbol(field, &["secure_policy", "securePolicy"])?;
        let selection =
            field_policy_symbol(field, &["selection", "selection_policy", "selectionPolicy"])?;
        let shortcuts =
            field_policy_symbol(field, &["shortcuts", "shortcut_policy", "shortcutPolicy"])?;
        let tab = field_policy_symbol(field, &["tab", "tab_policy", "tabPolicy"])?;
        let vertical_navigation = field_policy_symbol(
            field,
            &[
                "vertical_navigation",
                "vertical_navigation_policy",
                "verticalNavigation",
                "verticalNavigationPolicy",
            ],
        )?;
        Ok(Self {
            purpose: text_control_purpose(view_id, purpose.as_deref(), field.mode())?,
            enter_key: text_control_enter_key(view_id, enter_key.as_deref())?,
            selection: text_control_selection_policy(view_id, selection.as_deref())?,
            shortcuts: text_control_shortcut_policy(view_id, shortcuts.as_deref())?,
            tab: text_control_tab_policy(view_id, tab.as_deref())?,
            vertical_navigation: text_control_vertical_navigation_policy(
                view_id,
                vertical_navigation.as_deref(),
            )?,
            secure: text_control_secure_policy(view_id, secure.as_deref(), field.mode())?,
        })
    }
}

fn field_policy_symbol(
    field: &ViewTextField,
    names: &[&str],
) -> Result<Option<String>, ViewSidecarError> {
    let argument = text_control_symbol_arg(field.args(), names)?;
    Ok(argument.or(modifier_symbol(field.modifiers(), |modifier| {
        modifier_property_expr(modifier, names)
    })?))
}

fn input_text_source_id(kind: &str, public_id: &str) -> String {
    format!("text.{kind}.{public_id}")
}

fn text_control_text_arg(
    args: &[ViewArg],
    names: &[&str],
) -> Result<Option<String>, ViewSidecarError> {
    names
        .iter()
        .find_map(|name| text_control_arg(args, name))
        .map(|expr| literal_text_source(expr, "text control label or placeholder"))
        .transpose()
}

fn text_control_symbol_arg(
    args: &[ViewArg],
    names: &[&str],
) -> Result<Option<String>, ViewSidecarError> {
    names
        .iter()
        .find_map(|name| text_control_arg(args, name))
        .map(|expr| text_control_policy_symbol(expr, "text control policy"))
        .transpose()
}

fn modifier_symbol(
    modifiers: &[ViewModifier],
    select: impl Fn(&ViewModifier) -> Option<&Expr>,
) -> Result<Option<String>, ViewSidecarError> {
    modifiers
        .iter()
        .find_map(select)
        .map(|expr| text_control_policy_symbol(expr, "text control modifier policy"))
        .transpose()
}

fn text_control_policy_symbol(
    expr: &Expr,
    context: &'static str,
) -> Result<String, ViewSidecarError> {
    let value = static_symbol_source(expr, context)?;
    let value = value.trim().trim_start_matches('.');
    if value.is_empty() {
        Err(ViewSidecarError::UnsupportedStaticExpression { context })
    } else {
        Ok(value.to_owned())
    }
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

fn modifier_property_expr<'a>(modifier: &'a ViewModifier, names: &[&str]) -> Option<&'a Expr> {
    match modifier {
        ViewModifier::Property { name, value } if names.contains(&name.as_str()) => Some(value),
        _ => None,
    }
}

fn text_control_handler_arg(
    args: &[ViewArg],
    name: &str,
) -> Result<Option<String>, ViewSidecarError> {
    text_control_arg(args, name)
        .map(|expr| match expr {
            Expr::EntityRef(reference) => Ok(normalize_entity_ref(reference)),
            expr => static_symbol_source(expr, "text control handler"),
        })
        .transpose()
}

fn text_control_submit_action_handler(field: &ViewTextField) -> Option<String> {
    match field.submit_action()? {
        ViewAction::ActionInvoke(action) => Some(normalize_entity_ref(action.action())),
        ViewAction::Noop | ViewAction::Projection(_) => None,
    }
}

fn text_control_bool_arg(
    args: &[ViewArg],
    name: &'static str,
) -> Result<Option<bool>, ViewSidecarError> {
    match text_control_arg(args, name) {
        Some(Expr::Literal(Literal::Bool(value))) => Ok(Some(*value)),
        Some(_) => Err(ViewSidecarError::UnsupportedStaticBoolean { context: name }),
        None => Ok(None),
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

fn modifier_placeholder(modifiers: &[ViewModifier]) -> Result<Option<String>, ViewSidecarError> {
    modifiers
        .iter()
        .find_map(|modifier| match modifier {
            ViewModifier::Placeholder(expr) => Some(expr),
            _ => None,
        })
        .map(|expr| literal_text_source(expr, "text control placeholder"))
        .transpose()
}

pub(super) fn modifier_label(
    modifiers: &[ViewModifier],
) -> Result<Option<String>, ViewSidecarError> {
    modifiers
        .iter()
        .find_map(|modifier| match modifier {
            ViewModifier::Label(expr) => Some(expr),
            _ => None,
        })
        .map(|expr| literal_text_source(expr, "View label"))
        .transpose()
}

pub(super) fn symbol_expr_name(expr: &Expr) -> Option<String> {
    let value = match expr {
        Expr::Literal(Literal::String(value)) => value.clone(),
        Expr::Path(value) => value.as_label().to_owned(),
        Expr::ShortVariant(value) => format!(".{value}"),
        Expr::EntityRef(reference) => normalize_entity_ref(reference),
        _ => return None,
    };
    let value = value.trim().trim_start_matches('.');
    (!value.is_empty()).then(|| value.to_owned())
}

fn text_control_purpose(
    view_id: &str,
    value: Option<&str>,
    mode: ViewTextFieldMode,
) -> Result<ViewInputPurpose, ViewSidecarError> {
    match value {
        Some("search") => Ok(ViewInputPurpose::Search),
        Some("name") => Ok(ViewInputPurpose::Name),
        Some("email") => Ok(ViewInputPurpose::Email),
        Some("url") => Ok(ViewInputPurpose::Url),
        Some("telephone" | "tel") => Ok(ViewInputPurpose::Telephone),
        Some("number") => Ok(ViewInputPurpose::Number),
        Some("decimal") => Ok(ViewInputPurpose::Decimal),
        Some("password") => Ok(ViewInputPurpose::Password),
        Some("pin") => Ok(ViewInputPurpose::Pin),
        Some("terminal") => Ok(ViewInputPurpose::Terminal),
        None if mode == ViewTextFieldMode::SecureField => Ok(ViewInputPurpose::Password),
        Some("text") | None => Ok(ViewInputPurpose::Text),
        Some(value) => Err(unknown_policy(view_id, "text input purpose", value)),
    }
}

fn text_control_enter_key(
    view_id: &str,
    value: Option<&str>,
) -> Result<EnterKeyHint, ViewSidecarError> {
    match value {
        Some("default") | None => Ok(EnterKeyHint::Default),
        Some("enter") => Ok(EnterKeyHint::Enter),
        Some("done") => Ok(EnterKeyHint::Done),
        Some("go") => Ok(EnterKeyHint::Go),
        Some("next") => Ok(EnterKeyHint::Next),
        Some("search") => Ok(EnterKeyHint::Search),
        Some("send") => Ok(EnterKeyHint::Send),
        Some(value) => Err(unknown_policy(view_id, "enter-key hint", value)),
    }
}

pub(super) fn text_control_selection_policy(
    view_id: &str,
    value: Option<&str>,
) -> Result<ViewTextSelectionPolicy, ViewSidecarError> {
    match value {
        Some("enabled" | "true" | "on") | None => Ok(ViewTextSelectionPolicy::Enabled),
        Some("disabled" | "none" | "false" | "off") => Ok(ViewTextSelectionPolicy::Disabled),
        Some(value) => Err(unknown_policy(view_id, "text selection", value)),
    }
}

fn text_control_shortcut_policy(
    view_id: &str,
    value: Option<&str>,
) -> Result<ViewTextShortcutPolicy, ViewSidecarError> {
    match value {
        Some("enabled" | "true" | "on") | None => Ok(ViewTextShortcutPolicy::Enabled),
        Some("disabled" | "none" | "false" | "off") => Ok(ViewTextShortcutPolicy::Disabled),
        Some(value) => Err(unknown_policy(view_id, "text shortcut", value)),
    }
}

fn text_control_tab_policy(
    view_id: &str,
    value: Option<&str>,
) -> Result<ViewTextTabPolicy, ViewSidecarError> {
    match value {
        Some("focus" | "focus_navigation" | "focusNavigation" | "navigation") | None => {
            Ok(ViewTextTabPolicy::FocusNavigation)
        }
        Some("insert" | "insert_tab" | "insertTab" | "text") => Ok(ViewTextTabPolicy::InsertTab),
        Some(value) => Err(unknown_policy(view_id, "Tab-key", value)),
    }
}

fn text_control_vertical_navigation_policy(
    view_id: &str,
    value: Option<&str>,
) -> Result<ViewTextVerticalNavigationPolicy, ViewSidecarError> {
    match value {
        Some("visual" | "visual_line" | "visualLine" | "soft_wrap" | "softWrap") => {
            Ok(ViewTextVerticalNavigationPolicy::VisualLine)
        }
        Some("logical" | "logical_line" | "logicalLine") | None => {
            Ok(ViewTextVerticalNavigationPolicy::LogicalLine)
        }
        Some(value) => Err(unknown_policy(view_id, "vertical navigation", value)),
    }
}

fn text_control_secure_policy(
    view_id: &str,
    value: Option<&str>,
    mode: ViewTextFieldMode,
) -> Result<ViewSecureInputPolicy, ViewSidecarError> {
    match value {
        Some("sensitive") => Ok(ViewSecureInputPolicy::Sensitive),
        Some("password") => Ok(ViewSecureInputPolicy::Password),
        Some("one_time_code" | "oneTimeCode" | "otp") => Ok(ViewSecureInputPolicy::OneTimeCode),
        None if mode == ViewTextFieldMode::SecureField => Ok(ViewSecureInputPolicy::Password),
        Some("plain") | None => Ok(ViewSecureInputPolicy::Plain),
        Some(value) => Err(unknown_policy(view_id, "secure input", value)),
    }
}

fn unknown_policy(view_id: &str, policy: &'static str, value: &str) -> ViewSidecarError {
    ViewSidecarError::UnknownPolicySymbol {
        view: view_resource_id(view_id),
        policy,
        value: value.to_owned(),
    }
}
