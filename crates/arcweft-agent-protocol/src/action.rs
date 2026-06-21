use serde::{Deserialize, Serialize};

/// Action that can be invoked semantically or via future input synthesis.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgentActionTarget {
    pub id: String,
    pub target: String,
    pub action: AgentActionKind,
    pub kind: AgentActionDispatch,
    pub enabled: bool,
}

/// Semantic action kind.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentActionKind {
    AdvanceText,
    SelectChoice,
    Invoke,
    PointerClick,
}

/// How an action should be dispatched.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentActionDispatch {
    Semantic,
    Physical,
}
