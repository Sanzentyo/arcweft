# Authority and catalog contract

## 1. Layer ownership

| Concern | Sole owner | Explicitly not the owner |
|---|---|---|
| Source field order and generic substitution | HIR/sema accepted nominal declaration | dialogue, core value |
| Canonical layout hash algorithm | existing `RuntimeTypeSchema::try_layout_hash` compiler projection | layout catalog, dialogue, producer |
| Executable field descriptor | `arcweft_core::value::RuntimeNominalRecordLayout` | `TypeLayoutHash`, names, schema strings |
| Generation catalog declaration | compiler/runtime-plan bridge from accepted semantic facts | external producer, restore code |
| Whole-plan admission | `arcweft_core::plan::RuntimePlan::try_admit` | individual value constructors |
| External producer authorization | non-Serde `RuntimeNominalRecordProducerAdmission` | producer ID string alone |
| Checked nominal publication | non-Serde `RuntimeNominalRecordAdmission::try_construct` | public layout/scalar constructor |
| CharacterDialogue payload validation | `CharacterDialogueRuntimeSchema` with active producer capability | core opaque implementation |

## 2. Canonical projection flow

```text
accepted nominal declaration + substituted checked field types
  -> transient RuntimeTypeSchema::Record
  -> RuntimeTypeSchema::try_layout_hash()              [existing sole hash]
  -> RuntimeNominalRecordLayout::try_from_checked_projection(...)
  -> RuntimeResolvedNominalRecord
  -> semantic-facts Arc interning by (nominal, semantic, layout)
  -> generation RuntimeNominalRecordCatalogDeclaration
  -> raw RuntimePlan (quarantine / Serde)
  -> RuntimePlan::try_admit
  -> RuntimeNominalRecordCatalog (operational, non-Serde)
```

The semantic-facts interning map at the pinned source currently canonicalizes
`Arc<RuntimeNominalRecordLayout>` and then drops the map. The implementation
retains that accepted map long enough to emit the generation catalog. It does
not reproject names or schemas later.

## 3. Catalog declaration invariants

A declaration is canonical only when:

1. layout rows are sorted by `RuntimeNominalRecordCatalogKey`;
2. a key appears once;
3. the descriptor's nominal, semantic identity, and layout equal its key;
4. equal keys have structurally equal defining-order fields;
5. producer rows are sorted by producer, then key;
6. a producer/key pair appears once;
7. every producer key resolves to the exact canonical layout row;
8. every layout is reachable from a verified plan carrier or a producer row;
9. every producer row is emitted from accepted closed checked-type facts for
   that producer; and
10. no row is reconstructed from nominal text, schema text, source names,
    semantic digest bytes, or layout hash alone.

The declaration carries a canonical descriptor once. Producer rows carry keys,
not field copies. Operational admission moves/interns those layouts into one
`Arc` map. Pointer identity is never semantic.

## 4. Whole-plan admission

`RuntimePlan::try_admit(self)` is consuming and performs this order:

1. existing complete plan/expression/entry verification;
2. collect every nominal descriptor referenced by expressions, patterns,
   checked entry roles, AWBC tables, root/replay contracts, and declared opaque
   producer payload contracts;
3. validate catalog sort/uniqueness and descriptor/key scalar equality;
4. reject structural conflicts for equal keys;
5. reject missing and unreachable catalog rows;
6. resolve every external producer/key authorization;
7. rebuild the operational catalog and restore `Arc` sharing;
8. return `AdmittedRuntimePlan`.

`RuntimePlan::verify` may remain a non-publishing diagnostic check, but no
runtime entry point accepts its success as executable authority. Only
`try_admit` returns the wrapper and catalog.

## 5. Why the capability is not an identity mint

The value handle contains a borrow of a catalog entry and has no public fields,
constructor, `Default`, or Serde implementation. Its `try_construct` method:

1. obtains the exact catalog-owned layout;
2. validates count and defining-order predicates;
3. calls crate-private `RuntimeNominalRecordValue::try_from_accepted_layout`;
4. returns a value containing only the descriptor's nominal ID/layout and the
   admitted fields.

It accepts no nominal ID, semantic ID, layout hash, or descriptor argument.
Thus an independently fabricated `RuntimeNominalRecordLayout` cannot be used to
publish a value. A raw serialized plan is untrusted data; it gains authority
only through whole-plan admission. This is an in-process typed authority, not a
new cryptographic artifact-signature mechanism.

## 6. Producer authorization

A producer capability is bound to one exact
`RuntimeOpaqueTypeProducerId` and one plan generation. `require(key)` succeeds
only if:

- the producer is admitted;
- the global catalog contains the exact key; and
- the producer authorization set contains that key.

CharacterDialogue receives the capability for `std.character_dialogue` from
the runtime driver after plan admission. Its schema constructor compares that
producer ID to the inherent canonical producer and preflights all nominal nodes
reachable from role/custom checked types. The schema never asks a global
registry by string.

Project/core evaluation uses `require_project`, which can access canonical
entries referenced by verified project carriers without an opaque producer
row. This is a separate admission domain represented in the handle; it is not a
fallback from failed external lookup.

## 7. Lookup classification

For a producer-bound expected nominal triple, lookup classifies in this order:

1. producer absent -> `ProducerNotAdmitted`;
2. nominal absent globally -> `Missing`;
3. nominal present but expected semantic identity absent ->
   `StaleSemanticIdentity`;
4. nominal+semantic present but expected layout absent -> `StaleLayout`;
5. exact key globally present but not in producer set -> `WrongProducer`;
6. exact key and authorization present -> issue handle.

Conflicting descriptors cannot reach lookup: they fail whole-plan admission.

## 8. Catalog-aware checked-tree validation

Validation always starts from an active expected `RuntimeCheckedType`.
Composite traversal is pre-order and left-to-right. For a nominal node it first
obtains a handle, then calls `validate_against_layout`, then recursively
validates each field under the descriptor's defining-order checked type.

Opaque checked types are atomic to this validator. It checks owner admission
but does not descend into an opaque payload; the registered producer validates
that payload at its domain boundary. CharacterDialogue is such a producer.

Choice alternatives are tried in declared source order. A shallow-incompatible
alternative is skipped. The first fully valid alternative wins. If none is
valid, the error is the first fully attempted typed failure; if no alternative
is shallow-compatible, the result is `CheckedType` at the current path. This
rule is deterministic and does not accept a value merely because any branch
has a matching nominal name.

## 9. `RuntimeCheckedType::Variant` correction

The pinned implementation's nominal `Variant` branch accepts only by owner and
does not check ordinal, case name, payload presence, or payload type. That is a
concrete defect for inline failure and other closed variants.

The existing inherent `RuntimeCheckedType::accepts_value` implementation is
extended in place. For `Variant` it requires:

1. exact nominal/semantic owner;
2. ordinal within `cases`;
3. exact case name at that ordinal;
4. payload absence/presence equal to the case declaration; and
5. recursive payload acceptance when present.

No extension trait, free helper, or dialogue-only variant checker becomes a
second checked-type authority.
