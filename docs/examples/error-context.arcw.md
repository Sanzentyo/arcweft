# Example: prefix `try`, context, Option conversion, and error trace

```arcw
mod game.routes.error_context_example

use game.prelude.*

pub flow @flow.error_context_example example(state: GameState) -> Result<FlowExit, FlowError> {
    let route = try state.route_override
        .context("missing route override for error_context_example")

    let bg = try (await asset.image(@asset:.bg.room) with:
        pending p:
            scene.show(@scene.loading)
            progress.set(p.ratio)
    ).context("while loading opening background")

    let _voice_audio = try (await voice.load(@voice.alice.opening.001) with:
        pending p:
            scene.show(@scene.loading_voice)
            progress.set(p.ratio)
    )
        .map_err(.Voice)
        .context("while loading Alice opening voice")

    Ok(FlowExit.Goto(route))
}
```

`voice.load` has type `Need<Result<AudioHandle, VoiceError>>`; its successful
`AudioHandle` identifies the loaded audio resource. The line-scoped playback
lease returned by `line.voice_handle()` is the distinct `VoiceHandle` type.

If `state.route_override` is `None`, `.context(...)` converts it to
`Result<T, ArcError>` and prefix `try` propagates with a trace frame containing:

```text
flow.error_context_example
game/routes/error_context_example.arcw:6
state path: GameState.route_override
context: missing route override for error_context_example
```

If `asset.image(...)` fails, the error trace contains the `await` source location, the asset ID, and the context string.
