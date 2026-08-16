# Arcweft Executable Runtime Core

Status: proposed architecture plus verified implementation cuts 1, 2, and the
shared compiled-region ABI substrate for cut 7.

Repository basis: `Sanzentyo/arcweft@23fc9206f08340df438aece556065d5235bb27eb`
(`docs: split integrated execution design requests`, 2026-06-24).

This design is intentionally one model. AWBC, the compact VM, fiber state, and
compiled regions do not own parallel representations of frames, control flow,
suspension, source identity, or host calls.

## 1. Decision summary

1. **AWBC v1 is the sole product executable representation.** It is a typed,
   register-based control-flow graph stored as canonical index tables. The old
   `compact_bytecode` table is removed rather than adapted. Structured
   `BytecodeProgram` remains only as an explicitly temporary compiler/parity
   input until the migration gates in section 15 pass.
2. **All table identities are implicit zero-based indices.** Rust newtypes make
   cross-table mistakes visible. Product bytes do not repeat string IDs or
   carry stringly opcode records.
3. **A function owns a contiguous range of global blocks, and each block owns a
   contiguous range of global non-terminator instructions.** Every block has
   exactly one typed terminator.
4. **Suspending operations are terminators.** Dialogue, choice, await,
   await-many, suspending host calls, callable boundaries, and budget yields
   reference a verified `AwbcResumePointId`.
5. **`FiberState` is the only mutable execution state exchanged between
   executors.** A compiled region receives the same frame/register/scope/source
   state that the compact VM uses; no marshal/unmarshal compatibility frame is
   permitted.
6. **The binary codec is purpose-built, canonical, and allocation-budgeted.** It
   uses a fixed envelope, fixed table order, minimal ULEB128 for counts/indices,
   fixed little-endian scalars, one-byte enum tags, and strict rejection of
   unknown tags or trailing bytes.
7. **Compiled code performs no host I/O.** Its safe Rust ABI returns a
   `CompiledStepExit`; the runtime dispatcher materializes suspension, host
   requests, traps, returns, budget accounting, or transactional VM fallback.
8. **Fallback is effect-free and consumes zero instruction budget.** The
   dispatcher restores a `FiberCheckpoint` before returning control to the VM.
9. **Persistent code-cache identity excludes generation.** Program/region
   semantics, AWBC ABI/codec, runtime and host layout digests, target features,
   backend identity/revision, optimization, and artifact kind form the cache
   key. Generation belongs only to dispatch/hot-swap identity.

## 2. Repository evidence and boundary placement

At the basis revision:

- `arcweft-core::bytecode::BytecodeProgram` wraps `FlowOp` directly and is the
  structured VM source of truth.
- `arcweft-core::awbc` is a first-pass table/verifier contract but is not the
  product execution payload.
- `arcweft-core::compact_bytecode` is a second, older validation sidecar.
- `arcweft-bundle::product::ProgramBytecode` carries both structured bytecode
  and the old compact sidecar.
- `arcweft-runtime-codegen` owns policy and placeholder region/frame contracts,
  but not a full-script execution ABI.
- `arcweft-lang-jit-cranelift` compiles pure helpers only.

The target dependency direction is:

```text
syntax / HIR / sema / verify
          |
          v
arcweft-runtime-plan (compiler-side RuntimePlan -> AWBC lowering)
          |
          v
arcweft-core::awbc
  schema + codec + verifier + FiberState + compact VM
          |
          +--------------------------+
          |                          |
          v                          v
arcweft-runtime-driver       arcweft-runtime-codegen
executor selection          region metadata/cache/ABI
          |                          |
          +-------------+------------+
                        v
              runtime-host / players
```

`arcweft-core` stays Sans I/O. Product players need only product bundle decode,
`arcweft-core`, runtime driver/host, and their adapter/render/audio layers. They
do not gain syntax, HIR, sema, compiler, verifier-front-end, CLI, or LSP
artifacts.

## 3. Canonical AWBC v1 payload

### 3.1 Envelope

A standalone canonical AWBC payload is:

| Offset | Size | Field |
|---:|---:|---|
| 0 | 8 | magic `41 57 42 43 0d 0a 1a 0a` (`AWBC\r\n\x1a\n`) |
| 8 | 2 | codec version, little-endian; v1 is `1` |
| 10 | 2 | reserved, must be zero |
| 12 | 8 | payload byte length, little-endian `u64` |
| 20 | N | canonical table payload |

`AwbcHeader` inside the table payload contains the executable ABI, minimum
runtime ABI, feature bits, runtime-layout digest, and host-ABI digest. Codec
version and executable ABI are separate: a future codec may encode the same
execution contract, and a future execution ABI may reuse the envelope.

### 3.2 Fixed table order

The payload stores these fields in exactly this order. Reordering is a codec
version change.

1. `header`
2. `strings`
3. `runtime_types`
4. `constants`
5. `effect_sets`
6. `signatures`
7. `frame_layouts`
8. `functions`
9. `blocks`
10. `instructions`
11. `resume_points`
12. `patterns`
13. `match_arms`
14. `intrinsics`
15. `host_calls`
16. `task_plans`
17. `effect_plans`
18. `choices`
19. `choice_options`
20. `content_units`
21. `line_task_groups`
22. `line_task_nodes`
23. `stream_plans`
24. `pure_helpers`
25. `display_map`
26. `source_map`
27. `resources`
28. `entries`

All IDs are `u32` table indices represented by distinct Rust newtypes. A
half-open table range is `{ start: u32, len: u32 }`; overflow and bounds are
verified before slicing.

### 3.3 Header and canonical string policy

