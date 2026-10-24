# 非同期・scheduler・Need

## TaskSpec

```rust
pub struct TaskSpec {
    pub generation: GenerationId,
    pub producer: NeedProducerInstance,
    pub class: TaskClass,
    pub priority: TaskPriority,
    pub cancel_scope: CancelScopeId,
    pub policy: TaskPolicy,
    pub outcome: TaskOutcomeContract,
    pub request: TaskRequest,
    pub debug_label: String,
}

pub enum TaskRequest<P = RuntimePayload, S = RuntimeTaskSource> {
    Host(HostTaskRequest<P>),
    Program(S),
}
```

Runtime adapter tasks are created through `ensure_task`; same-key tasks join
instead of creating duplicate backend work. Arcweft source-level `thread`
blocks execute as program-owned fibers whose task requests retain their original
Source. They do not require an adapter host-call declaration.

`NeedProducerDefinition` owns the shared original function, family, contract,
task-plan coordinate, site, payload type and scheduling policy. A Thread stores
that definition directly. `NeedProducerTemplate` combines the same definition
with a Host request only for producers that actually construct Host arguments.
Thread semantic endpoints and private source encoding contain no synthetic Host
capability, operation or request arguments; their captures retain their original
program roles. AWBC Host request kernels admit only actual Host templates;
they cannot certify a program body by returning an empty Host argument tuple.
The common Q definition records each Program capture's original type, source
order and local/capture/Need provenance through capture roles. It checks the
complete role allowance before constructing those metadata rows.

AWBC task plans keep scheduling and the payload type in one common row.
`AwbcTaskPlanExecution::Host` owns the Host request projection, signature,
arguments and closed Host plan kind. `Program` owns only the original expected
task-plan key and its `ProgramTask` kernel. The version-one wire grammar encodes
this distinction directly; a Program row has no synthetic Host argument packet.
Its Source admission resolves the key in the retained original executable and
checks the Thread operation, generated task-body definition, scheduling,
payload, and complete capture type relation. Capture metadata must preserve the
original local, source order and `ExternalBinding` transfer; owned kernel input
does not grant a language Copy or Move operation. One caller-owned semantic
meter bounds this work before a Source lease is copied. An assertion or decoded
row alone grants no execution authority, and ordinary function roots and calls
cannot enter a `ProgramTask` kernel.

`RuntimePlan::task_program_body` gives code generators a borrowed projection of
the actual sealed Thread operation and its enclosing declaration. Its shared
`RuntimeTaskSourceReadContext` bounds cumulative Source lookup work and remains
rejected after an invalid lookup. The projection contains the original capture
order, body and producer; it grants neither a launch receipt nor a runtime task.

Generated AWBC Program kernels use `AwbcFunctionTypeContext::ProgramBody` to
retain the enclosing invocation's type and effect binding while accepting the
original capture ABI. Ordinary callables use `Signature`; these contexts are
not interchangeable. The original Source checks both the lexical owner and
the complete invocation effect set.

`StartProgram` names the admitted task-plan row and a typed `ThreadHandle<T>`
destination. Captures stay in the parent and require unrestricted Copy custody.
The product executor prepares the child, authenticates the original Source and
captures, and admits the actual launch receipt before committing the parent
write, child queue, quota and identity cursors. Program kernels require a Task
fiber root owned by that accepted launch. A returning child retains its result
until attached descendants retire, then transfers the value to the common
producer registry. Saved child metadata records the handle and parent identity;
restore must resolve the actual executing launch and validate the original
Program row, generation, code owner and parent relation.

Native Thread operations also own a checked `ThreadHandle<T>` result pattern.
Statement use has a typed discard pattern; value use binds the actual accepted
task receipt. Child preparation checks the original Source and captures without
changing the parent environment. The journal admits the receipt and runs the
fallible result binding before inserting its Program launch. Binding rejection
or unwind rolls back the receipt and ordinal frontier. Successful admission
then publishes the prepared child, parent bindings, quota and continuation.
The result pattern participates in the common operation traversal and the
version-one source grammar, including counted preflight and materialization.

The value boundary requires a Host request for `Need<T>` and an accepted Program
launch with producer custody for `ThreadHandle<T>`; both require the exact
payload type. Await projects the payload from either checked source type.
Native rollback resolves a saved Program handle through its retained original
plan lease and source namespace.

