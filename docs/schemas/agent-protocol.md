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
