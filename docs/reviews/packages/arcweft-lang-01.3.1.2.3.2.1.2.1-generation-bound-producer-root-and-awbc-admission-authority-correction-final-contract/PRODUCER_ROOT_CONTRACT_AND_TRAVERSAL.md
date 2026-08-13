# Producer root contract, authorization closure, and traversal

## 1. Independent evidence

A producer contract has two independent components:

- payload roots: semantic evidence of values the producer is allowed to own;
- claimed authorization keys: a canonical cached projection used for codec
  stability and deterministic diagnostics.

Admission derives the key closure from payload roots. It never seeds traversal
from the claimed keys. Therefore a malicious raw artifact cannot authorize a
new layout merely by inserting a producer/key row.

## 2. Root sources

Project roots are emitted from accepted executable facts: entry/callable
signatures, frame slots, constants, root resources, root reducers, View inputs,
save/replay-visible slots, and every other typed runtime publication boundary.

Generic producer roots are emitted from one typed accepted producer payload
fact and identified by `RuntimeProducerRootId`.

CharacterDialogue roots are generated from its typed payload declaration:

- Stage, Portrait, Focus, Cleanup, Hook, Style, RichText;
- every custom field in ascending field-ID order.

There is no root for the top-level CharacterDialogue opaque payload itself,
because core treats it as an atomic producer-owned value. Its nested role/custom
values are the roots that authorize nominal construction.

## 3. Fixed limits

The final constants are:

| Limit | Value |
|---|---:|
| generation project roots | 65,536 |
| generation producers | 1,024 |
| roots per generic producer | 4,096 |
| CharacterDialogue custom fields | 4,096 |
| accepted Views per custom field | 256 |
| nominal catalog layouts | 65,536 |
| checked-type nesting depth | existing `MAX_RUNTIME_VALUE_NESTING_DEPTH` |
| traversal work units per generation | 1,048,576 |
| traversal work units per root | 65,536 |

A work unit is one checked-type node, one nominal field edge, or one variant
case edge. Limit failure retains root coordinate and checked-type path.

## 4. Canonical root ordering

- project roots: ascending `RuntimeProjectRootId`;
- producers: ascending `RuntimeOpaqueTypeProducerId`;
- generic roots: ascending `RuntimeProducerRootId`;
- CharacterDialogue roles: enum order;
- custom fields: ascending `CharacterDialogueCustomFieldId`;
- claimed authorization keys: ascending catalog key.

Decode rejects non-canonical order and duplicates before traversal. Checked
constructors sort trusted projection output and then encode it canonically.

## 5. Iterative traversal

Traversal uses an explicit stack; it does not recurse through untrusted depth.
Each stack item contains root coordinate, checked-type path, and expected type.

Children are pushed in reverse of their normative order so processing observes:

1. `Sequence`, `Option`: only child;
2. `Result`: ok, error;
3. `Tuple`: ascending ordinal;
4. `Choice`: source order;
5. `Variant`: ascending case ordinal; payload child when present;
6. `Nominal`: catalog lookup, then fields in defining layout order;
7. primitive/scalar/bytes/entity: no child;
8. `Opaque`: validate closed owner, then stop.

`Never` is allowed as a closed type but contributes no key and accepts no
runtime value.

## 6. Choice and variant traversal

Every Choice alternative is traversed. Authorization is the union of all
alternatives; admission never selects one branch based on a sample value.

Every nominal variant case payload is traversed in ordinal order. The case
owner's nominal/semantic identity is not a nominal-record catalog key unless a
nested `RuntimeCheckedType::Nominal` occurs in a payload.

Option and Result use their intrinsic case grammar but traverse only their
generic payload checked types.

## 7. Nominal node processing

For `RuntimeCheckedType::Nominal`:

1. form exact catalog key from nominal, semantic identity, and layout;
2. require one matching canonical layout descriptor;
3. insert the key into the current root closure;
4. if this exact key was already fully visited for the current traversal,
   do not descend again;
5. otherwise descend into every layout field checked type in defining order.

A nominal cycle terminates through the visited set. Conflicting same-key
descriptors fail during catalog validation before traversal.

## 8. Opaque handling

`RuntimeCheckedType::Opaque` is atomic to core. Admission validates:

- producer ID syntax;
- exact or producer-wide admission tag;
- semantic identity bytes.

It does not inspect an opaque payload or infer producer roots from it. A
producer-wide checked type contributes no nominal key and cannot wrap a
concrete value.

CharacterDialogue is special only because its typed producer declaration
publishes independent nested roots; core still does not descend through the
opaque value.

## 9. Derived and claimed closure equality

For each producer, compare the sorted derived set with the claimed set using a
merge walk:

- first derived-only key -> `MissingAuthorization`;
- first claimed-only key -> `ExtraAuthorization`;
- equality -> continue.

The first error is ordered by catalog key. Duplicate claimed keys are rejected
before this comparison.

## 10. Global catalog reachability

After project and producer traversal:

```text
reachable = project_closure UNION every producer_closure
```

The catalog layout-key set must equal `reachable`.

- first reachable key absent from catalog -> `MissingLayout`;
- first catalog key absent from reachable -> `UnreachableLayout`.

A key mentioned only by a claimed authorization set is not in `reachable`.

## 11. Producer lookup

`producer_shape(id)` is evaluated only after whole-generation admission.
Unknown producer fails typed lookup. The returned view contains the exact
derived set, not the raw claimed slice.

A required nominal key is resolved by nominal ID, semantic identity, layout,
then producer membership. Missing, stale semantic, stale layout, and wrong
producer retain the `.1.2` deterministic error order.

## 12. Runtime value validation

For an admitted shape view and expected checked type:

- primitives and composites follow exact closed structure;
- Choice requires exactly one successful branch;
- Variant checks owner/ordinal/name/presence/payload;
- Opaque checks owner and remains atomic;
- Nominal obtains a per-record shape, validates identity/layout/count/field IDs/
  first field predicate, and recursively validates fields.

All failures retain `RuntimeValuePath`. No boolean-only acceptance method is
used where typed evidence is required.
