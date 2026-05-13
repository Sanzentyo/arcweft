# Agent Protocol Schema Sketch

```rust
pub struct ObserveRequest {
    pub image: ImageRequest,
    pub objects: ObjectRequest,
    pub ui_tree: bool,
    pub scene_graph: bool,
    pub state: StateRequest,
    pub audio: bool,
    pub logs: LogRequest,
    pub signals: SignalRequest,
    pub masks: MaskRequest,
}

pub enum AgentAction {
    Physical(PhysicalAction),
    Semantic(SemanticAction),
    Wait(WaitPredicate),
    StepFrames(u32),
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

