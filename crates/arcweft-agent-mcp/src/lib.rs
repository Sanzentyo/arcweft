//! MCP-facing adapters for Arcweft Agent Debug Bus resources.
//!
//! This crate is Sans I/O. It does not own stdio, HTTP, sessions, or renderer
//! readback. It maps `arcweft-agent-protocol` records into MCP-compatible JSON
//! shapes so CLI, tests, and a future MCP transport share one contract.

use arcweft_agent_protocol::{
    AgentImageComposition, AgentImageKind, AgentImageRenderer, AgentImageScope, AgentResource,
    AgentResourceBody, AgentResourceKind, trace::AgentTraceRecord,
};
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

/// Returns the current Agent Debug Bus tool descriptors.
pub fn agent_tool_descriptors() -> Vec<McpToolDescriptor> {
    vec![
        agent_observe_tool_descriptor(),
        agent_action_tool_descriptor(),
        agent_wait_tool_descriptor(),
        agent_script_run_tool_descriptor(),
        agent_resource_read_tool_descriptor(),
        agent_capture_tool_descriptor(),
        agent_hit_test_tool_descriptor(),
        agent_session_info_tool_descriptor(),
        agent_get_state_tool_descriptor(),
        agent_signal_get_tool_descriptor(),
        agent_log_query_tool_descriptor(),
        agent_debug_search_tool_descriptor(),
        agent_rag_query_tool_descriptor(),
        agent_trace_read_tool_descriptor(),
    ]
}

fn agent_script_run_tool_descriptor() -> McpToolDescriptor {
    McpToolDescriptor {
        name: "arcweft.script.run".to_owned(),
        title: Some("Run Arcweft Agent Script".to_owned()),
        description: "Runs a .awfagent source or .awfb Agent controller bundle through the shared Agent Script runner and returns the structured run report.".to_owned(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Path to a .awfagent source file or .awfb Agent controller bundle." },
                "native_source": { "type": "string", "description": "Optional .arcw source to run the Agent script against using the native Agent session." },
                "manifest": { "type": "string", "description": "Launch manifest path for profile-based native Agent session. Defaults to arcw.toml when profile is supplied." },
                "profile": { "type": "string", "description": "Optional launch profile for the native Agent session. Mutually exclusive with native_source." },
                "entry": { "type": "string" },
                "flow": { "type": "string" },
                "native_steps": { "type": "integer", "minimum": 1, "default": 8 },
                "native_max_ops": { "type": "integer", "minimum": 1, "default": 64 },
                "max_steps": { "type": "integer", "minimum": 1, "default": 256 },
                "max_ops": { "type": "integer", "minimum": 1, "default": 1024 },
                "signals": {
                    "type": "object",
                    "description": "Deterministic CLI-session signal values keyed by signal id, using JSON bool/string/integer values."
                },
                "state": {
                    "type": "object",
                    "description": "Deterministic CLI-session debug state values keyed by dotted state path, using JSON bool/string/integer values."
                },
                "trace_out": { "type": "string", "description": "Optional .arcwx trace output path." },
                "blob_dir": { "type": "string", "description": "Optional directory for byte-backed capture blobs." },
                "run_id": { "type": "string", "default": "run.cli" },
                "viewport_width": { "type": "integer", "minimum": 1, "default": 1280 },
                "viewport_height": { "type": "integer", "minimum": 1, "default": 720 },
                "textbox_height": { "type": "integer", "minimum": 1 },
                "capture_time": { "type": "number", "minimum": 0 }
            },
            "required": ["path"]
        }),
    }
}

fn agent_wait_tool_descriptor() -> McpToolDescriptor {
    McpToolDescriptor {
        name: "arcweft.wait".to_owned(),
        title: Some("Wait For Arcweft Predicate".to_owned()),
        description: "Steps the active native Agent session until a typed Agent predicate is stable or a logical timeout is reached. With source/profile, observes first.".to_owned(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "source": { "type": "string", "description": "Optional .arcw source to observe before waiting. Mutually exclusive with profile." },
                "manifest": { "type": "string", "description": "Launch manifest path for profile-based observe-before-wait. Defaults to arcw.toml when profile is supplied." },
                "profile": { "type": "string", "description": "Optional launch profile to resolve before waiting. Mutually exclusive with source." },
                "entry": { "type": "string" },
                "flow": { "type": "string" },
                "viewport_width": { "type": "integer", "minimum": 1, "default": 1280 },
                "viewport_height": { "type": "integer", "minimum": 1, "default": 720 },
                "textbox_height": { "type": "integer", "minimum": 1 },
                "predicate": { "type": "object", "description": "Agent protocol Predicate JSON, using kind/probe/op/value fields." },
                "timeout_millis": { "type": "integer", "minimum": 1 },
                "stable_frames": { "type": "integer", "minimum": 1, "default": 1 },
                "poll_frames": { "type": "integer", "minimum": 1, "default": 1 }
            },
            "required": ["predicate", "timeout_millis"]
        }),
    }
}

fn agent_action_tool_descriptor() -> McpToolDescriptor {
    McpToolDescriptor {
        name: "arcweft.action".to_owned(),
        title: Some("Dispatch Arcweft Action".to_owned()),
        description: "Dispatches one enabled semantic Agent action from the latest observed frame, or observes a supplied source/profile first, then returns before/after frame state.".to_owned(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "source": { "type": "string", "description": "Optional .arcw source to observe before dispatching. Mutually exclusive with profile." },
                "manifest": { "type": "string", "description": "Launch manifest path for profile-based observe-before-action. Defaults to arcw.toml when profile is supplied." },
                "profile": { "type": "string", "description": "Optional launch profile to resolve before dispatching. Mutually exclusive with source." },
                "entry": { "type": "string" },
                "flow": { "type": "string" },
                "steps": { "type": "integer", "minimum": 1 },
                "capture_step": { "type": "integer", "minimum": 1 },
                "max_ops": { "type": "integer", "minimum": 1 },
                "viewport_width": { "type": "integer", "minimum": 1, "default": 1280 },
                "viewport_height": { "type": "integer", "minimum": 1, "default": 720 },
                "textbox_height": { "type": "integer", "minimum": 1 },
                "action_id": { "type": "string", "description": "Observed Agent action target id, such as action.advance_text.object.dialogue.0.0 or action.inspect.pulse." },
                "kind": { "type": "string", "enum": ["advance_text", "select_choice", "invoke"], "description": "Semantic action kind when action_id is not supplied." },
                "target": { "type": "string", "description": "Target public id/object id. Required for select_choice and invoke when action_id is not supplied." },
                "action": { "type": "string", "description": "Invoke action id. Required for invoke when action_id is not supplied." },
                "args": { "type": "object", "description": "Optional JSON object payload for invoke actions, lowered to AgentValue records." }
            },
            "anyOf": [
                { "required": ["action_id"] },
                { "required": ["kind"] }
            ]
        }),
    }
}

fn agent_observe_tool_descriptor() -> McpToolDescriptor {
    McpToolDescriptor {
            name: "arcweft.observe".to_owned(),
            title: Some("Observe Arcweft".to_owned()),
            description: "Runs a bounded Agent observation and returns resource links for the frame, objects, and optional image capture.".to_owned(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "source": { "type": "string", "description": "Direct .arcw source path. Mutually exclusive with profile." },
                    "manifest": { "type": "string", "description": "Launch manifest path for profile-based observation. Defaults to arcw.toml when profile is supplied." },
                    "profile": { "type": "string", "description": "Launch profile to resolve before observing. Mutually exclusive with source." },
                    "image": { "type": "string", "enum": ["overlay", "png", "raw-rgba"] },
                    "capture": { "type": "string", "enum": ["color", "object-id", "mask"], "default": "color" },
                    "layer": { "type": "string" },
                    "object": { "type": "string" },
                    "page": { "type": "integer", "minimum": 0, "description": "0-based rendered page index for native rich-text captures." },
                    "capture_time": { "type": "number", "minimum": 0, "description": "Native animation sample time in seconds for rich-text effects, shaders, motion functions, typewriter visibility, animated proxy bounds, animated image frame selection, hit-testing, and image capture." },
                    "viewport_width": { "type": "integer", "minimum": 1, "default": 1280, "description": "Observation viewport width in pixels." },
                    "viewport_height": { "type": "integer", "minimum": 1, "default": 720, "description": "Observation viewport height in pixels." },
                    "textbox_height": { "type": "integer", "minimum": 1, "description": "Optional observed dialogue textbox height in pixels for layout-sensitive rich-text debugging." },
                    "steps": { "type": "integer", "minimum": 1 },
                    "capture_step": { "type": "integer", "minimum": 1, "description": "Observe and capture the rendered frame after this many runtime steps. Overrides steps when supplied." },
                    "max_ops": { "type": "integer", "minimum": 1 }
                },
                "anyOf": [
                    { "required": ["source"] },
                    { "required": ["profile"] }
                ]
            }),
        }
}

