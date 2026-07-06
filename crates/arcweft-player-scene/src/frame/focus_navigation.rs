use arcweft_bundle::resource_codec::{
    ViewFocusDirection, ViewFocusGroupPolicy, ViewFocusInitialPolicy, ViewFocusSkipPolicy,
    ViewFocusTargetResolution, ViewFocusWrapPolicy, ViewRuntimeFocusGroup,
    ViewRuntimeFocusNavigation,
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
    groups: &[ViewRuntimeFocusGroup],
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
    navigation: &[ViewRuntimeFocusNavigation],
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

fn render_group_policy(policy: ViewFocusGroupPolicy) -> RenderFocusGroupPolicy {
    match policy {
        ViewFocusGroupPolicy::Normal => RenderFocusGroupPolicy::Normal,
        ViewFocusGroupPolicy::Trap => RenderFocusGroupPolicy::Trap,
        ViewFocusGroupPolicy::Modal => RenderFocusGroupPolicy::Modal,
    }
}

fn render_initial(
    initial: &ViewFocusInitialPolicy,
) -> Result<RenderFocusInitialPolicy, PlayerFrameError> {
    Ok(match initial {
        ViewFocusInitialPolicy::Auto => RenderFocusInitialPolicy::Auto,
        ViewFocusInitialPolicy::First => RenderFocusInitialPolicy::First,
        ViewFocusInitialPolicy::Last => RenderFocusInitialPolicy::Last,
        ViewFocusInitialPolicy::None => RenderFocusInitialPolicy::None,
        ViewFocusInitialPolicy::Explicit { target } => {
            RenderFocusInitialPolicy::Explicit(interaction_target(target)?)
        }
    })
}

fn render_wrap(wrap: ViewFocusWrapPolicy) -> RenderFocusWrapPolicy {
    match wrap {
        ViewFocusWrapPolicy::Wrap => RenderFocusWrapPolicy::Wrap,
        ViewFocusWrapPolicy::NoWrap => RenderFocusWrapPolicy::NoWrap,
    }
}

fn render_skip(skip: ViewFocusSkipPolicy) -> RenderFocusSkipPolicy {
    match skip {
        ViewFocusSkipPolicy::Skip => RenderFocusSkipPolicy::Skip,
        ViewFocusSkipPolicy::Stop => RenderFocusSkipPolicy::Stop,
    }
}

fn render_direction(direction: ViewFocusDirection) -> FocusNavigationDirection {
    match direction {
        ViewFocusDirection::Up => FocusNavigationDirection::Up,
        ViewFocusDirection::Down => FocusNavigationDirection::Down,
        ViewFocusDirection::Left => FocusNavigationDirection::Left,
        ViewFocusDirection::Right => FocusNavigationDirection::Right,
        ViewFocusDirection::Next => FocusNavigationDirection::Next,
        ViewFocusDirection::Previous => FocusNavigationDirection::Previous,
    }
}

fn render_target(
    target: &ViewFocusTargetResolution,
) -> Result<RenderFocusTargetResolution, PlayerFrameError> {
    Ok(match target {
        ViewFocusTargetResolution::Explicit { target } => {
            RenderFocusTargetResolution::Explicit(interaction_target(target)?)
        }
        ViewFocusTargetResolution::Auto => RenderFocusTargetResolution::Auto,
        ViewFocusTargetResolution::None => RenderFocusTargetResolution::None,
        ViewFocusTargetResolution::GroupBoundary => RenderFocusTargetResolution::GroupBoundary,
    })
}

fn interaction_target(value: &str) -> Result<InteractionTarget, PlayerFrameError> {
    PublicId::try_new(value)
        .map(InteractionTarget::new)
        .map_err(|_| PlayerFrameError::InvalidId {
            value: value.to_owned(),
        })
}
