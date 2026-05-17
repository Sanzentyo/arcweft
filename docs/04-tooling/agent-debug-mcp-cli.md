# Agent Debug Bus / MCP / CLI

## Agent Debug Bus

Verifier, CLI, and LSP diagnostics use the same stable JSON shape that future
Agent tools will embed into `Observation.diagnostics`. This means an Agent can
see an obligation id, source span, related proof/audit ids, and available
actions before the renderer/MCP runtime exists.

Runtime observation now has a Phase 2.0 headless slice through `arcw run --json`.
The shared diagnostic/action schema produced by `arcweft-verify` and consumed by
CLI/LSP remains the connection point for future Agent tools.

```awft
pub trait AgentDebugBus {
    fn observe(&mut self, req: ObserveRequest) -> Result<Observation, AgentError>;
    fn act(&mut self, action: AgentAction) -> Result<ActionResult, AgentError>;
    fn resource(&mut self, id: AgentResourceId) -> Result<AgentResource, AgentError>;
    fn subscribe(&mut self, filter: EventFilter) -> AgentEventStream;
}
```

## Observation

```awft
pub struct Observation {
    pub session_id: SessionId,
    pub tick: TickId,
    pub frame_id: FrameId,
    pub state_hash: StateHash,
    pub render_hash: RenderHash,
    pub viewport: ViewportInfo,
    pub images: Vec<ImageResource>,
    pub objects: Vec<ObservedObject>,
    pub actions: Vec<ActionTarget>,
    pub ui_tree: Option<UiTree>,
    pub scene_graph: Option<SceneGraphSlice>,
    pub audio_state: Option<AudioObservation>,
    pub logs: Vec<DecodedLog>,
    pub signals: Vec<SignalSnapshot>,
    pub diagnostics: Vec<Diagnostic>,
}
```

## Image / bbox / polygon / mask

- color screenshot
- overlay screenshot
- object-id image
- bbox
- polygon
- segmentation mask: RLE / PNG alpha / raw bitmap

## Action

Physical:

```awft
PointerClick { x, y, space, button }
PointerDrag { from, to, duration_ms }
KeyDown / KeyUp
TypeText
```

Semantic:

```awft
Invoke { target, action, args }
SelectChoice { choice }
AdvanceText
OpenMenu { menu }
SetSlider { target, value }
AudioSetBus { bus, gain }
```

Semantic action を優先し、座標 click は fallback。

## CLI

```bash
arcw agent start --headless --bundle dist/game.awfb
arcw agent observe --json
arcw agent observe --image overlay --out overlay.png
arcw agent choose choice.opening.listen
arcw agent click --x 520 --y 540
arcw agent wait --until 'flow == flow.alice_intro'
arcw agent state get 'GameState.affection[@character.alice]'
arcw agent audio state --json
arcw agent tts preview voice.alice.tts "おはよう" --out voice.wav
```

## MCP

Resources:

```text
arcweft://session/{sid}/observation/latest.json
arcweft://session/{sid}/frame/{tick}/color.png
arcweft://session/{sid}/frame/{tick}/overlay.png
arcweft://session/{sid}/frame/{tick}/objects.json
arcweft://session/{sid}/state/current.json
arcweft://session/{sid}/logs.ndjson
arcweft://session/{sid}/signals.json
arcweft://session/{sid}/audio.json
```

Tools:

```text
arcweft.observe
arcweft.click
arcweft.invoke
arcweft.choose
arcweft.advance_text
arcweft.wait_until
arcweft.step_frames
arcweft.get_state
arcweft.log_query
arcweft.signal_get
arcweft.audio_state
arcweft.tts_preview
arcweft.shader_preview
```

## Product flags

```text
--agent=off
--agent=observe
--agent=control
--agent=debug
```

Capabilities:

```awft
pub struct AgentPermissions {
    pub observe_image: bool,
    pub observe_state: bool,
    pub observe_audio: bool,
    pub control_input: bool,
    pub semantic_actions: bool,
    pub mutate_state: bool,
    pub hot_reload: bool,
}
```

Product mode は token、audit log、debug indicator、redaction 必須。