fn agent_resource_read_tool_descriptor() -> McpToolDescriptor {
    McpToolDescriptor {
        name: "arcweft.resource.read".to_owned(),
        title: Some("Read Arcweft Resource".to_owned()),
        description: "Reads an arcweft:// Agent Debug Bus resource, including PNG/raw image blobs."
            .to_owned(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "uri": { "type": "string" }
            },
            "required": ["uri"]
        }),
    }
}

fn agent_capture_tool_descriptor() -> McpToolDescriptor {
    McpToolDescriptor {
            name: "arcweft.capture".to_owned(),
            title: Some("Capture Arcweft Image".to_owned()),
            description: "Captures the latest observed viewport, layer, or object as PNG or raw RGBA image content; with source, observes first and then captures.".to_owned(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "source": { "type": "string", "description": "Optional .arcw source to observe before capturing. Mutually exclusive with profile." },
                    "manifest": { "type": "string", "description": "Launch manifest path for profile-based observe-before-capture. Defaults to arcw.toml when profile is supplied." },
                    "profile": { "type": "string", "description": "Optional launch profile to resolve before capturing. Mutually exclusive with source." },
                    "entry": { "type": "string" },
                    "flow": { "type": "string" },
                    "steps": { "type": "integer", "minimum": 1 },
                    "capture_step": { "type": "integer", "minimum": 1, "description": "Observe before capturing after this many runtime steps. Overrides steps when supplied." },
                    "max_ops": { "type": "integer", "minimum": 1 },
                    "uri": { "type": "string", "description": "Optional arcweft:// image resource URI from resources/list or resources/templates/list. When supplied, it selects format, capture kind, and viewport/layer/object scope." },
                    "format": { "type": "string", "enum": ["png", "raw-rgba"], "default": "png" },
                    "capture": { "type": "string", "enum": ["color", "object-id", "mask"], "default": "color" },
                    "layer": { "type": "string" },
                    "object": { "type": "string" },
                    "page": { "type": "integer", "minimum": 0, "description": "0-based rendered page index for native rich-text captures." },
                    "capture_time": { "type": "number", "minimum": 0, "description": "Native animation sample time in seconds for rich-text effects, shaders, motion functions, typewriter visibility, animated proxy bounds, animated image frame selection, hit-testing, and image capture." },
                    "viewport_width": { "type": "integer", "minimum": 1, "default": 1280, "description": "Observation viewport width in pixels when source is supplied." },
                    "viewport_height": { "type": "integer", "minimum": 1, "default": 720, "description": "Observation viewport height in pixels when source is supplied." },
                    "textbox_height": { "type": "integer", "minimum": 1, "description": "Optional observed dialogue textbox height in pixels when source is supplied." }
                }
            }),
        }
}

fn agent_hit_test_tool_descriptor() -> McpToolDescriptor {
    McpToolDescriptor {
            name: "arcweft.hit_test".to_owned(),
            title: Some("Hit-Test Arcweft".to_owned()),
            description: "Hit-tests the latest observed Agent frame, or observes a supplied source/profile first, and returns depth-sorted object/region hits with capture_refs for a viewport coordinate.".to_owned(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "source": { "type": "string", "description": "Optional .arcw source to observe before hit-testing. Mutually exclusive with profile." },
                    "manifest": { "type": "string", "description": "Launch manifest path for profile-based observe-before-hit-test. Defaults to arcw.toml when profile is supplied." },
                    "profile": { "type": "string", "description": "Optional launch profile to resolve before hit-testing. Mutually exclusive with source." },
                    "entry": { "type": "string" },
                    "flow": { "type": "string" },
                    "steps": { "type": "integer", "minimum": 1 },
                    "capture_step": { "type": "integer", "minimum": 1, "description": "Observe before hit-testing after this many runtime steps. Overrides steps when supplied." },
                    "max_ops": { "type": "integer", "minimum": 1 },
                    "capture_time": { "type": "number", "minimum": 0, "description": "Native animation sample time in seconds for rich-text effects, shaders, motion functions, typewriter visibility, animated proxy bounds, and animated image frame selection before hit-testing." },
                    "viewport_width": { "type": "integer", "minimum": 1, "default": 1280 },
                    "viewport_height": { "type": "integer", "minimum": 1, "default": 720 },
                    "textbox_height": { "type": "integer", "minimum": 1 },
                    "x": { "type": "integer", "minimum": 0 },
                    "y": { "type": "integer", "minimum": 0 }
                },
                "required": ["x", "y"]
            }),
        }
}

fn agent_session_info_tool_descriptor() -> McpToolDescriptor {
    McpToolDescriptor {
            name: "arcweft.session.info".to_owned(),
            title: Some("Inspect Arcweft Session".to_owned()),
            description: "Returns the latest Agent Debug Bus session/frame state, available resources, and current image metadata.".to_owned(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {}
            }),
        }
}

fn agent_trace_read_tool_descriptor() -> McpToolDescriptor {
    McpToolDescriptor {
        name: "arcweft.trace.read".to_owned(),
        title: Some("Read Agent Trace".to_owned()),
        description: "Loads a validated .arcwx Agent trace and exposes it as an MCP resource link for read-only replay/debugging.".to_owned(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Filesystem path to a .arcwx Agent trace file." }
            },
            "required": ["path"]
        }),
    }
}

fn agent_get_state_tool_descriptor() -> McpToolDescriptor {
    McpToolDescriptor {
        name: "arcweft.get_state".to_owned(),
        title: Some("Get Arcweft State".to_owned()),
        description: "Reads the latest observed Agent state summary, or one dotted field from it. With source/profile, observes first.".to_owned(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "source": { "type": "string", "description": "Optional .arcw source to observe before reading state. Mutually exclusive with profile." },
                "manifest": { "type": "string", "description": "Launch manifest path for profile-based observe-before-read. Defaults to arcw.toml when profile is supplied." },
                "profile": { "type": "string", "description": "Optional launch profile to resolve before reading state. Mutually exclusive with source." },
                "entry": { "type": "string" },
                "flow": { "type": "string" },
                "steps": { "type": "integer", "minimum": 1 },
                "capture_step": { "type": "integer", "minimum": 1 },
                "max_ops": { "type": "integer", "minimum": 1 },
                "path": { "type": "string", "description": "Optional dotted state summary path such as status, final_status, tick, state_hash, or render_hash." }
            }
        }),
    }
}

fn agent_signal_get_tool_descriptor() -> McpToolDescriptor {
    McpToolDescriptor {
        name: "arcweft.signal_get".to_owned(),
        title: Some("Get Arcweft Signal".to_owned()),
        description: "Reads one signal value from the latest observed Agent frame. With source/profile, observes first.".to_owned(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "source": { "type": "string", "description": "Optional .arcw source to observe before reading the signal. Mutually exclusive with profile." },
                "manifest": { "type": "string", "description": "Launch manifest path for profile-based observe-before-read. Defaults to arcw.toml when profile is supplied." },
                "profile": { "type": "string", "description": "Optional launch profile to resolve before reading the signal. Mutually exclusive with source." },
                "entry": { "type": "string" },
                "flow": { "type": "string" },
                "steps": { "type": "integer", "minimum": 1 },
                "capture_step": { "type": "integer", "minimum": 1 },
                "max_ops": { "type": "integer", "minimum": 1 },
                "name": { "type": "string", "description": "Signal id without @, such as signal.current_flow." }
            },
            "required": ["name"]
        }),
    }
}

