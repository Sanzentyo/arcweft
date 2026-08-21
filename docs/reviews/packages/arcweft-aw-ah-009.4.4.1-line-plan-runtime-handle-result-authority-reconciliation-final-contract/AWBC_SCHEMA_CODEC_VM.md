# In-place AWBC schema, codec, verifier, VM, reducer, and snapshots

## 1. Version rule

The final schema is unreleased and replaces the old shape in place:

```rust
pub const AWBC_ABI_VERSION: u32 = 1;
pub const AWBC_CODEC_VERSION: u16 = 1;
```

No version `2`, compatibility alias, optional legacy section, old discriminant
reader, or dual decode path is present.

## 2. Program tables and ids

Add directly to the existing schema:

```rust
awbc_id!(AwbcLineOperationId, "Index into the typed line-operation table.");
awbc_id!(AwbcLineHandleSiteId, "Index into one group's handle-site declarations.");

pub struct AwbcProgram {
    // existing tables ...
    pub line_task_groups: Vec<AwbcLineTaskGroup>,
    pub line_task_nodes: Vec<AwbcLineTaskNode>,
    pub line_operations: Vec<AwbcLineOperation>,
    // remaining existing tables ...
}
```

The canonical table order in the codec places `line_operations` immediately
after `line_task_nodes`.  Encoder, decoder, digest traversal, resource
accounting, table-count validation, and canonicalization all use that one
order.

## 3. Runtime opaque type replacement

```rust
pub enum AwbcRuntimeType {
    // existing variants ...
    Opaque {
        producer: AwbcStringId,
        semantic_identity: [u8; 32],
        admission: RuntimeOpaqueTypeAdmission,
        value_class: RuntimeOpaqueValueClass,
        persistence: RuntimeOpaquePersistence,
        arguments: Vec<AwbcTypeId>,
    },
    // existing variants ...
}
```

Affine/snapshot-only opaque types are forbidden in `AwbcConstant::Opaque`.
They may appear in registers, frames, dialogue result cells, captures, and
live snapshots only.

## 4. Function kind

Final explicit codec tags:

| Tag | `AwbcFunctionKind` |
|---:|---|
| 0 | `Flow` |
| 1 | `PureHelper` |
| 2 | `TraitMethod` |
| 3 | `StreamTransform` |
| 4 | `LineTask` |
| 5 | `Synthetic` |
| 6 | `LineActivation` |

`LineActivation` functions may execute line operations, suspend on their typed
host outcomes, and commit the owning dialogue result.  Ordinary `LineTask`
functions may execute line operations but may not commit unless the group
marks that exact function as its admitted cancellation-result producer.

## 5. Line-operation table

```rust
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum AwbcLineOperation {
    AcquireActor {
        site: AwbcLineHandleSiteId,
        character: CharacterId,
        scope: RuntimeLineHandleScope,
        result_type: AwbcTypeId,
    },
    Schedule {
        site: AwbcLineHandleSiteId,
        child: AwbcLineTaskNodeId,
        capture_types: Vec<AwbcTypeId>,
        result_type: AwbcTypeId,
    },
    ActorLook {
        site: AwbcLineHandleSiteId,
        character: CharacterId,
        actor_type: AwbcTypeId,
        look_type: AwbcTypeId,
        result_type: AwbcTypeId,
    },
    VoiceHandle {
        site: AwbcLineHandleSiteId,
        result_type: AwbcTypeId,
    },
}
```

Codec tags are `0=AcquireActor`, `1=Schedule`, `2=ActorLook`,
`3=VoiceHandle`.  No public-id or callable string is stored.

Instruction arguments are fixed by kind:

| Kind | `args` |
|---|---|
| AcquireActor | empty |
| Schedule | delay, then callback captures in declaration order |
| ActorLook | actor, look, crossfade |
| VoiceHandle | empty |

Every operation has a result and therefore requires `dst`.  An expression
statement still receives a typed temporary register whose ownership remains
with the current runtime scope.

## 6. Opcode mapping

The current reserved holes are used, avoiding renumbering existing opcodes:

| Encoded | Final opcode |
|---:|---|
| `0x1e` | `ExecuteLineOperation` |
| `0x20` | `CommitDialogueResult` |
| `0x86` | `Dialogue` with replaced typed payload |

