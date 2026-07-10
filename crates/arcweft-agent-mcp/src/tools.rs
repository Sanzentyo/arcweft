//! MCP tool descriptor builders for Arcweft Agent Debug Bus.
//!
//! This module owns the JSON Schema payloads for `tools/list`.

use serde_json;

use crate::model::McpToolDescriptor;
use crate::repl_command::MCP_REPL_COMMAND_TOOL;

pub fn agent_tool_descriptors() -> Vec<McpToolDescriptor> {
    vec![
        agent_observe_tool_descriptor(),
        agent_action_tool_descriptor(),
        agent_act_alias_tool_descriptor(),
        agent_session_step_frames_tool_descriptor(),
        agent_wait_tool_descriptor(),
        agent_script_run_tool_descriptor(),
        agent_resource_read_tool_descriptor(),
        agent_capture_tool_descriptor(),
        agent_hit_test_tool_descriptor(),
        agent_session_info_tool_descriptor(),
        agent_repl_command_tool_descriptor(),
        agent_get_state_tool_descriptor(),
        agent_signal_get_tool_descriptor(),
        agent_log_query_tool_descriptor(),
        agent_debug_search_tool_descriptor(),
        agent_rag_query_tool_descriptor(),
        agent_rag_explain_tool_descriptor(),
        agent_rag_context_read_tool_descriptor(),
        agent_debug_script_runs_tool_descriptor(),
        agent_debug_close_stale_sessions_tool_descriptor(),
        agent_debug_session_timeline_tool_descriptor(),
        agent_debug_repl_cells_tool_descriptor(),
        agent_debug_source_files_tool_descriptor(),
        agent_debug_graph_inventory_tool_descriptor(),
        agent_trace_read_tool_descriptor(),
    ]
}

fn agent_repl_command_tool_descriptor() -> McpToolDescriptor {
    McpToolDescriptor {
        name: MCP_REPL_COMMAND_TOOL.to_owned(),
        title: Some("Run Arcweft Agent REPL Command".to_owned()),
        description: "Parses raw Agent REPL input, dispatches typed meta-commands through the shared ReplCommandHandler stack, and returns structured ReplCommandResult evidence as JSON text.".to_owned(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "input": { "type": "string", "description": "Raw REPL input such as :tasks --all, :cancel all, :warm latest, or :codegen latest." },
                "command_id": { "type": "integer", "minimum": 1, "default": 1 },
                "trace_policy": { "type": "string", "enum": ["read_write", "read_only_trace"], "default": "read_write" },
                "max_items": { "type": "integer", "minimum": 1, "default": 32 },
                "max_string_bytes": { "type": "integer", "minimum": 1, "default": 240 },
                "include_diagnostics": { "type": "boolean", "default": true }
            },
            "required": ["input"]
        }),
    }
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
                "executor": { "type": "string", "enum": ["bytecode-vm", "aot"], "default": "bytecode-vm" },
                "pure_backend": { "type": "string", "enum": ["auto", "vm", "aot", "jit"] },
                "pure_workers": {
                    "oneOf": [
                        { "type": "string", "enum": ["auto"] },
                        { "type": "integer", "minimum": 1 }
                    ]
                },
                "pure_batch_min_len": { "type": "integer", "minimum": 1 },
                "pure_object_artifacts": { "type": "boolean", "default": false },
                "math_backend": { "type": "string", "enum": ["auto", "scalar", "glam", "ndarray", "wgpu"] },
                "math_wgpu_min_elements": { "type": "integer", "minimum": 1 },
                "native_steps": { "type": "integer", "minimum": 1, "default": 8 },
                "native_mode": { "type": "string", "enum": ["one-op", "drain", "game", "server"], "default": "drain" },
                "native_max_ops": { "type": "integer", "minimum": 1, "default": 64 },
                "max_steps": { "type": "integer", "minimum": 1, "default": 256 },
                "max_ops": { "type": "integer", "minimum": 1, "default": 1024 },
                "values": {
                    "type": "object",
                    "description": "Native runtime root bindings keyed by binding name, using the same bool/string/integer value syntax as arcw --value."
                },
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
                "kind": { "type": "string", "enum": ["advance_text", "select_choice", "invoke", "scroll"], "description": "Semantic action kind when action_id is not supplied." },
                "target": { "type": "string", "description": "Target public id/object id. Required for select_choice and invoke when action_id is not supplied." },
                "action": { "type": "string", "description": "Invoke action id. Required for invoke when action_id is not supplied." },
                "args": { "type": "object", "description": "Optional JSON object payload for invoke actions, lowered to AgentValue records." },
                "region": { "type": "string", "minLength": 1, "description": "Observed scroll-region target. Required for scroll when action_id is not supplied." },
                    "delta_x_milli": { "type": "integer", "minimum": -2_147_483_648, "maximum": 2_147_483_647, "description": "Horizontal input delta in milli logical pixels." },
                    "delta_y_milli": { "type": "integer", "minimum": -2_147_483_648, "maximum": 2_147_483_647, "description": "Vertical input delta in milli logical pixels." }
            },
            "anyOf": [
                { "required": ["action_id"] },
                { "required": ["kind"] }
            ]
        }),
    }
}

