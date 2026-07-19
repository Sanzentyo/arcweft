use super::{
    AGENT_MCP_STDIO_LOCK, agent_mcp_responses, project_shape_metadata, rich_text_showcase_path,
    run_agent_mcp_stdio, run_agent_mcp_stdio_local_dev, seed_debug_search_db, stable_hash,
    temp_arcw, temp_dir, workspace_path, workspace_root,
};
use std::fs;
use std::io::{BufRead as _, BufReader, Write as _};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use arcweft_agent_protocol::ids::{AgentRunId, PublicId, SessionId};
use arcweft_core::plan::EntryRuntimeId;
use arcweft_debug_model::script::{DebugScriptRun, DebugScriptRunOutcome};
use arcweft_debug_sqlite::store::DebugStore;
use base64::{Engine as _, engine::general_purpose};

include!("agent_observe_native/shared.rs");
include!("agent_observe_native/core.rs");
include!("agent_observe_native/native_vertical.rs");
include!("agent_observe_native/published_jlreq_class_mix.rs");
include!("agent_observe_native/published_jlreq_units.rs");
include!("agent_observe_native/native_samples_effects.rs");
include!("agent_observe_native/mcp_native_capture.rs");
include!("agent_observe_native/selected_capture_metadata.rs");
include!("agent_observe_native/visual_smoke.rs");
