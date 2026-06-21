use serde::{Deserialize, Serialize};

/// Diagnostic attached to an observation frame.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgentDiagnostic {
    pub step: usize,
    pub severity: AgentDiagnosticSeverity,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effect_id: Option<String>,
    pub message: String,
}

/// Diagnostic severity.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentDiagnosticSeverity {
    Error,
    Warning,
    Info,
}
