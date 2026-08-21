# Runtime Need and task identity contract

## Fixed owners

```rust
NeedId([u8; 32])
TaskKey([u8; 32])
TaskId([u8; 32])
NeedProducerContractDigest([u8; 32])
TaskPlanSemanticDigest([u8; 32])
RuntimeValueDigest([u8; 32])
```

All-zero bytes are reserved and reject at codec/restore boundaries. Constructors
are crate-owned. Lowercase hexadecimal is presentation only; no runtime FromStr,
string conversion constructor, suffix parser, or display lookup exists.

## Digest scalar grammar

Identity transcripts are distinct from the AWBC wire grammar. They use BLAKE3
256, an exact ASCII domain including its trailing NUL, u8 tags, u32-le/u64-le
integers, raw 32-byte digests, and variable bytes as u32-le length followed by
bytes. No debug formatting, source spelling, platform integer, or map iteration
order enters a transcript.

| Family | Domain | Ordered inputs after domain | Result |
|---|---|---|---|
| ordinary StartTask/HostTask | `arcweft.need.host-task.v1\0` | contract|plan_digest|site:u32le|arguments_digest | NeedId |
| verified View producer | `arcweft.need.view-producer.v1\0` | contract|checked_match_digest|view_program_digest|revision:u32le|site:u32le|arguments_digest | NeedId |
| line task | `arcweft.need.line-task.v1\0` | contract|plan_digest|line_group_digest|site:u32le|arguments_digest | NeedId |
| direct Await | `none` | RuntimeNeedHandle.need | same NeedId |
| AwaitMany base | `arcweft.need.await-many-base.v1\0` | parent_contract|plan_digest|site:u32le|count:u32le|ordered_source_digest | base NeedId |
| AwaitMany child | `arcweft.need.await-many-child.v1\0` | base_need_id|index:u32le|item_digest | child NeedId |
| NeedTimeout output | `arcweft.need.timeout.v1\0` | timeout_contract|producer_site:u32le|source_need_id|limit_value_digest | output NeedId |
| JoinSameKey task | `arcweft.task.key.v1\0` | generation:u64le|NeedId|policy=0|launch=0 | TaskKey then TaskId |
| AlwaysStart task | `arcweft.task.key.v1\0` | generation:u64le|NeedId|policy=1|launch_ordinal:u64le | TaskKey then TaskId |

The exact domains are:

```text
producer_contract = arcweft.need.producer-contract.v1\0
task_plan = arcweft.task.plan.v1\0
host_task = arcweft.need.host-task.v1\0
view_producer = arcweft.need.view-producer.v1\0
line_task = arcweft.need.line-task.v1\0
await_many_base = arcweft.need.await-many-base.v1\0
await_many_child = arcweft.need.await-many-child.v1\0
timeout = arcweft.need.timeout.v1\0
task_key = arcweft.task.key.v1\0
task_id = arcweft.task.id.v1\0
runtime_value = arcweft.runtime-value.v1\0
```

## AwbcTaskPlan replacement

```rust
pub struct AwbcTaskPlan {
    pub public_id: AwbcStringId,             // display/source map only
    pub producer: AwbcTaskProducer,
    pub capability: AwbcStringId,
    pub operation: AwbcStringId,
    pub signature: AwbcSignatureId,
    pub class: AwbcTaskClass,
    pub priority: i32,
    pub cancel_scope: AwbcStringId,
    pub policy: AwbcTaskPolicy,
    pub payload_type: AwbcTypeId,
    pub arguments: Vec<AwbcHostArgument>,
    pub many: Option<AwbcAwaitManyPolicy>,
}

pub struct AwbcTaskProducer {
    pub family: AwbcTaskProducerFamily,      // HostTask | ViewNeed | LineTask
    pub contract: NeedProducerContractDigest,
    pub site: u32,
    pub plan_digest: TaskPlanSemanticDigest,
}
```

The verifier recomputes `plan_digest` from producer family, typed capability and
operation bytes, signature digest, task class, priority, canonical cancel-scope
bytes, policy, payload type digest, argument contract, and `many` policy.
`public_id` is excluded because it is presentation. A mismatch rejects the
program. There is no `need_id`, optional string, or parallel legacy row.

## Task relation

`NeedId` names the logical outcome. The scheduler's active Need key is
`(GenerationId, NeedId)`. `JoinSameKey` derives a TaskKey with policy tag 0 and
launch ordinal 0; duplicate observers receive the same TaskId and publication.
`AlwaysStart` uses policy tag 1 and a monotonically journaled u64 launch ordinal,
therefore each launch has a distinct TaskKey/TaskId while retaining the same
logical NeedId when producer inputs are equal.

`TaskId = BLAKE3(task-id-domain || TaskKey || launch_ordinal:u64-le)`.
Task events always carry TaskId, TaskKey, NeedId, producer contract, and active
generation. Nothing infers NeedId from TaskId.

## Await, AwaitMany, timeout

Direct Await accepts only `RuntimeValue::NeedHandle`; it reads the embedded
NeedId. AwaitMany first derives one base identity from the ordered source list,
then each child identity from base + source-order u32 index + item digest. Equal
items at different indexes remain different. Snapshot restore recomputes both
base and child identities and validates TaskKey/TaskId.

Timeout stores source and output Need identities separately. The output commits
to the typed timeout contract, producer site, source NeedId, and exact limit
value digest. The source handle remains unchanged and cannot be recovered by
parsing the output ID.

## Fanout, replay, replacement

Equal JoinSameKey inputs create one task and many observers. Equal duplicate
terminal publications are idempotent; a different terminal value for the same
NeedId/contract is a deterministic correlation conflict. AwaitMany fanout is
index-stable. Replay restores the journaled launch ordinal and exact typed IDs.

A replacement may preserve state only when the explicit revision mapping and
all semantic/producer/type/resource/plan digests agree. Generation is never
silently omitted: active keys change generation even when logical NeedId stays
equal. Otherwise runtime cancels the old generation and constructs the new one
transactionally.
