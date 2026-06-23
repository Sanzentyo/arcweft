# Await, Need, Result, and `?`

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

`await need with:` waits for readiness and handles pending UI. It returns `Result<T, E>`.

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

## Postfix `?`

Arcweft supports Rust-like postfix `?`.

```arcw
let config = load_config()?
```

For `Result<T, E>`, this unwraps `Ok(T)` or propagates `Err(E)`. For `Option<T>`, this unwraps `Some(T)` or propagates absence according to the surrounding return type.

This ordinary postfix `?` is a core expression operator. It remains valid on any expression that has an appropriate `Result` or `Option` type:

```arcw
let config = load_config()?
let route = state.route_override.context("missing route")?
let bg = (await asset.image(@asset:.bg.room) with:
    pending p:
        scene.show(@scene.loading)
)?
```

`?` relies on the bottom type `!` for its error branch. Conceptually:

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
let bg = (await asset.image(@asset:.bg.room) with:
    pending p:
        scene.show(@scene.loading)
        progress.set(p.ratio)
)?
```

Therefore Arcweft's canonical user-facing form is:

```arcw
let bg = try await asset.image(@asset:.bg.room) with:
    pending p:
        scene.show(@scene.loading)
        progress.set(p.ratio)
```

Arcweft also accepts this equivalent prefix sugar:

```arcw
let bg = await? asset.image(@asset:.bg.room) with:
    pending p:
        scene.show(@scene.loading)
        progress.set(p.ratio)
```

Meaning:

```text
await Need<T, E>       -> Result<T, E>
(await Need<T, E>)?    -> T
try await Need<T, E>   -> T
await? Need<T, E>      -> T
```

`try await` and `await?` are not separate error models. They are sugar for awaiting a `Need` and applying the ordinary `?` operator to the resulting `Result`.

## Rejected ambiguous syntax

Do not write:

```arcw
await asset.image(@asset:.bg.room)? with:
    pending p:
        scene.show(@scene.loading)
```

That shape is visually close to Rust, but in Arcweft it is ambiguous because `with:` belongs to the await operation. Arcweft rejects only this grouped form: `await expr? with:`. The ordinary postfix `?` remains valid elsewhere, including `(await expr with: ...)?`.

Use one of:

```arcw
let bg_result = await asset.image(@asset:.bg.room) with:
    pending p:
        scene.show(@scene.loading)
let bg = try await asset.image(@asset:.bg.room) with:
    pending p:
        scene.show(@scene.loading)
let bg = await? asset.image(@asset:.bg.room) with:
    pending p:
        scene.show(@scene.loading)
```

Parenthesized `(await ...)?` remains valid for generated code or rare expression composition, but the formatter should prefer `try await`.

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

This is preferred over parenthesizing the whole await.

If the context must refer to the whole await operation, block form is allowed:

```arcw
let bg = (await asset.image(@asset:.bg.room) with:
    pending p:
        scene.show(@scene.loading)
        progress.set(p.ratio)
).context("opening background failed")?
```

But user-authored code should normally use `try await`.

## Await in flow

Visible `flow` code must provide pending behavior.

```arcw
let voice = try await voice.load(@voice.alice.001) with:
    pending p:
        scene.show(@scene.loading_voice)
        text.show("音声を読み込み中")
        progress.set(p.ratio)
```

## Await in task fn

Background `task fn` may use simpler await when not directly visible.

```arcw
task fn load_opening_assets() -> ArcResult<OpeningAssets> {
    let bg = try await asset.image(@asset:.bg.room)
    let voice = try await asset.audio(@asset:.voice.alice.001)
    Ok(OpeningAssets { bg, voice })
}
```

If a task may be visible to the player, callers still handle pending at the flow/UI layer.

## Summary

```text
await expr with:
  returns Result<T, E>

try await expr with:
  returns T and propagates E with `?` semantics

await? expr with:
  equivalent to try await

expr?
  ordinary Rust-like postfix try operator

await expr? with:
  rejected
```

## See also

- [Result / Option, `?`, and Context](result-option-context.md)
- [Error, Trace, `?`, and Context](error-trace-context.md)
- [Never / Bottom Type](never-bottom-type.md)
- [Expression Control Flow](expression-control-flow.md)
