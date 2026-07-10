//! Host-neutral controller input normalization.
//!
//! Native and Web adapters feed this module with already-observed button/axis
//! changes. The output is a small typed action set consumed by `InputController`.

use arcweft_render_wgpu::geometry::FocusNavigationDirection;
use std::time::Duration;

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
    RightX,
    RightY,
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

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum NormalizedControllerAction {
    Move(FocusNavigationDirection),
    /// Precision-scroll delta in the same input-space convention as a wheel.
    Scroll {
        delta_x: f32,
        delta_y: f32,
    },
    Confirm,
    Cancel,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ControllerNavigationConfig {
    pub dead_zone: f32,
    pub repeat_delay_millis: u64,
    pub repeat_interval_millis: u64,
    /// Full-deflection right-stick scroll velocity.
    pub analog_scroll_pixels_per_second: f32,
    /// Caps one integrated sample after a delayed or stalled input poll.
    pub analog_scroll_max_sample_millis: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ControllerInputNormalizer {
    config: ControllerNavigationConfig,
    active_axis_direction: Option<FocusNavigationDirection>,
    next_axis_repeat_millis: u64,
    right_axis_sample_millis: [Option<u64>; 2],
}

impl Default for ControllerNavigationConfig {
    fn default() -> Self {
        Self {
            dead_zone: 0.35,
            repeat_delay_millis: 320,
            repeat_interval_millis: 80,
            analog_scroll_pixels_per_second: 720.0,
            analog_scroll_max_sample_millis: 100,
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
            right_axis_sample_millis: [None; 2],
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

    /// Clears held-axis/repeat timing while preserving the configured policy.
    pub fn reset_transient_state(&mut self) {
        *self = Self::new(self.config);
    }

    fn normalize_axis(
        &mut self,
        axis: ControllerAxis,
        value: f32,
        time_millis: u64,
    ) -> Vec<NormalizedControllerAction> {
        if matches!(axis, ControllerAxis::RightX | ControllerAxis::RightY) {
            return self.normalize_scroll_axis(axis, value, time_millis);
        }
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

    fn normalize_scroll_axis(
        &mut self,
        axis: ControllerAxis,
        value: f32,
        time_millis: u64,
    ) -> Vec<NormalizedControllerAction> {
        let sample_index = match axis {
            ControllerAxis::RightX => 0,
            ControllerAxis::RightY => 1,
            ControllerAxis::LeftX | ControllerAxis::LeftY => unreachable!(),
        };
        let Some(value) = normalized_axis_value(value, self.config.dead_zone) else {
            self.right_axis_sample_millis[sample_index] = None;
            return Vec::new();
        };
        let previous = self.right_axis_sample_millis[sample_index].replace(time_millis);
        let Some(previous) = previous else {
            return Vec::new();
        };
        let elapsed_millis = time_millis
            .saturating_sub(previous)
            .min(self.config.analog_scroll_max_sample_millis);
        if elapsed_millis == 0 {
            return Vec::new();
        }
        let speed = if self.config.analog_scroll_pixels_per_second.is_finite() {
            self.config.analog_scroll_pixels_per_second.max(0.0)
        } else {
            0.0
        };
        let distance = value * speed * Duration::from_millis(elapsed_millis).as_secs_f32();
        if distance.abs() <= f32::EPSILON {
            return Vec::new();
        }
        let action = match axis {
            // Precision-scroll input uses the wheel convention: negative deltas
            // move retained content toward positive x/y offsets.
            ControllerAxis::RightX => NormalizedControllerAction::Scroll {
                delta_x: -distance,
                delta_y: 0.0,
            },
            ControllerAxis::RightY => NormalizedControllerAction::Scroll {
                delta_x: 0.0,
                delta_y: -distance,
            },
            ControllerAxis::LeftX | ControllerAxis::LeftY => unreachable!(),
        };
        vec![action]
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
        (ControllerAxis::RightX | ControllerAxis::RightY, _) => return None,
    })
}

fn normalized_axis_value(value: f32, dead_zone: f32) -> Option<f32> {
    if !value.is_finite() || !dead_zone.is_finite() {
        return None;
    }
    let dead_zone = dead_zone.clamp(0.0, 1.0);
    let magnitude = value.abs().clamp(0.0, 1.0);
    if magnitude <= dead_zone || dead_zone >= 1.0 {
        return None;
    }
    Some(value.signum() * (magnitude - dead_zone) / (1.0 - dead_zone))
}
