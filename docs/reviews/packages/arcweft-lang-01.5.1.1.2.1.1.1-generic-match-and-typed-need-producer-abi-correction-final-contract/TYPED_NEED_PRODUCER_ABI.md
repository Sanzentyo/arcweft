# Typed Need producer ABI

## 1. Sole type/value projection

The complete projection is direct and exhaustive:

```text
TypeKind::Need(T)
 -> RuntimeTypeShape::Need(T)
 -> RuntimeCheckedType::Need(T)
 -> AwbcRuntimeType::NeedHandle { payload: T }
 -> RuntimeValue::NeedHandle(RuntimeNeedHandle)
```

`RuntimeNormalizedType::checked_type_at`, `AwbcProgram::checked_type`, and `awbc_lower::pattern::intern_runtime_type` gain the direct `Need` branches in the original Arcweft-owned enum implementations. `RuntimeUnsupportedTypeShape::Need` is deleted. A verified Need never projects to Dynamic.

The handle owns fixed 32-byte NeedId, producer contract digest, payload runtime-type digest, and immutable source-ordered producer arguments. It is not String/TaskHandle, not presentable, not a constant-pool value, and cannot be fabricated through generic serde/host text.

## 2. Deterministic NeedId

`AwbcInstruction::MakeNeedHandle { dst, plan, site, args }` computes:

```text
argument_digest = canonical RuntimeValue digest(args in source order)
NeedId = BLAKE3("arcweft-need-id-v1\0" || producer_contract || argument_digest)
```

The producer contract commits to producer function identity and `site`. Equal contract/arguments therefore join; different contract or canonical argument digest does not. Generation is intentionally the first coordinate of the journal key, not handle bytes. Mount, observer, task-plan table index, source spelling, and arbitrary runtime token are excluded.

## 3. Producer contract digest

The verifier hashes schema marker 1, producer function semantic digest, `MakeNeedHandle.site`, canonical task-plan digest after deletion of its old need string, producer parameter type digests, handle argument type digests, payload type digest, and policy tag. The bundle stores the digest and exact function/result/payload/task-plan/argument rows. The handle stores digest/payload digest/arguments. There is no source resolver and no second endpoint table.

## 4. Function/task-plan relationship

A verified producer function has result exactly `NeedHandle<T>`, is Synthetic/deterministic/may-allocate, does not suspend, and carries `NEED_PRODUCER`. Every reachable return is dataflow-derived from one `MakeNeedHandle` using the same plan/site. The function contains no StartTask, Await, host call, spawn, or dynamic target.

The task plan's payload type equals T. Its signature parameter list equals instruction arguments exactly. Host-argument metadata has the same count/order and named/spread constraints. View unary Need requires `JoinSameKey` and `many == None`.

`AwbcTaskPlan.need_id` is deleted. Need identity is per verified contract/arguments, not a static task-plan String.

## 5. Construction and verified extraction

The VM constructs the dedicated value only while executing verified `MakeNeedHandle`; construction emits no start. Before returning it validates argument count/type/order, recursive snapshot-clone ownership, canonical argument digest, payload digest, and exact contract.

`extract_need_handle` is the only runtime-driver transition from producer result to reactive journal. It requires:

1. `RuntimeValue::NeedHandle`, never String;
2. verified product/bundle generation equal to active `ProgramGeneration`;
3. one bundle producer binding for the handle contract;
4. binding function/result/task-plan indices in range;
5. result row `NeedHandle { payload }`, exact payload type/digest;
6. task-plan/contract recomputation equality;
7. exact argument count/order/type and recursive snapshot-clone admission;
8. NeedId recomputation from contract and canonical arguments;
9. resource registry digest equality; and
10. replacement/replay policy.

Only then does it create private `VerifiedNeedHandle { generation, handle, binding, plan }`. Journal creation, observer publication, and start-intent construction accept this verified owner, never raw RuntimeValue/NeedId.

## 6. Lazy start and journal

First observation looks up/creates `(active GenerationId, NeedId)` and begins `NotStarted`. The immutable descriptor is the verified binding plus handle arguments. Observing NotStarted emits one transactional start intent under the verified task key and JoinSameKey; duplicate observers join. Construction itself emits no task.

Progress, Ready(T), and Cancelled remain parent-contract states. Result/Option remain ordinary values nested inside Ready. Cancellation remains producer-owned.

## 7. String and TaskHandle split

The current grouped type check is split:

- String accepts only `RuntimeValue::String`;
- TaskHandle deliberately retains its current nonempty String carrier in this narrow correction; and
- NeedHandle accepts only `RuntimeValue::NeedHandle` with exact payload digest.

Full plan/contract/argument checks occur at construction/extraction. Affine, borrowed, unique, must-drop, frame-local, non-cloneable, and non-snapshot arguments fail before publication.

## 8. Codec/snapshot/restore/replay/replacement

Runtime type tag 19 now requires a payload AwbcTypeId. Old payloadless bytes are invalid under strict ABI 1; no compatibility reader exists.

`AwbcRuntimeValueSnapshot` gains a dedicated NeedHandle DTO with version 1, fixed NeedId, producer/payload digests, and recursive argument snapshots. Restore recomputes argument digest/NeedId and validates active product bindings before commit.

Replay uses existing journal ordering/cursors. Replacement carries only when contract, payload type, canonical argument digest, resource registry digest, and replacement policy match; the new active generation supplies GenerationId. Table indices are re-resolved from the new verified binding. Mismatch retires/cancels and never reinterprets stale IDs.

## 9. Atomic deletion inventory

The final switch removes payloadless NeedHandle type/tag, `AwbcTaskPlan.need_id`, `NamedTaskSpec.need_id`, NeedHandle-through-String type matching, `await_target` String-to-NeedId conversion, unsupported/untyped Need projection, obsolete verifier/codec/bundle/save rows, old fixtures/generated schemas, and old View Await consumers. No alias, migration reader, dual carrier, fallback, or source reconstruction remains.