```rust
struct AwbcHeader {
    abi_version: u32,
    minimum_runtime_abi: u32,
    feature_bits: u64,
    runtime_layout_digest: AwbcDigest, // [u8; 32]
    host_abi_digest: AwbcDigest,       // [u8; 32]
}
```

The string table is UTF-8 and strictly byte-lexicographically sorted with no
duplicates. Producers intern all stable public IDs, field/case names,
capability/operation names, source file IDs, display keys, and resource IDs
before assigning indices. This makes independently produced equivalent payloads
byte-identical.

### 3.4 Runtime type, constant, signature, and frame tables

`AwbcRuntimeType` covers the runtime value domain without host pointers:

- unit, bool;
- signed `i8/i16/i32/i64/i128/isize` and unsigned
  `u8/u16/u32/u64/u128/usize`;
- `f32`, `f64`, string, char, logical duration, entity reference;
- tuple, homogeneous sequence, named-layout record, named-layout variant;
- matrix/tensor f32/f64;
- task handle, need handle, and explicit `Dynamic`.

Records store `{ public_id?, fields: [{ name, ty }] }`; variants store
`{ public_id?, cases: [{ name, payload? }] }`. The layout digest covers the
runtime representation for all type tags, not source-language spelling.

`AwbcConstant` stores exact value bits: signed/unsigned integers use 16-byte
little-endian payloads plus width kind; floats use IEEE bit patterns; aggregate
constants reference other constants; records/variants include their type ID;
tensors store shape and exact scalar bits. Cycles are rejected.

```rust
struct AwbcEffectSet { effects: Vec<AwbcStringId> } // sorted, unique
struct AwbcSignature {
    params: Vec<AwbcTypeId>,
    result: Option<AwbcTypeId>,
    effects: AwbcEffectSetId,
}
struct AwbcFrameLayout {
    slots: Vec<AwbcFrameSlot>,
    max_scope_depth: u32,
}
struct AwbcFrameSlot {
    name: Option<AwbcStringId>,
    ty: AwbcTypeId,
    role: Parameter | Local | Temporary | ReturnValue | RuntimeState,
    scope_depth: u32,
}
```

Parameters must occupy the leading slots and match the function signature in
order. The single slot vector is simultaneously the VM register file and the
compiled-region frame contract. A slot never changes type within a function.

### 3.5 Functions, blocks, instructions, and resume points

```rust
struct AwbcFunction {
    public_id: Option<AwbcStringId>,
    kind: Flow | PureHelper | StreamTransform | LineTask | Synthetic,
    signature: AwbcSignatureId,
    frame_layout: AwbcFrameLayoutId,
    blocks: AwbcTableRange,
    entry_block: AwbcBlockId,
    flags: AwbcFunctionFlags,
}

struct AwbcBlock {
    owner: AwbcFunctionId,
    instructions: AwbcTableRange,
    terminator: AwbcTerminator,
    safe_point: AwbcSafePointKind,
    source_map: Option<AwbcSourceMapId>,
}

struct AwbcResumePoint {
    function: AwbcFunctionId,
    block: AwbcBlockId,
    frame_layout: AwbcFrameLayoutId,
    kind: AwbcSafePointKind,
}
```

Function flags are `MAY_SUSPEND`, `MAY_ALLOCATE`, `DETERMINISTIC`, and
`HAS_DYNAMIC_TARGET`. `safe_point` describes the block **entry**. A suspending
terminator references the destination entry through a resume point. Thus a flow
entry may immediately terminate in a host call without trying to encode two
safe-point kinds on one block.

Resume points are first-class table rows rather than raw block indices because
they bind function, frame layout, and reason. Both VM and compiled dispatcher
validate all four facts before resuming.

## 4. Complete compact opcode set

Opcode bytes `0x00..0x1f` are non-terminators. Bytes `0x80..0x8e` are
terminators. `0x20..0x7f` and `0x8f..0xff` are reserved and rejected in v1.

### 4.1 Non-terminators

| Code | Typed instruction | Semantics |
|---:|---|---|
| `00` | `Nop` | no state change |
| `01` | `LoadConst { dst, constant }` | clone canonical constant into typed slot |
| `02` | `Move { dst, src }` | typed value copy/move at VM policy level |
| `03` | `Clear { register }` | mark slot uninitialized |
| `04` | `EnterScope { scope }` | push lexical scope ID |
| `05` | `ExitScope { scope }` | pop matching scope; clear deeper locals |
| `06` | `BindPattern { pattern, value, mode }` | declare or assign pattern bindings |
| `07` | `TestPattern { dst, pattern, value }` | match without committing bindings; write bool |
| `08` | `MakeTuple { dst, items }` | construct typed tuple |
| `09` | `MakeSequence { dst, items }` | construct homogeneous sequence |
| `0a` | `RepeatSequence { dst, value, len }` | repeat value by integer length |
| `0b` | `SequenceLen { dst, sequence }` | sequence length |
| `0c` | `SequenceGet { dst, sequence, index }` | checked element access |
| `0d` | `SequenceSlice { dst, sequence, start }` | checked tail slice |
| `0e` | `SequencePush { sequence, value }` | append typed element |
| `0f` | `MakeRecord { dst, ty, fields }` | construct record in layout order |
| `10` | `MakeVariant { dst, ty, case, payload }` | construct typed variant |
| `11` | `ProjectTuple { dst, target, ordinal }` | checked tuple projection |
| `12` | `ProjectRecord { dst, target, ordinal }` | checked layout projection |
| `13` | `ProjectField { dst, target, field }` | checked named record projection |
| `14` | `Unary { dst, op, src }` | `Not` or `Neg` |
| `15` | `Binary { dst, op, lhs, rhs }` | equality/order/arithmetic/logical operators |
| `16` | `CallPureHelper { dst, helper, args }` | deterministic helper call |
| `17` | `CallIntrinsic { dst?, intrinsic, args }` | typed runtime registry intrinsic |
| `18` | `EnsureContent { content }` | stage/validate content readiness as data request |
| `19` | `EmitEffect { effect, args }` | append typed effect request; never perform I/O |
| `1a` | `StartTask { dst, plan, args }` | create/ensure host task handle request |
| `1b` | `SpawnFiber { dst?, function, args }` | spawn executor-neutral child fiber |
| `1c` | `StreamYield { stream, value }` | enqueue deterministic stream output |
| `1d` | `StreamClose { stream }` | close stream state |
| `1e` | reserved | rejected in v1 |
| `1f` | `Drop { register }` | deterministic value/resource drop and uninitialize |

