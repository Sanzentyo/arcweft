use super::bundle_view::{
    ViewSidecarError, expr_source, inline_style_properties, normalize_property_name,
};
use arcweft_lang_syntax::ast::view::{ViewModifier, ViewStyleModifier};

pub(in crate::app) fn validate_interactive_overflow_modifiers(
    element: &str,
    modifiers: &[ViewModifier],
    is_scroll: bool,
) -> Result<(), ViewSidecarError> {
    if is_scroll {
        return Ok(());
    }
    for modifier in modifiers {
        match modifier {
            ViewModifier::Property { name, value } => {
                reject_interactive_overflow_property(element, name, &expr_source(value))?;
            }
            ViewModifier::Style(
                ViewStyleModifier::InlineArcweft(source) | ViewStyleModifier::InlineCss(source),
            ) => {
                for (name, value) in inline_style_properties(source) {
                    reject_interactive_overflow_property(element, &name, &value)?;
                }
            }
            ViewModifier::Style(ViewStyleModifier::Named(_))
            | ViewModifier::Fx(_)
            | ViewModifier::Part(_)
            | ViewModifier::Label(_)
            | ViewModifier::AgentTarget(_)
            | ViewModifier::Placeholder(_)
            | ViewModifier::Purpose(_)
            | ViewModifier::EnterKey(_)
            | ViewModifier::Enabled(_)
            | ViewModifier::Focusable(_)
            | ViewModifier::Environment(_)
            | ViewModifier::Focus(_)
            | ViewModifier::Navigation(_)
            | ViewModifier::OnEvent { .. }
            | ViewModifier::Raw(_) => {}
        }
    }
    Ok(())
}

fn reject_interactive_overflow_property(
    element: &str,
    name: &str,
    value: &str,
) -> Result<(), ViewSidecarError> {
    let property = normalize_property_name(name);
    if !matches!(property.as_str(), "overflow" | "overflow-x" | "overflow-y") {
        return Ok(());
    }
    if interactive_overflow_symbol(value).is_none() {
        return Ok(());
    }
    Err(ViewSidecarError::InteractiveOverflowRequiresScroll {
        element: element.to_owned(),
        property,
        value: value.trim().trim_matches('"').to_owned(),
    })
}

fn interactive_overflow_symbol(value: &str) -> Option<()> {
    match value.trim().trim_matches('"').trim_start_matches('.') {
        "auto" | "scroll" => Some(()),
        _ => None,
    }
}
