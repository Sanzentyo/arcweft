# Result / Option, `?`, and Context

Arcweft includes Rust-like `Result`, `Option`, and postfix `?`, plus standard context helpers inspired by `anyhow::Context`.

## Result and Option

```awft
pub enum Result<T, E> {
    Ok(T),
    Err(E),
}

pub enum Option<T> {
    Some(T),
    None,
}
```

## Postfix `?` on Result

```awft
let config = load_config()?
```

Conceptually:

```awft
let config = match load_config() {
    .Ok(v) => v
    .Err(e) => return .Err(From::from(e))
}
```

The error path has type `!`, so the expression has type `T`.

Arcweft also reserves prefix `try expr` as equivalent propagation syntax. It is mainly useful when another prefix construct must group before propagation, most notably `try await expr with { ... }`.

## Postfix `?` on Option

In an `Option`-returning function:

```awft
fn selected_route(state: GameState) -> Option<Ref<Flow>> {
    let route = state.route_override?
    Some(route)
}
```

In an `ArcResult<T>` context, `Option<T>?` is allowed as convenience and converts `None` to `ArcError::missing_value()` with a default source trace.

```awft
fn selected_route(state: GameState) -> ArcResult<Ref<Flow>> {
    let route = state.route_override?
    Ok(route)
}
```

For public/user-facing code, prefer explicit context:

```awft
let route = state.route_override
    .context("route override is missing")?
```

In a typed `Result<T, E>` context that is not `ArcResult`, `Option<T>?` is accepted only if the error type implements the standard conversion from `MissingValueError`. Otherwise use `.ok_or(...)`, `.ok_or_else(...)`, or `.context(...)`.

## Context helpers

Arcweft provides `context` and `with_context` for both `Result` and `Option`.

```awft
let config = load_config()
    .context("failed to load project config")?

let route = state.route_override
    .context("missing route override")?

let voice = voice_catalog.find(line.voice_key)
    .with_context(|| fmt("missing voice for {line}", line=line.id))?
```

Standard traits:

```awft
trait ResultContext<T> {
    fn context(self, message: impl Into<Content>) -> ArcResult<T>
    fn with_context(self, f: fn() -> Content) -> ArcResult<T>
    fn context_entity(self, entity: Ref<Entity>) -> ArcResult<T>
    fn context_source(self, source: SourceAnchor) -> ArcResult<T>
    fn field(self, key: String, value: impl Display) -> Self
}

trait OptionContext<T> {
    fn context(self, message: impl Into<Content>) -> ArcResult<T>
    fn with_context(self, f: fn() -> Content) -> ArcResult<T>
}
```

`context` does not erase the original cause. It appends a context frame to the error trace.

## Result / Option conversion methods

Built-ins:

```awft
Option<T>.ok_or(err) -> Result<T, E>
Option<T>.ok_or_else(f) -> Result<T, E>
Option<T>.context(msg) -> ArcResult<T>
Option<T>.with_context(f) -> ArcResult<T>
Option<T>.unwrap_or(default) -> T
Option<T>.unwrap_or_else(f) -> T
Option<T>.map(f) -> Option<U>
Option<T>.and_then(f) -> Option<U>

Result<T, E>.ok() -> Option<T>
Result<T, E>.err() -> Option<E>
Result<T, E>.map(f) -> Result<U, E>
Result<T, E>.map_err(f) -> Result<T, F>
Result<T, E>.and_then(f) -> Result<U, E>
Result<T, E>.or_else(f) -> Result<T, F>
Result<T, E>.context(msg) -> ArcResult<T>
Result<T, E>.with_context(f) -> ArcResult<T>
```

Transpose:

```awft
Option<Result<T, E>>.transpose() -> Result<Option<T>, E>
Result<Option<T>, E>.transpose_option() -> Option<Result<T, E>>
```

The second name is intentionally `transpose_option` rather than another overload of `transpose`, to avoid confusion in diagnostics.

## `?` with `await`

`await need with:` returns `Result<T, E>`. The ergonomic form for propagation is `try await`.

```awft
let bg = try await asset.image(#asset.bg.room)
    .context("opening background failed")
with:
    pending p:
        scene #scene.loading:
            progress p.ratio
```

Equivalent explicit form:

```awft
let bg_result = await asset.image(#asset.bg.room)
    .context("opening background failed")
with:
    pending p:
        scene #scene.loading:
            progress p.ratio

let bg = bg_result?
```

Rejected:

```awft
await? asset.image(#asset.bg.room) with:
    pending p:
        scene #scene.loading
```

## `bail`, `ensure`, and `fail`

Arcweft standard library includes convenience helpers:

```awft
bail "invalid route"
ensure condition, "message"
fail ErrorKind::InvariantBroken
```

Semantics:

```text
bail msg:
  construct ArcError with current trace and return Err(...)

ensure cond, msg:
  if !cond { bail msg }

fail kind:
  construct ArcError of the given kind and diverge with type !
```

Examples:

```awft
fn validate_score(score: i32) -> ArcResult<Unit> {
    ensure score >= 0, "score must be non-negative"
    Ok(())
}
```

## Avoiding unsafe unwraps

`unwrap` and `expect` are debug-only by default unless a project explicitly enables them in production.

Preferred:

```awft
let route = route_override.context("route missing")?
```

Instead of:

```awft
let route = route_override.unwrap()
```