`EmitEffect` is for already modeled Sans I/O effect requests. A call that must
cross a host ABI boundary uses `HostCall` below. There is no generic “execute
string opcode”.

### 4.2 Terminators

| Code | Typed terminator | Successor / exit |
|---:|---|---|
| `80` | `Jump { target }` | one CFG successor |
| `81` | `Branch { condition, then_block, else_block }` | bool branch |
| `82` | `Match { scrutinee, arms, default }` | pattern/guard dispatch |
| `83` | `CallFunction { function, args, dst?, resume }` | push frame; return through resume point |
| `84` | `GotoStatic { function, args }` | tail transfer, no caller continuation |
| `85` | `GotoDynamic { target, args }` | verified dynamic flow/entity target; VM-capable fallback boundary |
| `86` | `Dialogue { content, line_task_group, resume }` | suspend for presentation/input |
| `87` | `Choice { choice, dst, resume }` | suspend; selected option is written to `dst` |
| `88` | `Await { task, binding?, resume }` | suspend one host task; bind ready value |
| `89` | `AwaitMany { plan, source, binding?, resume }` | suspend deterministic bounded task fan-out |
| `8a` | `HostCall { call, args, dst?, resume }` | structured host request; immediate mode may resume in same step |
| `8b` | `Return { value? }` | pop frame or terminate root fiber |
| `8c` | `Trap { code, message? }` | typed terminal failure |
| `8d` | `BudgetYield { resume }` | cooperative preemption at verified point |
| `8e` | `Unreachable` | verifier-known dead endpoint / defensive trap |

Loops are CFG, not one-off opcodes. A loop header is a block entry marked
`LoopBackedge`; every backward edge targets such an entry. `break` and
`continue` lower to jumps selected from an explicit lowering-time loop stack.
Match, scoped bindings, dynamic targets, return, traps, and budget yielding are
therefore covered without embedding structured AST nodes into instructions.

## 5. Remaining executable tables

### 5.1 Patterns and match arms

`AwbcPattern` is an acyclic graph with variants for bind, discard, literal,
entity, tuple, record, sequence with rest slot, variant, and whole-value bind.
Record pattern fields store layout ordinals, not names. A match arm is
`{ pattern, guard: Option<AwbcFunctionId>, target: AwbcBlockId }`; guards are
pure `(scrutinee) -> bool` functions.

### 5.2 Intrinsics, host calls, tasks, and effects

```rust
struct AwbcIntrinsic {
    public_id: AwbcStringId,
    registry_code: u32,
    signature: AwbcSignatureId,
    revision: u32,
}
struct AwbcHostCall {
    public_id: AwbcStringId,
    capability: AwbcStringId,
    operation: AwbcStringId,
    signature: AwbcSignatureId,
    mode: Immediate | Suspend,
    deterministic: bool,
}
struct AwbcTaskPlan {
    public_id: AwbcStringId,
    capability: AwbcStringId,
    operation: AwbcStringId,
    signature: AwbcSignatureId,
    class: AwbcTaskClass,
    priority: i32,
    cancel_scope: AwbcStringId,
    policy: JoinSameKey | AlwaysStart,
    arguments: Vec<{ name: Option<StringId>, spread: bool }>,
    many: Option<{ item_binding: RegisterId, limit: u32 }>,
}
struct AwbcEffectPlan {
    kind: AwbcEffectKind,
    signature: AwbcSignatureId,
    capability: Option<AwbcStringId>,
    static_args: Vec<AwbcConstantId>,
    resources: Vec<AwbcResourceAccess>,
}
```

Task classes preserve the current LocalView/Io/Cpu/GPU/shader/Wasm/asset/audio/TTS/
BGM/LSP/background set. Effect kinds preserve register/drop handle, wait, audio,
call, log, signal/metric/event/out, return/goto, panic/fail/bail, ensure/assert,
close/select, and loop control. Resource access stores resource ID, access mode,
and typed conflict policy (`Error`, `Append`, priority LWW, merge patch, or a
fixed reduce operator).

The host-ABI digest is computed by the producer from the canonical catalog of
`(public_id, capability, operation, signature, mode, revision/policy)`. The
runtime supplies its expected digest to the verifier. Individual capability
names are additionally checked against product policy.

### 5.3 Choice, content, and line-task tables

```rust
struct AwbcChoice { public_id: Option<StringId>, options: TableRange }
struct AwbcChoiceOption {
    public_id: Option<StringId>, label: StringId,
    condition: Option<FunctionId>, target: Option<FunctionId>,
    out_effect: Option<EffectPlanId>, effects: Vec<EffectPlanId>,
}
struct AwbcContentUnit {
    public_id: StringId,
    line_task_group: Option<LineTaskGroupId>,
    display: Option<DisplayMapId>, source: Option<SourceMapId>,
    resources: Vec<ResourceId>,
}
```

