//! Lowers style, event, and focus-navigation modifiers.

use super::{
    BundleDigest, StyleAssignOp, StyleSourceIdentity, StyleSourceRef, StyleSyntax, ViewElement,
    ViewFocusDirection, ViewFocusGroupPolicy, ViewFocusGroupResource, ViewFocusInitialPolicy,
    ViewFocusNavigationEdge, ViewFocusNavigationResource, ViewFocusSkipPolicy,
    ViewFocusTargetResolution, ViewFocusWrapPolicy, ViewLoweringState, ViewModifier,
    ViewNavigationDirection, ViewNavigationInitial, ViewNavigationTarget, ViewNavigationTrap,
    ViewPartStyleRule, ViewProgramInstruction, ViewSidecarError, ViewStyleApplyRef,
    ViewStyleDeclaration, ViewStyleModifier, ViewStyleSelector, ViewStyleValue,
    inline_style_properties, next_focus_group_id, normalize_entity_ref, normalize_style_ref,
    view_resource_id,
};
use arcweft_lang_syntax::ast::style::{
    StyleAssignOp as SyntaxStyleAssignOp, StyleDeclarationDecl, StylePatch,
};
use arcweft_lang_syntax::expr::{CallArg, Expr, Literal, UnaryOp, UnitNumberSuffix};

pub(super) fn lower_modifiers(
    view_id: &str,
    modifiers: &[ViewModifier],
    state: &mut ViewLoweringState,
) -> Result<(), ViewSidecarError> {
    lower_modifiers_with_text_style(view_id, modifiers, state, false)
}

pub(super) fn lower_text_modifiers(
    view_id: &str,
    modifiers: &[ViewModifier],
    state: &mut ViewLoweringState,
) -> Result<(), ViewSidecarError> {
    lower_modifiers_with_text_style(view_id, modifiers, state, true)
}

fn lower_modifiers_with_text_style(
    view_id: &str,
    modifiers: &[ViewModifier],
    state: &mut ViewLoweringState,
    mut skip_first_named_style: bool,
) -> Result<(), ViewSidecarError> {
    for modifier in modifiers {
        match modifier {
            ViewModifier::Style(ViewStyleModifier::Named(_)) if skip_first_named_style => {
                skip_first_named_style = false;
            }
            ViewModifier::Style(style) => {
                let style = lower_style_apply(view_id, style, state);
                state.instructions.push(ViewProgramInstruction::ApplyStyle {
                    style,
                    source: None,
                });
            }
            ViewModifier::Fx(application) => {
                if let Some(instruction) = super::lower_fx_application(application, state)? {
                    state.instructions.push(instruction);
                }
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
    Ok(())
}

pub(super) fn lower_button_modifiers(
    view_id: &str,
    modifiers: &[ViewModifier],
    state: &mut ViewLoweringState,
) -> Result<(), ViewSidecarError> {
    for modifier in modifiers {
        match modifier {
            ViewModifier::Style(style) => {
                let style = lower_style_apply(view_id, style, state);
                state.instructions.push(ViewProgramInstruction::ApplyStyle {
                    style,
                    source: None,
                });
            }
            ViewModifier::Fx(application) => {
                if let Some(instruction) = super::lower_fx_application(application, state)? {
                    state.instructions.push(instruction);
                }
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
    Ok(())
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

pub(super) fn lower_navigation_group(
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

pub(super) fn lower_navigation_target(
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
        ViewStyleModifier::Inline(patch) => match patch {
            StylePatch::Arcweft { declarations, .. } => {
                let source = inline_arcweft_identity_source(declarations);
                let declarations = declarations
                    .iter()
                    .map(lower_inline_arcweft_declaration)
                    .collect();
                let patch_id =
                    next_patch_id(view_id, &source, StyleSyntax::Arcweft, declarations, state);
                ViewStyleApplyRef::InlineArcweft { patch_id }
            }
            StylePatch::Css(source) => {
                let declarations = inline_style_declarations(source.source());
                let patch_id = next_patch_id(
                    view_id,
                    source.source(),
                    StyleSyntax::Css,
                    declarations,
                    state,
                );
                ViewStyleApplyRef::InlineCss { patch_id }
            }
        },
    }
}

fn next_patch_id(
    view_id: &str,
    source: &str,
    syntax: StyleSyntax,
    declarations: Vec<ViewStyleDeclaration>,
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

fn inline_arcweft_identity_source(declarations: &[StyleDeclarationDecl]) -> String {
    declarations
        .iter()
        .map(|declaration| {
            let op = match declaration.op() {
                SyntaxStyleAssignOp::Replace => "=",
                SyntaxStyleAssignOp::Append => "+=",
            };
            format!(
                "{} {op} {}",
                declaration.property().text(),
                declaration.value().source()
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn lower_inline_arcweft_declaration(declaration: &StyleDeclarationDecl) -> ViewStyleDeclaration {
    ViewStyleDeclaration {
        property: declaration.property().text().to_owned(),
        value: inline_arcweft_value(declaration.value().expr())
            .unwrap_or_else(|| ViewStyleValue::Text(declaration.value().source().to_owned())),
        op: match declaration.op() {
            SyntaxStyleAssignOp::Replace => StyleAssignOp::Replace,
            SyntaxStyleAssignOp::Append => StyleAssignOp::Append,
        },
    }
}

fn inline_arcweft_value(expr: &Expr) -> Option<ViewStyleValue> {
    match expr {
        Expr::Literal(Literal::String(value)) => Some(ViewStyleValue::Text(value.clone())),
        Expr::Literal(Literal::Bool(value)) => Some(ViewStyleValue::Text(value.to_string())),
        Expr::Literal(Literal::UnitNumber {
            raw,
            suffix: UnitNumberSuffix::Milli,
        }) => raw
            .strip_suffix("milli")?
            .replace('_', "")
            .parse::<i32>()
            .ok()
            .map(ViewStyleValue::Milli),
        Expr::Path(path) => Some(ViewStyleValue::Text(path.as_label().to_owned())),
        Expr::ShortVariant(name) => Some(ViewStyleValue::Text(format!(".{name}"))),
        Expr::Unary {
            op: UnaryOp::Neg,
            expr,
        } => match inline_arcweft_value(expr)? {
            ViewStyleValue::Milli(value) => value.checked_neg().map(ViewStyleValue::Milli),
            _ => None,
        },
        Expr::Call { callee, args } => {
            let name = callee.dotted_selector_label()?;
            let value = args.iter().find_map(|arg| match arg {
                CallArg::Positional(value) => Some(value),
                CallArg::Named { .. } | CallArg::Spread { .. } => None,
            })?;
            match name.as_str() {
                "token" => value.dotted_selector_label().map(ViewStyleValue::Token),
                "resource" => value.dotted_selector_label().map(ViewStyleValue::Resource),
                "text" => match value {
                    Expr::Literal(Literal::String(value)) => {
                        Some(ViewStyleValue::Text(value.clone()))
                    }
                    _ => None,
                },
                "milli" => match value {
                    Expr::Literal(Literal::Int(value)) => i32::try_from(value.magnitude().ok()?)
                        .ok()
                        .map(ViewStyleValue::Milli),
                    _ => None,
                },
                _ => None,
            }
        }
        _ => None,
    }
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
