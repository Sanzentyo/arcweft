# Event Ownership and Caching

Arcweft does not expose universal top-level event hooks or author-controlled
memo blocks. Events belong to the subsystem that defines their lifetime, and
caches belong to the subsystem that can validate their keys and invalidation
rules.

There is therefore no canonical top-level `hook` declaration, `memo fn`
declaration, or generic `memo(...) { ... }` expression.

## Event ownership

Author-facing event handling stays next to its owner:

- View input uses a View modifier such as `.on_click { ... }`.
- choice availability is the choice option's typed `enabled` expression.
- choice selection is the option's `select` body.
- line-local signal, mark, timeout, and task reactions use the line plan's
  scoped `on` branches.
- live-source lifecycle events use handlers inside the `source` declaration.
- durable state transitions go through the selected typed state-transition
  function rather than a global callback.
- Agent and debugger observation is read-only trace data.

For example, View input is local to the View node:

```arcw
Button("聞いてみる")
    .agent_target(@choice.opening.listen)
    .on_click {
        action.invoke(@action.choice.select, @choice.opening.listen)
    }
```

Choice availability is expressed on the option itself:

```arcw
option @.listen {
    label = "聞いてみる"
    enabled = state.affection[@character.alice] >= 3

    select {
        goto @flow.alice_intro
    }
}
```

These surfaces may lower to a shared deterministic subscription or dispatch
table. That lower-level representation is not a reason to expose one universal
source declaration: the source language keeps target, event, lifetime, and
allowed effects explicit through the owning construct.

## Deterministic dispatch

Every event owner must define a stable ordering. Runtime dispatch may use phase,
tree order, explicit priority where the owner supports it, and stable entity
identity. Replay records observable owner-local events and their results, not a
global callback registration order.

Presentation and observation phases do not gain durable-state mutation merely
because they share an internal dispatch substrate. State changes remain typed
events or commands processed at the state-transition boundary.

## Caching ownership

Ordinary derived values are ordinary pure functions:

```arcw
fn route_title(route: Ref<Flow>) -> String {
    registry.flow(route).title
}
```

The implementation decides whether such a call is worth optimizing. Authors do
not provide hand-written cache keys, dependency lists, or cache lifetimes on the
function declaration.

Longer-lived reuse is owned by the subsystem with enough information to make it
safe:

| Use | Owner |
|---|---|
| pure function optimization | compiler / VM |
| retained View invalidation | View evaluator |
| in-flight task joining | scheduler and `TaskKey` |
| text layout, image, shader, and audio reuse | corresponding resource subsystem |
| static route or bundle derivation | build / bundle pipeline |
| persistent cached data | typed storage schema and version policy |

Cache hits and misses never change program semantics or replay identity. A
subsystem cache must include all typed identity, semantic-version, dependency,
and lifetime facts required by that subsystem. Borrowed data cannot be retained
beyond its lifetime, and cancelled or pending work is not converted into a
generic cached value.

## Invalidation

Invalidation is derived from the owning subsystem's typed facts:

- View reads create retained dependency edges.
- resource caches include resource revision and environment identity.
- task joining uses the scheduler's typed task key and cancellation policy.
- build caches include semantic hashes and declared build dependencies.

There is no author-facing global cache namespace or global invalidation event.
If a subsystem cannot derive a sound invalidation contract, it must recompute
instead of accepting an ad hoc key.

## Final rules

1. Event behavior is written inside the construct that owns the event.
2. Internal subscription tables remain typed, deterministic, and replayable.
3. Observation paths are read-only; durable mutation uses typed state events.
4. Ordinary computation uses ordinary functions.
5. Caches are implementation strategies owned by the responsible subsystem.
6. Generic author-controlled cache keys, scopes, and invalidation are not part
   of the language.
