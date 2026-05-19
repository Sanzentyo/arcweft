# Arcweft Runtime Boundary Refactor: no-alias destructive direction

## Premise

This document intentionally does **not** preserve backward compatibility. Do not add `deprecated` names, type aliases, compatibility modules, or wrapper APIs. The project is still early enough that the correct names and shapes should replace the wrong ones directly.

The current implementation still exposes `FrameInput`, `FrameInputView`, `FrameOutput`, and `FrameOutputWriter` in `crates/arcweft-core/src/frame.rs`; the engine imports and consumes them from `engine.rs`; and the CLI directly constructs `FrameInput` and reports `RuntimeFrameRunSummary`. The core runtime also still uses `RuntimeFrame` / `RuntimeFrameKind` as flow-control stack entries and stores them in `FlowFiber.frames`.

The correct direction is to make `RuntimeStep*` the core vocabulary and reserve `Frame` only for game/render/audio adapter layers.

---

## Mandatory naming decisions

### Delete these core names

```text
FrameInput
FrameInputView
FrameOutput
FrameOutputWriter
RuntimeFrame
RuntimeFrameKind
FlowFiber.frames
external_values
line_effects as top-level RuntimeStepOutput field
task_requests as top-level RuntimeStepOutput field
source_events as string-only top-level RuntimeStepOutput field
stream_events as string-only top-level RuntimeStepOutput field
```

### Use these core names instead

```text
RuntimeStepInput
RuntimeStepInputRef
RuntimeStepOutput
RuntimeStepOutputSink
RuntimeStepResult
RuntimeStepOptions
RuntimeStepBudget
RuntimeStepMode
RuntimeStepStopReason
FlowControlStackEntry
FlowControlStackEntryKind
FlowFiber.control_stack
RuntimeStepInput.bindings
RuntimeStepOutput.effects
RuntimeStepOutput.requests
RuntimeSourceEvent
RuntimeStreamEvent
RuntimePayload
```

### Keep `Frame` only here

```text
GameFrame
PresentationFrame
AudioFrame
InputRoutingFrame
RenderFrame
```

Those names belong outside `arcweft-core`, or at least outside its VM runtime boundary. Core uses `RuntimeStep`.

---

## Exact file-level changes

### `docs/00-overview/architecture.md`

Replace the architecture line:

```text
Engine::step(FrameInput) -> FrameOutput
```

with:

```text
Engine::step(RuntimeStepInput, RuntimeStepOptions) -> RuntimeStepResult
```

Replace prose saying `FrameOutput` returns `Command` / `EffectRequest` / `TaskSpec` with prose saying `RuntimeStepOutput` contains typed effect batches and host request batches.

### `docs/02-runtime/core.md`

Rewrite the public API section around the following target API:

```rust
pub struct RuntimeStepInput {
    pub clock: Option<LogicalClockInput>,
    pub bindings: Vec<RuntimeBinding>,
    pub events: HostEventBatch,
}

pub struct RuntimeStepOutput {
    pub diagnostics: Vec<RuntimeDiagnostic>,
    pub flow_events: Vec<FlowEvent>,
    pub effects: RuntimeEffectBatch,
    pub requests: HostRequestBatch,
    pub observations: RuntimeObservationDelta,
}

pub struct RuntimeStepResult {
    pub output: RuntimeStepOutput,
    pub fiber_status: FlowFiberStatus,
    pub stop_reason: RuntimeStepStopReason,
}
```

Remove examples that show `FrameInput`, `FrameOutput`, and `FrameInput::external_values`.

### `crates/arcweft-core/src/frame.rs`

Rename the file to:

```text
crates/arcweft-core/src/step.rs
```

Replace the old structs entirely. Do not keep aliases. Do not re-export old names.

Target contents should introduce:

