# Example: `?`, context, Option conversion, and error trace

```arcw
mod game::routes::error_context_example

use game::prelude::*

pub flow @flow.error_context_example example(state: GameState) -> Result<FlowExit, FlowError> {
    let route = state.route_override
        .context("missing route override for error_context_example")?

    let bg = try await asset.image(@asset.bg.room)
        .context("while loading opening background")
    with:
        pending p:
            scene.show(@scene.loading)
            progress.set(p.ratio)

    let voice = try await voice.load(@voice.alice.opening.001)
        .map_err(.Voice)
        .context("while loading Alice opening voice")
    with:
        pending p:
            scene.show(@scene.loading_voice)
            progress.set(p.ratio)

    alice.say(voice=voice)[
        読み込みが完了しました。[p]
    ]

    Ok(FlowExit::Goto(route))
}
```

If `state.route_override` is `None`, `.context(...)` converts it to `Result<T, ArcError>` and `?` returns early with a trace frame containing:

```text
flow.error_context_example
game/routes/error_context_example.arcw:6
state path: GameState.route_override
context: missing route override for error_context_example
```

If `asset.image(...)` fails, the error trace contains the `await` source location, the asset ID, and the context string.