fn agent_act_alias_tool_descriptor() -> McpToolDescriptor {
    let mut descriptor = agent_action_tool_descriptor();
    "arcweft.act".clone_into(&mut descriptor.name);
    descriptor.title = Some("Dispatch Arcweft Action Alias".to_owned());
    "Alias for arcweft.action with the same input schema and result contract."
        .clone_into(&mut descriptor.description);
    descriptor
}

fn agent_session_step_frames_tool_descriptor() -> McpToolDescriptor {
    McpToolDescriptor {
        name: "arcweft.session.step_frames".to_owned(),
        title: Some("Step Arcweft Session Frames".to_owned()),
        description: "Steps the active native Agent session by a deterministic frame count and returns the resulting frame summary.".to_owned(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "source": { "type": "string", "description": "Optional .arcw source to observe before stepping. Mutually exclusive with profile." },
                "manifest": { "type": "string", "description": "Launch manifest path for profile-based observe-before-step. Defaults to arcw.toml when profile is supplied." },
                "profile": { "type": "string", "description": "Optional launch profile to resolve before stepping. Mutually exclusive with source." },
                "entry": { "type": "string" },
                "flow": { "type": "string" },
                "count": { "type": "integer", "minimum": 1, "default": 1 },
                "viewport_width": { "type": "integer", "minimum": 1, "default": 1280 },
                "viewport_height": { "type": "integer", "minimum": 1, "default": 720 },
                "textbox_height": { "type": "integer", "minimum": 1 },
                "max_ops": { "type": "integer", "minimum": 1 }
            }
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
                "uri": { "type": "string" },
                "max_privacy": {
                    "type": "string",
                    "enum": ["public", "project", "sensitive", "secret"],
                    "default": "project",
                    "description": "Highest privacy class allowed for returned resource contents. Image/capture resources are sensitive by default."
                },
                "path": {
                    "type": "string",
                    "description": "Optional filesystem path to an Arcweft debug SQLite database. When supplied, allowed and blocked resource reads are audited as resource_read debug events."
                }
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
                "max_privacy": {
                    "type": "string",
                    "enum": ["public", "project", "sensitive", "secret"],
                    "default": "project",
                    "description": "Highest privacy class allowed for returned observation-derived state."
                },
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
                "max_privacy": {
                    "type": "string",
                    "enum": ["public", "project", "sensitive", "secret"],
                    "default": "project",
                    "description": "Highest privacy class allowed for returned observation-derived signal values."
                },
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
                "max_privacy": {
                    "type": "string",
                    "enum": ["public", "project", "sensitive", "secret"],
                    "default": "project",
                    "description": "Highest privacy class allowed for returned observation-derived log messages."
                },
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
                "source": { "type": "string", "description": "Optional .arcw source to observe before querying and include as source/project RAG context. Mutually exclusive with profile." },
                "sources": {
                    "description": "Additional .arcw files or directories to parse, lower, and include as source/project RAG context without observing them.",
                    "oneOf": [
                        { "type": "string" },
                        { "type": "array", "items": { "type": "string" } }
                    ]
                },
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
                },
                "path": {
                    "type": "string",
                    "description": "Optional filesystem path to an Arcweft debug SQLite database. When supplied, the query reads privacy-allowed pre-indexed chunks, upserts MCP-derived chunks, and records the selected RAG audit for later explain/context.read."
                },
                "local_embedding": {
                    "type": "boolean",
                    "default": false,
                    "description": "When true, use the deterministic local hash query embedding to fuse stored-vector hits from path into the RAG context pack. Requires path."
                },
                "local_embedding_model_id": {
                    "type": "string",
                    "default": "arcweft-local-hash",
                    "description": "Local embedding model id used to select stored debug-store embeddings."
                },
                "local_embedding_model_revision": {
                    "type": "string",
                    "default": "1",
                    "description": "Local embedding model revision used to select stored debug-store embeddings."
                },
                "local_embedding_dimensions": {
                    "type": "integer",
                    "minimum": 1,
                    "default": 32,
                    "description": "Local embedding vector dimensions used to select stored debug-store embeddings."
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
        description: "Searches the rebuildable Arcweft debug SQLite store through lexical, vector, graph, history, diagnostic, or test-result channels with privacy filtering before limit.".to_owned(),
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
                "diagnostic_query": {
                    "type": "string",
                    "description": "Text query for indexed diagnostics by id, code, severity, phase, message, source path, related ids, and payload."
                },
                "test_query": {
                    "type": "string",
                    "description": "Text query for indexed test results by id, test id, kind, outcome, summary, diagnostic ids, and artifact refs."
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

fn agent_rag_explain_tool_descriptor() -> McpToolDescriptor {
    McpToolDescriptor {
        name: "arcweft.rag.explain".to_owned(),
        title: Some("Explain Arcweft RAG Context".to_owned()),
        description: "Explains the latest cached RagContextPack, or a cached/debug-store persisted pack selected by query_id, without inlining large item bodies.".to_owned(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "query_id": {
                    "type": "string",
                    "description": "Optional cached or persisted RAG query id. Defaults to the latest arcweft.rag.query result."
                },
                "path": {
                    "type": "string",
                    "description": "Filesystem path to the Arcweft debug SQLite database used when query_id is not cached. Defaults to .arcweft/cache/agent-debug.sqlite3."
                },
                "max_privacy": {
                    "type": "string",
                    "enum": ["public", "project", "sensitive", "secret"],
                    "default": "project",
                    "description": "Highest privacy class allowed when reading persisted query hits from the debug store."
                }
            }
        }),
    }
}

fn agent_rag_context_read_tool_descriptor() -> McpToolDescriptor {
    McpToolDescriptor {
        name: "arcweft.rag.context.read".to_owned(),
        title: Some("Read Arcweft RAG Context Item".to_owned()),
        description: "Reads one selected context item body from a cached or debug-store persisted RagContextPack by chunk_id, with an explicit byte cap.".to_owned(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "query_id": {
                    "type": "string",
                    "description": "Optional cached or persisted RAG query id. Defaults to the latest arcweft.rag.query result."
                },
                "path": {
                    "type": "string",
                    "description": "Filesystem path to the Arcweft debug SQLite database used when query_id is not cached. Defaults to .arcweft/cache/agent-debug.sqlite3."
                },
                "chunk_id": {
                    "type": "string",
                    "description": "RAG context chunk id returned by arcweft.rag.query or arcweft.rag.explain."
                },
                "max_bytes": {
                    "type": "integer",
                    "minimum": 1,
                    "default": 8192,
                    "description": "Maximum UTF-8 bytes of the item body to return."
                },
                "max_privacy": {
                    "type": "string",
                    "enum": ["public", "project", "sensitive", "secret"],
                    "default": "project",
                    "description": "Highest privacy class allowed when reading persisted query hits from the debug store."
                }
            },
            "required": ["chunk_id"]
        }),
    }
}

