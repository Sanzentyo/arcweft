# Rust-shaped live schemas

`schemas/final_contract.rs` is the complete normative Rust-shaped excerpt. It
is intentionally not a standalone crate: names already owned by Arcweft refer
to their current modules, and implementation must edit those owners directly.

## Owner map

| Owner | Types/behavior |
|---|---|
| `arcweft_core::task` | `RuntimeNeedHandle`, its state and inherent constructors; AwaitMany request/template; observer IDs; Host operation catalog; all adapter envelopes; `TaskLaunchAdapter` |
| `arcweft_core::value` | final `RuntimeValue::NeedHandle` arm, canonical NeedId-only value transcript, exact ownership constructors |
| `arcweft_core::value::awbc_save` | in-place `AwbcRuntimeValueSnapshot` evolution and all nested snapshot rows |
| `arcweft_runtime_scheduler` | `EnsureBatchPlan`, batch/cancel deltas, atomic apply/rollback and await behavior |
| `arcweft_lang_sema::final_analysis` | `CheckedExpressionChildRole`, semantic tag methods, callable joins and transcript builder |
| `arcweft_compiler` | compiler-local `HirSnapshotId + ExprId` Match cache only |

No extension trait is introduced for an Arcweft-owned enum merely to avoid
adding an inherent method. `TaskLaunchAdapter` is a legitimate protocol trait
for external/upward implementers.

## Complete `TaskSpec`

The retained final shape is:

```rust
pub struct TaskSpec {
    producer: NeedProducerSpec,
    class: TaskClass,
    priority: TaskPriority,
    cancel_scope: CancelScopeId,
    policy: TaskPolicy,
    outcome: TaskOutcomeContract,
    execution: TaskExecution,
    debug: TaskDebugMetadata,
}

pub enum TaskExecution {
    Host(HostTaskRequest),
    Runtime(RuntimeTaskRequest),
}
```

There is one execution field. AwaitMany and Timeout remain explicit Runtime
request variants. Every reusable-handle snapshot stores the complete version-1
projection of all eight fields; no request digest substitutes for the request.

## Sealed accepted-launch proof

```rust
pub(crate) struct AcceptedTaskLaunch<'a> {
    journal: &'a TaskJournalRow,
    need: &'a RuntimeNeedCell,
}
```

Only the scheduler creates this value after atomic application. Its constructor
is private to the scheduler module and verifies that the journal and Need rows
share the complete `TaskCorrelation`, producer and outcome. Passing raw fields
to `try_from_accepted_launch` is not an alternative API.

## Snapshot constructor result

```rust
impl AwbcRuntimeValueSnapshot {
    pub fn from_runtime_value(
        value: &RuntimeValue,
        authority: &RuntimeSnapshotAuthorityV1,
        limits: RuntimeSnapshotLimitsV1,
    ) -> Result<Self, AwbcRuntimeValueSnapshotError>;

    pub fn into_runtime_value(
        self,
        authority: &RuntimeSnapshotAuthorityV1,
        limits: RuntimeSnapshotLimitsV1,
    ) -> Result<RuntimeValue, AwbcRuntimeValueSnapshotError>;
}
```

`RuntimeSnapshotAuthorityV1` contains the exact generation-pinned AWBC program,
task journal, host operation catalog, nominal/layout catalogs and opaque
producer catalog needed by the accepted rows. It is not serialized by
reference name and is not a compatibility fallback.

## Typed rejection inventory

The projection errors include, at minimum:

```rust
pub enum AwbcRuntimeValueSnapshotError {
    WorkLimit,
    UnknownTag,
    TrailingBytes,
    InvalidLength,
    InvalidFieldIdentity,
    DuplicateField,
    InvalidNominalJoin,
    InvalidOpaqueJoin,
    InvalidVariantJoin,
    InvalidNeedHandle,
    InvalidTaskSpec,
    MissingAcceptedLaunch,
    UnrebindableStructuredFunction,
    MissingAwbcExecutableAuthority,
}
```

Every error occurs before publication. Encoding is performed into a private
staging sink and is returned only after success, so a rejected Structured
function cannot leave a partial snapshot.