A line-task group contains a root node, constant options, optional binding/out
functions, cancellation handlers, and cleanup policy. Line nodes are sequence,
start, join-all parallel, child task, or effect. Child nodes preserve trigger,
join, cancellation, and nested scope. This removes the current line plan’s
string expressions from the product boundary: executable expressions lower to
functions/registers, while only stable display/config strings remain strings.

### 5.4 Streams

```rust
struct AwbcStreamPlan {
    public_id: StringId, item_type: TypeId, error_type: TypeId,
    transform: FunctionId,
}
```

External capability operations returning `Stream<T, E>` are represented by the
ordinary callable catalog and host-call tables. Stream transforms use the
ordinary CFG/opcode set; there is no separate plan, handler table, or
stream-only function kind.

### 5.5 Pure helpers, maps, resources, and entries

Pure helpers store public ID, signature, function, scalar-fast-path flag, and
annotated/inferred origin. They are ordinary AWBC functions; a backend may
compile them separately, but the compact VM can always execute them.

Display maps are `{ content, display_key }` with unique content identity. Source
maps are `{ location, source_file, start, end, anchor? }`, where location is an
instruction, block, or resume point. Source spans are byte offsets and must be
ordered and budget-bounded. A trap reports an optional `AwbcSourceMapId`, so VM
and compiled code produce the same failure location.

Resource rows are `{ public_id, kind, digest, decoded_len, residency }` with
startup/on-demand/streaming residency. No bytes or filesystem paths are opened
inside core.

Entrypoints preserve game/CLI/server/activity/test/bench/custom kind, signature,
and either a function or typed routes. A route stores method, path, target
function, and explicit register bindings from path parameters.

## 6. Canonical binary codec and decode budgets

### 6.1 Primitive encoding

- `u8`, fixed arrays, and digests are exact bytes.
- `u16/u32/u64/i32` scalar payloads are little-endian where fixed-width is
  specified by the schema.
- Table counts, vector lengths, string lengths, IDs, ordinals, and ranges use
  **minimal unsigned LEB128**. Encodings with redundant continuation bytes are
  rejected.
- Enums use one-byte stable tags defined by their owning enum implementation.
  Unknown tags are errors; they are not skipped as extensions in v1.
- `Option<T>` uses tag `0`/`1`; booleans use `0`/`1`; other tags are rejected.
- Strings are length-prefixed UTF-8 without NUL termination.
- There is no padding, native alignment, serde/bincode layout, map iteration, or
  platform-width integer in product bytes.
- A decoder must consume the declared payload exactly.

The encoder emits tables in stored order. Producers canonicalize strings,
effect sets, function/block/instruction ownership order, source/display rows,
and any semantically unordered collections before encoding. The verifier rejects
noncanonical forms rather than silently sorting attacker-controlled payloads.

### 6.2 Default decode budgets

All limits are checked before allocation and are caller-overridable downward.

| Budget | Default |
|---|---:|
| encoded bytes | 256 MiB |
| strings | 1,000,000 |
| total UTF-8 bytes | 64 MiB |
| runtime types / effect sets / signatures / frame layouts | 262,144 each |
| constants | 1,000,000 |
| functions | 262,144 |
| blocks | 1,000,000 |
| instructions | 8,000,000 |
| resume points | 2,000,000 |
| patterns | 1,000,000 |
| match arms | 2,000,000 |
| intrinsics / host calls / task plans | 262,144 each |
| effect plans | 1,000,000 |
| choices | 262,144 |
| choice options | 1,000,000 |
| content units / line task groups | 1,000,000 each |
| line task nodes | 4,000,000 |
| stream plans | 262,144 |
| pure helpers | 262,144 |
| display-map rows | 2,000,000 |
| source-map rows | 8,000,000 |
| resources | 1,000,000 |
| entries | 262,144 |
| aggregate collection items | 16,000,000 |
| tensor elements | 16,000,000 |
| nesting depth | 64 |

A product profile may set smaller values. Raising defaults requires a resource
analysis and a codec-version-compatible implementation review.

## 7. Lowering `RuntimePlan`, `FlowOp`, and `RuntimeExpr`

Lowering lives compiler-side in `arcweft-runtime-plan::awbc_lower`, not in a
player. It uses four deterministic passes:

1. **Inventory:** collect and sort strings, public IDs, resources, capabilities,
   runtime types, signatures, and callable identities.
2. **Function planning:** assign every flow, pure helper, stream transform,
   line-task helper, condition/guard, and synthetic expression body a function
   and frame layout. Allocate stable slots by parameter order, lexical local
   declaration order, then deterministic temporary order.
3. **CFG lowering:** lower structured operations/expressions to blocks,
   instructions, terminators, patterns, and resume points. Emit explicit scope
   and loop stacks only in the lowering context; they do not survive as AST
   payloads.
4. **Canonicalization and proof:** flatten function-local blocks/instructions to
   global contiguous tables, resolve IDs, canonicalize maps/sets, verify, encode,
   decode, and verify again in compiler tests.

### 7.1 `FlowOp` mapping

