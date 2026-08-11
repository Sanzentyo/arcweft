# Path, lifetime, pathless-pattern, call, and Thread contract

## Path resolution

A `HirPath` retains `ImplicitCrate`, `Crate`, `SelfModule`, or `Super { depth }` and non-empty typed segments. Resolution consumes `HirPathResolutionContext { snapshot, owner_scope }` and never splits a dotted label.

- `ImplicitCrate`: consult the immutable import-alias table for the first segment; a unique alias substitutes its typed publication identity. If no alias exists, begin at crate root. Ambiguity poisons; it does not fall back.
- `Crate`: begin at crate root and do not treat the first segment as an alias root.
- `SelfModule`: begin at the owner module.
- `Super { depth }`: walk exactly `depth` parents; depth zero canonicalizes to SelfModule; escaping the crate is typed failure.

Expression, Pattern, and Type owners use their respective typed source-query variant. Path roots and segments never use `expr_source_site` after the public switch.

## Pathless variant patterns

`HirVariantPatternHead` is `Qualified(HirPath)` or `Unqualified(HirUnqualifiedVariantForm)`. No empty-path state exists.

| Source | HIR head | Name | Resolver behavior |
|---|---|---|---|
| `.Foo` | `Unqualified(DotShorthand)` | `Foo` | expected enum type only |
| `Some` / `None` | `Unqualified(BareExpectedType)` | authored name | expected enum type only; no Option hard-code |
| `Ok` / `Err` | `Unqualified(BareExpectedType)` | authored name | expected enum type only; no Result hard-code |
| `crate.game.State.Ready` | `Qualified(HirPath)` | terminal name | root-preserving qualified resolution |

The optional payload is a same-module PatternId whose kind is Tuple or Record. It inherits scope. A missing/foreign/wrong-kind payload retains Variant plus poison. Unknown expected type or unknown variant is a sema error over clean HIR. A fieldless valid variant has no payload role; querying it returns `AbsentOptional`. Its negative test uses an inapplicable role or wrong-kind payload ID, never an impossible field mutation.

## Type regions versus registry lifetime

`HirTypeRegion` appears only in TypeId-owned reference types. Named regions retain `HirRegionName`. An elided region carries:

```text
SyntheticOwner::Type(reference_type_id)
SyntheticRole::ElidedRegion
ordinal 0
HirTypeSourceRole::Region(ElisionInsertion)
```

The insertion anchor comes directly from typed syntax and is revision-bound. `SyntheticKey::try_new` rejects every other owner kind or ordinal without probing a raw slot.

`HirLifetimeRegistryPath` appears only in runtime registry operations. Scopes are Frame, Tick, Cue, Line, Scene, Flow, Session, Global, Persistent, or Named. `LifetimePath` expressions are Read; statement owners use Write, MoveOut, Drop, or Expose. Optional non-read access is invalid.

## Ordinary and associated calls

`HirCallExpr` retains one `HirCallCallee` and ordered arguments. A value callee owns an ExprId. An associated type callee owns a TypeId receiver root, member HirName, and `DotFallback` or `ExplicitDoubleColon` syntax category.

Dot-member calls check the target as a value first; any value-space result, including a value-space error, owns the call. Nominal fallback occurs only on definitive value-space absence. Explicit `Type::member` is nominal-only. The TypeId tree retains generic parameters, aliases, module/project identity, and source components. It projects directly to the existing nominal report, `ResolvedAssociatedTypeReceiver`, and single shared `CallCallee::AssociatedType`. Bare `Vec` fails generic arity before candidate admission.

Environment methods precede capacity; capacity precedes associated traits. Untyped/data-last fallback is ineligible. No second resolver, Capacity-only HIR, display parser, or argument replay pass is permitted.

## Thread

Thread ownership remains unchanged: optional HirName, Attached/Detached mode, one child ScopeId, and ordered exhaustive `HirThreadFlowItem`s. There is no block ExprId and no tail. Empty authored body is valid Unit; missing required body poisons. Attached threads join/cancel with the parent set; detached threads require owned/static captures. Poisoned threads do not reach runtime-plan.
