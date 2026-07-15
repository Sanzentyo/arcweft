//! Lowers materialized text, button, and image content into bundle records.

use super::{
    BundleImageObjectBounds, DialogueTextProjection, Expr, Literal, SemanticDialogueViewProjection,
    ViewAction, ViewActionButtonActionResource, ViewActionButtonResource, ViewActionPayload,
    ViewActionPayloadResource, ViewButton, ViewButtonLabel, ViewElementKind, ViewImage,
    ViewLayoutCursor, ViewLayoutFrame, ViewLoweringState, ViewModifier, ViewProgramInstruction,
    ViewRuntimeButtonBounds, ViewSemanticTarget, ViewSidecarError, ViewText, ViewTextBlockBounds,
    ViewTextBlockResource, ViewTextSelectionPolicy, ViewTextSourceKind, ViewTextSourceRecord,
    ViewTextSurface, button_bounds, expr_source, first_part, lower_button_modifiers,
    lower_modifiers, lower_navigation_target, lower_text_control_payload_field,
    lower_text_modifiers, modifier_label, modifier_layout_length_i32, modifier_layout_length_u32,
    normalize_entity_ref, normalize_input_payload_ref, symbol_expr_name, text_block_frame,
    text_control_selection_policy, view_resource_id,
};

pub(super) fn lower_text(
    view_id: &str,
    text: &ViewText,
    state: &mut ViewLoweringState,
    layout: ViewLayoutCursor,
) -> Result<ViewLayoutFrame, ViewSidecarError> {
    let id = next_text_source_id(view_id, state);
    let (kind, text_value) = lower_text_source(text, state)?;
    state.text_sources.push(ViewTextSourceRecord {
        public_id: id.clone(),
        kind,
        source: None,
    });
    let text_block_id = next_text_block_id(view_id, state);
    let styles = state.producer_styles(text.range());
    state.instructions.push(ViewProgramInstruction::EmitText {
        text_source: id.clone(),
        text_block: text_block_id.clone(),
        styles,
        part: first_part(text.modifiers())?,
        source: None,
    });
    lower_text_modifiers(view_id, text.modifiers(), state)?;
    let frame = text_block_frame(&text_value, text.modifiers());
    let view = Some(view_resource_id(view_id));
    let scroll_region = state.scroll_stack.last().cloned();
    let origin_x = modifier_layout_length_i32(text.modifiers(), &["x"]).unwrap_or(layout.x_milli);
    let origin_y = modifier_layout_length_i32(text.modifiers(), &["y"]).unwrap_or(layout.y_milli);
    let mut text_block = ViewTextBlockResource::new(
        text_block_id,
        view,
        scroll_region,
        id,
        ViewTextBlockBounds::new(origin_x, origin_y, frame.width_milli, frame.height_milli),
    )
    .with_surface(if text.rich_surface().is_some() {
        ViewTextSurface::RichText
    } else {
        ViewTextSurface::Text
    });
    text_block.selection_policy = text_block_selection_policy(text.modifiers());
    state.text_blocks.push(text_block);
    Ok(frame)
}

fn lower_text_source(
    text: &ViewText,
    state: &ViewLoweringState,
) -> Result<(ViewTextSourceKind, String), ViewSidecarError> {
    let source = text.source();
    if let Some(label) = source.dotted_selector_label()
        && let Some((parameter, field)) = label.split_once('.')
        && let Some(model) = state.dialogue_parameters.get(parameter)
        && let Some(projection) = model.projection(field)
    {
        let surface_matches = match projection {
            SemanticDialogueViewProjection::Speaker => text.rich_surface().is_none(),
            SemanticDialogueViewProjection::Content => text.rich_surface().is_some(),
            SemanticDialogueViewProjection::Occurrence
            | SemanticDialogueViewProjection::Stage
            | SemanticDialogueViewProjection::Reveal
            | SemanticDialogueViewProjection::PrimaryAction => false,
        };
        if !surface_matches {
            return Err(ViewSidecarError::UnsupportedTextSource {
                expression: format!("{source:?}"),
            });
        }
        let projection = match projection {
            SemanticDialogueViewProjection::Speaker => DialogueTextProjection::Speaker,
            SemanticDialogueViewProjection::Content => DialogueTextProjection::Content,
            SemanticDialogueViewProjection::Occurrence
            | SemanticDialogueViewProjection::Stage
            | SemanticDialogueViewProjection::Reveal
            | SemanticDialogueViewProjection::PrimaryAction => {
                unreachable!("non-text dialogue projections are rejected before bundle lowering")
            }
        };
        return Ok((
            ViewTextSourceKind::Dialogue {
                parameter: parameter.to_owned(),
                projection,
            },
            String::new(),
        ));
    }
    match source {
        Expr::Literal(Literal::String(value)) | Expr::Raw(value) => Ok((
            ViewTextSourceKind::Literal {
                value: value.clone(),
            },
            value.clone(),
        )),
        Expr::Path(path) => {
            let label = path.as_label();
            if state.value_compiler.is_local(label) {
                Ok((
                    ViewTextSourceKind::Local {
                        name: label.to_owned(),
                    },
                    String::new(),
                ))
            } else {
                Ok((
                    ViewTextSourceKind::Projection {
                        path: path
                            .segments()
                            .iter()
                            .map(|segment| segment.as_str().to_owned())
                            .collect(),
                    },
                    String::new(),
                ))
            }
        }
        Expr::Select(_) => {
            let label = source.dotted_selector_label().ok_or_else(|| {
                ViewSidecarError::UnsupportedTextSource {
                    expression: format!("{source:?}"),
                }
            })?;
            Ok((
                ViewTextSourceKind::Projection {
                    path: label.split('.').map(str::to_owned).collect(),
                },
                String::new(),
            ))
        }
        Expr::EntityRef(reference) => Ok((
            ViewTextSourceKind::Localized {
                key: normalize_entity_ref(reference),
                locale: None,
            },
            String::new(),
        )),
        _ => Err(ViewSidecarError::UnsupportedTextSource {
            expression: format!("{source:?}"),
        }),
    }
}

