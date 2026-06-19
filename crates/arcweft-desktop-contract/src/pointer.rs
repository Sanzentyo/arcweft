use crate::{PhysicalPosition, WindowTarget};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PointerCoordinateSpace {
    GlobalPhysical,
    WindowPhysical,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PointerPosition {
    pub position: PhysicalPosition,
    pub space: PointerCoordinateSpace,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CursorIcon {
    #[default]
    Default,
    Pointer,
    Text,
    Crosshair,
    Move,
    NotAllowed,
    Wait,
    Progress,
    Help,
    ZoomIn,
    ZoomOut,
    Grab,
    Grabbing,
    ResizeHorizontal,
    ResizeVertical,
    ResizeDiagonalNorthEastSouthWest,
    ResizeDiagonalNorthWestSouthEast,
    Hidden,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CursorGrabMode {
    #[default]
    None,
    Confined,
    Locked,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case", tag = "operation")]
pub enum OwnedCursorRequest {
    SetIcon {
        target: WindowTarget,
        icon: CursorIcon,
    },
    SetVisible {
        target: WindowTarget,
        visible: bool,
    },
    SetGrab {
        target: WindowTarget,
        mode: CursorGrabMode,
    },
    SetPosition {
        target: WindowTarget,
        position: PhysicalPosition,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case", tag = "operation")]
pub enum GlobalPointerRequest {
    Position,
    Move { position: PhysicalPosition },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case", tag = "result", content = "value")]
pub enum GlobalPointerResponse {
    Position(PointerPosition),
    Applied,
}
