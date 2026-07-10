use serde::{Deserialize, Serialize};

/// Minimal View tree slice.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgentViewTree {
    pub root: String,
    pub children: Vec<String>,
}

/// One authored Scroll target and its internal viewport/content metadata.
///
/// The internal parts explain the retained geometry, but are not separate
/// semantic or actionable nodes.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct AgentObservedScrollRegion {
    pub target: String,
    pub role: AgentScrollRegionRole,
    pub parts: AgentScrollRegionParts,
    pub axis: AgentScrollAxis,
    pub overflow: AgentScrollOverflow,
    pub indicators: AgentScrollIndicatorsPolicy,
    pub overscroll: AgentScrollOverscrollPolicy,
    pub auto_scroll_focus: AgentFocusAutoScrollPolicy,
}

/// Semantic role of an authored scroll observation target.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentScrollRegionRole {
    ScrollRegion,
}

/// Internal retained parts owned by one authored Scroll target.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct AgentScrollRegionParts {
    pub viewport: AgentScrollViewportPart,
    pub content: AgentScrollContentPart,
}

/// Non-actionable scroll viewport metadata in logical pixels.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct AgentScrollViewportPart {
    pub internal: bool,
    /// `[x, y, width, height]` in viewport-space logical pixels.
    pub bounds: [f64; 4],
}

/// Non-actionable retained content metadata in logical pixels.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct AgentScrollContentPart {
    pub internal: bool,
    /// `[width, height]` before viewport clipping.
    pub size: [f64; 2],
    /// Persisted, clamped `[x, y]` content offset.
    pub offset: [f64; 2],
    /// Maximum persisted `[x, y]` content offset.
    pub max_offset: [f64; 2],
}

/// Primary axis owned by a scroll region.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentScrollAxis {
    Vertical,
    Horizontal,
}

/// Overflow behavior that determines whether a region is actionable.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentScrollOverflow {
    Auto,
    Scroll,
    Hidden,
}

/// Effective scroll-indicator policy.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentScrollIndicatorsPolicy {
    Auto,
    Visible,
    Hidden,
}

/// Effective boundary behavior for unconsumed scroll delta.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentScrollOverscrollPolicy {
    Clamp,
    Contain,
    Elastic,
}

/// Effective focus auto-scroll alignment policy.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentFocusAutoScrollPolicy {
    Nearest,
    Start,
    End,
    Disabled,
}
