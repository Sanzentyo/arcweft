# Await, Need, Result, and `try`

Arcweft uses `Need<T, E>` for values that may take time.

```text
Need<T, E>:
  NotStarted / Pending(Progress) / Ready(T) / Err(E) / Cancelled
```

`Need` represents time and pending work. `Result` represents success or failure. `Option` represents presence or absence.

```text
Need<T, E>       time/pending dimension
Result<T, E>     success/failure dimension
Option<T>        presence/absence dimension
```

## Await result type

`await need with:` waits for readiness and handles pending View. It returns `Result<T, E>`.

```arcw
let bg_result = await asset.image(@asset:.bg.room) with:
    pending p:
        scene.show(@scene.loading)
        progress.set(p.ratio)
```

Type:

```arcw
Result<ImageHandle, AssetError>
```

`with { ... }` is the canonical block form. The indentation form `with:` is syntax sugar for the same block and may be used where scenario-style readability matters.

Handle it explicitly:

```arcw
let bg = match bg_result {
    .Ok(bg) => bg
    .Err(e) => return Err(.Asset(e))
}
```

## Try propagation

Arcweft uses the prefix `try` operator for propagation.

```arcw
let config = try load_config()
```

For `Result<T, E>`, this unwraps `Ok(T)` or propagates `Err(E)`. For `Option<T>`, this unwraps `Some(T)` or propagates absence according to the surrounding return type.

`try` is valid on any expression that has an appropriate `Result` or `Option` type:

```arcw
let config = try load_config()
let route = try state.route_override.context("missing route")
let bg = try (await asset.image(@asset:.bg.room) with:
    pending p:
        scene.show(@scene.loading)
)
```

`try` relies on the bottom type `!` for its error branch. Conceptually:

```arcw
let image = match load_image() {
    .Ok(v) => v
    .Err(e) => return Err(e)
}
```

The error arm has type `!`, so the whole expression has type `Image`.

## The ergonomic await form: `try await`

Writing this is technically valid but unpleasant:

```arcw
let bg = try (await asset.image(@asset:.bg.room) with:
    pending p:
        scene.show(@scene.loading)
        progress.set(p.ratio)
)
```

Therefore Arcweft's canonical user-facing form is:

```arcw
let bg = try await asset.image(@asset:.bg.room) with:
    pending p:
        scene.show(@scene.loading)
        progress.set(p.ratio)
```

Meaning:

```text
await Need<T, E>       -> Result<T, E>
try (await Need<T, E>) -> T
try await Need<T, E>   -> T
```

`try await` is not a separate error model. It is the ordinary prefix `try`
operation applied to the `Result` produced by `await`.

Use one of:

```arcw
let bg_result = await asset.image(@asset:.bg.room) with:
    pending p:
        scene.show(@scene.loading)
let bg = try await asset.image(@asset:.bg.room) with:
    pending p:
        scene.show(@scene.loading)
let bg = try await asset.image(@asset:.bg.room) with:
    pending p:
        scene.show(@scene.loading)
```

Parenthesized `try (await ...)` is equivalent to the compact `try await ...`
form. Both lower to a `Try` expression around a result-preserving `Await`.

## Context with await

Context can be attached to the `Need` before awaiting. The context is applied to the eventual error.

```arcw
let bg = try await asset.image(@asset:.bg.room)
    .context("opening background failed")
with:
    pending p:
        scene.show(@scene.loading)
        text.show("背景を読み込み中")
        progress.set(p.ratio)
```

This form avoids parenthesizing the whole Await when propagation is intended.

If the context must refer to the whole await operation, block form is allowed:

```arcw
let bg = (await asset.image(@asset:.bg.room) with:
    pending p:
        scene.show(@scene.loading)
        progress.set(p.ratio)
).context("opening background failed")
```

The parenthesized form remains appropriate when the surrounding general `try`
is the intended semantic grouping.

## Await in flow

Visible `flow` code must provide pending behavior.

```arcw
let voice = try await voice.load(@voice.alice.001) with:
    pending p:
        scene.show(@scene.loading_voice)
        text.show("音声を読み込み中")
        progress.set(p.ratio)
```

## Direct-style await in functions

Any ordinary `fn` may use direct-style `await` when its suspension is not
directly visible.

```arcw
fn load_opening_assets() -> ArcResult<OpeningAssets> {
    let bg = try await asset.image(@asset:.bg.room)
    let voice = try await asset.audio(@asset:.voice.alice.001)
    Ok(OpeningAssets { bg, voice })
}
```

If a task may be visible to the player, callers still handle pending at the flow/View layer.

## Summary

```text
await expr with:
  returns Result<T, E>

try await expr with:
  returns T and propagates E

try expr
  general Try operation
```

## See also

- [Result / Option, `try`, and Context](result-option-context.md)
- [Error, Trace, `try`, and Context](error-trace-context.md)
- [Never / Bottom Type](never-bottom-type.md)
- [Expression Control Flow](expression-control-flow.md)
