# Persistence, replay, restoration, and replacement

## Selector result lifetime

A selector result is an ordinary owning `RuntimeValue::Variant` with an owning tuple. It may be present temporarily across the caller boundary and is immediately decoded and transactionally installed. Public View state stores only installed locals and selected body state; it does not persist `DecodedViewMatchSelection`.

When a fiber/session snapshot happens before decode, the existing recursive `AwbcRuntimeValueSnapshot::Variant`/`Tuple` rows persist the value. Restore re-verifies the selector's active result type and bundle binding. A stale or malformed result aborts the whole session restore.

## Need handle snapshot

The new dedicated snapshot row recursively projects `RuntimeNeedHandle`. Projection fails for an invalid NeedId, unknown producer contract, payload digest mismatch, argument ownership violation, excessive depth/count/bytes, unsupported function value, or non-snapshot opaque value. It never falls back to String serialization.

Restore has two phases:

1. Decode DTOs into a staging graph with structural and exact-limit checks.
2. Validate every handle against the target active bundle, resource registry digest, producer binding, task plan, and replacement policy; only then swap the staging graph into the live session.

No partial fiber, local, observer, or journal mutation is visible on failure.

## Journal replay

The journal key remains `(GenerationId, NeedId)`. Events retain the existing logical epoch and sequence ordering. Replay requires monotonically increasing publication cursors, exact state transition legality, and payload agreement with the verified handle's `NeedHandle<T>` type.

A duplicated or regressed event is rejected. Ready payloads are recursively checked against `T`; fallible producers use ordinary `Result<T,E>` nested in Ready. Cancellation remains producer-owned.

## Replacement

At quiescence, replacement classifies each live producer:

- **carry** only when old and new producer contract digest, payload type digest, canonical argument digest, resource registry digest, and snapshot/replay version 1 are equal, and the new bundle has one verified producer binding;
- **retire/cancel** otherwise.

A carried producer receives the new active `GenerationId` coordinate without changing `NeedId`; its journal transfer is a single replacement transaction. A task-plan table index is never trusted across generations: the new binding resolves its current plan after contract equality.

Observer subscriptions are rejoined only after producer transfer succeeds. A failed replacement leaves the old generation active. No source text, function name, task-plan need string, or compatibility map reconstructs an identity.

## Digest tamper behavior

Tampering with selector result owner/case/payload, NeedId, producer contract, payload type, arguments, resource registry digest, bundle section, task plan, or manifest is a hard error. There is no warning-only path and no value sanitization.
