//! Prepared focus graph and deterministic focus resolution helpers.
//!
//! This module is deliberately renderer/local-data only. It consumes already
//! normalized focus metadata and frame geometry; it never talks to DOM, native
//! widgets, files, or platform controllers.

use super::{FocusNavigationDirection, KeyboardFocusCandidate, PreparedFrame};
use arcweft_presentation::input::InteractionTarget;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PreparedFocusGraph {
    groups: Vec<PreparedFocusGroup>,
    targets: Vec<PreparedFocusNavigationTarget>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RenderFocusGroup {
    pub public_id: String,
    pub parent: Option<String>,
    pub policy: RenderFocusGroupPolicy,
    pub initial: RenderFocusInitialPolicy,
    pub wrap: RenderFocusWrapPolicy,
    pub disabled_skip: RenderFocusSkipPolicy,
    pub hidden_skip: RenderFocusSkipPolicy,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RenderFocusNavigation {
    pub target: InteractionTarget,
    pub group: Option<String>,
    pub edges: Vec<RenderFocusNavigationEdge>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RenderFocusNavigationEdge {
    pub direction: FocusNavigationDirection,
    pub target: RenderFocusTargetResolution,
}

#[derive(Clone, Debug, PartialEq)]
pub enum RenderFocusTargetResolution {
    Explicit(InteractionTarget),
    Auto,
    None,
    GroupBoundary,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum RenderFocusGroupPolicy {
    #[default]
    Normal,
    Trap,
    Modal,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub enum RenderFocusInitialPolicy {
    #[default]
    Auto,
    First,
    Last,
    Explicit(InteractionTarget),
    None,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum RenderFocusWrapPolicy {
    #[default]
    Wrap,
    NoWrap,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum RenderFocusSkipPolicy {
    #[default]
    Skip,
    Stop,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PreparedFocusGroup {
    pub public_id: String,
    pub parent: Option<String>,
    pub policy: RenderFocusGroupPolicy,
    pub initial: RenderFocusInitialPolicy,
    pub wrap: RenderFocusWrapPolicy,
    pub disabled_skip: RenderFocusSkipPolicy,
    pub hidden_skip: RenderFocusSkipPolicy,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PreparedFocusNavigationTarget {
    pub target: InteractionTarget,
    pub group: Option<String>,
    pub edges: Vec<RenderFocusNavigationEdge>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FocusNavigationDebug {
    pub focused_target: Option<String>,
    pub group_count: usize,
    pub target_count: usize,
    pub candidates: Vec<FocusNavigationDebugCandidate>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FocusNavigationDebugCandidate {
    pub direction: String,
    pub target: Option<String>,
    pub source: String,
}

impl PreparedFocusGraph {
    #[must_use]
    pub fn new(groups: Vec<RenderFocusGroup>, navigation: Vec<RenderFocusNavigation>) -> Self {
        Self {
            groups: groups
                .into_iter()
                .map(|group| PreparedFocusGroup {
                    public_id: group.public_id,
                    parent: group.parent,
                    policy: group.policy,
                    initial: group.initial,
                    wrap: group.wrap,
                    disabled_skip: group.disabled_skip,
                    hidden_skip: group.hidden_skip,
                })
                .collect(),
            targets: navigation
                .into_iter()
                .map(|target| PreparedFocusNavigationTarget {
                    target: target.target,
                    group: target.group,
                    edges: target.edges,
                })
                .collect(),
        }
    }

    #[must_use]
    pub fn groups(&self) -> &[PreparedFocusGroup] {
        &self.groups
    }

    #[must_use]
    pub fn targets(&self) -> &[PreparedFocusNavigationTarget] {
        &self.targets
    }

    #[must_use]
    pub fn edge(
        &self,
        target: &InteractionTarget,
        direction: FocusNavigationDirection,
    ) -> Option<&RenderFocusNavigationEdge> {
        self.targets
            .iter()
            .find(|candidate| candidate.target == *target)
            .and_then(|candidate| {
                candidate
                    .edges
                    .iter()
                    .find(|edge| edge.direction == direction)
            })
    }

    #[must_use]
    pub fn allows_wrap(&self, target: Option<&InteractionTarget>) -> bool {
        target
            .and_then(|target| self.group_for_target(target))
            .is_none_or(|group| matches!(group.wrap, RenderFocusWrapPolicy::Wrap))
    }

    #[must_use]
    pub fn disabled_skip_policy(&self, target: &InteractionTarget) -> RenderFocusSkipPolicy {
        self.group_for_target(target)
            .map_or(RenderFocusSkipPolicy::Skip, |group| group.disabled_skip)
    }

    #[must_use]
    fn debug_for(
        &self,
        focused: Option<&InteractionTarget>,
        _candidates: &[KeyboardFocusCandidate],
    ) -> FocusNavigationDebug {
        let directions = [
            FocusNavigationDirection::Up,
            FocusNavigationDirection::Down,
            FocusNavigationDirection::Left,
            FocusNavigationDirection::Right,
            FocusNavigationDirection::Next,
            FocusNavigationDirection::Previous,
        ];
        let candidates = focused
            .map(|target| {
                directions
                    .iter()
                    .map(|direction| match self.edge(target, *direction) {
                        Some(edge) => FocusNavigationDebugCandidate {
                            direction: direction_label(*direction).to_owned(),
                            target: edge.target.explicit_target().map(target_label),
                            source: "explicit".to_owned(),
                        },
                        None => FocusNavigationDebugCandidate {
                            direction: direction_label(*direction).to_owned(),
                            target: None,
                            source: "auto".to_owned(),
                        },
                    })
                    .collect()
            })
            .unwrap_or_default();
        FocusNavigationDebug {
            focused_target: focused.map(target_label),
            group_count: self.groups.len(),
            target_count: self.targets.len(),
            candidates,
        }
    }

    fn group_for_target(&self, target: &InteractionTarget) -> Option<&PreparedFocusGroup> {
        let group_id = self
            .targets
            .iter()
            .find(|candidate| candidate.target == *target)
            .and_then(|candidate| candidate.group.as_deref())?;
        self.groups.iter().find(|group| group.public_id == group_id)
    }
}

impl PreparedFrame {
    #[must_use]
    pub fn focus_target(
        &self,
        current: Option<&InteractionTarget>,
        direction: FocusNavigationDirection,
    ) -> Option<InteractionTarget> {
        let current = current.or_else(|| self.interaction_focused_target());
        let Some(current) = current else {
            return self.first_keyboard_focus_target();
        };
        if let Some(edge) = self.focus_graph.edge(current, direction) {
            return self.resolve_focus_edge(current, edge, direction);
        }
        self.auto_focus_target(current, direction)
    }

    #[must_use]
    pub fn focus_debug(&self) -> FocusNavigationDebug {
        self.focus_graph.debug_for(
            self.interaction_focused_target(),
            &self.keyboard_focus_candidates(),
        )
    }

    fn resolve_focus_edge(
        &self,
        current: &InteractionTarget,
        edge: &RenderFocusNavigationEdge,
        direction: FocusNavigationDirection,
    ) -> Option<InteractionTarget> {
        match &edge.target {
            RenderFocusTargetResolution::Auto => self.auto_focus_target(current, direction),
            RenderFocusTargetResolution::None | RenderFocusTargetResolution::GroupBoundary => None,
            RenderFocusTargetResolution::Explicit(target) => {
                if self.is_enabled_keyboard_focus_target(target) {
                    Some(target.clone())
                } else if matches!(
                    self.focus_graph.disabled_skip_policy(current),
                    RenderFocusSkipPolicy::Skip
                ) {
                    self.auto_focus_target(current, direction)
                } else {
                    None
                }
            }
        }
    }

    fn auto_focus_target(
        &self,
        current: &InteractionTarget,
        direction: FocusNavigationDirection,
    ) -> Option<InteractionTarget> {
        match direction {
            FocusNavigationDirection::Next => self.adjacent_keyboard_focus_target_with_wrap(
                Some(current),
                1,
                self.focus_graph.allows_wrap(Some(current)),
            ),
            FocusNavigationDirection::Previous => self.adjacent_keyboard_focus_target_with_wrap(
                Some(current),
                -1,
                self.focus_graph.allows_wrap(Some(current)),
            ),
            FocusNavigationDirection::Up
            | FocusNavigationDirection::Down
            | FocusNavigationDirection::Left
            | FocusNavigationDirection::Right => {
                self.geometric_keyboard_focus_target(current, direction)
            }
        }
    }
}

impl RenderFocusTargetResolution {
    fn explicit_target(&self) -> Option<&InteractionTarget> {
        match self {
            Self::Explicit(target) => Some(target),
            Self::Auto | Self::None | Self::GroupBoundary => None,
        }
    }
}

fn direction_label(direction: FocusNavigationDirection) -> &'static str {
    match direction {
        FocusNavigationDirection::Up => "up",
        FocusNavigationDirection::Down => "down",
        FocusNavigationDirection::Left => "left",
        FocusNavigationDirection::Right => "right",
        FocusNavigationDirection::Next => "next",
        FocusNavigationDirection::Previous => "previous",
    }
}

fn target_label(target: &InteractionTarget) -> String {
    target.id().as_str().to_owned()
}
