//! Host-neutral controller input normalization.
//!
//! Native and Web adapters feed this module with already-observed button/axis
//! changes. The output is a small typed action set consumed by `InputController`.

use arcweft_render_wgpu::geometry::FocusNavigationDirection;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ControllerButton {
    DPadUp,
    DPadDown,
    DPadLeft,
    DPadRight,
    Confirm,
    Cancel,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ControllerAxis {
    LeftX,
    LeftY,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ControllerInputChange {
    Button {
        button: ControllerButton,
        pressed: bool,
        time_millis: u64,
    },
    Axis {
        axis: ControllerAxis,
        value: f32,
        time_millis: u64,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NormalizedControllerAction {
    Move(FocusNavigationDirection),
    Confirm,
    Cancel,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ControllerNavigationConfig {
    pub dead_zone: f32,
    pub repeat_delay_millis: u64,
    pub repeat_interval_millis: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ControllerInputNormalizer {
    config: ControllerNavigationConfig,
    active_axis_direction: Option<FocusNavigationDirection>,
    next_axis_repeat_millis: u64,
}

impl Default for ControllerNavigationConfig {
    fn default() -> Self {
        Self {
            dead_zone: 0.35,
            repeat_delay_millis: 320,
            repeat_interval_millis: 80,
        }
    }
}

impl Default for ControllerInputNormalizer {
    fn default() -> Self {
        Self::new(ControllerNavigationConfig::default())
    }
}

impl ControllerInputNormalizer {
    #[must_use]
    pub const fn new(config: ControllerNavigationConfig) -> Self {
        Self {
            config,
            active_axis_direction: None,
            next_axis_repeat_millis: 0,
        }
    }

    pub fn normalize(&mut self, change: ControllerInputChange) -> Vec<NormalizedControllerAction> {
        match change {
            ControllerInputChange::Button {
                button,
                pressed: true,
                ..
            } => vec![button_action(button)],
            ControllerInputChange::Button { .. } => Vec::new(),
            ControllerInputChange::Axis {
                axis,
                value,
                time_millis,
            } => self.normalize_axis(axis, value, time_millis),
        }
    }

    fn normalize_axis(
        &mut self,
        axis: ControllerAxis,
        value: f32,
        time_millis: u64,
    ) -> Vec<NormalizedControllerAction> {
        let Some(direction) = axis_direction(axis, value, self.config.dead_zone) else {
            self.active_axis_direction = None;
            self.next_axis_repeat_millis = 0;
            return Vec::new();
        };
        if self.active_axis_direction != Some(direction) {
            self.active_axis_direction = Some(direction);
            self.next_axis_repeat_millis =
                time_millis.saturating_add(self.config.repeat_delay_millis);
            return vec![NormalizedControllerAction::Move(direction)];
        }
        if time_millis >= self.next_axis_repeat_millis {
            self.next_axis_repeat_millis =
                time_millis.saturating_add(self.config.repeat_interval_millis);
            vec![NormalizedControllerAction::Move(direction)]
        } else {
            Vec::new()
        }
    }
}

fn button_action(button: ControllerButton) -> NormalizedControllerAction {
    match button {
        ControllerButton::DPadUp => NormalizedControllerAction::Move(FocusNavigationDirection::Up),
        ControllerButton::DPadDown => {
            NormalizedControllerAction::Move(FocusNavigationDirection::Down)
        }
        ControllerButton::DPadLeft => {
            NormalizedControllerAction::Move(FocusNavigationDirection::Left)
        }
        ControllerButton::DPadRight => {
            NormalizedControllerAction::Move(FocusNavigationDirection::Right)
        }
        ControllerButton::Confirm => NormalizedControllerAction::Confirm,
        ControllerButton::Cancel => NormalizedControllerAction::Cancel,
    }
}

fn axis_direction(
    axis: ControllerAxis,
    value: f32,
    dead_zone: f32,
) -> Option<FocusNavigationDirection> {
    if !value.is_finite() || value.abs() < dead_zone {
        return None;
    }
    Some(match (axis, value.is_sign_positive()) {
        (ControllerAxis::LeftX, true) => FocusNavigationDirection::Right,
        (ControllerAxis::LeftX, false) => FocusNavigationDirection::Left,
        (ControllerAxis::LeftY, true) => FocusNavigationDirection::Down,
        (ControllerAxis::LeftY, false) => FocusNavigationDirection::Up,
    })
}