fn agent_log_query_tool_descriptor() -> McpToolDescriptor {
    McpToolDescriptor {
        name: "arcweft.log_query".to_owned(),
        title: Some("Query Arcweft Logs".to_owned()),
        description: "Filters logs from the latest observed Agent frame by level and message substring. With source/profile, observes first.".to_owned(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "source": { "type": "string", "description": "Optional .arcw source to observe before querying logs. Mutually exclusive with profile." },
                "manifest": { "type": "string", "description": "Launch manifest path for profile-based observe-before-read. Defaults to arcw.toml when profile is supplied." },
                "profile": { "type": "string", "description": "Optional launch profile to resolve before querying logs. Mutually exclusive with source." },
                "entry": { "type": "string" },
                "flow": { "type": "string" },
                "steps": { "type": "integer", "minimum": 1 },
                "capture_step": { "type": "integer", "minimum": 1 },
                "max_ops": { "type": "integer", "minimum": 1 },
                "level": { "type": "string", "description": "Optional exact log level filter." },
                "contains": { "type": "string", "description": "Optional case-sensitive message substring filter." },
                "limit": { "type": "integer", "minimum": 0, "default": 50 }
            }
        }),
    }
}

fn agent_rag_query_tool_descriptor() -> McpToolDescriptor {
    McpToolDescriptor {
        name: "arcweft.rag.query".to_owned(),
        title: Some("Query Arcweft Debug Context".to_owned()),
        description: "Builds an explainable RagContextPack from the current Agent Debug Bus session, or observes a supplied source/profile first.".to_owned(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "source": { "type": "string", "description": "Optional .arcw source to observe before querying. Mutually exclusive with profile." },
                "manifest": { "type": "string", "description": "Launch manifest path for profile-based observe-before-query. Defaults to arcw.toml when profile is supplied." },
                "profile": { "type": "string", "description": "Optional launch profile to resolve before querying. Mutually exclusive with source." },
                "entry": { "type": "string" },
                "flow": { "type": "string" },
                "steps": { "type": "integer", "minimum": 1 },
                "capture_step": { "type": "integer", "minimum": 1 },
                "max_ops": { "type": "integer", "minimum": 1 },
                "query": { "type": "string", "description": "Natural-language or identifier query text." },
                "roots": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Optional public ids or observed object ids to bias retrieval."
                },
                "graph_depth": { "type": "integer", "minimum": 0, "default": 1 },
                "limit": { "type": "integer", "minimum": 1, "default": 8 },
                "max_context_bytes": { "type": "integer", "minimum": 1, "default": 32768 },
                "max_privacy": {
                    "type": "string",
                    "enum": ["public", "project", "sensitive", "secret"],
                    "default": "project",
                    "description": "Highest privacy class allowed in returned context items."
                }
            },
            "required": ["query"]
        }),
    }
}

fn agent_debug_search_tool_descriptor() -> McpToolDescriptor {
    McpToolDescriptor {
        name: "arcweft.debug.search".to_owned(),
        title: Some("Search Arcweft Debug Store".to_owned()),
        description: "Searches the rebuildable Arcweft debug SQLite store through lexical, vector, graph, or history channels with privacy filtering before limit.".to_owned(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Filesystem path to the Arcweft debug SQLite database. Defaults to .arcweft/cache/agent-debug.sqlite3."
                },
                "query": {
                    "type": "string",
                    "description": "Literal query text for the debug-store chunk FTS index."
                },
                "query_vector": {
                    "description": "Vector query for stored embeddings, as an array of numbers or a comma-separated string.",
                    "oneOf": [
                        { "type": "array", "items": { "type": "number" }, "minItems": 1 },
                        { "type": "string" }
                    ]
                },
                "graph_query": {
                    "type": "string",
                    "description": "Text query for indexed symbols and graph edges."
                },
                "graph_depth": { "type": "integer", "minimum": 0, "default": 1 },
                "history_query": {
                    "type": "string",
                    "description": "Text query for indexed history entries."
                },
                "model_id": {
                    "type": "string",
                    "description": "Embedding model id required with query_vector."
                },
                "model_revision": {
                    "type": "string",
                    "description": "Embedding model revision required with query_vector."
                },
                "limit": { "type": "integer", "minimum": 1, "default": 10 },
                "max_privacy": {
                    "type": "string",
                    "enum": ["public", "project", "sensitive", "secret"],
                    "default": "project",
                    "description": "Highest privacy class allowed in returned hits."
                }
            }
        }),
    }
}

/// Returns the Agent Debug Bus resource templates understood by the current
/// one-shot CLI/MCP session model.
pub fn agent_resource_templates() -> Vec<McpResourceTemplateDescriptor> {
    vec![
        resource_template(
            "arcweft://session/{session_id}/observation/latest.json",
            "latest-observation",
            "Latest observation",
            "Latest Agent observation JSON, including viewport, layers, objects, actions, logs, signals, diagnostics, and image resource refs.",
            Some("application/json"),
        ),
        resource_template(
            "arcweft://session/{session_id}/frame/{tick}/objects.json",
            "observed-objects",
            "Observed objects",
            "Observed object JSON for the frame, including textbox and rich-text child bboxes plus object-local capture refs.",
            Some("application/json"),
        ),
        resource_template(
            "arcweft://session/{session_id}/frame/{tick}/presentation-tree.json",
            "presentation-tree",
            "Presentation tree",
            "Typed presentation object tree for the frame, including layer/object hierarchy and lightweight rich-text visual feature indexes.",
            Some("application/json"),
        ),
        resource_template(
            "arcweft://session/{session_id}/frame/{tick}/presentation-tree.json?{filter_key}={filter_value}",
            "presentation-tree-filter",
            "Filtered presentation tree",
            "Typed presentation object tree filtered by role, rich_text_kind, object_layer, effect, shader, motion, proxy id/type/role/struct/params, or has_transform while preserving ancestors.",
            Some("application/json"),
        ),
        resource_template(
            "arcweft://session/{session_id}/frame/{tick}/{capture}.{extension}",
            "viewport-capture",
            "Viewport capture",
            "Full-frame image capture. capture is color, object-id, or mask; extension is png or rgba.",
            None,
        ),
        resource_template(
            "arcweft://session/{session_id}/frame/{tick}/layer.{layer_id}.{extension}",
            "layer-color-capture",
            "Layer color capture",
            "Selected layer color capture. extension is png or rgba.",
            None,
        ),
        resource_template(
            "arcweft://session/{session_id}/frame/{tick}/layer.{layer_id}.object-id.{extension}",
            "layer-object-id-capture",
            "Layer object-id capture",
            "Selected layer object-id capture. extension is png or rgba.",
            None,
        ),
        resource_template(
            "arcweft://session/{session_id}/frame/{tick}/layer.{layer_id}.mask.{extension}",
            "layer-mask-capture",
            "Layer mask capture",
            "Selected layer mask capture. extension is png or rgba.",
            None,
        ),
        resource_template(
            "arcweft://session/{session_id}/frame/{tick}/object.{object_id}.{extension}",
            "object-color-capture",
            "Object color capture",
            "Selected object color capture, including rich-text child objects. extension is png or rgba.",
            None,
        ),
        resource_template(
            "arcweft://session/{session_id}/frame/{tick}/object.{object_id}.object-id.{extension}",
            "object-object-id-capture",
            "Object object-id capture",
            "Selected object object-id capture, including rich-text child objects. extension is png or rgba.",
            None,
        ),
        resource_template(
            "arcweft://session/{session_id}/frame/{tick}/object.{object_id}.mask.{extension}",
            "object-mask-capture",
            "Object mask capture",
            "Selected object mask capture, including rich-text child objects. extension is png or rgba.",
            None,
        ),
        resource_template(
            "arcweft://run/{run_id}/trace.arcwx",
            "agent-trace",
            "Agent trace",
            "Validated Agent execution trace records for read-only replay and regression comparison.",
            Some(AGENT_TRACE_MIME_TYPE),
        ),
    ]
}

