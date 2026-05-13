# Activity model

Activity はノベル本編、トラックゲーム、FPS、WASM plugin、Rust plugin、外部 process を統一する。

## Activity trait

```rust
pub trait Activity {
    fn manifest() -> ActivityManifest where Self: Sized;
    fn mount(&mut self, ctx: MountContext<'_>) -> Result<MountResult, ActivityError>;
    fn step(&mut self, input: FrameInputView<'_>, output: FrameOutputWriter<'_>) -> StepStatus;
    fn save(&self, out: SaveWriter<'_>) -> Result<()>;
    fn load(&mut self, input: SaveReader<'_>) -> Result<()>;
}
```

## Mode

```rust
pub enum ActivityMode {
    Deterministic,
    CheckpointedRealtime,
    ExternalRealtime,
}
```

## DSL

```awft
pub activity #activity.truck_game from rust "mini_games/truck" {
    mode = deterministic
    tick = fixed(60hz)

    input {
        player_input: stream<InputEvent>
        route_seed: u64
    }

    output {
        result: event<TruckResult>
        telemetry: shared<TruckTelemetry> transient
    }

    contract {
        ensures result.score >= 0
        ensures result.route_delta in [-3, 3]
    }
}
```

## Flow usage

```awft
let result = await #<activity.truck_game>.run({ seed = state.seed })? with {
    pending .Realizing(p) => scene #scene.loading_plugin { progress p.ratio }
    pending .Running(p) => scene #scene.truck_loading { progress p.ratio }
}
```

## Render path

Portable Activity は `RenderCommandBuffer` を返す。

```awft
RenderCommand::DrawMesh {
    mesh,
    material: #shader.material.truck_road,
    params,
}
```

Trusted Activity のみ直接 wgpu callback を許可する。

