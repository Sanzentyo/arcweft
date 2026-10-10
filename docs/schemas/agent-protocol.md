# Agent Protocol Schema Sketch

```rust
pub struct ObserveRequest {
    pub image: ImageRequest,
    pub objects: ObjectRequest,
    pub view_tree: bool,
    pub scene_graph: bool,
    pub state: StateRequest,
    pub audio: bool,
    pub logs: LogRequest,
    pub signals: SignalRequest,
    pub masks: MaskRequest,
}

pub enum AgentAction {
    AdvanceText,
    SelectChoice { choice: PublicId },
    Invoke(AgentInvokeAction),
    Scroll(AgentScrollAction),
    PointerClick { x: u32, y: u32, button: PointerButton },
}

pub struct AgentScrollAction {
    pub region: String,
    pub delta_x_milli: i32,
    pub delta_y_milli: i32,
}

pub struct ActionResult {
    pub accepted: bool,
    pub before_tick: TickId,
    pub after_tick: TickId,
    pub before_state_hash: StateHash,
    pub after_state_hash: StateHash,
    pub produced_events: Vec<GameEventSummary>,
    pub diagnostics: Vec<Diagnostic>,
    pub observation: Option<Observation>,
}
```

Scroll uses the authored region id and signed milli-logical-pixel input deltas:

```json
{
  "kind": "scroll",
  "region": "scroll.Inventory.0",
  "delta_x_milli": 0,
  "delta_y_milli": -90000
}
```

Observation keeps one authored `scroll_region` target. Its internal viewport
and retained content are metadata parts, not additional semantic/action nodes:

```json
{
  "target": "scroll.Inventory.0",
  "role": "scroll_region",
  "parts": {
    "viewport": {
      "internal": true,
      "bounds": [48, 48, 420, 180]
    },
    "content": {
      "internal": true,
      "size": [420, 960],
      "offset": [0, 240],
      "max_offset": [0, 780]
    }
  },
  "axis": "vertical",
  "overflow": "auto",
  "indicators": "auto",
  "overscroll": "clamp",
  "auto_scroll_focus": "nearest"
}
```

Mounted retained lists are reported separately from the rendered object tree.
The range is half-open and every finite-source item remains addressable even
when it is not materialized:

```json
{
  "target": "view.mount.7",
  "scroll_target": "scroll.Inventory.0",
  "axis": "vertical",
  "viewport_extent_milli": 60000,
  "offset_milli": 120000,
  "total_extent_milli": 240000,
  "materialized_start": 2,
  "materialized_end": 3,
  "items": [
    {
      "target": "view.mount.7.item.42",
      "index": 0,
      "key": 42,
      "start_milli": 0,
      "extent_milli": 60000,
      "materialized": false
    },
    {
      "target": "view.mount.7.item.43",
      "index": 1,
      "key": 43,
      "start_milli": 60000,
      "extent_milli": 60000,
      "materialized": false
    },
    {
      "target": "view.mount.7.item.44",
      "index": 2,
      "key": 44,
      "start_milli": 120000,
      "extent_milli": 60000,
      "materialized": true
    },
    {
      "target": "view.mount.7.item.45",
      "index": 3,
      "key": 45,
      "start_milli": 180000,
      "extent_milli": 60000,
      "materialized": false
    }
  ]
}
```

`virtual_lists[]` is produced from exact per-mount session state only when an
observation or capture report is requested. `scroll_target` names the authored
region accepted by the `scroll` action; an off-window item target does not by
itself imply render geometry or an image capture.

## Evaluated observation values

The version 1 observation contract retains evaluated signal and metric data
through the Core effect and observation owners. Those owners accept only
unrestricted value graphs, so an observation copy cannot duplicate a live
affine resource. Session saves use the existing inert runtime-value snapshot
schema, and restoration validates values through the selected program and
the required asset catalogue before committing the new state.

Agent reports, their resources, MCP reads, and script envelopes use the same
tagged Agent value grammar:

```json
{"name":"signal.current_flow","value":{"kind":"entity","value":"flow.opening"}}
```

A checked local or imported EntityRef is projected through its typed owner.
The Core value keeps its declaration identity and imported source-generation
digests; the Agent public ID is observation metadata and grants no execution
capability. A literal string such as "@flow.opening", "true", or "42" remains
a tagged string. Agent predicates compare these typed values directly.

Signed values that fit i64 and unsigned values that fit u64 use those wire
kinds regardless of their Core integer width. Finite f32 values widen to f64.
Tuples and all sequence storage forms project to lists, and structural records
project to maps. Unsupported values, non-finite numbers, out-of-range integers,
and affine graphs fail projection without publishing a replacement report.
Signal and metric rows share the existing envelope map, and duplicate canonical
observation identities are rejected. CLI display summaries retain their explicit
text rendering contract.
