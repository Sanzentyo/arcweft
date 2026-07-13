//! Lowers event and focus-navigation modifiers.

use super::{
    ViewElement, ViewFocusDirection, ViewFocusGroupPolicy, ViewFocusGroupResource,
    ViewFocusInitialPolicy, ViewFocusNavigationEdge, ViewFocusNavigationResource,
    ViewFocusSkipPolicy, ViewFocusTargetResolution, ViewFocusWrapPolicy, ViewLoweringState,
    ViewModifier, ViewNavigationDirection, ViewNavigationInitial, ViewNavigationTarget,
    ViewNavigationTrap, ViewProgramInstruction, ViewSidecarError, next_focus_group_id,
    normalize_entity_ref, view_resource_id,
};

pub(super) fn lower_modifiers(
    view_id: &str,
    modifiers: &[ViewModifier],
    state: &mut ViewLoweringState,
) -> Result<(), ViewSidecarError> {
    lower_modifiers_without_style(view_id, modifiers, state)
}

pub(super) fn lower_text_modifiers(
    view_id: &str,
    modifiers: &[ViewModifier],
    state: &mut ViewLoweringState,
) -> Result<(), ViewSidecarError> {
    lower_modifiers_without_style(view_id, modifiers, state)
}

fn lower_modifiers_without_style(
    view_id: &str,
    modifiers: &[ViewModifier],
    state: &mut ViewLoweringState,
) -> Result<(), ViewSidecarError> {
    for modifier in modifiers {
        match modifier {
            ViewModifier::Fx(application) => {
                if let Some(instruction) = super::lower_fx_application(application, state)? {
                    state.instructions.push(instruction);
                }
            }
            ViewModifier::OnEvent { name, .. } => {
                lower_event_handler_modifier(view_id, name, state);
            }
            ViewModifier::Style(_)
            | ViewModifier::Part(_)
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
    lower_modifiers_without_style(view_id, modifiers, state)
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