```rust
pub trait TaskHost {
    fn ensure_task(&mut self, spec: TaskSpec) -> TaskHandle;
    fn cancel_scope(&mut self, scope: CancelScopeId);
    fn poll_frame(&mut self, budget: SchedulerBudget) -> Vec<TaskEvent>;
}
```

`arcweft-core` keeps this interface as Sans I/O data. The task body lives in the
compiled runtime plan or in a host adapter table. `TaskRequest::Host` selects the
closed 14-family host grammar. `TaskRequest::Program` retains an actual Source
lease; admission checks its executable owner and expected task-plan key before
copying the frame or captures. Class, priority, cancellation scope and the issued
producer contract identify the scheduled work. `debug_label` is for diagnostics only and is never an execution
discriminator. Backends such as Tokio, Rayon, web workers, or a cooperative test
executor sit outside core.

Every execution frame retains its immutable original executable lease, including
dormant frames, suspended dialogue execution and the AWBC public facade. A Native
Engine executes one immutable plan; replacement constructs a new Engine. An
in-memory rollback image retains that same lease and checks exact instance
identity before projecting or restoring frame values. Equal program contents do
not authorize rebinding a frame to another executable. Frames have no ownerless
default, and public persistence does not serialize this executable lease.

Native root execution keeps one code position within that original executable.
The position records its function owner and an original expression or operation
range; it carries no independent executable authority. Function calls install
their own position and retain the caller's position in the return continuation.
Root restoration rejects a current position that disagrees with the original
body or retained function context. AOT Flow blocks resolve by the same function
identity, independently of the order of Flow wrappers in the plan.

Program dispatch remains in flight while Core executes its body and joins its
attached descendants. Core issues `ProgramTaskCompletion` on actual retirement;
the report retains the actual issuing Source lease, and the host checks it
against the accepted request before publishing its terminal through the original dispatch identity, bound
outcome and next publication revision. A host adapter cannot manufacture this
report by decoding serialized data. Duplicate or foreign terminal batches reject
before advancing the scheduler, publication frontiers or completion counters.
Equal deterministic task coordinates from another executable are insufficient.
Re-ensuring the same live Web dispatch preserves its pending work and frontier.

The common producer registry owns a Program task's returned value in its
`Ready` state. Its Program origin retains the accepted launch, with no synthetic
Host invocation token, restart policy, argument packet or initial publication
cursor. A native Await reads this local terminal state without requiring a Host
round trip. `AlwaysStart` transfers the value once into its waiter and retains
`ReadyTransferred`; an acknowledgement cannot restore a consumed value.

`ProgramTaskCompletion` retains retirement status and the authenticated original
launch. The private bound publication retains this Source proof until the
scheduler accepts the matching dispatch. The delivered
`TaskEventKind::ProgramTerminal` is a status acknowledgement, grants no execution
authority, and carries no returned Ready payload. Ordinary Host Ready events cannot complete a
Program task. Core validates the actual returned value before scope cleanup,
keeps it on its returning frame through joins and rollback, and moves it into
the registry on retirement. Empty bodies and implicit fallthrough admit Unit
through the same original result contract; they cannot manufacture success for
a non-Unit task.

Result ownership and retirement delivery have distinct state. Rollback preserves
the current delivery phase. Resume queues one metadata replay for an
unacknowledged terminal task without running the body or copying its result.
An acknowledged task does not replay. Persistence records the launch origin and
delivery phase in the existing version-one registry grammar; Program state
resolves its complete specification through the accepted task journal.

One runtime step can issue a program request and retire its actual body before
the executor reaches a terminal status. Hosts register the step's issued task
requests before publishing its program completions, including in that terminal
step. Direct host-call requests use the same output-draining boundary. The
terminal flag is applied after the step's accepted outputs have been processed.

Core retirement order does not have to match replay publication order. The
Native host binds reports to their accepted dispatches while constructing one
ordered packet with `TaskEvent::publication_order`; the scheduler receives that
whole atomic packet. Ordered-tree insertion still costs O(log n) per report and
is included in event-construction timing. Scheduler sort counters measure its
own normalization work; already ordered singleton packets also count as skipped
sort items. Native completion totals include both scheduled work and direct
runtime host calls, with `completed_host_calls` exposing the direct-call subset.
The internal `flow_thread.run_child` and `line_task.run_child` host-call routes
are absent from adapter manifests, policies and bundle requirements.