```rust
pub struct RuntimeStepInput { ... }
pub struct RuntimeStepInputRef<'a> { ... }
pub struct RuntimeStepOutput { ... }
pub struct RuntimeStepOutputSink<'a> { ... }
pub struct RuntimeStepResult { ... }
pub struct HostEventBatch { ... }
pub struct RuntimeEffectBatch { ... }
pub struct HostRequestBatch { ... }
pub enum RuntimePayload { ... }
```

`RuntimeStepOutputSink` replaces the old `FrameOutputWriter`; it should expose output sinks by semantic domain, not just `push_diagnostic` and `merge`.

### `crates/arcweft-core/src/lib.rs`

Replace:

```rust
pub mod frame;
```

with:

```rust
pub mod step;
```

Add `payload`, `host`, or `effect_batch` modules only if the new file grows too large.

### `crates/arcweft-core/src/engine.rs`

Replace imports:

```rust
use crate::frame::{FrameInput, FrameOutput, RuntimeDiagnostic};
```

with:

```rust
use crate::step::{RuntimeStepInput, RuntimeStepOutput, RuntimeStepResult, RuntimeStepOptions, RuntimeDiagnostic};
```

Change `Engine::step` to:

```rust
pub fn step(
    &mut self,
    input: RuntimeStepInput,
    options: RuntimeStepOptions,
) -> RuntimeStepResult
```

No `step(input)` convenience overload.

Replace:

```rust
pub frames: Vec<RuntimeFrame>,
```

with:

```rust
pub control_stack: Vec<FlowControlStackEntry>,
```

Replace `RuntimeFrame` / `RuntimeFrameKind` definitions with:

```rust
pub struct FlowControlStackEntry {
    pub kind: FlowControlStackEntryKind,
}

pub enum FlowControlStackEntryKind {
    Scope,
    Loop { body: Vec<FlowOp>, result: Option<RuntimePattern> },
    While { condition: RuntimeExpr, body: Vec<FlowOp> },
    WhileLet { pattern: RuntimePattern, expr: RuntimeExpr, guard: Option<RuntimeExpr>, body: Vec<FlowOp> },
}
```

### `crates/arcweft-core/src/engine/flow.rs`

Replace all `FrameInput` / `FrameOutput` function parameters with `RuntimeStepInput` / `RuntimeStepOutput` or narrower refs/sinks.

Replace all stack operations:

```rust
self.fiber.frames.push(RuntimeFrame { kind: RuntimeFrameKind::Loop { ... } });
self.fiber.frames.last()
self.fiber.frames.pop()
```

with:

```rust
self.fiber.control_stack.push(FlowControlStackEntry { kind: FlowControlStackEntryKind::Loop { ... } });
self.fiber.control_stack.last()
self.fiber.control_stack.pop()
```

Rename helper functions:

```text
pop_scope_frame              -> pop_scope_entry
pop_scope_frames_until_loop  -> pop_scope_entries_until_loop
pop_loop_frame               -> pop_loop_entry
```

### `crates/arcweft-core/src/engine/source.rs` and `crates/arcweft-core/src/source.rs`

Replace `SourceEvent<String, String>` with `RuntimeSourceEvent` or `SourceEvent<RuntimePayload, RuntimePayload>`.

Source queues should store `RuntimePayload` or `RuntimeValue`, not `String` labels.

### `crates/arcweft-core/src/engine/stream.rs` and `crates/arcweft-core/src/stream.rs`

Replace `StreamEvent<String, String>` with `RuntimeStreamEvent` or `StreamEvent<RuntimePayload, RuntimePayload>`.

Do not lower every stream item through `runtime_value_label`; store structured payloads.

### `crates/arcweft-core/src/task.rs`

Replace `TaskSource { label: String }` as an execution discriminator with a typed request:

```rust
pub struct TaskSpec {
    pub id: TaskId,
    pub key: TaskKey,
    pub class: TaskClass,
    pub priority: TaskPriority,
    pub cancel_scope: CancelScopeId,
    pub policy: TaskPolicy,
    pub request: HostTaskRequest,
    pub debug_label: String,
}
```