/// Converts the static Agent Debug Bus templates into an MCP
/// `resources/templates/list` result.
pub fn list_resource_templates_result() -> McpListResourceTemplatesResult {
    McpListResourceTemplatesResult {
        resource_templates: agent_resource_templates(),
    }
}

/// Converts an Agent resource into an MCP resource descriptor.
pub fn resource_descriptor(resource: &AgentResource) -> McpResourceDescriptor {
    let size = match &resource.body {
        AgentResourceBody::Json(value) => serde_json::to_vec(value)
            .ok()
            .and_then(|bytes| u64::try_from(bytes.len()).ok()),
        AgentResourceBody::Text(text) => u64::try_from(text.len()).ok(),
        AgentResourceBody::BytesBase64(body) => decoded_base64_len(&body.data),
    };
    McpResourceDescriptor {
        uri: resource.uri.clone(),
        name: resource_name(resource),
        title: Some(resource_title(resource)),
        description: Some(resource_description(resource)),
        mime_type: Some(resource.mime_type.clone()),
        size,
    }
}

fn resource_template(
    uri_template: &str,
    name: &str,
    title: &str,
    description: &str,
    mime_type: Option<&str>,
) -> McpResourceTemplateDescriptor {
    McpResourceTemplateDescriptor {
        uri_template: uri_template.to_owned(),
        name: name.to_owned(),
        title: Some(title.to_owned()),
        description: Some(description.to_owned()),
        mime_type: mime_type.map(ToOwned::to_owned),
    }
}

/// Converts Agent resources into an MCP `resources/list` result.
pub fn list_resources_result(resources: &[AgentResource]) -> McpListResourcesResult {
    McpListResourcesResult {
        resources: resources.iter().map(resource_descriptor).collect(),
    }
}

/// Builds an MCP-addressable Agent trace resource from typed trace records.
///
/// The trace remains JSON at this boundary; portable binary/archive packaging
/// for large blobs is handled by higher-level tooling.
pub fn trace_resource(records: &[AgentTraceRecord]) -> Result<AgentResource, serde_json::Error> {
    Ok(AgentResource {
        uri: trace_resource_uri(records),
        kind: AgentResourceKind::Trace,
        mime_type: AGENT_TRACE_MIME_TYPE.to_owned(),
        hash: trace_resource_hash(records),
        image: None,
        body: AgentResourceBody::Json(serde_json::to_value(records)?),
    })
}

fn trace_resource_uri(records: &[AgentTraceRecord]) -> String {
    records.first().map_or_else(
        || "arcweft://run/unknown/trace.arcwx".to_owned(),
        |record| format!("arcweft://run/{}/trace.arcwx", record.run_id.as_str()),
    )
}

fn trace_resource_hash(records: &[AgentTraceRecord]) -> String {
    records.last().map_or_else(
        || "trace:empty".to_owned(),
        |record| {
            format!(
                "trace:{}:{}:{}",
                record.run_id.as_str(),
                records.len(),
                record.payload_hash.as_str()
            )
        },
    )
}

/// Converts an Agent resource into an MCP `resources/read` result.
pub fn read_resource_result(
    resource: &AgentResource,
) -> Result<McpReadResourceResult, serde_json::Error> {
    Ok(McpReadResourceResult {
        contents: vec![resource_contents(resource)?],
    })
}

/// Converts a set of Agent resources into an observe tool result. The result is
/// intentionally link-oriented so MCP clients can choose which image/blob to
/// fetch without embedding every frame resource in the initial tool response.
pub fn tool_result_for_resources(resources: &[AgentResource]) -> McpCallToolResult {
    McpCallToolResult {
        content: resources.iter().map(resource_link).collect(),
        is_error: false,
    }
}

/// Converts an Agent resource into a tool result. Image resources become MCP
/// image content so multimodal clients can render them directly.
pub fn tool_result_for_resource(
    resource: &AgentResource,
) -> Result<McpCallToolResult, serde_json::Error> {
    let content = match &resource.body {
        AgentResourceBody::BytesBase64(body) if resource.mime_type.starts_with("image/") => {
            let mut content = image_metadata_content(resource)?;
            content.push(McpContentBlock::Image {
                data: body.data.clone(),
                mime_type: resource.mime_type.clone(),
            });
            content
        }
        _ => {
            let mut content = image_metadata_content(resource)?;
            content.push(McpContentBlock::Resource {
                resource: resource_contents(resource)?,
            });
            content
        }
    };
    Ok(McpCallToolResult {
        content,
        is_error: false,
    })
}

fn image_metadata_content(
    resource: &AgentResource,
) -> Result<Vec<McpContentBlock>, serde_json::Error> {
    resource.image.as_ref().map_or_else(
        || Ok(Vec::new()),
        |metadata| {
            Ok(vec![McpContentBlock::Text {
                text: serde_json::to_string(&serde_json::json!({
                    "uri": resource.uri,
                    "mime_type": resource.mime_type,
                    "image": metadata,
                }))?,
            }])
        },
    )
}

/// Converts an Agent resource into an MCP content block link.
pub fn resource_link(resource: &AgentResource) -> McpContentBlock {
    let descriptor = resource_descriptor(resource);
    McpContentBlock::ResourceLink {
        uri: descriptor.uri,
        name: descriptor.name,
        title: descriptor.title,
        description: descriptor.description,
        mime_type: descriptor.mime_type,
        size: descriptor.size,
    }
}

fn resource_contents(resource: &AgentResource) -> Result<McpResourceContents, serde_json::Error> {
    match &resource.body {
        AgentResourceBody::Json(value) => Ok(McpResourceContents::Text(McpTextResourceContents {
            uri: resource.uri.clone(),
            mime_type: Some(resource.mime_type.clone()),
            text: serde_json::to_string(value)?,
        })),
        AgentResourceBody::Text(text) => Ok(McpResourceContents::Text(McpTextResourceContents {
            uri: resource.uri.clone(),
            mime_type: Some(resource.mime_type.clone()),
            text: text.clone(),
        })),
        AgentResourceBody::BytesBase64(body) => {
            Ok(McpResourceContents::Blob(McpBlobResourceContents {
                uri: resource.uri.clone(),
                mime_type: Some(resource.mime_type.clone()),
                blob: body.data.clone(),
            }))
        }
    }
}

fn resource_name(resource: &AgentResource) -> String {
    resource
        .uri
        .rsplit('/')
        .next()
        .filter(|name| !name.is_empty())
        .unwrap_or(resource.uri.as_str())
        .to_owned()
}

fn resource_title(resource: &AgentResource) -> String {
    match resource.kind {
        AgentResourceKind::ObservationLatest => "Latest observation",
        AgentResourceKind::Objects => "Observed objects",
        AgentResourceKind::PresentationTree => "Presentation tree",
        AgentResourceKind::OverlaySvg => "Overlay SVG",
        AgentResourceKind::Image => "Captured image",
        AgentResourceKind::Logs => "Runtime logs",
        AgentResourceKind::Signals => "Runtime signals",
        AgentResourceKind::Audio => "Audio state",
        AgentResourceKind::Trace => "Agent trace",
    }
    .to_owned()
}

fn resource_description(resource: &AgentResource) -> String {
    if resource.kind == AgentResourceKind::Trace {
        return format!(
            "Agent execution trace resource for read-only replay ({})",
            resource.mime_type
        );
    }
    if let Some(image) = &resource.image {
        let page = if image.page == 0 {
            String::new()
        } else {
            format!(", page={}", image.page)
        };
        return format!(
            "Agent Debug Bus image resource (mime_type={}, kind={}, renderer={}, scope={}, composition={}{}, width={}, height={})",
            resource.mime_type,
            image_kind_description(image.kind),
            image_renderer_description(image.renderer),
            image_scope_description(&image.scope),
            image_composition_description(image.composition),
            page,
            image.width,
            image.height
        );
    }
    format!("Agent Debug Bus resource ({})", resource.mime_type)
}

fn image_scope_description(scope: &AgentImageScope) -> String {
    match scope {
        AgentImageScope::Viewport => "viewport".to_owned(),
        AgentImageScope::Layer { id } => format!("layer:{id}"),
        AgentImageScope::Object { id } => format!("object:{id}"),
    }
}