The producer contract identifies the selected callable schema or registered
host manifest contract. Standard Agent host operations use their selected
`CallableSignatureSchema`; changing an evaluated argument or its source
expression does not redefine that contract. Manifest-backed operations retain
their manifest-owned host-call contract.

Source admission retains the exact accepted expression, generation, execution
instance and canonical site. It must not mint or rehash a final
`TaskPlanSemanticDigest`. That key is issued only by the completed original
Core program's executable seal and its common function/request/control
semantics. Arguments remain part of producer-instance identity, and changed
executable expressions remain part of the completed plan's identity.

Source admission checks the producer's original declaration effect instantiation.
Instance identity includes an explicit absent/present tag, the function's
semantic type context, and its ordered effect-binding rows with canonical
effect labels. Equal Source and argument values with different admitted
instantiations must produce distinct instances. Repeating the same complete
binding is deterministic. The issued instance retains this immutable binding;
its task specification and bound outcome use that same declaration solution.
Ready states and queued events are validated against the complete accepted task
specification, including its invocation, rather than a result type in isolation.
Result construction uses this binding for both cases. A scoped outcome without
its original binding is rejected before accepting an outcome authority.

Retained descriptors count the invocation's context, rows and labels in the
enclosing resource allowance. Absent invocation metadata adds no logical node or
encoded bytes; the issued instance key already commits its absence or presence.
Immutable rows may share storage; resource
admission still checks every row before a producer or retained field copies its
lease. Version-one saves retain the binding in the producer instance rather than
an independent launch column. Restore checks the saved instance against the
original Source and the complete launch journal before admitting queued Ready
payloads; decoded binding metadata does not itself grant Source authority.

View task construction binds the exact original Core coordinates to the accepted
View program, source site and complete Match admission transcript. After every
Core slot seals, the View resource publishes inert link assertions containing
the expected task keys and those original site/admission identities. This
publication shares one immutable slot across resource clones and does not copy
the retained View source graph. Generated link assertions do not change the
accepted View semantic revision.

Complete Source decoding creates a fresh Core coordinate namespace. Its View
authority checks the resource's complete current source set before indexing
link assertions, then uses an expected key only to select an accepted binding.
The common seal recomputes the actual E/F/Q/C and upper View transcript, consumes
the one-use completion capability and compares the final expected key. A decoded
assertion cannot issue a task key or rebind the construction authority to a
foreign candidate. Core retains the actual checked prefix and its original sticky
meter inside that capability. The upper owner supplies the accepted program,
site and admission binding; it cannot replace the prefix or provide a hash state.
Core admits the complete binding's work and bytes before hashing or issuing a key.
AWBC execution retains the admitted executable and original
Source together. Entry and Flow activation, View invocation, compatible code
replacement, continuation rollback and owned restore retain that complete lease.
The saved Need registry resolves its inert Source assertions through the retained
admitted program; a raw code/type lease cannot authorize Source reconstruction.

Bundle admission returns that executable authority using the complete View
product accepted by the same bundle validation. The Core admitted AWBC program
also retains the optional plain-text context proof checked against the actual
dialogue catalog. Activation, pure invocation, continuation, compatible replacement
and owned restore retain that same complete program authority. The proof is not
an independent executor, session or rollback column. A program without that
catalog fact does not authorize String context conversion. Borrowed View
candidates enter
source and resource validation before canonical codec materialization. Agent
controllers and native bundle runners retain the returned executable/Source
lease through activation. The pure accelerator's cache owns either its exact
Native plan or its admitted AWBC program; Native cache slots require a function
capability from the matching Native plan.

Program-bound external calls transfer a `RuntimeProgramExecutionContext` with
that complete executable authority and the current formatter context. Product
VM entry, child, dialogue activation, root callable, choice-condition and task
request execution retain this context. Nested data-field defaults execute
through the same authority and locale. Raw program owners remain nominal
type/value-codec leases and cannot construct this AWBC execution context.

Backend-taking View evaluation and event dispatch also require the caller's
explicit formatter context. A Session supplies its selected locale. Expression
memoization and handler seals include that context as an execution dependency;
changing it invalidates an expression result and reseals handler routes. Event
dispatch rejects a formatter different from the presented seal before executing
or copying captures. These derived caches and seals are rebuilt after restore.
Their locale metadata enters the existing retained-metadata admission before
cloning. Cached expression hits still charge the original operation count to
the same shared frame value budget.