Introduce:

```rust
pub enum HostTaskRequest {
    FileReadText(FileReadTextRequest),
    FileReadBytes(FileReadBytesRequest),
    FileWriteText(FileWriteTextRequest),
    FileWriteBytes(FileWriteBytesRequest),
    HttpFetch(HttpFetchRequest),
    AssetLoad(AssetRequest),
    ShaderCompile(ShaderRequest),
    AudioDecode(AudioDecodeRequest),
    TtsSynthesis(TtsRequest),
    WasmCall(WasmCallRequest),
    ProcessRun(ProcessRunRequest),
    Custom { capability: CapabilityId, operation: String, args: Vec<RuntimePayload> },
}
```

`debug_label` is for diagnostics only.

### `crates/arcweft-core/src/observation.rs`

Keep cumulative observation state, but add `RuntimeObservationDelta` for per-step output. The step output should not require consumers to diff cumulative state.

### `crates/arcweft-runtime-plan/src/expr.rs`

The strict runtime lowering currently rejects ordinary calls, method calls, index expressions, try, await, range, closure, lifetime paths, and raw expressions in value positions. This is right for unsupported executable semantics, but it also creates many “spec should compile/run” gaps.

Fix by distinguishing:

```text
pure runtime value calls         -> RuntimeExpr::Call or FunctionCall
capability calls returning Need  -> HostTaskRequest through AwaitTarget / NeedRequest
effect-only calls                -> RuntimeEffectBatch / HostEffect
adapter-facing labels            -> only for observation/debug payloads
```

Do not allow arbitrary lossy string labels in executable value positions.

### `crates/arcweft-runtime-plan/src/flow.rs`

`lower_runtime_plan` should no longer pick the first flow as implicit entry once `entry` exists. It should lower entries into `RuntimePlan.entries` and fail if the selected entry is missing.

Target:

```rust
pub struct RuntimePlan {
    pub entries: Vec<RuntimeEntryPlan>,
    pub flows: Vec<RuntimeFlow>,
    ...
}
```

### `crates/arcweft-cli/src/main.rs`

Remove `--frames`. Use runtime step vocabulary:

```bash
arcw run <file.awft> --entry @entry.main --mode game --steps 8
arcw run <file.awft> --entry @entry.main --mode drain --max-ops 10000
arcw cli <file.awft> --entry @entry.main -- ARGS...
arcw serve <file.awft> --entry @entry.http --adapter native-http
```

Replace `RuntimeRunOptions.frames` with:

```rust
steps: usize,
mode: RuntimeStepMode,
max_ops: Option<usize>,
entry: Option<String>,
```

Replace all `FrameInput { external_values: ... }` construction with `RuntimeStepInput { bindings: ... }`.

### `crates/arcweft-cli/src/output.rs`

Rename:

```text
RuntimeFrameRunSummary -> RuntimeStepRunSummary
frames                 -> steps
frame.index            -> step.index
line_effects           -> effects
```

Output JSON should separate:

```json
{
  "effects": {
    "presentation": [],
    "audio": [],
    "log": [],
    "signal": [],
    "metric": [],
    "event": [],
    "host": [],
    "control": []
  },
  "requests": {
    "tasks": [],
    "cancels": [],
    "source_close": []
  }
}
```

---

## Parser / AWFT syntax gaps that must be fixed

The current parser is much better split than before, but code reading still shows several compile-able patterns that are likely to be fragile or rejected.

### Gap 1: line-based ADT/state/trait parsing

`parse_enum_variants`, `parse_struct_fields`, `parse_state_fields`, and `parse_trait_members` parse with `body.lines()` and trim each line independently. That means documented constructs with nested/multiline payloads, multi-line defaults, or default trait methods are at risk.

Fix locations:

```text
crates/arcweft-lang-syntax/src/parser/items.rs
  parse_enum_variants
  parse_struct_fields
  parse_state_fields
  parse_trait_members
  parse_trait_member
```

