# Final contract — entity-family applied type projection correction

## 1. Contract status and baseline

This document is normative. “Must”, “must not”, “only”, and “exactly” are
implementation requirements.

The contract is fixed to `Sanzentyo/arcweft` `main` at commit
`4fd6331dc342d30a7f4ac7774852b60801866ef7`. The uploaded request is byte-for-byte identical to repository
blob `5445ff2e48c47a4cb2455b56fb5348784038beb6`. Production code is not changed by this package.

## 2. Canonical authored syntax

`Ref<Entity>` remains canonical source syntax.

The source grammar continues to represent it with the existing generic
`TypeRef` form. No dedicated syntax node, compatibility alias, alternate
spelling, or migration diagnostic is added. `Ref` requires exactly one
argument. The argument is an authored entity-family atom, not an ordinary
`TypeKind` and not a string.

For authored source, the entity-family inventory is the fixed variants of
`EntityKind` listed by `EntityKind::AUTHORED_FAMILIES`. `EntityKind::Other` is
not source-authorable through this contract. A future dynamic family registry
requires a separate contract.

The exact surface result is:

```rust
TypeKind::entity_ref(entity_kind)
// exactly:
TypeKind::Ref(EntityType::new(entity_kind, None))
```

The optional `EntityType::value` payload is always `None` for authored
`Ref<Entity>`. `Ref<Entity, Value>` is wrong arity and is not a payload syntax.

## 3. Single owner model

The selected owner model is **a correction of the existing closed
`BuiltinTypeConstructor` table**.

1. Add `BuiltinTypeConstructor::Ref`.
2. Add inherent typed behavior to `BuiltinTypeConstructor` for path selection,
   per-argument expectations, and entity-family projection.
3. Define the closed contextual subset as
   `BuiltinTypeConstructor::ENTITY_FAMILY_PROJECTIONS =
   &[Ref, Speaker, SpeakerPreset]`.
4. Keep `TypeNameResolution::Builtin(BuiltinTypeConstructor)` and
   `TypeArityTarget::Builtin(BuiltinTypeConstructor)` as the owner facts.
5. Keep child evidence as `TypeNameResolution::EntityFamily(EntityKind)`.
6. Keep `AcceptedNominalSemantics` unchanged.

There is no new accepted-record semantic, no parallel contextual registry, and
no second name resolver. The owner enum already belongs to Arcweft; missing
behavior is added to that enum’s inherent implementation rather than encoded in
free spelling helpers, extension traits, or consumer branches.

## 4. Typed argument model correction

The current resolver can represent a successful child as an ordinary type,
const integer, or entity family. A `Ref<3>` child has `const_int` evidence but
no `TypeKind`; the current entity-family special branch therefore cannot emit
the same authoritative wrong-kind diagnostic used for an ordinary type. That
is a concrete repository flaw exposed by this correction.

Add the closed `TypeArgumentKind` carrier and change the `actual` field of
`WrongArgumentKind` to that carrier. The diagnostic code remains
`sema.nominal.wrong_kind`; the diagnostic and poison substrate is otherwise
unchanged. This correction prevents a const argument from degrading into an
unexplained upstream poison.

## 5. Resolution algorithm

The single public operation remains `resolve_type_ref`.

### 5.1 Constructor selection

For an implicit-crate, one-segment type path, call
`BuiltinTypeConstructor::from_type_path`. Builtin selection occurs at the same
language-owned priority it has now: scoped generics/`Self`, then builtin,
then project/external, then accepted exact/open resolution. Therefore a direct
`Ref` cannot be shadowed by project, external, exact accepted, or open entries.
Qualified `pkg::Ref` is not the language constructor and follows normal
qualified-name resolution.

The resolver-local free `builtin(path)` spelling table is deleted. All users
call the enum’s inherent selector.

### 5.2 Child traversal

Every supplied generic child is still visited before arity validation, as in
the current recursive resolver. For each supplied argument index:

1. Ask the selected builtin’s inherent `argument_expectation(index)`.
2. If it is `EntityFamily`, invoke the existing entity-family node path within
   the same recursive traversal.
