use serde::{Deserialize, Serialize};

/// Minimal UI tree slice.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgentUiTree {
    pub root: String,
    pub children: Vec<String>,
}
