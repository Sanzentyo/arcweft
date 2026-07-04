use arcweft_bundle::resource_codec::{
    UiFocusDirection, UiFocusGroupPolicy, UiFocusInitialPolicy, UiFocusSkipPolicy,
    UiFocusTargetResolution, UiFocusWrapPolicy, UiRuntimeFocusGroup, UiRuntimeFocusNavigation,
};
use arcweft_id::PublicId;
use arcweft_presentation::input::InteractionTarget;
use arcweft_render_wgpu::geometry::{
    FocusNavigationDirection, RenderFocusGroup, RenderFocusGroupPolicy, RenderFocusInitialPolicy,
    RenderFocusNavigation, RenderFocusNavigationEdge, RenderFocusSkipPolicy,
    RenderFocusTargetResolution, RenderFocusWrapPolicy,
};

use super::PlayerFrameError;

pub(super) fn render_focus_groups(
    groups: &[UiRuntimeFocusGroup],
) -> Result<Vec<RenderFocusGroup>, PlayerFrameError> {
    groups
        .iter()
        .map(|group| {
            Ok(RenderFocusGroup {
                public_id: group.public_id.clone(),
                parent: group.parent.clone(),
                policy: render_group_policy(group.policy),
                initial: render_initial(&group.initial)?,
                wrap: render_wrap(group.wrap),
                disabled_skip: render_skip(group.disabled_skip),
                hidden_skip: render_skip(group.hidden_skip),
            })
        })
        .collect()
}

pub(super) fn render_focus_navigation(
    navigation: &[UiRuntimeFocusNavigation],
) -> Result<Vec<RenderFocusNavigation>, PlayerFrameError> {
    navigation
        .iter()
        .map(|target| {
            Ok(RenderFocusNavigation {
                target: interaction_target(&target.public_id)?,
                group: target.group.clone(),
                edges: target
                    .edges
                    .iter()
                    .map(|edge| {
                        Ok(RenderFocusNavigationEdge {
                            direction: render_direction(edge.direction),
                            target: render_target(&edge.target)?,
                        })
                    })
                    .collect::<Result<Vec<_>, PlayerFrameError>>()?,
            })
        })
        .collect()
}

fn render_group_policy(policy: UiFocusGroupPolicy) -> RenderFocusGroupPolicy {
    match policy {
        UiFocusGroupPolicy::Normal => RenderFocusGroupPolicy::Normal,
        UiFocusGroupPolicy::Trap => RenderFocusGroupPolicy::Trap,
        UiFocusGroupPolicy::Modal => RenderFocusGroupPolicy::Modal,
    }
}

fn render_initial(
    initial: &UiFocusInitialPolicy,
) -> Result<RenderFocusInitialPolicy, PlayerFrameError> {
    Ok(match initial {
        UiFocusInitialPolicy::Auto => RenderFocusInitialPolicy::Auto,
        UiFocusInitialPolicy::First => RenderFocusInitialPolicy::First,
        UiFocusInitialPolicy::Last => RenderFocusInitialPolicy::Last,
        UiFocusInitialPolicy::None => RenderFocusInitialPolicy::None,
        UiFocusInitialPolicy::Explicit { target } => {
            RenderFocusInitialPolicy::Explicit(interaction_target(target)?)
        }
    })
}

fn render_wrap(wrap: UiFocusWrapPolicy) -> RenderFocusWrapPolicy {
    match wrap {
        UiFocusWrapPolicy::Wrap => RenderFocusWrapPolicy::Wrap,
        UiFocusWrapPolicy::NoWrap => RenderFocusWrapPolicy::NoWrap,
    }
}

fn render_skip(skip: UiFocusSkipPolicy) -> RenderFocusSkipPolicy {
    match skip {
        UiFocusSkipPolicy::Skip => RenderFocusSkipPolicy::Skip,
        UiFocusSkipPolicy::Stop => RenderFocusSkipPolicy::Stop,
    }
}

fn render_direction(direction: UiFocusDirection) -> FocusNavigationDirection {
    match direction {
        UiFocusDirection::Up => FocusNavigationDirection::Up,
        UiFocusDirection::Down => FocusNavigationDirection::Down,
        UiFocusDirection::Left => FocusNavigationDirection::Left,
        UiFocusDirection::Right => FocusNavigationDirection::Right,
        UiFocusDirection::Next => FocusNavigationDirection::Next,
        UiFocusDirection::Previous => FocusNavigationDirection::Previous,
    }
}

fn render_target(
    target: &UiFocusTargetResolution,
) -> Result<RenderFocusTargetResolution, PlayerFrameError> {
    Ok(match target {
        UiFocusTargetResolution::Explicit { target } => {
            RenderFocusTargetResolution::Explicit(interaction_target(target)?)
        }
        UiFocusTargetResolution::Auto => RenderFocusTargetResolution::Auto,
        UiFocusTargetResolution::None => RenderFocusTargetResolution::None,
        UiFocusTargetResolution::GroupBoundary => RenderFocusTargetResolution::GroupBoundary,
    })
}

fn interaction_target(value: &str) -> Result<InteractionTarget, PlayerFrameError> {
    PublicId::try_new(value)
        .map(InteractionTarget::new)
        .map_err(|_| PlayerFrameError::InvalidId {
            value: value.to_owned(),
        })
}