fn image_kind_description(kind: AgentImageKind) -> &'static str {
    match kind {
        AgentImageKind::Color => "color",
        AgentImageKind::Overlay => "overlay",
        AgentImageKind::OverlaySvg => "overlay_svg",
        AgentImageKind::ObjectId => "object_id",
        AgentImageKind::Mask => "mask",
    }
}

fn image_renderer_description(renderer: AgentImageRenderer) -> &'static str {
    match renderer {
        AgentImageRenderer::Native => "native",
    }
}

fn image_composition_description(composition: AgentImageComposition) -> &'static str {
    match composition {
        AgentImageComposition::OverlayVector => "overlay_vector",
        AgentImageComposition::Framebuffer => "framebuffer",
        AgentImageComposition::FramebufferCrop => "framebuffer_crop",
        AgentImageComposition::ObjectIdAttachment => "object_id_attachment",
        AgentImageComposition::MaskAttachment => "mask_attachment",
        AgentImageComposition::MaskedFramebufferCrop => "masked_framebuffer_crop",
        AgentImageComposition::IsolatedRegions => "isolated_regions",
        AgentImageComposition::DebugGeometry => "debug_geometry",
    }
}

fn decoded_base64_len(value: &str) -> Option<u64> {
    let padding = value
        .as_bytes()
        .iter()
        .rev()
        .take_while(|byte| **byte == b'=')
        .count();
    let groups = value.len().checked_div(4)?;
    let len = groups.checked_mul(3)?.checked_sub(padding)?;
    u64::try_from(len).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_agent_protocol::{
        AgentBinaryEncoding, AgentBinaryResourceBody, AgentCoordinateSpace, AgentImageComposition,
        AgentImageContentBBox, AgentImageCropOrigin, AgentImageKind, AgentImageMetadata,
        AgentImageRenderer, AgentImageScope, AgentResourceBody,
        ids::{AgentRunId, SessionId, StableHash},
        trace::{AgentTraceKind, AgentTraceRecord},
    };

    #[test]
    fn tool_descriptors_include_wait_control_surface() {
        let tools = agent_tool_descriptors();
        let wait = tools
            .iter()
            .find(|tool| tool.name == "arcweft.wait")
            .expect("wait tool is listed");

        assert_eq!(wait.title.as_deref(), Some("Wait For Arcweft Predicate"));
        assert_eq!(
            wait.input_schema["required"],
            serde_json::json!(["predicate", "timeout_millis"])
        );
        assert_eq!(
            wait.input_schema["properties"]["predicate"]["type"],
            "object"
        );
    }

    #[test]
    fn tool_descriptors_include_script_run_surface() {
        let tools = agent_tool_descriptors();
        let script_run = tools
            .iter()
            .find(|tool| tool.name == "arcweft.script.run")
            .expect("script run tool is listed");

        assert_eq!(
            script_run.title.as_deref(),
            Some("Run Arcweft Agent Script")
        );
        assert_eq!(
            script_run.input_schema["required"],
            serde_json::json!(["path"])
        );
        assert_eq!(
            script_run.input_schema["properties"]["path"]["type"],
            "string"
        );
        assert_eq!(
            script_run.input_schema["properties"]["signals"]["type"],
            "object"
        );
        assert_eq!(
            script_run.input_schema["properties"]["state"]["type"],
            "object"
        );
    }

    #[test]
    fn image_agent_resource_maps_to_mcp_blob_and_image_tool_content() {
        let resource = AgentResource {
            uri: "arcweft://session/cli/frame/0/layer.dialogue.png".to_owned(),
            kind: AgentResourceKind::Image,
            mime_type: "image/png".to_owned(),
            hash: "hash".to_owned(),
            image: Some(AgentImageMetadata {
                kind: AgentImageKind::Color,
                renderer: AgentImageRenderer::Native,
                scope: AgentImageScope::Layer {
                    id: "dialogue".to_owned(),
                },
                composition: AgentImageComposition::MaskedFramebufferCrop,
                page: 0,
                capture_step: 0,
                capture_time_millis: 0,
                width: 320,
                height: 180,
                crop_origin: Some(AgentImageCropOrigin {
                    space: AgentCoordinateSpace::Viewport,
                    x: 96,
                    y: 548,
                }),
                pixel_format: None,
                row_stride_bytes: None,
                content_bbox: Some(AgentImageContentBBox {
                    x: 10,
                    y: 12,
                    width: 32,
                    height: 24,
                }),
                content_viewport_bbox: Some(AgentImageContentBBox {
                    x: 106,
                    y: 560,
                    width: 32,
                    height: 24,
                }),
                content_pixels: Some(512),
                object: None,
                diagnostics: Vec::new(),
            }),
            body: AgentResourceBody::BytesBase64(AgentBinaryResourceBody {
                encoding: AgentBinaryEncoding::Base64,
                data: "iVBORw0KGgo=".to_owned(),
            }),
        };

        let descriptor = resource_descriptor(&resource);
        let read = read_resource_result(&resource).expect("resource read serializes");
        let tool = tool_result_for_resource(&resource).expect("tool result serializes");

        assert_eq!(descriptor.name, "layer.dialogue.png");
        assert_eq!(descriptor.mime_type.as_deref(), Some("image/png"));
        assert_eq!(descriptor.size, Some(8));
        let description = descriptor.description.as_deref().unwrap();
        assert!(description.contains("kind=color"));
        assert!(description.contains("renderer=native"));
        assert!(description.contains("scope=layer:dialogue"));
        assert!(description.contains("composition=masked_framebuffer_crop"));
        assert_eq!(
            image_composition_description(AgentImageComposition::ObjectIdAttachment),
            "object_id_attachment"
        );
        assert_eq!(
            image_composition_description(AgentImageComposition::MaskAttachment),
            "mask_attachment"
        );
        assert!(description.contains("width=320"));
        assert!(description.contains("height=180"));
        assert!(matches!(
            read.contents.as_slice(),
            [McpResourceContents::Blob(McpBlobResourceContents { blob, .. })] if blob == "iVBORw0KGgo="
        ));
        assert!(matches!(
            tool.content.as_slice(),
            [
                McpContentBlock::Text { text },
                McpContentBlock::Image { data, mime_type },
            ] if text.contains("\"width\":320")
                && text.contains("\"renderer\":\"native\"")
                && text.contains("\"scope\"")
                && text.contains("\"kind\":\"layer\"")
                && text.contains("\"id\":\"dialogue\"")
                && text.contains("\"composition\":\"masked_framebuffer_crop\"")
                && text.contains("\"crop_origin\"")
                && text.contains("\"content_viewport_bbox\"")
                && text.contains("\"content_pixels\":512")
                && data == "iVBORw0KGgo="
                && mime_type == "image/png"
        ));
    }

    #[test]
    fn image_tool_content_preserves_object_rich_text_ref_metadata() {
        let metadata: AgentImageMetadata =
            serde_json::from_value(proxy_object_image_metadata_fixture())
                .expect("object image metadata deserializes");
        let resource = AgentResource {
            uri: "arcweft://session/cli/frame/0/object.object.dialogue.0.0.proxy.0.0.mask.rgba"
                .to_owned(),
            kind: AgentResourceKind::Image,
            mime_type: "application/octet-stream".to_owned(),
            hash: "hash".to_owned(),
            image: Some(metadata),
            body: AgentResourceBody::BytesBase64(AgentBinaryResourceBody {
                encoding: AgentBinaryEncoding::Base64,
                data: "AAAA".to_owned(),
            }),
        };

        let tool = tool_result_for_resource(&resource).expect("tool result serializes");

        let [
            McpContentBlock::Text { text },
            McpContentBlock::Resource { .. },
        ] = tool.content.as_slice()
        else {
            panic!(
                "raw image tool result should expose metadata text plus resource blob: {:?}",
                tool.content
            );
        };
        let json: serde_json::Value =
            serde_json::from_str(text).expect("metadata text is JSON object");
        assert_eq!(
            json["image"]["object"]["id"],
            "object.dialogue.0.0.proxy.0.0"
        );
        assert_eq!(
            json["image"]["object"]["rich_text_ref"]["kind"],
            "text_object_proxy"
        );
        assert_eq!(
            json["image"]["object"]["rich_text_ref"]["presentation"]["object_proxies"][0]["params"]
                ["channel"]["value"],
            "choice"
        );
        assert_eq!(
            json["image"]["object"]["bbox"]["space"],
            serde_json::json!("viewport")
        );
        assert_eq!(
            json["image"]["object"]["capture_refs"]["object_id_color"]["alpha"],
            255
        );
    }

    #[test]
    fn image_tool_content_preserves_image_object_frame_metadata() {
        let metadata: AgentImageMetadata =
            serde_json::from_value(image_object_frame_metadata_fixture())
                .expect("image object frame metadata deserializes");
        let resource = AgentResource {
            uri: "arcweft://session/cli/frame/0/object.object.image.layer.foreground.0.1.rgba"
                .to_owned(),
            kind: AgentResourceKind::Image,
            mime_type: "application/octet-stream".to_owned(),
            hash: "hash".to_owned(),
            image: Some(metadata),
            body: AgentResourceBody::BytesBase64(AgentBinaryResourceBody {
                encoding: AgentBinaryEncoding::Base64,
                data: "AAAA".to_owned(),
            }),
        };

        let tool = tool_result_for_resource(&resource).expect("tool result serializes");

        let [
            McpContentBlock::Text { text },
            McpContentBlock::Resource { .. },
        ] = tool.content.as_slice()
        else {
            panic!(
                "raw image tool result should expose metadata text plus resource blob: {:?}",
                tool.content
            );
        };
        let json: serde_json::Value =
            serde_json::from_str(text).expect("metadata text is JSON object");
        assert_eq!(
            json["image"]["object"]["image_ref"]["asset"],
            "asset.bg.pulse"
        );
        assert_eq!(json["image"]["object"]["image_ref"]["frame_index"], 1);
        assert_eq!(
            json["image"]["object"]["image_ref"]["local_time_millis"],
            150
        );
        assert_eq!(
            json["image"]["object"]["image_ref"]["proxies"][0]["id"],
            "proxy.pulse_sprite.hotspot"
        );
        assert_eq!(
            json["image"]["object"]["image_ref"]["params"]["param.role"]["value"],
            "animated-hotspot"
        );
    }

    fn proxy_object_image_metadata_fixture() -> serde_json::Value {
        serde_json::json!({
            "kind": "mask",
            "renderer": "native",
            "scope": { "kind": "object", "id": "object.dialogue.0.0.proxy.0.0" },
            "composition": "mask_attachment",
            "width": 12,
            "height": 8,
            "pixel_format": "rgba8_unorm",
            "row_stride_bytes": 48,
            "content_pixels": 24,
            "object": {
                "id": "object.dialogue.0.0.proxy.0.0",
                "layer": "dialogue.rich_text",
                "role": "rich_text_proxy",
                "bbox": { "space": "viewport", "x": 120, "y": 520, "width": 12, "height": 8 },
                "polygon": [
                    { "x": 120, "y": 520 },
                    { "x": 132, "y": 520 },
                    { "x": 132, "y": 528 },
                    { "x": 120, "y": 528 }
                ],
                "capture_refs": {
                    "object_id_color": {
                        "red": 10,
                        "green": 20,
                        "blue": 30,
                        "alpha": 255
                    },
                    "captures": [{
                        "kind": "mask",
                        "uri": "arcweft://session/cli/frame/0/object.object.dialogue.0.0.proxy.0.0.mask.rgba",
                        "mime_type": "application/octet-stream",
                        "width": 12,
                        "height": 8
                    }]
                },
                "text": "proxy",
                "rich_text_ref": {
                    "kind": "text_object_proxy",
                    "index": 0,
                    "range": { "start": 10, "end": 15 },
                    "node_index": 3,
                    "presentation": {
                        "object_proxies": [{
                            "id": "hotspot",
                            "type_name": "KeywordHit",
                            "role": "keyword",
                            "depth": 4000,
                            "hit_test": true,
                            "params": {
                                "channel": { "kind": "selector", "value": "choice" }
                            }
                        }]
                    },
                    "object_depth": 4000,
                    "hit_test": true,
                    "hit_regions": []
                }
            }
        })
    }

    fn image_object_frame_metadata_fixture() -> serde_json::Value {
        serde_json::from_str(
            r#"{
                "kind": "color",
                "renderer": "native",
                "scope": { "kind": "object", "id": "object.image.layer.foreground.0.1" },
                "composition": "framebuffer_crop",
                "width": 360,
                "height": 180,
                "pixel_format": "rgba8_unorm",
                "row_stride_bytes": 1440,
                "content_pixels": 64800,
                "object": {
                    "id": "object.image.layer.foreground.0.1",
                    "entity": "image.sample.pulse_sprite",
                    "layer": "layer.foreground",
                    "role": "image",
                    "bbox": { "space": "viewport", "x": 120, "y": 84, "width": 360, "height": 180 },
                    "polygon": [
                        { "x": 120, "y": 84 },
                        { "x": 480, "y": 84 },
                        { "x": 480, "y": 264 },
                        { "x": 120, "y": 264 }
                    ],
                    "capture_refs": {
                        "object_id_color": {
                            "red": 10,
                            "green": 20,
                            "blue": 30,
                            "alpha": 255
                        },
                        "captures": [{
                            "kind": "color",
                            "uri": "arcweft://session/cli/frame/0/object.object.image.layer.foreground.0.1.rgba",
                            "mime_type": "application/octet-stream",
                            "width": 360,
                            "height": 180
                        }]
                    },
                    "object_layer": "layer.foreground",
                    "object_depth": 2500,
                    "image_ref": {
                        "source": "ui.image.1",
                        "object": "image.sample.pulse_sprite",
                        "target": "target.sample.pulse_sprite",
                        "asset": "asset.bg.pulse",
                        "frame_index": 1,
                        "local_time_millis": 150,
                        "opacity_milli": 500,
                        "intrinsic_width": 2,
                        "intrinsic_height": 1,
                        "actions": ["action.inspect.pulse_sprite"],
                        "params": {
                            "param.role": { "kind": "text", "value": "animated-hotspot" }
                        },
                        "proxies": [{
                            "id": "proxy.pulse_sprite.hotspot",
                            "type_name": "PulseSpriteHotspot",
                            "role": "inspect",
                            "layer": "layer.hit",
                            "depth": 2600,
                            "hit_test": true,
                            "params": {
                                "param.channel": { "kind": "text", "value": "preview" }
                            }
                        }]
                    }
                }
            }"#,
        )
        .expect("image object frame metadata fixture is valid JSON")
    }

    #[test]
    fn resource_list_and_observe_tool_result_expose_resource_links() {
        let resources = vec![
            AgentResource {
                uri: "arcweft://session/cli/observation/latest.json".to_owned(),
                kind: AgentResourceKind::ObservationLatest,
                mime_type: "application/json".to_owned(),
                hash: "hash".to_owned(),
                image: None,
                body: AgentResourceBody::Json(serde_json::json!({ "status": "ok" })),
            },
            AgentResource {
                uri: "arcweft://session/cli/frame/0/layer.dialogue.object-id.png".to_owned(),
                kind: AgentResourceKind::Image,
                mime_type: "image/png".to_owned(),
                hash: "hash".to_owned(),
                image: Some(AgentImageMetadata {
                    kind: AgentImageKind::ObjectId,
                    renderer: AgentImageRenderer::Native,
                    scope: AgentImageScope::Layer {
                        id: "dialogue".to_owned(),
                    },
                    composition: AgentImageComposition::ObjectIdAttachment,
                    page: 0,
                    capture_step: 0,
                    capture_time_millis: 0,
                    width: 320,
                    height: 180,
                    crop_origin: None,
                    pixel_format: None,
                    row_stride_bytes: None,
                    content_bbox: None,
                    content_viewport_bbox: None,
                    content_pixels: None,
                    object: None,
                    diagnostics: Vec::new(),
                }),
                body: AgentResourceBody::BytesBase64(AgentBinaryResourceBody {
                    encoding: AgentBinaryEncoding::Base64,
                    data: "iVBORw0KGgo=".to_owned(),
                }),
            },
        ];

        let list = list_resources_result(&resources);
        let tool = tool_result_for_resources(&resources);

        assert_eq!(list.resources.len(), 2);
        assert_eq!(list.resources[1].name, "layer.dialogue.object-id.png");
        assert_eq!(list.resources[1].mime_type.as_deref(), Some("image/png"));
        assert!(
            list.resources[1]
                .description
                .as_deref()
                .is_some_and(|description| description.contains("kind=object_id")
                    && description.contains("renderer=native")
                    && description.contains("scope=layer:dialogue")
                    && description.contains("composition=object_id_attachment"))
        );
        assert!(matches!(
            tool.content.as_slice(),
            [
                McpContentBlock::ResourceLink { name: first, .. },
                McpContentBlock::ResourceLink { name: second, mime_type: Some(mime_type), .. },
            ] if first == "latest.json" && second == "layer.dialogue.object-id.png" && mime_type == "image/png"
        ));
    }

    #[test]
    fn resource_templates_list_capture_uri_patterns() {
        let templates = list_resource_templates_result();

        assert!(templates.resource_templates.iter().any(|template| {
            template.name == "viewport-capture"
                && template.uri_template
                    == "arcweft://session/{session_id}/frame/{tick}/{capture}.{extension}"
        }));
        assert!(templates.resource_templates.iter().any(|template| {
            template.name == "layer-mask-capture"
                && template
                    .uri_template
                    .contains("layer.{layer_id}.mask.{extension}")
                && template
                    .description
                    .as_deref()
                    .is_some_and(|description| description.contains("png or rgba"))
        }));
        assert!(templates.resource_templates.iter().any(|template| {
            template.name == "layer-object-id-capture"
                && template
                    .uri_template
                    .contains("layer.{layer_id}.object-id.{extension}")
                && template
                    .description
                    .as_deref()
                    .is_some_and(|description| description.contains("png or rgba"))
        }));
        assert!(templates.resource_templates.iter().any(|template| {
            template.name == "object-color-capture"
                && template
                    .uri_template
                    .contains("object.{object_id}.{extension}")
                && template
                    .description
                    .as_deref()
                    .is_some_and(|description| description.contains("rich-text child objects"))
        }));
        assert!(templates.resource_templates.iter().any(|template| {
            template.name == "object-object-id-capture"
                && template
                    .uri_template
                    .contains("object.{object_id}.object-id.{extension}")
                && template
                    .description
                    .as_deref()
                    .is_some_and(|description| description.contains("rich-text child objects"))
        }));
        assert!(templates.resource_templates.iter().any(|template| {
            template.name == "presentation-tree-filter"
                && template
                    .uri_template
                    .contains("presentation-tree.json?{filter_key}={filter_value}")
                && template.description.as_deref().is_some_and(|description| {
                    description.contains("proxy id/type/role/struct/params")
                        && description.contains("preserving ancestors")
                })
        }));
        assert!(templates.resource_templates.iter().any(|template| {
            template.name == "agent-trace"
                && template.uri_template == "arcweft://run/{run_id}/trace.arcwx"
                && template.mime_type.as_deref() == Some(AGENT_TRACE_MIME_TYPE)
        }));
    }

    #[test]
    fn trace_resource_maps_to_mcp_text_resource_and_link() {
        let records = trace_records_fixture();
        let resource = trace_resource(&records).expect("trace resource serializes");
        let list = list_resources_result(std::slice::from_ref(&resource));
        let read = read_resource_result(&resource).expect("trace resource reads");
        let tool = tool_result_for_resource(&resource).expect("trace tool result serializes");

        assert_eq!(resource.kind, AgentResourceKind::Trace);
        assert_eq!(resource.uri, "arcweft://run/run.cli/trace.arcwx");
        assert_eq!(resource.mime_type, AGENT_TRACE_MIME_TYPE);
        assert_eq!(resource.hash, "trace:run.cli:2:blake3:run-finished-payload");
        assert_eq!(list.resources[0].name, "trace.arcwx");
        assert_eq!(list.resources[0].title.as_deref(), Some("Agent trace"));
        assert!(
            list.resources[0]
                .description
                .as_deref()
                .is_some_and(|description| description.contains("read-only replay"))
        );
        assert!(matches!(
            read.contents.as_slice(),
            [McpResourceContents::Text(McpTextResourceContents { mime_type: Some(mime_type), text, .. })]
                if mime_type == AGENT_TRACE_MIME_TYPE && text.contains("\"run_finished\"")
        ));
        assert!(matches!(
            tool.content.as_slice(),
            [McpContentBlock::Resource { resource: McpResourceContents::Text(McpTextResourceContents { uri, .. }) }]
                if uri == "arcweft://run/run.cli/trace.arcwx"
        ));
    }

    #[test]
    fn json_agent_resource_maps_to_mcp_text_resource() {
        let resource = AgentResource {
            uri: "arcweft://session/cli/observation/latest.json".to_owned(),
            kind: AgentResourceKind::ObservationLatest,
            mime_type: "application/json".to_owned(),
            hash: "hash".to_owned(),
            image: None,
            body: AgentResourceBody::Json(serde_json::json!({ "status": "ok" })),
        };

        let read = read_resource_result(&resource).expect("resource read serializes");
        let link = resource_link(&resource);

        assert!(matches!(
            read.contents.as_slice(),
            [McpResourceContents::Text(McpTextResourceContents { text, .. })] if text == "{\"status\":\"ok\"}"
        ));
        assert!(matches!(
            link,
            McpContentBlock::ResourceLink { name, mime_type: Some(mime_type), .. }
                if name == "latest.json" && mime_type == "application/json"
        ));
    }

    #[test]
    fn agent_tools_describe_observe_and_resource_read() {
        let tools = agent_tool_descriptors();

        assert!(tools.iter().any(|tool| tool.name == "arcweft.observe"));
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "arcweft.resource.read")
        );
        assert!(tools.iter().any(|tool| tool.name == "arcweft.capture"));
        assert!(tools.iter().any(|tool| tool.name == "arcweft.hit_test"));
        assert!(tools.iter().any(|tool| tool.name == "arcweft.session.info"));
        assert!(tools.iter().any(|tool| tool.name == "arcweft.get_state"));
        assert!(tools.iter().any(|tool| tool.name == "arcweft.signal_get"));
        assert!(tools.iter().any(|tool| tool.name == "arcweft.log_query"));
        assert!(tools.iter().any(|tool| tool.name == "arcweft.rag.query"));
        assert!(tools.iter().any(|tool| tool.name == "arcweft.trace.read"));
    }

    #[test]
    fn tool_schemas_expose_image_capture_scope_and_uri() {
        let tools = agent_tool_descriptors();
        let observe = tools
            .iter()
            .find(|tool| tool.name == "arcweft.observe")
            .expect("observe tool is described");
        let properties = &observe.input_schema["properties"];

        assert_eq!(
            properties["image"]["enum"],
            serde_json::json!(["overlay", "png", "raw-rgba"])
        );
        assert_eq!(
            properties["capture"]["enum"],
            serde_json::json!(["color", "object-id", "mask"])
        );
        assert!(properties.get("renderer").is_none());
        assert_eq!(properties["source"]["type"], "string");
        assert_eq!(properties["manifest"]["type"], "string");
        assert_eq!(properties["profile"]["type"], "string");
        assert_eq!(
            observe.input_schema["anyOf"],
            serde_json::json!([
                { "required": ["source"] },
                { "required": ["profile"] }
            ])
        );
        assert_eq!(properties["layer"]["type"], "string");
        assert_eq!(properties["object"]["type"], "string");
        assert_eq!(properties["page"]["type"], "integer");
        assert_eq!(properties["page"]["minimum"], 0);
        assert_eq!(properties["capture_time"]["type"], "number");
        assert_eq!(properties["capture_time"]["minimum"], 0);
        assert_capture_time_description_mentions_animated_presentation_objects(
            &properties["capture_time"],
            true,
        );
        assert_eq!(properties["capture_step"]["type"], "integer");
        assert_eq!(properties["capture_step"]["minimum"], 1);
        assert_eq!(properties["viewport_width"]["type"], "integer");
        assert_eq!(properties["viewport_width"]["minimum"], 1);
        assert_eq!(properties["viewport_height"]["type"], "integer");
        assert_eq!(properties["viewport_height"]["minimum"], 1);
        assert_eq!(properties["textbox_height"]["type"], "integer");
        assert_eq!(properties["textbox_height"]["minimum"], 1);

        let capture = tools
            .iter()
            .find(|tool| tool.name == "arcweft.capture")
            .expect("capture tool is described");
        let properties = &capture.input_schema["properties"];
        assert_eq!(properties["uri"]["type"], "string");
        assert_eq!(properties["source"]["type"], "string");
        assert_eq!(properties["manifest"]["type"], "string");
        assert_eq!(properties["profile"]["type"], "string");
        assert!(properties.get("renderer").is_none());
        assert_eq!(
            properties["format"]["enum"],
            serde_json::json!(["png", "raw-rgba"])
        );
        assert_eq!(properties["page"]["type"], "integer");
        assert_eq!(properties["page"]["minimum"], 0);
        assert_eq!(properties["capture_time"]["type"], "number");
        assert_eq!(properties["capture_time"]["minimum"], 0);
        assert_capture_time_description_mentions_animated_presentation_objects(
            &properties["capture_time"],
            true,
        );
        assert_eq!(properties["capture_step"]["type"], "integer");
        assert_eq!(properties["capture_step"]["minimum"], 1);
        assert_eq!(properties["viewport_width"]["type"], "integer");
        assert_eq!(properties["viewport_width"]["minimum"], 1);
        assert_eq!(properties["viewport_height"]["type"], "integer");
        assert_eq!(properties["viewport_height"]["minimum"], 1);
        assert_eq!(properties["textbox_height"]["type"], "integer");
        assert_eq!(properties["textbox_height"]["minimum"], 1);
    }

    #[test]
    fn hit_test_tool_schema_requires_viewport_coordinate() {
        let tools = agent_tool_descriptors();
        let hit_test = tools
            .iter()
            .find(|tool| tool.name == "arcweft.hit_test")
            .expect("hit-test tool is described");
        let properties = &hit_test.input_schema["properties"];

        assert_eq!(
            hit_test.input_schema["required"],
            serde_json::json!(["x", "y"])
        );
        assert_eq!(properties["x"]["type"], "integer");
        assert_eq!(properties["x"]["minimum"], 0);
        assert_eq!(properties["y"]["type"], "integer");
        assert_eq!(properties["y"]["minimum"], 0);
        assert_eq!(properties["capture_time"]["type"], "number");
        assert_capture_time_description_mentions_animated_presentation_objects(
            &properties["capture_time"],
            false,
        );
        assert_eq!(properties["capture_step"]["minimum"], 1);
    }

    #[test]
    fn debug_read_tool_schemas_expose_state_signal_and_log_filters() {
        let tools = agent_tool_descriptors();
        let action = tools
            .iter()
            .find(|tool| tool.name == "arcweft.action")
            .expect("action tool is described");
        assert_eq!(
            action.input_schema["properties"]["kind"]["enum"],
            serde_json::json!(["advance_text", "select_choice", "invoke"])
        );
        assert_eq!(
            action.input_schema["properties"]["action_id"]["type"],
            "string"
        );
        assert_eq!(action.input_schema["properties"]["args"]["type"], "object");

        let state = tools
            .iter()
            .find(|tool| tool.name == "arcweft.get_state")
            .expect("state tool is described");
        assert_eq!(state.input_schema["properties"]["path"]["type"], "string");
        assert_eq!(state.input_schema["properties"]["source"]["type"], "string");
        assert_eq!(
            state.input_schema["properties"]["profile"]["type"],
            "string"
        );

        let signal = tools
            .iter()
            .find(|tool| tool.name == "arcweft.signal_get")
            .expect("signal tool is described");
        assert_eq!(signal.input_schema["required"], serde_json::json!(["name"]));
        assert_eq!(signal.input_schema["properties"]["name"]["type"], "string");

        let logs = tools
            .iter()
            .find(|tool| tool.name == "arcweft.log_query")
            .expect("log query tool is described");
        assert_eq!(logs.input_schema["properties"]["level"]["type"], "string");
        assert_eq!(
            logs.input_schema["properties"]["contains"]["type"],
            "string"
        );
        assert_eq!(logs.input_schema["properties"]["limit"]["minimum"], 0);

        let debug_search = tools
            .iter()
            .find(|tool| tool.name == "arcweft.debug.search")
            .expect("debug search tool is described");
        assert_eq!(
            debug_search.input_schema["properties"]["path"]["type"],
            "string"
        );
        assert_eq!(
            debug_search.input_schema["properties"]["query"]["type"],
            "string"
        );
        assert!(debug_search.input_schema["properties"]["query_vector"]["oneOf"].is_array());
        assert_eq!(
            debug_search.input_schema["properties"]["graph_query"]["type"],
            "string"
        );
        assert_eq!(
            debug_search.input_schema["properties"]["graph_depth"]["minimum"],
            0
        );
        assert_eq!(
            debug_search.input_schema["properties"]["history_query"]["type"],
            "string"
        );
        assert_eq!(
            debug_search.input_schema["properties"]["model_id"]["type"],
            "string"
        );
        assert_eq!(
            debug_search.input_schema["properties"]["model_revision"]["type"],
            "string"
        );
        assert_eq!(
            debug_search.input_schema["properties"]["limit"]["minimum"],
            1
        );
        assert_eq!(
            debug_search.input_schema["properties"]["max_privacy"]["enum"],
            serde_json::json!(["public", "project", "sensitive", "secret"])
        );

        let rag = tools
            .iter()
            .find(|tool| tool.name == "arcweft.rag.query")
            .expect("rag query tool is described");
        assert_eq!(rag.input_schema["required"], serde_json::json!(["query"]));
        assert_eq!(rag.input_schema["properties"]["query"]["type"], "string");
        assert_eq!(rag.input_schema["properties"]["roots"]["type"], "array");
        assert_eq!(rag.input_schema["properties"]["graph_depth"]["minimum"], 0);
        assert_eq!(rag.input_schema["properties"]["limit"]["minimum"], 1);
        assert_eq!(
            rag.input_schema["properties"]["max_context_bytes"]["minimum"],
            1
        );
        assert_eq!(
            rag.input_schema["properties"]["max_privacy"]["enum"],
            serde_json::json!(["public", "project", "sensitive", "secret"])
        );
    }

    fn assert_capture_time_description_mentions_animated_presentation_objects(
        property: &serde_json::Value,
        includes_image_capture: bool,
    ) {
        let description = property["description"]
            .as_str()
            .expect("capture_time description");
        assert!(description.contains("animation sample time"));
        assert!(description.contains("motion functions"));
        assert!(description.contains("typewriter visibility"));
        assert!(description.contains("animated proxy bounds"));
        assert!(description.contains("animated image frame selection"));
        if includes_image_capture {
            assert!(description.contains("image capture"));
        } else {
            assert!(description.contains("before hit-testing"));
        }
    }

    fn trace_records_fixture() -> Vec<AgentTraceRecord> {
        vec![
            trace_record(0, AgentTraceKind::RunStarted, "blake3:run-started-payload"),
            trace_record(
                1,
                AgentTraceKind::RunFinished,
                "blake3:run-finished-payload",
            ),
        ]
    }

    fn trace_record(sequence: u64, kind: AgentTraceKind, payload_hash: &str) -> AgentTraceRecord {
        AgentTraceRecord {
            schema_version: 1,
            run_id: AgentRunId::new("run.cli").expect("test run id"),
            session_id: Some(SessionId::new("session.cli").expect("test session id")),
            sequence,
            tick: None,
            kind,
            payload_hash: StableHash::new(payload_hash).expect("test trace hash"),
            payload: serde_json::json!({ "sequence": sequence }),
            blob_refs: Vec::new(),
        }
    }
}