3. Otherwise resolve the child normally.
4. Extra arguments whose index is outside the declared arity are resolved
   normally, then the outer constructor emits wrong arity.

The unary-chain optimization stores a typed
`Option<TypeArgumentExpectation>` in each frame. It must not store a boolean
or rediscover `Ref`, `Speaker`, or `SpeakerPreset` by spelling.

### 5.3 Entity-family classification

A child is classified as an entity family only when all conditions hold:

- the current constructor argument expectation is `EntityFamily`;
- the child node itself is `TypeRef::Path`;
- the path is implicit-crate and exactly one segment;
- that segment is in `EntityKind::AUTHORED_FAMILIES`.

Classification does not recurse through a generic, alias, project nominal,
external export, accepted record, open rule, generic parameter, or display
string. Thus `Ref<Option<String>>`, `Ref<Speaker<Character>>`, `Ref<T>`, and
`Ref<ProjectType>` are not entity families.

A project declaration named `Character` may exist and resolves as a project
nominal outside an entity-family slot. Inside `Ref<Character>`, `Character` is
the canonical entity-family atom by contextual ownership.

### 5.4 Projection

After exact arity validation, `apply_builtin` calls
`BuiltinTypeConstructor::project_entity_family` for the closed projection
subset. The method is exhaustive for `Ref`, `Speaker`, and `SpeakerPreset` and
returns `None` for every other builtin.

- entity-family child → project to the constructor-specific `TypeKind`;
- successful non-family child → one wrong-kind diagnostic and poison;
- already-poisoned child → propagate its poison, emit no duplicate wrong-kind;
- detached-unavailable child → propagate detached status, emit no authoritative
  accepted-world diagnostic.

## 6. Exact node facts

For valid `Ref<Character>`:

| Node | Recovered | `TypeNameResolution` | Source |
|---|---|---|---|
| root `Ref<Character>` | `Some(TypeKind::entity_ref(Character))` | `Builtin(Ref)` | whole root; head/terminal `Ref` |
| argument `Character` | `None` | `EntityFamily(Character)` | whole/head/terminal `Character` |

For `Ref<String>`:

| Node | Recovered | `TypeNameResolution` |
|---|---|---|
| root | `Some(TypeKind::Error(p))` | `Builtin(Ref)` |
| argument | `Some(TypeKind::String)` | `Failed(WrongArgumentKind { target: Builtin(Ref), argument: 0, expected: EntityFamily, actual: Type(String) })` |

The child’s recovered type is retained when its outcome is replaced. This
preserves typed evidence for diagnostics while preventing it from becoming a
valid project-nominal tooling edge.

For unknown, ambiguous, inaccessible, syntax-poisoned, or detached arguments,
the child keeps its existing corresponding outcome. The outer node keeps
`Builtin(Ref)` because constructor ownership succeeded; its recovered product
is the propagated `TypeKind::Error`.

For wrong arity, the outer node is
`Failed(WrongArity { target: Builtin(Ref), expected: Exact(1), actual })`.
Already visited child nodes remain in the report.

## 7. Accepted and detached worlds

The builtin constructor table and fixed authored entity-family inventory are
language-owned and available in both worlds.

- detached `Ref<Character>` and detached `Ref<Flow>` are `Complete`;
- detached bare `Character` or `Flow` outside an entity-family slot is
  `DetachedUnavailable` unless an explicit detached-only open rule owns it;
- detached `Ref<Missing>` records the child as `DetachedUnavailable`, returns a
  `Detached` report, and does not pretend success;
- a project-only name in detached resolution is unavailable, not wrong-kind,
  because the project world is absent and its kind cannot be proven;
- accepted-world project/external/accepted/open ordinary types are proven and
  therefore produce wrong-kind when supplied to `Ref`.

## 8. Collision and registration policy

Direct `Ref` is reserved at every existing registration boundary.

- HIR project nominal declaration named `Ref` → existing
  `ProjectSymbolLinkError::ReservedTypeName` /
  `aw.project.symbol.reserved_type_name`.
- callable declaration named `Ref` where the existing type-name reservation
  applies → same existing error.
- direct one-segment external type binding/import alias named `Ref` → same
  existing error.