pub(super) fn lower_button(
    view_id: &str,
    button: &ViewButton,
    state: &mut ViewLoweringState,
    layout: ViewLayoutCursor,
) -> Result<ViewLayoutFrame, ViewSidecarError> {
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
    let styles = state.producer_styles(button.range());
    state
        .instructions
        .push(ViewProgramInstruction::OpenElement {
            element: ViewElementKind::Button,
            target: Some(button_id.clone()),
            styles,
            part: first_part(button.modifiers())?,
            key: None,
            source: None,
        });
    lower_button_modifiers(view_id, button.modifiers(), state)?;
    state
        .instructions
        .push(ViewProgramInstruction::CloseElement);
    lower_navigation_target(view_id, &button_id, button.modifiers(), state);

    let action = match button.activation() {
        Some(ViewAction::ActionInvoke(action)) => ViewActionButtonActionResource::ActionInvoke {
            action: normalize_entity_ref(action.action()),
            payload: action.payload().map(lower_action_payload),
        },
        Some(ViewAction::Projection(projection)) => {
            let label = projection.dotted_selector_label().ok_or_else(|| {
                ViewSidecarError::InvalidDialogueActionProjection {
                    expression: format!("{projection:?}"),
                }
            })?;
            let (parameter, field) = label.split_once('.').ok_or_else(|| {
                ViewSidecarError::InvalidDialogueActionProjection {
                    expression: label.clone(),
                }
            })?;
            let valid = state
                .dialogue_parameters
                .get(parameter)
                .and_then(|model| model.projection(field))
                == Some(SemanticDialogueViewProjection::PrimaryAction);
            if !valid {
                return Err(ViewSidecarError::InvalidDialogueActionProjection {
                    expression: label,
                });
            }
            ViewActionButtonActionResource::DialoguePrimaryAction {
                parameter: parameter.to_owned(),
            }
        }
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

pub(super) fn lower_image(
    view_id: &str,
    image: &ViewImage,
    state: &mut ViewLoweringState,
    layout: ViewLayoutCursor,
) -> Result<ViewLayoutFrame, ViewSidecarError> {
    let image_source = expr_source(image.source());
    let materialized = image_source_object_id(image.source())
        .and_then(|source_id| {
            state
                .source_image_objects
                .iter()
                .find(|object| object.id == source_id)
                .cloned()
        })
        .and_then(|mut object| {
            let width_milli = modifier_layout_length_u32(image.modifiers(), &["width", "w"])
                .unwrap_or(object.bounds.width_milli);
            let height_milli = modifier_layout_length_u32(image.modifiers(), &["height", "h"])
                .unwrap_or(object.bounds.height_milli);
            if width_milli == 0 || height_milli == 0 {
                return None;
            }
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
            Some(object)
        });
    let target = materialized.as_ref().map(|object| object.id.clone());
    let styles = state.producer_styles(image.range());
    state.instructions.push(ViewProgramInstruction::EmitImage {
        image: image_source,
        target,
        styles,
        part: first_part(image.modifiers())?,
        source: None,
    });
    lower_modifiers(view_id, image.modifiers(), state)?;
    let Some(object) = materialized else {
        return Ok(ViewLayoutFrame::zero());
    };
    let frame = ViewLayoutFrame::new(object.bounds.width_milli, object.bounds.height_milli);
    state.image_objects.push(object);
    Ok(frame)
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

fn button_label_text(label: &ViewButtonLabel) -> String {
    match label {
        ViewButtonLabel::Literal(value) => value.clone(),
        ViewButtonLabel::Expr(expr) => expr_source(expr),
        ViewButtonLabel::Empty => String::new(),
    }
}

fn button_display_label(button: &ViewButton, button_id: &str) -> String {
    modifier_label(button.modifiers())
        .or_else(|| match button.label() {
            ViewButtonLabel::Empty => None,
            label => Some(button_label_text(label)),
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

pub(super) fn assign_action_button_bounds(state: &mut ViewLoweringState) {
    if state.action_buttons.is_empty() {
        return;
    }
    for (fallback_index, button) in state.action_buttons.iter_mut().enumerate() {
        if button.bounds.width_milli == 0 || button.bounds.height_milli == 0 {
            button.bounds = ViewRuntimeButtonBounds::default_slot(fallback_index);
        }
    }
}
