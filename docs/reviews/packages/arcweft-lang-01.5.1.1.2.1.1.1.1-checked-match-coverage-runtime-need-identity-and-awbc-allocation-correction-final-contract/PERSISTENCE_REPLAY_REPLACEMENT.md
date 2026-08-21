# Persistence, replay, and hot replacement

## Snapshot schemas

All version fields remain 1. Old String fields are removed rather than defaulted.
The final AwaitMany in-flight row is:

```rust
pub struct FiberAwaitManyInFlight {
    pub index: u32,
    pub base_need_id: NeedId,
    pub task_id: TaskId,
    pub task_key: TaskKey,
    pub need_id: NeedId,
    pub producer: NeedProducerContractDigest,
    pub generation: GenerationId,
}
```

A task snapshot additionally stores plan digest, payload type digest, launch
policy and ordinal, and canonical terminal/publication state. Timeout snapshots
store both source and output NeedId plus timeout contract/site/limit digest.
View producer snapshots store product coordinate, checked Match digest,
producer contract, resource digest, and payload type digest.

## Restore transaction

Restore decodes into final typed values under budgets, rejects zero IDs, checks
all schema markers equal 1, recomputes plan/Need/Task/AwaitMany/timeout
derivations, validates bundle/resource/type/producer digests, and checks terminal
publication consistency. Only after every row succeeds does runtime install the
new generation and observer indexes. Any failure drops the candidate and leaves
current state unchanged.

No malformed identity is repaired by parsing a display string, appending an
index, consulting source text, or reconstructing a producer from a public ID.

## Replay

Journal order owns AlwaysStart launch ordinals. Replay therefore reconstructs
exact TaskKey and TaskId. JoinSameKey observer additions do not allocate another
launch. Equal terminal repeats are idempotent. A different terminal value,
state, payload type, or producer contract for one logical NeedId is a hard
correlation error.

## Replacement

Compatible replacement is explicit. A mapping row names old/new program
revision and stable Match site and carries old/new semantic digest, producer
contract, payload type digest, plan digest, and resource digest. All must agree.
Runtime-driver binds the new active generation only after decode/install
succeeds. Incompatible rows cancel old producers and construct new ones.
Installation failure rolls back without changing the authority generation.