- exact accepted record at implicit-crate `Ref` →
  `AcceptedNominalCatalogError::ReservedPath`.
- open exact pattern `Ref` → `InvalidOpenPattern` with `ReservedPath`.
- open namespace prefix `Ref` → `InvalidOpenPattern` with `ReservedPath`.
- qualified `pkg::Ref` may exist; it is selected only by its qualified path and
  never shadows direct `Ref`.

No permanent “removed spelling” diagnostic is added. Collision rejection is
registration-time ownership enforcement, not a resolver fallback.

## 9. Consumer contract

Every consumer uses the checked result from the existing recursive resolver
and accepted `NominalResolutionIndex`.

- **normal checker:** field, alias, function, flow, contract, and generic type
  positions consume `TypeKind::Ref`; no local `TypeRef` conversion.
- **callable schemas:** `ProjectSignatureResolver` keeps using
  `TypeResolutionInput::accepted`, the shared cache, and the shared index.
  Parameters and returns can carry `TypeKind::Ref` directly.
- **entry contracts:** entry-bound callable types can carry `TypeKind::Ref`.
  The persisted data-shape adapter remains a projection over checked types and
  never resolves `Ref` by spelling.
- **project semantic index:** checked callable/type entries may contain
  `TypeKind::Ref`. Valid contextual nodes create no project-nominal reference
  edge. A project nominal supplied illegally as the argument has its child
  outcome replaced with `Failed`, so it also creates no rename edge.
- **LSP:** hover and completion use typed builtin/entity-family inventories and
  node facts. Definition and rename ownership are specified in the tooling
  document; no source identity is fabricated for language atoms.
- **runtime-plan/verify:** consume `TypeKind::Ref(EntityType)` as the existing
  semantic reference type. No display-string conversion is permitted.
- **bytecode/persistent digest:** the current authored `TypeRef` structural
  digest already records generic base and child. No new `TypeKind` wire table,
  schema version, compatibility reader, or fallback is introduced here.
- **save/replay:** `arcweft_data::TypeShape` has no entity-reference variant.
  This correction does not make `TypeKind::Ref` a persisted data shape and
  must not encode it as `TypeShape::Named`. Persisted entry state/event fields
  containing `Ref` remain a typed, deterministic unsupported-shape error until
  a separate versioned wire contract exists. Therefore no `Ref` save/replay
  round trip crosses this boundary in this slice.

## 10. Speaker and SpeakerPreset

`Speaker<T>` and `SpeakerPreset<T>` use the **identical argument projection
rule**, not merely the same child carrier:

- same owner enum;
- same exact arity `1`;
- same `EntityFamily` expectation at index `0`;
- same direct authored inventory;
- same source-node facts;
- same diagnostic code, subject shape, poison behavior, and work accounting;
- same detached behavior.

Only `project_entity_family`’s final result differs.

## 11. Determinism and work accounting

Keep the existing accounting model:

- one work unit for each ordinary visited type node;
- two for an alias-target node;
- existing project candidate, accepted/external scan, open-rule scan, alias
  substitution, limit, and diagnostic accounting is unchanged;
- builtin lookup and entity-family projection add no scan charge.

With an empty accepted catalog/open-rule set and no project candidate work:

- `Ref<Character>` / `Ref<Flow>` → `work_charged == 2`;
- `Ref<String>` / `Ref<3>` → `work_charged == 2`;
- bare `Ref` → `work_charged == 1`;
- `Ref<Character, String>` → `work_charged == 3`;
- `Ref<ProjectType>` with one selected project nominal → `work_charged == 3`.

Repeated resolution of the same accepted cache key must produce byte-for-byte
equal reports. Diagnostics, poisons, unavailable paths, nodes, and aliases
remain sorted/deduplicated by the existing rules.

## 12. Completion criteria

Implementation is complete only when all rows in `TEST_MATRIX.csv` pass, the
normal and Tier 2 validation slices in `IMPLEMENTATION_ORDER.md` pass, and the
last context-free `TypeRef -> TypeKind` branch that recognizes `Ref` by
spelling is deleted. A partial implementation, compatibility fallback, or
successful result synthesized after a failed checked resolution is not this
contract.
