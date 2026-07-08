use serde::{Deserialize, Serialize};

/// Minimal View tree slice.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgentViewTree {
    pub root: String,
    pub children: Vec<String>,
}