| Structured operation | AWBC lowering |
|---|---|
| `Bind(bindings)` | constants/expressions into assigned local slots; one `BindPattern` per binding |
| `Let { pattern, expr }` | lower expression to temp; `BindPattern(Declare)` |
| `LetElse` | temp + `TestPattern`; branch to bind/body or explicit else CFG |
| `Dialogue` | `EnsureContent`; `Dialogue` terminator and dialogue resume block |
| `Choice` | choice/option rows; optional pure condition functions; `Choice` terminator; resume block dispatches selected option effects/target |
| `Await` | lower pending effects; materialize task plan/start; `Await` terminator with optional pattern |
| `AwaitMany` | lower source sequence and bounded task plan; `AwaitMany` terminator; persisted `FiberAwaitManyState` controls deterministic launch/result order |
| `If` | condition temp + `Branch`, then/else blocks, join block |
| `IfLet` | temp + `TestPattern`; optional pure guard block; commit bindings only on successful path |
| `Match` | scrutinee temp; pattern rows and optional guard functions; `Match` terminator; arm/join blocks |
| `Loop` / `LoopNext` | loop-header block marked `LoopBackedge`; body; backward `Jump`; insert `BudgetYield` according to lowering budget policy |
| `LetLoop` | value-producing loop with dedicated result slot and break join |
| `While` / `WhileNext` | header condition + `Branch`; body backedge; exit block |
| `WhileLet` / `WhileLetNext` | expression + pattern/guard blocks at header; binding scope per iteration |
| `For` / `ForNext` | sequence/index slots; header `SequenceLen`/comparison; `SequenceGet`; pattern bind; increment; backedge |
| `Thread` | synthetic flow function + `SpawnFiber`; optional name becomes public/debug ID, not semantics |
| `Scope` | `EnterScope`; body; all normal/control exits route through cleanup block and `ExitScope` |
| `LetScope` | scope plus result slot; expression/bind on successful cleanup exit |
| `Break(value?)` | optional result write, cleanup chain, jump to current loop exit |
| `Continue` | cleanup chain, jump to current loop header |
| `Goto` | `GotoStatic` tail transfer |
| `GotoExpr` | target expression + `GotoDynamic`; function flag `HAS_DYNAMIC_TARGET` |
| `Return(String)` | intern/load string constant then `Return` |
| `ReturnExpr` | expression temp then `Return` |
| `Effect` | typed `EffectPlan`; `EmitEffect`, `HostCall`, `Goto*`, `Return`, or `Trap` according to the existing effect enum’s semantic category |
| `EnterScope` / `ExitScope` | direct scope instructions |
| `ExitScopeBind` | evaluate value, exit/cleanup, bind in parent scope |
| `Noop` | `Nop`, removed when no map anchor requires it |

The runtime-only continuation variants (`LoopNext`, `WhileNext`,
`WhileLetNext`, `ForNext`) are not serialized as distinct operations. The
compiler lowers the originating structured loop before execution. If a parity
adapter receives a continuation form, it reconstructs the same header/body CFG
and then discards the continuation object.

### 7.2 `RuntimeExpr` mapping

- `Value`, `Local`, `EntityRef`: `LoadConst` or existing slot.
- expression `Let`: child scope, expression temp, binding slot, body, exit.
- tuple/bracket/repeat/record/variant: corresponding construction opcodes.
- field/tuple/record projection: corresponding projection opcode.
- `Call`: statically resolved pure/runtime function -> `CallFunction`; host
  catalog target -> `HostCall`; unresolved dynamic call is rejected before AWBC.
- `PureCall`: `CallPureHelper`.
- spread arguments: flattened at lowering when tuple arity is static; otherwise
  a typed sequence materialization plus a host/intrinsic ABI that explicitly
  accepts a sequence. No spread marker survives in register argument lists.
- method calls: resolve to intrinsic, pure helper, function, or host call before
  encoding.
- map/sum: synthetic loop CFG or a revisioned typed intrinsic when the registry
  guarantees identical semantics.
- unary/binary: corresponding typed instruction; `&&` and `||` lower to CFG for
  short-circuiting rather than eager `Binary`.
- expression `If`: branch with one result slot and join.
- `IfLet`: test/guard, binding scope, result slot and join.
- expression `Match`: match table, arm result writes, join.

Patterns lower structurally into the pattern table. Bind targets are frame slot
IDs, typed patterns reference `AwbcTypeId`, and record fields become verified
layout ordinals. Duplicate binding names are rejected before or during AWBC
verification.

### 7.3 Line tasks, streams, and pure helpers

Current string-valued line binding/out/assertion fragments must not be copied
into product AWBC. The compiler lowers executable fragments into `LineTask`
functions and typed effect plans. External capability calls use the ordinary
host-call catalog, while stream operations become `StreamTransform` functions.
Pure helpers become `PureHelper` functions and a metadata row; the existing
scalar evaluator/JIT may use the metadata but must produce the same runtime
value and trap behavior as the compact VM.

## 8. Executor-neutral `FiberState`

```rust
struct FiberState {
    generation: u64,
    entry: AwbcEntryId,
    cursor: { function, block, instruction_offset },
    frames: Vec<FiberFrame>,
    status: Running | Suspended | Returned | Trapped,
    suspension: Option<FiberSuspension>,
    terminal: Option<Returned(value?) | Trapped(FiberTrap)>,
    budget: { remaining, quantum },
    line_cursor: u64,
    streams: Vec<FiberStreamState>,
}
struct FiberFrame {
    function: AwbcFunctionId,
    layout: AwbcFrameLayoutId,
    return_to: Option<{
        resume: AwbcResumePointId,
        destination: Option<AwbcRegisterId>,
    }>,
    registers: Vec<Option<RuntimeValue>>,
    scopes: Vec<{ id, depth }>,
}
```

Suspension reasons are dialogue, choice, await, await-many, host call, and
budget yield. Await-many state contains plan, optional bind pattern, input
items, next launch index, in-flight `(index, task_id, need_id)` rows, and
index-aligned optional results. This is enough to resume deterministically after
host events, snapshot/replay, or hand-off between VM and compiled code.

Invariants:

- active frame function equals cursor function;
- frame register count equals its immutable layout slot count;
- safe-point cursors have instruction offset zero;
- running fibers have no suspension; suspended fibers have exactly one;
- returned/trapped fibers have exactly one terminal value;
- scope stack IDs/depths agree with verifier-proven block-entry state;
- stream vectors are indexed by their AWBC table IDs;
- generation is checked before dispatch into a compiled artifact;
- budget is charged only by the dispatcher/VM, never by host or generated code;
- a nested return validates value/destination shape before popping the callee,
  resumes the caller at `return_to.resume`, and writes the value to
  `return_to.destination`; a root return alone makes the fiber terminal.

