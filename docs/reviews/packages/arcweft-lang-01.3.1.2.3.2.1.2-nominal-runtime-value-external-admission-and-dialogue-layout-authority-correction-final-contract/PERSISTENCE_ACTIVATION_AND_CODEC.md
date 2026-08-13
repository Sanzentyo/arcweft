# Persistence, activation, and codec contract

## 1. Raw versus admitted types

- `RuntimePlan` remains the interim Serde/wire carrier and is quarantined.
- `AdmittedRuntimePlan` is operational, non-Serde, and required by runtime
  constructors/activation.
- `RuntimeValue` may be deserialized as raw data, but a live slot/root/dialogue
  wrapper is not created until validation against the admitted generation.
- `CharacterDialogueTypedValue`, role wrappers, `CharacterDialogueValue`,
  catalog handles, and producer handles are live admitted types and do not
  implement `Deserialize`.

No API treats successful Serde decoding as semantic admission.

## 2. Plan and bundle ingress

```text
AWFB/logical bundle bytes
  -> container/section decode and existing integrity checks
  -> raw RuntimePlan + raw RuntimeValue-bearing resources
  -> RuntimePlan::try_admit
  -> AdmittedRuntimePlan + RuntimeNominalRecordCatalog
  -> resolve generation role/slot/View/dialogue expected checked types
  -> validate raw values
  -> construct session/runtime generation
```

A plan catalog conflict/missing producer row fails before executable selection.
A bundle does not reconstruct descriptors from entry schemas or layout hashes.

## 3. CharacterDialogue wire ingress

Persisted CharacterDialogue is a raw `RuntimeValue::Opaque`. The active runtime
constructs `CharacterDialogueRuntimeSchema` from accepted catalogs, role/custom
types, and the admitted `std.character_dialogue` producer capability, then
calls `try_decode_opaque`.

The result is published only after exact producer/semantic identity, payload
shape, nested descriptors, checked types, domain catalogs/digests, limits, and
canonical form pass. No old nominal root/custom/inline reader remains.

## 4. Typed role/custom value ingress

Persistence encodes a role/custom field as its `RuntimeValue` inside its owning
wire envelope. Decode does not invoke `CharacterDialogueTypedValue::deserialize`
(the implementation is deleted). It resolves the active role/custom descriptor
and calls exactly one schema admission method:

- `try_admit_stage_value`;
- `try_admit_portrait_value`;
- `try_admit_focus_value`;
- `try_admit_cleanup_value`;
- `try_admit_hook_value`;
- `try_admit_style_value`;
- `try_admit_rich_text_value`; or
- `try_admit_custom_value(field_id, value)`.

Each method normalizes with its expected type, validates the complete tree,
applies role limits, and returns the live wrapper.

## 5. Session/save restore

Restore order is fixed:

1. validate artifact/generation identity;
2. admit the active plan;
3. map each saved binding/root/View/dialogue value to its active checked owner;
4. validate nominal trees and opaque producer payloads;
5. validate save-domain counters/contracts;
6. perform ownership/nesting traversal;
7. construct executor/View/session state; and
8. activate.

A saved nominal value never carries its own executable descriptor. The active
plan supplies it. A saved CharacterDialogue exact owner does not replace the
active producer schema.

## 6. Root and replay

Root event/command/state and replay outcomes retain their existing typed owner
or slot identity. Replay first maps that identity to the active checked type,
validates the value tree, and then compares/executes the transition. Missing or
stale descriptors fail before ownership traversal and before recorded domain
outcomes are applied.

No replay fallback compares only nominal ID/layout or reconstructs from a
record name.

## 7. View runtime and presentation

View parameter/state/dialogue inputs are admitted against the active generated
View input type before mounting or restoring a View occurrence. A
CharacterDialogue View input is decoded through the dialogue schema; other
nested nominal values use the project or appropriate producer capability.

Agent, CLI, headless, native, Web, and runtime-accelerator projections may
inspect admitted values through accessors. They may not reproduce nominal
values from stored scalars or clone opaque payloads into a new owner without
producer validation.

## 8. A4 versus A6

A4 contains:

- catalog and producer capabilities;
- `AdmittedRuntimePlan` and mandatory activation signatures;
- CharacterDialogue representation replacement;
- role/custom live admission;
- descriptor-aware normalize/clear/patch;
- restore/replay/bundle validation hooks; and
- deletion of unchecked constructors and old readers.

A6 contains:

- final exhaustive canonical-byte/golden fixture updates;
- full bundle/save/AWBC cross-product and tamper fixtures; and
- codec audit closure.

A4 may add focused byte tests needed to prove no unchecked interval, but it does
not claim A6 complete. A6 cannot defer an A4 semantic validation or preserve an
old representation reader.

## 9. Version decision

Every Arcweft-owned schema, ABI, codec, digest-domain, protocol, session-save,
root-replay, AWBC, bundle, and CharacterDialogue version remains exactly `1`.
The unreleased representation is replaced directly. No version discriminator,
migration branch, dual reader, or compatibility writer is added.