Replace line-based iteration with `collect_logical_block_items` or, preferably, CST-owned block item events.

### Gap 2: naive brace counting in `collect_logical_block_items`

`collect_logical_block_items` counts `{` / `}` in raw characters and does not distinguish strings, raw strings, comments, or dialogue raw spans. This can split block items incorrectly.

Fix location:

```text
crates/arcweft-lang-syntax/src/parser/helpers.rs
  collect_logical_block_items
```

Move this to CST token-aware collection or use balanced punctuation helpers that understand strings/comments.

### Gap 3: callable items keep body as raw text

`parse_callable_item` currently captures reducer/view/activity-like callable bodies as raw body text and contracts, unlike `parse_function_item`, which parses `body_statements` and `body_value`.

Fix location:

```text
crates/arcweft-lang-syntax/src/parser/items.rs
  parse_callable_item
  CallableItem model
  HIR lowering for CallableItem
```

Reducers/views should parse structured bodies and participate in readiness/typecheck the same way functions do.

### Gap 4: strict runtime value lowering rejects pure calls

`lower_runtime_expr_strict` rejects ordinary call and method-call values except constructors. A simple flow such as `let n = add(1, 2)` should compile/run if `add` is a pure function or a recognized deterministic function.

Fix locations:

```text
crates/arcweft-runtime-plan/src/expr.rs
crates/arcweft-runtime-plan/src/flow.rs
crates/arcweft-core/src/value.rs
```

Add `RuntimeExpr::FunctionCall` or typed pure call support. Do not lower executable values into strings.

### Gap 5: entry/capability grammar is missing

The language needs `entry` and `extern capability` declarations so game/CLI/server entry points and host I/O are explicit instead of overloading the first flow or stringly calls.

Fix locations:

```text
crates/arcweft-lang-syntax/src/ast/items.rs
crates/arcweft-lang-syntax/src/parser/top_level.rs
crates/arcweft-lang-syntax/src/parser/items.rs
crates/arcweft-lang-hir/src/model.rs
crates/arcweft-lang-hir/src/lower.rs
crates/arcweft-lang-sema/src/check.rs
crates/arcweft-runtime-plan/src/flow.rs
```

### Gap 6: runtime step budget does not exist

`Engine::step` advances in a fixed way. It needs explicit `RuntimeStepOptions` and `RuntimeStepBudget`, otherwise CLI/server/game use-cases fight over one hardcoded stepping behavior.

Fix locations:

```text
crates/arcweft-core/src/engine.rs
crates/arcweft-core/src/engine/flow.rs
crates/arcweft-core/src/engine/stream.rs
crates/arcweft-cli/src/main.rs
```

---

## New AWFT grammar additions

### Entry declarations

```text
EntryDecl := Visibility? 'entry' EntryKind EntryId? EntryBlock
EntryKind := 'game' | 'cli' | 'server' | 'activity' | 'test' | 'bench' | Ident
EntryItem := 'start' EntityRef | 'run' EntityRef | RouteDecl | EntryOption
RouteDecl := 'route' HttpMethod String '->' EntityRef
```

Example:

```awft
entry game @entry.main {
    start @flow.opening
}
```

```awft
entry cli @entry.greet {
    run @flow.cli_main
}
```

```awft
entry server @entry.http {
    route GET "/health" -> @flow.health
    route GET "/hello/:name" -> @flow.hello
}
```

### Capabilities

```text
ExternCapabilityDecl := Visibility? 'extern' 'capability' CapabilityId CapabilityBlock
CapabilityItem := CapabilityFnDecl | TypeDecl | CapabilityPolicyDecl
CapabilityFnDecl := 'fn' Ident GenericParams? ParamGroup+ ReturnType? EffectClause?
EffectClause := 'effects' '{' CapabilityEffect* '}'
```

Example:

```awft
extern capability fs {
    type FsError

    fn read_text(path: VirtualPath) -> Need<String, FsError>
        effects { fs.read }

    fn write_text(path: VirtualPath, body: String) -> Need<Unit, FsError>
        effects { fs.write }
}
```

