# Error, Trace, `?`, and Context

Arcweft uses `Result<T, E>` and `Option<T>` for recoverable failure and absence. For application-level failures, the default error type is `ArcError`.

```arcw
type ArcResult<T> = Result<T, ArcError>
```

`ArcError` stores a structured Arcweft source trace by default. This is not just a native stack trace; it contains script file positions, flow IDs, dialogue line IDs, text keys, entity IDs, hook/task IDs, and user-provided context.

## Error structure

```arcw
pub struct ArcError {
    kind: ErrorKind
    message: Content
    source: Option<ArcError>
    trace: ErrorTrace
    data: OrderedMap<String, ErrorValue>
}

pub struct ErrorTrace {
    origin: TraceFrame
    frames: Vec<TraceFrame>
    state_hash: Option<StateHash>
    tick: Option<TickId>
    replay_cursor: Option<TraceCursor>
    native_backtrace: Option<NativeBacktrace>
}

pub struct TraceFrame {
    source: Option<SourceAnchor>
    flow: Option<Ref<Flow>>
    line: Option<Ref<DialogueLine>>
    text_key: Option<TextKey>
    voice_key: Option<VoiceKey>
    entity: Option<EntityId>
    function: Option<FunctionId>
    hook: Option<Ref<Hook>>
    task: Option<TaskId>
    await_target: Option<EntityId>
    message: Option<Content>
}
```

## Default trace capture

When an error is created in `.arcw` code, Arcweft captures:

```text
- file path
- line and column
- byte/source span
- flow ID
- function ID
- current dialogue line ID, if any
- current text key, if any
- current voice key, if any
- current hook ID, if any
- current task / Need ID, if any
- entity reference involved, if any
- current tick and state hash when runtime
```

When the error is propagated with `?`, Arcweft appends a lightweight propagation frame at the call site.

```arcw
let bg = load_bg()?   // Err path gets a propagation frame here
```

## Native backtrace

Native stack backtrace is optional and profile-controlled. Arcweft source trace is the default.

```toml
[error.trace]
source_trace = "always"
native_backtrace = "debug"  # off | debug | always
max_frames = 64
redact_source_text_in_product = true
```

## Display format

Default dev/test display:

```text
error[ASSET_MISSING]: failed to load background image

context:
  while loading opening background
  while entering flow flow.opening

trace:
  0: game/routes/opening.arcw:12:14
     flow.opening
     await asset.image(@asset:.bg.room)

  1: game/routes/opening.arcw:9:5
     say.opening.narration.001 / text.opening.narration.001
     地の文: 扉の向こうから、雨の音がした。[p]

ids:
  flow: flow.opening
  asset: asset.bg.room
  state_hash: b3:8a12...
  tick: 182
```

## `?` operator and trace

For `Result<T, E>`:

```arcw
let image = load_image()?
```

means:

```arcw
let image = match load_image() {
    .Ok(v) => v
    .Err(e) => return Err(IntoError::into_error(e).at_current_site())
}
```

The error branch has type `!`, so the expression has type `Image`.

For `Option<T>` in an `ArcResult` context, `None` becomes `ArcError::missing_value()` with a trace frame. Use `.context(...)` for better messages.

```arcw
let route = state.route_override
    .context("route override is missing")?
```

## Context helpers

```arcw
let save = save_slot.load()
    .context("failed to load save slot")?
```

Lazy context:

```arcw
let bg = load_bg(id)
    .with_context(|| "failed to load background " + fmt(id))?
```

Typed context:

```arcw
let voice = voice.load(@voice.alice.001)
    .context("voice load failed")
    .field("speaker", @character.alice)
    .field("line", @say.opening.001)?
```

On `Option<T>`:

```arcw
let route = state.route_override
    .context("route override missing")?
```

This converts `None` to `Err(ArcError)` and attaches the context.

## Context with await

Context can be attached to the `Need` before awaiting.

```arcw
let bg = try await asset.image(@asset:.bg.room)
    .context("while loading opening background")
with:
    pending p:
        scene.show(@scene.loading)
        progress.set(p.ratio)
```

This is preferred.

The explicit parenthesized form is valid but not recommended for hand-written code:

```arcw
let bg = (await asset.image(@asset:.bg.room) with:
    pending p:
        scene.show(@scene.loading)
        progress.set(p.ratio)
).context("while loading opening background")?
```

`await? expr with:` is accepted as prefix sugar for `try await expr with:`, but
diagnostics and formatter output should prefer `try await` unless the user
explicitly asks for prefix-`?` style.

Rejected because postfix `?` groups with the expression before `with:`:

```arcw
await asset.image(@asset:.bg.room)? with:
    pending p:
        scene.show(@scene.loading)
```

## Automatic source frames

The compiler inserts trace frames at major boundaries:

```text
flow entry
function call
parser call
await boundary
hook dispatch
dialogue line start
activity call
custom tag call
```

Authors should not need to add line/file data manually. Context helpers are for human-readable explanation, not source tracking.

## Product mode

Product builds should preserve trace data internally for crash bundles, but UI display may be reduced.

```toml
[error.display.dev]
show_stack = true
show_ids = true
show_source = true

[error.display.product]
show_stack = false
show_ids = false
show_source = false
store_crash_bundle = true
```

Crash bundle:

```text
crash/
  error.json
  trace.arcwx
  state_hash.txt
  screenshot.png
  logs.jsonl
  signals.json
```

## JSON schema example

```json
{
  "kind": "AssetMissing",
  "message": "failed to load background image",
  "trace": {
    "tick": 182,
    "state_hash": "b3:8a12...",
    "frames": [
      {
        "kind": "Await",
        "flow": "flow.opening",
        "source": {
          "file": "game/routes/opening.arcw",
          "line": 12,
          "column": 14
        },
        "entity": "asset.bg.room"
      },
      {
        "kind": "DialogueLine",
        "line": "say.opening.narration.001",
        "text_key": "text.opening.narration.001",
        "speaker": "character.narrator"
      }
    ]
  }
}
```
