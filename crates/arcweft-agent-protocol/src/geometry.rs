use serde::{Deserialize, Serialize};

/// Viewport that coordinates object bounds and image resources.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct AgentViewport {
    pub width: u32,
    pub height: u32,
    pub scale: f32,
}

/// RGBA color used by object-id debug images.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgentRgbaColor {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
    pub alpha: u8,
}

/// Axis-aligned object bounds.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgentBBox {
    pub space: AgentCoordinateSpace,
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl AgentBBox {
    /// Returns the viewport-space rectangle corners in clockwise order.
    pub fn polygon(&self) -> Vec<AgentPoint> {
        vec![
            AgentPoint {
                x: self.x,
                y: self.y,
            },
            AgentPoint {
                x: self.x + self.width,
                y: self.y,
            },
            AgentPoint {
                x: self.x + self.width,
                y: self.y + self.height,
            },
            AgentPoint {
                x: self.x,
                y: self.y + self.height,
            },
        ]
    }
}

/// Coordinate space for Agent geometry.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentCoordinateSpace {
    Viewport,
    World,
    View,
}

/// Point in an Agent coordinate space.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgentPoint {
    pub x: u32,
    pub y: u32,
}