`FiberCheckpoint` owns a clone of the complete state and is used only around a
compiled-region transaction. It is not a persistence format; save/replay
versioning is a later request.

## 9. Shared frames, resume points, safe points, and suspension

A `FiberSafePoint` is `{ generation, cursor, frame_layout, resume? }`. Compiled
metadata lists accepted `(function, block, resume?)` entries. Dispatch requires:

1. matching compiled ABI and program/runtime/host digests;
2. matching fiber generation;
3. running status;
4. cursor at instruction offset zero;
5. active frame function/layout matching metadata;
6. when a resume ID is present, its function/block/layout matching the cursor.

The compact VM checks budget at block entry and before a backward transfer. It
may produce `BudgetYield` only through a verified resume point. Dialogue,
choice, await, await-many, and host call commit their arguments/effect staging,
store a `FiberSuspension`, and return to the driver. On host input, the driver
writes the result to the declared destination/pattern and calls `resume_at`;
`resume_at` validates function/layout and moves to the resume block.

A compiled region follows the same transition. It may update frame registers
and deterministic fiber-local state, but external effects are only staged as
`CompiledStepExit` data. The dispatcher owns status, suspension, terminal, and
budget transitions.

## 10. Semantic verifier

Verification occurs after budgeted decode and before execution or codegen.
Required phases are:

1. **Header/canonical form:** executable ABI, minimum runtime ABI, feature bits,
   host digest, strictly sorted strings/effects, reserved flags.
2. **Table shape:** every index/range, checked range overflow, one owner for each
   block/instruction, contiguous function/block ranges, entry block ownership,
   unique public/map identities where required.
3. **Type graph/constants:** all child type IDs, record/variant name uniqueness,
   acyclic/bounded aggregate constants, exact integer/float/char/tensor shape
   validity, aggregate constant/layout compatibility.
4. **Signatures/frames:** parameter and frame budgets, leading parameter slots,
   exact parameter types, legal slot scope depth, result slot policy, function
   kind/flag consistency.
5. **Patterns:** acyclic bounded graph, field/case bounds, duplicate binding
   targets, typed compatibility, rest-slot compatibility.
6. **CFG:** all targets stay in owner function, every declared block reachable,
   backward edges target `LoopBackedge`, terminator/result shape validity,
   resume ownership and kind.
7. **Definite initialization:** forward dataflow; predecessor merge is
   intersection; every read is initialized; `Clear`, `Drop`, and scope exit
   uninitialize slots; predecessor scope stacks must be identical.
8. **Opcode typing:** construction/projection/operator types, sequence element
   compatibility, helper/intrinsic/function/host/task arity and results,
   return signature, dynamic-target type and function flag.
9. **Effects/capabilities:** callee required effect set is a subset of caller’s
   declared set; host/task/effect capability is allowed; host-call signature and
   mode match catalog/digest; pure helpers and guards have no undeclared effects.
10. **Entrypoints:** entry signature equals function/route target signature;
    route bindings reference parameter slots and compatible adapter value types.
11. **Line/choice/stream:** table kind/function kind compatibility, valid
    task/cleanup links, bounded await-many limit, choice targets/conditions/
    effects.
12. **Maps/resources:** source span ordering/budget, location ownership,
    display/content cross-reference agreement, unique resource identity and
    resource access bounds.

Default semantic budgets are 65,536 frame slots/function, 4,096 parameters,
4,096 call arguments, 16,000,000 CFG edges, pattern depth 64, 32,000,000
dataflow steps, and 64 MiB per source span.

Diagnostics carry typed table/index/function/block/register data. They do not
fall back to “invalid bytecode” strings.

## 11. Compact VM execution

The compact VM belongs in `arcweft-core::awbc::vm` and implements a small
executor interface used by the runtime driver:

```rust
trait AwbcExecutor {
    fn step(
        &mut self,
        program: &VerifiedAwbcProgram,
        fiber: &mut FiberState,
        input: &RuntimeStepInput,
        budget: u64,
    ) -> AwbcStepOutput;
}
```

`VerifiedAwbcProgram` is created only by successful verification and holds
precomputed immutable lookup facts (function block ranges, block instruction
ranges, optional decoded constant cache). It does not duplicate executable
semantics.

Dispatch loop:

1. validate running fiber/cursor/frame;
2. check block-entry budget/safe-point policy;
3. execute the block’s instruction range in order, charging one baseline unit
   per instruction (revisioned cost tables are future ABI work);
4. execute exactly one terminator;
5. continue to an internal successor while budget remains, or return a
   structured step output at suspension/host/effect/return/trap/yield;
6. never call filesystem/network/audio/render APIs directly.

Parity with the structured VM is defined over normalized observations, not
internal instruction counts: flow events in order, task requests, effect
requests, dialogue/choice payloads, source/stream queue state, return/trap,
logical timing, and replay-visible IDs. Host events are fed identically to both
executors. Differential tests compare observations after every externally
visible step and final fiber state after canonical normalization.

The dev/test selector is explicit (`StructuredParity`, `CompactVm`, or
`Differential`) and never silently chooses one model after an error. Product
selection becomes compact-only at cut 6.

## 12. Baseline full-script compiled-region ABI

The implemented safe Rust boundary is:

