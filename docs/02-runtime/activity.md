# Activity model

Activity はノベル本編、トラックゲーム、FPS、WASM plugin、Rust plugin、外部 process を統一する。

## Activity trait

```rust
pub trait Activity {
    fn manifest() -> ActivityManifest where Self: Sized;
    fn mount(&mut self, ctx: MountContext<'_>) -> Result<MountResult, ActivityError>;
    fn step(&mut self, input: RuntimeStepInputRef<'_>, output: RuntimeStepOutputSink<'_>) -> StepStatus;
    fn snapshot(&self) -> Result<ActivitySnapshot, ActivityError>;
    fn restore(&mut self, snapshot: ActivitySnapshotRef<'_>) -> Result<(), ActivityError>;
}
```

Activity snapshot は Sans I/O data。Activity は path を開かず、host adapter が snapshot bytes の serialize、compress、encrypt、file write、cloud sync を担当する。

## Mode

```rust
pub enum ActivityMode {
    Deterministic,
    CheckpointedRealtime,
    ExternalRealtime,
}
```

## DSL

```arcw
pub activity @activity.truck_game from rust "mini_games/truck" {
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

```arcw
let result = try await @<activity.truck_game>.run({ seed = state.seed }) with {
    pending .Realizing(p) => scene.show(@scene.loading_plugin); progress.set(p.ratio)
    pending .Running(p) => scene.show(@scene.truck_loading); progress.set(p.ratio)
}
```

## Render path

Portable Activity は `RenderCommandBuffer` を返す。

```arcw
RenderCommand::DrawMesh {
    mesh,
    material: @shader.material.truck_road,
    params,
}
```

Trusted Activity のみ直接 wgpu callback を許可する。

