use arcweft_bundle::{
    container::BundleDigest,
    resource_codec::{
        UiActionButtonActionResource, UiActionButtonResource, UiInputResource, UiProgramResource,
        UiRuntimeButtonBounds, UiStyleResource, UiTextResource,
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
            ComponentViewBody, ViewAction, ViewButton, ViewButtonLabel, ViewElement, ViewExpr,
            ViewImage, ViewModifier, ViewStyleModifier, ViewText, ViewTextField, ViewTextFieldMode,
            ViewTextSubmitImePolicy,
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
    inline_arcweft_sources: Vec<StyleSourceIdentity>,
    inline_css_sources: Vec<StyleSourceIdentity>,
    text_counter: u32,
    input_counter: u32,
    button_counter: u32,
    handler_counter: u32,
    patch_counter: u32,
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
    if state.instructions.is_empty()
        && state.text_sources.is_empty()
        && state.input_options.is_empty()
        && state.action_buttons.is_empty()
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
        lower_modifiers(component_id, element.modifiers(), state);
        for child in element.children() {
            lower_view_expr(component_id, child, state);
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
    if field.input().is_some() {
        state.instructions.push(UiProgramInstruction::OpenElement {
            element: ui_element_kind_for_text_field(field.mode()),
            style: None,
            part: first_part(field.modifiers()),
            key: None,
            source: None,
        });
        lower_modifiers(component_id, field.modifiers(), state);
        state.instructions.push(UiProgramInstruction::CloseElement);
        return;
    }
    let id = next_input_id(component_id, field.mode(), state);
    let value_text_source = format!("text.value.{id}");
    state.text_sources.push(UiTextSourceRecord {
        public_id: value_text_source.clone(),
        kind: UiTextSourceKind::Literal {
            value: expr_source(field.value()),
        },
        source: None,
    });
    state.input_options.push(UiInputOptions {
        public_id: id.clone(),
        kind: ui_input_kind(field.mode()),
        value_text_source,
        placeholder_text_source: None,
        purpose: if field.mode() == ViewTextFieldMode::SecureField {
            UiInputPurpose::Password
        } else {
            UiInputPurpose::Text
        },
        autocorrect: TextAssistPolicy::PlatformDefault,
        spellcheck: TextAssistPolicy::PlatformDefault,
        capitalization: TextCapitalization::None,
        enter_key: EnterKeyHint::Default,
        multiline: field.mode() == ViewTextFieldMode::TextArea,
        secure_policy: if field.mode() == ViewTextFieldMode::SecureField {
            UiSecureInputPolicy::Password
        } else {
            UiSecureInputPolicy::Plain
        },
        composition_on_blur: CompositionOnBlurPolicy::Commit,
        submit_handler: None,
        change_handler: None,
        adapter_requirements: Vec::new(),
    });
    state.semantic_targets.push(UiSemanticTarget {
        public_id: id.clone(),
        target: id,
        label_text_source: None,
        source: None,
    });
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
        bounds: button_slot_bounds(state.action_buttons.len()),
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
            | ViewModifier::Raw(_)
            | ViewModifier::OnEvent { .. } => {}
        }
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

fn normalize_style_ref(reference: &EntityRefSyntax) -> String {
    normalize_entity_ref(reference)
}

fn normalize_entity_ref(reference: &EntityRefSyntax) -> String {
    match reference {
        EntityRefSyntax::Absolute(entity) => entity.body().to_owned(),
        EntityRefSyntax::FamilyRelative(relative) => {
            format!("{}.{}", relative.family(), relative.relative().suffix())
        }
    }
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

fn button_slot_bounds(index: usize) -> UiRuntimeButtonBounds {
    let y = 112_000_i32.saturating_add(
        i32::try_from(index)
            .unwrap_or(i32::MAX)
            .saturating_mul(56_000),
    );
    UiRuntimeButtonBounds::new(48_000, y, 180_000, 44_000)
}