fn agent_debug_script_runs_tool_descriptor() -> McpToolDescriptor {
    McpToolDescriptor {
        name: "arcweft.debug.script.runs".to_owned(),
        title: Some("Read Arcweft Agent Script Runs".to_owned()),
        description: "Reads persisted Agent Script run lifecycle rows from the rebuildable SQLite debug store with optional session filtering.".to_owned(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Filesystem path to the Arcweft debug SQLite database. Defaults to .arcweft/cache/agent-debug.sqlite3."
                },
                "session_id": {
                    "type": "string",
                    "description": "Optional Agent Debug Bus session id filter."
                },
                "limit": {
                    "type": "integer",
                    "minimum": 1,
                    "default": 20
                },
                "max_privacy": {
                    "type": "string",
                    "enum": ["public", "project", "sensitive", "secret"],
                    "default": "project",
                    "description": "Maximum privacy class allowed in lifecycle metadata readback. Project metadata and summary are omitted when set to public."
                }
            }
        }),
    }
}

fn agent_debug_close_stale_sessions_tool_descriptor() -> McpToolDescriptor {
    McpToolDescriptor {
        name: "arcweft.debug.sessions.close_stale".to_owned(),
        title: Some("Close Stale Arcweft Debug Sessions".to_owned()),
        description: "Applies the debug-store lifecycle policy for long-lived running sessions, optionally as a dry run, and closes stale rows as abandoned.".to_owned(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Filesystem path to the Arcweft debug SQLite database. Defaults to .arcweft/cache/agent-debug.sqlite3."
                },
                "stale_after_millis": {
                    "type": "integer",
                    "minimum": 1,
                    "description": "Running sessions older than this duration are stale."
                },
                "reason": {
                    "type": "string",
                    "default": "stale_running_session",
                    "description": "Lifecycle policy reason recorded in session metadata."
                },
                "dry_run": {
                    "type": "boolean",
                    "default": false,
                    "description": "When true, report matching sessions without closing them."
                }
            },
            "required": ["stale_after_millis"]
        }),
    }
}

