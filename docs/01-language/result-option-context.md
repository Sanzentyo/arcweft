# Result / Option, `?`, and Context

Arcweft includes Rust-like `Result` and `Option`, two retained Try spellings
(`expr?` and `try expr`), and standard context helpers inspired by
`anyhow::Context`.

## Result and Option

```arcw
pub enum Result<T, E> {
    Ok(T),
    Err(E),
}

pub enum Option<T> {
    Some(T),
    None,
}
```

## Try on Result

```arcw
let config = load_config()?
let cached = try load_cached_config()
```

Conceptually:

```arcw
let config = match load_config() {
    .Ok(v) => v
    .Err(e) => return .Err(e)
}
```

The error path has type `!`, so the expression has type `T`.

`try expr` and `expr?` are both canonical authored forms of the same general
Try operation. Neither is deprecated or an alias for the other. Source-facing
tools preserve the authored form. Prefix Try binds at prefix precedence `90`,
while postfix Try binds at postfix precedence `100`.

When the surrounding result type uses an anonymous error sum, `?` widens only
through exact branch injection:

```arcw
fn load_config(path: VirtualPath) -> Result<Config, FsError | ParseError> {
    let text = read_text(path)?
    parse_config(text)?
}
```

No trait-based implicit conversion is used to choose anonymous sum branches; use
an explicit conversion when the target branch is not the expression's exact type.

More generally, Try does not invoke an implicit `From`, `Into`, `ArcError`, or
function-name conversion. After nominal and generic resolution, the enclosing
`Result<_, ExpectedError>` must accept the operand's
`Result<_, ActualError>` error type under the ordinary directional type rule.
An explicit `map_err`, `context`, or other typed conversion is checked before
Try and may therefore change that error type.

## Try on Option

In an `Option`-returning function:

```arcw
fn selected_route(state: GameState) -> Option<Ref<Flow>> {
    let route = state.route_override?
    Some(route)
}
```

An `Option<T>` Try expression is valid only in an enclosing `Option<_>`
propagation boundary. It does not implicitly convert `None` into an error for
`ArcResult` or another `Result` type.

```arcw
fn selected_route(state: GameState) -> ArcResult<Ref<Flow>> {
    let route = state.route_override
        .context("route override is missing")?
    Ok(route)
}
```

The conversion is explicit because `context(...)` changes the checked operand
from `Option<T>` to `ArcResult<T>` before Try is applied. `ok_or(...)` and
`ok_or_else(...)` provide the corresponding conversion into a typed `Result`.

```arcw
let route = state.route_override.ok_or(MissingRouteError.new())?
```

The reverse direction is equally explicit: a `Result<T, E>` Try expression is
not accepted in an `Option<_>` boundary. Use an ordinary conversion such as
`.ok()` before applying Try when discarding the error is intentional.

## Context helpers

Arcweft provides `context` and `with_context` for both `Result` and `Option`.

```arcw
let config = load_config()
    .context("failed to load project config")?

let route = state.route_override
    .context("missing route override")?

let voice = voice_catalog.find(line.voice_key)
    .with_context(|| fmt("missing voice for {line}", line=line.id))?
```

Standard traits:

```arcw
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

```arcw
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

```arcw
Option<Result<T, E>>.transpose() -> Result<Option<T>, E>
Result<Option<T>, E>.transpose_option() -> Option<Result<T, E>>
```

The second name is intentionally `transpose_option` rather than another overload of `transpose`, to avoid confusion in diagnostics.

## `?` with `await`

`await need with:` returns `Result<T, E>`. Both propagating Await spellings are
retained:

```arcw
let bg = try await asset.image(@asset:.bg.room)
    .context("opening background failed")
with:
    pending p:
        scene.show(@scene.loading)
        progress.set(p.ratio)
```

Equivalent explicit form:

```arcw
let bg_result = await asset.image(@asset:.bg.room)
    .context("opening background failed")
with:
    pending p:
        scene.show(@scene.loading)
        progress.set(p.ratio)

let bg = bg_result?
```

`await? expr with:` has the same propagation semantics as `try await expr
with:`, but its source form remains distinct. Formatters and refactorings retain
the authored spelling.

```arcw
let bg = await? asset.image(@asset:.bg.room)
    .context("opening background failed")
with:
    pending p:
        scene.show(@scene.loading)
        progress.set(p.ratio)
```

Rejected because `with:` belongs to the await operation and the postfix grouping
is ambiguous:

```arcw
await asset.image(@asset:.bg.room)? with:
    pending p:
        scene.show(@scene.loading)
```

## `bail`, `ensure`, and `fail`

Arcweft standard library includes convenience helpers:

```arcw
bail("invalid route")
ensure(condition, "message")
fail(ErrorKind::InvariantBroken)
```

Semantics:

```text
bail(msg):
  construct ArcError with current trace and return Err(...)

ensure(cond, msg):
  if !cond { bail(msg) }

fail(kind):
  construct ArcError of the given kind and diverge with type !
```

Examples:

```arcw
fn validate_score(score: i32) -> ArcResult<Unit> {
    ensure(score >= 0, "score must be non-negative")
    Ok(())
}
```

## Avoiding unsafe unwraps

`unwrap` and `expect` are debug-only by default unless a project explicitly enables them in production.

Preferred:

```arcw
let route = route_override.context("route missing")?
```

Instead of:

```arcw
let route = route_override.unwrap()
```
