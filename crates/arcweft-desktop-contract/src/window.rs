use crate::{PhysicalRect, ScaleFactor};
use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;

/// Opaque, host-scoped window identifier. Native handles never cross the boundary.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(try_from = "String", into = "String")]
pub struct WindowId(String);

impl WindowId {
    pub fn try_new(value: impl Into<String>) -> Result<Self, WindowIdError> {
        let value = value.into();
        if value.is_empty() {
            return Err(WindowIdError::Empty);
        }
        if value.len() > 256 {
            return Err(WindowIdError::TooLong);
        }
        Ok(Self(value))
    }

    pub fn new(value: impl Into<String>) -> Option<Self> {
        Self::try_new(value).ok()
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for WindowId {
    type Error = WindowIdError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::try_new(value)
    }
}

impl From<WindowId> for String {
    fn from(value: WindowId) -> Self {
        value.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum WindowIdError {
    #[error("window id cannot be empty")]
    Empty,
    #[error("window id is too long")]
    TooLong,
}

impl fmt::Display for WindowId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WindowScope {
    Owned,
    External,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WindowMode {
    #[default]
    Normal,
    Minimized,
    Maximized,
    Fullscreen,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "id")]
pub enum WindowTarget {
    PrimaryOwned,
    Owned(WindowId),
    External(WindowId),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WindowSnapshot {
    pub id: WindowId,
    pub scope: WindowScope,
    pub title: Option<String>,
    pub application_name: Option<String>,
    pub process_id: Option<u32>,
    pub bounds: Option<PhysicalRect>,
    pub scale_factor: Option<ScaleFactor>,
    pub mode: WindowMode,
    pub visible: Option<bool>,
    pub focused: Option<bool>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case", tag = "operation")]
pub enum OwnedWindowRequest {
    List,
    Get {
        target: WindowTarget,
    },
    SetTitle {
        target: WindowTarget,
        title: String,
    },
    SetVisible {
        target: WindowTarget,
        visible: bool,
    },
    SetMode {
        target: WindowTarget,
        mode: WindowMode,
    },
    SetBounds {
        target: WindowTarget,
        bounds: PhysicalRect,
    },
    RequestFocus {
        target: WindowTarget,
    },
    RequestClose {
        target: WindowTarget,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case", tag = "result", content = "value")]
pub enum OwnedWindowResponse {
    Windows(Vec<WindowSnapshot>),
    Window(WindowSnapshot),
    Applied,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case", tag = "operation")]
pub enum ExternalWindowRequest {
    List,
    Get { id: WindowId },
    Activate { id: WindowId },
    SetBounds { id: WindowId, bounds: PhysicalRect },
    RequestClose { id: WindowId },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case", tag = "result", content = "value")]
pub enum ExternalWindowResponse {
    Windows(Vec<WindowSnapshot>),
    Window(WindowSnapshot),
    Applied,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_id_deserialization_rejects_empty_values() {
        let error =
            serde_json::from_str::<WindowId>("\"\"").expect_err("empty window id is invalid");
        assert!(error.to_string().contains("cannot be empty"));
    }
}
