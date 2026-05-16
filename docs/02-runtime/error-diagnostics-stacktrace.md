# Error Diagnostics, Context, and Script Stack Traces

Arcweft errors preserve source, ID, and runtime trace information by default.

The default user-facing error type is `ArcError`.

```awft
pub struct ArcError {
    kind: ErrorKind
    message: Content
    trace: ErrorTrace
    source: Option<ErrorSource>
    related: List<RelatedDiagnostic>
}
```

## Default behavior

Any error created by Arcweft runtime, parser, compiler, hooks, tasks, dialogue, audio, rendering, capture, or user code should carry an `ErrorTrace` unless explicitly marked as low-level/internal.

The trace is displayed by default in dev/test and summarized in product builds.

## ErrorTrace

```awft
pub struct ErrorTrace {
    frames: List<ErrorFrame>
    state_hash: Option<StateHash>
    tick: Option<TickId>
    replay_cursor: Option<TraceCursor>
}
```

Frame kinds:

```awft
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
    Hook {
        hook: Ref<Hook>,
        phase: HookPhase,
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
    Parser {
        parser: Ref<Parser>,
        input_span: TextRange,
        source: SourceAnchor,
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
- hook id
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
  0: game/routes/opening.awft:12:14
     flow.opening
     await asset.image(@asset.bg.room)?

  1: game/routes/opening.awft:9:5
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

```awft
let bg = try await asset.image(@asset.bg.room)
    .context("while loading opening background")
with:
    pending p:
        scene.show(@scene.loading)
        progress.set(p.ratio)
```

`context` can be attached before `try await`; the context is applied to the eventual error stored in `Need<T, E>`.

Parenthesized whole-await context is valid but not preferred:

```awft
let bg = (await asset.image(@asset.bg.room) with:
    pending p:
        scene.show(@scene.loading)
        progress.set(p.ratio)
).context("while loading opening background")?
```

Rejected:

```awft
await asset.image(@asset.bg.room)? with:
    pending p:
        scene.show(@scene.loading)
```

Use `try await asset.image(...) with:`, `await? asset.image(...) with:`, or the
explicit parenthesized form `(await asset.image(...) with: ...)?`. Only
`await expr? with:` is rejected because it makes the owner of `with:` visually
ambiguous.

## Automatic source frames

The compiler inserts source frames at major boundaries:

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

Product builds should still preserve trace data internally for crash bundles, but UI display may be reduced.

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
  trace.awftx
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
          "file": "game/routes/opening.awft",
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