```rust
trait CompiledRegion: Send + Sync {
    fn metadata(&self) -> &CompiledRegionMetadata;
    fn step(&self, input: CompiledRegionInput<'_>) -> CompiledRegionResult;
}
struct CompiledRegionInput<'a> {
    program: &'a AwbcProgram,
    fiber: &'a mut FiberState,
    instruction_budget: u64,
}
struct CompiledRegionResult {
    consumed: u64,
    exit: CompiledStepExit,
}
```

Baseline codegen is permitted to be an unoptimized direct translation of AWBC
blocks. It must preserve register layout, operation order, exact numeric
semantics, source IDs, and safe-point exits. It receives no host trait or raw
function pointer. Native loaders and executable memory remain outside this
crate.

`CompiledStepExit` mapping is:

| Exit | Dispatcher action |
|---|---|
| `Continue { next }` | validate safe point; set cursor; retain deterministic mutations; charge consumed budget |
| `HostRequest(request)` | validate HostCall resume; store host suspension; return request |
| `Suspended(state)` | map reason to expected resume kind; store suspension |
| `Returned(value)` | mark root/call result according to frame policy |
| `BudgetExhausted { resume }` | validate BudgetYield resume; store budget suspension |
| `Failed(failure)` | restore entry checkpoint, charge consumed work, materialize typed trap/source map |
| `FallbackToVm(fallback)` | restore entry checkpoint; require `consumed == 0`; validate original safe point; dispatch VM |

The dispatcher resets `fiber.budget` to its entry value before charging the
reported count, so generated code cannot self-account or mint budget. A result
that reports more than `min(requested_budget, entry_remaining)` is rejected and
rolled back.

## 13. VM fallback rules

Fallback is allowed only when policy permits it and the fiber is at a verified
region entry. Reasons include unsupported opcode/type/intrinsic, dynamic target,
missing region, stale generation, rejected artifact, host/runtime ABI mismatch,
backend unavailable, budget preemption, and explicit dev selection.

Before a region starts, capability inventory determines whether every opcode in
the region is supported. Predictable unsupported operations should therefore
avoid compiled dispatch entirely. Runtime fallback is reserved for facts that
can change or are data-dependent. No host request/effect may be emitted before a
fallback. Transactional restoration and zero consumed budget make retrying in
the VM observationally identical to never entering compiled code.

ABI/digest mismatch is normally artifact rejection before execution. If policy
allows VM fallback, the selector records the rejection and starts the VM at the
same safe point; it does not invoke an incompatible region and then attempt to
repair state.

## 14. Code artifact and cache identity

A code region records function, entry block, entry resume points, supported
opcode bitset, semantic digest, and contract flags (may suspend/request host,
dynamic target, stages effects). Artifact kinds are JIT, native object, native
shared library, and Wasm module.

Persistent `RuntimeCodeCacheKey` includes:

- cache-key schema version;
- artifact kind;
- canonical program digest;
- canonical region semantic digest;
- AWBC executable ABI and codec version;
- runtime-layout digest;
- host-ABI digest;
- target triple;
- canonical CPU feature digest;
- optional Wasm feature digest;
- backend ID and backend revision;
- optimization level.

The key is hashed with a domain-separated BLAKE3 transcript and
length-prefixed strings. Native JIT and AOT use CPU features; Wasm AOT must set
Wasm features and use a canonical Wasm target triple. Generation is excluded so
an unchanged region can be reused across hot swaps; `RuntimeDispatchKey` adds
`{ generation, region, artifact_digest }` for the live dispatch table.

## 15. Migration and deletion gates

The structured `BytecodeProgram` may be removed from product AWBC/AWFB only when
all gates are green on supported profiles:

1. canonical encode/decode determinism and decode-budget tests pass;
2. verifier negative corpus covers unknown tags, indices/ranges, ownership,
   registers, CFG, types, host ABI, effects/capabilities, entries, and maps;
3. compiler emits AWBC for every product fixture without carrying a serialized
   `FlowOp` fallback;
4. differential structured/compact tests pass for flow, dialogue, choice,
   await, await-many, static/dynamic goto, match, every loop form, scoped
   cleanup/binding, line tasks, and streams;
5. suspend/save-in-memory/resume tests pass at every safe-point kind;
6. runtime driver and runtime host construct executors from verified AWBC and no
   product player imports structured bytecode execution;
7. source/display maps and failure diagnostics are byte-for-byte or
   semantically normalized equivalents across executors;
8. baseline compiled regions pass VM fallback and differential tests;
9. product AWFB decoder accepts a fixture whose ProgramBytecode section contains
   canonical AWBC only;
10. dependency audit proves players have no syntax/HIR/sema/compiler/verifier
    front-end/CLI/LSP dependency;
11. repository search finds no product serialization/deserialization of
    `BytecodeProgram`, `FlowOp`, or `compact_bytecode`;
12. old sidecar and conversion APIs are deleted in the same cleanup cut, not
    retained behind an undocumented compatibility path.

Compiler-only `RuntimePlan` remains a useful lowering IR. Deleting it is not a
migration goal. The deletion target is its use as product runtime payload and
its execution as a second product VM.

## 16. Implementation cuts

### Cut 1 — freeze schema and codec

- Replace `arcweft-core/src/awbc.rs` with `awbc/{schema,codec}.rs`.
- Add typed IDs, all tables/opcodes, stable enum mappings, canonical envelope,
  manual codec, and decode budgets.
- Tests: deterministic bytes, round trip, unknown tag/noncanonical varint,
  payload/trailing/UTF-8 and each allocation budget family.

### Cut 2 — semantic verifier

- Add `awbc/verify/{structure,code}.rs`.
- Verify ownership, CFG, definite initialization, scopes, types, calls,
  effects/capabilities, entrypoints, maps, and budgets.
- Fuzz target accepts bytes only through budgeted decode then verify; no I/O.

