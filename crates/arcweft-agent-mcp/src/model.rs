//! Shared MCP DTOs and constants for Arcweft Agent Debug Bus resources.
//!
//! The module keeps the transport-neutral shapes in one place so tools and
//! resource adapters can share the same contract without a broad root facade.

use serde::{Deserialize, Serialize};

pub const AGENT_TRACE_MIME_TYPE: &str = "application/vnd.arcweft.agent-trace+json";

/// Resource descriptor returned by MCP `resources/list`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct McpResourceDescriptor {
    pub uri: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(rename = "mimeType", skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
}

/// Result body returned by MCP `resources/list`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct McpListResourcesResult {
    pub resources: Vec<McpResourceDescriptor>,
}

/// Resource template descriptor returned by MCP `resources/templates/list`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct McpResourceTemplateDescriptor {
    #[serde(rename = "uriTemplate")]
    pub uri_template: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(rename = "mimeType", skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}

/// Result body returned by MCP `resources/templates/list`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct McpListResourceTemplatesResult {
    #[serde(rename = "resourceTemplates")]
    pub resource_templates: Vec<McpResourceTemplateDescriptor>,
}

/// Result body returned by MCP `resources/read`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct McpReadResourceResult {
    pub contents: Vec<McpResourceContents>,
}

/// MCP resource content. Text resources carry `text`; binary resources carry
/// base64 `blob`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(untagged)]
pub enum McpResourceContents {
    Text(McpTextResourceContents),
    Blob(McpBlobResourceContents),
}

/// Text resource content for MCP.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct McpTextResourceContents {
    pub uri: String,
    #[serde(rename = "mimeType", skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    pub text: String,
}

/// Binary resource content for MCP.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct McpBlobResourceContents {
    pub uri: String,
    #[serde(rename = "mimeType", skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    pub blob: String,
}

/// Tool descriptor returned by MCP `tools/list`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct McpToolDescriptor {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub description: String,
    #[serde(rename = "inputSchema")]
    pub input_schema: serde_json::Value,
}

/// Result returned by MCP `tools/call`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct McpCallToolResult {
    pub content: Vec<McpContentBlock>,
    #[serde(rename = "isError")]
    pub is_error: bool,
}

/// MCP content blocks relevant to Agent observation tools.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum McpContentBlock {
    Text {
        text: String,
    },
    Image {
        data: String,
        #[serde(rename = "mimeType")]
        mime_type: String,
    },
    Resource {
        resource: McpResourceContents,
    },
    ResourceLink {
        uri: String,
        name: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        title: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        description: Option<String>,
        #[serde(rename = "mimeType", skip_serializing_if = "Option::is_none")]
        mime_type: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        size: Option<u64>,
    },
}
