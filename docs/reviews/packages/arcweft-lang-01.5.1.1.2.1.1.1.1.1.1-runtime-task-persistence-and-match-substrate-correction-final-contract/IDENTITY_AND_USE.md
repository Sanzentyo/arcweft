# Retained identity transcripts and NeedHandle use contract

## 1. Nonduplication

| Semantic field | Sole owner |
|---|---|
| accepted producer contract | `NeedProducerContractDigest` |
| static executable/task semantics | `TaskPlanSemanticDigest` |
| producer family/site/payload/actual arguments | `NeedProducerInstanceKey` |
| launch policy and ordinal | `NeedId` |
| active generation and producer/policy | `TaskKey` |
| launch identity | `TaskId` |
| runtime value argument identity | `RuntimeValueDigest` |
| View program identity | `ViewProgramId` |
| accepted View build/revision | `AcceptedViewProgramRevision` |
| Match semantics | `CheckedMatchSemanticDigest` |
| retained ownership evidence | `OwnershipEvidenceDigest` |
| generation-bound use | `TaskCorrelation`/scheduler active generation |

No transcript copies a field owned by another row merely to “strengthen” the
hash.

## 2. Frozen task/Need derivation

```text
NeedProducerInstanceKey =
  BLAKE3("arcweft.need.producer-instance.v1\0"
       || NeedProducerFamily
       || NeedProducerContractDigest
       || TaskPlanSemanticDigest
       || producer_site:u32-le
       || RuntimeTypeSemanticDigest
       || RuntimeValueDigest)

NeedId =
  BLAKE3("arcweft.need.id.v1\0"
       || NeedProducerInstanceKey
       || TaskPolicy
       || TaskLaunchOrdinal)

TaskKey =
  BLAKE3("arcweft.task.key.v1\0"
       || GenerationId:u64-le
       || NeedProducerInstanceKey
       || TaskPolicy)

TaskId =
  BLAKE3("arcweft.task.id.v1\0"
       || TaskKey
       || TaskLaunchOrdinal)
```

Join ordinal is zero. AlwaysStart ordinals are positive and begin at one.
Fixed outputs reject all-zero without rehash/salt/retry. Semantic digests accept
all outputs.

## 3. RuntimeValue NeedHandle

```text
RuntimeValue tag = 20
payload = NeedId:digest32
```

No generation, TaskKey, TaskId, producer spec, payload type, origin, debug label
or accepted revision is added.

`RuntimeNeedHandle` stores complete structural evidence but implements
`Eq`/`Hash`/`Ord` using NeedId only. This makes canonical bytes and Rust value
semantics agree.

## 4. Structural constructor

`RuntimeNeedHandle::try_new` validates:

1. `correlation.producer == producer.instance_key()`;
2. policy/ordinal relation;
3. NeedId, TaskKey and TaskId rederive exactly;
4. `outcome.payload_type == producer.payload_type`;
5. family-specific handle policy;
6. Join reusable handles use ordinal zero;
7. an AlwaysStart handle is the output of an accepted launch, never a
   pre-launch fabricated value;
8. origin/debug evidence is bounded and cannot affect any comparison.

A snapshot restore repeats the same constructor. It never reconstructs the
handle through field assignment.

## 5. Ordinary use

Await and timeout perform:

```rust
handle.validate_structure()?;
handle.validate_use(scheduler.active_generation())?;
```

before Need lookup, observer allocation or task staging.

`validate_use` compares only correlation generation to the scheduler active
generation after structural validation. Thus a stale handle:

- remains equal/hash-equal/order-equal to the same NeedId value;
- has the same canonical bytes;
- fails use with `RuntimeNeedUseError::StaleGeneration`;
- causes no mutation.

## 6. Replacement rebind

A `ValidatedReplacementMapping` proves:

- old/current program and revisions are accepted;
- checked Match, View admission, Need admission, ownership, resource,
  producer contract, payload type, plan and argument digest agree as required;
- old handle structure is valid;
- mapping preserves producer instance, policy, ordinal and NeedId;
- new generation is the selected replacement generation.

`rebind_for_replacement` constructs a new correlation with the preserved
producer/policy/ordinal and rederived new TaskKey/TaskId. It does not mutate the
old handle and does not change value identity.

## 7. Required tests

| Case | Value equality | Use |
|---|---|---|
| same NeedId, different diagnostic/spec labels | equal | accepted if both structures valid/current |
| same NeedId, stale generation | equal | rejected before mutation |
| valid explicit replacement rebind | equal old/new as value | new accepted in new generation |
| tampered producer/spec/correlation | unspecified because constructor fails | rejected at construction/restore |
| same producer but different AlwaysStart ordinal | not equal | independent Need |