```rust
pub enum AwbcOpcode {
    // existing opcodes ...
    ExecuteLineOperation,
    Drop,
    CommitDialogueResult,
    AssignRecordField,
    // ...
    Dialogue,
    // ...
}

pub enum AwbcInstruction {
    // existing instructions ...
    ExecuteLineOperation {
        dst: AwbcRegisterId,
        operation: AwbcLineOperationId,
        args: Vec<AwbcRegisterId>,
    },
    CommitDialogueResult {
        source: AwbcRegisterId,
    },
    // existing instructions ...
}
```

## 7. Dialogue and group schema

```rust
pub struct AwbcDialogueResultTarget {
    pub ty: AwbcTypeId,
    pub pattern: AwbcPatternId,
    pub destination: AwbcRegisterId,
}

pub enum AwbcTerminator {
    // existing variants ...
    Dialogue {
        content: AwbcContentUnitId,
        values: Vec<AwbcDialogueValueBinding>,
        line_task_captures: Vec<AwbcRegisterId>,
        result: AwbcDialogueResultTarget,
        resume: AwbcResumePointId,
    },
    // existing variants ...
}

pub struct AwbcLineHandleSite {
    pub source_ordinal: u32,
    pub kind: RuntimeHandleKind,
    pub result_type: AwbcTypeId,
    pub character: Option<CharacterId>,
    pub scheduled_child: Option<AwbcLineTaskNodeId>,
}

pub struct AwbcLineTaskGroup {
    pub captures: Vec<RuntimeLocalDeclarationId>,
    pub activation: AwbcFunctionId,
    pub result_type: AwbcTypeId,
    pub handle_sites: Vec<AwbcLineHandleSite>,
    pub root: AwbcLineTaskNodeId,
    pub nodes: AwbcTableRange,
    pub cancel_handlers: Vec<AwbcLineCancelHandler>,
    pub cleanup_completed: Option<AwbcFunctionId>,
    pub cleanup_cancelled: Option<AwbcFunctionId>,
    pub cleanup_failed: Option<AwbcFunctionId>,
    pub cleanup: AwbcLineCleanupPolicy,
}

pub enum AwbcLineTaskTrigger {
    Immediate,
    Mark(RuntimeDialogueMarkId),
    Scheduled(AwbcLineHandleSiteId),
}
```

The old `DelayNanos` discriminant is deleted from schema, codec, verifier, VM,
and tests.  Authored `at` delay is an evaluated register argument to
`ExecuteLineOperation::Schedule`.

## 8. Binary grammar

All integer fields are little-endian.  IDs and lengths are `u32`.  Optional
fields are `u8 tag` (`0` absent, `1` present) followed by payload when present.
Vectors are `u32 length` followed by elements.

### 8.1 `ExecuteLineOperation` (`0x1e`)

```text
u8   opcode = 0x1e
u32  dst_register
u32  line_operation_id
u32  arg_count
u32[arg_count] arg_registers
```

### 8.2 `CommitDialogueResult` (`0x20`)

```text
u8   opcode = 0x20
u32  source_register
```

### 8.3 `Dialogue` terminator (`0x86`)

```text
u8   opcode = 0x86
u32  content_id
u32  value_count
repeat value_count:
    u32 dialogue_value_slot
    u8  role                 # 0 interpolation, 1 condition
    u32 value_register
u32  capture_count
u32[capture_count] capture_registers
u32  result_type
u32  result_pattern
u32  result_destination
u32  resume_point
```

The old shorter `0x86` payload is not recognized.  Truncation, trailing bytes,
out-of-range ids, or wrong field types are codec/verifier errors.

### 8.4 Line operation entry

```text
u8 kind
case 0 AcquireActor:
    u32 site
    bytes32 character_semantic_id
    u8 scope                  # 0 line
    u32 result_type
case 1 Schedule:
    u32 site
    u32 child_node
    u32 capture_type_count
    u32[capture_type_count] capture_types
    u32 result_type
case 2 ActorLook:
    u32 site
    bytes32 character_semantic_id
    u32 actor_type
    u32 look_type
    u32 result_type
case 3 VoiceHandle:
    u32 site
    u32 result_type
```

Character ids use the repository's existing canonical semantic identity bytes,
not display/public strings.

## 9. Effect-kind deletion and final tags