Cross-tier pure execution returns RuntimeProgramExecutionError with the actual
Native or AWBC cause. Its large executor errors remain boxed at the owning
boundary; no string-only generic failure replaces their kinds or source chains.

Borrowed pure-program inputs use the same activation admission as owned inputs.
Custody, the complete positional ABI, binding patterns and the selected frame
must be prepared before copying a borrowed packet. Copy permission is checked
for every transitive input graph. Rejection preserves owned inputs or releases
the original borrow; constructing an error must not copy an unadmitted value.
AWBC pure-call statistics advance only after activation is accepted. This
boundary does not grant a fresh argument, frame or execution-budget allowance.

The current Rust implementation keeps the deterministic scheduling layer in
`arcweft-runtime-scheduler`. It depends only on `arcweft-core`, accepts
`BoundTaskSpec` values, joins in-flight `JoinSameKey` tasks, sorts dispatch by
priority and stable submission order, records cancellation requests as data,
and normalizes completed `TaskEvent` values.

Adapter completions pass a mutable event slice to the scheduler's single
completion boundary. The scheduler preflights the complete batch before
reordering packets, replacing invalid Ready outcomes, or advancing its state
and counters. A rejected batch retains its original packet order and owned
payloads with the caller; no payload clone or independently reusable admission
token is required. Successful completion normalizes the same packets in place.
CLI native file tasks and source-level flow `thread` work use this scheduler. Joinable source
`thread name { ... }` lowers to `FlowOp::Thread`; entering the op publishes its
admitted Program submission and spawns the body as a scoped VM child fiber.
The scheduler retains that submission until Core's actual terminal report.
Child fibers are stepped
cooperatively by the VM and are deterministic runtime scheduling data, not OS
thread creation. Detached `thread` blocks are rejected by runtime-plan lowering
until detached capture and cancellation contracts are checked explicitly.

A task's terminal return, expression root, or body completion stays at its
original frame position while its attached descendants run. Selecting a child
does not execute or requeue the parent's return and does not charge an operation
for polling the join. Return expressions are evaluated after the join. Ordinary
function returns resume their caller in the same task without joining its
children. Cancellation still enters the owning fiber to unwind its scopes.

```rust
pub enum HostTaskRequest {
    FileReadText(FileReadTextRequest),
    FileReadBytes(FileReadBytesRequest),
    FileWriteText(FileWriteTextRequest),
    FileWriteBytes(FileWriteBytesRequest),
    HttpFetch(HttpFetchRequest),
    HttpRespond(HttpRespondRequest),
    ProcessRun(ProcessRunRequest),
    AssetLoad(AssetRequest),
    ShaderCompile(ShaderRequest),
    AudioDecode(AudioDecodeRequest),
    TtsSynthesis(TtsRequest),
    WasmCall(WasmCallRequest),
    SystemInfo(SystemInfoRequest),
    Custom {
        capability: HostCapabilityId,
        operation: String,
        args: Vec<RuntimePayload>,
    },
}
```

`system.core_count()`, `system.thread_count()`, and
`system.available_parallelism()` lower to `HostTaskRequest::SystemInfo`. The
request is still Sans I/O data in core; native adapters resolve it from the
host runtime and report the numeric result as the awaited ready value. On the
native CLI adapter, `core_count` is the host physical core count,
`thread_count` is the host logical CPU count, and `available_parallelism` is
Rust's runtime parallelism limit for the current process.

## Task class

```rust
pub enum TaskClass {
    LocalView,
    Io,
    Cpu,
    GpuPrepare,
    ShaderCompile,
    WasmCall,
    AssetDecode,
    AudioDecode,
    AudioRender,
    TtsSynthesis,
    BgmPrecompose,
    Lsp,
    Background,
}
```

## 完了順の正規化

```rust
pub fn normalize_task_events(mut events: Vec<TaskEvent>) -> Vec<TaskEvent> {
    events.sort_by_key(|event| (
        event.logical_epoch,
        event.task_id.clone(),
        event.sequence,
    ));
    events
}
```

The normalized event envelope is:

```rust
pub struct TaskEvent {
    pub logical_epoch: LogicalEpoch,
    pub task_id: TaskId,
    pub sequence: TaskSequence,
    pub kind: TaskEventKind,
}

pub enum TaskEventKind {
    Ready(RuntimePayload),
    Failed(String),
    Cancelled,
    Progress(Progress),
}
```