### Virtual paths

```awft
extern capability path {
    fn save(path: String) -> VirtualPath
    fn asset(path: String) -> VirtualPath
    fn temp(path: String) -> VirtualPath
    fn export(path: String) -> VirtualPath
}
```

OS paths do not appear directly in `.awft` source.

---

## Test architecture

Use fixtures as source-of-truth. Parser/HIR/sema tests and CLI tests should load the same `.awft` files.

```text
tests/fixtures/awft/current_pass/check/*.awft
tests/fixtures/awft/current_pass/run/*.awft
tests/fixtures/awft/spec_should_pass/check/*.awft
tests/fixtures/awft/spec_should_pass/run/*.awft
tests/fixtures/awft/spec_should_fail/*.awft
```

### Current-pass fixtures

These should pass on the current implementation and must stay green.

### Spec-should-pass fixtures

These encode patterns that should compile/run under the specification. They may fail now. Keep them ignored or xfail until implementation catches up, then unignore.

### Spec-should-fail fixtures

These encode intentionally rejected constructs, such as absolute OS paths or old `@` commands.

---

## Implementation checklist

### Runtime names and structure

- [ ] Rename `crates/arcweft-core/src/frame.rs` to `step.rs`.
- [ ] Delete `FrameInput`, `FrameInputView`, `FrameOutput`, `FrameOutputWriter`.
- [ ] Add `RuntimeStepInput`, `RuntimeStepInputRef`, `RuntimeStepOutput`, `RuntimeStepOutputSink`.
- [ ] Add `RuntimeStepResult`, `RuntimeStepOptions`, `RuntimeStepBudget`, `RuntimeStepMode`, `RuntimeStepStopReason`.
- [ ] Rename `FlowFiber.frames` to `FlowFiber.control_stack`.
- [ ] Delete `RuntimeFrame` and `RuntimeFrameKind`.
- [ ] Add `FlowControlStackEntry` and `FlowControlStackEntryKind`.
- [ ] Replace `external_values` with `bindings`.
- [ ] Replace top-level `line_effects` with `RuntimeEffectBatch`.
- [ ] Replace top-level `task_requests` with `HostRequestBatch`.
- [ ] Add `RuntimePayload`.
- [ ] Replace string-only source/stream events with structured payload events.

### CLI

- [ ] Remove `--frames` from `arcw run`.
- [ ] Add `--steps`, `--mode`, `--max-ops`, and `--entry`.
- [ ] Rename runtime report fields from frames to steps.
- [ ] Add `arcw cli` after entry/capability grammar lands.
- [ ] Add `arcw serve` after server adapter lands.

### Parser / language

- [ ] Add `entry` declarations.
- [ ] Add `extern capability` declarations.
- [ ] Add capability effect facts.
- [ ] Parse callable bodies structurally.
- [ ] Replace line-based ADT/state/trait parsing with logical/CST item parsing.
- [ ] Replace naive brace counting in `collect_logical_block_items`.
- [ ] Add virtual path capabilities.

### Runtime lowering

- [ ] Do not infer runtime entry from first flow once `entry` exists.
- [ ] Add `RuntimePlan.entries`.
- [ ] Add pure function call runtime lowering.
- [ ] Add typed `HostTaskRequest` lowering for file/HTTP/asset/shader/audio/TTS/process.
- [ ] Keep VM as semantic source of truth.
- [ ] Add AOT executor later as another `RuntimeExecutor` implementation, not as different semantics.

### Tests

- [ ] Add parser/HIR/sema fixture loader.
- [ ] Add CLI check fixture loader.
- [ ] Add CLI run fixture loader.
- [ ] Add current-pass fixtures.
- [ ] Add spec-should-pass fixtures.
- [ ] Add spec-should-fail fixtures.
- [ ] Unignore spec fixtures as implementation lands.