fn agent_debug_session_timeline_tool_descriptor() -> McpToolDescriptor {
    McpToolDescriptor {
        name: "arcweft.debug.session.timeline".to_owned(),
        title: Some("Read Arcweft Debug Timeline".to_owned()),
        description: "Reads debug-store event timeline rows from the rebuildable SQLite cache with optional session/run filters and privacy filtering before limit.".to_owned(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Filesystem path to the Arcweft debug SQLite database. Defaults to .arcweft/cache/agent-debug.sqlite3."
                },
                "session_id": {
                    "type": "string",
                    "description": "Optional Agent Debug Bus session id filter."
                },
                "run_id": {
                    "type": "string",
                    "description": "Optional Agent Script run id filter."
                },
                "limit": {
                    "type": "integer",
                    "minimum": 1,
                    "default": 50
                },
                "max_privacy": {
                    "type": "string",
                    "enum": ["public", "project", "sensitive", "secret"],
                    "default": "project",
                    "description": "Highest privacy class allowed in returned event payloads."
                }
            }
        }),
    }
}

fn agent_debug_repl_cells_tool_descriptor() -> McpToolDescriptor {
    McpToolDescriptor {
        name: "arcweft.debug.repl.cells".to_owned(),
        title: Some("Read Arcweft Agent REPL Cells".to_owned()),
        description: "Reads persisted Agent REPL cell rows from the rebuildable SQLite debug store for one session.".to_owned(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Filesystem path to the Arcweft debug SQLite database. Defaults to .arcweft/cache/agent-debug.sqlite3."
                },
                "session_id": {
                    "type": "string",
                    "description": "Agent Debug Bus session id whose REPL cells should be returned."
                },
                "limit": {
                    "type": "integer",
                    "minimum": 1,
                    "default": 50
                }
            },
            "required": ["session_id"]
        }),
    }
}

fn agent_debug_source_files_tool_descriptor() -> McpToolDescriptor {
    McpToolDescriptor {
        name: "arcweft.debug.source.files".to_owned(),
        title: Some("Read Arcweft Debug Source Files".to_owned()),
        description: "Reads program-owned source-file inventory rows from the rebuildable SQLite debug store.".to_owned(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Filesystem path to the Arcweft debug SQLite database. Defaults to .arcweft/cache/agent-debug.sqlite3."
                },
                "program_hash": {
                    "type": "string",
                    "description": "Program hash whose source-file inventory should be returned."
                },
                "max_privacy": {
                    "type": "string",
                    "enum": ["public", "project", "sensitive", "secret"],
                    "default": "project",
                    "description": "Maximum privacy class the caller may receive. Source-file inventory is project-private and is omitted for public."
                }
            },
            "required": ["program_hash"]
        }),
    }
}

fn agent_debug_graph_inventory_tool_descriptor() -> McpToolDescriptor {
    McpToolDescriptor {
        name: "arcweft.debug.graph.inventory".to_owned(),
        title: Some("Read Arcweft Debug Graph Inventory".to_owned()),
        description: "Reads program-owned graph symbol and edge inventory rows from the rebuildable SQLite debug store.".to_owned(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Filesystem path to the Arcweft debug SQLite database. Defaults to .arcweft/cache/agent-debug.sqlite3."
                },
                "program_hash": {
                    "type": "string",
                    "description": "Program hash whose graph symbol and edge inventory should be returned."
                },
                "max_privacy": {
                    "type": "string",
                    "enum": ["public", "project", "sensitive", "secret"],
                    "default": "project",
                    "description": "Maximum privacy class the caller may receive. Graph inventory is project-private and is omitted for public."
                }
            },
            "required": ["program_hash"]
        }),
    }
}