## single-thread と multi-thread

- native multi-thread: Tokio/Rayon 等の worker。
- native single-thread: cooperative job。
- web single-thread: requestAnimationFrame + cooperative job。
- web multi-thread: optional SharedArrayBuffer/worker pool。

## CooperativeJob

```rust
pub trait CooperativeJob {
    type Output;
    fn resume(&mut self, budget: JobBudget) -> JobPoll<Self::Output>;
}
```

Scheduler JSON counters are adapter-visible and path-free:

```text
submitted
joined
joined_completed
dispatched
completed
failed
cancelled
cancel_requested
in_flight
max_in_flight
```

## Need handling

unary `Need<T>` は `T` へ暗黙変換しない。domain failureは
`Need<Result<T, E>>`のpayloadが所有する。

Generic wait limits use the resolved standard combinator
`timeout(Need<T>, Duration) -> Need<Result<T, Timeout>>`. Timeout is a derived
Need producer driven only by `RuntimeStepInput.dt`; it is not an Await branch
and does not cancel the source producer. Same-step ordering is scope
cancellation, source terminal publication, timeout expiration, then Pending.
See [Deterministic Need timeout](need-timeout.md) for start, race, progress,
snapshot, and AWBC rules.

flow:

```arcw
let data = try await load() with {
    pending p => scene.show(@scene.loading); progress.set(p.ratio)
}
```

View:

```arcw
match load_avatar(user) {
    .pending(progress) => SkeletonCircle(progress = progress)
    .ready(.Ok(image)) => Image(image)
    .ready(.Err(error)) => ErrorMessage(error)
}
```

In the Phase 2.0 Sans I/O runtime slice, `await ... with` lowers to a
`FlowOp::Await` containing an `AwaitTarget` and pending effects. `AwaitTarget`
stores the `NeedId`, `TaskId`, and typed `HostTaskRequest` that the host adapter
must execute. Entering the op emits a `TaskSpec` using that request and switches
`FlowFiberStatus` to `Waiting`. The runtime resumes only when a matching
`TaskEvent` arrives in a later `RuntimeStepInput`.

Bounded effectful traversal lowers to `FlowOp::AwaitMany`:

```arcw
let texts = try await paths.traverse(fs.read_text).parallel(limit = 4) with {
    pending p => progress.set(p.ratio)
}
```

`AwaitMany` evaluates the source `Vec<T>` once, creates at most `limit`
in-flight `TaskSpec`s, and resumes with a `Vec` of ready values in source order.
Each child task uses a deterministic indexed task id and a same-request task key,
so duplicate requests can join through `TaskPolicy::JoinSameKey` while still
delivering completion events to each waiting index. The VM owns only the
bounded fanout state; actual thread pools, async runtimes, and filesystem or
network I/O remain adapter responsibilities.

```text
TaskEventKind::Ready(value)    -> validate and resume with the exact admitted Need payload
TaskEventKind::Progress(value) -> keep waiting and publish canonical Progress
TaskEventKind::Failed(error)   -> runtime fault; never fabricate a domain Result error
TaskEventKind::Cancelled       -> non-returning cancellation transfer
```

Actual asset loading, worker execution, clocks, renderer progress View, and audio
work remain outside `arcweft-core`. Core only produces deterministic task
requests and consumes deterministic task events.
The native CLI adapter completes read-only dispatch batches on a worker pool and
reports path-free `native_io.parallel_*` counters; write tasks stay ordered so
benchmarks can distinguish scheduler fanout from host-side mutation ordering.
Native dispatch is constrained by adapter manifests: each `HostTaskRequest`
exposes a stable host-call id, manifest data forms the active `HostCallPolicy`,
and a separate host-adapter registry maps allowed ids to concrete
implementations. Missing policy ids produce deterministic task error events.
Allowed ids without a registered native implementation also produce
deterministic task error events, so an adapter mismatch is observable instead of
becoming an implicit compatibility fallback.

Recognized capability calls such as `fs.read_text(...)`, `http.fetch(...)`,
`asset.image(...)`, `shader.compile(...)`, `audio.decode(...)`,
`tts.synthesize(...)`, `process.run(...)`, and `wasm.call(...)` lower to
dedicated `HostTaskRequest` variants. Unknown awaited call namespaces lower to
`HostTaskRequest::Custom` with structured argument payloads.

