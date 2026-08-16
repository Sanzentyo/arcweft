# Runtime Dispatch and Caches

This chapter describes the internal substrate behind owner-local event handling
and subsystem caches. It does not define author-facing top-level event or memo
declarations; see [Event Ownership and Caching](../01-language/hooks-and-memoization.md).

## Dispatch plans

View input handlers, line-plan branches, live-stream lifecycle observations,
Activity events, and read-only observation points may lower to shared typed
dispatch records. Each record retains its owner identity, event kind, allowed
effects, source range, and deterministic ordering key.

```rust
pub struct DispatchRecord {
    pub owner: EntityId,
    pub event: DispatchEventKind,
    pub phase: DispatchPhase,
    pub order: StableDispatchOrder,
    pub effects: EffectSet,
    pub program: ProgramRef,
}
```

The lower-level table is produced from already typed owner-local constructs. It
never reparses a target, phase, or policy string and it never grants an effect
that the owner surface does not permit.

## Step integration

`arcweft-core` remains Sans I/O. Host adapters normalize input and external
events, then `Engine::step` visits explicit dispatch points. Handler outputs are
typed values such as `InputDisposition`, semantic actions, commands, line-plan
outcomes, or stream events. Durable state is not mutated by an arbitrary
callback.

Stable order is derived from the owning subsystem, for example:

```text
dispatch phase
  -> committed LayerTree / stream / line-plan order
  -> owner-local explicit priority when supported
  -> stable entity identity
```

The runtime records routed events and observable outputs for replay. Read-only
Agent/debug observation consumes that trace rather than injecting gameplay
mutation.

## Re-entrancy and budgets

Dispatch created by a handler is queued at the owning phase boundary. Runtime
budgets cap nested semantic events, repeated owner/event pairs per tick, and
total handler work. Exceeding a budget produces a structured runtime conflict;
it never silently changes ordering.

## Subsystem caches

Each cache is owned by the subsystem that understands the full key and
invalidation contract:

- the compiler/VM may reuse pure function results;
- the View evaluator retains dependency-indexed values;
- the scheduler joins in-flight work using `TaskKey`;
- render/text/audio/resource owners retain typed artifacts;
- the build pipeline retains semantic-hash-indexed artifacts;
- persistent storage uses explicit schema and version policy.

A cache record includes the owner-specific semantic identity, typed dependency
revisions, lifetime, and value layout. Generic author-provided key expressions
or global invalidation namespaces are not accepted.

```rust
pub struct CacheRecord<K, V> {
    pub key: K,
    pub value: V,
    pub dependencies: DependencyRevisionSet,
    pub owner_revision: SemanticHash,
}
```

Pending, cancelled, and failed work follows the owning scheduler/resource
policy. It is not automatically converted into an ordinary cached value.

## Determinism and replay

Cache hit/miss affects performance only. It does not enter gameplay state hash,
change handler order, or skip observable effects. Recomputing from the same
typed inputs must produce the same value.

Debug tooling may expose dispatch traces and subsystem-specific cache metrics,
but mutation operations remain behind the responsible debug capability and
owner API. There is no universal runtime command for installing a source
callback or invalidating every cache by an author-defined namespace.
