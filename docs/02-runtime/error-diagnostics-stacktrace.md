# Error Diagnostics, Context, and Script Stack Traces

Arcweft errors preserve source, ID, and runtime trace information by default.

The default user-facing error type is `ArcError`.

```arcw
pub struct ArcError {
    kind: ErrorKind
    message: Content
    trace: ErrorTrace
    source: Option<ErrorSource>
    related: Vec<RelatedDiagnostic>
}
```

## Default behavior

Any error created by Arcweft runtime, parser, compiler, hooks, tasks, dialogue, audio, rendering, capture, or user code should carry an `ErrorTrace` unless explicitly marked as low-level/internal.

The trace is displayed by default in dev/test and summarized in product builds.

## ErrorTrace

```arcw
pub struct ErrorTrace {
    frames: Vec<ErrorFrame>
    state_hash: Option<StateHash>
    tick: Option<TickId>
    replay_cursor: Option<TraceCursor>
}
```

Frame kinds:

```arcw
pub enum ErrorFrame {
    Flow {
        flow: Ref<Flow>,
        source: SourceAnchor,
    },
    DialogueLine {
        line: Ref<SayLine>,
        text_key: TextKey,
        speaker: Ref<Character>,
        source: SourceAnchor,
    },
    Function {
        function: Ref<Function>,
        source: SourceAnchor,
    },
    HandlerDispatch {
        owner: EntityId,
        event: EntityId,
        source: SourceAnchor,
    },
    Await {
        task: TaskId,
        awaited: AwaitTarget,
        source: SourceAnchor,
    },
    Activity {
        activity: Ref<Activity>,
        source: Option<SourceAnchor>,
    },
}
```

Each frame should carry as much identity information as possible:

```text
- EntityId
- PublicId
- TextKey / VoiceKey when applicable
- source file, line, column, byte span
- flow id
- dialogue line id
- owner-local dispatch owner/event IDs
- task id
- state hash / tick when runtime
```

## Display format

Example:

```text
error[ASSET_MISSING]: failed to load background image

context:
  while loading opening background
  while entering flow flow.opening

trace:
  0: game/routes/opening.arcw:12:14
     flow.opening
     try await asset.image(@asset:.bg.room)

  1: game/routes/opening.arcw:9:5
     say.opening.narration.001 / text.opening.narration.001
     地の文: 扉の向こうから、雨の音がした。[p]

ids:
  flow: flow.opening
  asset: asset.bg.room
  state_hash: b3:8a12...
  tick: 182
```

## Context propagation

`.context(...)` and `.with_context(...)` append context frames without discarding the original cause.

```arcw
let bg = try (await asset.image(@asset:.bg.room) with:
    pending p:
        scene.show(@scene.loading)
        progress.set(p.ratio)
).context("while loading opening background")
```

`context` applies to the Result payload, not to Need itself. Attach it after
Await (or map the producer's Result payload before wrapping it in Need):

The explicit two-step form is equivalent:

```arcw
let bg_result = await asset.image(@asset:.bg.room) with:
    pending p:
        scene.show(@scene.loading)
        progress.set(p.ratio)
let bg = try bg_result.context("while loading opening background")
```

Await owns its temporal `with` observers and produces the exact Need payload.
For `Need<Result<T,E>>`, prefix `try` is the sole propagation operation.
Postfix `?` and attached `await?` are not part of the language.

## Automatic source frames

The compiler inserts source frames at major boundaries:

```text
flow entry
function call
codec or cursor decode call
await boundary
owner-local handler dispatch
dialogue line start
activity call
registered content/action call
```

Authors should not need to add line/file data manually. Context helpers are for human-readable explanation, not source tracking.

## Product mode

Product builds should still preserve trace data internally for crash bundles, but View display may be reduced.

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