### Cut 3 — compiler lowering with structured parity

- Add `arcweft-runtime-plan/src/awbc_lower/{inventory,frame,expr,flow,line}.rs`.
- Compiler emits and verifies AWBC in addition to the explicit structured parity
  artifact. Product bundle format is not switched yet.
- Golden tests assert canonical table shapes and source-map anchors.

### Cut 4 — compact VM behind explicit dev/test selection

- Add `arcweft-core/src/awbc/vm/{dispatch,expr,pattern,suspend}.rs`.
- Use `FiberState`; emit existing `RuntimeStepOutput` observations.
- No implicit fallback to structured execution.

### Cut 5 — differential fixtures

- Add a shared harness that drives identical `RuntimeStepInput`/task/stream
  events into structured and compact executors and compares normalized outputs
  at each external boundary.

### Cut 6 — migrate driver/host construction

- Runtime driver owns `VerifiedAwbcProgram` and `FiberState`.
- Runtime host/player constructors take an explicit executor selection and
  verified AWBC. Structured executor remains test-only and compiler-feature
  gated.

### Cut 7 — baseline full-script region lowering

- Reshape `arcweft-runtime-codegen` around AWBC region metadata, cache identity,
  and the implemented `CompiledRegion` ABI.
- First backend may interpret the lowered region or emit unoptimized code; the
  purpose is ABI parity, not speed.

### Cut 8 — product payload switch and deletion

- Change `arcweft-bundle::product::ProgramBytecode` to contain canonical AWBC
  bytes/metadata only.
- Update AWFB validation/decode and player fixtures.
- Delete old compact sidecar and structured product payload/conversions after
  section 15 gates pass.

This order is retained. Moving codegen earlier would freeze a duplicate frame or
resume ABI; moving product migration earlier would make parity failures harder
to isolate.

## 17. Test matrix

### Codec

- same program encodes identically across runs and after decode/re-encode;
- all opcode/tag values round-trip; reserved/unknown tag fails at stable offset;
- nonminimal ULEB128, bad magic/version/reserved bits, bad length, trailing data,
  invalid UTF-8, and nesting overflow fail;
- every table/count/string/tensor/aggregate budget fails before allocation.

### Verifier

- bad table/range/owner, register, branch, backedge, resume, frame, entry,
  host-call signature, capability, effect set, type, pattern cycle/depth,
  source/display map and resource diagnostic;
- diamond CFG proves definite-initialization intersection;
- scope-stack mismatch at a join fails;
- all entry kinds/routes and result signatures pass/fail appropriately.

### VM parity

Fixtures cover bind/let/let-else; dialogue and line cancellation/cleanup; choice
conditions/effects/targets; await ready/error/progress/cancel; await-many limits
and result ordering; if/if-let/match; all loop variants and break values;
threads/scopes; static/dynamic goto; returns/traps; stream transforms and
external capability calls; pure helpers and numeric edge cases.

### Safe points and codegen

- suspend/resume for dialogue, choice, await, await-many, host, and budget;
- call-frame return through resume point;
- compiled continue, host request, suspension, return, failure, and budget exit;
- fallback restores every mutated fiber field and consumes zero;
- stale generation/digest/layout/host ABI and invalid safe point reject;
- JIT/native/Wasm cache-key sensitivity and generation-independent reuse.

### Product migration

- AWFB decodes and runs with no structured bytecode payload;
- malformed AWBC is rejected under product budgets;
- source/display/resource maps survive bundle round trip;
- dependency and repository-search guards prevent structured payload regression.

## 18. Affected crates and modules

| Crate/module | Change |
|---|---|
| `arcweft-core::awbc` | canonical schema, codec, verifier, fiber; later compact VM |
| `arcweft-runtime-plan` | compiler-side deterministic AWBC lowering |
| `arcweft-runtime-codegen` | AWBC region inventory, cache key, safe compiled-step ABI |
| `arcweft-runtime-driver` | verified program/fiber ownership and executor dispatch |
| `arcweft-runtime-host` | materialize typed requests/exits; no executable semantics duplication |
| `arcweft-bundle::product` | cut 8 canonical AWBC-only ProgramBytecode section |
| players | consume verified AWBC through runtime boundary only |
| `arcweft-lang-jit-cranelift` | retain pure-helper backend; later implement same region ABI where supported |
| tests/fixtures | differential, negative codec/verifier, product migration corpus |

## 19. Explicit non-goals

- optimizing code generation, register allocation, inlining, deoptimization, or
  tiering heuristics;
- save-game/persistent cache file lifecycle, generation hot-swap protocol, or
  Agent REPL runtime tiers, which are later sequence requests;
- embedding syntax/HIR/type-checker data in product AWBC;
- arbitrary native plugin ABI, executable-memory allocation, dynamic-library
  loading, or host I/O in core/codegen contracts;
- preserving the old compact sidecar or structured product VM as a silent
  compatibility layer;
- making unknown v1 opcodes skippable;
- using `unsafe`, unstable Rust, platform pointers, or native struct layout in
  the executable ABI.

## 20. Implementation included with this design

The accompanying overlay implements and tests:

- the complete typed schema/opcode/table model above;
- canonical manual AWBC v1 encode/decode and budgets;
- structural and dataflow verifier;
- executor-neutral fiber/checkpoint/suspension state;
- AWBC-backed code artifact/cache model;
- baseline safe compiled-region ABI, exit mapping, budget ownership, and
  transactional zero-budget VM fallback.

It deliberately does **not** claim to implement the compiler lowerer, compact VM,
runtime-driver migration, or product AWFB switch. Those are specified as cuts
3–8 and remain visible in the implementation record rather than hidden behind
stubs or a second compatibility opcode model.
