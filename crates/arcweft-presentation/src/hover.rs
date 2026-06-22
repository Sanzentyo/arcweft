use crate::input::{InteractionTarget, PointerId};

/// Stable hover path for one pointer, ordered from root-like owner to leaf target.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HoverPath {
    pointer: PointerId,
    targets: Vec<InteractionTarget>,
}

/// Enter/leave transition derived from two stable hover paths.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HoverTransition {
    pointer: PointerId,
    exited: Vec<InteractionTarget>,
    entered: Vec<InteractionTarget>,
}

impl HoverPath {
    pub fn new(pointer: PointerId, targets: Vec<InteractionTarget>) -> Self {
        Self { pointer, targets }
    }

    pub const fn pointer(&self) -> PointerId {
        self.pointer
    }

    pub fn targets(&self) -> &[InteractionTarget] {
        &self.targets
    }

    pub fn leaf(&self) -> Option<&InteractionTarget> {
        self.targets.last()
    }

    pub fn contains(&self, target: &InteractionTarget) -> bool {
        self.targets.iter().any(|candidate| candidate == target)
    }
}

impl HoverTransition {
    pub fn diff(previous: Option<&HoverPath>, next: Option<&HoverPath>) -> Option<Self> {
        let pointer = previous
            .map(HoverPath::pointer)
            .or_else(|| next.map(HoverPath::pointer))?;
        let previous_targets = previous.map(HoverPath::targets).unwrap_or_default();
        let next_targets = next.map(HoverPath::targets).unwrap_or_default();
        let common_prefix = previous_targets
            .iter()
            .zip(next_targets.iter())
            .take_while(|(left, right)| left == right)
            .count();
        let exited = previous_targets[common_prefix..]
            .iter()
            .rev()
            .cloned()
            .collect();
        let entered = next_targets[common_prefix..].to_vec();

        Some(Self {
            pointer,
            exited,
            entered,
        })
    }

    pub const fn pointer(&self) -> PointerId {
        self.pointer
    }

    pub fn exited(&self) -> &[InteractionTarget] {
        &self.exited
    }

    pub fn entered(&self) -> &[InteractionTarget] {
        &self.entered
    }

    pub fn is_empty(&self) -> bool {
        self.exited.is_empty() && self.entered.is_empty()
    }
}