Delete `RegisterHandle`, `DropHandle`, and `Out`.  The one final codec mapping
is:

| Tag | `AwbcEffectKind` |
|---:|---|
| 0 | Wait |
| 1 | Audio |
| 2 | Call |
| 3 | Log |
| 4 | SignalWrite |
| 5 | MetricWrite |
| 6 | EmitEvent |
| 7 | Return |
| 8 | Goto |
| 9 | Panic |
| 10 | Fail |
| 11 | Bail |
| 12 | Ensure |
| 13 | Assert |
| 14 | Close |
| 15 | Select |
| 16 | Break |
| 17 | Continue |

Old tags are not accepted under a legacy interpretation.

## 10. Lowering

`AwbcFlowLowerer::lower_line_task_group`:

1. reserves the activation function;
2. lowers `activation_ops` into an `AwbcFunctionKind::LineActivation` frame;
3. lowers each typed line operation to the operation table plus opcode `0x1e`;
4. lowers `CommitDialogueResult` to `0x20`;
5. lowers scheduled child functions with exact capture signatures;
6. writes exact result type and handle sites into the group;
7. lowers cancel/cleanup functions;
8. emits diagnostics instead of dynamic types when a required proof is absent.

`FlowOp::Dialogue` allocates an exact result destination register, lowers the
sole target pattern, and emits the replaced `Dialogue` terminator.  It no
longer has a result-less continuation coordinate.

## 11. Verifier obligations

### Type/code

- `ExecuteLineOperation.dst` exists and has the operation's exact result type;
- argument count and register types equal the kind's fixed ABI;
- `CommitDialogueResult.source` equals the owning group's result type;
- commit instruction occurs only in an admitted producer function;
- all completing paths in that function commit exactly once;
- no instruction follows a commit on that path;
- `Dialogue.result.ty` equals content group's result type;
- pattern admits that type and destination register has that type;
- line capabilities never appear as AWBC runtime types or constants;
- affine snapshot-only opaque constants are rejected.

### Topology

- handle sites are dense and source ordinals increasing;
- operation site kind/result/Character equals declaration;
- scheduled operation and child form a one-to-one pair;
- scheduled child trigger points back to the site;
- capture vector length/types equal child signature;
- root and node ranges are in-bounds and acyclic;
- detached children have no affine or result authority;
- every cleanup function belongs to the same group and has the exact capture
  signature.

### Dialogue

- content exists and references the same group used by result verification;
- parent capture registers match group capture declarations;
- result pattern registers belong to the parent frame, not activation frame;
- result destination is initialized only by successful publication;
- resume point owner, layout, and safe-point kind are exact.

## 12. VM and product-step state

```rust
pub enum AwbcDialogueResultState {
    Uncommitted { ty: AwbcTypeId },
    Committed { ty: AwbcTypeId, value: RuntimeValue },
    Published,
}

pub struct AwbcDialogueActivationFrame {
    pub activation: DialogueActivationId,
    pub function: AwbcFunctionId,
    pub block: AwbcBlockId,
    pub registers: Box<[Option<RuntimeValue>]>,
    pub scopes: Box<[AwbcScopeId]>,
}

pub struct AwbcDialogueRuntimeState {
    pub phase: DialogueRuntimePhase,
    pub activation_frame: Option<AwbcDialogueActivationFrame>,
    pub result: AwbcDialogueResultState,
    pub ledger: RuntimeLineHandleLedger,
    pub scheduled: Box<[RuntimeScheduledLineTask]>,
    pub reducer: LineTaskLiveSnapshot,
}
```

The VM delegates line topology transitions to the existing common reducer.
When the reducer yields a work tag, the VM launches the mapped AWBC function
with that scheduled capture vector.  The structured executor launches the
mapped `FlowOp` body.  Neither executor modifies reducer state through a
parallel implementation.

## 13. Suspension and snapshot encoding

An activation function can suspend on dialogue preparation, lazy voice start,
stage acquire/look outcome, or deterministic budget yield.  Its complete
frame, block/resume point, scopes, pending command id, result state, ledger,
schedule state, and reducer snapshot are persisted.

Snapshot version fields stay `1`.  Replaced snapshot structs have one codec;
old fields such as string registered handles or string line-out payloads are
removed and not defaulted.
